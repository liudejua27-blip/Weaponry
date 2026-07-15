use std::{
    env,
    fs::{self, OpenOptions},
    io::{Read, Write},
    net::{SocketAddr, TcpStream},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::Mutex,
    thread,
    time::Duration,
};

#[cfg(unix)]
use std::os::unix::{fs::OpenOptionsExt, process::CommandExt};

use serde::Deserialize;
use serde::Serialize;
use tauri::{Manager, State};

const AGENT_HOST: &str = "127.0.0.1";
const AGENT_PORT: u16 = 8000;
const AGENT_MODE_PACKAGED: &str = "packaged-sidecar";
const AGENT_MODE_LOCAL: &str = "local-dev-python";

struct AgentProcessState {
    child: Mutex<Option<Child>>,
    mode: Mutex<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ProviderConfigMetadata {
    base_url: String,
    model: String,
    configured: bool,
    storage: String,
    #[serde(default = "provider_status_not_checked")]
    metadata_status: String,
    #[serde(default = "provider_status_not_checked")]
    secret_status: String,
    #[serde(default = "provider_status_not_checked")]
    supervisor_status: String,
    #[serde(default = "provider_status_unavailable")]
    capability_status: String,
    #[serde(default)]
    failure_code: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct SaveProviderConfigRequest {
    base_url: String,
    model: String,
    api_key: String,
}

const KEYCHAIN_SERVICE: &str = "ForgeCAD Agent Provider";
const KEYCHAIN_ACCOUNT: &str = "default";

#[tauri::command]
fn get_provider_config(state: State<'_, AgentProcessState>) -> Result<ProviderConfigMetadata, String> {
    let metadata_path_exists = provider_metadata_path().is_file();
    let mut metadata = read_provider_metadata().unwrap_or_else(|_| ProviderConfigMetadata {
        base_url: "https://api.deepseek.com".to_string(),
        model: "deepseek-v4-pro".to_string(),
        configured: false,
        storage: provider_storage_name().to_string(),
        metadata_status: if metadata_path_exists { "invalid" } else { "missing" }.to_string(),
        secret_status: "not_checked".to_string(),
        supervisor_status: "not_checked".to_string(),
        capability_status: "unavailable".to_string(),
        failure_code: Some(if metadata_path_exists { "PROVIDER_METADATA_INVALID" } else { "PROVIDER_METADATA_MISSING" }.to_string()),
    });
    if metadata_path_exists && metadata.metadata_status == "not_checked" {
        metadata.metadata_status = "valid".to_string();
    }
    let secret_available = read_provider_secret()
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false);
    metadata.secret_status = if secret_available { "available" } else { "missing" }.to_string();
    metadata.configured = metadata.metadata_status == "valid"
        && !metadata.base_url.is_empty()
        && !metadata.model.is_empty()
        && secret_available;
    metadata.supervisor_status = if matches!(probe_agent(), AgentProbe::Healthy) {
        "running"
    } else {
        "unavailable"
    }.to_string();
    metadata.capability_status = probe_agent_provider_capability()
        .unwrap_or_else(|_| "unavailable".to_string());
    if metadata.configured && metadata.capability_status != "ready" {
        metadata.failure_code = Some("PROVIDER_CAPABILITY_MISMATCH".to_string());
    } else if metadata.configured {
        metadata.failure_code = None;
    }
    let _ = state;
    Ok(metadata)
}

#[tauri::command]
fn save_provider_config(
    request: SaveProviderConfigRequest,
    state: State<'_, AgentProcessState>,
) -> Result<ProviderConfigMetadata, String> {
    let (base_url, model, api_key) = validate_provider_config_input(
        &request.base_url,
        &request.model,
        &request.api_key,
    )?;
    write_provider_secret(&api_key)?;
    let metadata = ProviderConfigMetadata {
        base_url,
        model,
        configured: true,
        storage: provider_storage_name().to_string(),
        metadata_status: "valid".to_string(),
        secret_status: "available".to_string(),
        supervisor_status: "not_checked".to_string(),
        capability_status: "unavailable".to_string(),
        failure_code: None,
    };
    write_provider_metadata(&metadata)?;
    activate_provider_config(metadata, state)
}

fn validate_provider_config_input(
    base_url: &str,
    model: &str,
    api_key: &str,
) -> Result<(String, String, String), String> {
    let base_url = base_url.trim().trim_end_matches('/').to_string();
    let model = model.trim().to_string();
    let api_key = api_key.trim().to_string();
    if !(base_url.starts_with("https://") || base_url.starts_with("http://")) {
        return Err("API Base URL 必须是 http(s) 地址。".to_string());
    }
    if model.is_empty() || model.len() > 160 {
        return Err("Model 不能为空且不能超过 160 个字符。".to_string());
    }
    if api_key.is_empty() || api_key.len() > 4096 {
        return Err("API Key 不能为空。".to_string());
    }
    Ok((base_url, model, api_key))
}

#[tauri::command]
fn clear_provider_config(state: State<'_, AgentProcessState>) -> Result<ProviderConfigMetadata, String> {
    clear_provider_secret()?;
    let metadata = ProviderConfigMetadata {
        base_url: "https://api.deepseek.com".to_string(),
        model: "deepseek-v4-pro".to_string(),
        configured: false,
        storage: provider_storage_name().to_string(),
        metadata_status: "valid".to_string(),
        secret_status: "missing".to_string(),
        supervisor_status: "not_checked".to_string(),
        capability_status: "offline".to_string(),
        failure_code: None,
    };
    write_provider_metadata(&metadata)?;
    activate_provider_config(metadata, state)
}

fn provider_status_not_checked() -> String {
    "not_checked".to_string()
}

fn provider_status_unavailable() -> String {
    "unavailable".to_string()
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

    status_from_probe(probe_agent(), managed_running, pid, &mode_name)
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
            if let AgentProbe::Healthy = probe_agent() {
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

    match probe_agent() {
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
        AgentMode::PackagedSidecar => start_packaged_sidecar()?,
        AgentMode::LocalDev => {
            let repo_root = repo_root()?;
            start_local_python_sidecar(&repo_root)?
        }
    };
    let pid = child.id();
    *child_guard = Some(child);
    drop(child_guard);

    // A frozen sidecar may need to unpack its onefile payload before it can
    // apply SQLite migrations, so the local Alpha cold-start window is longer
    // than the source-Python development path.
    for _ in 0..300 {
        match probe_agent() {
            AgentProbe::Healthy => {
                append_supervisor_log(&format!(
                    "ForgeCAD supervisor healthy mode={mode_name} pid={pid}"
                ));
                return Ok(status_from_probe(
                    AgentProbe::Healthy,
                    true,
                    Some(pid),
                    &mode_name,
                ))
            }
            AgentProbe::WrongService(reason) => {
                state.shutdown_managed();
                return Err(format!(
                    "Agent service started but health probe returned a non-Wushen service: {reason}"
                ));
            }
            AgentProbe::Offline => thread::sleep(Duration::from_millis(100)),
        }
    }

    state.shutdown_managed();
    Err(
        "Agent service did not become healthy on http://127.0.0.1:8000/api/health within 30s"
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
    status_from_probe(probe_agent(), false, None, &mode)
}

fn activate_provider_config(
    mut metadata: ProviderConfigMetadata,
    state: State<'_, AgentProcessState>,
) -> Result<ProviderConfigMetadata, String> {
    let verified = read_provider_metadata()
        .map_err(|_| "Provider metadata could not be read after saving.".to_string())?;
    let secret_available = read_provider_secret()
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false);
    if verified.base_url != metadata.base_url || verified.model != metadata.model {
        metadata.metadata_status = "invalid".to_string();
        metadata.failure_code = Some("PROVIDER_METADATA_INVALID".to_string());
        return Ok(metadata);
    }
    metadata.metadata_status = "valid".to_string();
    metadata.secret_status = if secret_available { "available" } else { "missing" }.to_string();
    if metadata.configured && !secret_available {
        metadata.failure_code = Some("PROVIDER_SECRET_MISSING".to_string());
        return Ok(metadata);
    }

    state.shutdown_managed();
    match probe_agent() {
        AgentProbe::Healthy => {
            metadata.supervisor_status = "restart_failed".to_string();
            metadata.capability_status = probe_agent_provider_capability()
                .unwrap_or_else(|_| "unavailable".to_string());
            metadata.failure_code = Some("PROVIDER_SUPERVISOR_UNMANAGED".to_string());
            return Ok(metadata);
        }
        AgentProbe::WrongService(_) => {
            metadata.supervisor_status = "restart_failed".to_string();
            metadata.capability_status = "mismatch".to_string();
            metadata.failure_code = Some("PROVIDER_SUPERVISOR_WRONG_SERVICE".to_string());
            return Ok(metadata);
        }
        AgentProbe::Offline => {}
    }

    match start_agent_service(state) {
        Ok(status) if status.running => {
            metadata.supervisor_status = "running".to_string();
        }
        Ok(_) => {
            metadata.supervisor_status = "restart_failed".to_string();
            metadata.failure_code = Some("PROVIDER_SUPERVISOR_RESTART_FAILED".to_string());
            return Ok(metadata);
        }
        Err(_) => {
            metadata.supervisor_status = "restart_failed".to_string();
            metadata.failure_code = Some("PROVIDER_SUPERVISOR_RESTART_FAILED".to_string());
            return Ok(metadata);
        }
    }
    metadata.capability_status = probe_agent_provider_capability()
        .unwrap_or_else(|_| "unavailable".to_string());
    let expected = if metadata.configured { "ready" } else { "offline" };
    if metadata.capability_status != expected {
        metadata.failure_code = Some("PROVIDER_CAPABILITY_MISMATCH".to_string());
    } else {
        metadata.failure_code = None;
    }
    Ok(metadata)
}

fn probe_agent_provider_capability() -> Result<String, String> {
    let addr = SocketAddr::from(([127, 0, 0, 1], AGENT_PORT));
    let mut stream = TcpStream::connect_timeout(&addr, Duration::from_millis(300))
        .map_err(|_| "Agent capability endpoint is unavailable.".to_string())?;
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));
    let request = format!(
        "GET /api/v1/agent/provider HTTP/1.1\r\nHost: {AGENT_HOST}:{AGENT_PORT}\r\nConnection: close\r\n\r\n"
    );
    stream
        .write_all(request.as_bytes())
        .map_err(|_| "Agent capability request failed.".to_string())?;
    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .map_err(|_| "Agent capability response failed.".to_string())?;
    if !response.starts_with("HTTP/1.1 200") && !response.starts_with("HTTP/1.0 200") {
        return Err("Agent capability endpoint did not return HTTP 200.".to_string());
    }
    let body = response
        .split_once("\r\n\r\n")
        .map(|(_, body)| body)
        .unwrap_or_default();
    let value: serde_json::Value = serde_json::from_str(body)
        .map_err(|_| "Agent capability response was not valid JSON.".to_string())?;
    value
        .get("capability_status")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| "Agent capability response omitted capability_status.".to_string())
}

