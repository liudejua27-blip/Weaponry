use forgecad_contracts::CandidateConfirmRequest;
use forgecad_runtime::{build_cohort_sha256, LocalIpcEndpoint, Runtime, RuntimeError};
use serde::Serialize;
use serde_json::{json, Value};
use std::env;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

const USAGE: &str = "usage: forgecad-runtime serve --database <path> --cas-root <path> --endpoint-dir <path> --ready-file <path> [--diagnostic-fixture]";

#[derive(Debug)]
struct Options {
    database: PathBuf,
    cas_root: PathBuf,
    endpoint_dir: PathBuf,
    ready_file: PathBuf,
    diagnostic_fixture: bool,
}

#[derive(Debug, Serialize)]
struct ReadyHandoff {
    schema_version: &'static str,
    status: &'static str,
    socket_path: String,
    token: String,
    runtime_capabilities: Value,
    diagnostic_fixture: Option<Value>,
}

struct ReadyFileGuard {
    path: PathBuf,
}

impl Drop for ReadyFileGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn main() {
    if env::args().skip(1).eq(["--build-identity"]) {
        println!(
            "{}",
            serde_json::to_string(&json!({
                "schema_version": "ForgeCADDevBuildIdentity@1",
                "component": "forgecad-runtime",
                "build_cohort_sha256": build_cohort_sha256()
            }))
            .expect("build identity serializes")
        );
        return;
    }
    if let Err(code) = run() {
        eprintln!("forgecad-runtime: {code}");
        std::process::exit(if code.starts_with("RUNTIME_BUSY") {
            2
        } else {
            1
        });
    }
}

fn run() -> Result<(), String> {
    let options = parse_args(env::args().skip(1))?;
    let runtime =
        Runtime::open_with_cas(&options.database, &options.cas_root).map_err(|error| {
            if let RuntimeError::ProcessLock(message) = error {
                message
            } else {
                "runtime initialization failed".to_owned()
            }
        })?;

    let fixture = if options.diagnostic_fixture {
        Some(create_diagnostic_fixture(&runtime)?)
    } else {
        None
    };

    fs::create_dir_all(&options.endpoint_dir)
        .map_err(|_| "endpoint directory initialization failed".to_owned())?;
    restrict_directory(&options.endpoint_dir)?;
    let endpoint = LocalIpcEndpoint::new(&options.endpoint_dir)
        .map_err(|_| "IPC endpoint initialization failed".to_owned())?;
    let server = runtime
        .ipc_server(&endpoint)
        .map_err(|_| "IPC server initialization failed".to_owned())?;
    write_ready_handoff(
        &options.ready_file,
        &ReadyHandoff {
            schema_version: "ForgeCADRuntimeLauncherReady@1",
            status: "ready",
            socket_path: endpoint.socket_path().to_string_lossy().into_owned(),
            token: endpoint.token().to_owned(),
            runtime_capabilities: serde_json::to_value(runtime.capabilities())
                .map_err(|_| "runtime capability serialization failed".to_owned())?,
            diagnostic_fixture: fixture,
        },
    )?;
    let _ready_file_guard = ReadyFileGuard {
        path: options.ready_file,
    };

    eprintln!("forgecad-runtime: ready");
    server
        .serve_forever(&runtime)
        .map_err(|_| "authenticated IPC server stopped".to_owned())
}

fn parse_args<I>(mut args: I) -> Result<Options, String>
where
    I: Iterator<Item = String>,
{
    if args.next().as_deref() != Some("serve") {
        return Err(USAGE.to_owned());
    }

    let mut database = None;
    let mut cas_root = None;
    let mut endpoint_dir = None;
    let mut ready_file = None;
    let mut diagnostic_fixture = false;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--database" => database = Some(next_value(&mut args, "--database")?),
            "--cas-root" => cas_root = Some(next_value(&mut args, "--cas-root")?),
            "--endpoint-dir" => endpoint_dir = Some(next_value(&mut args, "--endpoint-dir")?),
            "--ready-file" => ready_file = Some(next_value(&mut args, "--ready-file")?),
            "--diagnostic-fixture" => diagnostic_fixture = true,
            _ => return Err(USAGE.to_owned()),
        }
    }

    match (database, cas_root, endpoint_dir, ready_file) {
        (Some(database), Some(cas_root), Some(endpoint_dir), Some(ready_file)) => Ok(Options {
            database: PathBuf::from(database),
            cas_root: PathBuf::from(cas_root),
            endpoint_dir: PathBuf::from(endpoint_dir),
            ready_file: PathBuf::from(ready_file),
            diagnostic_fixture,
        }),
        _ => Err(USAGE.to_owned()),
    }
}

fn next_value<I>(args: &mut I, flag: &str) -> Result<String, String>
where
    I: Iterator<Item = String>,
{
    args.next()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("{USAGE}: missing value for {flag}"))
}

