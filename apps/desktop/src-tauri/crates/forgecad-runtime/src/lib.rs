mod ipc;

pub use forgecad_contracts::{
    is_opaque_id, supports_mcp_protocol, RuntimeCapabilities, RuntimeResourceContents,
    RuntimeResourceDescriptor, SelectionRecord, CONTRACT_SET, MCP_PROTOCOL_COMPAT_VERSION,
    MCP_PROTOCOL_VERSION, MCP_PROTOCOL_VERSIONS,
};
pub use forgecad_core::{canonical_json_hash, sha256_hex};
pub use forgecad_store::{CasError, CasObject, CasStore, LeaseGrant, Store, StoreError};
pub use ipc::{IpcError, LocalIpcClient, LocalIpcEndpoint, LocalIpcServer};

use forgecad_contracts::{
    CandidateRecord, DesignAssetVersionRecord, JobEventRecord, JobSummary, ProjectRecord,
    ProjectSummary, SnapshotRecord, SnapshotSummary,
};
use serde_json::{json, Value};
use std::path::Path;
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum RuntimeError {
    #[error("store error: {0}")]
    Store(#[from] StoreError),
    #[error("IPC error: {0}")]
    Ipc(#[from] IpcError),
    #[error("invalid runtime input: {0}")]
    InvalidInput(String),
}

pub struct Runtime {
    store: Store,
    capabilities: RuntimeCapabilities,
    lease: LeaseGrant,
}

impl Runtime {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, RuntimeError> {
        let store = Store::open(path)?;
        Self::from_store(store)
    }

    pub fn open_with_cas(
        database_path: impl AsRef<Path>,
        cas_root: impl AsRef<Path>,
    ) -> Result<Self, RuntimeError> {
        let store = Store::open_with_cas(database_path, cas_root)?;
        Self::from_store(store)
    }

    pub fn ephemeral() -> Result<Self, RuntimeError> {
        Self::from_store(Store::memory()?)
    }

    pub fn from_store(store: Store) -> Result<Self, RuntimeError> {
        let owner = format!("runtime-{}", Uuid::new_v4().simple());
        let lease = store.acquire_writer_lease(&owner)?;
        Ok(Self {
            store,
            capabilities: RuntimeCapabilities::default(),
            lease,
        })
    }

    pub fn capabilities(&self) -> &RuntimeCapabilities {
        &self.capabilities
    }

    pub fn projects(&self) -> Result<Vec<ProjectSummary>, RuntimeError> {
        Ok(self.store.list_projects()?)
    }

    pub fn project(&self, id: &str) -> Result<Option<ProjectRecord>, RuntimeError> {
        validate_id(id)?;
        Ok(self.store.get_project(id)?)
    }

    pub fn snapshot(&self, id: &str) -> Result<Option<SnapshotSummary>, RuntimeError> {
        validate_id(id)?;
        Ok(self.store.get_snapshot(id)?)
    }

    pub fn snapshot_record(&self, id: &str) -> Result<Option<SnapshotRecord>, RuntimeError> {
        validate_id(id)?;
        Ok(self.store.get_snapshot_record(id)?)
    }

    pub fn candidate(&self, id: &str) -> Result<Option<CandidateRecord>, RuntimeError> {
        validate_id(id)?;
        Ok(self.store.get_candidate(id)?)
    }

    pub fn version(&self, id: &str) -> Result<Option<DesignAssetVersionRecord>, RuntimeError> {
        validate_id(id)?;
        Ok(self.store.get_version(id)?)
    }

    pub fn versions(
        &self,
        project_id: Option<&str>,
    ) -> Result<Vec<DesignAssetVersionRecord>, RuntimeError> {
        if let Some(project_id) = project_id {
            validate_id(project_id)?;
        }
        Ok(self.store.list_versions(project_id)?)
    }

    pub fn job(&self, id: &str) -> Result<Option<JobSummary>, RuntimeError> {
        validate_id(id)?;
        Ok(self.store.get_job(id)?)
    }

    pub fn job_events(
        &self,
        id: &str,
        after_sequence: i64,
    ) -> Result<Vec<JobEventRecord>, RuntimeError> {
        validate_id(id)?;
        if after_sequence < 0 {
            return Err(RuntimeError::InvalidInput(
                "negative event cursor".to_owned(),
            ));
        }
        Ok(self.store.list_job_events(id, after_sequence)?)
    }

    pub fn selection(&self) -> SelectionRecord {
        SelectionRecord {
            schema_version: "Selection@1".to_owned(),
            available: false,
            project_id: None,
            snapshot_id: None,
            version_id: None,
            part_ids: Vec::new(),
            limitation: Some(
                "Viewer selection is not connected to the MCP read model until MCP010.".to_owned(),
            ),
        }
    }

    pub fn resource_descriptors(&self) -> Result<Vec<RuntimeResourceDescriptor>, RuntimeError> {
        let mut resources = vec![RuntimeResourceDescriptor {
            schema_version: "RuntimeResource@1".to_owned(),
            uri: "forgecad://capabilities".to_owned(),
            name: "Runtime capabilities".to_owned(),
            description: "Live Runtime contract and capability state".to_owned(),
            mime_type: "application/json".to_owned(),
            read_only: true,
        }];
        for project in self.projects()? {
            resources.push(RuntimeResourceDescriptor {
                schema_version: "RuntimeResource@1".to_owned(),
                uri: format!("forgecad://projects/{}/snapshot", project.project_id),
                name: format!("{} snapshot", project.name),
                description: "The project's current ActiveDesignSnapshot projection".to_owned(),
                mime_type: "application/json".to_owned(),
                read_only: true,
            });
            resources.push(RuntimeResourceDescriptor {
                schema_version: "RuntimeResource@1".to_owned(),
                uri: format!("forgecad://projects/{}/selection", project.project_id),
                name: format!("{} selection", project.name),
                description: "Ephemeral Viewer selection; never a version truth".to_owned(),
                mime_type: "application/json".to_owned(),
                read_only: true,
            });
        }
        Ok(resources)
    }

    pub fn read_resource(&self, uri: &str) -> Result<RuntimeResourceContents, RuntimeError> {
        let segments = resource_segments(uri)?;
        let value = match segments.as_slice() {
            ["capabilities"] => serde_json::to_value(self.capabilities())
                .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?,
            ["projects", project_id, "snapshot"] => {
                let project = self
                    .project(project_id)?
                    .ok_or_else(|| RuntimeError::InvalidInput("project not found".to_owned()))?;
                let snapshot = project
                    .head_snapshot_id
                    .as_deref()
                    .map(|id| self.snapshot_record(id))
                    .transpose()?
                    .flatten();
                json!({
                    "schema_version": "ProjectSnapshotResource@1",
                    "project": project,
                    "snapshot": snapshot,
                })
            }
            ["projects", project_id, "selection"] => {
                if self.project(project_id)?.is_none() {
                    return Err(RuntimeError::InvalidInput("project not found".to_owned()));
                }
                serde_json::to_value(self.selection())
                    .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?
            }
            ["candidates", candidate_id] => serde_json::to_value(
                self.candidate(candidate_id)?
                    .ok_or_else(|| RuntimeError::InvalidInput("candidate not found".to_owned()))?,
            )
            .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?,
            ["jobs", job_id] => {
                let job = self
                    .job(job_id)?
                    .ok_or_else(|| RuntimeError::InvalidInput("job not found".to_owned()))?;
                let events = self
                    .job_events(job_id, 0)?
                    .into_iter()
                    .take(128)
                    .collect::<Vec<_>>();
                json!({"schema_version":"RuntimeJobResource@1","job":job,"events":events})
            }
            ["versions", version_id] => serde_json::to_value(
                self.version(version_id)?
                    .ok_or_else(|| RuntimeError::InvalidInput("version not found".to_owned()))?,
            )
            .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?,
            ["renders", ..] | ["skills", ..] | ["artifacts", ..] => {
                return Err(RuntimeError::InvalidInput(
                    "resource capability is unavailable".to_owned(),
                ));
            }
            _ => {
                return Err(RuntimeError::InvalidInput(
                    "unknown ForgeCAD resource".to_owned(),
                ))
            }
        };
        let text = serde_json::to_string(&value)
            .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?;
        Ok(RuntimeResourceContents {
            schema_version: "RuntimeResourceContents@1".to_owned(),
            uri: uri.to_owned(),
            mime_type: "application/json".to_owned(),
            size_bytes: text.len() as u64,
            text,
        })
    }

    pub fn create_project(&self, name: &str, policy: Value) -> Result<ProjectRecord, RuntimeError> {
        if name.trim().is_empty() || name.len() > 200 {
            return Err(RuntimeError::InvalidInput(
                "project name is invalid".to_owned(),
            ));
        }
        let project_id = format!("project-{}", Uuid::new_v4().simple());
        let timestamp = now_string();
        let digest_input = json!({
            "schema_version": "Project@1",
            "project_id": project_id,
            "name": name,
            "policy": policy,
            "created_at": timestamp,
        });
        let project = ProjectRecord {
            schema_version: "Project@1".to_owned(),
            project_id,
            name: name.to_owned(),
            policy,
            created_at: timestamp.clone(),
            updated_at: timestamp,
            active_snapshot_revision: 0,
            head_snapshot_id: None,
            canonical_sha256: canonical_json_hash(&digest_input),
        };
        self.store.insert_project(&project)?;
        Ok(project)
    }

    pub fn insert_candidate(&self, candidate: &CandidateRecord) -> Result<(), RuntimeError> {
        self.store.insert_candidate(candidate)?;
        Ok(())
    }

    pub fn insert_version(&self, version: &DesignAssetVersionRecord) -> Result<(), RuntimeError> {
        self.store.insert_version(version)?;
        Ok(())
    }

    pub fn put_object(
        &self,
        bytes: &[u8],
        expected_sha256: Option<&str>,
        mime: &str,
        kind: &str,
    ) -> Result<CasObject, RuntimeError> {
        if mime.is_empty() || kind.is_empty() {
            return Err(RuntimeError::InvalidInput(
                "object metadata is incomplete".to_owned(),
            ));
        }
        Ok(self
            .store
            .put_object(bytes, expected_sha256, mime, kind, &now_string())?)
    }

    pub fn cas_read(&self, sha256: &str) -> Result<Vec<u8>, RuntimeError> {
        if !forgecad_contracts::is_sha256(sha256) {
            return Err(RuntimeError::InvalidInput("invalid CAS hash".to_owned()));
        }
        Ok(self
            .store
            .cas()
            .read_verified(sha256)
            .map_err(StoreError::Cas)?)
    }

    pub fn ipc_server(&self, endpoint: &LocalIpcEndpoint) -> Result<LocalIpcServer, RuntimeError> {
        Ok(LocalIpcServer::bind(endpoint)?)
    }

    pub fn serve_ipc_once(&self, server: &LocalIpcServer) -> Result<(), RuntimeError> {
        server.serve_once(self)?;
        Ok(())
    }

    pub(crate) fn dispatch_ipc(
        &self,
        method: &str,
        payload: &Value,
    ) -> Result<Value, RuntimeError> {
        match method {
            "capabilities_get" => Ok(serde_json::to_value(self.capabilities())
                .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?),
            "project_list" => Ok(serde_json::to_value(self.projects()?)
                .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?),
            "project_get" => {
                let id = payload
                    .get("project_id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        RuntimeError::InvalidInput("project_id is required".to_owned())
                    })?;
                Ok(serde_json::to_value(self.project(id)?)
                    .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?)
            }
            "snapshot_get" => {
                let id = payload
                    .get("snapshot_id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        RuntimeError::InvalidInput("snapshot_id is required".to_owned())
                    })?;
                Ok(serde_json::to_value(self.snapshot(id)?)
                    .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?)
            }
            "selection_get" => Ok(serde_json::to_value(self.selection())
                .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?),
            "candidate_get" => {
                let id = payload
                    .get("candidate_id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        RuntimeError::InvalidInput("candidate_id is required".to_owned())
                    })?;
                Ok(serde_json::to_value(self.candidate(id)?)
                    .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?)
            }
            "job_get" => {
                let id = payload
                    .get("job_id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| RuntimeError::InvalidInput("job_id is required".to_owned()))?;
                Ok(serde_json::to_value(self.job(id)?)
                    .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?)
            }
            "job_events_read" => {
                let id = payload
                    .get("job_id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| RuntimeError::InvalidInput("job_id is required".to_owned()))?;
                let after_sequence = payload
                    .get("after_sequence")
                    .and_then(Value::as_i64)
                    .unwrap_or(0);
                Ok(serde_json::to_value(self.job_events(id, after_sequence)?)
                    .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?)
            }
            "version_list" => {
                let project_id = payload.get("project_id").and_then(Value::as_str);
                Ok(serde_json::to_value(self.versions(project_id)?)
                    .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?)
            }
            "version_get" => {
                let id = payload
                    .get("version_id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        RuntimeError::InvalidInput("version_id is required".to_owned())
                    })?;
                Ok(serde_json::to_value(self.version(id)?)
                    .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?)
            }
            "resources_list" => Ok(serde_json::to_value(self.resource_descriptors()?)
                .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?),
            "resource_read" => {
                let uri = payload
                    .get("uri")
                    .and_then(Value::as_str)
                    .ok_or_else(|| RuntimeError::InvalidInput("uri is required".to_owned()))?;
                Ok(serde_json::to_value(self.read_resource(uri)?)
                    .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?)
            }
            _ => Err(RuntimeError::InvalidInput(
                "unsupported IPC method".to_owned(),
            )),
        }
    }
}

