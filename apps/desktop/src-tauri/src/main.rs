mod app_server_bridge;
mod asset_render_compat;
mod c110g_packaged_probe;
mod deepseek_delta_acceptance_probe;
mod deepseek_forge_visual_acceptance_probe;
mod deepseek_mvp_acceptance_probe;
mod deepseek_provider;
mod k003_packaged_probe;
mod local_universal_provider;
mod mvp_arm_packaged_probe;
mod mvp_arm_provider;
mod provider_credentials;
mod rust_core_runtime;
mod rust_product_catalog;
mod vision_evidence_adapter;

use std::{
    collections::{BTreeMap, HashMap},
    env,
    ffi::OsStr,
    fs::{self, OpenOptions},
    io::{Read, Write},
    net::{SocketAddr, TcpStream},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{Arc, Condvar, Mutex, OnceLock},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

#[cfg(unix)]
use std::os::unix::{fs::PermissionsExt, process::CommandExt};

use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use forgecad_app_server::{
    compatibility::{AllowedHttpMethod, LocalAgentEndpoint, PreparedCompatHttpRequest},
    BudgetedVisualReferenceComparisonProvider, CancellationToken, VisionEvidenceCoordinator,
    VisionEvidenceImage, VisionEvidenceProviderRequest,
};
use forgecad_app_server_protocol::{
    AgentTurn, AgentTurnStatus, CompatHttpResponse, ProtocolHttpBody, TurnCommandOutcome,
    TurnCommandResult,
};
use forgecad_core::{
    c111b_visual_reference_acceptance_policy_for_domain, semantic_sha256, MultimodalDesignRequest,
    ReferenceEvidenceKind, SurfaceAdornmentProgram, VisualEvidenceGraph,
    VISUAL_REFERENCE_COMPARISON_AUTHORIZATION_LIFETIME_MS,
    VISUAL_REFERENCE_COMPARISON_MAXIMUM_CALLS,
    VISUAL_REFERENCE_COMPARISON_MAXIMUM_VARIABLE_COST_MICROUSD,
};
use serde::Deserialize;
use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tauri::{Manager, State};

use app_server_bridge::{
    forgecad_candidate_pbr_capture_issue, forgecad_candidate_pbr_capture_resume,
    forgecad_candidate_pbr_capture_submit, forgecad_protocol_connect, forgecad_protocol_disconnect,
    forgecad_protocol_send, AppServerBridge,
};
use deepseek_provider::{
    DeepSeekPricing, DeepSeekProviderClient, DeepSeekProviderConfig, ReqwestDeepSeekTransport,
};
use forgecad_app_server::ProviderClient;
use mvp_arm_provider::{LocalRoboticArmMvpProvider, MVP_MODEL};
use local_universal_provider::{
    LocalUniversalVisualAuthorProvider, LOCAL_UNIVERSAL_ENV, LOCAL_UNIVERSAL_MODEL,
};
use provider_credentials::{
    validate_provider_config_input, ProviderConfigMetadata, ProviderCredentialStore,
};
use rust_core_runtime::RustCoreRuntime;
use vision_evidence_adapter::{
    OpenAiCompatibleVisionEvidenceAdapter, PrivateFileVisionEvidenceCredentialStore,
    ReqwestVisionEvidenceTransport, VisionEvidenceConfigMetadata,
};
use zeroize::Zeroize;

const AGENT_HOST: &str = "127.0.0.1";
const AGENT_PORT: u16 = 8000;
const AGENT_MODE_PACKAGED: &str = "packaged-sidecar";
const AGENT_MODE_LOCAL: &str = "local-dev-python";
const K001_PACKAGED_PROBE_SCHEMA: &str = "ForgeCADK001PackagedProbe@1";
const K001_PACKAGED_PROBE_MARKER: &str = "ForgeCAD K001 packaged WebView probe report=";
const K002_PACKAGED_PROBE_SCHEMA: &str = "ForgeCADK002PackagedProbe@1";
const K002_PACKAGED_PROBE_MARKER: &str = "ForgeCAD K002 packaged WebView probe report=";
const ARM_WEBVIEW_QA_SCHEMA: &str = "ForgeCADArmWebViewQa@1";
const ARM_WEBVIEW_QA_MARKER: &str = "ForgeCAD mechanical-arm packaged WebView QA report=";
const ARM_WEBVIEW_QA_PROGRESS_MARKER: &str =
    "ForgeCAD mechanical-arm packaged WebView QA progress=";
const ARM_WEBVIEW_QA_CAPTURE_MAX_PNG_BYTES: usize = 8 * 1024 * 1024;
const ARM_WEBVIEW_QA_CAPTURE_MAX_GLB_BYTES: usize = 16 * 1024 * 1024;
const C111B_PACKAGED_WEBGL_SCHEMA: &str = "C111BPackagedWebGL@1";
const C111B_PACKAGED_WEBGL_MARKER: &str = "ForgeCAD C111B packaged WebGL QA report=";
const C111B_PACKAGED_WEBGL_PROGRESS_MARKER: &str = "ForgeCAD C111B packaged WebGL QA progress=";
const C111B_PACKAGED_WEBGL_PRODUCTION_SHA256: &str =
    "48ccc5c6a725936d43cb731ed5e20b93f10ef751712ed79469ea406318160b6b";
// C111B Agent packaged QA runs the reviewed ForgeVisualProgram authoring seam.
// This is the canonical persisted ShapeProgram lowering for that seam.  It is
// deliberately distinct from the Recipe expansion SHA (9016...1424) recorded
// in the C111A inventory before ForgeVisualProgram materialization.
const C111B_PACKAGED_WEBGL_VISUAL_PROGRAM_SHAPE_SHA256: &str =
    "f7077b747530d660b0bfb2c91f10610e9626d4a071b05ad6d9f8dc2da274d3ef";
const C111B_PACKAGED_WEBGL_TRIANGLES: u64 = 138_248;
const C111B_PACKAGED_WEBGL_PRIMITIVES: u64 = 157;
const C111B_PACKAGED_WEBGL_MATERIALS: u64 = 12;
// Retaining the reviewed A005 program does not change geometry, but it appends
// the exact dynamic PBR rows used by the committed V2 material zones.  The
// resulting GLB hash is action-lineage dependent, so it is carried from the
// initial process into restart instead of being confused with the frozen V1
// production hash above.
const C111B_PACKAGED_WEBGL_AGENT_V2_MATERIALS: u64 = 14;
const C111B_PACKAGED_WEBGL_AGENT_V2_COMPLETE_PBR_MATERIALS: u64 = 12;
const C111B_PACKAGED_WEBGL_PBR_RENDERER_ID: &str = "forgecad-workbench-pbr@1";
const C111B_PACKAGED_WEBGL_PBR_RENDER_MANIFEST_SHA256: &str =
    "024d7e8f707c75eafd12f22e9a5e9f9c5ab0fcbd1a6ce1a4de6726ace7b2451a";
const C111B_PACKAGED_WEBGL_PBR_VISUAL_ENVIRONMENT_ID: &str = "env_forgecad_room_studio_v2";
const C111B_PACKAGED_WEBGL_PBR_VISUAL_ENVIRONMENT_SHA256: &str =
    "0884e4f7b32c11ce94b4d406260f9ea89ca0c7933e0088d14e9eb89f382508a4";
const C111B_PACKAGED_WEBGL_CAPTURE_MAX_PNG_BYTES: usize = 8 * 1024 * 1024;
const C111B_PACKAGED_WEBGL_AUXILIARY_MAX_PNG_BYTES: usize = 8 * 1024 * 1024;
const C111B_PACKAGED_WEBGL_SOURCE_MAX_BYTES: usize = 48 * 1024 * 1024;
const RESTRICTED_GEOMETRY_CAPABILITY_HEADER: &str = "X-ForgeCAD-Restricted-Geometry-Capability";
const RESTRICTED_GEOMETRY_OWNERSHIP_PATH: &str = "/api/v1/internal/geometry/capability/ownership";
const SIDECAR_SUPERVISOR_SESSION_ENV: &str = "FORGECAD_SUPERVISOR_SESSION_ID";
const MANAGED_SIDECAR_LEASE_SCHEMA: &str = "ForgeCADManagedSidecarLease@1";
// Deterministic budget-accounting coefficients, not a claim about current
// DeepSeek billing. The per-Turn cost gate remains an explicit conservative
// policy even when external prices change.
const K002_INPUT_BUDGET_MICROUSD_PER_MILLION_TOKENS: u64 = 1_000_000;
const K002_OUTPUT_BUDGET_MICROUSD_PER_MILLION_TOKENS: u64 = 4_000_000;

struct AgentProcessState {
    child: Mutex<Option<Child>>,
    mode: Mutex<String>,
    internal_capability_token: String,
    supervisor_session_id: String,
    provider_credentials: Arc<ProviderCredentialStore>,
}

#[derive(Clone)]
struct C111bPackagedQaMetricsState {
    local_mvp_provider: Option<Arc<LocalRoboticArmMvpProvider>>,
    timeline: Arc<Mutex<C111bPackagedQaTimeline>>,
}

#[derive(Debug)]
struct C111bPackagedQaTimeline {
    started: Instant,
    stages: Vec<(String, u64)>,
}

struct NativeProviderClientBundle {
    client: Arc<dyn ProviderClient>,
    local_mvp_provider: Option<Arc<LocalRoboticArmMvpProvider>>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
struct ManagedSidecarLease {
    schema_version: String,
    supervisor_session_id: String,
    desktop_pid: u32,
    sidecar_process_group_id: u32,
}

#[derive(Debug, PartialEq, Eq)]
struct ForgecadSidecarIdentity {
    supervisor_session_id: String,
    process_group_id: u32,
}

struct VisualProviderState {
    repository: Arc<forgecad_core::CoreRepository>,
    vision_credentials: Arc<PrivateFileVisionEvidenceCredentialStore>,
    vision_coordinator: VisionEvidenceCoordinator,
    vision_active_requests: Mutex<HashMap<String, CancellationToken>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SaveVisionEvidenceProviderConfigRequest {
    base_url: String,
    model: String,
    api_key: String,
}

impl Drop for SaveVisionEvidenceProviderConfigRequest {
    fn drop(&mut self) {
        self.api_key.zeroize();
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AnalyzeVisualEvidenceRequest {
    client_request_id: String,
    request: MultimodalDesignRequest,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AnalyzeVisualEvidenceResult {
    /// Rust-normalized request carrying semantic hashes of the immutable
    /// ReferenceEvidence records. The UI must bind this exact request, rather
    /// than its pre-analysis draft, into turn/start.
    request: MultimodalDesignRequest,
    visual_evidence_graph: VisualEvidenceGraph,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AuthorizeVisualReferenceComparisonRequest {
    client_request_id: String,
    request: MultimodalDesignRequest,
    visual_evidence_graph: VisualEvidenceGraph,
    maximum_calls: u8,
    maximum_variable_cost_microusd: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AuthorizeVisualReferenceComparisonResult {
    authorization_id: String,
    authorization_binding_sha256: String,
    expires_at_unix_ms: i64,
    maximum_calls: u8,
    maximum_variable_cost_microusd: u64,
}

/// Explicit user approval for exactly one already-captured category-open
/// candidate. The request carries no model payload: Rust derives the complete
/// comparison scope from the live execution and sealed evidence.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AuthorizeCandidatePbrVisualComparisonRequest {
    client_request_id: String,
    execution_id: String,
    project_id: String,
    turn_id: String,
}

static K001_PACKAGED_PROBE_COMPLETION: OnceLock<(Mutex<bool>, Condvar)> = OnceLock::new();
static K002_PACKAGED_PROBE_COMPLETION: OnceLock<(Mutex<bool>, Condvar)> = OnceLock::new();

pub(crate) fn wait_for_k001_packaged_probe_if_enabled() {
    if env::var("FORGECAD_K001_PACKAGED_PROBE").as_deref() != Ok("1") {
        return;
    }
    let (lock, condition) =
        K001_PACKAGED_PROBE_COMPLETION.get_or_init(|| (Mutex::new(false), Condvar::new()));
    let Ok(completed) = lock.lock() else {
        return;
    };
    let _ = condition.wait_timeout_while(completed, Duration::from_secs(30), |done| !*done);
}

fn signal_k001_packaged_probe_completion() {
    let (lock, condition) =
        K001_PACKAGED_PROBE_COMPLETION.get_or_init(|| (Mutex::new(false), Condvar::new()));
    if let Ok(mut completed) = lock.lock() {
        *completed = true;
        condition.notify_all();
    }
}

pub(crate) fn wait_for_k002_packaged_probe_if_enabled() {
    if env::var("FORGECAD_K002_PACKAGED_PROBE").as_deref() != Ok("1") {
        return;
    }
    let (lock, condition) =
        K002_PACKAGED_PROBE_COMPLETION.get_or_init(|| (Mutex::new(false), Condvar::new()));
    let Ok(completed) = lock.lock() else {
        return;
    };
    let _ = condition.wait_timeout_while(completed, Duration::from_secs(30), |done| !*done);
}

fn signal_k002_packaged_probe_completion() {
    let (lock, condition) =
        K002_PACKAGED_PROBE_COMPLETION.get_or_init(|| (Mutex::new(false), Condvar::new()));
    if let Ok(mut completed) = lock.lock() {
        *completed = true;
        condition.notify_all();
    }
}

// The K003 worker must observe a probe only after its report has passed the
// contract checks and has been appended to the bounded supervisor log.  Keep
// this ordering explicit and testable; signaling on receipt lets K003 race a
// still-invalid or not-yet-recorded K001/K002 report.
fn finish_packaged_probe_report<T, Validate, Signal>(
    validate_and_record: Validate,
    signal: Signal,
) -> Result<T, String>
where
    Validate: FnOnce() -> Result<T, String>,
    Signal: FnOnce(),
{
    let result = validate_and_record();
    signal();
    result
}

fn build_native_provider_client(
    credentials: Arc<ProviderCredentialStore>,
) -> Result<NativeProviderClientBundle, String> {
    if local_universal_author_enabled() {
        let provider = Arc::new(LocalUniversalVisualAuthorProvider::new());
        let client: Arc<dyn ProviderClient> = provider;
        return Ok(NativeProviderClientBundle {
            client,
            local_mvp_provider: None,
        });
    }
    if mvp_offline_arm_enabled() {
        let local_mvp_provider = Arc::new(LocalRoboticArmMvpProvider::new());
        let client: Arc<dyn ProviderClient> = local_mvp_provider.clone();
        return Ok(NativeProviderClientBundle {
            client,
            local_mvp_provider: Some(local_mvp_provider),
        });
    }
    let pricing = DeepSeekPricing::new(
        K002_INPUT_BUDGET_MICROUSD_PER_MILLION_TOKENS,
        K002_OUTPUT_BUDGET_MICROUSD_PER_MILLION_TOKENS,
    )
    .map_err(|_| "ForgeCAD Provider budget policy is invalid.".to_string())?;
    let config = DeepSeekProviderConfig::bounded(pricing);
    let transport = ReqwestDeepSeekTransport::production(config.max_response_bytes)
        .map_err(|_| "ForgeCAD HTTPS Provider transport could not be initialized.".to_string())?;
    let client = DeepSeekProviderClient::new(credentials, Arc::new(transport), config)
        .map_err(|_| "ForgeCAD DeepSeek Provider client could not be initialized.".to_string())?;
    Ok(NativeProviderClientBundle {
        client: Arc::new(client),
        local_mvp_provider: None,
    })
}

fn local_universal_author_enabled() -> bool {
    let enabled = env::var(LOCAL_UNIVERSAL_ENV).as_deref() == Ok("1");
    if enabled {
        // Temporary local bring-up marker; removed after the packaged path is
        // verified so the runtime does not retain diagnostic filesystem I/O.
        let _ = fs::write(
            "/tmp/forgecad-local-provider-selected",
            LOCAL_UNIVERSAL_MODEL.as_bytes(),
        );
    }
    enabled
}

fn mvp_offline_arm_enabled() -> bool {
    env::var("FORGECAD_MVP_OFFLINE_ARM").as_deref() == Ok("1")
}

fn mvp_provider_config_with_runtime_status(
    internal_capability_token: &str,
) -> ProviderConfigMetadata {
    let supervisor_status = match probe_agent(internal_capability_token) {
        AgentProbe::Healthy => "running",
        AgentProbe::WrongService(_) | AgentProbe::CapabilityMismatch(_) => "mismatch",
        AgentProbe::Offline => "unavailable",
    };
    ProviderConfigMetadata {
        base_url: "local://forgecad-mvp-arm".into(),
        model: MVP_MODEL.into(),
        configured: true,
        storage: "rust-offline-deterministic".into(),
        credential_id: None,
        metadata_status: "ready".into(),
        secret_status: "not_required".into(),
        supervisor_status: supervisor_status.into(),
        capability_status: if supervisor_status == "running" {
            "ready".into()
        } else {
            supervisor_status.into()
        },
        failure_code: None,
    }
}

#[derive(Deserialize)]
struct SaveProviderConfigRequest {
    base_url: String,
    model: String,
    api_key: String,
}

impl Drop for SaveProviderConfigRequest {
    fn drop(&mut self) {
        self.api_key.zeroize();
    }
}

#[derive(Debug, Clone, Serialize)]
struct K001PackagedProbeConfig {
    schema_version: &'static str,
    phase: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    expected: Option<K001PackagedProbeExpected>,
}

#[derive(Debug, Clone, Serialize)]
struct K001PackagedProbeExpected {
    project_id: String,
    thread_id: String,
    asset_version_id: String,
    last_event_id: String,
    cursor: String,
    glb_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct K001PackagedProbeReport {
    schema_version: String,
    phase: String,
    ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    project_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    asset_version_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    first_event_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    last_event_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    cursor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    resume_from_event_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    resume_from_cursor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    glb_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    protocol_glb_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    resource_glb_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    notification_count: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    native_lifecycle_transport: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    native_item_replay_verified: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    product_state_owner: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    python_product_api_used: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    turn_status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    turn_error_code: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    provider_calls: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    error_code: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    diagnostic: Option<K001ProbeDiagnostic>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct K001ProbeDiagnostic {
    method: String,
    route: String,
    status: u16,
    error_code: String,
    phase: String,
    correlation_id: String,
}

#[derive(Debug, Clone, Serialize)]
struct K002PackagedProbeConfig {
    schema_version: &'static str,
    phase: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    expected: Option<K002PackagedProbeExpected>,
}

#[derive(Debug, Clone, Serialize)]
struct K002PackagedProbeExpected {
    thread_id: String,
    turn_id: String,
    items_sha256: String,
    item_count: u64,
    last_sequence: u64,
    turn_error_code: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct K002PackagedProbeReport {
    schema_version: String,
    phase: String,
    ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    turn_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    turn_status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    turn_error_code: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    provider_status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    provider_configured: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    provider_network_call_made: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    supervisor_running: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    supervisor_state: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    supervisor_managed_by_desktop: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    reasoning_content_present: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    legacy_lifecycle_post_status: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    provider_calls: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    item_count: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    last_sequence: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    item_sequences: Option<Vec<u64>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    item_ids: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    item_types: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    items_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    replay_items_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    error_code: Option<String>,
}

/// The WebView acceptance harness has an explicit, bounded report contract.
/// It is intentionally not part of the public app-server protocol: the only
/// caller is the packaged WebView and normal launches cannot even retrieve a
/// configuration.  Values are stable IDs and hashes only; no prompt, secret,
/// filesystem path or Item payload crosses this boundary.
#[derive(Debug, Clone, Serialize)]
struct ArmWebviewQaConfig {
    schema_version: &'static str,
    phase: String,
    reference_class: String,
    r007b_visual_evidence: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ArmWebviewQaReport {
    schema_version: String,
    phase: String,
    ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    project_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    turn_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    preview_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    preview_artifact_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    v1_asset_version_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    v2_asset_version_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    v3_asset_version_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    snapshot_revision: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    renderer_generation: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    active_webgl_contexts: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    production_glb_render_source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    a005_preview_seen: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    r007b_preview_seen: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    r007b_v3_confirmed: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    v3_glb_download_confirmed: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    v3_production_glb: Option<ArmWebviewQaGlbCapture>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    v3_viewport_screenshot: Option<ArmWebviewQaPngCapture>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    visual_fidelity_validated: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    restart_hydrated: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    r007b_visual_run: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    error_code: Option<String>,
}

/// The QA WebView may save two bounded binary artifacts only: its final
/// already-rendered viewport frame and the exact GLB Blob created by the
/// visible export action.  This is deliberately not a generic file-write API:
/// callers cannot select a path, URL, MIME type, or filename.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ArmWebviewQaCaptureRequest {
    schema_version: String,
    phase: String,
    kind: String,
    bytes_base64: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ArmWebviewQaPngCapture {
    relative_path: String,
    sha256: String,
    byte_size: u64,
    width: u32,
    height: u32,
}

#[derive(Debug, Clone, Serialize)]
struct C111bPackagedWebglQaConfig {
    schema_version: &'static str,
    phase: String,
    mode: String,
    source_sha256: &'static str,
    triangle_count: u64,
    primitive_count: u64,
    material_count: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    expected_project_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    expected_asset_version_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    expected_snapshot_revision: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    expected_export_sha256: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct C111bPackagedWebglQaSourceRequest {
    schema_version: String,
    include_bytes: bool,
}

#[derive(Debug, Clone, Serialize)]
struct C111bPackagedWebglQaSource {
    schema_version: &'static str,
    file_name: &'static str,
    sha256: String,
    byte_size: u64,
    triangle_count: u64,
    primitive_count: u64,
    material_count: u64,
    complete_pbr_material_count: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    bytes_base64: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct C111bPackagedWebglQaCaptureRequest {
    schema_version: String,
    phase: String,
    view_id: String,
    source_sha256: String,
    bytes_base64: String,
    auxiliary_width: u32,
    auxiliary_height: u32,
    auxiliary_pass_ids: Vec<String>,
    auxiliary_bytes_base64: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct C111bPackagedWebglQaRasterReadability {
    pixel_encoding: String,
    display_transfer: String,
    sample_pixel_count: u64,
    foreground_pixel_count: u64,
    foreground_coverage_bps: u64,
    foreground_median_luma: u64,
    foreground_readable_bps: u64,
    background_rgb: [u8; 3],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct C111bPackagedWebglQaCapture {
    view_id: String,
    relative_path: String,
    sha256: String,
    byte_size: u64,
    width: u32,
    height: u32,
    source_sha256: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    readability: Option<C111bPackagedWebglQaRasterReadability>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    auxiliary_width: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    auxiliary_height: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    auxiliary_pass_ids: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    auxiliary_byte_size: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    auxiliary_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    auxiliary_relative_path: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct C111bPackagedWebglQaReadbackRequest {
    schema_version: String,
    project_id: String,
    asset_version_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct C111bPackagedWebglQaReadback {
    schema_version: String,
    project_id: String,
    asset_version_id: String,
    source_sha256: String,
    shape_program_schema: String,
    external_reference: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    glb_byte_size: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    glb_triangle_count: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    glb_primitive_count: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    glb_material_count: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct C111bPackagedWebglQaReport {
    schema_version: String,
    phase: String,
    ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    project_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    asset_version_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    snapshot_revision: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    source_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    triangle_count: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    primitive_count: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    material_count: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    complete_pbr_material_count: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    renderer_generation: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    active_webgl_contexts: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    canvas_count: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    blockout_glb_kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    render_source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    light_preset: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    renderer_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    render_manifest_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    visual_environment_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    visual_environment_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    output_color_space: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    tone_mapping: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pbr_texture_count: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pbr_color_spaces: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pbr_sampling_valid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    captures: Option<Vec<C111bPackagedWebglQaCapture>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    readback: Option<C111bPackagedWebglQaReadback>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    formal_eligible: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    human_benchmark_evidence: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    reference_comparison: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    turn_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    provider_protocol_requests: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    product_tool_calls: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    input_tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    output_tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    prompt_cache_hit_tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    prompt_cache_miss_tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    same_intent_repair_attempts: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    same_intent_repairs_applied: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    provider_schema_repair_requests: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    product_tool_schema_repair_requests: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    estimated_cost_microusd: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    billable_variable_cost_microusd: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    billable_variable_cost_source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    turn_total_elapsed_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    turn_phase_timings_ms: Option<BTreeMap<String, u64>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    turn_trace_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    turn_metrics_source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    network_provider_calls: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    network_call_made: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    credential_reads: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    provider_metrics_source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    credential_metrics_source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    end_to_end_elapsed_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    stage_timings: Option<Vec<C111bPackagedQaStageTiming>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    timing_metrics_source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    restart_hydrated: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    error_code: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct C111bPackagedQaStageTiming {
    stage: String,
    elapsed_ms: u64,
    duration_since_previous_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ArmWebviewQaGlbCapture {
    relative_path: String,
    sha256: String,
    byte_size: u64,
    triangle_count: u64,
    complete_pbr_material_count: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
enum ArmWebviewQaCaptureReceipt {
    Png(ArmWebviewQaPngCapture),
    Glb(ArmWebviewQaGlbCapture),
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct R007BPackagedLineageRequest {
    schema_version: String,
    project_id: String,
    rebuild_plan_id: String,
    preview_change_set_id: String,
    confirmed_asset_version_id: String,
}

#[tauri::command]
fn forgecad_arm_webview_qa_config() -> Result<Option<ArmWebviewQaConfig>, String> {
    if env::var("FORGECAD_ARM_WEBVIEW_QA").as_deref() != Ok("1") {
        return Ok(None);
    }
    // The acceptance harness is meaningful only against the deterministic
    // local C106 Provider.  Without this guard a normal user's configured
    // provider could be driven by an unattended test run.
    if !mvp_offline_arm_enabled() {
        return Err("Mechanical-arm WebView QA requires the local offline C106 Provider.".into());
    }
    let phase = env::var("FORGECAD_ARM_WEBVIEW_QA_PHASE")
        .map_err(|_| "Mechanical-arm WebView QA phase is missing.".to_string())?;
    if !matches!(phase.as_str(), "initial" | "restart") {
        return Err("Mechanical-arm WebView QA phase must be initial or restart.".into());
    }
    let reference_class = env::var("FORGECAD_R007B_PACKAGED_REFERENCE_CLASS")
        .unwrap_or_else(|_| "single_image".into());
    if !matches!(
        reference_class.as_str(),
        "single_image" | "multi_view_contact_sheet" | "strict_glb_readback"
    ) {
        return Err("Mechanical-arm WebView QA reference class is invalid.".into());
    }
    Ok(Some(ArmWebviewQaConfig {
        schema_version: ARM_WEBVIEW_QA_SCHEMA,
        phase,
        reference_class,
        r007b_visual_evidence: env::var_os("FORGECAD_R007B_PACKAGED_ARTIFACT_DIR").is_some(),
    }))
}

#[tauri::command]
fn forgecad_arm_webview_qa_capture(
    capture: ArmWebviewQaCaptureRequest,
) -> Result<ArmWebviewQaCaptureReceipt, String> {
    let Some(config) = forgecad_arm_webview_qa_config()? else {
        return Err("Mechanical-arm WebView QA capture is disabled.".into());
    };
    if capture.schema_version != ARM_WEBVIEW_QA_SCHEMA || capture.phase != config.phase {
        return Err("Mechanical-arm WebView QA capture identity is invalid.".into());
    }
    let (max_bytes, extension) = match capture.kind.as_str() {
        "v3_viewport_png" | "r007b_reference_png" | "r007b_result_png" => {
            (ARM_WEBVIEW_QA_CAPTURE_MAX_PNG_BYTES, "png")
        }
        "v3_production_glb" => (ARM_WEBVIEW_QA_CAPTURE_MAX_GLB_BYTES, "glb"),
        _ => return Err("Mechanical-arm WebView QA capture kind is invalid.".into()),
    };
    if capture.kind.starts_with("r007b_") && config.phase != "initial" {
        return Err("R007B visual captures are initial-phase only.".into());
    }
    // Base64 carries at most four bytes for every three binary bytes. Reject
    // excess before decode so the opt-in QA channel cannot become a generic
    // memory or filesystem transport.
    if capture.bytes_base64.len() > (max_bytes.saturating_add(2) / 3).saturating_mul(4) {
        return Err("Mechanical-arm WebView QA capture is too large.".into());
    }
    let bytes = BASE64_STANDARD
        .decode(capture.bytes_base64.as_bytes())
        .map_err(|_| "Mechanical-arm WebView QA capture is not valid Base64.".to_string())?;
    if bytes.is_empty() || bytes.len() > max_bytes {
        return Err("Mechanical-arm WebView QA capture byte length is invalid.".into());
    }
    let (artifact_root, relative_path) = if capture.kind.starts_with("r007b_") {
        let kind = match capture.kind.as_str() {
            "r007b_reference_png" => "reference",
            "r007b_result_png" => "result",
            _ => unreachable!("R007B capture kind already checked"),
        };
        (
            r007b_packaged_artifact_root()?,
            format!("captures/{}/{}.{}", config.reference_class, kind, extension),
        )
    } else {
        (
            sidecar_log_path()
                .parent()
                .ok_or_else(|| {
                    "Mechanical-arm WebView QA artifact root is unavailable.".to_string()
                })?
                .to_path_buf(),
            format!(
                "qa-artifacts/arm-webview/{}/{}.{}",
                config.phase, capture.kind, extension
            ),
        )
    };
    let path = artifact_root.join(&relative_path);
    let parent = path
        .parent()
        .ok_or_else(|| "Mechanical-arm WebView QA artifact parent is unavailable.".to_string())?;
    fs::create_dir_all(parent).map_err(|_| {
        "Mechanical-arm WebView QA artifact directory could not be created.".to_string()
    })?;
    let sha256 = format!("{:x}", Sha256::digest(&bytes));
    let receipt = match capture.kind.as_str() {
        "v3_viewport_png" | "r007b_reference_png" | "r007b_result_png" => {
            let (width, height) = arm_webview_qa_png_dimensions(&bytes)?;
            ArmWebviewQaCaptureReceipt::Png(ArmWebviewQaPngCapture {
                relative_path,
                sha256,
                byte_size: bytes.len() as u64,
                width,
                height,
            })
        }
        "v3_production_glb" => {
            let (triangle_count, complete_pbr_material_count) =
                arm_webview_qa_glb_readback(&bytes)?;
            ArmWebviewQaCaptureReceipt::Glb(ArmWebviewQaGlbCapture {
                relative_path,
                sha256,
                byte_size: bytes.len() as u64,
                triangle_count,
                complete_pbr_material_count,
            })
        }
        _ => unreachable!("capture kind already checked"),
    };
    fs::write(&path, bytes)
        .map_err(|_| "Mechanical-arm WebView QA artifact could not be written.".to_string())?;
    Ok(receipt)
}

fn c111b_packaged_webgl_source_path() -> Result<PathBuf, String> {
    let home = env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| "C111B packaged WebGL QA HOME is missing.".to_string())?;
    if !home.is_absolute() {
        return Err("C111B packaged WebGL QA HOME must be absolute.".into());
    }
    Ok(home.join("qa-inputs").join("c111b-production.glb"))
}

fn c111b_packaged_webgl_config() -> Result<Option<C111bPackagedWebglQaConfig>, String> {
    if env::var("FORGECAD_C111B_WEBVIEW_QA").as_deref() != Ok("1") {
        return Ok(None);
    }
    if env::var("FORGECAD_DISABLE_PROVIDER_CONFIG").as_deref() != Ok("1") {
        return Err(
            "C111B packaged WebGL QA requires Provider configuration to be disabled.".into(),
        );
    }
    let phase = env::var("FORGECAD_C111B_WEBVIEW_QA_PHASE")
        .map_err(|_| "C111B packaged WebGL QA phase is missing.".to_string())?;
    if !matches!(phase.as_str(), "initial" | "restart") {
        return Err("C111B packaged WebGL QA phase must be initial or restart.".into());
    }
    let mode =
        env::var("FORGECAD_C111B_WEBVIEW_QA_MODE").unwrap_or_else(|_| "external_reference".into());
    if !matches!(mode.as_str(), "external_reference" | "agent_asset") {
        return Err(
            "C111B packaged WebGL QA mode must be external_reference or agent_asset.".into(),
        );
    }
    let expected = if phase == "restart" {
        Some((
            k002_probe_stable_id_env("FORGECAD_C111B_WEBVIEW_QA_EXPECT_PROJECT_ID")?,
            k002_probe_stable_id_env("FORGECAD_C111B_WEBVIEW_QA_EXPECT_ASSET_VERSION_ID")?,
            k002_probe_u64_env(
                "FORGECAD_C111B_WEBVIEW_QA_EXPECT_SNAPSHOT_REVISION",
                1,
                u64::MAX,
            )?,
            env::var("FORGECAD_C111B_WEBVIEW_QA_EXPECT_EXPORT_SHA256").map_err(|_| {
                "C111B packaged WebGL QA expected export SHA is missing.".to_string()
            })?,
        ))
    } else {
        None
    };
    if expected
        .as_ref()
        .is_some_and(|value| validate_k001_probe_sha(&value.3).is_err())
    {
        return Err("C111B packaged WebGL QA expected export SHA is invalid.".into());
    }
    let material_count = if mode == "agent_asset" {
        C111B_PACKAGED_WEBGL_AGENT_V2_MATERIALS
    } else {
        C111B_PACKAGED_WEBGL_MATERIALS
    };
    Ok(Some(C111bPackagedWebglQaConfig {
        schema_version: C111B_PACKAGED_WEBGL_SCHEMA,
        phase,
        mode,
        source_sha256: C111B_PACKAGED_WEBGL_PRODUCTION_SHA256,
        triangle_count: C111B_PACKAGED_WEBGL_TRIANGLES,
        primitive_count: C111B_PACKAGED_WEBGL_PRIMITIVES,
        material_count,
        expected_project_id: expected.as_ref().map(|value| value.0.clone()),
        expected_asset_version_id: expected.as_ref().map(|value| value.1.clone()),
        expected_snapshot_revision: expected.as_ref().map(|value| value.2),
        expected_export_sha256: expected.map(|value| value.3),
    }))
}

fn c111b_packaged_webgl_validate_source(bytes: &[u8]) -> Result<(u64, u64, u64, u64), String> {
    if bytes.is_empty() || bytes.len() > C111B_PACKAGED_WEBGL_SOURCE_MAX_BYTES {
        return Err("C111B packaged WebGL QA source byte length is invalid.".into());
    }
    let sha256 = format!("{:x}", Sha256::digest(bytes));
    if sha256 != C111B_PACKAGED_WEBGL_PRODUCTION_SHA256 {
        return Err(
            "C111B packaged WebGL QA source SHA-256 is not the frozen production asset.".into(),
        );
    }
    let (triangle_count, complete_pbr_material_count) = arm_webview_qa_glb_readback(bytes)?;
    let document = c111b_packaged_webgl_glb_json(bytes)?;
    let primitive_count = document
        .get("meshes")
        .and_then(Value::as_array)
        .ok_or_else(|| "C111B packaged WebGL QA GLB meshes are missing.".to_string())?
        .iter()
        .map(|mesh| {
            mesh.get("primitives")
                .and_then(Value::as_array)
                .map(|primitives| primitives.len() as u64)
                .ok_or_else(|| "C111B packaged WebGL QA GLB primitives are missing.".to_string())
        })
        .try_fold(0_u64, |total, next| {
            total
                .checked_add(next?)
                .ok_or_else(|| "C111B packaged WebGL QA primitive count overflowed.".to_string())
        })?;
    let material_count = document
        .get("materials")
        .and_then(Value::as_array)
        .map(|materials| materials.len() as u64)
        .ok_or_else(|| "C111B packaged WebGL QA GLB materials are missing.".to_string())?;
    if triangle_count != C111B_PACKAGED_WEBGL_TRIANGLES
        || primitive_count != C111B_PACKAGED_WEBGL_PRIMITIVES
        || material_count != C111B_PACKAGED_WEBGL_MATERIALS
        || complete_pbr_material_count == 0
    {
        return Err(
            "C111B packaged WebGL QA source inventory drifted from the frozen contract.".into(),
        );
    }
    Ok((
        triangle_count,
        primitive_count,
        material_count,
        complete_pbr_material_count,
    ))
}

#[tauri::command]
fn forgecad_c111b_webview_qa_config() -> Result<Option<C111bPackagedWebglQaConfig>, String> {
    c111b_packaged_webgl_config()
}

#[tauri::command]
fn forgecad_c111b_webview_qa_source(
    request: C111bPackagedWebglQaSourceRequest,
) -> Result<C111bPackagedWebglQaSource, String> {
    let Some(config) = c111b_packaged_webgl_config()? else {
        return Err("C111B packaged WebGL QA source is disabled.".into());
    };
    if config.mode != "external_reference" {
        return Err("C111B Agent-asset QA does not accept an external GLB source.".into());
    }
    if request.schema_version != C111B_PACKAGED_WEBGL_SCHEMA {
        return Err("C111B packaged WebGL QA source schema is invalid.".into());
    }
    let path = c111b_packaged_webgl_source_path()?;
    let bytes = fs::read(&path)
        .map_err(|_| "C111B packaged WebGL QA exact production GLB is missing.".to_string())?;
    let (triangle_count, primitive_count, material_count, complete_pbr_material_count) =
        c111b_packaged_webgl_validate_source(&bytes)?;
    Ok(C111bPackagedWebglQaSource {
        schema_version: config.schema_version,
        file_name: "c111b-production.glb",
        sha256: config.source_sha256.to_string(),
        byte_size: bytes.len() as u64,
        triangle_count,
        primitive_count,
        material_count,
        complete_pbr_material_count,
        bytes_base64: request.include_bytes.then(|| BASE64_STANDARD.encode(bytes)),
    })
}

#[tauri::command]
fn forgecad_c111b_webview_qa_capture(
    capture: C111bPackagedWebglQaCaptureRequest,
) -> Result<C111bPackagedWebglQaCapture, String> {
    let Some(config) = c111b_packaged_webgl_config()? else {
        return Err("C111B packaged WebGL QA capture is disabled.".into());
    };
    if capture.schema_version != C111B_PACKAGED_WEBGL_SCHEMA || capture.phase != config.phase {
        return Err("C111B packaged WebGL QA capture identity is invalid.".into());
    }
    if validate_k001_probe_sha(&capture.source_sha256).is_err()
        || (config.mode == "external_reference" && capture.source_sha256 != config.source_sha256)
        || (config.mode == "agent_asset"
            && config.phase == "restart"
            && config.expected_export_sha256.as_deref() != Some(capture.source_sha256.as_str()))
    {
        return Err("C111B packaged WebGL QA capture source lineage is invalid.".into());
    }
    if !matches!(
        capture.view_id.as_str(),
        "iso" | "front" | "back" | "left" | "right" | "top" | "gripper_iso" | "gripper_front"
    ) {
        return Err("C111B packaged WebGL QA view is not in the frozen eight-view set.".into());
    }
    if capture.auxiliary_width != 960
        || capture.auxiliary_height != 640
        || capture.auxiliary_pass_ids.iter().map(String::as_str).collect::<Vec<_>>()
            != ["silhouette", "normal", "depth", "part_id", "material_id"]
    {
        return Err("C111B packaged WebGL QA auxiliary pass contract is invalid.".into());
    }
    if capture.bytes_base64.len()
        > (C111B_PACKAGED_WEBGL_CAPTURE_MAX_PNG_BYTES.saturating_add(2) / 3).saturating_mul(4)
    {
        return Err("C111B packaged WebGL QA screenshot is too large.".into());
    }
    let bytes = BASE64_STANDARD
        .decode(capture.bytes_base64.as_bytes())
        .map_err(|_| "C111B packaged WebGL QA screenshot is not valid Base64.".to_string())?;
    if bytes.is_empty() || bytes.len() > C111B_PACKAGED_WEBGL_CAPTURE_MAX_PNG_BYTES {
        return Err("C111B packaged WebGL QA screenshot byte length is invalid.".into());
    }
    let (width, height) = arm_webview_qa_png_dimensions(&bytes)?;
    if capture.auxiliary_bytes_base64.len()
        > (C111B_PACKAGED_WEBGL_AUXILIARY_MAX_PNG_BYTES.saturating_add(2) / 3).saturating_mul(4)
    {
        return Err("C111B packaged WebGL QA auxiliary screenshot is too large.".into());
    }
    let auxiliary_bytes = BASE64_STANDARD
        .decode(capture.auxiliary_bytes_base64.as_bytes())
        .map_err(|_| {
            "C111B packaged WebGL QA auxiliary screenshot is not valid Base64.".to_string()
        })?;
    if auxiliary_bytes.is_empty()
        || auxiliary_bytes.len() > C111B_PACKAGED_WEBGL_AUXILIARY_MAX_PNG_BYTES
    {
        return Err("C111B packaged WebGL QA auxiliary screenshot byte length is invalid.".into());
    }
    let (auxiliary_width, auxiliary_height) = arm_webview_qa_png_dimensions(&auxiliary_bytes)?;
    if auxiliary_width != capture.auxiliary_width || auxiliary_height != capture.auxiliary_height {
        return Err("C111B packaged WebGL QA auxiliary screenshot dimensions are invalid.".into());
    }
    let relative_path = format!(
        "qa-artifacts/c111b-webgl/{}/{}.png",
        config.phase, capture.view_id
    );
    let auxiliary_relative_path = format!(
        "qa-artifacts/c111b-webgl/{}/{}.auxiliary.png",
        config.phase, capture.view_id
    );
    let root = sidecar_log_path()
        .parent()
        .ok_or_else(|| "C111B packaged WebGL QA artifact root is unavailable.".to_string())?
        .to_path_buf();
    let path = root.join(&relative_path);
    let auxiliary_path = root.join(&auxiliary_relative_path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|_| {
            "C111B packaged WebGL QA artifact directory could not be created.".to_string()
        })?;
    }
    let sha256 = format!("{:x}", Sha256::digest(&bytes));
    let auxiliary_sha256 = format!("{:x}", Sha256::digest(&auxiliary_bytes));
    fs::write(&path, &bytes)
        .map_err(|_| "C111B packaged WebGL QA screenshot could not be written.".to_string())?;
    fs::write(&auxiliary_path, &auxiliary_bytes).map_err(|_| {
        "C111B packaged WebGL QA auxiliary screenshot could not be written.".to_string()
    })?;
    Ok(C111bPackagedWebglQaCapture {
        view_id: capture.view_id,
        relative_path,
        sha256,
        byte_size: bytes.len() as u64,
        width,
        height,
        source_sha256: capture.source_sha256,
        readability: None,
        auxiliary_width: Some(auxiliary_width),
        auxiliary_height: Some(auxiliary_height),
        auxiliary_pass_ids: Some(capture.auxiliary_pass_ids),
        auxiliary_byte_size: Some(auxiliary_bytes.len() as u64),
        auxiliary_sha256: Some(auxiliary_sha256),
        auxiliary_relative_path: Some(auxiliary_relative_path),
    })
}

#[tauri::command]
async fn forgecad_c111b_webview_qa_readback(
    request: C111bPackagedWebglQaReadbackRequest,
    bridge: State<'_, AppServerBridge>,
) -> Result<C111bPackagedWebglQaReadback, String> {
    let Some(config) = c111b_packaged_webgl_config()? else {
        return Err("C111B packaged WebGL QA readback is disabled.".into());
    };
    if request.schema_version != C111B_PACKAGED_WEBGL_SCHEMA
        || !forgecad_app_server_protocol::valid_stable_id(&request.project_id)
        || !forgecad_app_server_protocol::valid_stable_id(&request.asset_version_id)
    {
        return Err("C111B packaged WebGL QA readback request is invalid.".into());
    }
    let asset = r007b_packaged_get_json(
        bridge.inner(),
        format!("/api/v1/agent/asset-versions/{}", request.asset_version_id),
    )
    .await?;
    if asset.get("project_id").and_then(Value::as_str) != Some(request.project_id.as_str())
        || asset.get("asset_version_id").and_then(Value::as_str)
            != Some(request.asset_version_id.as_str())
    {
        return Err("C111B packaged WebGL QA asset identity is invalid.".into());
    }
    if config.mode == "external_reference"
        && (asset
            .pointer("/shape_program/schema_version")
            .and_then(Value::as_str)
            != Some("ExternalGLBReference@1")
            || asset
                .pointer("/shape_program/editable")
                .and_then(Value::as_bool)
                != Some(false)
            || asset
                .pointer("/shape_program/source_sha256")
                .and_then(Value::as_str)
                != Some(C111B_PACKAGED_WEBGL_PRODUCTION_SHA256))
    {
        return Err("C111B packaged WebGL QA exact external-reference lineage is invalid.".into());
    }
    if config.mode == "agent_asset" {
        let parent_asset_version_id = c111b_validate_agent_asset_lineage(
            &asset,
            C111B_PACKAGED_WEBGL_VISUAL_PROGRAM_SHAPE_SHA256,
        )?;
        let parent = r007b_packaged_get_json(
            bridge.inner(),
            format!("/api/v1/agent/asset-versions/{parent_asset_version_id}"),
        )
        .await?;
        c111b_validate_agent_parent_lineage(
            &parent,
            &request.project_id,
            &parent_asset_version_id,
            C111B_PACKAGED_WEBGL_VISUAL_PROGRAM_SHAPE_SHA256,
        )?;
        let (_response, bytes) = r007b_packaged_get_binary(
            bridge.inner(),
            format!(
                "/api/v1/agent/asset-versions/{}:model.glb",
                request.asset_version_id
            ),
        )
        .await?;
        let source_sha256 = format!("{:x}", Sha256::digest(&bytes));
        let (triangle_count, complete_pbr_material_count) = arm_webview_qa_glb_readback(&bytes)?;
        let document = c111b_packaged_webgl_glb_json(&bytes)?;
        let primitive_count = document
            .get("meshes")
            .and_then(Value::as_array)
            .ok_or_else(|| "C111B packaged WebGL QA Agent GLB meshes are missing.".to_string())?
            .iter()
            .map(|mesh| {
                mesh.get("primitives")
                    .and_then(Value::as_array)
                    .map(|primitives| primitives.len() as u64)
                    .ok_or_else(|| {
                        "C111B packaged WebGL QA Agent GLB primitives are missing.".to_string()
                    })
            })
            .try_fold(0_u64, |total, next| {
                total.checked_add(next?).ok_or_else(|| {
                    "C111B packaged WebGL QA Agent primitive count overflowed.".to_string()
                })
            })?;
        let material_count = document
            .get("materials")
            .and_then(Value::as_array)
            .map(|materials| materials.len() as u64)
            .ok_or_else(|| {
                "C111B packaged WebGL QA Agent GLB materials are missing.".to_string()
            })?;
        if triangle_count != C111B_PACKAGED_WEBGL_TRIANGLES
            || primitive_count != C111B_PACKAGED_WEBGL_PRIMITIVES
            || material_count != C111B_PACKAGED_WEBGL_AGENT_V2_MATERIALS
            || complete_pbr_material_count != C111B_PACKAGED_WEBGL_AGENT_V2_COMPLETE_PBR_MATERIALS
            || (config.phase == "restart"
                && config.expected_export_sha256.as_deref() != Some(source_sha256.as_str()))
        {
            append_supervisor_log(&format!(
                "ForgeCAD C111B packaged WebGL QA diagnostic=agent_production_glb_inventory_mismatch actual_sha256={source_sha256} expected_restart_sha256={} actual_triangles={triangle_count} expected_triangles={} actual_primitives={primitive_count} expected_primitives={} actual_materials={material_count} expected_materials={} complete_pbr_materials={complete_pbr_material_count} expected_complete_pbr_materials={}",
                config.expected_export_sha256.as_deref().unwrap_or("initial_dynamic_lineage"),
                C111B_PACKAGED_WEBGL_TRIANGLES,
                C111B_PACKAGED_WEBGL_PRIMITIVES,
                C111B_PACKAGED_WEBGL_AGENT_V2_MATERIALS,
                C111B_PACKAGED_WEBGL_AGENT_V2_COMPLETE_PBR_MATERIALS,
            ));
            return Err(
                "C111B packaged WebGL QA Agent production GLB inventory or hash is invalid.".into(),
            );
        }
        return Ok(C111bPackagedWebglQaReadback {
            schema_version: C111B_PACKAGED_WEBGL_SCHEMA.into(),
            project_id: request.project_id,
            asset_version_id: request.asset_version_id,
            source_sha256,
            shape_program_schema: "ShapeProgram@1".into(),
            external_reference: false,
            glb_byte_size: Some(bytes.len() as u64),
            glb_triangle_count: Some(triangle_count),
            glb_primitive_count: Some(primitive_count),
            glb_material_count: Some(material_count),
        });
    }
    Ok(C111bPackagedWebglQaReadback {
        schema_version: C111B_PACKAGED_WEBGL_SCHEMA.into(),
        project_id: request.project_id,
        asset_version_id: request.asset_version_id,
        source_sha256: C111B_PACKAGED_WEBGL_PRODUCTION_SHA256.to_string(),
        shape_program_schema: "ExternalGLBReference@1".into(),
        external_reference: true,
        glb_byte_size: None,
        glb_triangle_count: None,
        glb_primitive_count: None,
        glb_material_count: None,
    })
}

fn c111b_validate_agent_asset_lineage(
    asset: &Value,
    expected_shape_program_sha256: &str,
) -> Result<String, String> {
    let shape_program = asset.get("shape_program").ok_or_else(|| {
        "C111B packaged WebGL QA Agent ShapeProgram lineage is invalid.".to_string()
    })?;
    let shape_program_sha256 = semantic_sha256(shape_program).map_err(|_| {
        "C111B packaged WebGL QA Agent ShapeProgram lineage is invalid.".to_string()
    })?;
    if shape_program.get("schema_version").and_then(Value::as_str) != Some("ShapeProgram@1") {
        return Err("C111B packaged WebGL QA Agent ShapeProgram schema is invalid.".into());
    }
    if shape_program
        .get("non_functional_only")
        .and_then(Value::as_bool)
        != Some(true)
    {
        return Err("C111B packaged WebGL QA Agent non-functional scope is invalid.".into());
    }
    if shape_program_sha256 != expected_shape_program_sha256 {
        append_supervisor_log(&format!(
            "ForgeCAD C111B packaged WebGL QA diagnostic=shape_program_sha256_mismatch actual={shape_program_sha256} expected={expected_shape_program_sha256}"
        ));
        return Err("C111B packaged WebGL QA Agent ShapeProgram hash is invalid.".into());
    }
    if asset.get("version_no").and_then(Value::as_u64) != Some(2) {
        return Err("C111B packaged WebGL QA Agent V2 lineage is invalid.".into());
    }
    if asset.get("status").and_then(Value::as_str) != Some("committed") {
        return Err("C111B packaged WebGL QA Agent V2 status is invalid.".into());
    }
    let parent_asset_version_id = asset
        .get("parent_asset_version_id")
        .and_then(Value::as_str)
        .filter(|value| forgecad_app_server_protocol::valid_stable_id(value))
        .ok_or_else(|| "C111B packaged WebGL QA Agent parent lineage is invalid.".to_string())?;

    let adornments = asset
        .pointer("/assembly_graph/surface_adornments")
        .and_then(Value::as_array)
        .ok_or_else(|| "C111B packaged WebGL QA Agent A005 lineage is missing.".to_string())?;
    let matching = adornments
        .iter()
        .filter(|value| {
            value.get("target_zone_id").and_then(Value::as_str) == Some("zone_arm_link_shell")
        })
        .collect::<Vec<_>>();
    if matching.len() != 1 {
        return Err("C111B packaged WebGL QA Agent A005 lineage is invalid.".into());
    }
    let program: SurfaceAdornmentProgram = serde_json::from_value((*matching[0]).clone())
        .map_err(|_| "C111B packaged WebGL QA Agent A005 lineage is invalid.".to_string())?;
    program
        .validate()
        .map_err(|_| "C111B packaged WebGL QA Agent A005 lineage is invalid.".to_string())?;
    if program.target_zone_id != "zone_arm_link_shell"
        || program.kind != "normal_relief"
        || program.motif != "parallel_groove"
        || program.intensity != "subtle"
        || program.coverage != "center_band"
        || program.base_material != "mat_composite"
        || program.skill_id != "skill_first_party_surface_adornment"
        || program.skill_version != 3
    {
        return Err("C111B packaged WebGL QA Agent A005 lineage is invalid.".into());
    }
    let program_sha256 = program
        .canonical_sha256()
        .map_err(|_| "C111B packaged WebGL QA Agent A005 lineage is invalid.".to_string())?;
    let binding_key = format!("{}:{}", program.target_part_id, program.target_zone_id);
    let expected_material_id = format!("mat_a005_{}", &program_sha256[..32]);
    if asset
        .get("material_bindings")
        .and_then(Value::as_object)
        .and_then(|bindings| bindings.get(&binding_key))
        .and_then(Value::as_str)
        != Some(expected_material_id.as_str())
    {
        return Err("C111B packaged WebGL QA Agent A005 material seal is invalid.".into());
    }
    Ok(parent_asset_version_id.to_string())
}

fn c111b_validate_agent_parent_lineage(
    parent: &Value,
    project_id: &str,
    parent_asset_version_id: &str,
    expected_shape_program_sha256: &str,
) -> Result<(), String> {
    let shape_program = parent
        .get("shape_program")
        .ok_or_else(|| "C111B packaged WebGL QA Agent parent lineage is invalid.".to_string())?;
    let shape_program_sha256 = semantic_sha256(shape_program)
        .map_err(|_| "C111B packaged WebGL QA Agent parent lineage is invalid.".to_string())?;
    if parent.get("project_id").and_then(Value::as_str) != Some(project_id)
        || parent.get("asset_version_id").and_then(Value::as_str) != Some(parent_asset_version_id)
        || parent
            .get("parent_asset_version_id")
            .is_some_and(|value| !value.is_null())
        || parent.get("version_no").and_then(Value::as_u64) != Some(1)
        || shape_program_sha256 != expected_shape_program_sha256
    {
        return Err("C111B packaged WebGL QA Agent parent lineage is invalid.".into());
    }
    Ok(())
}

#[tauri::command]
async fn forgecad_c111b_webview_qa_report(
    mut report: C111bPackagedWebglQaReport,
    metrics: State<'_, C111bPackagedQaMetricsState>,
    bridge: State<'_, AppServerBridge>,
) -> Result<(), String> {
    let Some(config) = c111b_packaged_webgl_config()? else {
        return Err("C111B packaged WebGL QA reporting is disabled.".into());
    };
    if report.schema_version != C111B_PACKAGED_WEBGL_SCHEMA || report.phase != config.phase {
        return Err("C111B packaged WebGL QA report identity is invalid.".into());
    }
    attach_c111b_packaged_native_metrics(&mut report, &config, &metrics, Some(&bridge)).await?;
    if !report.ok {
        let code = report
            .error_code
            .as_deref()
            .ok_or_else(|| "C111B packaged WebGL QA failure requires error_code.".to_string())?;
        if !forgecad_app_server_protocol::valid_stable_id(code) {
            return Err("C111B packaged WebGL QA error_code is invalid.".into());
        }
    } else {
        validate_c111b_packaged_webgl_success(&report, &config)?;
    }
    let encoded = serde_json::to_string(&report)
        .map_err(|_| "C111B packaged WebGL QA report could not be serialized.".to_string())?;
    append_supervisor_log(&format!("{C111B_PACKAGED_WEBGL_MARKER}{encoded}"));
    Ok(())
}

async fn attach_c111b_packaged_native_metrics(
    report: &mut C111bPackagedWebglQaReport,
    config: &C111bPackagedWebglQaConfig,
    metrics: &C111bPackagedQaMetricsState,
    bridge: Option<&AppServerBridge>,
) -> Result<(), String> {
    attach_c111b_packaged_timing_metrics(report, metrics)?;
    if config.mode == "agent_asset" {
        let provider = metrics.local_mvp_provider.as_ref().ok_or_else(|| {
            "C111B Agent packaged QA requires the native offline Provider counter.".to_string()
        })?;
        if !report.ok {
            clear_c111b_turn_metrics(report);
            report.provider_protocol_requests = Some(provider.calls());
            report.turn_metrics_source =
                Some("native_failure_without_terminal_turn_projection".into());
        } else if config.phase == "initial" {
            let thread_id = report
                .thread_id
                .as_deref()
                .filter(|value| forgecad_app_server_protocol::valid_stable_id(value))
                .ok_or_else(|| "C111B Agent packaged QA thread ID is invalid.".to_string())?;
            let turn_id = report
                .turn_id
                .as_deref()
                .filter(|value| forgecad_app_server_protocol::valid_stable_id(value))
                .ok_or_else(|| "C111B Agent packaged QA turn ID is invalid.".to_string())?;
            let value = mvp_arm_packaged_probe::native(
                bridge.ok_or_else(|| {
                    "C111B Agent packaged QA terminal Turn bridge is unavailable.".to_string()
                })?,
                "c111b_webview_report_turn_read",
                "turn/read",
                json!({
                    "schema_version": "AgentTurnCommand@1",
                    "command_id": "c111b_webview_report_turn_read",
                    "command": {
                        "operation": "read",
                        "thread_id": thread_id,
                        "turn_id": turn_id,
                    }
                }),
            )
            .await
            .map_err(|_| "C111B Agent packaged QA terminal Turn readback failed.".to_string())?;
            let turn = c111b_terminal_turn_from_result(value)?;
            attach_c111b_terminal_turn_metrics(report, &turn, provider.calls())?;
        } else {
            report.thread_id = None;
            report.turn_id = None;
            clear_c111b_turn_metrics(report);
            report.provider_protocol_requests = Some(provider.calls());
            report.turn_metrics_source = Some("native_no_turn_on_restart".into());
        }
        report.network_provider_calls = Some(0);
        report.network_call_made = Some(false);
        report.credential_reads = Some(0);
        report.provider_metrics_source = Some(
            if report.ok && config.phase == "initial" {
                "rust_terminal_turn_plus_native_local_mvp_counter"
            } else {
                "native_local_mvp_atomic_counter"
            }
            .into(),
        );
        report.credential_metrics_source = Some("native_structural_no_credential_source".into());
        report.billable_variable_cost_microusd = Some(0);
        report.billable_variable_cost_source = Some("native_offline_no_billable_transport".into());
    } else {
        report.thread_id = None;
        report.turn_id = None;
        clear_c111b_turn_metrics(report);
        report.provider_protocol_requests = Some(0);
        report.network_provider_calls = Some(0);
        report.network_call_made = Some(false);
        report.credential_reads = Some(0);
        report.provider_metrics_source = Some("native_no_agent_provider_path".into());
        report.credential_metrics_source = Some("native_no_agent_provider_path".into());
        report.billable_variable_cost_microusd = Some(0);
        report.billable_variable_cost_source = Some("native_no_agent_provider_path".into());
    }
    Ok(())
}

fn c111b_terminal_turn_from_result(value: Value) -> Result<AgentTurn, String> {
    let terminal: TurnCommandResult = serde_json::from_value(value).map_err(|_| {
        "C111B Agent packaged QA terminal Turn result contract is invalid.".to_string()
    })?;
    terminal.validate().map_err(|_| {
        "C111B Agent packaged QA terminal Turn result validation failed.".to_string()
    })?;
    if terminal.command_id != "c111b_webview_report_turn_read" {
        return Err("C111B Agent packaged QA terminal Turn command identity diverged.".into());
    }
    match terminal.result {
        TurnCommandOutcome::Turn { turn } => Ok(turn),
        _ => Err("C111B Agent packaged QA terminal Turn outcome is invalid.".into()),
    }
}

fn clear_c111b_turn_metrics(report: &mut C111bPackagedWebglQaReport) {
    report.product_tool_calls = Some(0);
    report.input_tokens = Some(0);
    report.output_tokens = Some(0);
    report.prompt_cache_hit_tokens = Some(0);
    report.prompt_cache_miss_tokens = Some(0);
    report.same_intent_repair_attempts = Some(0);
    report.same_intent_repairs_applied = Some(0);
    report.provider_schema_repair_requests = Some(0);
    report.product_tool_schema_repair_requests = Some(0);
    report.estimated_cost_microusd = Some(0);
    report.turn_total_elapsed_ms = Some(0);
    report.turn_phase_timings_ms = Some(BTreeMap::new());
    report.turn_trace_sha256 = None;
    report.turn_metrics_source = None;
}

fn attach_c111b_terminal_turn_metrics(
    report: &mut C111bPackagedWebglQaReport,
    turn: &AgentTurn,
    provider_calls: u64,
) -> Result<(), String> {
    if Some(turn.thread_id.as_str()) != report.thread_id.as_deref()
        || Some(turn.turn_id.as_str()) != report.turn_id.as_deref()
        || turn.status != AgentTurnStatus::Completed
    {
        return Err("C111B Agent packaged QA terminal Turn identity is invalid.".into());
    }
    let usage = &turn.usage;
    let required_u64 = |field: &str| {
        usage
            .get(field)
            .and_then(Value::as_u64)
            .ok_or_else(|| format!("C111B Agent packaged QA Turn usage {field} is invalid."))
    };
    let provider_requests = required_u64("provider_requests")?;
    if provider_requests != provider_calls
        || usage.get("network_call_made").and_then(Value::as_bool) != Some(false)
        || usage.get("outcome").and_then(Value::as_str) != Some("completed")
    {
        return Err("C111B Agent packaged QA Turn and native Provider facts disagree.".into());
    }
    let trace = usage
        .get("redacted_trace")
        .ok_or_else(|| "C111B Agent packaged QA redacted Turn trace is missing.".to_string())?;
    let (turn_total_elapsed_ms, turn_phase_timings_ms) = c111b_turn_timings(trace)?;
    report.provider_protocol_requests = Some(provider_requests);
    report.product_tool_calls = Some(required_u64("product_tool_calls")?);
    report.input_tokens = Some(required_u64("input_tokens")?);
    report.output_tokens = Some(required_u64("output_tokens")?);
    report.prompt_cache_hit_tokens = Some(required_u64("prompt_cache_hit_tokens")?);
    report.prompt_cache_miss_tokens = Some(required_u64("prompt_cache_miss_tokens")?);
    report.estimated_cost_microusd = Some(required_u64("estimated_cost_microusd")?);
    let (same_intent_repair_attempts, provider_schema_repairs, product_tool_schema_repairs) =
        c111b_repair_request_counts(trace)?;
    let same_intent_repairs_applied = c111b_same_intent_repairs_applied(turn)?;
    if same_intent_repairs_applied > same_intent_repair_attempts || same_intent_repair_attempts > 2
    {
        return Err("C111B Agent packaged QA repair evidence is inconsistent.".into());
    }
    report.same_intent_repair_attempts = Some(same_intent_repair_attempts);
    report.same_intent_repairs_applied = Some(same_intent_repairs_applied);
    report.provider_schema_repair_requests = Some(provider_schema_repairs);
    report.product_tool_schema_repair_requests = Some(product_tool_schema_repairs);
    report.turn_total_elapsed_ms = Some(turn_total_elapsed_ms);
    report.turn_phase_timings_ms = Some(turn_phase_timings_ms);
    report.turn_trace_sha256 = Some(
        usage
            .get("trace_sha256")
            .and_then(Value::as_str)
            .filter(|value| value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit()))
            .ok_or_else(|| "C111B Agent packaged QA Turn trace digest is invalid.".to_string())?
            .to_string(),
    );
    report.turn_metrics_source = Some("rust_terminal_turn_readback".into());
    Ok(())
}

fn c111b_same_intent_repairs_applied(turn: &AgentTurn) -> Result<u64, String> {
    turn.items
        .iter()
        .rev()
        .find_map(|item| {
            (item.payload.get("tool_name").and_then(Value::as_str) == Some("evaluate_candidate"))
                .then(|| {
                    item.payload
                        .get("tool_result")
                        .and_then(|value| {
                            value.pointer(
                        "/validated_output/value/visual_convergence_report/repair_attempt_count",
                    )
                        })
                        .and_then(Value::as_u64)
                })
                .flatten()
        })
        .ok_or_else(|| {
            "C111B Agent packaged QA repair count is missing from Rust evaluation.".into()
        })
}

fn c111b_repair_request_counts(trace: &Value) -> Result<(u64, u64, u64), String> {
    let entries = trace
        .get("entries")
        .and_then(Value::as_array)
        .ok_or_else(|| "C111B Agent packaged QA redacted trace entries are missing.".to_string())?;
    let same_intent = entries
        .iter()
        .filter(|entry| {
            entry.get("phase").and_then(Value::as_str) == Some("product_tool")
                && entry.get("event").and_then(Value::as_str) == Some("started")
                && entry.get("tool_name").and_then(Value::as_str)
                    == Some("patch_forge_visual_program")
        })
        .count() as u64;
    let provider_schema = entries
        .iter()
        .filter(|entry| {
            entry.get("error_code").and_then(Value::as_str)
                == Some("PROVIDER_SCHEMA_REPAIR_REQUESTED")
        })
        .count() as u64;
    let product_tool_schema = entries
        .iter()
        .filter(|entry| {
            entry.get("error_code").and_then(Value::as_str)
                == Some("PRODUCT_TOOL_SCHEMA_REPAIR_REQUESTED")
        })
        .count() as u64;
    Ok((same_intent, provider_schema, product_tool_schema))
}

fn c111b_turn_timings(trace: &Value) -> Result<(u64, BTreeMap<String, u64>), String> {
    let entries = trace
        .get("entries")
        .and_then(Value::as_array)
        .filter(|entries| !entries.is_empty())
        .ok_or_else(|| "C111B Agent packaged QA redacted trace entries are missing.".to_string())?;
    let mut starts = HashMap::<String, (u64, Option<String>)>::new();
    let mut timings = BTreeMap::<String, u64>::new();
    let mut total = 0u64;
    for (index, entry) in entries.iter().enumerate() {
        if entry.get("sequence").and_then(Value::as_u64) != Some(index as u64 + 1) {
            return Err("C111B Agent packaged QA trace sequence is invalid.".into());
        }
        let phase = entry
            .get("phase")
            .and_then(Value::as_str)
            .ok_or_else(|| "C111B Agent packaged QA trace phase is invalid.".to_string())?;
        let event = entry
            .get("event")
            .and_then(Value::as_str)
            .ok_or_else(|| "C111B Agent packaged QA trace event is invalid.".to_string())?;
        let elapsed = entry
            .get("elapsed_ms")
            .and_then(Value::as_u64)
            .filter(|value| *value <= 300_000)
            .ok_or_else(|| "C111B Agent packaged QA trace elapsed time is invalid.".to_string())?;
        if elapsed < total {
            return Err("C111B Agent packaged QA trace elapsed time moved backwards.".into());
        }
        total = elapsed;
        if event == "started" {
            if starts
                .insert(
                    phase.to_string(),
                    (
                        elapsed,
                        entry
                            .get("call_id")
                            .and_then(Value::as_str)
                            .map(str::to_string),
                    ),
                )
                .is_some()
            {
                return Err("C111B Agent packaged QA trace has overlapping phase spans.".into());
            }
        } else if matches!(
            event,
            "completed"
                | "failed"
                | "rejected"
                | "cancelled"
                | "budget_exceeded"
                | "late_result_ignored"
        ) {
            if let Some((started, started_call_id)) = starts.remove(phase) {
                let completed_call_id = entry.get("call_id").and_then(Value::as_str);
                if started_call_id.as_deref().is_some()
                    && started_call_id.as_deref() != completed_call_id
                {
                    return Err("C111B Agent packaged QA trace call pairing is invalid.".into());
                }
                let duration = elapsed.saturating_sub(started);
                if let Some(stage) =
                    c111b_pipeline_stage(phase, entry.get("tool_name").and_then(Value::as_str))
                {
                    let aggregate = timings.entry(stage.into()).or_default();
                    *aggregate = aggregate.saturating_add(duration);
                }
            }
        }
    }
    if !starts.is_empty()
        || total == 0
        || ![
            "author",
            "lower",
            "compile_readback",
            "render",
            "evaluate",
            "preview",
        ]
        .iter()
        .all(|phase| timings.contains_key(*phase))
    {
        return Err("C111B Agent packaged QA Turn phase timing is incomplete.".into());
    }
    Ok((total, timings))
}

fn c111b_pipeline_stage(phase: &str, tool_name: Option<&str>) -> Option<&'static str> {
    match (phase, tool_name) {
        ("provider", Some("author_forge_visual_program"))
        | ("product_tool", Some("author_forge_visual_program")) => Some("author"),
        ("product_tool", Some("build_candidate_geometry")) => Some("lower"),
        ("product_tool", Some("compile_readback_candidate")) => Some("compile_readback"),
        ("product_tool", Some("render_candidate_views")) => Some("render"),
        ("product_tool", Some("evaluate_candidate")) => Some("evaluate"),
        ("product_tool", Some("prepare_candidate_preview")) => Some("preview"),
        _ => None,
    }
}

fn attach_c111b_packaged_timing_metrics(
    report: &mut C111bPackagedWebglQaReport,
    metrics: &C111bPackagedQaMetricsState,
) -> Result<(), String> {
    let mut timeline = metrics
        .timeline
        .lock()
        .map_err(|_| "C111B packaged QA timing state is unavailable.".to_string())?;
    let elapsed = u64::try_from(timeline.started.elapsed().as_millis())
        .unwrap_or(u64::MAX)
        .min(900_000);
    if timeline
        .stages
        .last()
        .is_none_or(|(stage, _)| stage != "report_received")
    {
        timeline.stages.push(("report_received".into(), elapsed));
    }
    let mut previous = 0u64;
    let timings = timeline
        .stages
        .iter()
        .map(|(stage, elapsed_ms)| {
            let timing = C111bPackagedQaStageTiming {
                stage: stage.clone(),
                elapsed_ms: *elapsed_ms,
                duration_since_previous_ms: elapsed_ms.saturating_sub(previous),
            };
            previous = *elapsed_ms;
            timing
        })
        .collect::<Vec<_>>();
    report.end_to_end_elapsed_ms = Some(elapsed);
    report.stage_timings = Some(timings);
    report.timing_metrics_source = Some("native_monotonic_progress_receipts".into());
    Ok(())
}

#[tauri::command]
fn forgecad_c111b_webview_qa_progress(
    stage: String,
    metrics: State<'_, C111bPackagedQaMetricsState>,
) -> Result<(), String> {
    if c111b_packaged_webgl_config()?.is_none() {
        return Err("C111B packaged WebGL QA progress is disabled.".into());
    }
    if !forgecad_app_server_protocol::valid_stable_id(&stage) {
        return Err("C111B packaged WebGL QA progress stage is invalid.".into());
    }
    if c111b_fixed_timing_stage(&stage) {
        let mut timeline = metrics
            .timeline
            .lock()
            .map_err(|_| "C111B packaged QA timing state is unavailable.".to_string())?;
        if timeline
            .stages
            .iter()
            .any(|(existing, _)| existing == &stage)
        {
            return Err("C111B packaged QA timing stage was reported twice.".into());
        }
        let elapsed = u64::try_from(timeline.started.elapsed().as_millis()).unwrap_or(u64::MAX);
        if elapsed > 900_000 {
            return Err("C111B packaged QA timing exceeded its bounded window.".into());
        }
        timeline.stages.push((stage.clone(), elapsed));
    }
    append_supervisor_log(&format!("{C111B_PACKAGED_WEBGL_PROGRESS_MARKER}{stage}"));
    Ok(())
}

fn c111b_fixed_timing_stage(stage: &str) -> bool {
    matches!(
        stage,
        "workbench_ready"
            | "visible_import_requested"
            | "external_asset_ready"
            | "external_captures_ready"
            | "external_restart_workbench_ready"
            | "external_restart_snapshot_hydrated"
            | "external_restart_captures_ready"
            | "agent_workbench_ready"
            | "agent_brief_sent"
            | "agent_v1_confirmed"
            | "agent_selection_card_ready"
            | "agent_link_part_selected"
            | "agent_adornment_drawer_ready"
            | "agent_v2_confirmed"
            | "agent_export_readback_ready"
            | "agent_captures_ready"
            | "agent_restart_workbench_ready"
            | "agent_restart_snapshot_hydrated"
            | "agent_restart_export_readback_ready"
            | "agent_restart_captures_ready"
    )
}

fn r007b_packaged_artifact_root() -> Result<PathBuf, String> {
    let home = env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| "R007B packaged artifact HOME is missing.".to_string())?;
    let configured = env::var_os("FORGECAD_R007B_PACKAGED_ARTIFACT_DIR")
        .map(PathBuf::from)
        .ok_or_else(|| "R007B packaged artifact directory is missing.".to_string())?;
    if !configured.is_absolute() {
        return Err("R007B packaged artifact directory must be absolute.".into());
    }
    fs::create_dir_all(&configured)
        .map_err(|_| "R007B packaged artifact directory could not be created.".to_string())?;
    let canonical_home = home
        .canonicalize()
        .map_err(|_| "R007B packaged artifact HOME is invalid.".to_string())?;
    let canonical = configured
        .canonicalize()
        .map_err(|_| "R007B packaged artifact directory is invalid.".to_string())?;
    if canonical == canonical_home || !canonical.starts_with(&canonical_home) {
        return Err("R007B packaged artifact directory must remain inside HOME.".into());
    }
    Ok(canonical)
}

async fn r007b_packaged_get_json(bridge: &AppServerBridge, path: String) -> Result<Value, String> {
    let endpoint =
        LocalAgentEndpoint::parse("http://127.0.0.1:1").map_err(|error| error.message)?;
    let response = bridge
        .execute_k003_packaged_compat(
            PreparedCompatHttpRequest {
                endpoint,
                method: AllowedHttpMethod::Get,
                path,
                headers: Vec::new(),
                body: ProtocolHttpBody::Empty,
            },
            CancellationToken::new(),
        )
        .await
        .map_err(|error| error.message)?;
    if !(200..300).contains(&response.status) {
        return Err("R007B packaged lineage read was rejected.".into());
    }
    let ProtocolHttpBody::Utf8 { data } = response.body else {
        return Err("R007B packaged lineage read was not JSON.".into());
    };
    serde_json::from_str(&data)
        .map_err(|_| "R007B packaged lineage JSON could not be decoded.".to_string())
}

async fn r007b_packaged_get_binary(
    bridge: &AppServerBridge,
    path: String,
) -> Result<(CompatHttpResponse, Vec<u8>), String> {
    let endpoint =
        LocalAgentEndpoint::parse("http://127.0.0.1:1").map_err(|error| error.message)?;
    let response = bridge
        .execute_k003_packaged_compat(
            PreparedCompatHttpRequest {
                endpoint,
                method: AllowedHttpMethod::Get,
                path,
                headers: Vec::new(),
                body: ProtocolHttpBody::Empty,
            },
            CancellationToken::new(),
        )
        .await
        .map_err(|error| error.message)?;
    if !(200..300).contains(&response.status) {
        return Err("C111B packaged Agent GLB readback was rejected.".into());
    }
    let ProtocolHttpBody::Base64 { data } = &response.body else {
        return Err("C111B packaged Agent GLB readback was not binary.".into());
    };
    let bytes = BASE64_STANDARD
        .decode(data)
        .map_err(|_| "C111B packaged Agent GLB readback Base64 is invalid.".to_string())?;
    Ok((response, bytes))
}

/// Returns only sealed Rust-owned facts needed by the opt-in packaged visual
/// producer. It cannot write product state and is unreachable in normal runs.
#[tauri::command]
async fn forgecad_arm_webview_qa_r007b_lineage(
    request: R007BPackagedLineageRequest,
    bridge: State<'_, AppServerBridge>,
) -> Result<Value, String> {
    let Some(config) = forgecad_arm_webview_qa_config()? else {
        return Err("R007B packaged lineage capture is disabled.".into());
    };
    if config.phase != "initial"
        || request.schema_version != ARM_WEBVIEW_QA_SCHEMA
        || ![
            request.project_id.as_str(),
            request.rebuild_plan_id.as_str(),
            request.preview_change_set_id.as_str(),
            request.confirmed_asset_version_id.as_str(),
        ]
        .into_iter()
        .all(forgecad_app_server_protocol::valid_stable_id)
    {
        return Err("R007B packaged lineage request is invalid.".into());
    }
    let plan_read = r007b_packaged_get_json(
        bridge.inner(),
        format!(
            "/api/v1/agent/projects/{}/reference-guided-rebuild-plans/{}",
            request.project_id, request.rebuild_plan_id
        ),
    )
    .await?;
    let change_set = r007b_packaged_get_json(
        bridge.inner(),
        format!(
            "/api/v1/agent/change-sets/{}",
            request.preview_change_set_id
        ),
    )
    .await?;
    let asset = r007b_packaged_get_json(
        bridge.inner(),
        format!(
            "/api/v1/agent/asset-versions/{}",
            request.confirmed_asset_version_id
        ),
    )
    .await?;
    let plan = plan_read
        .get("reference_guided_rebuild_plan")
        .ok_or_else(|| "R007B packaged plan is missing.".to_string())?;
    let analysis = plan_read
        .get("reference_surface_analysis")
        .ok_or_else(|| "R007B packaged analysis is missing.".to_string())?;
    let pair = plan_read
        .get("reference_result_pair")
        .ok_or_else(|| "R007B packaged result pair is missing.".to_string())?;
    if plan.get("project_id").and_then(Value::as_str) != Some(&request.project_id)
        || plan.get("rebuild_plan_id").and_then(Value::as_str) != Some(&request.rebuild_plan_id)
        || plan.get("preview_change_set_id").and_then(Value::as_str)
            != Some(&request.preview_change_set_id)
        || plan
            .get("confirmed_asset_version_id")
            .and_then(Value::as_str)
            != Some(&request.confirmed_asset_version_id)
        || plan.get("status").and_then(Value::as_str) != Some("confirmed")
        || change_set.get("status").and_then(Value::as_str) != Some("confirmed")
        || change_set
            .get("resulting_asset_version_id")
            .and_then(Value::as_str)
            != Some(&request.confirmed_asset_version_id)
    {
        return Err("R007B packaged lineage identities diverged.".into());
    }
    let operations = change_set
        .get("operations")
        .and_then(Value::as_array)
        .ok_or_else(|| "R007B packaged sealed operations are missing.".to_string())?;
    let sealed_operations = operations
        .iter()
        .map(|operation| {
            let mut value = json!({
                "op": operation.get("op").and_then(Value::as_str),
                "sha256": semantic_sha256(operation).map_err(|error| error.to_string())?,
            });
            if operation.get("op").and_then(Value::as_str) == Some("apply_surface_adornment") {
                let program = operation
                    .get("surface_adornment_program")
                    .ok_or_else(|| "R007B packaged adornment program is missing.".to_string())?;
                value["program_sha256"] =
                    Value::String(semantic_sha256(program).map_err(|error| error.to_string())?);
            }
            Ok::<Value, String>(value)
        })
        .collect::<Result<Vec<_>, _>>()?;
    if !sealed_operations
        .iter()
        .any(|operation| operation["op"] == "apply_surface_adornment")
    {
        return Err("R007B packaged result has no sealed A005 effect.".into());
    }
    let shape_operations = asset
        .pointer("/shape_program/operations")
        .and_then(Value::as_array)
        .ok_or_else(|| "R007B packaged ShapeProgram operations are missing.".to_string())?;
    let mut operation_kinds = shape_operations
        .iter()
        .filter_map(|operation| operation.get("op").and_then(Value::as_str))
        .map(str::to_string)
        .collect::<Vec<_>>();
    operation_kinds.sort();
    operation_kinds.dedup();
    let parts = asset
        .pointer("/assembly_graph/parts")
        .and_then(Value::as_array)
        .ok_or_else(|| "R007B packaged C106 parts are missing.".to_string())?;
    let mut zones = parts
        .iter()
        .flat_map(|part| {
            part.get("material_zone_ids")
                .or_else(|| part.get("material_zones"))
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(str::to_string)
        })
        .collect::<Vec<_>>();
    zones.sort();
    zones.dedup();
    let instances = asset
        .pointer("/assembly_graph/component_recipe_instances")
        .and_then(Value::as_array)
        .ok_or_else(|| "R007B packaged C106 Recipe instances are missing.".to_string())?;
    let root_recipe_id = instances
        .iter()
        .find(|instance| {
            instance
                .get("parent_instance_id")
                .is_some_and(Value::is_null)
        })
        .and_then(|instance| instance.pointer("/recipe/recipe_id"))
        .and_then(Value::as_str)
        .ok_or_else(|| "R007B packaged C106 root Recipe is missing.".to_string())?;
    let result_glb_sha256 = pair
        .get("result_glb_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| "R007B packaged result GLB identity is missing.".to_string())?;
    Ok(json!({
        "reference_class": config.reference_class,
        "capability_ceiling": analysis.get("fidelity_ceiling"),
        "analysis": {
            "analysis_id": analysis.get("analysis_id"),
            "sha256": semantic_sha256(analysis).map_err(|error| error.to_string())?,
            "evidence_id": analysis.get("evidence_id"),
            "source_object_sha256": analysis.get("source_object_sha256"),
            "fidelity_ceiling": analysis.get("fidelity_ceiling"),
            "retained": plan.get("retained_evidence"),
            "intentionally_changed": plan.get("intended_differences"),
            "unresolved": plan.get("unresolved_uncertainties"),
        },
        "plan": {
            "rebuild_plan_id": plan.get("rebuild_plan_id"),
            "sha256": semantic_sha256(plan).map_err(|error| error.to_string())?,
            "analysis_id": analysis.get("analysis_id"),
            "evidence_id": plan.get("evidence_id"),
            "source_object_sha256": analysis.get("source_object_sha256"),
            "base_asset_version_id": plan.get("base_asset_version_id"),
            "confirmed_asset_version_id": plan.get("confirmed_asset_version_id"),
            "capability_ceiling": analysis.get("fidelity_ceiling"),
            "status": plan.get("status"),
        },
        "sealed_effect": {
            "change_set_id": change_set.get("change_set_id"),
            "sha256": semantic_sha256(&change_set).map_err(|error| error.to_string())?,
            "base_asset_version_id": change_set.get("base_asset_version_id"),
            "resulting_asset_version_id": change_set.get("resulting_asset_version_id"),
            "status": change_set.get("status"),
            "operations": sealed_operations,
        },
        "result_glb_sha256": result_glb_sha256,
        "geometry_readback": {
            "artifact_profile_id": "production_concept",
            "asset_kind": "c106_robotic_arm",
            "root_recipe_id": root_recipe_id,
            "root_operation_kind": operation_kinds.first(),
            "shape_operation_kinds": operation_kinds,
            "part_count": parts.len(),
            "material_zone_count": zones.len(),
            "glb_sha256": result_glb_sha256,
        },
    }))
}

fn arm_webview_qa_png_dimensions(bytes: &[u8]) -> Result<(u32, u32), String> {
    const PNG_SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";
    if bytes.len() < 45 || !bytes.starts_with(PNG_SIGNATURE) {
        return Err("Mechanical-arm WebView QA screenshot is not a PNG.".into());
    }
    let mut offset = 8usize;
    let mut dimensions = None;
    let mut saw_idat = false;
    while offset
        .checked_add(12)
        .map_or(false, |end| end <= bytes.len())
    {
        let length =
            u32::from_be_bytes(bytes[offset..offset + 4].try_into().unwrap_or([0; 4])) as usize;
        let data_start = offset + 8;
        let data_end = data_start
            .checked_add(length)
            .ok_or_else(|| "Mechanical-arm WebView QA screenshot chunk overflowed.".to_string())?;
        let chunk_end = data_end
            .checked_add(4)
            .ok_or_else(|| "Mechanical-arm WebView QA screenshot chunk overflowed.".to_string())?;
        if chunk_end > bytes.len() {
            return Err("Mechanical-arm WebView QA screenshot is truncated.".into());
        }
        match &bytes[offset + 4..offset + 8] {
            b"IHDR" if dimensions.is_none() && length == 13 => {
                let width = u32::from_be_bytes(
                    bytes[data_start..data_start + 4]
                        .try_into()
                        .unwrap_or([0; 4]),
                );
                let height = u32::from_be_bytes(
                    bytes[data_start + 4..data_start + 8]
                        .try_into()
                        .unwrap_or([0; 4]),
                );
                dimensions = Some((width, height));
            }
            b"IDAT" if length > 0 => saw_idat = true,
            b"IEND" if length == 0 && chunk_end == bytes.len() => break,
            b"IEND" => return Err("Mechanical-arm WebView QA screenshot IEND is invalid.".into()),
            _ => {}
        }
        offset = chunk_end;
    }
    let (width, height) = dimensions
        .ok_or_else(|| "Mechanical-arm WebView QA screenshot IHDR is invalid.".to_string())?;
    if !saw_idat || offset >= bytes.len() || &bytes[offset + 4..offset + 8] != b"IEND" {
        return Err("Mechanical-arm WebView QA screenshot payload is invalid.".into());
    }
    if width < 320 || height < 240 {
        return Err("Mechanical-arm WebView QA screenshot dimensions are too small.".into());
    }
    Ok((width, height))
}

fn arm_webview_qa_glb_readback(bytes: &[u8]) -> Result<(u64, u64), String> {
    const GLB_MAGIC: u32 = 0x4654_6c67;
    const GLB_JSON_CHUNK: u32 = 0x4e4f_534a;
    if bytes.len() < 20 {
        return Err("Mechanical-arm WebView QA GLB is truncated.".into());
    }
    let read_u32 = |offset: usize| -> Result<u32, String> {
        let slice = bytes
            .get(offset..offset + 4)
            .ok_or_else(|| "Mechanical-arm WebView QA GLB is truncated.".to_string())?;
        Ok(u32::from_le_bytes(slice.try_into().map_err(|_| {
            "Mechanical-arm WebView QA GLB is malformed.".to_string()
        })?))
    };
    if read_u32(0)? != GLB_MAGIC || read_u32(4)? != 2 || read_u32(8)? as usize != bytes.len() {
        return Err("Mechanical-arm WebView QA GLB header is invalid.".into());
    }
    let json_length = read_u32(12)? as usize;
    if read_u32(16)? != GLB_JSON_CHUNK
        || 20usize
            .checked_add(json_length)
            .map_or(true, |end| end > bytes.len())
    {
        return Err("Mechanical-arm WebView QA GLB JSON chunk is invalid.".into());
    }
    let json_end = 20 + json_length;
    let json_bytes = bytes
        .get(20..json_end)
        .ok_or_else(|| "Mechanical-arm WebView QA GLB JSON is truncated.".to_string())?;
    let document: serde_json::Value = serde_json::from_slice(json_bytes)
        .map_err(|_| "Mechanical-arm WebView QA GLB JSON cannot be decoded.".to_string())?;
    if document
        .pointer("/asset/version")
        .and_then(serde_json::Value::as_str)
        != Some("2.0")
    {
        return Err("Mechanical-arm WebView QA GLB version is invalid.".into());
    }
    let accessors = document
        .get("accessors")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| "Mechanical-arm WebView QA GLB accessors are missing.".to_string())?;
    let meshes = document
        .get("meshes")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| "Mechanical-arm WebView QA GLB meshes are missing.".to_string())?;
    let mut triangle_count = 0u64;
    for mesh in meshes {
        let primitives = mesh
            .get("primitives")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| "Mechanical-arm WebView QA GLB primitives are missing.".to_string())?;
        for primitive in primitives {
            if primitive
                .get("mode")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(4)
                != 4
            {
                return Err("Mechanical-arm WebView QA GLB uses a non-triangle primitive.".into());
            }
            let accessor_index = primitive
                .get("indices")
                .and_then(serde_json::Value::as_u64)
                .and_then(|index| usize::try_from(index).ok())
                .ok_or_else(|| {
                    "Mechanical-arm WebView QA GLB primitive indices are missing.".to_string()
                })?;
            let index_count = accessors
                .get(accessor_index)
                .and_then(|accessor| accessor.get("count"))
                .and_then(serde_json::Value::as_u64)
                .ok_or_else(|| {
                    "Mechanical-arm WebView QA GLB index accessor is invalid.".to_string()
                })?;
            if index_count == 0 || index_count % 3 != 0 {
                return Err("Mechanical-arm WebView QA GLB triangle indices are invalid.".into());
            }
            triangle_count = triangle_count.checked_add(index_count / 3).ok_or_else(|| {
                "Mechanical-arm WebView QA GLB triangle count overflowed.".to_string()
            })?;
        }
    }
    let complete_pbr_material_count = document
        .get("materials")
        .and_then(serde_json::Value::as_array)
        .map(|materials| {
            materials
                .iter()
                .filter(|material| {
                    let Some(pbr) = material.get("pbrMetallicRoughness") else {
                        return false;
                    };
                    pbr.get("baseColorTexture").is_some()
                        && pbr.get("metallicRoughnessTexture").is_some()
                        && material.get("normalTexture").is_some()
                        && material.get("occlusionTexture").is_some()
                        && material.get("emissiveTexture").is_some()
                })
                .count() as u64
        })
        .unwrap_or(0);
    if triangle_count == 0 || complete_pbr_material_count == 0 {
        return Err("Mechanical-arm WebView QA GLB production PBR readback is incomplete.".into());
    }
    Ok((triangle_count, complete_pbr_material_count))
}

fn c111b_packaged_webgl_glb_json(bytes: &[u8]) -> Result<Value, String> {
    const GLB_MAGIC: u32 = 0x4654_6c67;
    const GLB_JSON_CHUNK: u32 = 0x4e4f_534a;
    if bytes.len() < 20 {
        return Err("C111B packaged WebGL QA GLB is truncated.".into());
    }
    let read_u32 = |offset: usize| -> Result<u32, String> {
        let slice = bytes
            .get(offset..offset + 4)
            .ok_or_else(|| "C111B packaged WebGL QA GLB is truncated.".to_string())?;
        Ok(u32::from_le_bytes(slice.try_into().map_err(|_| {
            "C111B packaged WebGL QA GLB is malformed.".to_string()
        })?))
    };
    if read_u32(0)? != GLB_MAGIC || read_u32(4)? != 2 || read_u32(8)? as usize != bytes.len() {
        return Err("C111B packaged WebGL QA GLB header is invalid.".into());
    }
    let json_length = read_u32(12)? as usize;
    if read_u32(16)? != GLB_JSON_CHUNK
        || 20usize
            .checked_add(json_length)
            .map_or(true, |end| end > bytes.len())
    {
        return Err("C111B packaged WebGL QA GLB JSON chunk is invalid.".into());
    }
    serde_json::from_slice(
        bytes
            .get(20..20 + json_length)
            .ok_or_else(|| "C111B packaged WebGL QA GLB JSON is truncated.".to_string())?,
    )
    .map_err(|_| "C111B packaged WebGL QA GLB JSON cannot be decoded.".to_string())
}

fn validate_c111b_packaged_webgl_success(
    report: &C111bPackagedWebglQaReport,
    config: &C111bPackagedWebglQaConfig,
) -> Result<(), String> {
    let report_source_sha256 = report.source_sha256.as_deref().unwrap_or_default();
    let source_lineage_valid = if config.mode == "agent_asset" {
        validate_k001_probe_sha(report_source_sha256).is_ok()
            && (config.phase != "restart"
                || config.expected_export_sha256.as_deref() == Some(report_source_sha256))
    } else {
        report_source_sha256 == config.source_sha256
    };
    let complete_pbr_valid = if config.mode == "agent_asset" {
        report.complete_pbr_material_count
            == Some(C111B_PACKAGED_WEBGL_AGENT_V2_COMPLETE_PBR_MATERIALS)
    } else {
        report.complete_pbr_material_count.unwrap_or_default() > 0
    };
    if report.error_code.is_some()
        || !source_lineage_valid
        || report.triangle_count != Some(config.triangle_count)
        || report.primitive_count != Some(config.primitive_count)
        || report.material_count != Some(config.material_count)
        || !complete_pbr_valid
        || report.renderer_generation.unwrap_or_default() == 0
        || report.active_webgl_contexts != Some(1)
        || report.canvas_count != Some(1)
        || report.blockout_glb_kind.as_deref()
            != Some(if config.mode == "agent_asset" {
                "compiled_agent_production_pbr"
            } else {
                "external_reference"
            })
        || report.render_source.as_deref()
            != Some(if config.mode == "agent_asset" {
                "glb_pbr"
            } else {
                "external_reference"
            })
        || report.light_preset.as_deref() != Some("soft_studio")
        || report.renderer_id.as_deref() != Some(C111B_PACKAGED_WEBGL_PBR_RENDERER_ID)
        || report.render_manifest_sha256.as_deref()
            != Some(C111B_PACKAGED_WEBGL_PBR_RENDER_MANIFEST_SHA256)
        || report.visual_environment_id.as_deref()
            != Some(C111B_PACKAGED_WEBGL_PBR_VISUAL_ENVIRONMENT_ID)
        || report.visual_environment_sha256.as_deref()
            != Some(C111B_PACKAGED_WEBGL_PBR_VISUAL_ENVIRONMENT_SHA256)
        || report.output_color_space.as_deref() != Some("srgb")
        || report.tone_mapping.as_deref() != Some("aces_filmic")
        || report.pbr_texture_count.is_none_or(|value| value < 5)
        || report.pbr_color_spaces.as_deref() != Some("valid")
        || report.pbr_sampling_valid.as_deref() != Some("true")
        || report.formal_eligible != Some(false)
        || report.human_benchmark_evidence != Some(false)
        || report.reference_comparison != Some(false)
        || report.provider_protocol_requests
            != Some(
                if config.mode == "agent_asset" && config.phase == "initial" {
                    1
                } else {
                    0
                },
            )
        || report.network_provider_calls != Some(0)
        || report.network_call_made != Some(false)
        || report.credential_reads != Some(0)
        || report.billable_variable_cost_microusd != Some(0)
        || report.billable_variable_cost_source.as_deref()
            != Some(if config.mode == "agent_asset" {
                "native_offline_no_billable_transport"
            } else {
                "native_no_agent_provider_path"
            })
        || report.provider_metrics_source.as_deref()
            != Some(
                if config.mode == "agent_asset" && config.phase == "initial" {
                    "rust_terminal_turn_plus_native_local_mvp_counter"
                } else if config.mode == "agent_asset" {
                    "native_local_mvp_atomic_counter"
                } else {
                    "native_no_agent_provider_path"
                },
            )
        || report.credential_metrics_source.as_deref()
            != Some(if config.mode == "agent_asset" {
                "native_structural_no_credential_source"
            } else {
                "native_no_agent_provider_path"
            })
    {
        return Err(
            "C111B packaged WebGL QA renderer, source or non-claim facts are invalid.".into(),
        );
    }
    validate_c111b_packaged_metrics(report, config)?;
    let project_id = report
        .project_id
        .as_deref()
        .ok_or_else(|| "C111B packaged WebGL QA project ID is missing.".to_string())?;
    let asset_version_id = report
        .asset_version_id
        .as_deref()
        .ok_or_else(|| "C111B packaged WebGL QA asset version ID is missing.".to_string())?;
    if !forgecad_app_server_protocol::valid_stable_id(project_id)
        || !forgecad_app_server_protocol::valid_stable_id(asset_version_id)
        || report.snapshot_revision.unwrap_or_default() == 0
    {
        return Err("C111B packaged WebGL QA product identity is invalid.".into());
    }
    if config.phase == "restart"
        && (config.expected_project_id.as_deref() != Some(project_id)
            || config.expected_asset_version_id.as_deref() != Some(asset_version_id)
            || config.expected_snapshot_revision != report.snapshot_revision)
    {
        return Err("C111B packaged WebGL QA restart Snapshot lineage diverged.".into());
    }
    let readback = report
        .readback
        .as_ref()
        .ok_or_else(|| "C111B packaged WebGL QA exact readback is missing.".to_string())?;
    if readback.project_id != project_id
        || readback.asset_version_id != asset_version_id
        || readback.source_sha256 != report_source_sha256
        || (config.mode == "agent_asset"
            && (readback.shape_program_schema != "ShapeProgram@1"
                || readback.external_reference
                || readback.glb_byte_size.is_none()
                || readback.glb_triangle_count != Some(config.triangle_count)
                || readback.glb_primitive_count != Some(config.primitive_count)
                || readback.glb_material_count != Some(config.material_count)))
        || (config.mode == "external_reference"
            && (readback.shape_program_schema != "ExternalGLBReference@1"
                || !readback.external_reference))
    {
        return Err("C111B packaged WebGL QA exact readback lineage is invalid.".into());
    }
    let captures = report
        .captures
        .as_ref()
        .ok_or_else(|| "C111B packaged WebGL QA fixed-view captures are missing.".to_string())?;
    let expected_views = [
        "iso",
        "front",
        "back",
        "left",
        "right",
        "top",
        "gripper_iso",
        "gripper_front",
    ];
    if captures.len() != expected_views.len()
        || expected_views.iter().any(|view| {
            captures
                .iter()
                .filter(|capture| capture.view_id == *view)
                .count()
                != 1
        })
    {
        return Err("C111B packaged WebGL QA fixed-view set is incomplete or duplicated.".into());
    }
    for capture in captures {
        let expected_path = format!(
            "qa-artifacts/c111b-webgl/{}/{}.png",
            config.phase, capture.view_id
        );
        if capture.relative_path != expected_path
            || capture.source_sha256 != report_source_sha256
            || validate_k001_probe_sha(&capture.sha256).is_err()
            || capture.byte_size == 0
            || capture.width < 320
            || capture.height < 240
            || capture.auxiliary_relative_path.as_deref() != Some(
                format!(
                    "qa-artifacts/c111b-webgl/{}/{}.auxiliary.png",
                    config.phase, capture.view_id
                )
                .as_str(),
            )
            || capture.auxiliary_width != Some(960)
            || capture.auxiliary_height != Some(640)
            || capture.auxiliary_pass_ids.as_ref().is_none_or(|ids| {
                ids.iter().map(String::as_str).collect::<Vec<_>>()
                    != ["silhouette", "normal", "depth", "part_id", "material_id"]
            })
            || capture.auxiliary_byte_size != Some(960 * 640 * 4)
            || capture
                .auxiliary_sha256
                .as_deref()
                .is_none_or(|value| validate_k001_probe_sha(value).is_err())
        {
            return Err("C111B packaged WebGL QA screenshot receipt is invalid.".into());
        }
        let readability = capture.readability.as_ref().ok_or_else(|| {
            "C111B packaged WebGL QA screenshot readability is missing.".to_string()
        })?;
        if readability.pixel_encoding != "display_srgb"
            || readability.display_transfer != "wkwebview_linear_lit_surface_to_srgb"
            || readability.sample_pixel_count != 96 * 96
            || readability.foreground_pixel_count == 0
            || readability.foreground_coverage_bps < 100
            || readability.foreground_median_luma < 24
            || readability.foreground_readable_bps < 5000
        {
            return Err("C111B packaged WebGL QA screenshot readability is invalid.".into());
        }
    }
    if config.phase == "initial" && report.restart_hydrated != Some(false) {
        return Err("C111B packaged WebGL QA initial report has invalid restart state.".into());
    }
    if config.phase == "restart" && report.restart_hydrated != Some(true) {
        return Err("C111B packaged WebGL QA restart report has invalid hydration state.".into());
    }
    Ok(())
}

fn validate_c111b_packaged_metrics(
    report: &C111bPackagedWebglQaReport,
    config: &C111bPackagedWebglQaConfig,
) -> Result<(), String> {
    let agent_initial = config.mode == "agent_asset" && config.phase == "initial";
    if agent_initial {
        let expected_pipeline = [
            "author",
            "lower",
            "compile_readback",
            "render",
            "evaluate",
            "preview",
        ];
        let phase_timings = report
            .turn_phase_timings_ms
            .as_ref()
            .ok_or_else(|| "C111B packaged QA Turn timing evidence is missing.".to_string())?;
        if report.product_tool_calls != Some(6)
            || report.input_tokens != Some(1)
            || report.output_tokens != Some(1)
            || report.prompt_cache_hit_tokens != Some(0)
            || report.prompt_cache_miss_tokens != Some(0)
            || report.same_intent_repair_attempts != Some(0)
            || report.same_intent_repairs_applied != Some(0)
            || report.provider_schema_repair_requests != Some(0)
            || report.product_tool_schema_repair_requests != Some(0)
            || report.estimated_cost_microusd != Some(1)
            || report
                .turn_total_elapsed_ms
                .is_none_or(|value| value == 0 || value > 300_000)
            || phase_timings.len() != expected_pipeline.len()
            || expected_pipeline
                .iter()
                .any(|stage| !phase_timings.contains_key(*stage))
            || report
                .turn_trace_sha256
                .as_deref()
                .is_none_or(|value| validate_k001_probe_sha(value).is_err())
            || report.turn_metrics_source.as_deref() != Some("rust_terminal_turn_readback")
        {
            return Err(
                "C111B packaged QA terminal Turn usage, repair, cost or timing facts are invalid."
                    .into(),
            );
        }
    } else if report.product_tool_calls != Some(0)
        || report.input_tokens != Some(0)
        || report.output_tokens != Some(0)
        || report.prompt_cache_hit_tokens != Some(0)
        || report.prompt_cache_miss_tokens != Some(0)
        || report.same_intent_repair_attempts != Some(0)
        || report.same_intent_repairs_applied != Some(0)
        || report.provider_schema_repair_requests != Some(0)
        || report.product_tool_schema_repair_requests != Some(0)
        || report.estimated_cost_microusd != Some(0)
        || report.turn_total_elapsed_ms != Some(0)
        || report
            .turn_phase_timings_ms
            .as_ref()
            .is_none_or(|value| !value.is_empty())
        || report.turn_trace_sha256.is_some()
        || (config.mode == "agent_asset"
            && report.turn_metrics_source.as_deref() != Some("native_no_turn_on_restart"))
    {
        return Err("C111B packaged QA non-Turn phase reported fabricated Turn metrics.".into());
    }

    let expected_stages: &[&str] = match (config.mode.as_str(), config.phase.as_str()) {
        ("agent_asset", "initial") => &[
            "agent_workbench_ready",
            "agent_brief_sent",
            "agent_v1_confirmed",
            "agent_selection_card_ready",
            "agent_link_part_selected",
            "agent_adornment_drawer_ready",
            "agent_v2_confirmed",
            "agent_export_readback_ready",
            "agent_captures_ready",
            "report_received",
        ],
        ("agent_asset", "restart") => &[
            "agent_restart_workbench_ready",
            "agent_restart_snapshot_hydrated",
            "agent_restart_export_readback_ready",
            "agent_restart_captures_ready",
            "report_received",
        ],
        ("external_reference", "initial") => &[
            "workbench_ready",
            "visible_import_requested",
            "external_asset_ready",
            "external_captures_ready",
            "report_received",
        ],
        ("external_reference", "restart") => &[
            "external_restart_workbench_ready",
            "external_restart_snapshot_hydrated",
            "external_restart_captures_ready",
            "report_received",
        ],
        _ => return Err("C111B packaged QA timing mode or phase is invalid.".into()),
    };
    let timings = report
        .stage_timings
        .as_ref()
        .ok_or_else(|| "C111B packaged QA native lifecycle timing is missing.".to_string())?;
    let mut previous = 0u64;
    let duration_invalid = timings.iter().any(|timing| {
        let invalid =
            timing.duration_since_previous_ms != timing.elapsed_ms.saturating_sub(previous);
        previous = timing.elapsed_ms;
        invalid
    });
    if timings.len() != expected_stages.len()
        || timings
            .iter()
            .zip(expected_stages)
            .any(|(timing, expected)| timing.stage != *expected || timing.elapsed_ms > 900_000)
        || timings
            .windows(2)
            .any(|pair| pair[0].elapsed_ms > pair[1].elapsed_ms)
        || duration_invalid
        || report.end_to_end_elapsed_ms != timings.last().map(|timing| timing.elapsed_ms)
        || report.timing_metrics_source.as_deref() != Some("native_monotonic_progress_receipts")
    {
        return Err("C111B packaged QA native lifecycle timing is invalid.".into());
    }
    Ok(())
}

#[tauri::command]
fn forgecad_arm_webview_qa_report(report: ArmWebviewQaReport) -> Result<(), String> {
    let Some(config) = forgecad_arm_webview_qa_config()? else {
        return Err("Mechanical-arm WebView QA reporting is disabled.".into());
    };
    if report.schema_version != ARM_WEBVIEW_QA_SCHEMA || report.phase != config.phase {
        return Err("Mechanical-arm WebView QA report identity is invalid.".into());
    }
    if !report.ok {
        let code = report
            .error_code
            .as_deref()
            .ok_or_else(|| "Mechanical-arm WebView QA failure requires error_code.".to_string())?;
        if !forgecad_app_server_protocol::valid_stable_id(code) {
            return Err("Mechanical-arm WebView QA error_code is invalid.".into());
        }
    } else {
        validate_arm_webview_qa_success(&report)?;
        if config.r007b_visual_evidence && config.phase == "initial" {
            validate_r007b_packaged_visual_run(&report, &config.reference_class)?;
        } else if report.r007b_visual_run.is_some() {
            return Err("R007B packaged visual evidence was not requested.".into());
        }
        if config.phase == "restart" {
            validate_arm_webview_restart_expected(&report)?;
        }
    }
    let encoded = serde_json::to_string(&report)
        .map_err(|_| "Mechanical-arm WebView QA report could not be serialized.".to_string())?;
    append_supervisor_log(&format!("{ARM_WEBVIEW_QA_MARKER}{encoded}"));
    Ok(())
}

fn validate_r007b_packaged_visual_run(
    report: &ArmWebviewQaReport,
    reference_class: &str,
) -> Result<(), String> {
    let run = report
        .r007b_visual_run
        .as_ref()
        .ok_or_else(|| "R007B packaged visual run is missing.".to_string())?;
    if report.phase != "initial"
        || run.get("reference_class").and_then(Value::as_str) != Some(reference_class)
        || run
            .pointer("/workbench/runtime_kind")
            .and_then(Value::as_str)
            != Some("packaged_tauri_webview")
        || run
            .pointer("/workbench/real_workbench")
            .and_then(Value::as_bool)
            != Some(true)
        || run
            .pointer("/workbench/fixture_or_proxy_used")
            .and_then(Value::as_bool)
            != Some(false)
        || run
            .pointer("/renderer/same_renderer")
            .and_then(Value::as_bool)
            != Some(true)
        || run
            .pointer("/renderer/canvas_count")
            .and_then(Value::as_u64)
            != Some(1)
        || run
            .pointer("/renderer/active_webgl_contexts")
            .and_then(Value::as_u64)
            != Some(1)
    {
        return Err("R007B packaged visual runtime proof is invalid.".into());
    }
    let expected_reference = format!("captures/{reference_class}/reference.png");
    let expected_result = format!("captures/{reference_class}/result.png");
    for (kind, expected) in [
        ("reference", expected_reference),
        ("result", expected_result),
    ] {
        let capture = run
            .pointer(&format!("/screenshots/{kind}"))
            .ok_or_else(|| "R007B packaged paired capture is missing.".to_string())?;
        let expected_displayed_kind = if kind == "reference" {
            "same_renderer_read_only_reference"
        } else {
            "production_result"
        };
        if capture.get("capture_kind").and_then(Value::as_str) != Some(kind)
            || capture
                .get("displayed_reference_kind")
                .and_then(Value::as_str)
                != Some(expected_displayed_kind)
            || capture.get("relative_path").and_then(Value::as_str) != Some(expected.as_str())
            || capture
                .get("width")
                .and_then(Value::as_u64)
                .unwrap_or_default()
                < 320
            || capture
                .get("height")
                .and_then(Value::as_u64)
                .unwrap_or_default()
                < 240
            || capture.get("renderer_generation").and_then(Value::as_u64)
                != run
                    .pointer("/renderer/renderer_generation")
                    .and_then(Value::as_u64)
            || capture
                .get("sha256")
                .and_then(Value::as_str)
                .is_none_or(|value| validate_k001_probe_sha(value).is_err())
        {
            return Err("R007B packaged paired capture receipt is invalid.".into());
        }
    }
    let glb = report
        .v3_production_glb
        .as_ref()
        .ok_or_else(|| "R007B packaged V3 GLB receipt is missing.".to_string())?;
    if run.get("result_glb_sha256").and_then(Value::as_str) != Some(glb.sha256.as_str())
        || run
            .pointer("/geometry_readback/glb_sha256")
            .and_then(Value::as_str)
            != Some(glb.sha256.as_str())
        || run
            .pointer("/geometry_readback/triangle_count")
            .and_then(Value::as_u64)
            != Some(glb.triangle_count)
        || glb.triangle_count < 1_000
        || run
            .pointer("/geometry_readback/asset_kind")
            .and_then(Value::as_str)
            != Some("c106_robotic_arm")
        || run
            .pointer("/sealed_effect/operations")
            .and_then(Value::as_array)
            .is_none_or(|operations| {
                !operations.iter().any(|operation| {
                    operation.get("op").and_then(Value::as_str) == Some("apply_surface_adornment")
                })
            })
    {
        return Err("R007B packaged exact lineage or geometry readback is invalid.".into());
    }
    Ok(())
}

#[tauri::command]
fn forgecad_arm_webview_qa_progress(stage: String) -> Result<(), String> {
    if forgecad_arm_webview_qa_config()?.is_none() {
        return Err("Mechanical-arm WebView QA progress is disabled.".into());
    }
    if !matches!(
        stage.as_str(),
        "workbench_ready"
            | "project_ready"
            | "brief_sent"
            | "single_result_ready"
            | "viewport_wait_started"
            | "viewport_profile_invalid"
            | "viewport_load_failed"
            | "viewport_load_timeout"
            | "viewport_render_source_invalid"
            | "viewport_pending_preview_ready"
            | "viewport_pending_production_loading"
            | "viewport_pending_production_source"
            | "viewport_pending_empty"
            | "viewport_pending_other"
            | "preview_ready"
            | "v1_confirmed"
            | "part_selected"
            | "a005_open"
            | "a005_primary_ready"
            | "a005_primary_clicked"
            | "a005_activation_ready"
            | "a005_preview_clicked"
            | "a005_retain_ready"
            | "a005_retained"
            | "v2_ready"
            | "r007b_menu_open"
            | "r007b_drawer_open"
            | "r007b_file_selected"
            | "r007b_evidence_save_requested"
            | "r007b_evidence_saved"
            | "r007b_preview_requested"
            | "r007b_preview_ready"
            | "r007b_retain_requested"
            | "r007b_retained"
            | "v3_ready"
            | "v3_glb_downloaded"
            | "restart_hydrated"
    ) {
        return Err("Mechanical-arm WebView QA progress stage is invalid.".into());
    }
    append_supervisor_log(&format!("{ARM_WEBVIEW_QA_PROGRESS_MARKER}{stage}"));
    Ok(())
}

fn validate_arm_webview_qa_success(report: &ArmWebviewQaReport) -> Result<(), String> {
    if report.error_code.is_some() {
        return Err("Mechanical-arm WebView QA success report must not include error_code.".into());
    }
    for (field, value) in [
        ("project_id", report.project_id.as_deref()),
        ("v3_asset_version_id", report.v3_asset_version_id.as_deref()),
    ] {
        let value =
            value.ok_or_else(|| format!("Mechanical-arm WebView QA {field} is missing."))?;
        if !forgecad_app_server_protocol::valid_stable_id(value) {
            return Err(format!("Mechanical-arm WebView QA {field} is invalid."));
        }
    }
    if report.snapshot_revision.unwrap_or_default() == 0
        || report.renderer_generation.unwrap_or_default() == 0
        || report.active_webgl_contexts != Some(1)
        || report.production_glb_render_source.as_deref() != Some("glb_pbr")
    {
        return Err("Mechanical-arm WebView QA renderer evidence is invalid.".into());
    }
    match report.phase.as_str() {
        "initial" => {
            for (field, value) in [
                ("turn_id", report.turn_id.as_deref()),
                ("preview_id", report.preview_id.as_deref()),
                ("v1_asset_version_id", report.v1_asset_version_id.as_deref()),
                ("v2_asset_version_id", report.v2_asset_version_id.as_deref()),
            ] {
                let value = value
                    .ok_or_else(|| format!("Mechanical-arm WebView QA {field} is missing."))?;
                if !forgecad_app_server_protocol::valid_stable_id(value) {
                    return Err(format!("Mechanical-arm WebView QA {field} is invalid."));
                }
            }
            let sha = report.preview_artifact_sha256.as_deref().ok_or_else(|| {
                "Mechanical-arm WebView QA preview_artifact_sha256 is missing.".to_string()
            })?;
            validate_k001_probe_sha(sha)?;
            if report.v1_asset_version_id == report.v2_asset_version_id
                || report.v2_asset_version_id == report.v3_asset_version_id
                || report.a005_preview_seen != Some(true)
                || report.r007b_preview_seen != Some(true)
                || report.r007b_v3_confirmed != Some(true)
                || report.v3_glb_download_confirmed != Some(true)
                || report.visual_fidelity_validated != Some(false)
                || report.restart_hydrated != Some(false)
            {
                return Err(
                    "Mechanical-arm WebView QA V1 to A005 V2 to R007B V3 evidence is invalid."
                        .into(),
                );
            }
            validate_arm_webview_qa_glb_capture(
                report.v3_production_glb.as_ref(),
                "qa-artifacts/arm-webview/initial/v3_production_glb.glb",
            )?;
            validate_arm_webview_qa_png_capture(
                report.v3_viewport_screenshot.as_ref(),
                "qa-artifacts/arm-webview/initial/v3_viewport_png.png",
            )?;
        }
        "restart" => {
            if report.turn_id.is_some()
                || report.preview_id.is_some()
                || report.preview_artifact_sha256.is_some()
                || report.v1_asset_version_id.is_some()
                || report.v2_asset_version_id.is_some()
                || report.a005_preview_seen != Some(false)
                || report.r007b_preview_seen != Some(false)
                || report.r007b_v3_confirmed != Some(false)
                || report.v3_glb_download_confirmed != Some(false)
                || report.v3_production_glb.is_some()
                || report.v3_viewport_screenshot.is_some()
                || report.visual_fidelity_validated.is_some()
                || report.restart_hydrated != Some(true)
            {
                return Err(
                    "Mechanical-arm WebView QA restart report contains initial-only facts.".into(),
                );
            }
        }
        _ => return Err("Mechanical-arm WebView QA phase is invalid.".into()),
    }
    Ok(())
}

fn validate_arm_webview_qa_glb_capture(
    capture: Option<&ArmWebviewQaGlbCapture>,
    expected_relative_path: &str,
) -> Result<(), String> {
    let capture = capture
        .ok_or_else(|| "Mechanical-arm WebView QA V3 GLB capture is missing.".to_string())?;
    if capture.relative_path != expected_relative_path
        || validate_k001_probe_sha(&capture.sha256).is_err()
        || capture.byte_size == 0
        || !(12_000..=24_000).contains(&capture.triangle_count)
        || capture.complete_pbr_material_count == 0
    {
        return Err("Mechanical-arm WebView QA V3 GLB capture is invalid.".into());
    }
    Ok(())
}

fn validate_arm_webview_qa_png_capture(
    capture: Option<&ArmWebviewQaPngCapture>,
    expected_relative_path: &str,
) -> Result<(), String> {
    let capture = capture
        .ok_or_else(|| "Mechanical-arm WebView QA V3 screenshot capture is missing.".to_string())?;
    if capture.relative_path != expected_relative_path
        || validate_k001_probe_sha(&capture.sha256).is_err()
        || capture.byte_size == 0
        || capture.width < 320
        || capture.height < 240
    {
        return Err("Mechanical-arm WebView QA V3 screenshot capture is invalid.".into());
    }
    Ok(())
}

fn validate_arm_webview_restart_expected(report: &ArmWebviewQaReport) -> Result<(), String> {
    for (name, actual) in [
        (
            "FORGECAD_ARM_WEBVIEW_QA_EXPECT_PROJECT_ID",
            report.project_id.as_deref(),
        ),
        (
            "FORGECAD_ARM_WEBVIEW_QA_EXPECT_V3_ASSET_VERSION_ID",
            report.v3_asset_version_id.as_deref(),
        ),
    ] {
        let expected = k002_probe_stable_id_env(name)?;
        if actual != Some(expected.as_str()) {
            return Err("Mechanical-arm WebView QA restart lineage diverged.".into());
        }
    }
    let expected_revision = k002_probe_u64_env(
        "FORGECAD_ARM_WEBVIEW_QA_EXPECT_SNAPSHOT_REVISION",
        1,
        u64::MAX,
    )?;
    if report.snapshot_revision != Some(expected_revision) {
        return Err("Mechanical-arm WebView QA restart Snapshot diverged.".into());
    }
    Ok(())
}

#[tauri::command]
fn forgecad_k001_packaged_probe_config() -> Result<Option<K001PackagedProbeConfig>, String> {
    if env::var("FORGECAD_K001_PACKAGED_PROBE").as_deref() != Ok("1") {
        return Ok(None);
    }
    let configured = (|| {
        let phase = env::var("FORGECAD_K001_PACKAGED_PROBE_PHASE")
            .map_err(|_| "K001 packaged probe phase is missing.".to_string())?;
        let expected = match phase.as_str() {
            "initial" => None,
            "restart" => Some(K001PackagedProbeExpected {
                project_id: k001_probe_stable_id_env("FORGECAD_K001_EXPECT_PROJECT_ID")?,
                thread_id: k001_probe_stable_id_env("FORGECAD_K001_EXPECT_THREAD_ID")?,
                asset_version_id: k001_probe_stable_id_env(
                    "FORGECAD_K001_EXPECT_ASSET_VERSION_ID",
                )?,
                last_event_id: k001_probe_event_id_env("FORGECAD_K001_EXPECT_LAST_EVENT_ID")?,
                cursor: k001_probe_cursor_env("FORGECAD_K001_EXPECT_CURSOR")?,
                glb_sha256: k001_probe_sha_env("FORGECAD_K001_EXPECT_GLB_SHA256")?,
            }),
            _ => return Err("K001 packaged probe phase must be initial or restart.".to_string()),
        };
        Ok(K001PackagedProbeConfig {
            schema_version: K001_PACKAGED_PROBE_SCHEMA,
            phase,
            expected,
        })
    })();
    if configured.is_err() {
        let phase = env::var("FORGECAD_K001_PACKAGED_PROBE_PHASE")
            .ok()
            .filter(|value| matches!(value.as_str(), "initial" | "restart"))
            .unwrap_or_else(|| "initial".to_string());
        let failure = serde_json::json!({
            "schema_version": K001_PACKAGED_PROBE_SCHEMA,
            "phase": phase,
            "ok": false,
            "error_code": "PROBE_CONFIG_INVALID"
        });
        append_supervisor_log(&format!("{K001_PACKAGED_PROBE_MARKER}{failure}"));
    };
    configured.map(Some)
}

#[tauri::command]
fn forgecad_k001_packaged_probe_report(report: K001PackagedProbeReport) -> Result<(), String> {
    let Some(config) = forgecad_k001_packaged_probe_config()? else {
        return Err("K001 packaged probe reporting is disabled.".to_string());
    };
    finish_packaged_probe_report(
        move || {
            if report.schema_version != K001_PACKAGED_PROBE_SCHEMA || report.phase != config.phase {
                return Err("K001 packaged probe report identity is invalid.".to_string());
            }
            if !report.ok {
                let error_code = report.error_code.as_deref().ok_or_else(|| {
                    "K001 packaged probe failure requires error_code.".to_string()
                })?;
                if !forgecad_app_server_protocol::valid_stable_id(error_code) {
                    return Err("K001 packaged probe error_code is invalid.".to_string());
                }
                if let Some(diagnostic) = report.diagnostic.as_ref() {
                    if diagnostic.method.len() > 16
                        || diagnostic.route.len() > 120
                        || diagnostic.error_code.len() > 80
                        || diagnostic.phase != config.phase
                        || diagnostic.correlation_id.len() > 64
                    {
                        return Err("K001 packaged probe diagnostic is unbounded or mismatched."
                            .to_string());
                    }
                }
            } else {
                validate_k001_probe_success(&report)?;
                if let Some(expected) = config.expected {
                    if report.project_id.as_deref() != Some(expected.project_id.as_str())
                        || report.thread_id.as_deref() != Some(expected.thread_id.as_str())
                        || report.asset_version_id.as_deref()
                            != Some(expected.asset_version_id.as_str())
                        || report.resume_from_event_id.as_deref()
                            != Some(expected.last_event_id.as_str())
                        || report.resume_from_cursor.as_deref() != Some(expected.cursor.as_str())
                        || report.last_event_id.as_deref() != Some(expected.last_event_id.as_str())
                        || report.cursor.as_deref() != Some(expected.cursor.as_str())
                        || report.glb_sha256.as_deref() != Some(expected.glb_sha256.as_str())
                    {
                        return Err("K001 packaged restart probe diverged from first-run truth."
                            .to_string());
                    }
                    let checkpoint = expected
                        .last_event_id
                        .parse::<u64>()
                        .map_err(|_| "K001 packaged expected event ID is invalid.".to_string())?;
                    let first = report
                        .first_event_id
                        .as_deref()
                        .and_then(|value| value.parse::<u64>().ok())
                        .ok_or_else(|| {
                            "K001 packaged restart first event ID is missing.".to_string()
                        })?;
                    if first == 0 || first > checkpoint {
                        return Err(
                            "K001 packaged restart did not replay the persisted native Item interval."
                                .to_string(),
                        );
                    }
                }
            }
            let encoded = serde_json::to_string(&report)
                .map_err(|_| "K001 packaged probe report could not be serialized.".to_string())?;
            append_supervisor_log(&format!("{K001_PACKAGED_PROBE_MARKER}{encoded}"));
            Ok(())
        },
        signal_k001_packaged_probe_completion,
    )
}

fn validate_k001_probe_success(report: &K001PackagedProbeReport) -> Result<(), String> {
    for (field, value) in [
        ("project_id", report.project_id.as_deref()),
        ("thread_id", report.thread_id.as_deref()),
        ("asset_version_id", report.asset_version_id.as_deref()),
    ] {
        let value = value.ok_or_else(|| format!("K001 packaged probe {field} is missing."))?;
        if !forgecad_app_server_protocol::valid_stable_id(value) {
            return Err(format!("K001 packaged probe {field} is invalid."));
        }
    }
    let last_event_id = report
        .last_event_id
        .as_deref()
        .ok_or_else(|| "K001 packaged probe last_event_id is missing.".to_string())?;
    validate_k001_probe_event_id(last_event_id)?;
    let cursor = report
        .cursor
        .as_deref()
        .ok_or_else(|| "K001 packaged probe cursor is missing.".to_string())?;
    let decoded_cursor = forgecad_app_server_protocol::AppServerCursor::decode(cursor)
        .map_err(|_| "K001 packaged probe cursor is invalid.".to_string())?;
    let last_sequence = last_event_id
        .parse::<u64>()
        .map_err(|_| "K001 packaged probe last_event_id is invalid.".to_string())?;
    if decoded_cursor.thread_id != report.thread_id.as_deref().unwrap_or_default()
        || decoded_cursor.source_sequence != last_sequence
    {
        return Err("K001 packaged probe cursor is not bound to its Thread event.".to_string());
    }
    if let Some(first_event_id) = report.first_event_id.as_deref() {
        validate_k001_probe_event_id(first_event_id)?;
        if first_event_id.parse::<u64>().unwrap_or(u64::MAX) > last_sequence {
            return Err("K001 packaged probe event interval is invalid.".to_string());
        }
    }
    match (
        report.resume_from_event_id.as_deref(),
        report.resume_from_cursor.as_deref(),
    ) {
        (None, None) => {}
        (Some(resume_event_id), Some(resume_cursor)) => {
            validate_k001_probe_event_id(resume_event_id)?;
            let decoded_resume =
                forgecad_app_server_protocol::AppServerCursor::decode(resume_cursor)
                    .map_err(|_| "K001 packaged probe resume cursor is invalid.".to_string())?;
            if decoded_resume.thread_id != report.thread_id.as_deref().unwrap_or_default()
                || decoded_resume.source_sequence
                    != resume_event_id.parse::<u64>().unwrap_or_default()
            {
                return Err(
                    "K001 packaged probe resume cursor is not bound to its Thread event."
                        .to_string(),
                );
            }
        }
        _ => {
            return Err(
                "K001 packaged probe resume event and cursor must be supplied together."
                    .to_string(),
            );
        }
    }
    let glb_sha = report
        .glb_sha256
        .as_deref()
        .ok_or_else(|| "K001 packaged probe GLB SHA is missing.".to_string())?;
    validate_k001_probe_sha(glb_sha)?;
    let protocol_sha = report
        .protocol_glb_sha256
        .as_deref()
        .ok_or_else(|| "K001 packaged probe protocol GLB SHA is missing.".to_string())?;
    let resource_sha = report
        .resource_glb_sha256
        .as_deref()
        .ok_or_else(|| "K001 packaged probe resource GLB SHA is missing.".to_string())?;
    validate_k001_probe_sha(protocol_sha)?;
    validate_k001_probe_sha(resource_sha)?;
    if glb_sha != protocol_sha || glb_sha != resource_sha {
        return Err("K001 packaged probe GLB bytes disagree across transports.".to_string());
    }
    if report.notification_count.unwrap_or_default() == 0 {
        return Err("K001 packaged probe did not observe any persisted notification.".to_string());
    }
    if report.native_lifecycle_transport != Some(true)
        || report.native_item_replay_verified != Some(true)
        || report.product_state_owner.as_deref() != Some("rust_app_server")
        || report.python_product_api_used != Some(false)
        || report.turn_status.as_deref() != Some("failed")
        || report.turn_error_code.as_deref() != Some("PROVIDER_NOT_CONFIGURED")
        || report.provider_calls != Some(0)
    {
        return Err(
            "K001 packaged probe did not prove native lifecycle, native replay, and Rust product ownership."
                .to_string(),
        );
    }
    Ok(())
}

fn k001_probe_stable_id_env(name: &str) -> Result<String, String> {
    let value = env::var(name).map_err(|_| format!("{name} is missing."))?;
    if !forgecad_app_server_protocol::valid_stable_id(&value) {
        return Err(format!("{name} is invalid."));
    }
    Ok(value)
}

fn k001_probe_event_id_env(name: &str) -> Result<String, String> {
    let value = env::var(name).map_err(|_| format!("{name} is missing."))?;
    validate_k001_probe_event_id(&value)?;
    Ok(value)
}

fn validate_k001_probe_event_id(value: &str) -> Result<(), String> {
    if value.is_empty()
        || value.len() > 20
        || value
            .parse::<u64>()
            .ok()
            .filter(|number| *number > 0)
            .is_none()
    {
        return Err("K001 packaged probe event ID is invalid.".to_string());
    }
    Ok(())
}

fn k001_probe_cursor_env(name: &str) -> Result<String, String> {
    let value = env::var(name).map_err(|_| format!("{name} is missing."))?;
    forgecad_app_server_protocol::AppServerCursor::decode(&value)
        .map_err(|_| format!("{name} is invalid."))?;
    Ok(value)
}

fn k001_probe_sha_env(name: &str) -> Result<String, String> {
    let value = env::var(name).map_err(|_| format!("{name} is missing."))?;
    validate_k001_probe_sha(&value)?;
    Ok(value)
}

fn validate_k001_probe_sha(value: &str) -> Result<(), String> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("K001 packaged probe SHA-256 is invalid.".to_string());
    }
    Ok(())
}

#[tauri::command]
fn forgecad_k002_packaged_probe_config() -> Result<Option<K002PackagedProbeConfig>, String> {
    if env::var("FORGECAD_K002_PACKAGED_PROBE").as_deref() != Ok("1") {
        return Ok(None);
    }
    let configured = (|| {
        let phase = env::var("FORGECAD_K002_PACKAGED_PROBE_PHASE")
            .map_err(|_| "K002 packaged probe phase is missing.".to_string())?;
        let expected = match phase.as_str() {
            "initial" => None,
            "restart" => Some(K002PackagedProbeExpected {
                thread_id: k002_probe_stable_id_env("FORGECAD_K002_EXPECT_THREAD_ID")?,
                turn_id: k002_probe_stable_id_env("FORGECAD_K002_EXPECT_TURN_ID")?,
                items_sha256: k002_probe_sha_env("FORGECAD_K002_EXPECT_ITEMS_SHA256")?,
                item_count: k002_probe_u64_env("FORGECAD_K002_EXPECT_ITEM_COUNT", 2, 200)?,
                last_sequence: k002_probe_u64_env(
                    "FORGECAD_K002_EXPECT_LAST_SEQUENCE",
                    1,
                    u64::MAX,
                )?,
                turn_error_code: k002_probe_stable_id_env("FORGECAD_K002_EXPECT_TURN_ERROR_CODE")?,
            }),
            _ => return Err("K002 packaged probe phase must be initial or restart.".to_string()),
        };
        Ok(K002PackagedProbeConfig {
            schema_version: K002_PACKAGED_PROBE_SCHEMA,
            phase,
            expected,
        })
    })();
    if configured.is_err() {
        let phase = env::var("FORGECAD_K002_PACKAGED_PROBE_PHASE")
            .ok()
            .filter(|value| matches!(value.as_str(), "initial" | "restart"))
            .unwrap_or_else(|| "initial".to_string());
        let failure = serde_json::json!({
            "schema_version": K002_PACKAGED_PROBE_SCHEMA,
            "phase": phase,
            "ok": false,
            "error_code": "PROBE_CONFIG_INVALID"
        });
        append_supervisor_log(&format!("{K002_PACKAGED_PROBE_MARKER}{failure}"));
    }
    configured.map(Some)
}

#[tauri::command]
fn forgecad_k002_packaged_probe_report(report: K002PackagedProbeReport) -> Result<(), String> {
    let Some(config) = forgecad_k002_packaged_probe_config()? else {
        return Err("K002 packaged probe reporting is disabled.".to_string());
    };
    finish_packaged_probe_report(
        move || {
            if report.schema_version != K002_PACKAGED_PROBE_SCHEMA || report.phase != config.phase {
                return Err("K002 packaged probe report identity is invalid.".to_string());
            }
            if !report.ok {
                let error_code = report.error_code.as_deref().ok_or_else(|| {
                    "K002 packaged probe failure requires error_code.".to_string()
                })?;
                if !forgecad_app_server_protocol::valid_stable_id(error_code) {
                    return Err("K002 packaged probe error_code is invalid.".to_string());
                }
            } else {
                validate_k002_probe_success(&report)?;
                if let Some(expected) = config.expected {
                    if report.thread_id.as_deref() != Some(expected.thread_id.as_str())
                        || report.turn_id.as_deref() != Some(expected.turn_id.as_str())
                        || report.items_sha256.as_deref() != Some(expected.items_sha256.as_str())
                        || report.item_count != Some(expected.item_count)
                        || report.last_sequence != Some(expected.last_sequence)
                        || report.turn_error_code.as_deref()
                            != Some(expected.turn_error_code.as_str())
                    {
                        return Err(
                            "K002 packaged restart probe diverged from first-run lifecycle truth."
                                .to_string(),
                        );
                    }
                }
            }
            let encoded = serde_json::to_string(&report)
                .map_err(|_| "K002 packaged probe report could not be serialized.".to_string())?;
            append_supervisor_log(&format!("{K002_PACKAGED_PROBE_MARKER}{encoded}"));
            Ok(())
        },
        signal_k002_packaged_probe_completion,
    )
}

fn validate_k002_probe_success(report: &K002PackagedProbeReport) -> Result<(), String> {
    for (field, value) in [
        ("thread_id", report.thread_id.as_deref()),
        ("turn_id", report.turn_id.as_deref()),
        ("turn_error_code", report.turn_error_code.as_deref()),
    ] {
        let value = value.ok_or_else(|| format!("K002 packaged probe {field} is missing."))?;
        if !forgecad_app_server_protocol::valid_stable_id(value) {
            return Err(format!("K002 packaged probe {field} is invalid."));
        }
    }
    if report.turn_status.as_deref() != Some("failed")
        || report.provider_status.as_deref() != Some("unconfigured")
        || report.provider_configured != Some(false)
        || report.provider_network_call_made != Some(false)
        || report.supervisor_running != Some(true)
        || report.supervisor_state.as_deref() != Some("running")
        || report.supervisor_managed_by_desktop != Some(true)
        || report.reasoning_content_present != Some(false)
        || report.legacy_lifecycle_post_status != Some(410)
        || report.provider_calls != Some(0)
    {
        return Err(
            "K002 packaged probe did not prove the closed unconfigured no-network lifecycle."
                .to_string(),
        );
    }

    let item_count = report
        .item_count
        .filter(|count| (2..=200).contains(count))
        .ok_or_else(|| "K002 packaged probe item_count is invalid.".to_string())?;
    let last_sequence = report
        .last_sequence
        .filter(|sequence| *sequence > 0)
        .ok_or_else(|| "K002 packaged probe last_sequence is invalid.".to_string())?;
    let sequences = report
        .item_sequences
        .as_deref()
        .ok_or_else(|| "K002 packaged probe item_sequences are missing.".to_string())?;
    let item_ids = report
        .item_ids
        .as_deref()
        .ok_or_else(|| "K002 packaged probe item_ids are missing.".to_string())?;
    let item_types = report
        .item_types
        .as_deref()
        .ok_or_else(|| "K002 packaged probe item_types are missing.".to_string())?;
    if sequences.len() as u64 != item_count
        || item_ids.len() as u64 != item_count
        || item_types.len() as u64 != item_count
        || sequences.last().copied() != Some(last_sequence)
        || sequences
            .windows(2)
            .any(|pair| pair[0] == 0 || pair[0] >= pair[1])
        || sequences.first().copied().unwrap_or_default() == 0
    {
        return Err("K002 packaged probe Item order is invalid.".to_string());
    }
    let mut unique_ids = std::collections::BTreeSet::new();
    if item_ids.iter().any(|item_id| {
        !forgecad_app_server_protocol::valid_stable_id(item_id)
            || !unique_ids.insert(item_id.as_str())
    }) {
        return Err("K002 packaged probe Item identity is invalid.".to_string());
    }
    const ALLOWED_ITEM_TYPES: &[&str] = &[
        "user_message",
        "assistant_message",
        "plan",
        "tool_call",
        "tool_result",
        "preview",
        "approval_request",
        "clarification",
        "artifact",
    ];
    if item_types.first().map(String::as_str) != Some("user_message")
        || !item_types
            .iter()
            .any(|item_type| item_type == "tool_result")
        || item_types
            .iter()
            .any(|item_type| !ALLOWED_ITEM_TYPES.contains(&item_type.as_str()))
    {
        return Err("K002 packaged probe Item type order is invalid.".to_string());
    }

    let items_sha = report
        .items_sha256
        .as_deref()
        .ok_or_else(|| "K002 packaged probe items_sha256 is missing.".to_string())?;
    let replay_sha = report
        .replay_items_sha256
        .as_deref()
        .ok_or_else(|| "K002 packaged probe replay_items_sha256 is missing.".to_string())?;
    validate_k002_probe_sha(items_sha)?;
    validate_k002_probe_sha(replay_sha)?;
    if items_sha != replay_sha {
        return Err("K002 packaged probe Item replay hash diverged.".to_string());
    }
    Ok(())
}

fn k002_probe_stable_id_env(name: &str) -> Result<String, String> {
    let value = env::var(name).map_err(|_| format!("{name} is missing."))?;
    if !forgecad_app_server_protocol::valid_stable_id(&value) {
        return Err(format!("{name} is invalid."));
    }
    Ok(value)
}

fn k002_probe_sha_env(name: &str) -> Result<String, String> {
    let value = env::var(name).map_err(|_| format!("{name} is missing."))?;
    validate_k002_probe_sha(&value)?;
    Ok(value)
}

fn validate_k002_probe_sha(value: &str) -> Result<(), String> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
    {
        return Err("K002 packaged probe SHA-256 is invalid.".to_string());
    }
    if !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("K002 packaged probe SHA-256 is invalid.".to_string());
    }
    Ok(())
}

fn k002_probe_u64_env(name: &str, minimum: u64, maximum: u64) -> Result<u64, String> {
    let value = env::var(name).map_err(|_| format!("{name} is missing."))?;
    value
        .parse::<u64>()
        .ok()
        .filter(|number| (*number >= minimum) && (*number <= maximum))
        .ok_or_else(|| format!("{name} is invalid."))
}

#[tauri::command]
fn get_provider_config(
    state: State<'_, AgentProcessState>,
) -> Result<ProviderConfigMetadata, String> {
    if local_universal_author_enabled() {
        return Ok(local_universal_provider_config_with_runtime_status(
            &state.internal_capability_token,
        ));
    }
    if mvp_offline_arm_enabled() {
        return Ok(mvp_provider_config_with_runtime_status(
            &state.internal_capability_token,
        ));
    }
    Ok(provider_config_with_runtime_status(
        state.provider_credentials.inspect_metadata_only(),
        &state.internal_capability_token,
    ))
}

fn local_universal_provider_config_with_runtime_status(
    internal_capability_token: &str,
) -> ProviderConfigMetadata {
    let supervisor_status = match probe_agent(internal_capability_token) {
        AgentProbe::Healthy => "running",
        AgentProbe::WrongService(_) | AgentProbe::CapabilityMismatch(_) => "mismatch",
        AgentProbe::Offline => "unavailable",
    };
    ProviderConfigMetadata {
        base_url: "local://forgecad-universal-visual-author".into(),
        model: LOCAL_UNIVERSAL_MODEL.into(),
        configured: true,
        storage: "rust-offline-deterministic".into(),
        credential_id: None,
        metadata_status: "ready".into(),
        secret_status: "not_required".into(),
        supervisor_status: supervisor_status.into(),
        capability_status: if supervisor_status == "running" {
            "ready".into()
        } else {
            supervisor_status.into()
        },
        failure_code: None,
    }
}

#[tauri::command]
fn save_provider_config(
    mut request: SaveProviderConfigRequest,
    state: State<'_, AgentProcessState>,
) -> Result<ProviderConfigMetadata, String> {
    if local_universal_author_enabled() {
        return Err("本机通用视觉作者不读取或保存 Provider Key。".into());
    }
    if mvp_offline_arm_enabled() {
        return Err("本机机械臂 MVP 不读取或保存 Provider Key。".into());
    }
    let (base_url, model, api_key) =
        validate_provider_config_input(&request.base_url, &request.model, &request.api_key)?;
    request.api_key.zeroize();
    let metadata = state.provider_credentials.save(base_url, model, api_key)?;
    Ok(provider_config_with_runtime_status(
        metadata,
        &state.internal_capability_token,
    ))
}

#[tauri::command]
fn clear_provider_config(
    state: State<'_, AgentProcessState>,
) -> Result<ProviderConfigMetadata, String> {
    if local_universal_author_enabled() {
        return Err("本机通用视觉作者不读取或清除 Provider Key。".into());
    }
    if mvp_offline_arm_enabled() {
        return Err("本机机械臂 MVP 不读取或清除 Provider Key。".into());
    }
    let metadata = state.provider_credentials.clear()?;
    Ok(provider_config_with_runtime_status(
        metadata,
        &state.internal_capability_token,
    ))
}

#[tauri::command]
fn get_vision_evidence_provider_config(
    state: State<'_, VisualProviderState>,
) -> Result<VisionEvidenceConfigMetadata, String> {
    state
        .vision_credentials
        .inspect_metadata()
        .map_err(|error| format!("{}: {}", error.code, error.message))
}

#[tauri::command]
fn save_vision_evidence_provider_config(
    mut request: SaveVisionEvidenceProviderConfigRequest,
    state: State<'_, VisualProviderState>,
) -> Result<VisionEvidenceConfigMetadata, String> {
    let api_key = std::mem::take(&mut request.api_key);
    let base_url = std::mem::take(&mut request.base_url);
    let model = std::mem::take(&mut request.model);
    state
        .vision_credentials
        .save(base_url, model, api_key)
        .map_err(|error| format!("{}: {}", error.code, error.message))
}

#[tauri::command]
fn clear_vision_evidence_provider_config(
    state: State<'_, VisualProviderState>,
) -> Result<VisionEvidenceConfigMetadata, String> {
    state
        .vision_credentials
        .clear()
        .map_err(|error| format!("{}: {}", error.code, error.message))
}

/// Explicit read-only vision analysis. This command never creates a Version,
/// ChangeSet, Snapshot, preview or geometry artifact. It resolves every image
/// from sealed Rust CAS using evidence identifiers supplied by the already
/// validated multimodal request.
#[tauri::command]
async fn analyze_visual_evidence(
    input: AnalyzeVisualEvidenceRequest,
    state: State<'_, VisualProviderState>,
) -> Result<AnalyzeVisualEvidenceResult, String> {
    validate_visual_client_request_id(&input.client_request_id)
        .map_err(|_| "VISION_EVIDENCE_REQUEST_ID_INVALID".to_string())?;
    let cancellation =
        register_visual_evidence_request(&state.vision_active_requests, &input.client_request_id)?;
    let result = analyze_visual_evidence_inner(input.request, &state, cancellation).await;
    finish_visual_evidence_request(&state.vision_active_requests, &input.client_request_id);
    result
}

async fn analyze_visual_evidence_inner(
    mut request: MultimodalDesignRequest,
    state: &VisualProviderState,
    cancellation: CancellationToken,
) -> Result<AnalyzeVisualEvidenceResult, String> {
    let mut evidence = Vec::with_capacity(request.reference_inputs.len());
    let mut images = Vec::new();
    for reference in &request.reference_inputs {
        if cancellation.is_cancelled() {
            return Err("VISION_EVIDENCE_CANCELLED".into());
        }
        let (sealed, bytes) = state
            .repository
            .read_reference_evidence_content(&request.project_id, &reference.evidence_id)
            .map_err(|error| format!("{}: {}", error.code(), error))?;
        if sealed.kind == ReferenceEvidenceKind::Image {
            images.push(VisionEvidenceImage {
                evidence_id: sealed.evidence_id.clone(),
                media_type: sealed.source_media_type.clone(),
                bytes: Arc::from(bytes),
            });
        }
        evidence.push(sealed);
    }
    for reference in &mut request.reference_inputs {
        let sealed = evidence
            .iter()
            .find(|item| item.evidence_id == reference.evidence_id)
            .ok_or_else(|| "VISION_EVIDENCE_REFERENCE_NOT_FOUND".to_string())?;
        reference.evidence_sha256 =
            semantic_sha256(sealed).map_err(|error| format!("{}: {}", error.code(), error))?;
    }
    let normalized_request = request.clone();
    let visual_evidence_graph = state
        .vision_coordinator
        .analyze(
            VisionEvidenceProviderRequest {
                request,
                evidence,
                images,
            },
            cancellation,
        )
        .await
        .map_err(|error| format!("{}: {}", error.code, error.message))?;
    Ok(AnalyzeVisualEvidenceResult {
        request: normalized_request,
        visual_evidence_graph,
    })
}

/// Explicit one-click authorization for the subsequent reference comparison
/// loop. Rust re-reads and validates all sealed evidence, fixes the reviewed
/// policy and hard caps, and persists a short-lived grant. This command does
/// not call a Provider or create any design/version artifact.
#[tauri::command]
fn authorize_visual_reference_comparison(
    input: AuthorizeVisualReferenceComparisonRequest,
    state: State<'_, VisualProviderState>,
) -> Result<AuthorizeVisualReferenceComparisonResult, String> {
    validate_visual_client_request_id(&input.client_request_id)
        .map_err(|_| "VISUAL_REFERENCE_AUTHORIZATION_REQUEST_ID_INVALID".to_string())?;
    if input.maximum_calls != VISUAL_REFERENCE_COMPARISON_MAXIMUM_CALLS
        || input.maximum_variable_cost_microusd
            != VISUAL_REFERENCE_COMPARISON_MAXIMUM_VARIABLE_COST_MICROUSD
    {
        return Err("VISUAL_REFERENCE_AUTHORIZATION_CAP_MISMATCH".into());
    }
    let mut evidence = Vec::with_capacity(input.request.reference_inputs.len());
    for reference in &input.request.reference_inputs {
        let (sealed, _) = state
            .repository
            .read_reference_evidence_content(&input.request.project_id, &reference.evidence_id)
            .map_err(|error| format!("{}: {}", error.code(), error))?;
        evidence.push(sealed);
    }
    if !evidence
        .iter()
        .any(|item| item.kind == ReferenceEvidenceKind::Image)
    {
        return Err("VISUAL_REFERENCE_AUTHORIZATION_IMAGE_REQUIRED".into());
    }
    input
        .request
        .validate_with_evidence(&evidence)
        .map_err(|error| format!("{}: {}", error.code(), error))?;
    input
        .visual_evidence_graph
        .validate_against(&input.request, &evidence)
        .map_err(|error| format!("{}: {}", error.code(), error))?;
    let policy = if input.request.domain_pack_id == "pack_robotic_arm_concept" {
        c111b_visual_reference_acceptance_policy_for_domain(&input.request.domain_pack_id)
            .map_err(|error| format!("{}: {}", error.code(), error))?
    } else {
        forgecad_core::VisualReferenceAcceptancePolicy::default_policy()
    };
    let request_sha256 =
        semantic_sha256(&input.request).map_err(|error| format!("{}: {}", error.code(), error))?;
    let graph_sha256 = semantic_sha256(&input.visual_evidence_graph)
        .map_err(|error| format!("{}: {}", error.code(), error))?;
    let policy_sha256 =
        semantic_sha256(&policy).map_err(|error| format!("{}: {}", error.code(), error))?;
    let now_unix_ms = current_unix_ms();
    let authorization_id = generate_visual_reference_authorization_id()?;
    let authorization = state
        .repository
        .issue_visual_reference_comparison_authorization(
            &authorization_id,
            &input.client_request_id,
            &input.request.project_id,
            &request_sha256,
            &graph_sha256,
            &policy_sha256,
            now_unix_ms,
            now_unix_ms + VISUAL_REFERENCE_COMPARISON_AUTHORIZATION_LIFETIME_MS,
        )
        .map_err(|error| format!("{}: {}", error.code(), error))?;
    Ok(AuthorizeVisualReferenceComparisonResult {
        authorization_id: authorization.authorization_id,
        authorization_binding_sha256: authorization.authorization_binding_sha256,
        expires_at_unix_ms: authorization.expires_at_unix_ms,
        maximum_calls: authorization.maximum_calls,
        maximum_variable_cost_microusd: authorization.maximum_variable_cost_microusd,
    })
}

/// Creates and binds a short-lived Qwen comparison grant for the exact
/// universal candidate that is paused after same-renderer PBR capture. This
/// command deliberately performs no Provider call, no geometry work and no
/// preview/version write; the subsequent Rust-owned continuation still has
/// to reserve the grant against the recomputed comparison input.
#[tauri::command]
fn authorize_candidate_pbr_visual_comparison(
    input: AuthorizeCandidatePbrVisualComparisonRequest,
    bridge: State<'_, AppServerBridge>,
    state: State<'_, VisualProviderState>,
) -> Result<AuthorizeVisualReferenceComparisonResult, String> {
    validate_visual_client_request_id(&input.client_request_id)
        .map_err(|_| "UNIVERSAL_VISUAL_AUTHORIZATION_REQUEST_ID_INVALID".to_string())?;
    let scope = bridge
        .pending_universal_visual_comparison_authorization(
            &input.execution_id,
            &input.project_id,
            &input.turn_id,
        )?
        .ok_or_else(|| {
            "UNIVERSAL_VISUAL_AUTHORIZATION_NOT_REQUIRED_OR_ALREADY_BOUND".to_string()
        })?;
    if scope.maximum_calls != VISUAL_REFERENCE_COMPARISON_MAXIMUM_CALLS
        || scope.maximum_variable_cost_microusd
            != VISUAL_REFERENCE_COMPARISON_MAXIMUM_VARIABLE_COST_MICROUSD
    {
        return Err("UNIVERSAL_VISUAL_AUTHORIZATION_CAP_MISMATCH".into());
    }
    let now_unix_ms = current_unix_ms();
    let authorization_id = generate_visual_reference_authorization_id()?;
    let authorization = state
        .repository
        .issue_visual_reference_comparison_authorization(
            &authorization_id,
            &input.client_request_id,
            &scope.project_id,
            &scope.request_sha256,
            &scope.evidence_graph_sha256,
            &scope.acceptance_policy_sha256,
            now_unix_ms,
            now_unix_ms + VISUAL_REFERENCE_COMPARISON_AUTHORIZATION_LIFETIME_MS,
        )
        .map_err(|error| format!("{}: {}", error.code(), error))?;
    let bound = bridge.bind_universal_visual_comparison_authorization(
        &input.execution_id,
        &input.project_id,
        &input.turn_id,
        &authorization.authorization_id,
        &authorization.authorization_binding_sha256,
    )?;
    if bound != scope {
        return Err("UNIVERSAL_VISUAL_AUTHORIZATION_SCOPE_DRIFT".into());
    }
    Ok(AuthorizeVisualReferenceComparisonResult {
        authorization_id: authorization.authorization_id,
        authorization_binding_sha256: authorization.authorization_binding_sha256,
        expires_at_unix_ms: authorization.expires_at_unix_ms,
        maximum_calls: authorization.maximum_calls,
        maximum_variable_cost_microusd: authorization.maximum_variable_cost_microusd,
    })
}

#[tauri::command]
fn cancel_visual_evidence_analysis(
    client_request_id: String,
    state: State<'_, VisualProviderState>,
) -> Result<bool, String> {
    validate_visual_client_request_id(&client_request_id)
        .map_err(|_| "VISION_EVIDENCE_REQUEST_ID_INVALID".to_string())?;
    cancel_visual_evidence_request(&state.vision_active_requests, &client_request_id)
}

fn validate_visual_client_request_id(value: &str) -> Result<(), ()> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b'@'))
    {
        return Err(());
    }
    Ok(())
}

fn register_visual_evidence_request(
    active_requests: &Mutex<HashMap<String, CancellationToken>>,
    client_request_id: &str,
) -> Result<CancellationToken, String> {
    let cancellation = CancellationToken::new();
    let mut active = active_requests
        .lock()
        .map_err(|_| "VISION_EVIDENCE_STATE_UNAVAILABLE".to_string())?;
    if active.contains_key(client_request_id) {
        return Err("VISION_EVIDENCE_REQUEST_ALREADY_ACTIVE".into());
    }
    active.insert(client_request_id.to_string(), cancellation.clone());
    Ok(cancellation)
}

fn cancel_visual_evidence_request(
    active_requests: &Mutex<HashMap<String, CancellationToken>>,
    client_request_id: &str,
) -> Result<bool, String> {
    let active = active_requests
        .lock()
        .map_err(|_| "VISION_EVIDENCE_STATE_UNAVAILABLE".to_string())?;
    if let Some(cancellation) = active.get(client_request_id) {
        cancellation.cancel();
        return Ok(true);
    }
    Ok(false)
}

fn finish_visual_evidence_request(
    active_requests: &Mutex<HashMap<String, CancellationToken>>,
    client_request_id: &str,
) {
    if let Ok(mut active) = active_requests.lock() {
        active.remove(client_request_id);
    }
}

impl AgentProcessState {
    fn shutdown_managed(&self) {
        if let Ok(mut child_guard) = self.child.lock() {
            if let Some(mut child) = child_guard.take() {
                // PyInstaller onefile uses a parent process which unpacks and
                // then starts the actual Agent child. The desktop owns a
                // dedicated process group so normal window close stops both,
                // rather than leaving a hidden listener on port 8000.
                terminate_process_group(child.id());
                let _ = child.kill();
                let _ = child.wait();
            }
        }
        clear_managed_sidecar_lease(&self.supervisor_session_id);
    }

    fn record_managed_sidecar(&self, process_group_id: u32) -> Result<(), String> {
        write_managed_sidecar_lease(&ManagedSidecarLease {
            schema_version: MANAGED_SIDECAR_LEASE_SCHEMA.to_string(),
            supervisor_session_id: self.supervisor_session_id.clone(),
            desktop_pid: std::process::id(),
            sidecar_process_group_id: process_group_id,
        })
    }
}

impl Drop for AgentProcessState {
    fn drop(&mut self) {
        self.shutdown_managed();
    }
}

#[derive(Serialize)]
struct AgentServiceStatus {
    base_url: String,
    health_url: String,
    endpoint: String,
    running: bool,
    managed_by_desktop: bool,
    pid: Option<u32>,
    mode: String,
    state: &'static str,
    last_error: Option<String>,
}

#[derive(Clone, Copy)]
enum AgentMode {
    LocalDev,
    PackagedSidecar,
}

enum AgentProbe {
    Healthy,
    Offline,
    WrongService(String),
    CapabilityMismatch(String),
}

#[tauri::command]
fn agent_health_endpoint() -> String {
    agent_health_url(&agent_base_url())
}

#[tauri::command]
fn agent_service_status(state: State<'_, AgentProcessState>) -> AgentServiceStatus {
    let mode_name = managed_mode_name(&state);
    let (managed_running, pid) = match state.child.lock() {
        Ok(mut guard) => {
            let pid = guard.as_mut().and_then(|child| {
                if child.try_wait().ok().flatten().is_none() {
                    Some(child.id())
                } else {
                    None
                }
            });
            let managed_running = pid.is_some();
            if !managed_running {
                *guard = None;
            }
            (managed_running, pid)
        }
        Err(_) => (false, None),
    };

    status_from_probe(
        probe_agent(&state.internal_capability_token),
        managed_running,
        pid,
        &mode_name,
    )
}

#[tauri::command]
fn start_agent_service(state: State<'_, AgentProcessState>) -> Result<AgentServiceStatus, String> {
    if let Ok(mut child_guard) = state.child.lock() {
        let managed_running = child_guard
            .as_mut()
            .map(|child| child.try_wait().ok().flatten().is_none())
            .unwrap_or(false);
        if managed_running {
            let pid = child_guard.as_ref().map(|child| child.id());
            if let AgentProbe::Healthy = probe_agent(&state.internal_capability_token) {
                let mode_name = managed_mode_name(&state);
                return Ok(status_from_probe(
                    AgentProbe::Healthy,
                    true,
                    pid,
                    &mode_name,
                ));
            }
        }
    }

    match recover_orphaned_managed_sidecar()? {
        true => append_supervisor_log(
            "ForgeCAD supervisor reclaimed a stale managed sidecar before startup",
        ),
        false => {}
    }

    match probe_agent(&state.internal_capability_token) {
        AgentProbe::Healthy => {
            let mode_name = runtime_mode_name(runtime_mode());
            return Ok(status_from_probe(
                AgentProbe::Healthy,
                false,
                None,
                &mode_name,
            ));
        }
        AgentProbe::WrongService(reason) => {
            return Err(format!(
                "Port 8000 is occupied by a non-ForgeCAD service: {reason}"
            ));
        }
        AgentProbe::CapabilityMismatch(reason) => {
            return Err(format!(
                "Port 8000 is occupied by an active ForgeCAD sidecar from another desktop session; it was not stopped: {reason}"
            ));
        }
        AgentProbe::Offline => {}
    }

    state.shutdown_managed();

    let mut child_guard = state
        .child
        .lock()
        .map_err(|_| "agent process mutex poisoned".to_string())?;
    let mut mode_guard = state
        .mode
        .lock()
        .map_err(|_| "agent mode mutex poisoned".to_string())?;
    let mode = runtime_mode();
    let mode_name = match mode {
        AgentMode::LocalDev => AGENT_MODE_LOCAL,
        AgentMode::PackagedSidecar => AGENT_MODE_PACKAGED,
    }
    .to_string();
    *mode_guard = mode_name.clone();

    *child_guard = None;
    let child = match mode {
        AgentMode::PackagedSidecar => start_packaged_sidecar(
            &state.internal_capability_token,
            &state.supervisor_session_id,
        )?,
        AgentMode::LocalDev => {
            let repo_root = repo_root()?;
            start_local_python_sidecar(
                &repo_root,
                &state.internal_capability_token,
                &state.supervisor_session_id,
            )?
        }
    };
    let pid = child.id();
    if let Err(error) = state.record_managed_sidecar(pid) {
        terminate_process_group(pid);
        let mut child = child;
        let _ = child.kill();
        let _ = child.wait();
        return Err(error);
    }
    *child_guard = Some(child);
    drop(child_guard);

    // A frozen sidecar may need to unpack its onefile payload before it can
    // apply SQLite migrations.  On a cold macOS launch the measured arm64
    // path can exceed 30 seconds, so keep the supervisor's bounded window
    // above the packaged smoke budget while still failing deterministically.
    for _ in 0..900 {
        match probe_agent(&state.internal_capability_token) {
            AgentProbe::Healthy => {
                append_supervisor_log(&format!(
                    "ForgeCAD supervisor healthy mode={mode_name} pid={pid}"
                ));
                return Ok(status_from_probe(
                    AgentProbe::Healthy,
                    true,
                    Some(pid),
                    &mode_name,
                ));
            }
            AgentProbe::WrongService(reason) => {
                state.shutdown_managed();
                return Err(format!(
                    "Agent service started but health probe returned a non-Wushen service: {reason}"
                ));
            }
            AgentProbe::CapabilityMismatch(reason) => {
                // The recorded child is ours and may be stopped, but the
                // listener that answered the ownership check is deliberately
                // left untouched: it may belong to another desktop process.
                state.shutdown_managed();
                return Err(format!(
                    "Agent service ownership handshake did not match this desktop session: {reason}"
                ));
            }
            AgentProbe::Offline => thread::sleep(Duration::from_millis(100)),
        }
    }

    state.shutdown_managed();
    Err(
        "Agent service did not become healthy on http://127.0.0.1:8000/api/health within 90s"
            .to_string(),
    )
}

fn runtime_mode_name(mode: AgentMode) -> String {
    match mode {
        AgentMode::LocalDev => AGENT_MODE_LOCAL.to_string(),
        AgentMode::PackagedSidecar => AGENT_MODE_PACKAGED.to_string(),
    }
}

#[tauri::command]
fn stop_agent_service(state: State<'_, AgentProcessState>) -> AgentServiceStatus {
    state.shutdown_managed();
    let mode = managed_mode_name(&state);
    status_from_probe(
        probe_agent(&state.internal_capability_token),
        false,
        None,
        &mode,
    )
}

fn provider_config_with_runtime_status(
    mut metadata: ProviderConfigMetadata,
    internal_capability_token: &str,
) -> ProviderConfigMetadata {
    let probe = probe_agent(internal_capability_token);
    metadata.supervisor_status = match &probe {
        AgentProbe::Healthy => "running",
        AgentProbe::WrongService(_) | AgentProbe::CapabilityMismatch(_) => "mismatch",
        AgentProbe::Offline => "unavailable",
    }
    .to_string();
    metadata.capability_status = match probe {
        AgentProbe::Healthy if metadata.configured => "ready",
        AgentProbe::Healthy => "offline",
        AgentProbe::CapabilityMismatch(_) => "mismatch",
        AgentProbe::WrongService(_) | AgentProbe::Offline if metadata.configured => "unavailable",
        AgentProbe::WrongService(_) | AgentProbe::Offline => "offline",
    }
    .to_string();
    metadata
}

fn main() {
    let internal_capability_token = generate_internal_capability_token()
        .expect("ForgeCAD must create an ephemeral Rust-to-Python capability token");
    let supervisor_session_id = generate_supervisor_session_id()
        .expect("ForgeCAD must create a non-secret managed-sidecar session marker");
    let provider_credentials = ProviderCredentialStore::production();
    let native_provider_bundle = build_native_provider_client(provider_credentials.clone())
        .expect("ForgeCAD must initialize its Rust-owned DeepSeek Provider client");
    let native_provider = native_provider_bundle.client.clone();
    let c111b_packaged_qa_metrics = C111bPackagedQaMetricsState {
        local_mvp_provider: native_provider_bundle.local_mvp_provider,
        timeline: Arc::new(Mutex::new(C111bPackagedQaTimeline {
            started: Instant::now(),
            stages: Vec::new(),
        })),
    };
    let library_root = rust_core_library_root()
        .expect("ForgeCAD must resolve the local Rust product-state library");
    let rust_core = Arc::new(
        RustCoreRuntime::open(
            &library_root,
            generate_rust_core_instance_id()
                .expect("ForgeCAD must create a bounded Rust core writer identity"),
        )
        .expect("ForgeCAD must open the Rust-owned product-state core"),
    );
    if let Err(error) = rust_core.recover_orphaned_turns(&rust_core_timestamp()) {
        let _ = rust_core.rollback_cutover_before_publish();
        panic!("ForgeCAD Rust core lifecycle recovery failed: {error}");
    }
    let app_server_bridge = match AppServerBridge::new_production(
        &agent_base_url(),
        internal_capability_token.clone(),
        native_provider.clone(),
        Arc::clone(&rust_core),
    ) {
        Ok(bridge) => bridge,
        Err(error) => {
            let _ = rust_core.rollback_cutover_before_publish();
            panic!("ForgeCAD app-server bridge initialization failed before cutover: {error}");
        }
    };
    let vision_credentials = Arc::new(PrivateFileVisionEvidenceCredentialStore::new(
        library_root
            .join("secrets")
            .join("vision-evidence-provider"),
    ));
    let vision_transport = ReqwestVisionEvidenceTransport::new().unwrap_or_else(|error| {
        let _ = rust_core.rollback_cutover_before_publish();
        panic!(
            "Forge Studio vision-evidence HTTPS transport failed before cutover: {}",
            error.message
        );
    });
    let vision_adapter = Arc::new(
        OpenAiCompatibleVisionEvidenceAdapter::new(
            vision_credentials.clone(),
            Arc::new(vision_transport),
            Duration::from_secs(115),
        )
        .unwrap_or_else(|error| {
            let _ = rust_core.rollback_cutover_before_publish();
            panic!(
                "Forge Studio vision-evidence adapter failed before cutover: {}",
                error.message
            );
        }),
    );
    let budgeted_visual_comparison = Arc::new(BudgetedVisualReferenceComparisonProvider::new(
        vision_adapter.clone(),
        Arc::new(rust_core.repository().clone()),
    ));
    app_server_bridge
        .attach_visual_reference_comparison_provider(budgeted_visual_comparison)
        .unwrap_or_else(|error| {
            let _ = rust_core.rollback_cutover_before_publish();
            panic!("Forge Studio visual reference comparator failed before cutover: {error}");
        });
    let vision_coordinator =
        VisionEvidenceCoordinator::new(vision_adapter, Duration::from_secs(100)).unwrap_or_else(
            |error| {
                let _ = rust_core.rollback_cutover_before_publish();
                panic!(
                    "Forge Studio vision-evidence coordinator failed before cutover: {}",
                    error.message
                );
            },
        );
    let visual_provider_state = VisualProviderState {
        repository: Arc::new(rust_core.repository().clone()),
        vision_credentials,
        vision_coordinator,
        vision_active_requests: Mutex::new(HashMap::new()),
    };
    if let Err(error) = rust_core.publish() {
        let _ = rust_core.rollback_cutover_before_publish();
        panic!("ForgeCAD Rust core cutover could not be published: {error}");
    }
    append_supervisor_log(
        "ForgeCAD runtime cutover published state_owner=rust-core python_role=restricted_geometry_executor",
    );
    let resource_bridge = app_server_bridge.clone();
    let packaged_probe_core = Arc::clone(&rust_core);
    let app = tauri::Builder::default()
        .manage(app_server_bridge)
        .manage(visual_provider_state)
        .manage(c111b_packaged_qa_metrics)
        .manage(AgentProcessState {
            child: Mutex::new(None),
            mode: Mutex::new(AGENT_MODE_LOCAL.to_string()),
            internal_capability_token,
            supervisor_session_id,
            provider_credentials,
        })
        .register_asynchronous_uri_scheme_protocol(
            "forgecad-resource",
            move |_context, request, responder| {
                let bridge = resource_bridge.clone();
                tauri::async_runtime::spawn(async move {
                    responder.respond(bridge.resource_response(request).await);
                });
            },
        )
        .setup(move |app| {
            // A packaged release has no repository or Python dependency. Start
            // its bundled sidecar before the WebView is ready. A background
            // `State` access is not reliable across every macOS launch path;
            // this synchronous, idempotent startup makes cold first launch
            // deterministic. The WebView call remains a recovery/status check.
            let state = app.state::<AgentProcessState>();
            match start_agent_service(state) {
                Ok(_) => {
                    let bridge = app.state::<AppServerBridge>().inner().clone();
                    k003_packaged_probe::run_if_enabled(bridge, Arc::clone(&packaged_probe_core));
                    mvp_arm_packaged_probe::run_if_enabled(
                        app.state::<AppServerBridge>().inner().clone(),
                    );
                    c110g_packaged_probe::run_if_enabled(
                        app.state::<AppServerBridge>().inner().clone(),
                    );
                    deepseek_mvp_acceptance_probe::run_if_enabled(
                        app.state::<AppServerBridge>().inner().clone(),
                    );
                    deepseek_forge_visual_acceptance_probe::run_if_enabled(
                        app.state::<AppServerBridge>().inner().clone(),
                    );
                    deepseek_delta_acceptance_probe::run_if_enabled(
                        app.state::<AppServerBridge>().inner().clone(),
                    );
                }
                Err(error) => {
                    eprintln!("ForgeCAD Agent startup failed: {error}");
                    append_supervisor_log(&format!("ForgeCAD supervisor startup failed: {error}"));
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            agent_health_endpoint,
            agent_service_status,
            start_agent_service,
            stop_agent_service,
            get_provider_config,
            save_provider_config,
            clear_provider_config,
            get_vision_evidence_provider_config,
            save_vision_evidence_provider_config,
            clear_vision_evidence_provider_config,
            analyze_visual_evidence,
            cancel_visual_evidence_analysis,
            authorize_visual_reference_comparison,
            authorize_candidate_pbr_visual_comparison,
            forgecad_k001_packaged_probe_config,
            forgecad_k001_packaged_probe_report,
            forgecad_k002_packaged_probe_config,
            forgecad_k002_packaged_probe_report,
            forgecad_arm_webview_qa_config,
            forgecad_arm_webview_qa_capture,
            forgecad_arm_webview_qa_r007b_lineage,
            forgecad_arm_webview_qa_report,
            forgecad_arm_webview_qa_progress,
            forgecad_c111b_webview_qa_config,
            forgecad_c111b_webview_qa_source,
            forgecad_c111b_webview_qa_capture,
            forgecad_c111b_webview_qa_readback,
            forgecad_c111b_webview_qa_report,
            forgecad_c111b_webview_qa_progress,
            forgecad_candidate_pbr_capture_issue,
            forgecad_candidate_pbr_capture_submit,
            forgecad_candidate_pbr_capture_resume,
            forgecad_protocol_connect,
            forgecad_protocol_send,
            forgecad_protocol_disconnect
        ])
        .on_window_event(|window, event| {
            if matches!(event, tauri::WindowEvent::CloseRequested { .. }) {
                if let Some(state) = window.try_state::<AppServerBridge>() {
                    state.shutdown();
                }
                if let Some(state) = window.try_state::<AgentProcessState>() {
                    state.shutdown_managed();
                }
            }
        })
        .build(tauri::generate_context!())
        .expect("error while building Wushen Forge desktop");
    app.run(|app, event| {
        if matches!(
            event,
            tauri::RunEvent::ExitRequested { .. } | tauri::RunEvent::Exit
        ) {
            if let Some(state) = app.try_state::<AppServerBridge>() {
                state.shutdown();
            }
            if let Some(state) = app.try_state::<AgentProcessState>() {
                state.shutdown_managed();
            }
        }
    });
}

fn status_from_probe(
    probe: AgentProbe,
    managed_running: bool,
    pid: Option<u32>,
    mode_name: &str,
) -> AgentServiceStatus {
    let base_url = agent_base_url();
    let health_url = agent_health_url(&base_url);
    let (running, state, last_error) = match probe {
        AgentProbe::Healthy => (true, "running", None),
        AgentProbe::Offline if managed_running => (false, "starting", None),
        AgentProbe::Offline => (false, "stopped", None),
        AgentProbe::WrongService(reason) => (false, "wrong_service", Some(reason)),
        AgentProbe::CapabilityMismatch(reason) => (false, "capability_mismatch", Some(reason)),
    };

    AgentServiceStatus {
        endpoint: health_url.clone(),
        base_url,
        health_url,
        running,
        managed_by_desktop: managed_running,
        pid,
        mode: mode_name.to_string(),
        state,
        last_error,
    }
}

fn start_local_python_sidecar(
    repo_root: &Path,
    internal_capability_token: &str,
    supervisor_session_id: &str,
) -> Result<Child, String> {
    let log_path = repo_root.join(".wushen-agent.log");
    if let Some(parent) = log_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create Agent log directory: {error}"))?;
    }
    let mut command = Command::new(&agent_python(repo_root));
    apply_sidecar_environment(&mut command, env::vars_os());
    #[cfg(unix)]
    command.process_group(0);
    let log_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)
        .map_err(|error| format!("failed to open Agent log file: {error}"))?;
    let stderr_log = log_file
        .try_clone()
        .map_err(|error| format!("failed to clone Agent log file: {error}"))?;

    command
        .arg("-m")
        .arg("uvicorn")
        .arg("wushen_agent.main:create_app")
        .arg("--factory")
        .arg("--host")
        .arg(AGENT_HOST)
        .arg("--port")
        .arg(AGENT_PORT.to_string())
        .current_dir(repo_root)
        .env("PYTHONPATH", repo_root.join("apps/agent"))
        .env("PYTHONUNBUFFERED", "1")
        .stdout(Stdio::from(log_file))
        .stderr(Stdio::from(stderr_log));
    configure_python_facet_environment(
        &mut command,
        internal_capability_token,
        supervisor_session_id,
    );

    command.spawn().map_err(|error| {
        format!(
            "failed to start local-agent service with {}: {error}",
            agent_python(repo_root).display()
        )
    })
}

fn start_packaged_sidecar(
    internal_capability_token: &str,
    supervisor_session_id: &str,
) -> Result<Child, String> {
    let sidecar = sidecar_binary_path()?;
    let log_path = sidecar_log_path();
    if let Some(parent) = log_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create sidecar log directory: {error}"))?;
    }
    let log_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)
        .map_err(|error| format!("failed to open sidecar log file: {error}"))?;
    let stderr_log = log_file
        .try_clone()
        .map_err(|error| format!("failed to clone sidecar log file: {error}"))?;

    let mut command = Command::new(&sidecar);
    apply_sidecar_environment(&mut command, env::vars_os());
    #[cfg(unix)]
    command.process_group(0);
    command
        .arg("agent")
        .arg("serve")
        .current_dir(sidecar.parent().unwrap_or_else(|| Path::new(".")))
        .stdout(Stdio::from(log_file))
        .stderr(Stdio::from(stderr_log));
    configure_python_facet_environment(
        &mut command,
        internal_capability_token,
        supervisor_session_id,
    );
    command.spawn().map_err(|error| {
        format!(
            "failed to start packaged-sidecar with {}: {error}",
            sidecar.display()
        )
    })
}

const SIDECAR_SAFE_INHERITED_ENVIRONMENT_KEYS: &[&str] = &[
    // Minimal cross-platform process, home, temporary-directory and locale
    // context. Dynamic-loader and shell-initialization variables are
    // intentionally excluded.
    "HOME",
    "USERPROFILE",
    "LOCALAPPDATA",
    "APPDATA",
    "TMPDIR",
    "TMP",
    "TEMP",
    "SystemRoot",
    "WINDIR",
    "PATH",
    "LANG",
    "LANGUAGE",
    "LC_ALL",
    "LC_CTYPE",
    "LC_MESSAGES",
    "__CF_USER_TEXT_ENCODING",
    // Code-owned ForgeCAD runtime switches used by packaged verification,
    // bounded worker control and deterministic recovery. Provider metadata,
    // endpoints and credentials never belong in this list.
    "WUSHEN_AGENT_RUNTIME_MODE",
    "FORGECAD_DISABLE_PROVIDER_CONFIG",
    "FORGECAD_CONCEPT_WORKER_ENABLED",
    "WUSHEN_LOCAL_WORKER_ENABLED",
    "WUSHEN_RECOVER_ON_STARTUP",
    "FORGECAD_CONCEPT_RECOVER_ON_STARTUP",
];

const PROVIDER_ENVIRONMENT_KEYS: &[&str] = &[
    "FORGECAD_AGENT_PROVIDER",
    "FORGECAD_AGENT_BASE_URL",
    "FORGECAD_AGENT_MODEL",
    "FORGECAD_AGENT_API_KEY",
    "FORGECAD_AGENT_API_KEY_FILE",
    "FORGECAD_CONCEPT_PLANNER_PROVIDER",
    "FORGECAD_CONCEPT_PLANNER_BASE_URL",
    "FORGECAD_CONCEPT_PLANNER_MODEL",
    "FORGECAD_CONCEPT_PLANNER_API_KEY",
    "FORGECAD_CONCEPT_PLANNER_API_KEY_FILE",
    "WUSHEN_LLM_PROVIDER",
    "WUSHEN_LLM_BASE_URL",
    "WUSHEN_LLM_MODEL",
    "WUSHEN_LLM_API_KEY",
    "WUSHEN_LLM_API_KEY_FILE",
    "OPENAI_API_KEY",
    "DEEPSEEK_API_KEY",
    "ANTHROPIC_API_KEY",
];

fn apply_sidecar_environment<I, K, V>(command: &mut Command, environment: I)
where
    I: IntoIterator<Item = (K, V)>,
    K: AsRef<OsStr>,
    V: AsRef<OsStr>,
{
    // The Python compatibility process is a product-tool capability boundary,
    // not a child shell. Start from an empty environment and restore only the
    // small code-owned allowlist. Library roots, PYTHONPATH and the unguessable
    // internal capability are injected explicitly by each launcher afterward.
    command.env_clear();
    for (name, value) in environment {
        if is_safe_sidecar_environment_key(name.as_ref()) {
            command.env(name, value);
        }
    }
    strip_provider_environment(command);
}

fn is_safe_sidecar_environment_key(name: &OsStr) -> bool {
    let Some(name) = name.to_str() else {
        return false;
    };
    SIDECAR_SAFE_INHERITED_ENVIRONMENT_KEYS
        .iter()
        .any(|allowed| {
            if cfg!(windows) {
                allowed.eq_ignore_ascii_case(name)
            } else {
                *allowed == name
            }
        })
}

fn strip_provider_environment(command: &mut Command) {
    // K002 keeps Provider metadata and secrets in the Rust desktop process.
    // Explicit removals also defeat inherited shell variables, so the Python
    // persistence/product-tool process cannot accidentally regain a Provider.
    for name in PROVIDER_ENVIRONMENT_KEYS {
        command.env_remove(name);
    }
}

fn configure_python_facet_environment(
    command: &mut Command,
    internal_capability_token: &str,
    supervisor_session_id: &str,
) {
    // K003 gives Python only one ephemeral compiler capability. Database,
    // object-store and Provider locations are deliberately absent. No
    // environment switch can select the retired Python product writer.
    command.env(
        "FORGECAD_RESTRICTED_GEOMETRY_CAPABILITY_TOKEN",
        internal_capability_token,
    );
    command.env(SIDECAR_SUPERVISOR_SESSION_ENV, supervisor_session_id);
}

fn generate_internal_capability_token() -> Result<String, String> {
    let mut bytes = [0_u8; 32];
    getrandom::fill(&mut bytes)
        .map_err(|_| "secure random capability generation failed".to_string())?;
    let mut token = String::with_capacity(bytes.len() * 2);
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for byte in bytes {
        token.push(HEX[(byte >> 4) as usize] as char);
        token.push(HEX[(byte & 0x0f) as usize] as char);
    }
    Ok(token)
}

fn generate_visual_reference_authorization_id() -> Result<String, String> {
    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes)
        .map_err(|_| "secure visual comparison authorization generation failed".to_string())?;
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut suffix = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        suffix.push(HEX[(byte >> 4) as usize] as char);
        suffix.push(HEX[(byte & 0x0f) as usize] as char);
    }
    Ok(format!("visauth_{suffix}"))
}

fn current_unix_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|duration| i64::try_from(duration.as_millis()).ok())
        .unwrap_or(0)
}

fn generate_supervisor_session_id() -> Result<String, String> {
    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes)
        .map_err(|_| "secure supervisor session generation failed".to_string())?;
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut session_id = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        session_id.push(HEX[(byte >> 4) as usize] as char);
        session_id.push(HEX[(byte & 0x0f) as usize] as char);
    }
    Ok(session_id)
}

