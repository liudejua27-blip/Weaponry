use forgecad_runtime::{LocalIpcClient, LocalIpcEndpoint};
use serde_json::{json, Value};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

const DATA_DIR_ENV: &str = "FORGECAD_RUNTIME_DATA_DIR";
const READ_MODEL_SCHEMA: &str = "ForgeCADViewerReadModel@1";

/// Read the optional Viewer projection without starting Runtime or opening
/// SQLite/CAS. The Viewer is deliberately a read-only IPC client.
pub fn read_model() -> Value {
    match runtime_data_root().and_then(|root| read_model_from_root(&root)) {
        Ok(model) => model,
        Err(code) => unavailable(code),
    }
}

/// Read one bounded GLB payload for the Tauri Viewer canvas. The Viewer is a
/// read-only client; all bytes still come from Runtime CAS over authenticated
/// local IPC.
pub fn read_artifact_bytes(artifact_id: &str, candidate_id: &str) -> Value {
    match runtime_data_root()
        .and_then(|root| read_artifact_bytes_from_root(&root, artifact_id, candidate_id))
    {
        Ok(value) => value,
        Err(code) => unavailable(code),
    }
}

fn read_artifact_bytes_from_root(
    root: &Path,
    artifact_id: &str,
    candidate_id: &str,
) -> Result<Value, String> {
    let ready_path = root.join("ipc").join("ready.json");
    let handoff = read_bounded_json(&ready_path)?;
    if handoff.get("status").and_then(Value::as_str) != Some("ready") {
        return Err("RUNTIME_UNAVAILABLE".to_owned());
    }
    let socket = handoff
        .get("socket_path")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "RUNTIME_UNAVAILABLE".to_owned())?;
    let token = handoff
        .get("token")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "RUNTIME_UNAVAILABLE".to_owned())?;
    let mut client = LocalIpcClient::connect(&LocalIpcEndpoint::from_parts(socket, token))
        .map_err(|_| "RUNTIME_UNAVAILABLE".to_owned())?;
    client
        .call(
            "artifact_bytes_get",
            json!({"artifact_id":artifact_id,"candidate_id":candidate_id}),
        )
        .map_err(|_| "RUNTIME_REQUEST_FAILED".to_owned())
}

fn read_model_from_root(root: &Path) -> Result<Value, String> {
    let ready_path = root.join("ipc").join("ready.json");
    let handoff = read_bounded_json(&ready_path)?;
    if handoff.get("status").and_then(Value::as_str) != Some("ready") {
        return Err("RUNTIME_UNAVAILABLE".to_owned());
    }
    let socket = handoff
        .get("socket_path")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "RUNTIME_UNAVAILABLE".to_owned())?;
    let token = handoff
        .get("token")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "RUNTIME_UNAVAILABLE".to_owned())?;
    let mut client = LocalIpcClient::connect(&LocalIpcEndpoint::from_parts(socket, token))
        .map_err(|_| "RUNTIME_UNAVAILABLE".to_owned())?;
    let projects = client
        .call("project_list", json!({}))
        .map_err(|_| "RUNTIME_REQUEST_FAILED".to_owned())?;
    let project_values = projects.as_array().cloned().unwrap_or_default();
    let mut projections = Vec::with_capacity(project_values.len());
    for project in project_values {
        let Some(project_id) = project.get("project_id").and_then(Value::as_str) else {
            continue;
        };
        let record = client
            .call("project_get", json!({"project_id": project_id}))
            .map_err(|_| "RUNTIME_REQUEST_FAILED".to_owned())?;
        let versions = client
            .call("version_list", json!({"project_id": project_id}))
            .map_err(|_| "RUNTIME_REQUEST_FAILED".to_owned())?;
        let candidates = client
            .call("candidate_list", json!({"project_id": project_id}))
            .map_err(|_| "RUNTIME_REQUEST_FAILED".to_owned())?;
        let mut candidate_views = Vec::new();
        for candidate in candidates.as_array().cloned().unwrap_or_default() {
            let candidate_id = candidate
                .get("candidate_id")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let artifact = candidate
                .get("manifest_hash")
                .and_then(Value::as_str)
                .filter(|hash| hash.len() == 64)
                .and_then(|artifact_id| {
                    client
                        .call(
                            "artifact_readback_get",
                            json!({"artifact_id":artifact_id,"candidate_id":candidate_id}),
                        )
                        .ok()
                });
            candidate_views.push(json!({"candidate":candidate,"artifact":artifact}));
        }
        let snapshot = record
            .get("head_snapshot_id")
            .and_then(Value::as_str)
            .filter(|id| !id.is_empty())
            .and_then(|id| client.call("snapshot_get", json!({"snapshot_id": id})).ok());
        projections.push(json!({
            "project": project,
            "record": record,
            "versions": versions,
            "candidates": candidate_views,
            "head_snapshot": snapshot,
        }));
    }
    Ok(json!({
        "schema_version": READ_MODEL_SCHEMA,
        "status": "Ready",
        "retryable": false,
        "projects": projections,
    }))
}

fn unavailable(code: String) -> Value {
    json!({
        "schema_version": READ_MODEL_SCHEMA,
        "status": "Unavailable",
        "retryable": true,
        "code": code,
        "projects": [],
    })
}

