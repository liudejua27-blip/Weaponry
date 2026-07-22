mod app_server_bridge;
mod asset_render_compat;
mod c110g_packaged_probe;
mod deepseek_delta_acceptance_probe;
mod deepseek_mvp_acceptance_probe;
mod deepseek_provider;
mod k003_packaged_probe;
mod mvp_arm_packaged_probe;
mod mvp_arm_provider;
mod provider_credentials;
mod rust_core_runtime;
mod rust_product_catalog;

use std::{
    env,
    ffi::OsStr,
    fs::{self, OpenOptions},
    io::{Read, Write},
    net::{SocketAddr, TcpStream},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{Arc, Condvar, Mutex, OnceLock},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[cfg(unix)]
use std::os::unix::process::CommandExt;

use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use forgecad_app_server::{
    compatibility::{AllowedHttpMethod, LocalAgentEndpoint, PreparedCompatHttpRequest},
    CancellationToken,
};
use forgecad_app_server_protocol::ProtocolHttpBody;
use forgecad_core::semantic_sha256;
use serde::Deserialize;
use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tauri::{Manager, State};

use app_server_bridge::{
    forgecad_protocol_connect, forgecad_protocol_disconnect, forgecad_protocol_send,
    AppServerBridge,
};
use deepseek_provider::{
    DeepSeekPricing, DeepSeekProviderClient, DeepSeekProviderConfig, ReqwestDeepSeekTransport,
};
use forgecad_app_server::ProviderClient;
use mvp_arm_provider::{LocalRoboticArmMvpProvider, MVP_MODEL};
use provider_credentials::{
    validate_provider_config_input, ProviderConfigMetadata, ProviderCredentialStore,
};
use rust_core_runtime::RustCoreRuntime;
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
const RESTRICTED_GEOMETRY_CAPABILITY_HEADER: &str = "X-ForgeCAD-Restricted-Geometry-Capability";
const RESTRICTED_GEOMETRY_OWNERSHIP_PATH: &str = "/api/v1/internal/geometry/capability/ownership";
// Deterministic budget-accounting coefficients, not a claim about current
// DeepSeek billing. The per-Turn cost gate remains an explicit conservative
// policy even when external prices change.
const K002_INPUT_BUDGET_MICROUSD_PER_MILLION_TOKENS: u64 = 1_000_000;
const K002_OUTPUT_BUDGET_MICROUSD_PER_MILLION_TOKENS: u64 = 4_000_000;

struct AgentProcessState {
    child: Mutex<Option<Child>>,
    mode: Mutex<String>,
    internal_capability_token: String,
    provider_credentials: Arc<ProviderCredentialStore>,
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
) -> Result<Arc<dyn ProviderClient>, String> {
    if mvp_offline_arm_enabled() {
        return Ok(Arc::new(LocalRoboticArmMvpProvider::new()));
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
    Ok(Arc::new(client))
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

#[tauri::command]
fn save_provider_config(
    mut request: SaveProviderConfigRequest,
    state: State<'_, AgentProcessState>,
) -> Result<ProviderConfigMetadata, String> {
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
    if mvp_offline_arm_enabled() {
        return Err("本机机械臂 MVP 不读取或清除 Provider Key。".into());
    }
    let metadata = state.provider_credentials.clear()?;
    Ok(provider_config_with_runtime_status(
        metadata,
        &state.internal_capability_token,
    ))
}

impl AgentProcessState {
    fn shutdown_managed(&self) {
        if let Ok(mut child_guard) = self.child.lock() {
            if let Some(child) = child_guard.as_mut() {
                // PyInstaller onefile uses a parent process which unpacks and
                // then starts the actual Agent child. The desktop owns a
                // dedicated process group so normal window close stops both,
                // rather than leaving a hidden listener on port 8000.
                #[cfg(unix)]
                let _ = Command::new("kill")
                    .arg("-TERM")
                    .arg(format!("-{}", child.id()))
                    .status();
                let _ = child.kill();
                let _ = child.wait();
            }
            *child_guard = None;
        }
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
                "Port 8000 is occupied by a ForgeCAD sidecar not owned by this desktop session: {reason}"
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
        AgentMode::PackagedSidecar => start_packaged_sidecar(&state.internal_capability_token)?,
        AgentMode::LocalDev => {
            let repo_root = repo_root()?;
            start_local_python_sidecar(&repo_root, &state.internal_capability_token)?
        }
    };
    let pid = child.id();
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
    let provider_credentials = ProviderCredentialStore::production();
    let native_provider = build_native_provider_client(provider_credentials.clone())
        .expect("ForgeCAD must initialize its Rust-owned DeepSeek Provider client");
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
        native_provider,
        Arc::clone(&rust_core),
    ) {
        Ok(bridge) => bridge,
        Err(error) => {
            let _ = rust_core.rollback_cutover_before_publish();
            panic!("ForgeCAD app-server bridge initialization failed before cutover: {error}");
        }
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
    tauri::Builder::default()
        .manage(app_server_bridge)
        .manage(AgentProcessState {
            child: Mutex::new(None),
            mode: Mutex::new(AGENT_MODE_LOCAL.to_string()),
            internal_capability_token,
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
            forgecad_k001_packaged_probe_config,
            forgecad_k001_packaged_probe_report,
            forgecad_k002_packaged_probe_config,
            forgecad_k002_packaged_probe_report,
            forgecad_arm_webview_qa_config,
            forgecad_arm_webview_qa_capture,
            forgecad_arm_webview_qa_r007b_lineage,
            forgecad_arm_webview_qa_report,
            forgecad_arm_webview_qa_progress,
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
        .run(tauri::generate_context!())
        .expect("error while running Wushen Forge desktop");
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
) -> Result<Child, String> {
    let log_path = repo_root.join(".wushen-agent.log");
    if let Some(parent) = log_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create Agent log directory: {error}"))?;
    }
    let mut command = Command::new(&agent_python(repo_root));
    apply_sidecar_environment(&mut command, env::vars_os());
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
    configure_python_facet_environment(&mut command, internal_capability_token);

    command.spawn().map_err(|error| {
        format!(
            "failed to start local-agent service with {}: {error}",
            agent_python(repo_root).display()
        )
    })
}

fn start_packaged_sidecar(internal_capability_token: &str) -> Result<Child, String> {
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
    configure_python_facet_environment(&mut command, internal_capability_token);
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

fn configure_python_facet_environment(command: &mut Command, internal_capability_token: &str) {
    // K003 gives Python only one ephemeral compiler capability. Database,
    // object-store and Provider locations are deliberately absent. No
    // environment switch can select the retired Python product writer.
    command.env(
        "FORGECAD_RESTRICTED_GEOMETRY_CAPABILITY_TOKEN",
        internal_capability_token,
    );
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
        process::{Command, Stdio},
    };

    use forgecad_app_server_protocol::{AppServerCursor, CursorPhase};

    use super::{
        agent_health_url, apply_sidecar_environment, arm_webview_qa_glb_readback,
        arm_webview_qa_png_dimensions, build_loopback_get_request,
        classify_capability_ownership_response, finish_packaged_probe_report,
        generate_internal_capability_token, status_from_probe, valid_internal_capability_token,
        validate_arm_webview_qa_success, validate_k001_probe_success, validate_k002_probe_success,
        validate_provider_config_input, AgentProbe, ArmWebviewQaGlbCapture, ArmWebviewQaPngCapture,
        ArmWebviewQaReport, K001PackagedProbeReport, K002PackagedProbeReport,
        LoopbackProbeResponse, ProviderConfigMetadata, ARM_WEBVIEW_QA_SCHEMA,
        K001_PACKAGED_PROBE_SCHEMA, K002_PACKAGED_PROBE_SCHEMA, PROVIDER_ENVIRONMENT_KEYS,
        RESTRICTED_GEOMETRY_CAPABILITY_HEADER, RESTRICTED_GEOMETRY_OWNERSHIP_PATH,
    };

    const SIDECAR_ENVIRONMENT_PROBE_CHILD: &str = "FORGECAD_TEST_SIDECAR_ENVIRONMENT_PROBE_CHILD";
    const SIDECAR_ENVIRONMENT_PROBE_MARKER: &str = "ForgeCAD sidecar environment probe=";

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
            "  https://api.example.test/// ",
            "  demo-model  ",
            "  secret  ",
        )
        .expect("valid provider input");
        assert_eq!(result.0, "https://api.example.test");
        assert_eq!(result.1, "demo-model");
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
            storage: "macos-keychain".to_string(),
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