fn generate_rust_core_instance_id() -> Result<String, String> {
    let mut bytes = [0_u8; 12];
    getrandom::fill(&mut bytes)
        .map_err(|_| "secure Rust core writer identity generation failed".to_string())?;
    let mut suffix = String::with_capacity(bytes.len() * 2);
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for byte in bytes {
        suffix.push(HEX[(byte >> 4) as usize] as char);
        suffix.push(HEX[(byte & 0x0f) as usize] as char);
    }
    Ok(format!("forgecad-desktop-{}-{suffix}", std::process::id()))
}

fn rust_core_library_root() -> Result<PathBuf, String> {
    match runtime_mode() {
        AgentMode::PackagedSidecar => Ok(packaged_library_root()),
        AgentMode::LocalDev => repo_root().map(|root| local_library_root(&root)),
    }
}

fn rust_core_timestamp() -> String {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    format!("unix_ms_{}", elapsed.as_millis())
}

fn sidecar_log_path() -> PathBuf {
    let base = env::var_os("LOCALAPPDATA")
        .or_else(|| env::var_os("APPDATA"))
        .or_else(|| env::var_os("HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("WushenForge").join("agent.log")
}

fn managed_sidecar_lease_path() -> PathBuf {
    sidecar_log_path().with_file_name("managed-sidecar-lease.json")
}

fn valid_supervisor_session_id(value: &str) -> bool {
    value.len() == 32
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn write_managed_sidecar_lease(lease: &ManagedSidecarLease) -> Result<(), String> {
    if lease.schema_version != MANAGED_SIDECAR_LEASE_SCHEMA
        || !valid_supervisor_session_id(&lease.supervisor_session_id)
        || lease.desktop_pid == 0
        || lease.sidecar_process_group_id == 0
    {
        return Err("ForgeCAD refused to write an invalid managed-sidecar lease".to_string());
    }
    let path = managed_sidecar_lease_path();
    let parent = path
        .parent()
        .ok_or_else(|| "managed-sidecar lease has no parent directory".to_string())?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("failed to create managed-sidecar lease directory: {error}"))?;
    let temporary = path.with_extension(format!("tmp-{}", std::process::id()));
    let encoded = serde_json::to_vec(lease)
        .map_err(|_| "failed to serialize managed-sidecar lease".to_string())?;
    {
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&temporary)
            .map_err(|error| format!("failed to open managed-sidecar lease: {error}"))?;
        file.write_all(&encoded)
            .map_err(|error| format!("failed to write managed-sidecar lease: {error}"))?;
        file.sync_all()
            .map_err(|error| format!("failed to flush managed-sidecar lease: {error}"))?;
    }
    #[cfg(unix)]
    fs::set_permissions(&temporary, fs::Permissions::from_mode(0o600)).map_err(|error| {
        format!("failed to restrict managed-sidecar lease permissions: {error}")
    })?;
    fs::rename(&temporary, &path)
        .map_err(|error| format!("failed to publish managed-sidecar lease: {error}"))?;
    Ok(())
}

fn read_managed_sidecar_lease() -> Option<ManagedSidecarLease> {
    let bytes = fs::read(managed_sidecar_lease_path()).ok()?;
    let lease = serde_json::from_slice::<ManagedSidecarLease>(&bytes).ok()?;
    (lease.schema_version == MANAGED_SIDECAR_LEASE_SCHEMA
        && valid_supervisor_session_id(&lease.supervisor_session_id)
        && lease.desktop_pid != 0
        && lease.sidecar_process_group_id != 0)
        .then_some(lease)
}

fn clear_managed_sidecar_lease(supervisor_session_id: &str) {
    let Some(lease) = read_managed_sidecar_lease() else {
        return;
    };
    if lease.supervisor_session_id == supervisor_session_id {
        let _ = fs::remove_file(managed_sidecar_lease_path());
    }
}

fn probe_forgecad_sidecar_identity() -> Option<ForgecadSidecarIdentity> {
    let response = loopback_get("/api/health", None).ok()?;
    if response.status != 200 {
        return None;
    }
    let payload = serde_json::from_str::<Value>(&response.body).ok()?;
    if payload.get("status").and_then(Value::as_str) != Some("ok")
        || payload.get("service").and_then(Value::as_str)
            != Some("forgecad-restricted-geometry-executor")
        || payload
            .get("persistent_state_writer")
            .and_then(Value::as_bool)
            != Some(false)
    {
        return None;
    }
    let supervisor_session_id = payload
        .get("supervisor_session_id")
        .and_then(Value::as_str)?
        .to_string();
    let process_group_id = payload
        .get("supervisor_process_group_id")
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())?;
    if !valid_supervisor_session_id(&supervisor_session_id) || process_group_id == 0 {
        return None;
    }
    Some(ForgecadSidecarIdentity {
        supervisor_session_id,
        process_group_id,
    })
}

