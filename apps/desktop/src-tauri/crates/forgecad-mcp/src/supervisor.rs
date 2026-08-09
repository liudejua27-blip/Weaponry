//! Small Runtime child supervisor used by the single-user MVP.
//!
//! This is intentionally not a service broker. Every MCP adapter keeps a
//! passive supervisor, while an OS launcher lock elects at most one adapter to
//! remove a stale handoff and spawn Runtime. Runtime still owns the database
//! writer lock. Once Runtime is reachable, all adapters use the shared dynamic
//! handoff and the launcher lock is released for later crash recovery.

use forgecad_runtime::{sha256_hex, LocalIpcClient, LocalIpcEndpoint};
use std::env;
use std::fs::{self, File, OpenOptions};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const RUNTIME_DATA_DIR_ENV: &str = "FORGECAD_RUNTIME_DATA_DIR";
const RUNTIME_COMMAND_ENV: &str = "FORGECAD_RUNTIME_COMMAND";
const RESTART_BACKOFF: Duration = Duration::from_millis(100);
const MAX_RESTARTS: u8 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Starting,
    Ready,
    Restarting,
    Degraded,
    Busy,
}

impl State {
    fn as_str(self) -> &'static str {
        match self {
            Self::Starting => "Starting",
            Self::Ready => "Ready",
            Self::Restarting => "Restarting",
            Self::Degraded => "Degraded",
            Self::Busy => "Busy",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReadyProbe {
    Authenticated,
    ListenerReachable,
    Unavailable,
}

struct LauncherLock {
    file: File,
    held: bool,
}

impl LauncherLock {
    fn open(path: &Path) -> Result<Self, String> {
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let file = options
            .open(path)
            .map_err(|_| "Runtime launcher lock initialization failed".to_owned())?;
        restrict_file(path)?;
        Ok(Self { file, held: false })
    }

    fn try_acquire(&mut self) -> Result<bool, String> {
        if self.held {
            return Ok(true);
        }
        #[cfg(unix)]
        {
            use std::os::fd::AsRawFd;
            let result =
                unsafe { libc::flock(self.file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
            if result == 0 {
                self.held = true;
                return Ok(true);
            }
            let error = std::io::Error::last_os_error();
            if matches!(error.raw_os_error(), Some(code) if code == libc::EWOULDBLOCK || code == libc::EAGAIN)
            {
                return Ok(false);
            }
            return Err("Runtime launcher lock acquisition failed".to_owned());
        }
        #[cfg(not(unix))]
        {
            self.held = true;
            Ok(true)
        }
    }

    fn release(&mut self) {
        if !self.held {
            return;
        }
        #[cfg(unix)]
        {
            use std::os::fd::AsRawFd;
            let _ = unsafe { libc::flock(self.file.as_raw_fd(), libc::LOCK_UN) };
        }
        self.held = false;
    }

    fn is_held(&self) -> bool {
        self.held
    }
}

impl Drop for LauncherLock {
    fn drop(&mut self) {
        self.release();
    }
}

pub(crate) struct MvpSupervisor {
    command: PathBuf,
    data_root: PathBuf,
    endpoint_dir: PathBuf,
    ready_file: PathBuf,
    status_file: PathBuf,
    launcher_lock: LauncherLock,
    child: Option<Child>,
    state: State,
    restart_count: u8,
    last_exit_code: Option<i32>,
    terminal_failure: bool,
    next_launch_at: Option<Instant>,
}

impl MvpSupervisor {
    pub(crate) fn new(command: PathBuf, data_root: PathBuf) -> Result<Self, String> {
        if command.as_os_str().is_empty() {
            return Err("Runtime command is empty".to_owned());
        }
        fs::create_dir_all(&data_root)
            .map_err(|_| "Runtime data directory initialization failed".to_owned())?;
        restrict_directory(&data_root)?;
        let endpoint_dir = stable_endpoint_dir(&data_root)?;
        let handoff_dir = data_root.join("ipc");
        fs::create_dir_all(&handoff_dir)
            .map_err(|_| "Runtime handoff directory initialization failed".to_owned())?;
        restrict_directory(&handoff_dir)?;
        restrict_directory(&endpoint_dir)?;
        let ready_file = handoff_dir.join("ready.json");
        let status_file = handoff_dir.join("status.json");
        let launcher_lock = LauncherLock::open(&handoff_dir.join("launcher.lock"))?;
        Ok(Self {
            command,
            data_root,
            endpoint_dir,
            ready_file,
            status_file,
            launcher_lock,
            child: None,
            state: State::Starting,
            restart_count: 0,
            last_exit_code: None,
            terminal_failure: false,
            next_launch_at: None,
        })
    }

    pub(crate) fn ready_file(&self) -> &Path {
        &self.ready_file
    }

    pub(crate) fn status_file(&self) -> &Path {
        &self.status_file
    }

    /// Starts only when this adapter wins the launcher lock and the handoff is
    /// proven stale. A live handoff leaves this supervisor passive.
    pub(crate) fn start(&mut self) {
        match probe_ready_handoff(&self.ready_file) {
            ReadyProbe::Authenticated => self.mark_ready(),
            ReadyProbe::ListenerReachable => self.state = State::Starting,
            ReadyProbe::Unavailable => self.try_launch_if_elected(),
        }
    }

    /// Poll without blocking MCP stdio on Runtime startup. A passive adapter
    /// may become the launcher after the prior Runtime endpoint disappears.
    pub(crate) fn poll(&mut self) {
        if let Some(child) = self.child.as_mut() {
            let status = match child.try_wait() {
                Ok(Some(status)) => Some(Ok(status)),
                Ok(None) => None,
                Err(_) => Some(Err(())),
            };
            if let Some(status) = status {
                self.child = None;
                match status {
                    Ok(status) => {
                        self.last_exit_code = status.code();
                        if matches!(
                            probe_ready_handoff(&self.ready_file),
                            ReadyProbe::Authenticated
                        ) {
                            self.mark_ready();
                            return;
                        }
                        if status.code() == Some(2) {
                            self.mark_busy_retryable();
                            return;
                        }
                        if self.restart_count < MAX_RESTARTS {
                            self.state = State::Restarting;
                            self.try_restart_if_elected();
                            return;
                        }
                        self.fail_and_release("RUNTIME_UNAVAILABLE");
                        return;
                    }
                    Err(()) => {
                        self.fail_and_release("RUNTIME_PROCESS_STATUS_FAILED");
                        return;
                    }
                }
            }
        }

        match probe_ready_handoff(&self.ready_file) {
            ReadyProbe::Authenticated => self.mark_ready(),
            ReadyProbe::ListenerReachable => {
                // A listener exists but is currently busy or rejecting this
                // handoff. Never delete or replace a reachable endpoint.
            }
            ReadyProbe::Unavailable if self.child.is_none() && !self.terminal_failure => {
                if self
                    .next_launch_at
                    .is_some_and(|deadline| Instant::now() < deadline)
                {
                    return;
                }
                self.next_launch_at = None;
                if self.state == State::Restarting {
                    self.try_restart_if_elected();
                } else {
                    self.try_launch_if_elected();
                }
            }
            ReadyProbe::Unavailable => {}
        }
    }

    fn try_launch_if_elected(&mut self) {
        let elected = match self.launcher_lock.try_acquire() {
            Ok(elected) => elected,
            Err(_) => {
                self.state = State::Degraded;
                self.terminal_failure = true;
                self.launcher_lock.release();
                return;
            }
        };
        if !elected {
            self.state = State::Starting;
            return;
        }

        // Recheck after election. Another launcher may have published a live
        // endpoint between the first probe and this lock acquisition.
        match probe_ready_handoff(&self.ready_file) {
            ReadyProbe::Authenticated => {
                self.mark_ready();
                return;
            }
            ReadyProbe::ListenerReachable => {
                self.state = State::Starting;
                self.launcher_lock.release();
                return;
            }
            ReadyProbe::Unavailable => {}
        }

        self.remove_stale_handoff();
        self.state = State::Starting;
        self.write_status(None);
        self.launch();
    }

    fn try_restart_if_elected(&mut self) {
        let elected = match self.launcher_lock.try_acquire() {
            Ok(elected) => elected,
            Err(_) => {
                self.state = State::Degraded;
                self.terminal_failure = true;
                self.launcher_lock.release();
                return;
            }
        };
        if !elected {
            self.state = State::Restarting;
            return;
        }

        match probe_ready_handoff(&self.ready_file) {
            ReadyProbe::Authenticated => {
                self.mark_ready();
                return;
            }
            ReadyProbe::ListenerReachable => {
                self.state = State::Restarting;
                self.launcher_lock.release();
                return;
            }
            ReadyProbe::Unavailable => {}
        }

        self.restart_count += 1;
        self.remove_stale_handoff();
        self.write_status(Some("RUNTIME_RESTARTING"));
        thread::sleep(RESTART_BACKOFF);
        self.launch();
    }

    fn mark_ready(&mut self) {
        self.state = State::Ready;
        self.terminal_failure = false;
        self.next_launch_at = None;
        if self.launcher_lock.is_held() {
            self.write_status(None);
            self.launcher_lock.release();
        }
    }

    fn mark_busy_retryable(&mut self) {
        self.state = State::Busy;
        self.terminal_failure = false;
        self.next_launch_at = Some(Instant::now() + RESTART_BACKOFF);
        let can_write =
            self.launcher_lock.is_held() || self.launcher_lock.try_acquire().unwrap_or(false);
        if can_write {
            match probe_ready_handoff(&self.ready_file) {
                ReadyProbe::Authenticated => {
                    self.mark_ready();
                    return;
                }
                ReadyProbe::ListenerReachable => {}
                ReadyProbe::Unavailable => self.write_status(Some("RUNTIME_BUSY")),
            }
        }
        self.launcher_lock.release();
    }

    fn remove_stale_handoff(&self) {
        if !self.launcher_lock.is_held() {
            return;
        }
        let _ = fs::remove_file(&self.ready_file);
    }

    fn launch(&mut self) {
        if !self.launcher_lock.is_held() {
            return;
        }
        let result = Command::new(&self.command)
            .args(["serve", "--database"])
            .arg(self.data_root.join("runtime.sqlite"))
            .args(["--cas-root"])
            .arg(self.data_root.join("cas"))
            .args(["--endpoint-dir"])
            .arg(&self.endpoint_dir)
            .args(["--ready-file"])
            .arg(&self.ready_file)
            .env_remove(RUNTIME_DATA_DIR_ENV)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
        match result {
            Ok(child) => {
                self.child = Some(child);
                self.next_launch_at = None;
                // Do not require another MCP request to release election. The
                // Runtime process lock remains the final single-writer guard;
                // losing Runtime launches cannot publish a ready handoff.
                self.launcher_lock.release();
            }
            Err(_) => {
                self.fail_and_release("RUNTIME_LAUNCH_FAILED");
            }
        }
    }

    fn fail_and_release(&mut self, code: &str) {
        self.state = State::Degraded;
        self.terminal_failure = true;
        self.next_launch_at = None;
        let can_write =
            self.launcher_lock.is_held() || self.launcher_lock.try_acquire().unwrap_or(false);
        if can_write {
            match probe_ready_handoff(&self.ready_file) {
                ReadyProbe::Authenticated => {
                    self.mark_ready();
                    return;
                }
                ReadyProbe::ListenerReachable => {
                    self.launcher_lock.release();
                    return;
                }
                ReadyProbe::Unavailable => self.write_status(Some(code)),
            }
        }
        self.launcher_lock.release();
    }

    fn write_status(&self, code: Option<&str>) {
        if !self.launcher_lock.is_held() {
            return;
        }
        let payload = serde_json::json!({
            "schema_version": "ForgeCADRuntimeSupervisorStatus@1",
            "state": self.state.as_str(),
            "retryable": !matches!(self.state, State::Ready),
            "restart_count": self.restart_count,
            "last_exit_code": self.last_exit_code,
            "code": code,
            "scope": "single-user MVP Runtime handoff"
        });
        let Ok(bytes) = serde_json::to_vec(&payload) else {
            return;
        };
        let temporary = self
            .status_file
            .with_extension(format!("{}.tmp", std::process::id()));
        if fs::write(&temporary, bytes).is_ok() {
            let _ = restrict_file(&temporary);
            let _ = fs::rename(&temporary, &self.status_file);
            let _ = restrict_file(&self.status_file);
        }
        let _ = fs::remove_file(temporary);
    }
}

impl Drop for MvpSupervisor {
    fn drop(&mut self) {
        let live = !matches!(
            probe_ready_handoff(&self.ready_file),
            ReadyProbe::Unavailable
        );
        if let Some(mut child) = self.child.take() {
            if live {
                // A ready Runtime is shared by every Desktop/CLI adapter. Do
                // not tie its lifetime to whichever MCP happened to launch it.
                drop(child);
            } else {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }
}

pub(crate) fn runtime_command() -> PathBuf {
    if let Some(value) = env::var_os(RUNTIME_COMMAND_ENV) {
        return PathBuf::from(value);
    }
    sibling_command("forgecad-runtime").unwrap_or_else(|| PathBuf::from("forgecad-runtime"))
}

pub(crate) fn runtime_data_root() -> Result<PathBuf, String> {
    if let Some(value) = env::var_os(RUNTIME_DATA_DIR_ENV) {
        let path = PathBuf::from(value);
        if path.as_os_str().is_empty() {
            return Err("Runtime data directory is empty".to_owned());
        }
        return Ok(path);
    }

    #[cfg(target_os = "macos")]
    {
        return Ok(PathBuf::from(
            env::var_os("HOME").ok_or_else(|| "HOME is unavailable".to_owned())?,
        )
        .join("Library")
        .join("Application Support")
        .join("ForgeCAD Runtime")
        .join("runtime-data"));
    }
    #[cfg(target_os = "windows")]
    {
        return Ok(PathBuf::from(
            env::var_os("LOCALAPPDATA").ok_or_else(|| "LOCALAPPDATA is unavailable".to_owned())?,
        )
        .join("ForgeCAD Runtime")
        .join("runtime-data"));
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        if let Some(value) = env::var_os("XDG_DATA_HOME") {
            return Ok(PathBuf::from(value)
                .join("forgecad-runtime")
                .join("runtime-data"));
        }
        Ok(
            PathBuf::from(env::var_os("HOME").ok_or_else(|| "HOME is unavailable".to_owned())?)
                .join(".local")
                .join("share")
                .join("forgecad-runtime")
                .join("runtime-data"),
        )
    }
}

fn sibling_command(name: &str) -> Option<PathBuf> {
    let executable = env::current_exe().ok()?;
    let parent = executable.parent()?;
    [
        parent.join(name),
        parent.join(format!("{name}{}", env::consts::EXE_SUFFIX)),
        parent.join("..").join("Resources").join(name),
        parent
            .join("..")
            .join("Resources")
            .join(format!("{name}{}", env::consts::EXE_SUFFIX)),
    ]
    .into_iter()
    .find(|path| path.is_file())
}

fn probe_ready_handoff(path: &Path) -> ReadyProbe {
    let Ok(bytes) = fs::read(path) else {
        return ReadyProbe::Unavailable;
    };
    if bytes.len() > 64 * 1024 {
        return ReadyProbe::Unavailable;
    }
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
        return ReadyProbe::Unavailable;
    };
    if value.get("status").and_then(serde_json::Value::as_str) != Some("ready") {
        return ReadyProbe::Unavailable;
    }
    let Some(socket) = value
        .get("socket_path")
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.is_empty())
    else {
        return ReadyProbe::Unavailable;
    };
    let Some(token) = value
        .get("token")
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.is_empty())
    else {
        return ReadyProbe::Unavailable;
    };
    let endpoint = LocalIpcEndpoint::from_parts(socket.to_owned(), token.to_owned());
    if LocalIpcClient::connect(&endpoint).is_ok() {
        ReadyProbe::Authenticated
    } else if endpoint.listener_reachable() {
        ReadyProbe::ListenerReachable
    } else {
        ReadyProbe::Unavailable
    }
}

fn stable_endpoint_dir(data_root: &Path) -> Result<PathBuf, String> {
    let digest = sha256_hex(data_root.to_string_lossy().as_bytes());
    let endpoint_dir = env::temp_dir().join(format!("fc-{}", &digest[..12]));
    fs::create_dir_all(&endpoint_dir)
        .map_err(|_| "Runtime IPC directory initialization failed".to_owned())?;
    Ok(endpoint_dir)
}

fn restrict_directory(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))
            .map_err(|_| "private directory permission setup failed".to_owned())?;
    }
    Ok(())
}