fn runtime_data_root() -> Result<PathBuf, String> {
    if let Some(path) = env::var_os(DATA_DIR_ENV) {
        if path.is_empty() {
            return Err("RUNTIME_UNAVAILABLE".to_owned());
        }
        return Ok(PathBuf::from(path));
    }
    #[cfg(target_os = "macos")]
    {
        return Ok(PathBuf::from(
            env::var_os("HOME").ok_or_else(|| "RUNTIME_UNAVAILABLE".to_owned())?,
        )
        .join("Library")
        .join("Application Support")
        .join("ForgeCAD Runtime")
        .join("runtime-data"));
    }
    #[cfg(target_os = "windows")]
    {
        return Ok(PathBuf::from(
            env::var_os("LOCALAPPDATA").ok_or_else(|| "RUNTIME_UNAVAILABLE".to_owned())?,
        )
        .join("ForgeCAD Runtime")
        .join("runtime-data"));
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        if let Some(path) = env::var_os("XDG_DATA_HOME") {
            return Ok(PathBuf::from(path)
                .join("forgecad-runtime")
                .join("runtime-data"));
        }
        Ok(
            PathBuf::from(env::var_os("HOME").ok_or_else(|| "RUNTIME_UNAVAILABLE".to_owned())?)
                .join(".local")
                .join("share")
                .join("forgecad-runtime")
                .join("runtime-data"),
        )
    }
}

fn read_bounded_json(path: &Path) -> Result<Value, String> {
    let bytes = fs::read(path).map_err(|_| "RUNTIME_UNAVAILABLE".to_owned())?;
    if bytes.len() > 64 * 1024 {
        return Err("RUNTIME_UNAVAILABLE".to_owned());
    }
    serde_json::from_slice(&bytes).map_err(|_| "RUNTIME_UNAVAILABLE".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use forgecad_runtime::{canonical_json_hash, LocalIpcEndpoint, Runtime};
    use std::thread;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn viewer_reads_projects_and_versions_through_authenticated_ipc() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let root = PathBuf::from(format!("/tmp/fc-viewer-{unique}"));
        let socket_root = PathBuf::from(format!("/tmp/fc-viewer-sock-{unique}"));
        fs::create_dir_all(&root).expect("root");
        let runtime =
            Runtime::open_with_cas(root.join("runtime.sqlite"), root.join("cas")).expect("runtime");
        let project = runtime
            .create_project("Viewer IPC fixture", json!({"scope":"test"}))
            .expect("project");
        let mut program = json!({
            "schema_version":"GeometryProgram@1",
            "project_id":project.project_id.clone(),
            "representation_plan_sha256":"f".repeat(64),
            "nodes":[{"node_id":"viewer-torso","operator_id":"forgecad.geometry.primitive@1","part_id":"viewer-torso","parameters":{"shape":"box","size":[1.0,1.0,1.0],"position":[0,0,0],"material_zone_id":"zone-white-shell"}}],
            "budgets":{"max_nodes":4,"max_triangles":1000,"max_runtime_ms":1000}
        });
        program["canonical_sha256"] = Value::String(canonical_json_hash(&program));
        let geometry = runtime
            .prepare_geometry_candidate(
                &project.project_id,
                None,
                json!({"typed":"geometry","geometry_program":program}),
            )
            .expect("geometry candidate");
        let artifact_id = geometry["artifact"]["artifact_id"].as_str().expect("artifact").to_owned();
        let candidate_id = geometry["candidate"]["candidate_id"].as_str().expect("candidate").to_owned();
        let endpoint = LocalIpcEndpoint::new(&socket_root).expect("endpoint");
        let server = runtime.ipc_server(&endpoint).expect("server");
        let ready_path = root.join("ipc").join("ready.json");
        fs::create_dir_all(ready_path.parent().expect("ready parent")).expect("ipc dir");
        fs::write(
            &ready_path,
            serde_json::to_vec(&json!({
                "status":"ready",
                "socket_path":endpoint.socket_path(),
                "token":endpoint.token(),
            }))
            .expect("handoff"),
        )
        .expect("ready");

        let shutdown_endpoint = endpoint.clone();
        let server_thread = thread::spawn(move || server.serve_forever(&runtime));
        let model = read_model_from_root(&root).expect("read model");
        assert_eq!(model["status"], "Ready");
        assert_eq!(
            model["projects"][0]["project"]["project_id"],
            project.project_id
        );
        assert_eq!(model["projects"][0]["versions"], json!([]));
        assert_eq!(model["projects"][0]["candidates"].as_array().unwrap().len(), 1);
        assert_eq!(model["projects"][0]["candidates"][0]["candidate"]["candidate_id"], candidate_id);
        assert_eq!(model["projects"][0]["candidates"][0]["artifact"]["artifact_id"], artifact_id);
        assert_eq!(model["projects"][0]["candidates"][0]["artifact"]["part_ids"][0], "viewer-torso");

        let mut client = LocalIpcClient::connect(&shutdown_endpoint).expect("shutdown client");
        assert_eq!(
            client
                .call("runtime_shutdown", Value::Null)
                .expect("shutdown")["shutting_down"],
            true
        );
        server_thread.join().expect("server join").expect("server");
        fs::remove_dir_all(root).expect("cleanup");
        fs::remove_dir_all(socket_root).expect("socket cleanup");
    }

    #[test]
    fn unavailable_viewer_model_is_retryable_and_empty() {
        let value = unavailable("RUNTIME_UNAVAILABLE".to_owned());
        assert_eq!(value["status"], "Unavailable");
        assert_eq!(value["retryable"], true);
        assert_eq!(value["projects"], json!([]));
    }
}