fn managed_sidecar_lease_is_orphaned(
    identity: &ForgecadSidecarIdentity,
    lease: &ManagedSidecarLease,
    desktop_is_alive: bool,
) -> bool {
    identity.supervisor_session_id == lease.supervisor_session_id
        && identity.process_group_id == lease.sidecar_process_group_id
        && !desktop_is_alive
}

fn recover_orphaned_managed_sidecar() -> Result<bool, String> {
    let Some(identity) = probe_forgecad_sidecar_identity() else {
        return Ok(false);
    };
    let Some(lease) = read_managed_sidecar_lease() else {
        return Ok(false);
    };
    if !managed_sidecar_lease_is_orphaned(&identity, &lease, process_is_alive(lease.desktop_pid)) {
        return Ok(false);
    }
    terminate_process_group(lease.sidecar_process_group_id);
    for _ in 0..50 {
        if probe_forgecad_sidecar_identity().is_none() {
            clear_managed_sidecar_lease(&lease.supervisor_session_id);
            return Ok(true);
        }
        thread::sleep(Duration::from_millis(20));
    }
    Err("a stale ForgeCAD-managed sidecar did not release port 8000 after termination".to_string())
}

#[cfg(unix)]
fn process_is_alive(process_id: u32) -> bool {
    process_id != 0
        && Command::new("/bin/kill")
            .arg("-0")
            .arg(process_id.to_string())
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
}

