use forgecad_runtime::{LocalIpcClient, LocalIpcEndpoint};
use serde_json::{json, Value};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

const DATA_DIR_ENV: &str = "FORGECAD_RUNTIME_DATA_DIR";
const READ_MODEL_SCHEMA: &str = "ForgeCADViewerReadModel@1";
const READ_MODEL_SUMMARY_SCHEMA: &str = "ForgeCADViewerReadModelSummary@1";
const AGENTIC_PROJECTION_SCHEMA: &str = "ForgeCADAgenticDesignProjection@1";
const AGENTIC_SESSION_READBACK_SCHEMA: &str = "ForgeCADAgenticDesignSessionReadback@1";

/// Read the optional Viewer projection without starting Runtime or opening
/// SQLite/CAS. The Viewer is deliberately a read-only IPC client.
pub fn read_model() -> Value {
    match runtime_data_root().and_then(|root| read_model_from_root(&root)) {
        Ok(model) => model,
        Err(code) => unavailable(code),
    }
}

/// Read only the small change-detection projection used by the Desktop
/// Viewer. The full read model is fetched only after a candidate, version or
/// head-snapshot field changes, which keeps large artifact/reference payloads
/// out of the steady-state refresh loop.
pub fn read_model_summary() -> Value {
    match runtime_data_root().and_then(|root| read_model_summary_from_root(&root)) {
        Ok(model) => model,
        Err(code) => json!({
            "schema_version": READ_MODEL_SUMMARY_SCHEMA,
            "status": "Unavailable",
            "retryable": true,
            "code": code,
            "projects": [],
        }),
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

/// Read one bounded reference image for the compare surface. The Viewer does
/// not read the Runtime database or CAS directly; it asks the authenticated
/// Runtime projection for candidate/project-bound bytes.
pub fn read_reference_bytes(reference_id: &str, project_id: &str) -> Value {
    match runtime_data_root()
        .and_then(|root| read_reference_bytes_from_root(&root, reference_id, project_id))
    {
        Ok(value) => value,
        Err(code) => unavailable(code),
    }
}

/// Read one fixed-render pass on demand. Loading a single AOV keeps the Viewer
/// bounded while still exposing all nine Runtime-owned passes through tabs.
pub fn read_render_pass(render_set_hash: &str, pass: &str) -> Value {
    match runtime_data_root()
        .and_then(|root| read_render_pass_from_root(&root, render_set_hash, pass))
    {
        Ok(value) => value,
        Err(code) => unavailable(code),
    }
}

/// Read candidate-bound render/comparison/quality metadata without image
/// payloads. This powers the compare panel and remains strictly read-only.
pub fn read_visual_evidence(candidate_id: &str) -> Value {
    match runtime_data_root().and_then(|root| read_visual_evidence_from_root(&root, candidate_id)) {
        Ok(value) => value,
        Err(code) => unavailable(code),
    }
}

/// Read the Agentic design projection through the same authenticated Runtime
/// IPC boundary. Runtime errors, an unsupported method, or a binding failure
/// become explicit unavailable state instead of a locally fabricated stage or
/// quality result.
pub fn read_agentic_projection(project_id: &str, candidate_id: &str) -> Value {
    match runtime_data_root()
        .and_then(|root| read_agentic_projection_from_root(&root, project_id, candidate_id))
    {
        Ok(value) => value,
        Err(code) => unavailable_agentic_projection(project_id, candidate_id, &code),
    }
}

/// Read the durable DesignSession/Checkpoint state plus the derived projection
/// through authenticated Runtime IPC. The Viewer never opens SQLite/CAS and
/// exposes no write command.
pub fn read_agentic_session(project_id: &str, candidate_id: &str) -> Value {
    match runtime_data_root()
        .and_then(|root| read_agentic_session_from_root(&root, project_id, candidate_id))
    {
        Ok(value) => value,
        Err(code) => unavailable_agentic_session(project_id, candidate_id, &code),
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

fn read_reference_bytes_from_root(
    root: &Path,
    reference_id: &str,
    project_id: &str,
) -> Result<Value, String> {
    let mut client = connect_runtime(root)?;
    client
        .call(
            "reference_bytes_get",
            json!({"reference_id":reference_id,"project_id":project_id}),
        )
        .map_err(|_| "RUNTIME_REQUEST_FAILED".to_owned())
}

fn read_render_pass_from_root(
    root: &Path,
    render_set_hash: &str,
    pass: &str,
) -> Result<Value, String> {
    let mut client = connect_runtime(root)?;
    client
        .call(
            "render_pass_get",
            json!({"render_set_hash":render_set_hash,"pass":pass}),
        )
        .map_err(|_| "RUNTIME_REQUEST_FAILED".to_owned())
}

fn read_visual_evidence_from_root(root: &Path, candidate_id: &str) -> Result<Value, String> {
    let mut client = connect_runtime(root)?;
    client
        .call("visual_evidence_get", json!({"candidate_id":candidate_id}))
        .map_err(|_| "RUNTIME_REQUEST_FAILED".to_owned())
}

fn read_agentic_projection_from_root(
    root: &Path,
    project_id: &str,
    candidate_id: &str,
) -> Result<Value, String> {
    if project_id.is_empty() || candidate_id.is_empty() {
        return Err("AGENTIC_PROJECTION_UNAVAILABLE".to_owned());
    }
    let mut client = connect_runtime(root)?;
    client
        .call(
            "agentic_scene_observe",
            json!({"project_id":project_id,"candidate_id":candidate_id}),
        )
        .map_err(|_| "AGENTIC_PROJECTION_UNAVAILABLE".to_owned())
}

fn read_agentic_session_from_root(
    root: &Path,
    project_id: &str,
    candidate_id: &str,
) -> Result<Value, String> {
    if project_id.is_empty() || candidate_id.is_empty() {
        return Err("AGENTIC_SESSION_BINDING_MISSING".to_owned());
    }
    let mut client = connect_runtime(root)?;
    let durable_session = client
        .call(
            "agentic_session_lookup",
            json!({"project_id":project_id,"candidate_id":candidate_id}),
        )
        .map_err(|_| "AGENTIC_SESSION_UNAVAILABLE".to_owned())?;
    if durable_session
        .get("project_id")
        .and_then(Value::as_str)
        != Some(project_id)
        || durable_session
            .get("candidate_id")
            .and_then(Value::as_str)
            != Some(candidate_id)
    {
        return Err("AGENTIC_SESSION_BINDING_MISMATCH".to_owned());
    }
    if durable_session.get("status").and_then(Value::as_str) == Some("unavailable") {
        return Ok(json!({
            "schema_version": AGENTIC_SESSION_READBACK_SCHEMA,
            "status": "Unavailable",
            "retryable": false,
            "readback_kind": "design-session-checkpoint",
            "source": "Runtime authenticated read-only durable lookup",
            "read_only": true,
            "project_id": project_id,
            "candidate_id": candidate_id,
            "durable_session": durable_session,
        }));
    }
    let projection = client
        .call(
            "agentic_scene_observe",
            json!({"project_id":project_id,"candidate_id":candidate_id}),
        )
        .map_err(|_| "AGENTIC_SESSION_PROJECTION_UNAVAILABLE".to_owned())?;
    if projection
        .get("projection_status")
        .and_then(Value::as_str)
        != Some("projection/read-only")
        || projection.get("read_only").and_then(Value::as_bool) != Some(true)
        || projection.get("project_id").and_then(Value::as_str) != Some(project_id)
        || projection.get("candidate_id").and_then(Value::as_str) != Some(candidate_id)
        || projection.get("design_session").and_then(Value::as_object).is_none()
        || projection.get("design_stage_plan").and_then(Value::as_object).is_none()
    {
        return Err("AGENTIC_SESSION_BINDING_MISMATCH".to_owned());
    }
    Ok(json!({
        "schema_version": AGENTIC_SESSION_READBACK_SCHEMA,
        "status": "Ready",
        "retryable": false,
        "readback_kind": "design-session-checkpoint",
        "source": "Runtime authenticated read-only projection",
        "read_only": true,
        "project_id": project_id,
        "candidate_id": candidate_id,
        "durable_session": durable_session,
        "projection": projection,
    }))
}

fn connect_runtime(root: &Path) -> Result<LocalIpcClient, String> {
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
    LocalIpcClient::connect(&LocalIpcEndpoint::from_parts(socket, token))
        .map_err(|_| "RUNTIME_UNAVAILABLE".to_owned())
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
            let quality = client
                .call("quality_get", json!({"candidate_id": candidate_id}))
                .ok();
            let reference = quality
                .as_ref()
                .and_then(|value| value.get("reference_id"))
                .and_then(Value::as_str)
                .and_then(|reference_id| {
                    client
                        .call("reference_get", json!({"reference_id": reference_id}))
                        .ok()
                });
            candidate_views.push(json!({
                "candidate":candidate,
                "artifact":artifact,
                "quality":quality,
                "reference":reference
            }));
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

fn read_model_summary_from_root(root: &Path) -> Result<Value, String> {
    let mut client = connect_runtime(root)?;
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
        let candidate_summaries = candidates
            .as_array()
            .map(|items| {
                items
                    .iter()
                    .map(|candidate| {
                        json!({
                            "candidate_id": candidate.get("candidate_id").cloned().unwrap_or(Value::Null),
                            "project_id": candidate.get("project_id").cloned().unwrap_or(Value::Null),
                            "state": candidate.get("state").cloned().unwrap_or(Value::Null),
                            "canonical_sha256": candidate.get("canonical_sha256").cloned().unwrap_or(Value::Null),
                            "quality_hard_gate_passed": candidate.get("quality_hard_gate_passed").cloned().unwrap_or(Value::Null),
                            "created_at": candidate.get("created_at").cloned().unwrap_or(Value::Null),
                            "updated_at": candidate.get("updated_at").cloned().unwrap_or(Value::Null),
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        projections.push(json!({
            "project": {
                "project_id": project.get("project_id").cloned().unwrap_or(Value::Null),
                "name": project.get("name").cloned().unwrap_or(Value::Null),
            },
            "record": {
                "head_snapshot_id": record.get("head_snapshot_id").cloned().unwrap_or(Value::Null),
            },
            "versions_count": versions.as_array().map(|items| items.len()).unwrap_or(0),
            "candidates": candidate_summaries,
        }));
    }
    Ok(json!({
        "schema_version": READ_MODEL_SUMMARY_SCHEMA,
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

fn unavailable_agentic_projection(project_id: &str, candidate_id: &str, code: &str) -> Value {
    json!({
        "schema_version": AGENTIC_PROJECTION_SCHEMA,
        "status": "Unavailable",
        "retryable": true,
        "source": "Runtime authenticated read-only projection",
        "code": code,
        "project_id": if project_id.is_empty() { Value::Null } else { Value::String(project_id.to_owned()) },
        "candidate_id": if candidate_id.is_empty() { Value::Null } else { Value::String(candidate_id.to_owned()) },
    })
}

fn unavailable_agentic_session(project_id: &str, candidate_id: &str, code: &str) -> Value {
    json!({
        "schema_version": AGENTIC_SESSION_READBACK_SCHEMA,
        "status": "Unavailable",
        "retryable": true,
        "readback_kind": "design-session-checkpoint",
        "source": "Runtime authenticated read-only projection",
        "read_only": true,
        "code": code,
        "project_id": if project_id.is_empty() { Value::Null } else { Value::String(project_id.to_owned()) },
        "candidate_id": if candidate_id.is_empty() { Value::Null } else { Value::String(candidate_id.to_owned()) },
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
        let reference = runtime
            .import_reference(&forgecad_runtime::ReferenceImportRequest {
                project_id: project.project_id.clone(),
                source: forgecad_runtime::ReferenceImportSource::InlineContent {
                    mime: "image/png".to_owned(),
                    content_base64: "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=".to_owned(),
                },
                authorization: forgecad_runtime::ReferenceAuthorization {
                    user_authorized: true,
                    declaration: "viewer IPC reference".to_owned(),
                },
                expected_sha256: None,
            })
            .expect("reference")
            .reference;
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
        let artifact_id = geometry["artifact"]["artifact_id"]
            .as_str()
            .expect("artifact")
            .to_owned();
        let candidate_id = geometry["candidate"]["candidate_id"]
            .as_str()
            .expect("candidate")
            .to_owned();
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
        assert_eq!(
            model["projects"][0]["candidates"].as_array().unwrap().len(),
            1
        );
        assert_eq!(
            model["projects"][0]["candidates"][0]["candidate"]["candidate_id"],
            candidate_id
        );
        assert_eq!(
            model["projects"][0]["candidates"][0]["artifact"]["artifact_id"],
            artifact_id
        );
        assert_eq!(
            model["projects"][0]["candidates"][0]["artifact"]["part_ids"][0],
            "viewer-torso"
        );
        let reference_payload =
            read_reference_bytes_from_root(&root, &reference.reference_id, &project.project_id)
                .expect("reference bytes");
        assert_eq!(reference_payload["reference_id"], reference.reference_id);
        assert_eq!(reference_payload["sha256"], reference.object_sha256);
        assert!(reference_payload["bytes_base64"]
            .as_str()
            .is_some_and(|value| !value.is_empty()));

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
    fn missing_agentic_projection_is_unavailable_over_authenticated_ipc() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let root = PathBuf::from(format!("/tmp/fc-viewer-agentic-{unique}"));
        let socket_root = PathBuf::from(format!("/tmp/fc-viewer-agentic-sock-{unique}"));
        fs::create_dir_all(&root).expect("root");
        let runtime =
            Runtime::open_with_cas(root.join("runtime.sqlite"), root.join("cas")).expect("runtime");
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
        let error = read_agentic_projection_from_root(&root, "project-a", "candidate-a")
            .expect_err("the current Runtime has no Agentic projection IPC");
        assert_eq!(error, "AGENTIC_PROJECTION_UNAVAILABLE");

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
    fn missing_agentic_session_is_unavailable_over_authenticated_ipc() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let root = PathBuf::from(format!("/tmp/fc-viewer-session-{unique}"));
        let socket_root = PathBuf::from(format!("/tmp/fc-viewer-session-sock-{unique}"));
        fs::create_dir_all(&root).expect("root");
        let runtime =
            Runtime::open_with_cas(root.join("runtime.sqlite"), root.join("cas")).expect("runtime");
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
        let value = read_agentic_session_from_root(&root, "project-a", "candidate-a")
            .expect("missing session must return explicit unavailable readback");
        assert_eq!(value["status"], "Unavailable");
        assert_eq!(value["read_only"], true);
        assert_eq!(value["durable_session"]["status"], "unavailable");

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

    #[test]
    fn unavailable_agentic_projection_is_explicit_and_has_no_quality_fallback() {
        let value = unavailable_agentic_projection(
            "project-a",
            "candidate-a",
            "AGENTIC_PROJECTION_UNAVAILABLE",
        );
        assert_eq!(value["schema_version"], AGENTIC_PROJECTION_SCHEMA);
        assert_eq!(value["status"], "Unavailable");
        assert_eq!(
            value["source"],
            "Runtime authenticated read-only projection"
        );
        assert_eq!(value["code"], "AGENTIC_PROJECTION_UNAVAILABLE");
        assert!(value.get("quality_report").is_none());
        assert!(value.get("gates").is_none());
    }

    #[test]
    fn agentic_session_frontend_source_guard_is_read_only() {
        let session_source =
            include_str!("../../src/features/runtime-viewer/agentic-session.ts");
        let viewer_source = include_str!("../../src/features/runtime-viewer/RuntimeViewer.tsx");
        for token in [
            "normalizeAgenticSessionProjection",
            "evidenceBindings",
            "uncertainty",
            "locked-read-only",
        ] {
            assert!(session_source.contains(token), "missing session source token: {token}");
        }
        for token in [
            "viewer_agentic_session",
            "DESIGN SESSION / CHECKPOINT",
            "restore prepare / approval",
            "允许显示",
        ] {
            assert!(viewer_source.contains(token), "missing Viewer source token: {token}");
        }
        for token in [
            "candidate_confirm(",
            "export_confirm(",
            "restore_confirm(",
        ] {
            assert!(!viewer_source.contains(token), "forbidden Viewer action invocation: {token}");
        }
        for token in ["invokeModel(", "fetch("] {
            assert!(!session_source.contains(token), "forbidden session readback invocation: {token}");
            assert!(!viewer_source.contains(token), "forbidden Viewer readback invocation: {token}");
        }
    }
}