impl Drop for Runtime {
    fn drop(&mut self) {
        let _ = self
            .store
            .release_writer_lease(&self.lease.owner, &self.lease.token);
    }
}

fn validate_id(id: &str) -> Result<(), RuntimeError> {
    if !is_opaque_id(id) {
        return Err(RuntimeError::InvalidInput("invalid opaque id".to_owned()));
    }
    Ok(())
}

fn resource_segments(uri: &str) -> Result<Vec<&str>, RuntimeError> {
    let rest = uri.strip_prefix("forgecad://").ok_or_else(|| {
        RuntimeError::InvalidInput("resource URI scheme is not allowed".to_owned())
    })?;
    if rest.is_empty() || rest.len() > 512 || rest.contains(['?', '#', '\\']) {
        return Err(RuntimeError::InvalidInput(
            "resource URI is invalid".to_owned(),
        ));
    }
    let segments = rest.split('/').collect::<Vec<_>>();
    if segments.iter().any(|segment| {
        segment.is_empty()
            || *segment == "."
            || *segment == ".."
            || (segment
                .as_bytes()
                .iter()
                .any(|byte| !byte.is_ascii_alphanumeric() && !matches!(byte, b'_' | b'-' | b'.'))
                && *segment != "capabilities"
                && *segment != "projects"
                && *segment != "candidates"
                && *segment != "jobs"
                && *segment != "versions"
                && *segment != "renders"
                && *segment != "skills"
                && *segment != "artifacts"
                && *segment != "snapshot"
                && *segment != "selection")
    }) {
        return Err(RuntimeError::InvalidInput(
            "resource URI contains an invalid segment".to_owned(),
        ));
    }
    Ok(segments)
}

fn now_string() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before UNIX_EPOCH")
        .as_secs()
        .to_string()
}

#[allow(dead_code)]
fn request_hash(value: &Value) -> String {
    forgecad_core::canonical_json_hash(value)
}

#[allow(dead_code)]
fn bytes_hash(bytes: &[u8]) -> String {
    sha256_hex(bytes)
}