#[cfg(not(unix))]
fn process_is_alive(_process_id: u32) -> bool {
    true
}

#[cfg(unix)]
fn process_group_is_alive(process_group_id: u32) -> bool {
    process_group_id != 0
        && Command::new("/bin/kill")
            .arg("-0")
            .arg(format!("-{process_group_id}"))
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
}

#[cfg(unix)]
fn terminate_process_group(process_group_id: u32) {
    if process_group_id == 0 {
        return;
    }
    let target = format!("-{process_group_id}");
    let _ = Command::new("/bin/kill").arg("-TERM").arg(&target).status();
    for _ in 0..25 {
        if !process_group_is_alive(process_group_id) {
            return;
        }
        thread::sleep(Duration::from_millis(20));
    }
    let _ = Command::new("/bin/kill").arg("-KILL").arg(target).status();
}

#[cfg(not(unix))]
fn terminate_process_group(_process_group_id: u32) {}

fn append_supervisor_log(message: &str) {
    let path = sidecar_log_path();
    let Some(parent) = path.parent() else {
        return;
    };
    if fs::create_dir_all(parent).is_err() {
        return;
    }
    let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) else {
        return;
    };
    let _ = writeln!(file, "{message}");
}

fn packaged_library_root() -> PathBuf {
    if let Ok(value) = env::var("WUSHEN_LIBRARY_ROOT") {
        return PathBuf::from(value);
    }
    if cfg!(target_os = "macos") {
        if let Ok(value) = env::var("HOME") {
            return PathBuf::from(value)
                .join("Library")
                .join("Application Support")
                .join("ForgeCAD")
                .join("Library");
        }
    }
    if let Ok(value) = env::var("LOCALAPPDATA") {
        return PathBuf::from(value).join("wushen-forge");
    }
    if let Ok(value) = env::var("HOME") {
        return PathBuf::from(value)
            .join(".local")
            .join("share")
            .join("wushen-forge");
    }
    PathBuf::from("WushenForgeLibrary")
}