fn restrict_file(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))
            .map_err(|_| "private file permission setup failed".to_owned())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_root(label: &str) -> PathBuf {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let root = env::temp_dir().join(format!(
            "fc-supervisor-{label}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&root).expect("test root");
        root
    }

    #[cfg(unix)]
    #[test]
    fn launcher_lock_elects_exactly_one_adapter_and_can_handoff() {
        let root = test_root("lock");
        let path = root.join("launcher.lock");
        let mut first = LauncherLock::open(&path).expect("first lock");
        let mut second = LauncherLock::open(&path).expect("second lock");
        assert!(first.try_acquire().expect("first election"));
        assert!(!second.try_acquire().expect("second remains passive"));
        first.release();
        assert!(second.try_acquire().expect("second takes over"));
        drop(second);
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn stale_handoff_cleanup_and_terminal_launch_failure_are_recoverable() {
        let root = test_root("stale");
        let handoff = root.join("ipc");
        fs::create_dir_all(&handoff).expect("handoff");
        fs::write(
            handoff.join("ready.json"),
            br#"{"status":"ready","socket_path":"/missing/socket","token":"stale"}"#,
        )
        .expect("stale ready");
        fs::write(handoff.join("status.json"), br#"{"state":"Ready"}"#).expect("stale status");

        let missing = root.join("missing-runtime");
        let mut elected = MvpSupervisor::new(missing.clone(), root.clone()).expect("elected");
        let mut passive = MvpSupervisor::new(missing, root.clone()).expect("passive");
        elected.start();
        assert!(!handoff.join("ready.json").exists());
        assert!(elected.terminal_failure);
        assert!(!elected.launcher_lock.is_held());
        passive.start();
        assert!(passive.terminal_failure);
        assert!(!passive.launcher_lock.is_held());
        let status = fs::read_to_string(handoff.join("status.json")).expect("status output");
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&status).expect("status JSON")["code"],
            "RUNTIME_LAUNCH_FAILED"
        );

        drop(passive);
        drop(elected);
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn dropping_an_adapter_during_startup_terminates_its_unready_child() {
        let root = test_root("drop-starting");
        let mut supervisor =
            MvpSupervisor::new(PathBuf::from("/usr/bin/yes"), root.clone()).expect("supervisor");
        supervisor.start();
        let child_id = supervisor.child.as_ref().expect("startup child").id() as libc::pid_t;
        drop(supervisor);
        let result = unsafe { libc::kill(child_id, 0) };
        assert_eq!(result, -1, "unready child must not be detached");
        assert_eq!(
            std::io::Error::last_os_error().raw_os_error(),
            Some(libc::ESRCH)
        );
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn one_adapter_reacquires_election_and_restarts_only_once() {
        let root = test_root("single-restart");
        let mut supervisor =
            MvpSupervisor::new(PathBuf::from("/usr/bin/false"), root.clone()).expect("supervisor");
        supervisor.start();

        for _ in 0..100 {
            supervisor.poll();
            if supervisor.terminal_failure {
                break;
            }
            thread::sleep(Duration::from_millis(5));
        }

        assert_eq!(supervisor.restart_count, 1);
        assert_eq!(supervisor.state, State::Degraded);
        assert!(supervisor.terminal_failure);
        assert!(supervisor.child.is_none());
        assert!(!supervisor.launcher_lock.is_held());
        let status = fs::read_to_string(root.join("ipc/status.json")).expect("final status");
        let status: serde_json::Value = serde_json::from_str(&status).expect("status JSON");
        assert_eq!(status["code"], "RUNTIME_UNAVAILABLE");
        assert_eq!(status["restart_count"], 1);
        drop(supervisor);
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn busy_cold_start_loser_can_reenter_election_and_reach_ready() {
        let root = test_root("busy-retry");
        // Python exits with code 2 when the supervisor-supplied `serve`
        // script path does not exist, matching forgecad-runtime's writer-lock
        // busy exit without requiring a second Runtime binary in this unit.
        let mut supervisor = MvpSupervisor::new(PathBuf::from("/usr/bin/python3"), root.clone())
            .expect("supervisor");
        supervisor.start();
        for _ in 0..100 {
            supervisor.poll();
            if supervisor.state == State::Busy && supervisor.child.is_none() {
                break;
            }
            thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(supervisor.state, State::Busy);
        assert!(!supervisor.terminal_failure);
        assert!(supervisor.child.is_none());
        assert!(!supervisor.launcher_lock.is_held());
        let status = fs::read_to_string(root.join("ipc/status.json")).expect("busy status");
        let status: serde_json::Value = serde_json::from_str(&status).expect("status JSON");
        assert_eq!(status["state"], "Busy");
        assert_eq!(status["code"], "RUNTIME_BUSY");
        assert_eq!(status["retryable"], true);

        // Once the external writer is gone, the same adapter can win a later
        // election instead of remaining terminal forever.
        supervisor.command = PathBuf::from("/usr/bin/yes");
        thread::sleep(RESTART_BACKOFF + Duration::from_millis(20));
        supervisor.poll();
        assert!(supervisor.child.is_some(), "same adapter must relaunch");

        // A concurrently started winner can publish the shared handoff; the
        // prior loser follows it and becomes a passive Ready adapter.
        let endpoint = LocalIpcEndpoint::new(&supervisor.endpoint_dir).expect("shared endpoint");
        let runtime = std::sync::Arc::new(forgecad_runtime::Runtime::ephemeral().expect("runtime"));
        let server = runtime.ipc_server(&endpoint).expect("server");
        let runtime_for_thread = runtime.clone();
        let server_thread = thread::spawn(move || server.serve_forever(&runtime_for_thread));
        fs::write(
            root.join("ipc/ready.json"),
            serde_json::to_vec(&serde_json::json!({
                "status":"ready",
                "socket_path":endpoint.socket_path().to_string_lossy(),
                "token":endpoint.token()
            }))
            .expect("ready JSON"),
        )
        .expect("ready handoff");
        supervisor.poll();
        assert_eq!(supervisor.state, State::Ready);

        let mut shutdown = LocalIpcClient::connect(&endpoint).expect("shutdown client");
        shutdown
            .call("runtime_shutdown", serde_json::Value::Null)
            .expect("shutdown");
        drop(shutdown);
        assert!(server_thread.join().expect("server thread").is_ok());
        fs::remove_file(root.join("ipc/ready.json")).expect("ready cleanup");
        drop(supervisor);
        drop(runtime);
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn idle_launcher_does_not_block_passive_takeover_after_endpoint_disappears() {
        let root = test_root("idle-takeover");
        let mut idle_launcher =
            MvpSupervisor::new(PathBuf::from("/usr/bin/yes"), root.clone()).expect("launcher");
        idle_launcher.start();
        assert!(idle_launcher.child.is_some());
        assert!(
            !idle_launcher.launcher_lock.is_held(),
            "launch election must be released without waiting for MCP poll"
        );

        // Use the same bounded, stable endpoint directory supplied to the
        // Runtime child. macOS Unix-domain socket paths are limited to roughly
        // 100 bytes, while the descriptive test data root can be much longer.
        let endpoint = LocalIpcEndpoint::new(&idle_launcher.endpoint_dir).expect("shared endpoint");
        let runtime =
            std::sync::Arc::new(forgecad_runtime::Runtime::ephemeral().expect("shared runtime"));
        let server = runtime.ipc_server(&endpoint).expect("server");
        let runtime_for_thread = runtime.clone();
        let server_thread = thread::spawn(move || server.serve_forever(&runtime_for_thread));
        fs::write(
            root.join("ipc/ready.json"),
            serde_json::to_vec(&serde_json::json!({
                "status":"ready",
                "socket_path":endpoint.socket_path().to_string_lossy(),
                "token":endpoint.token()
            }))
            .expect("ready JSON"),
        )
        .expect("ready handoff");

        let mut passive =
            MvpSupervisor::new(PathBuf::from("/usr/bin/yes"), root.clone()).expect("passive");
        passive.start();
        assert_eq!(passive.state, State::Ready);
        assert!(passive.child.is_none());

        let mut shutdown = LocalIpcClient::connect(&endpoint).expect("shutdown client");
        shutdown
            .call("runtime_shutdown", serde_json::Value::Null)
            .expect("shutdown");
        drop(shutdown);
        assert!(server_thread.join().expect("server thread").is_ok());
        fs::remove_file(root.join("ipc/ready.json")).expect("ready guard cleanup");
        idle_launcher
            .child
            .as_mut()
            .expect("idle child")
            .kill()
            .expect("simulate Runtime crash");
        idle_launcher
            .child
            .as_mut()
            .expect("idle child")
            .wait()
            .expect("crashed child reaped");
        idle_launcher.child = None;

        passive.poll();
        assert!(passive.child.is_some(), "passive adapter must take over");
        assert!(!passive.launcher_lock.is_held());

        drop(passive);
        drop(idle_launcher);
        drop(runtime);
        let _ = fs::remove_dir_all(root);
    }
}
