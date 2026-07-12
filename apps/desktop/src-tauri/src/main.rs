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

impl AgentProcessState {
    fn shutdown_managed(&self) {
        if let Ok(mut child_guard) = self.child.lock() {
            if let Some(child) = child_guard.as_mut() {
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
            let pid = guard
                .as_mut()
                .and_then(|child| {
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
                return Ok(status_from_probe(AgentProbe::Healthy, true, pid, &mode_name));
            }
        }
    }

    match probe_agent() {
        AgentProbe::Healthy => {
            let mode_name = runtime_mode_name(runtime_mode());
            return Ok(status_from_probe(AgentProbe::Healthy, false, None, &mode_name));
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
    let repo_root = repo_root()?;
    let mode = runtime_mode();
    let mode_name = match mode {
        AgentMode::LocalDev => AGENT_MODE_LOCAL,
        AgentMode::PackagedSidecar => AGENT_MODE_PACKAGED,
    }
    .to_string();
    *mode_guard = mode_name.clone();

    *child_guard = None;
    let child = match mode {
        AgentMode::PackagedSidecar => start_packaged_sidecar(&repo_root)?,
        AgentMode::LocalDev => start_local_python_sidecar(&repo_root)?,
    };
    let pid = child.id();
    *child_guard = Some(child);
    drop(child_guard);

    // The first local startup can apply SQLite migrations and register the
    // bundled module pack, so allow a realistic cold-start window.
    for _ in 0..100 {
        match probe_agent() {
            AgentProbe::Healthy => return Ok(status_from_probe(AgentProbe::Healthy, true, Some(pid), &mode_name)),
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
    Err("Agent service did not become healthy on http://127.0.0.1:8000/api/health within 10s".to_string())
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

fn main() {
    tauri::Builder::default()
        .manage(AgentProcessState {
            child: Mutex::new(None),
            mode: Mutex::new(AGENT_MODE_LOCAL.to_string()),
        })
        .invoke_handler(tauri::generate_handler![
            agent_health_endpoint,
            agent_service_status,
            start_agent_service,
            stop_agent_service
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

    command.spawn().map_err(|error| {
        format!(
            "failed to start local-agent service with {}: {error}",
            agent_python(repo_root).display()
        )
    })
}

fn start_packaged_sidecar(repo_root: &Path) -> Result<Child, String> {
    let sidecar = sidecar_binary_path(repo_root)?;
    let library_root = packaged_library_root(repo_root);
    let log_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(sidecar_log_path())
        .map_err(|error| format!("failed to open sidecar log file: {error}"))?;
    let stderr_log = log_file
        .try_clone()
        .map_err(|error| format!("failed to clone sidecar log file: {error}"))?;

    Command::new(&sidecar)
        .arg("agent")
        .arg("serve")
        .current_dir(repo_root)
        .env("WUSHEN_LIBRARY_ROOT", library_root)
        .env("WUSHEN_MIGRATIONS_DIR", repo_root.join("migrations"))
        .env("WUSHEN_REPO_ROOT", repo_root)
        .env_remove("WUSHEN_AGENT_PYTHON")
        .stdout(Stdio::from(log_file))
        .stderr(Stdio::from(stderr_log))
        .spawn()
        .map_err(|error| format!("failed to start packaged-sidecar with {}: {error}", sidecar.display()))
}

fn sidecar_log_path() -> PathBuf {
    let base = env::var_os("LOCALAPPDATA")
        .or_else(|| env::var_os("APPDATA"))
        .or_else(|| env::var_os("HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("WushenForge").join("agent.log")
}

fn packaged_library_root(repo_root: &Path) -> PathBuf {
    if let Ok(value) = env::var("WUSHEN_LIBRARY_ROOT") {
        return PathBuf::from(value);
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
    repo_root.join("WushenForgeLibrary")
}

fn sidecar_binary_path(repo_root: &Path) -> Result<PathBuf, String> {
    if let Ok(override_path) = env::var("WUSHEN_AGENT_SIDE_CAR") {
        let candidate = PathBuf::from(override_path);
        if candidate.exists() {
            return Ok(candidate);
        }
        return Err(format!("WUSHEN_AGENT_SIDE_CAR does not exist: {}", candidate.display()));
    }

    let binaries_dir = repo_root.join("apps/desktop/src-tauri/binaries");
    let candidates = candidate_sidecar_basenames();
    for name in candidates {
        let candidate = binaries_dir.join(name);
        if candidate.exists() {
            return Ok(candidate);
        }
    }
    Err("packaged sidecar binary not found. Expected binaries/wushen-agent-<target> in apps/desktop/src-tauri/binaries".to_string())
}

fn candidate_sidecar_basenames() -> &'static [&'static str] {
    if cfg!(target_os = "windows") {
        &["wushen-agent-x86_64-pc-windows-msvc.exe"]
    } else if cfg!(target_os = "macos") {
        &["wushen-agent-aarch64-apple-darwin", "wushen-agent-x86_64-apple-darwin"]
    } else {
        &["wushen-agent-x86_64-unknown-linux-gnu"]
    }
}

fn runtime_mode() -> AgentMode {
    match env::var("WUSHEN_AGENT_RUNTIME_MODE")
        .unwrap_or_else(|_| AGENT_MODE_LOCAL.to_string())
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
            if value
                .get("status")
                .and_then(serde_json::Value::as_str)
                == Some("ok")
                && value
                    .get("service")
                    .and_then(serde_json::Value::as_str)
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
    let default_path = env::var_os("HOME").map(|home| {
        PathBuf::from(home).join(
            "Library/Caches/ForgeCAD/Formalization/weapon-concept-v1-final-art-intake-20260711/final-pack",
        )
    });
    explicit
        .or(default_path)
        .filter(|candidate| candidate.join("pack.json").is_file())
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