fn sidecar_binary_path() -> Result<PathBuf, String> {
    if cfg!(debug_assertions) {
        if let Ok(override_path) = env::var("WUSHEN_AGENT_SIDE_CAR") {
            let candidate = PathBuf::from(override_path);
            if candidate.exists() {
                return Ok(candidate);
            }
            return Err(format!(
                "WUSHEN_AGENT_SIDE_CAR does not exist: {}",
                candidate.display()
            ));
        }
    }

    let executable = env::current_exe()
        .map_err(|error| format!("could not resolve packaged desktop executable: {error}"))?;
    let candidate = executable
        .parent()
        .ok_or_else(|| "packaged desktop executable has no parent directory".to_string())?
        .join(packaged_sidecar_name());
    if candidate.is_file() {
        return Ok(candidate);
    }
    Err(format!(
        "packaged sidecar binary not found beside the desktop executable: {}",
        candidate.display()
    ))
}

fn packaged_sidecar_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "wushen-agent.exe"
    } else {
        "wushen-agent"
    }
}

fn runtime_mode() -> AgentMode {
    let default_mode = if cfg!(debug_assertions) {
        AGENT_MODE_LOCAL
    } else {
        AGENT_MODE_PACKAGED
    };
    match env::var("WUSHEN_AGENT_RUNTIME_MODE")
        .unwrap_or_else(|_| default_mode.to_string())
        .as_str()
    {
        AGENT_MODE_PACKAGED => AgentMode::PackagedSidecar,
        _ => AgentMode::LocalDev,
    }
}