fn main() {
    tauri::Builder::default()
        .manage(AgentProcessState {
            child: Mutex::new(None),
            mode: Mutex::new(AGENT_MODE_LOCAL.to_string()),
        })
        .setup(|app| {
            // A packaged release has no repository or Python dependency. Start
            // its bundled sidecar before the WebView is ready. A background
            // `State` access is not reliable across every macOS launch path;
            // this synchronous, idempotent startup makes cold first launch
            // deterministic. The WebView call remains a recovery/status check.
            let state = app.state::<AgentProcessState>();
            if let Err(error) = start_agent_service(state) {
                eprintln!("ForgeCAD Agent startup failed: {error}");
                append_supervisor_log(&format!("ForgeCAD supervisor startup failed: {error}"));
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
            clear_provider_config
        ])
        .on_window_event(|window, event| {
            if matches!(event, tauri::WindowEvent::CloseRequested { .. }) {
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

fn start_local_python_sidecar(repo_root: &Path) -> Result<Child, String> {
    let log_path = repo_root.join(".wushen-agent.log");
    if let Some(parent) = log_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create Agent log directory: {error}"))?;
    }
    let mut command = Command::new(&agent_python(repo_root));
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
        .env("WUSHEN_LIBRARY_ROOT", local_library_root(repo_root))
        .env("WUSHEN_MIGRATIONS_DIR", repo_root.join("migrations"))
        .env("PYTHONUNBUFFERED", "1")
        .stdout(Stdio::from(log_file))
        .stderr(Stdio::from(stderr_log));

    apply_local_asset_pack(&mut command);
    apply_provider_config(&mut command);

    command.spawn().map_err(|error| {
        format!(
            "failed to start local-agent service with {}: {error}",
            agent_python(repo_root).display()
        )
    })
}

fn start_packaged_sidecar() -> Result<Child, String> {
    let sidecar = sidecar_binary_path()?;
    let library_root = packaged_library_root();
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
    #[cfg(unix)]
    command.process_group(0);
    command
        .arg("agent")
        .arg("serve")
        .current_dir(sidecar.parent().unwrap_or_else(|| Path::new(".")))
        .env("WUSHEN_LIBRARY_ROOT", library_root)
        .env_remove("WUSHEN_AGENT_PYTHON")
        .env_remove("WUSHEN_REPO_ROOT")
        .env_remove("WUSHEN_MIGRATIONS_DIR")
        .stdout(Stdio::from(log_file))
        .stderr(Stdio::from(stderr_log));
    apply_provider_config(&mut command);
    command.spawn().map_err(|error| {
        format!(
            "failed to start packaged-sidecar with {}: {error}",
            sidecar.display()
        )
    })
}

fn apply_provider_config(command: &mut Command) {
    // Local Alpha verification must prove the bundled offline path without
    // reading a Keychain record or sending a Provider request. This opt-out is
    // intentionally explicit; normal packaged launches still use Keychain.
    if env::var("FORGECAD_DISABLE_PROVIDER_CONFIG").as_deref() == Ok("1") {
        return;
    }
    let Ok(metadata) = read_provider_metadata() else {
        return;
    };
    let Ok(api_key) = read_provider_secret() else {
        return;
    };
    if !metadata.configured || api_key.trim().is_empty() {
        return;
    }
    command
        .env("FORGECAD_AGENT_PROVIDER", "openai_compatible")
        .env("FORGECAD_AGENT_BASE_URL", metadata.base_url)
        .env("FORGECAD_AGENT_MODEL", metadata.model)
        .env("FORGECAD_AGENT_API_KEY", api_key);
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

fn probe_agent() -> AgentProbe {
    let addr = SocketAddr::from(([127, 0, 0, 1], AGENT_PORT));
    let mut stream = match TcpStream::connect_timeout(&addr, Duration::from_millis(200)) {
        Ok(stream) => stream,
        Err(_) => return AgentProbe::Offline,
    };
    let _ = stream.set_read_timeout(Some(Duration::from_millis(500)));
    let _ = stream.set_write_timeout(Some(Duration::from_millis(500)));

    let request = format!(
        "GET /api/health HTTP/1.1\r\nHost: {AGENT_HOST}:{AGENT_PORT}\r\nConnection: close\r\n\r\n"
    );
    if let Err(error) = stream.write_all(request.as_bytes()) {
        return AgentProbe::WrongService(format!("health request failed: {error}"));
    }

    let mut response = String::new();
    if let Err(error) = stream.read_to_string(&mut response) {
        return AgentProbe::WrongService(format!("health response failed: {error}"));
    }
    if !response.starts_with("HTTP/1.1 200") && !response.starts_with("HTTP/1.0 200") {
        return AgentProbe::WrongService("health endpoint did not return HTTP 200".to_string());
    }

    let body = response
        .split_once("\r\n\r\n")
        .map(|(_, body)| body)
        .unwrap_or_default();
    match serde_json::from_str::<serde_json::Value>(body) {
        Ok(value)
            if value.get("status").and_then(serde_json::Value::as_str) == Some("ok")
                && value.get("service").and_then(serde_json::Value::as_str)
                    == Some("wushen-agent") =>
        {
            AgentProbe::Healthy
        }
        Ok(value) => AgentProbe::WrongService(format!("unexpected health payload: {value}")),
        Err(error) => AgentProbe::WrongService(format!("invalid health JSON: {error}")),
    }
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

fn apply_local_asset_pack(command: &mut Command) {
    // Local asset Pack discovery is non-sensitive and keeps the first-run
    // workbench populated even when macOS LaunchServices drops shell exports.
    // Provider credentials intentionally remain an explicit user opt-in.
    if let Some(pack) = local_formal_module_pack() {
        command.env("FORGECAD_BUNDLED_MODULE_PACK", pack);
    }
}

fn local_formal_module_pack() -> Option<PathBuf> {
    let explicit = env::var_os("FORGECAD_BUNDLED_MODULE_PACK").map(PathBuf::from);
    let default_paths = env::var_os("HOME").map(|home| {
        let root = PathBuf::from(home).join("Library/Caches/ForgeCAD/Formalization");
        vec![
            root.join("current/final-pack"),
            root.join("weapon-concept-v1-final-art-intake-20260711/final-pack"),
        ]
    });
    explicit
        .into_iter()
        .chain(default_paths.into_iter().flatten())
        .find(|candidate| candidate.join("pack.json").is_file())
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

fn provider_storage_name() -> &'static str {
    if cfg!(target_os = "macos") {
        "macos-keychain"
    } else {
        "secret-file-required"
    }
}

fn provider_metadata_path() -> PathBuf {
    let base = if cfg!(target_os = "macos") {
        env::var_os("HOME")
            .map(PathBuf::from)
            .map(|path| path.join("Library").join("Application Support"))
    } else if cfg!(target_os = "windows") {
        env::var_os("APPDATA").map(PathBuf::from)
    } else {
        env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| env::var_os("HOME").map(|path| PathBuf::from(path).join(".config")))
    };
    base.unwrap_or_else(|| PathBuf::from("."))
        .join("ForgeCAD")
        .join("provider.json")
}

fn read_provider_metadata() -> Result<ProviderConfigMetadata, String> {
    let payload =
        fs::read_to_string(provider_metadata_path()).map_err(|error| error.to_string())?;
    serde_json::from_str(&payload).map_err(|error| format!("invalid provider metadata: {error}"))
}

fn write_provider_metadata(metadata: &ProviderConfigMetadata) -> Result<(), String> {
    let path = provider_metadata_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let payload = serde_json::to_vec_pretty(metadata).map_err(|error| error.to_string())?;
    #[cfg(unix)]
    let options = {
        let mut value = OpenOptions::new();
        value.create(true).write(true).truncate(true).mode(0o600);
        value
    };
    #[cfg(not(unix))]
    let options = {
        let mut value = OpenOptions::new();
        value.create(true).write(true).truncate(true);
        value
    };
    let mut file = options.open(path).map_err(|error| error.to_string())?;
    file.write_all(&payload).map_err(|error| error.to_string())
}

fn read_provider_secret() -> Result<String, String> {
    if !cfg!(target_os = "macos") {
        return Err("system keychain is unavailable on this target".to_string());
    }
    let output = Command::new("/usr/bin/security")
        .args([
            "find-generic-password",
            "-a",
            KEYCHAIN_ACCOUNT,
            "-s",
            KEYCHAIN_SERVICE,
            "-w",
        ])
        .output()
        .map_err(|error| error.to_string())?;
    if !output.status.success() {
        return Err("provider key is not configured".to_string());
    }
    String::from_utf8(output.stdout)
        .map(|value| value.trim().to_string())
        .map_err(|error| error.to_string())
}

fn write_provider_secret(secret: &str) -> Result<(), String> {
    if !cfg!(target_os = "macos") {
        return Err("当前桌面仅支持 macOS Keychain 配置；其他平台请使用 secret file。".to_string());
    }
    let output = Command::new("/usr/bin/security")
        .args([
            "add-generic-password",
            "-U",
            "-a",
            KEYCHAIN_ACCOUNT,
            "-s",
            KEYCHAIN_SERVICE,
            "-w",
            secret,
        ])
        .output()
        .map_err(|error| error.to_string())?;
    if output.status.success() {
        Ok(())
    } else {
        Err("无法写入 macOS Keychain。".to_string())
    }
}

fn clear_provider_secret() -> Result<(), String> {
    if !cfg!(target_os = "macos") {
        return Ok(());
    }
    let output = Command::new("/usr/bin/security")
        .args([
            "delete-generic-password",
            "-a",
            KEYCHAIN_ACCOUNT,
            "-s",
            KEYCHAIN_SERVICE,
        ])
        .output()
        .map_err(|error| error.to_string())?;
    if output.status.success() || output.status.code() == Some(44) {
        Ok(())
    } else {
        Err("无法清除 macOS Keychain 配置。".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::{agent_health_url, validate_provider_config_input, ProviderConfigMetadata};

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
        assert_eq!(result.2, "secret");
    }

    #[test]
    fn provider_input_rejects_invalid_url() {
        let error = validate_provider_config_input("api.example.test", "model", "key")
            .expect_err("invalid URL must be rejected");
        assert!(error.contains("http(s)"));
    }

    #[test]
    fn provider_input_rejects_empty_or_oversized_fields() {
        assert!(validate_provider_config_input("https://example.test", "", "key").is_err());
        assert!(validate_provider_config_input("https://example.test", &"m".repeat(161), "key").is_err());
        assert!(validate_provider_config_input("https://example.test", "model", "").is_err());
        assert!(validate_provider_config_input("https://example.test", "model", &"k".repeat(4097)).is_err());
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
    fn health_url_is_stable_and_does_not_drop_base_path() {
        assert_eq!(agent_health_url("http://127.0.0.1:8000"), "http://127.0.0.1:8000/api/health");
        assert_eq!(agent_health_url("http://127.0.0.1:8000/agent"), "http://127.0.0.1:8000/agent/api/health");
    }
}