fn write_ready_handoff(path: &Path, handoff: &ReadyHandoff) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|_| "ready-file directory initialization failed".to_owned())?;
        restrict_directory(parent)?;
    }
    let bytes = serde_json::to_vec_pretty(handoff)
        .map_err(|_| "ready-file serialization failed".to_owned())?;
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    restrict_file(&mut options);
    let mut file = options
        .open(path)
        .map_err(|_| "ready-file creation failed".to_owned())?;
    file.write_all(&bytes)
        .and_then(|_| file.write_all(b"\n"))
        .and_then(|_| file.sync_all())
        .map_err(|_| "ready-file write failed".to_owned())
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

fn restrict_file(options: &mut OpenOptions) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
}

fn create_diagnostic_fixture(runtime: &Runtime) -> Result<Value, String> {
    let project = runtime
        .create_project("MCP004 diagnostic fixture", json!({"scope":"diagnostic"}))
        .map_err(|_| "diagnostic project creation failed".to_owned())?;
    // A host can be restarted against the same diagnostic data directory. All
    // approval and idempotency records therefore need a fixture-local scope;
    // fixed values make the second startup fail before MCP initialize.
    let fixture_scope = project.project_id.clone();
    let source_object_id = format!("{fixture_scope}-source-object");
    let source_quality_id = format!("{fixture_scope}-source-quality");
    let source_approval_id = format!("{fixture_scope}-source-approval");
    let source_idempotency_key = format!("{fixture_scope}-source-confirm");
    let fixture_session_id = format!("{fixture_scope}-session");
    let pending_object_id = format!("{fixture_scope}-pending-object");
    let pending_quality_id = format!("{fixture_scope}-pending-quality");

    let source_object = runtime
        .put_object(
            b"ForgeCAD MCP004 diagnostic source object",
            None,
            "application/octet-stream",
            "diagnostic-object",
        )
        .map_err(|_| "diagnostic source CAS write failed".to_owned())?;
    let source_prepared = runtime
        .prepare_candidate(
            &project.project_id,
            None,
            &source_object_id,
            &source_object.record.sha256,
            json!({"typed":"diagnostic","fixture":"source"}),
        )
        .map_err(|_| "diagnostic source candidate prepare failed".to_owned())?;
    let source_quality = runtime
        .mark_candidate_quality(
            &source_prepared.candidate.candidate_id,
            &source_quality_id,
            true,
        )
        .map_err(|_| "diagnostic source quality seam failed".to_owned())?;
    let source_confirm = runtime
        .confirm_candidate(&CandidateConfirmRequest {
            project_id: project.project_id.clone(),
            candidate_id: source_quality.candidate_id.clone(),
            base_version_id: None,
            prepared_object_id: source_object_id.clone(),
            prepared_object_sha256: source_object.record.sha256.clone(),
            quality_report_id: source_quality_id.clone(),
            approval_receipt_id: source_approval_id,
            approval_summary: "Create the MCP004 diagnostic source version".to_owned(),
            approval_session_id: fixture_session_id.clone(),
            approval_expires_at: "9999999999".to_owned(),
            idempotency_key: source_idempotency_key,
        })
        .map_err(|_| "diagnostic source confirm failed".to_owned())?;

    let pending_object = runtime
        .put_object(
            b"ForgeCAD MCP004 diagnostic pending object",
            None,
            "application/octet-stream",
            "diagnostic-object",
        )
        .map_err(|_| "diagnostic pending CAS write failed".to_owned())?;
    let pending_prepared = runtime
        .prepare_candidate(
            &project.project_id,
            Some(&source_confirm.version_id),
            &pending_object_id,
            &pending_object.record.sha256,
            json!({"typed":"diagnostic","fixture":"pending"}),
        )
        .map_err(|_| "diagnostic pending candidate prepare failed".to_owned())?;
    let pending_quality = runtime
        .mark_candidate_quality(
            &pending_prepared.candidate.candidate_id,
            &pending_quality_id,
            true,
        )
        .map_err(|_| "diagnostic pending quality seam failed".to_owned())?;

    Ok(json!({
        "project_id": project.project_id,
        "source_version_id": source_confirm.version_id,
        "pending_candidate_id": pending_quality.candidate_id,
        "pending_prepared_object_id": pending_object_id,
        "pending_prepared_object_sha256": pending_object.record.sha256,
        "pending_quality_report_id": pending_quality_id,
        "fixture_quality_seam": "Runtime internal diagnostic setup only; not an MCP tool"
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagnostic_fixture_can_be_recreated_in_one_runtime() {
        let runtime = Runtime::ephemeral().expect("runtime");
        let first = create_diagnostic_fixture(&runtime).expect("first fixture");
        let second = create_diagnostic_fixture(&runtime).expect("second fixture");

        assert_ne!(first["project_id"], second["project_id"]);
        assert_ne!(
            first["pending_quality_report_id"],
            second["pending_quality_report_id"]
        );
    }
}