fn read_mode(mode: &str) -> String {
    match mode {
        AGENT_MODE_PACKAGED => AGENT_MODE_PACKAGED.to_string(),
        _ => AGENT_MODE_LOCAL.to_string(),
    }
}

fn managed_mode_name(state: &AgentProcessState) -> String {
    state
        .mode
        .lock()
        .map(|mode| read_mode(&mode))
        .unwrap_or_else(|_| AGENT_MODE_LOCAL.to_string())
}

struct LoopbackProbeResponse {
    status: u16,
    body: String,
}

#[derive(Debug)]
enum LoopbackProbeError {
    Offline,
    Invalid(String),
}

fn probe_agent(internal_capability_token: &str) -> AgentProbe {
    if !valid_internal_capability_token(internal_capability_token) {
        return AgentProbe::CapabilityMismatch(
            "the desktop generated an invalid internal capability".to_string(),
        );
    }
    let health = match loopback_get("/api/health", None) {
        Ok(response) => response,
        Err(LoopbackProbeError::Offline) => return AgentProbe::Offline,
        Err(LoopbackProbeError::Invalid(reason)) => return AgentProbe::WrongService(reason),
    };
    if health.status != 200 {
        return AgentProbe::WrongService(format!(
            "health endpoint returned HTTP {}",
            health.status
        ));
    }
    match serde_json::from_str::<serde_json::Value>(&health.body) {
        Ok(value)
            if value.get("status").and_then(serde_json::Value::as_str) == Some("ok")
                && value.get("service").and_then(serde_json::Value::as_str)
                    == Some("forgecad-restricted-geometry-executor")
                && value
                    .get("persistent_state_writer")
                    .and_then(serde_json::Value::as_bool)
                    == Some(false) => {}
        Ok(value) => {
            return AgentProbe::WrongService(format!("unexpected health payload: {value}"));
        }
        Err(error) => {
            return AgentProbe::WrongService(format!("invalid health JSON: {error}"));
        }
    }

    let ownership = match loopback_get(
        RESTRICTED_GEOMETRY_OWNERSHIP_PATH,
        Some((
            RESTRICTED_GEOMETRY_CAPABILITY_HEADER,
            internal_capability_token,
        )),
    ) {
        Ok(response) => response,
        Err(LoopbackProbeError::Offline) => {
            return AgentProbe::CapabilityMismatch(
                "the healthy sidecar closed before capability ownership could be verified"
                    .to_string(),
            );
        }
        Err(LoopbackProbeError::Invalid(reason)) => {
            return AgentProbe::CapabilityMismatch(format!(
                "the capability ownership response was invalid: {reason}"
            ));
        }
    };
    classify_capability_ownership_response(&ownership)
}

fn valid_internal_capability_token(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn valid_capability_ownership_payload(value: &serde_json::Value) -> bool {
    let Some(fields) = value.as_object() else {
        return false;
    };
    let restricted_geometry = fields.len() == 12
        && fields
            .get("schema_version")
            .and_then(serde_json::Value::as_str)
            == Some("RestrictedGeometryCapabilityOwnership@1")
        && fields
            .get("protocol_version")
            .and_then(serde_json::Value::as_str)
            == Some("forgecad.restricted-geometry/1")
        && fields
            .get("capability_owner")
            .and_then(serde_json::Value::as_str)
            == Some("rust_forgecad_core")
        && fields
            .get("python_role")
            .and_then(serde_json::Value::as_str)
            == Some("restricted_geometry_executor")
        && [
            "database_access",
            "object_store_access",
            "provider_access",
            "thread_session_access",
            "snapshot_write",
            "accepts_caller_glb",
            "persistent_artifacts",
        ]
        .iter()
        .all(|name| fields.get(*name).and_then(serde_json::Value::as_bool) == Some(false))
        && fields.get("actions").and_then(serde_json::Value::as_array)
            == Some(&vec![
                serde_json::Value::String("compile_readback".to_string()),
                serde_json::Value::String("render".to_string()),
            ]);
    restricted_geometry
}

fn classify_capability_ownership_response(ownership: &LoopbackProbeResponse) -> AgentProbe {
    if ownership.status != 200 {
        let reason = match ownership.status {
            403 => "the sidecar rejected this desktop capability",
            404 => "the sidecar does not expose the expected ownership handshake",
            503 => "the sidecar started without an internal capability",
            _ => "the sidecar did not accept the ownership handshake",
        };
        return AgentProbe::CapabilityMismatch(format!("{reason} (HTTP {})", ownership.status));
    }
    match serde_json::from_str::<serde_json::Value>(&ownership.body) {
        Ok(value) if valid_capability_ownership_payload(&value) => AgentProbe::Healthy,
        Ok(_) => AgentProbe::CapabilityMismatch(
            "the sidecar returned an unexpected capability ownership payload".to_string(),
        ),
        Err(_) => AgentProbe::CapabilityMismatch(
            "the sidecar returned invalid capability ownership JSON".to_string(),
        ),
    }
}

fn loopback_get(
    path: &str,
    header: Option<(&str, &str)>,
) -> Result<LoopbackProbeResponse, LoopbackProbeError> {
    let request = build_loopback_get_request(path, header)?;
    let addr = SocketAddr::from(([127, 0, 0, 1], AGENT_PORT));
    let mut stream = match TcpStream::connect_timeout(&addr, Duration::from_millis(200)) {
        Ok(stream) => stream,
        Err(_) => return Err(LoopbackProbeError::Offline),
    };
    let _ = stream.set_read_timeout(Some(Duration::from_millis(500)));
    let _ = stream.set_write_timeout(Some(Duration::from_millis(500)));

    if let Err(error) = stream.write_all(request.as_bytes()) {
        return Err(LoopbackProbeError::Invalid(format!(
            "probe request failed: {error}"
        )));
    }

    let mut response = String::new();
    if let Err(error) = stream.take(64 * 1024).read_to_string(&mut response) {
        return Err(LoopbackProbeError::Invalid(format!(
            "probe response failed: {error}"
        )));
    }
    parse_loopback_response(&response)
}

fn build_loopback_get_request(
    path: &str,
    header: Option<(&str, &str)>,
) -> Result<String, LoopbackProbeError> {
    if !path.starts_with('/') || path.contains('\r') || path.contains('\n') {
        return Err(LoopbackProbeError::Invalid(
            "probe path is invalid".to_string(),
        ));
    }
    let extra_header = match header {
        Some((name, value))
            if !name.is_empty()
                && !name.contains(['\r', '\n', ':'])
                && !value.is_empty()
                && !value.contains(['\r', '\n']) =>
        {
            format!("{name}: {value}\r\n")
        }
        Some(_) => {
            return Err(LoopbackProbeError::Invalid(
                "probe header is invalid".to_string(),
            ));
        }
        None => String::new(),
    };
    Ok(format!(
        "GET {path} HTTP/1.1\r\nHost: {AGENT_HOST}:{AGENT_PORT}\r\n{extra_header}Connection: close\r\n\r\n"
    ))
}

fn parse_loopback_response(response: &str) -> Result<LoopbackProbeResponse, LoopbackProbeError> {
    let (headers, body) = response
        .split_once("\r\n\r\n")
        .ok_or_else(|| LoopbackProbeError::Invalid("probe response was truncated".to_string()))?;
    let status_line = headers
        .lines()
        .next()
        .ok_or_else(|| LoopbackProbeError::Invalid("probe status line is missing".to_string()))?;
    let mut parts = status_line.split_ascii_whitespace();
    let protocol = parts.next().unwrap_or_default();
    let status = parts
        .next()
        .and_then(|value| value.parse::<u16>().ok())
        .ok_or_else(|| LoopbackProbeError::Invalid("probe HTTP status is invalid".to_string()))?;
    if !matches!(protocol, "HTTP/1.0" | "HTTP/1.1") {
        return Err(LoopbackProbeError::Invalid(
            "probe HTTP protocol is invalid".to_string(),
        ));
    }
    Ok(LoopbackProbeResponse {
        status,
        body: body.to_string(),
    })
}

fn agent_base_url() -> String {
    format!("http://{AGENT_HOST}:{AGENT_PORT}")
}

fn agent_health_url(base_url: &str) -> String {
    format!("{base_url}/api/health")
}

fn repo_root() -> Result<PathBuf, String> {
    if let Ok(value) = env::var("WUSHEN_REPO_ROOT") {
        let candidate = PathBuf::from(value);
        if is_repository_root(&candidate) {
            return Ok(candidate);
        }
    }
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    if let Some(candidate) = manifest_dir
        .ancestors()
        .nth(3)
        .map(Path::to_path_buf)
        .filter(|candidate| is_repository_root(candidate))
    {
        return Ok(candidate);
    }
    if let Ok(executable) = env::current_exe() {
        if let Some(candidate) = executable
            .ancestors()
            .find(|candidate| is_repository_root(candidate))
            .map(Path::to_path_buf)
        {
            return Ok(candidate);
        }
    }
    Err("could not resolve a ForgeCAD repository root for local-dev-python mode".to_string())
}

fn is_repository_root(candidate: &Path) -> bool {
    candidate.join("apps").join("agent").is_dir()
        && candidate.join("migrations").is_dir()
        && candidate.join(".venv").join("bin").join("python").exists()
}

fn local_library_root(repo_root: &Path) -> PathBuf {
    env::var_os("WUSHEN_LIBRARY_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| repo_root.join("WushenForgeLibrary"))
}

fn agent_python(repo_root: &Path) -> PathBuf {
    if let Ok(value) = env::var("WUSHEN_AGENT_PYTHON") {
        return PathBuf::from(value);
    }
    let venv_python = repo_root.join(".venv/bin/python");
    if venv_python.exists() {
        return venv_python;
    }
    PathBuf::from("python3")
}

#[cfg(test)]
mod tests {
    use std::{
        cell::Cell,
        collections::BTreeMap,
        env,
        ffi::OsStr,
        future::Future,
        process::{Command, Stdio},
        sync::{Arc, Mutex},
        time::Instant,
    };

    use forgecad_app_server_protocol::{AgentTurn, AppServerCursor, CursorPhase};
    use forgecad_core::semantic_sha256;
    use serde_json::{json, Value};

    use super::{
        agent_health_url, apply_sidecar_environment, arm_webview_qa_glb_readback,
        arm_webview_qa_png_dimensions, attach_c111b_packaged_native_metrics,
        attach_c111b_terminal_turn_metrics, build_loopback_get_request,
        c111b_terminal_turn_from_result, c111b_validate_agent_asset_lineage,
        c111b_validate_agent_parent_lineage, cancel_visual_evidence_request,
        classify_capability_ownership_response, finish_packaged_probe_report,
        finish_visual_evidence_request, generate_internal_capability_token,
        generate_supervisor_session_id, managed_sidecar_lease_is_orphaned,
        register_visual_evidence_request, status_from_probe, valid_internal_capability_token,
        valid_supervisor_session_id, validate_arm_webview_qa_success, validate_k001_probe_success,
        validate_k002_probe_success, validate_provider_config_input, AgentProbe,
        ArmWebviewQaGlbCapture, ArmWebviewQaPngCapture, ArmWebviewQaReport,
        C111bPackagedQaMetricsState, C111bPackagedQaTimeline, C111bPackagedWebglQaConfig,
        C111bPackagedWebglQaReport, ForgecadSidecarIdentity, K001PackagedProbeReport,
        K002PackagedProbeReport, LocalRoboticArmMvpProvider, LoopbackProbeResponse,
        ManagedSidecarLease, ProviderConfigMetadata, ARM_WEBVIEW_QA_SCHEMA,
        K001_PACKAGED_PROBE_SCHEMA, K002_PACKAGED_PROBE_SCHEMA, PROVIDER_ENVIRONMENT_KEYS,
        RESTRICTED_GEOMETRY_CAPABILITY_HEADER, RESTRICTED_GEOMETRY_OWNERSHIP_PATH,
    };

    const SIDECAR_ENVIRONMENT_PROBE_CHILD: &str = "FORGECAD_TEST_SIDECAR_ENVIRONMENT_PROBE_CHILD";
    const SIDECAR_ENVIRONMENT_PROBE_MARKER: &str = "ForgeCAD sidecar environment probe=";

    fn run_visual_async<T>(future: impl Future<Output = T>) -> T {
        tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .unwrap()
            .block_on(future)
    }

    #[test]
    fn c111b_agent_readback_requires_exact_v1_to_a005_v2_lineage() {
        let shape_program = json!({
            "schema_version": "ShapeProgram@1",
            "non_functional_only": true,
            "operations": [],
            "outputs": []
        });
        let shape_program_sha256 = semantic_sha256(&shape_program).unwrap();
        let adornment = json!({
            "schema_version": "SurfaceAdornmentProgram@1",
            "program_id": "adorn_c111b_test",
            "target_part_id": "part_c111b_link",
            "target_zone_id": "zone_arm_link_shell",
            "kind": "normal_relief",
            "motif": "parallel_groove",
            "intensity": "subtle",
            "coverage": "center_band",
            "seed": 77,
            "base_material": "mat_composite",
            "execution": "texture_bake",
            "skill_id": "skill_first_party_surface_adornment",
            "skill_version": 3,
            "skill_sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "generator": "a005_v1",
            "non_functional_only": true
        });
        let adornment_sha256 = semantic_sha256(&adornment).unwrap();
        let material_id = format!("mat_a005_{}", &adornment_sha256[..32]);
        let mut material_bindings = serde_json::Map::new();
        material_bindings.insert(
            "part_c111b_link:zone_arm_link_shell".into(),
            Value::String(material_id),
        );
        let asset = json!({
            "project_id": "project_c111b",
            "asset_version_id": "asset_v2",
            "parent_asset_version_id": "asset_v1",
            "version_no": 2,
            "status": "committed",
            "shape_program": shape_program,
            "assembly_graph": {"surface_adornments": [adornment]},
            "material_bindings": Value::Object(material_bindings)
        });
        assert_eq!(
            c111b_validate_agent_asset_lineage(&asset, &shape_program_sha256).unwrap(),
            "asset_v1"
        );

        let parent = json!({
            "project_id": "project_c111b",
            "asset_version_id": "asset_v1",
            "parent_asset_version_id": null,
            "version_no": 1,
            "shape_program": asset["shape_program"].clone()
        });
        c111b_validate_agent_parent_lineage(
            &parent,
            "project_c111b",
            "asset_v1",
            &shape_program_sha256,
        )
        .unwrap();

        let mut drifted_shape = asset.clone();
        drifted_shape["shape_program"]["operations"] = json!([{"op":"box"}]);
        assert!(c111b_validate_agent_asset_lineage(&drifted_shape, &shape_program_sha256).is_err());
        let mut missing_seal = asset.clone();
        missing_seal["material_bindings"] = json!({});
        assert!(c111b_validate_agent_asset_lineage(&missing_seal, &shape_program_sha256).is_err());
        let mut wrong_parent = parent;
        wrong_parent["version_no"] = json!(2);
        assert!(c111b_validate_agent_parent_lineage(
            &wrong_parent,
            "project_c111b",
            "asset_v1",
            &shape_program_sha256,
        )
        .is_err());
    }

    #[test]
    fn c111b_packaged_metrics_are_native_owned_and_overwrite_webview_claims() {
        let mut report: C111bPackagedWebglQaReport = serde_json::from_value(json!({
            "schema_version": "C111BPackagedWebGL@1",
            "phase": "restart",
            "ok": true,
            "provider_protocol_requests": 999,
            "network_provider_calls": 999,
            "network_call_made": true,
            "credential_reads": 999,
            "provider_metrics_source": "untrusted_webview",
            "credential_metrics_source": "untrusted_webview"
        }))
        .unwrap();
        let config = C111bPackagedWebglQaConfig {
            schema_version: "C111BPackagedWebGL@1",
            phase: "restart".into(),
            mode: "agent_asset".into(),
            source_sha256: "48ccc5c6a725936d43cb731ed5e20b93f10ef751712ed79469ea406318160b6b",
            triangle_count: 138_248,
            primitive_count: 157,
            material_count: 12,
            expected_project_id: None,
            expected_asset_version_id: None,
            expected_snapshot_revision: None,
            expected_export_sha256: None,
        };
        let metrics = C111bPackagedQaMetricsState {
            local_mvp_provider: Some(Arc::new(LocalRoboticArmMvpProvider::new())),
            timeline: Arc::new(Mutex::new(C111bPackagedQaTimeline {
                started: Instant::now(),
                stages: Vec::new(),
            })),
        };
        run_visual_async(attach_c111b_packaged_native_metrics(
            &mut report,
            &config,
            &metrics,
            None,
        ))
        .unwrap();
        assert_eq!(report.provider_protocol_requests, Some(0));
        assert_eq!(report.network_provider_calls, Some(0));
        assert_eq!(report.network_call_made, Some(false));
        assert_eq!(report.credential_reads, Some(0));
        assert_eq!(
            report.provider_metrics_source.as_deref(),
            Some("native_local_mvp_atomic_counter")
        );
        assert_eq!(
            report.credential_metrics_source.as_deref(),
            Some("native_structural_no_credential_source")
        );

        let mut external_report = report;
        let external_config = C111bPackagedWebglQaConfig {
            mode: "external_reference".into(),
            ..config
        };
        let external_metrics = C111bPackagedQaMetricsState {
            local_mvp_provider: None,
            timeline: Arc::new(Mutex::new(C111bPackagedQaTimeline {
                started: Instant::now(),
                stages: Vec::new(),
            })),
        };
        run_visual_async(attach_c111b_packaged_native_metrics(
            &mut external_report,
            &external_config,
            &external_metrics,
            None,
        ))
        .unwrap();
        assert_eq!(external_report.provider_protocol_requests, Some(0));
        assert_eq!(external_report.network_provider_calls, Some(0));
        assert_eq!(external_report.network_call_made, Some(false));
        assert_eq!(
            external_report.provider_metrics_source.as_deref(),
            Some("native_no_agent_provider_path")
        );
        assert_eq!(
            external_report.credential_metrics_source.as_deref(),
            Some("native_no_agent_provider_path")
        );
    }

    #[test]
    fn c111b_terminal_turn_metrics_override_untrusted_usage_and_bind_six_stages() {
        let stages = [
            ("author_forge_visual_program", "provider"),
            ("author_forge_visual_program", "product_tool"),
            ("build_candidate_geometry", "product_tool"),
            ("compile_readback_candidate", "product_tool"),
            ("render_candidate_views", "product_tool"),
            ("evaluate_candidate", "product_tool"),
            ("prepare_candidate_preview", "product_tool"),
        ];
        let mut entries = vec![
            json!({"sequence":1,"phase":"context","event":"started","elapsed_ms":0}),
            json!({"sequence":2,"phase":"context","event":"completed","elapsed_ms":1}),
        ];
        let mut elapsed = 1u64;
        for (tool_name, phase) in stages {
            let call_id = format!("call_{tool_name}_{elapsed}");
            entries.push(json!({
                "sequence": entries.len() + 1,
                "phase": phase,
                "event": "started",
                "elapsed_ms": elapsed,
                "call_id": call_id,
            }));
            elapsed += 1;
            entries.push(json!({
                "sequence": entries.len() + 1,
                "phase": phase,
                "event": "completed",
                "elapsed_ms": elapsed,
                "call_id": call_id,
                "tool_name": tool_name,
            }));
        }
        entries.push(json!({
            "sequence": entries.len() + 1,
            "phase": "final",
            "event": "completed",
            "elapsed_ms": elapsed + 1,
        }));
        let turn = json!({
            "thread_id":"thread_c111b_test",
            "turn_id":"turn_c111b_test",
            "request_text":"生成机械臂",
            "status":"completed",
            "usage":{
                "provider_requests":1,
                "product_tool_calls":6,
                "input_tokens":1,
                "output_tokens":1,
                "prompt_cache_hit_tokens":0,
                "prompt_cache_miss_tokens":0,
                "estimated_cost_microusd":1,
                "network_call_made":false,
                "outcome":"completed",
                "trace_sha256":"a".repeat(64),
                "redacted_trace":{"entries":entries}
            },
            "items":[{
                "item_id":"item_c111b_evaluate",
                "thread_id":"thread_c111b_test",
                "turn_id":"turn_c111b_test",
                "sequence":1,
                "item_type":"tool_result",
                "status":"completed",
                "payload":{
                    "tool_name":"evaluate_candidate",
                    "tool_result":{"validated_output":{"value":{
                        "visual_convergence_report":{"repair_attempt_count":0}
                    }}}
                },
                "created_at":"2026-07-28T00:00:00Z"
            }],
            "created_at":"2026-07-28T00:00:00Z",
            "updated_at":"2026-07-28T00:00:01Z"
        });
        let turn: AgentTurn = serde_json::from_value(turn).unwrap();
        let terminal_result = json!({
            "schema_version":"AgentTurnCommandResult@1",
            "command_id":"c111b_webview_report_turn_read",
            "result":{
                "outcome":"turn",
                "turn":turn
            }
        });
        let turn = c111b_terminal_turn_from_result(terminal_result.clone()).unwrap();
        let mut unknown_field = terminal_result.clone();
        unknown_field["untrusted_extra"] = json!(true);
        assert!(c111b_terminal_turn_from_result(unknown_field).is_err());
        let mut wrong_command = terminal_result;
        wrong_command["command_id"] = json!("different_command");
        assert!(c111b_terminal_turn_from_result(wrong_command).is_err());
        let mut report: C111bPackagedWebglQaReport = serde_json::from_value(json!({
            "schema_version":"C111BPackagedWebGL@1",
            "phase":"initial",
            "ok":true,
            "thread_id":"thread_c111b_test",
            "turn_id":"turn_c111b_test",
            "provider_protocol_requests":999,
            "product_tool_calls":999,
            "input_tokens":999,
            "estimated_cost_microusd":999,
            "same_intent_repair_attempts":999
        }))
        .unwrap();
        attach_c111b_terminal_turn_metrics(&mut report, &turn, 1).unwrap();
        assert_eq!(report.provider_protocol_requests, Some(1));
        assert_eq!(report.product_tool_calls, Some(6));
        assert_eq!(report.input_tokens, Some(1));
        assert_eq!(report.estimated_cost_microusd, Some(1));
        assert_eq!(report.same_intent_repair_attempts, Some(0));
        assert_eq!(report.same_intent_repairs_applied, Some(0));
        assert_eq!(
            report
                .turn_phase_timings_ms
                .as_ref()
                .map(|value| value.keys().cloned().collect::<Vec<_>>()),
            Some(vec![
                "author".into(),
                "compile_readback".into(),
                "evaluate".into(),
                "lower".into(),
                "preview".into(),
                "render".into(),
            ])
        );
        assert_eq!(
            report.turn_metrics_source.as_deref(),
            Some("rust_terminal_turn_readback")
        );
    }

    #[test]
    fn pv006b_visual_analysis_registry_rejects_duplicates_cancels_and_cleans_up() {
        let active = Mutex::new(std::collections::HashMap::new());
        let request_id = "vision_evidence_registry_test";
        let cancellation = register_visual_evidence_request(&active, request_id).unwrap();
        assert!(!cancellation.is_cancelled());
        assert_eq!(
            register_visual_evidence_request(&active, request_id).unwrap_err(),
            "VISION_EVIDENCE_REQUEST_ALREADY_ACTIVE"
        );
        assert!(cancel_visual_evidence_request(&active, request_id).unwrap());
        assert!(cancellation.is_cancelled());
        finish_visual_evidence_request(&active, request_id);
        assert!(!cancel_visual_evidence_request(&active, request_id).unwrap());
    }

    #[test]
    fn packaged_probe_completion_is_signaled_after_validation_and_recording_even_on_failure() {
        let recorded = Cell::new(false);
        let signaled = Cell::new(false);
        let result = finish_packaged_probe_report(
            || {
                assert!(!signaled.get());
                recorded.set(true);
                Err::<(), _>("stable validation failure".to_string())
            },
            || {
                assert!(recorded.get());
                signaled.set(true);
            },
        );

        assert_eq!(result, Err("stable validation failure".to_string()));
        assert!(signaled.get());
    }

    fn capture_sidecar_environment<I, K, V>(environment: I) -> BTreeMap<String, String>
    where
        I: IntoIterator<Item = (K, V)>,
        K: AsRef<OsStr>,
        V: AsRef<OsStr>,
    {
        let mut command =
            Command::new(env::current_exe().expect("resolve current Rust test binary"));
        apply_sidecar_environment(&mut command, environment);
        let output = command
            .env(SIDECAR_ENVIRONMENT_PROBE_CHILD, "1")
            .arg("--exact")
            .arg("tests::sidecar_environment_probe_child")
            .arg("--nocapture")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .expect("run isolated sidecar environment probe");
        assert!(
            output.status.success(),
            "sidecar environment probe failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8(output.stdout).expect("probe stdout is UTF-8");
        let marker = stdout
            .find(SIDECAR_ENVIRONMENT_PROBE_MARKER)
            .expect("probe marker is present");
        let report = stdout[marker + SIDECAR_ENVIRONMENT_PROBE_MARKER.len()..]
            .lines()
            .next()
            .expect("probe report follows marker");
        serde_json::from_str(report).expect("probe report is valid JSON")
    }

    #[test]
    fn sidecar_environment_probe_child() {
        if env::var(SIDECAR_ENVIRONMENT_PROBE_CHILD).as_deref() != Ok("1") {
            return;
        }
        let environment: BTreeMap<String, String> = env::vars().collect();
        println!(
            "{SIDECAR_ENVIRONMENT_PROBE_MARKER}{}",
            serde_json::to_string(&environment).expect("serialize child environment")
        );
    }

    #[test]
    fn internal_python_capability_is_random_bounded_hex() {
        let first = generate_internal_capability_token().unwrap();
        let second = generate_internal_capability_token().unwrap();
        assert_eq!(first.len(), 64);
        assert!(first.bytes().all(|byte| byte.is_ascii_hexdigit()));
        assert!(valid_internal_capability_token(&first));
        assert_ne!(first, second);
    }

    #[test]
    fn managed_sidecar_orphan_recovery_requires_exact_forgecad_lease_identity() {
        let session_id = generate_supervisor_session_id().unwrap();
        assert!(valid_supervisor_session_id(&session_id));
        let lease = ManagedSidecarLease {
            schema_version: "ForgeCADManagedSidecarLease@1".to_string(),
            supervisor_session_id: session_id.clone(),
            desktop_pid: 4242,
            sidecar_process_group_id: 5252,
        };
        let identity = ForgecadSidecarIdentity {
            supervisor_session_id: session_id.clone(),
            process_group_id: 5252,
        };
        assert!(managed_sidecar_lease_is_orphaned(&identity, &lease, false));
        assert!(!managed_sidecar_lease_is_orphaned(&identity, &lease, true));
        assert!(!managed_sidecar_lease_is_orphaned(
            &ForgecadSidecarIdentity {
                supervisor_session_id: "b".repeat(32),
                process_group_id: 5252,
            },
            &lease,
            false,
        ));
        assert!(!managed_sidecar_lease_is_orphaned(
            &ForgecadSidecarIdentity {
                supervisor_session_id: session_id,
                process_group_id: 5253,
            },
            &lease,
            false,
        ));
    }

    #[test]
    fn capability_ownership_request_is_header_bound_and_injection_safe() {
        let token = "a".repeat(64);
        let request = build_loopback_get_request(
            RESTRICTED_GEOMETRY_OWNERSHIP_PATH,
            Some((RESTRICTED_GEOMETRY_CAPABILITY_HEADER, &token)),
        )
        .unwrap();
        assert!(
            request.starts_with("GET /api/v1/internal/geometry/capability/ownership HTTP/1.1\r\n")
        );
        assert_eq!(
            request
                .matches(RESTRICTED_GEOMETRY_CAPABILITY_HEADER)
                .count(),
            1
        );
        assert!(request.contains(&format!(
            "{RESTRICTED_GEOMETRY_CAPABILITY_HEADER}: {token}\r\n"
        )));
        assert!(build_loopback_get_request(
            RESTRICTED_GEOMETRY_OWNERSHIP_PATH,
            Some((
                RESTRICTED_GEOMETRY_CAPABILITY_HEADER,
                "bad\r\ninjected: value"
            )),
        )
        .is_err());
    }

    #[test]
    fn capability_ownership_requires_exact_success_and_reports_mismatch() {
        let success = LoopbackProbeResponse {
            status: 200,
            body: serde_json::json!({
                "schema_version": "RestrictedGeometryCapabilityOwnership@1",
                "protocol_version": "forgecad.restricted-geometry/1",
                "capability_owner": "rust_forgecad_core",
                "python_role": "restricted_geometry_executor",
                "database_access": false,
                "object_store_access": false,
                "provider_access": false,
                "thread_session_access": false,
                "snapshot_write": false,
                "accepts_caller_glb": false,
                "persistent_artifacts": false,
                "actions": ["compile_readback", "render"],
            })
            .to_string(),
        };
        assert!(matches!(
            classify_capability_ownership_response(&success),
            AgentProbe::Healthy
        ));

        let rejected = LoopbackProbeResponse {
            status: 403,
            body: "{}".to_string(),
        };
        assert!(matches!(
            classify_capability_ownership_response(&rejected),
            AgentProbe::CapabilityMismatch(reason) if reason.contains("rejected")
        ));
        let mut malformed = success;
        malformed.body = serde_json::json!({
            "schema_version": "RestrictedGeometryCapabilityOwnership@1",
            "protocol_version": "forgecad.restricted-geometry/1",
            "capability_owner": "rust_forgecad_core",
            "python_role": "restricted_geometry_executor",
            "database_access": true,
        })
        .to_string();
        assert!(matches!(
            classify_capability_ownership_response(&malformed),
            AgentProbe::CapabilityMismatch(_)
        ));
    }

    #[test]
    fn capability_mismatch_is_never_reported_as_running() {
        let status = status_from_probe(
            AgentProbe::CapabilityMismatch("owned by another desktop".to_string()),
            false,
            None,
            "packaged-sidecar",
        );
        assert!(!status.running);
        assert!(!status.managed_by_desktop);
        assert_eq!(status.state, "capability_mismatch");
        assert_eq!(
            status.last_error.as_deref(),
            Some("owned by another desktop")
        );
    }

    #[test]
    fn provider_input_is_trimmed_before_storage() {
        let result = validate_provider_config_input(
            "  https://api.deepseek.com/// ",
            "  deepseek-demo  ",
            "  secret  ",
        )
        .expect("valid provider input");
        assert_eq!(result.0, "https://api.deepseek.com");
        assert_eq!(result.1, "deepseek-demo");
        assert_eq!(result.2.as_str(), "secret");
    }

    #[test]
    fn provider_input_rejects_invalid_url() {
        let error = validate_provider_config_input("api.example.test", "model", "key")
            .expect_err("invalid URL must be rejected");
        assert!(error.contains("HTTPS"));
        assert!(validate_provider_config_input("http://api.example.test", "model", "key").is_err());
        assert!(validate_provider_config_input(
            "https://user:pass@api.example.test",
            "model",
            "key"
        )
        .is_err());
    }

    #[test]
    fn provider_input_rejects_empty_or_oversized_fields() {
        assert!(validate_provider_config_input("https://example.test", "", "key").is_err());
        assert!(
            validate_provider_config_input("https://example.test", &"m".repeat(161), "key")
                .is_err()
        );
        assert!(validate_provider_config_input("https://example.test", "model", "").is_err());
        assert!(
            validate_provider_config_input("https://example.test", "model", &"k".repeat(4097))
                .is_err()
        );
    }

    #[test]
    fn legacy_provider_metadata_defaults_to_explicit_preflight_states() {
        let metadata: ProviderConfigMetadata = serde_json::from_str(
            r#"{"base_url":"https://api.deepseek.com","model":"deepseek-v4-pro","configured":true,"storage":"macos-keychain"}"#,
        )
        .expect("legacy metadata remains readable");
        assert_eq!(metadata.metadata_status, "not_checked");
        assert_eq!(metadata.secret_status, "not_checked");
        assert_eq!(metadata.supervisor_status, "not_checked");
        assert_eq!(metadata.capability_status, "unavailable");
    }

    #[test]
    fn provider_metadata_serialization_has_no_secret_field() {
        let metadata = ProviderConfigMetadata {
            base_url: "https://api.deepseek.com".to_string(),
            model: "deepseek-v4-pro".to_string(),
            configured: true,
            storage: "private_secret_file".to_string(),
            credential_id: None,
            metadata_status: "valid".to_string(),
            secret_status: "available".to_string(),
            supervisor_status: "running".to_string(),
            capability_status: "ready".to_string(),
            failure_code: None,
        };
        let serialized = serde_json::to_string(&metadata).expect("serialize metadata");
        assert!(!serialized.contains("api_key"));
        assert!(!serialized.contains("secret\":"));
        assert!(serialized.contains("\"capability_status\":\"ready\""));
    }

    #[test]
    fn sidecar_environment_drops_unknown_secret_variables() {
        let environment = capture_sidecar_environment([
            ("HOME", "/safe/home"),
            ("DEEPSEEK_CREDENTIAL", "must-not-reach-python"),
            ("MY_PROVIDER_CREDENTIAL", "must-not-reach-python"),
            ("DASHSCOPE_API_KEY", "must-not-reach-python"),
        ]);

        assert_eq!(
            environment.get("HOME").map(String::as_str),
            Some("/safe/home")
        );
        for name in [
            "DEEPSEEK_CREDENTIAL",
            "MY_PROVIDER_CREDENTIAL",
            "DASHSCOPE_API_KEY",
        ] {
            assert!(
                !environment.contains_key(name),
                "unknown credential-like environment variable {name} must be absent"
            );
        }
    }

    #[test]
    fn sidecar_environment_forwards_required_safe_context_only() {
        let required = [
            ("HOME", "/safe/home"),
            ("USERPROFILE", "C:\\Users\\safe"),
            ("LOCALAPPDATA", "C:\\Users\\safe\\AppData\\Local"),
            ("APPDATA", "C:\\Users\\safe\\AppData\\Roaming"),
            ("TMPDIR", "/safe/tmpdir"),
            ("TMP", "/safe/tmp"),
            ("TEMP", "/safe/temp"),
            ("SystemRoot", "C:\\Windows"),
            ("WINDIR", "C:\\Windows"),
            ("PATH", "/safe/bin"),
            ("LANG", "zh_CN.UTF-8"),
            ("LC_ALL", "zh_CN.UTF-8"),
            ("WUSHEN_AGENT_RUNTIME_MODE", "packaged-sidecar"),
            ("FORGECAD_DISABLE_PROVIDER_CONFIG", "1"),
            ("FORGECAD_CONCEPT_WORKER_ENABLED", "0"),
            ("WUSHEN_LOCAL_WORKER_ENABLED", "0"),
            ("WUSHEN_RECOVER_ON_STARTUP", "0"),
            ("FORGECAD_CONCEPT_RECOVER_ON_STARTUP", "0"),
        ];
        let mut input = required.to_vec();
        input.extend([
            ("WUSHEN_LIBRARY_ROOT", "/must/be/explicit"),
            ("PYTHONPATH", "/must/be/explicit"),
            (
                "FORGECAD_K002_INTERNAL_CAPABILITY_TOKEN",
                "must-be-explicit",
            ),
            (
                "FORGECAD_RESTRICTED_GEOMETRY_CAPABILITY_TOKEN",
                "must-be-explicit",
            ),
            ("WUSHEN_MIGRATIONS_DIR", "/must/be/explicit"),
        ]);
        let environment = capture_sidecar_environment(input);

        for (name, value) in required {
            assert_eq!(
                environment.get(name).map(String::as_str),
                Some(value),
                "safe environment variable {name} must be forwarded"
            );
        }
        for name in [
            "WUSHEN_LIBRARY_ROOT",
            "PYTHONPATH",
            "FORGECAD_K002_INTERNAL_CAPABILITY_TOKEN",
            "FORGECAD_RESTRICTED_GEOMETRY_CAPABILITY_TOKEN",
            "WUSHEN_MIGRATIONS_DIR",
        ] {
            assert!(
                !environment.contains_key(name),
                "launcher-owned environment variable {name} must be injected explicitly"
            );
        }
    }

    #[test]
    fn sidecar_environment_explicitly_removes_known_provider_variables() {
        let environment = capture_sidecar_environment(
            PROVIDER_ENVIRONMENT_KEYS
                .iter()
                .map(|name| (*name, "must-not-reach-python")),
        );

        for name in PROVIDER_ENVIRONMENT_KEYS {
            assert!(
                !environment.contains_key(*name),
                "{name} must not reach the sidecar process"
            );
        }
    }

    #[test]
    fn packaged_python_facet_never_receives_probe_or_legacy_writer_switches() {
        let environment = capture_sidecar_environment([
            ("FORGECAD_K001_PACKAGED_PROBE", "1"),
            ("FORGECAD_K002_PACKAGED_PROBE", "1"),
            ("FORGECAD_K003_PACKAGED_PROBE", "1"),
            ("FORGECAD_TEST_ONLY_LEGACY_AGENT_LIFECYCLE", "1"),
            ("FORGECAD_TEST_ONLY_LEGACY_PRODUCT_CORE", "1"),
        ]);

        for name in [
            "FORGECAD_K001_PACKAGED_PROBE",
            "FORGECAD_K002_PACKAGED_PROBE",
            "FORGECAD_K003_PACKAGED_PROBE",
            "FORGECAD_TEST_ONLY_LEGACY_AGENT_LIFECYCLE",
            "FORGECAD_TEST_ONLY_LEGACY_PRODUCT_CORE",
        ] {
            assert!(
                !environment.contains_key(name),
                "Rust probe or legacy writer switch {name} must not reach the Python facet"
            );
        }
    }

    #[test]
    fn health_url_is_stable_and_does_not_drop_base_path() {
        assert_eq!(
            agent_health_url("http://127.0.0.1:8000"),
            "http://127.0.0.1:8000/api/health"
        );
        assert_eq!(
            agent_health_url("http://127.0.0.1:8000/agent"),
            "http://127.0.0.1:8000/agent/api/health"
        );
    }

    #[test]
    fn packaged_k001_probe_requires_native_replay_rust_product_and_one_glb() {
        let cursor = AppServerCursor::new(
            "thread_probe",
            Some("turn_probe".to_string()),
            7,
            CursorPhase::Item,
            Some("item_probe".to_string()),
        )
        .encode()
        .unwrap();
        let sha = "a".repeat(64);
        let mut report = K001PackagedProbeReport {
            schema_version: K001_PACKAGED_PROBE_SCHEMA.to_string(),
            phase: "initial".to_string(),
            ok: true,
            project_id: Some("project_probe".to_string()),
            thread_id: Some("thread_probe".to_string()),
            asset_version_id: Some("asset_probe".to_string()),
            first_event_id: Some("1".to_string()),
            last_event_id: Some("7".to_string()),
            cursor: Some(cursor),
            resume_from_event_id: None,
            resume_from_cursor: None,
            glb_sha256: Some(sha.clone()),
            protocol_glb_sha256: Some(sha.clone()),
            resource_glb_sha256: Some(sha),
            notification_count: Some(7),
            native_lifecycle_transport: Some(true),
            native_item_replay_verified: Some(true),
            product_state_owner: Some("rust_app_server".to_string()),
            python_product_api_used: Some(false),
            turn_status: Some("failed".to_string()),
            turn_error_code: Some("PROVIDER_NOT_CONFIGURED".to_string()),
            provider_calls: Some(0),
            error_code: None,
            diagnostic: None,
        };
        validate_k001_probe_success(&report).unwrap();

        report.resource_glb_sha256 = Some("b".repeat(64));
        assert!(validate_k001_probe_success(&report).is_err());
        report.resource_glb_sha256 = report.glb_sha256.clone();
        report.notification_count = Some(0);
        assert!(validate_k001_probe_success(&report).is_err());
        report.notification_count = Some(7);
        report.python_product_api_used = Some(true);
        assert!(validate_k001_probe_success(&report).is_err());
    }

    #[test]
    fn k002_packaged_probe_requires_failed_no_network_ordered_replay() {
        let mut report = valid_k002_packaged_probe_report();
        validate_k002_probe_success(&report).unwrap();

        report.provider_network_call_made = Some(true);
        assert!(validate_k002_probe_success(&report).is_err());
        report.provider_network_call_made = Some(false);
        report.supervisor_managed_by_desktop = Some(false);
        assert!(validate_k002_probe_success(&report).is_err());
        report.supervisor_managed_by_desktop = Some(true);
        report.reasoning_content_present = Some(true);
        assert!(validate_k002_probe_success(&report).is_err());
        report.reasoning_content_present = Some(false);
        report.item_sequences = Some(vec![1, 1]);
        assert!(validate_k002_probe_success(&report).is_err());
        report.item_sequences = Some(vec![1, 2]);
        report.replay_items_sha256 = Some("b".repeat(64));
        assert!(validate_k002_probe_success(&report).is_err());
    }

    #[test]
    fn mechanical_arm_webview_qa_requires_one_renderer_and_v3_r007b_lineage() {
        let mut report = valid_arm_webview_qa_report();
        validate_arm_webview_qa_success(&report).unwrap();

        report.active_webgl_contexts = Some(2);
        assert!(validate_arm_webview_qa_success(&report).is_err());
        report.active_webgl_contexts = Some(1);
        report.v2_asset_version_id = report.v1_asset_version_id.clone();
        assert!(validate_arm_webview_qa_success(&report).is_err());
        report.v2_asset_version_id = Some("asset_arm_v2".to_string());
        report.a005_preview_seen = Some(false);
        assert!(validate_arm_webview_qa_success(&report).is_err());
        report.a005_preview_seen = Some(true);
        report.r007b_v3_confirmed = Some(false);
        assert!(validate_arm_webview_qa_success(&report).is_err());
        report.r007b_v3_confirmed = Some(true);
        report.v3_glb_download_confirmed = Some(false);
        assert!(validate_arm_webview_qa_success(&report).is_err());
        report.v3_glb_download_confirmed = Some(true);
        report.visual_fidelity_validated = Some(true);
        assert!(validate_arm_webview_qa_success(&report).is_err());
        report.visual_fidelity_validated = Some(false);
        report.v3_production_glb.as_mut().unwrap().triangle_count = 4;
        assert!(validate_arm_webview_qa_success(&report).is_err());
        report.v3_production_glb.as_mut().unwrap().triangle_count = 14_392;
        report
            .v3_viewport_screenshot
            .as_mut()
            .unwrap()
            .relative_path = "../not-a-capture.png".to_string();
        assert!(validate_arm_webview_qa_success(&report).is_err());
        report
            .v3_viewport_screenshot
            .as_mut()
            .unwrap()
            .relative_path = "qa-artifacts/arm-webview/initial/v3_viewport_png.png".to_string();
        report.production_glb_render_source = Some("shape_program_fallback".to_string());
        assert!(validate_arm_webview_qa_success(&report).is_err());
    }

    #[test]
    fn mechanical_arm_webview_qa_capture_readback_rejects_lightweight_or_non_pbr_glb() {
        let production = arm_webview_qa_test_glb(36, true);
        assert_eq!(arm_webview_qa_glb_readback(&production).unwrap(), (12, 1));
        assert!(arm_webview_qa_glb_readback(&arm_webview_qa_test_glb(12, false)).is_err());
        assert!(arm_webview_qa_glb_readback(&arm_webview_qa_test_glb(10, true)).is_err());
    }

    #[test]
    fn mechanical_arm_webview_qa_capture_requires_real_sized_png_payload() {
        let png = arm_webview_qa_test_png(960, 720);
        assert_eq!(arm_webview_qa_png_dimensions(&png).unwrap(), (960, 720));
        assert!(arm_webview_qa_png_dimensions(&arm_webview_qa_test_png(48, 32)).is_err());
    }

    fn arm_webview_qa_test_glb(index_count: u64, complete_pbr: bool) -> Vec<u8> {
        let material = if complete_pbr {
            serde_json::json!({
                "pbrMetallicRoughness": {"baseColorTexture": {"index": 0}, "metallicRoughnessTexture": {"index": 1}},
                "normalTexture": {"index": 2},
                "occlusionTexture": {"index": 3},
                "emissiveTexture": {"index": 4}
            })
        } else {
            serde_json::json!({"pbrMetallicRoughness": {"baseColorTexture": {"index": 0}}})
        };
        let mut json = serde_json::to_vec(&serde_json::json!({
            "asset": {"version": "2.0"},
            "accessors": [{"count": index_count}],
            "meshes": [{"primitives": [{"indices": 0, "mode": 4}]}],
            "materials": [material]
        }))
        .unwrap();
        while json.len() % 4 != 0 {
            json.push(b' ');
        }
        let total = 20 + json.len();
        let mut glb = Vec::with_capacity(total);
        glb.extend_from_slice(&0x4654_6c67u32.to_le_bytes());
        glb.extend_from_slice(&2u32.to_le_bytes());
        glb.extend_from_slice(&(total as u32).to_le_bytes());
        glb.extend_from_slice(&(json.len() as u32).to_le_bytes());
        glb.extend_from_slice(&0x4e4f_534au32.to_le_bytes());
        glb.extend_from_slice(&json);
        glb
    }

    fn arm_webview_qa_test_png(width: u32, height: u32) -> Vec<u8> {
        let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
        let mut ihdr = Vec::new();
        ihdr.extend_from_slice(&width.to_be_bytes());
        ihdr.extend_from_slice(&height.to_be_bytes());
        ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
        arm_webview_qa_test_png_chunk(&mut png, b"IHDR", &ihdr);
        arm_webview_qa_test_png_chunk(&mut png, b"IDAT", &[1]);
        arm_webview_qa_test_png_chunk(&mut png, b"IEND", &[]);
        png
    }

    fn arm_webview_qa_test_png_chunk(output: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
        output.extend_from_slice(&(data.len() as u32).to_be_bytes());
        output.extend_from_slice(kind);
        output.extend_from_slice(data);
        output.extend_from_slice(&0u32.to_be_bytes());
    }

    fn valid_arm_webview_qa_report() -> ArmWebviewQaReport {
        ArmWebviewQaReport {
            schema_version: ARM_WEBVIEW_QA_SCHEMA.to_string(),
            phase: "initial".to_string(),
            ok: true,
            project_id: Some("project_arm".to_string()),
            turn_id: Some("turn_arm".to_string()),
            preview_id: Some("preview_arm".to_string()),
            preview_artifact_sha256: Some("a".repeat(64)),
            v1_asset_version_id: Some("asset_arm_v1".to_string()),
            v2_asset_version_id: Some("asset_arm_v2".to_string()),
            v3_asset_version_id: Some("asset_arm_v3".to_string()),
            snapshot_revision: Some(4),
            renderer_generation: Some(1),
            active_webgl_contexts: Some(1),
            production_glb_render_source: Some("glb_pbr".to_string()),
            a005_preview_seen: Some(true),
            r007b_preview_seen: Some(true),
            r007b_v3_confirmed: Some(true),
            v3_glb_download_confirmed: Some(true),
            v3_production_glb: Some(ArmWebviewQaGlbCapture {
                relative_path: "qa-artifacts/arm-webview/initial/v3_production_glb.glb".to_string(),
                sha256: "b".repeat(64),
                byte_size: 5_029_440,
                triangle_count: 14_392,
                complete_pbr_material_count: 1,
            }),
            v3_viewport_screenshot: Some(ArmWebviewQaPngCapture {
                relative_path: "qa-artifacts/arm-webview/initial/v3_viewport_png.png".to_string(),
                sha256: "c".repeat(64),
                byte_size: 20_000,
                width: 960,
                height: 720,
            }),
            visual_fidelity_validated: Some(false),
            restart_hydrated: Some(false),
            r007b_visual_run: None,
            error_code: None,
        }
    }

    fn valid_k002_packaged_probe_report() -> K002PackagedProbeReport {
        K002PackagedProbeReport {
            schema_version: K002_PACKAGED_PROBE_SCHEMA.to_string(),
            phase: "initial".to_string(),
            ok: true,
            thread_id: Some("thread_probe".to_string()),
            turn_id: Some("turn_probe".to_string()),
            turn_status: Some("failed".to_string()),
            turn_error_code: Some("PROVIDER_NOT_CONFIGURED".to_string()),
            provider_status: Some("unconfigured".to_string()),
            provider_configured: Some(false),
            provider_network_call_made: Some(false),
            supervisor_running: Some(true),
            supervisor_state: Some("running".to_string()),
            supervisor_managed_by_desktop: Some(true),
            reasoning_content_present: Some(false),
            legacy_lifecycle_post_status: Some(410),
            provider_calls: Some(0),
            item_count: Some(2),
            last_sequence: Some(2),
            item_sequences: Some(vec![1, 2]),
            item_ids: Some(vec!["item_user".to_string(), "item_gateway".to_string()]),
            item_types: Some(vec!["user_message".to_string(), "tool_result".to_string()]),
            items_sha256: Some("a".repeat(64)),
            replay_items_sha256: Some("a".repeat(64)),
            error_code: None,
        }
    }
}
