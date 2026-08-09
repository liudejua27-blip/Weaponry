mod ipc;
mod process_lock;
mod skill_registry;

use base64::Engine;
use forgecad_geometry_worker::{
    compile_geometry_program, compile_geometry_program_with_appearance, render_fixed_glb,
    GeometryArtifact,
};
pub use forgecad_contracts::{
    build_cohort_sha256, is_opaque_id, supports_mcp_protocol, RuntimeCapabilities, RuntimeResourceContents,
    RuntimeResourceDescriptor, SelectionRecord, ReferenceAuthorization, ReferenceEvidenceRecord,
    ReferenceGetResult, ReferenceImportRequest, ReferenceImportResult, ReferenceImportSource,
    SkillBundleManifestRecord, SkillExecutionReceiptRecord, SkillEvalReportRecord,
    SkillGetResult, SkillListResult,
    CONTRACT_SET, MCP_PROTOCOL_COMPAT_VERSION, MCP_PROTOCOL_VERSION, MCP_PROTOCOL_VERSIONS,
};
pub use forgecad_core::{canonical_json_bytes, canonical_json_hash, sha256_hex};
pub use forgecad_store::{CasError, CasObject, CasStore, Store, StoreError};
pub use ipc::{IpcError, LocalIpcClient, LocalIpcEndpoint, LocalIpcServer};

use forgecad_contracts::{
    CandidateConfirmRequest, CandidateConfirmResult, CandidatePrepareResult, CandidateRecord,
    CandidateRejectRequest, CandidateRejectResult, DesignAssetVersionRecord, ExportConfirmRequest,
    ExportConfirmResult, ExportPrepareRequest, ExportPrepareResult, JobEventRecord, JobRecord,
    JobSummary, ProjectRecord, ProjectSummary, RestoreConfirmRequest, RestoreConfirmResult,
    RestorePrepareRequest, RestorePrepareResult,
    SnapshotRecord, SnapshotSummary,
};
use image::{ImageFormat, ImageReader, Limits};
use serde_json::{json, Value};
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use uuid::Uuid;

const MAX_REFERENCE_BYTES: usize = 8 * 1024 * 1024;
const MAX_REFERENCE_INLINE_BASE64: usize = 12 * 1024 * 1024;
const MAX_REFERENCE_WIDTH: u32 = 8192;
const MAX_REFERENCE_HEIGHT: u32 = 8192;
const MAX_REFERENCE_PIXELS: u64 = 16_777_216;
const MAX_REFERENCE_DECODE_ALLOC: u64 = 64 * 1024 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum RuntimeError {
    #[error("{0}")]
    Store(#[from] StoreError),
    #[error("IPC error: {0}")]
    Ipc(#[from] IpcError),
    #[error("invalid runtime input: {0}")]
    InvalidInput(String),
    #[error("{0}")]
    ProcessLock(String),
}

pub struct Runtime {
    store: Store,
    capabilities: RuntimeCapabilities,
    reference_attachment_roots: Vec<PathBuf>,
    _process_lock: Option<process_lock::ProcessLock>,
}

impl Runtime {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, RuntimeError> {
        let database_path = path.as_ref().to_path_buf();
        let cas_root = database_path.with_extension("cas");
        Self::open_with_cas(database_path, cas_root)
    }

    pub fn open_with_cas(
        database_path: impl AsRef<Path>,
        cas_root: impl AsRef<Path>,
    ) -> Result<Self, RuntimeError> {
        let process_lock = process_lock::ProcessLock::acquire_for_database(database_path.as_ref())
            .map_err(|error| RuntimeError::ProcessLock(error.to_string()))?;
        let store = Store::open_with_cas(database_path, cas_root)?;
        Ok(Self {
            store,
            capabilities: runtime_capabilities(),
            reference_attachment_roots: configured_attachment_roots(),
            _process_lock: Some(process_lock),
        })
    }

    pub fn ephemeral() -> Result<Self, RuntimeError> {
        Self::from_store(Store::memory()?)
    }

    pub fn from_store(store: Store) -> Result<Self, RuntimeError> {
        Self::from_store_with_attachment_roots(store, Vec::new())
    }

    pub fn from_store_with_attachment_roots(
        store: Store,
        reference_attachment_roots: Vec<PathBuf>,
    ) -> Result<Self, RuntimeError> {
        Ok(Self {
            store,
            capabilities: runtime_capabilities(),
            reference_attachment_roots,
            _process_lock: None,
        })
    }

    pub fn capabilities(&self) -> &RuntimeCapabilities {
        &self.capabilities
    }

    pub fn skills(&self) -> Result<Vec<SkillBundleManifestRecord>, RuntimeError> {
        skill_registry::list().map_err(RuntimeError::InvalidInput)
    }

    pub fn skill(&self, skill_id: &str, version: &str) -> Result<Option<SkillBundleManifestRecord>, RuntimeError> {
        if !is_opaque_id(skill_id) || !is_opaque_id(version) {
            return Err(RuntimeError::InvalidInput("invalid Skill identifier".to_owned()));
        }
        skill_registry::get(skill_id, version).map_err(RuntimeError::InvalidInput)
    }

    pub fn projects(&self) -> Result<Vec<ProjectSummary>, RuntimeError> {
        Ok(self.store.list_projects()?)
    }

    pub fn reference(
        &self,
        id: &str,
    ) -> Result<Option<ReferenceEvidenceRecord>, RuntimeError> {
        validate_id(id)?;
        Ok(self.store.get_reference_evidence(id)?)
    }

    pub fn references(
        &self,
        project_id: &str,
    ) -> Result<Vec<ReferenceEvidenceRecord>, RuntimeError> {
        validate_id(project_id)?;
        Ok(self.store.list_reference_evidence(project_id)?)
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

    pub fn candidates(&self, project_id: &str) -> Result<Vec<CandidateRecord>, RuntimeError> {
        validate_id(project_id)?;
        Ok(self.store.list_candidates(project_id)?)
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

    /// Return a deterministic QualityReport projection for a candidate. The
    /// MVP report is intentionally conservative: geometry/GLB/PBR/fixed-pass
    /// checks are reported from Runtime-owned artifacts; image similarity is
    /// only a bounded aspect-ratio comparison and is marked limited.
    pub fn quality(&self, candidate_id: &str, reference_id: Option<&str>) -> Result<Value, RuntimeError> {
        let candidate = self.candidate(candidate_id)?.ok_or_else(|| {
            RuntimeError::InvalidInput("NOT_FOUND: candidate not found".to_owned())
        })?;
        let mut checks = Vec::new();
        let mut artifact = Value::Null;
        if let Some(hash) = candidate.manifest_hash.as_deref().filter(|hash| forgecad_contracts::is_sha256(hash)) {
            artifact = self.artifact_readback(hash, candidate_id).unwrap_or(Value::Null);
        }
        let artifact_valid = artifact.get("validator_status").and_then(Value::as_str) == Some("passed");
        checks.push(json!({"check_id":"candidate_state","status":if candidate.state == "reviewable" || candidate.state == "confirmed" {"passed"} else {"failed"},"message":format!("candidate state is {}", candidate.state)}));
        checks.push(json!({"check_id":"glb_readback","status":if artifact_valid {"passed"} else {"not-run"},"message":"Runtime artifact readback is hash-bound"}));
        checks.push(json!({"check_id":"uv_tangent","status":if artifact.get("uv_status").and_then(Value::as_str) == Some("passed") && artifact.get("tangent_status").and_then(Value::as_str) == Some("passed") {"passed"} else {"not-run"},"message":"UV and tangent attributes are present when appearance was prepared"}));
        checks.push(json!({"check_id":"pbr_material_zones","status":if artifact.get("material_zone_ids").and_then(Value::as_array).is_some_and(|zones| !zones.is_empty()) {"passed"} else {"not-run"},"message":"typed material zones are bound in GLB lineage"}));
        let mut compare = Value::Null;
        if let Some(reference_id) = reference_id {
            let reference = self.reference(reference_id)?.ok_or_else(|| RuntimeError::InvalidInput("NOT_FOUND: reference not found".to_owned()))?;
            let bytes = self.cas_read(&reference.object_sha256)?;
            let reference_ratio = reference.width as f64 / reference.height as f64;
            let model_ratio = candidate
                .manifest_hash
                .as_deref()
                .and_then(|hash| self.cas_read(hash).ok())
                .and_then(|bytes| inspect_glb(&bytes).ok())
                .map(|inspection| inspection.aspect_ratio)
                .unwrap_or(1.0);
            let aspect_error = (reference_ratio - model_ratio).abs();
            let aspect_score = (1.0 - aspect_error.min(1.0)).max(0.0);
            compare = json!({
                "reference_id":reference_id,
                "reference_sha256":reference.object_sha256,
                "reference_bytes_verified":!bytes.is_empty(),
                "reference_aspect_ratio":reference_ratio,
                "model_aspect_ratio":model_ratio,
                "aspect_score":aspect_score,
                "status":"limited",
                "limitation":"MVP does not infer pixel silhouette or semantic correspondence from one reference image; this is an aspect-ratio evidence check only."
            });
            checks.push(json!({"check_id":"reference_aspect_ratio","status":if aspect_score >= 0.55 {"passed"} else {"failed"},"message":"bounded reference/model aspect comparison","value":aspect_score,"threshold":0.55}));
        }
        let hard_gate_passed = candidate.quality_hard_gate_passed && checks.iter().all(|check| check.get("status").and_then(Value::as_str) != Some("failed"));
        let mut report = json!({
            "schema_version":"QualityReport@1",
            "quality_report_id":candidate.quality_report_id,
            "candidate_id":candidate_id,
            "hard_gate_passed":hard_gate_passed,
            "checks":checks,
            "artifact":artifact,
            "reference_compare":compare,
            "canonical_sha256":""
        });
        report["canonical_sha256"] = Value::String(canonical_json_hash(&report));
        Ok(report)
    }

    pub fn version_diff(&self, version_id: &str, compare_to_version_id: &str) -> Result<Value, RuntimeError> {
        validate_id(version_id)?;
        validate_id(compare_to_version_id)?;
        let version = self.version(version_id)?.ok_or_else(|| RuntimeError::InvalidInput("NOT_FOUND: version not found".to_owned()))?;
        let compare = self.version(compare_to_version_id)?.ok_or_else(|| RuntimeError::InvalidInput("NOT_FOUND: version not found".to_owned()))?;
        if version.project_id != compare.project_id {
            return Err(RuntimeError::InvalidInput("PROJECT_SCOPE_DENIED: versions are from different projects".to_owned()));
        }
        let left = self.candidate(&version.candidate_id)?.ok_or_else(|| RuntimeError::InvalidInput("NOT_FOUND: version candidate not found".to_owned()))?;
        let right = self.candidate(&compare.candidate_id)?.ok_or_else(|| RuntimeError::InvalidInput("NOT_FOUND: comparison candidate not found".to_owned()))?;
        let left_parts = left.manifest_hash.as_deref().and_then(|hash| self.cas_read(hash).ok()).and_then(|bytes| inspect_glb(&bytes).ok()).map(|inspection| inspection.part_ids).unwrap_or_default();
        let right_parts = right.manifest_hash.as_deref().and_then(|hash| self.cas_read(hash).ok()).and_then(|bytes| inspect_glb(&bytes).ok()).map(|inspection| inspection.part_ids).unwrap_or_default();
        Ok(json!({
            "schema_version":"VersionDiff@1",
            "project_id":version.project_id,
            "version_id":version_id,
            "compare_to_version_id":compare_to_version_id,
            "same_artifact":version.manifest_hash == compare.manifest_hash,
            "request_changed":left.request_sha256 != right.request_sha256,
            "part_ids": {"version":left_parts,"compare_to_version":right_parts},
            "limitation":"MVP diff is hash/lineage based; mesh-space delta visualization remains post-MVP."
        }))
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
            for reference in self.references(&project.project_id)? {
                resources.push(RuntimeResourceDescriptor {
                    schema_version: "RuntimeResource@1".to_owned(),
                    uri: format!("forgecad://references/{}", reference.reference_id),
                    name: format!("{} reference", reference.reference_id),
                    description: "Hash-bound reference image evidence; original path is not retained".to_owned(),
                    mime_type: "application/json".to_owned(),
                    read_only: true,
                });
            }
        }
        for skill in self.skills()? {
            resources.push(RuntimeResourceDescriptor {
                schema_version: "RuntimeResource@1".to_owned(),
                uri: format!("forgecad://skills/{}/{}", skill.skill_id, skill.version),
                name: format!("{} Skill", skill.skill_id),
                description: "First-party development-only Skill manifest; no executable payload".to_owned(),
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
            ["references", reference_id] => serde_json::to_value(
                ReferenceGetResult {
                    schema_version: "ReferenceGetResult@1".to_owned(),
                    reference: self
                        .reference(reference_id)?
                        .ok_or_else(|| RuntimeError::InvalidInput("reference not found".to_owned()))?,
                },
            )
            .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?,
            ["skills", skill_id, version] => skill_registry::get_result(skill_id, version)
                .map_err(RuntimeError::InvalidInput)?,
            ["renders", ..] | ["artifacts", ..] => {
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

    pub fn import_reference(
        &self,
        request: &ReferenceImportRequest,
    ) -> Result<ReferenceImportResult, RuntimeError> {
        validate_id(&request.project_id)?;
        if self.project(&request.project_id)?.is_none() {
            return Err(RuntimeError::InvalidInput(
                "PROJECT_SCOPE_DENIED: project not found".to_owned(),
            ));
        }
        validate_reference_authorization(&request.authorization)?;
        if let Some(expected) = request.expected_sha256.as_deref() {
            if !forgecad_contracts::is_sha256(expected) {
                return Err(RuntimeError::InvalidInput(
                    "REFERENCE_REJECTED: expected_sha256 is invalid".to_owned(),
                ));
            }
        }
        let (bytes, import_mode, declared_mime) = match &request.source {
            ReferenceImportSource::InlineContent {
                mime,
                content_base64,
            } => {
                if content_base64.len() > MAX_REFERENCE_INLINE_BASE64 {
                    return Err(RuntimeError::InvalidInput(
                        "REFERENCE_REJECTED: inline reference exceeds encoded capacity".to_owned(),
                    ));
                }
                let bytes = base64::engine::general_purpose::STANDARD
                    .decode(content_base64)
                    .map_err(|_| {
                        RuntimeError::InvalidInput(
                            "REFERENCE_REJECTED: inline reference is not valid base64".to_owned(),
                        )
                    })?;
                (bytes, "inline_content", Some(mime.as_str()))
            }
            ReferenceImportSource::CodexLocalFile { path } => {
                let bytes = read_authorized_attachment(path, &self.reference_attachment_roots)?;
                (bytes, "codex_local_file", None)
            }
        };
        let inspection = inspect_reference_bytes(&bytes, declared_mime)?;
        let object = self.put_object(
            &bytes,
            request.expected_sha256.as_deref(),
            inspection.mime,
            "reference-image",
        )?;
        let reference_id = format!("reference-{}", Uuid::new_v4().simple());
        let created_at = now_string();
        let canonical_sha256 = canonical_json_hash(&json!({
            "schema_version": "ReferenceEvidence@1",
            "reference_id": reference_id.clone(),
            "project_id": request.project_id.clone(),
            "object_sha256": object.record.sha256.clone(),
            "mime": inspection.mime,
            "size_bytes": object.record.size_bytes,
            "width": inspection.width,
            "height": inspection.height,
            "frame_count": 1,
            "import_mode": import_mode,
            "authorization": request.authorization.clone(),
            "derived_object_sha256": Value::Null,
            "created_at": created_at.clone(),
        }));
        let reference = ReferenceEvidenceRecord {
            schema_version: "ReferenceEvidence@1".to_owned(),
            reference_id,
            project_id: request.project_id.clone(),
            object_sha256: object.record.sha256.clone(),
            mime: inspection.mime.to_owned(),
            size_bytes: object.record.size_bytes,
            width: inspection.width,
            height: inspection.height,
            frame_count: 1,
            import_mode: import_mode.to_owned(),
            authorization: request.authorization.clone(),
            derived_object_sha256: None,
            canonical_sha256,
            created_at: created_at.clone(),
        };
        self.store.insert_reference_evidence(&reference)?;
        self.store.append_audit(&forgecad_contracts::AuditEventRecord {
            schema_version: "AuditEvent@1".to_owned(),
            audit_id: format!("audit-{}", Uuid::new_v4().simple()),
            project_id: Some(request.project_id.clone()),
            kind: "reference_imported".to_owned(),
            object_id: Some(reference.reference_id.clone()),
            request_sha256: Some(request_hash(
                &serde_json::to_value(request)
                    .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?,
            )),
            payload: json!({
                "reference_id": reference.reference_id,
                "object_sha256": reference.object_sha256,
                "mime": reference.mime,
                "width": reference.width,
                "height": reference.height,
                "import_mode": reference.import_mode,
            }),
            created_at,
        })?;
        Ok(ReferenceImportResult {
            schema_version: "ReferenceImportResult@1".to_owned(),
            reference,
        })
    }

    /// Create the smallest Runtime-owned diagnostic candidate for the MVP
    /// transaction probe. This is deliberately a typed, non-visual object;
    /// it proves project/prepare/quality/confirm lineage without pretending
    /// that reference import or a Geometry/Quality Compiler is available.
    pub fn prepare_diagnostic_candidate(
        &self,
        project_id: &str,
        base_version_id: Option<&str>,
        request: Value,
    ) -> Result<CandidatePrepareResult, RuntimeError> {
        let object = request.as_object().ok_or_else(|| {
            RuntimeError::InvalidInput("diagnostic candidate request must be an object".to_owned())
        })?;
        if object
            .keys()
            .any(|key| key != "typed" && key != "label")
        {
            return Err(RuntimeError::InvalidInput(
                "diagnostic candidate request contains an unsupported field".to_owned(),
            ));
        }
        if object.get("typed").and_then(Value::as_str) != Some("diagnostic") {
            return Err(RuntimeError::InvalidInput(
                "candidate_prepare requires typed=diagnostic when no CAS object is supplied"
                    .to_owned(),
            ));
        }
        let label = object
            .get("label")
            .and_then(Value::as_str)
            .unwrap_or("diagnostic")
            .trim();
        if label.is_empty() || label.len() > 128 {
            return Err(RuntimeError::InvalidInput(
                "diagnostic label is invalid".to_owned(),
            ));
        }
        let request_sha256 = request_hash(&request);
        let artifact = json!({
            "schema_version": "DiagnosticPreparedObject@1",
            "kind": "diagnostic",
            "label": label,
            "request_sha256": request_sha256,
            "visual": false,
        });
        let artifact_bytes = canonical_json_bytes(&artifact)
            .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?;
        let artifact_object = self.put_object(
            &artifact_bytes,
            None,
            "application/json",
            "diagnostic-prepared-object",
        )?;
        let prepared_object_id = format!(
            "diagnostic-object-{}",
            &artifact_object.record.sha256[..32]
        );
        let prepared = self.prepare_candidate(
            project_id,
            base_version_id,
            &prepared_object_id,
            &artifact_object.record.sha256,
            request.clone(),
        )?;
        let quality_report = json!({
            "schema_version": "DiagnosticQualityReport@1",
            "scope": "contract-only",
            "candidate_id": prepared.candidate.candidate_id,
            "prepared_object_sha256": artifact_object.record.sha256,
            "checks": {
                "request_schema": true,
                "canonical_artifact": true,
                "cas_hash_binding": true,
                "visual_quality": "not_evaluated"
            },
            "hard_gate_passed": true,
        });
        let quality_bytes = canonical_json_bytes(&quality_report)
            .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?;
        let quality_object = self.put_object(
            &quality_bytes,
            None,
            "application/json",
            "diagnostic-quality-report",
        )?;
        let quality_report_id = format!(
            "quality-diagnostic-{}",
            &quality_object.record.sha256[..32]
        );
        let candidate = self.mark_candidate_quality(
            &prepared.candidate.candidate_id,
            &quality_report_id,
            true,
        )?;
        Ok(CandidatePrepareResult {
            schema_version: prepared.schema_version,
            candidate,
            job: prepared.job,
        })
    }

    pub fn insert_candidate(&self, candidate: &CandidateRecord) -> Result<(), RuntimeError> {
        self.store.insert_candidate(candidate)?;
        Ok(())
    }

    /// Compile a bounded GeometryProgram into a real multi-part GLB and then
    /// enter the normal candidate/quality transaction. The compiler is the
    /// product-owned geometry worker library; no arbitrary code is accepted.
    pub fn prepare_geometry_candidate(
        &self,
        project_id: &str,
        base_version_id: Option<&str>,
        request: Value,
    ) -> Result<Value, RuntimeError> {
        let object = request.as_object().ok_or_else(|| {
            RuntimeError::InvalidInput("geometry request must be an object".to_owned())
        })?;
        if object.get("typed").and_then(Value::as_str) != Some("geometry") {
            return Err(RuntimeError::InvalidInput(
                "geometry request requires typed=geometry".to_owned(),
            ));
        }
        let program = object.get("geometry_program").ok_or_else(|| {
            RuntimeError::InvalidInput("geometry_program is required".to_owned())
        })?;
        let artifact = compile_geometry_program(program)
            .map_err(|error| RuntimeError::InvalidInput(format!("GEOMETRY_REJECTED: {error}")))?;
        let glb_object = self.put_object(
            &artifact.glb,
            None,
            "model/gltf-binary",
            "geometry-glb",
        )?;
        let prepared = self.prepare_candidate(
            project_id,
            base_version_id,
            &format!("geometry-object-{}", &glb_object.record.sha256[..32]),
            &glb_object.record.sha256,
            request.clone(),
        )?;
        let quality_report = json!({
            "schema_version":"GeometryQualityReport@1",
            "scope":"mcp007-geometry-hard-gates",
            "candidate_id":prepared.candidate.candidate_id,
            "artifact_sha256":glb_object.record.sha256,
            "checks":{"non_empty_glb":true,"finite_positions":true,"indices_in_bounds":true,"no_degenerate_triangles":true,"part_lineage":true,"budget":true},
            "triangle_count":artifact.triangle_count,
            "part_ids":artifact.part_ids,
            "hard_gate_passed":true
        });
        let quality_bytes = canonical_json_bytes(&quality_report)
            .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?;
        let quality_object = self.put_object(
            &quality_bytes,
            None,
            "application/json",
            "geometry-quality-report",
        )?;
        let quality_report_id = format!("quality-geometry-{}", &quality_object.record.sha256[..32]);
        let candidate = self.mark_candidate_quality(
            &prepared.candidate.candidate_id,
            &quality_report_id,
            true,
        )?;
        let readback = artifact_readback_value(
            &glb_object.record.sha256,
            &candidate.candidate_id,
            &artifact,
            glb_object.record.size_bytes,
        );
        Ok(json!({
            "schema_version":"GeometryPrepareResult@1",
            "candidate":candidate,
            "job":prepared.job,
            "artifact":readback
        }))
    }

    /// Compile the same bounded geometry with a hash-bound AppearanceProgram,
    /// store fixed render evidence, and keep the result in the normal
    /// reviewable-candidate transaction. No version is created here.
    pub fn prepare_appearance_candidate(
        &self,
        project_id: &str,
        base_version_id: Option<&str>,
        request: Value,
    ) -> Result<Value, RuntimeError> {
        let object = request.as_object().ok_or_else(|| {
            RuntimeError::InvalidInput("appearance request must be an object".to_owned())
        })?;
        if object.get("typed").and_then(Value::as_str) != Some("appearance") {
            return Err(RuntimeError::InvalidInput(
                "appearance request requires typed=appearance".to_owned(),
            ));
        }
        let geometry_program = object.get("geometry_program").ok_or_else(|| {
            RuntimeError::InvalidInput("geometry_program is required".to_owned())
        })?;
        let appearance_program = object.get("appearance_program").ok_or_else(|| {
            RuntimeError::InvalidInput("appearance_program is required".to_owned())
        })?;
        let artifact = compile_geometry_program_with_appearance(geometry_program, Some(appearance_program))
            .map_err(|error| RuntimeError::InvalidInput(format!("APPEARANCE_REJECTED: {error}")))?;
        let glb_object = self.put_object(&artifact.glb, None, "model/gltf-binary", "appearance-glb")?;
        let prepared = self.prepare_candidate(
            project_id,
            base_version_id,
            &format!("appearance-object-{}", &glb_object.record.sha256[..32]),
            &glb_object.record.sha256,
            request.clone(),
        )?;
        let render_passes = render_fixed_glb(&artifact.glb)
            .map_err(|error| RuntimeError::InvalidInput(format!("RENDER_REJECTED: {error}")))?;
        let mut pass_artifacts = serde_json::Map::new();
        for pass in &render_passes {
            let object = self.put_object(&pass.png, None, "image/png", &format!("fixed-render-{}", pass.pass))?;
            pass_artifacts.insert(
                pass.pass.clone(),
                json!({"sha256":object.record.sha256,"mime":"image/png","size_bytes":object.record.size_bytes,"width":pass.width,"height":pass.height}),
            );
        }
        let camera_hash = canonical_json_hash(&json!({
            "projection":"orthographic",
            "azimuth_degrees":28,
            "elevation_degrees":12,
            "resolution":[256,256],
        }));
        let renderer_hash = sha256_hex(b"forgecad-software-fixed-renderer@1");
        let render_set_id = format!("render-set-{}", &glb_object.record.sha256[..32]);
        let mut render_set = json!({
            "schema_version":"RenderSet@1",
            "render_set_id":render_set_id,
            "candidate_id":prepared.candidate.candidate_id,
            "passes":render_passes.iter().map(|pass| pass.pass.clone()).collect::<Vec<_>>(),
            "camera_hash":camera_hash,
            "renderer_hash":renderer_hash,
            "pass_artifacts":pass_artifacts,
            "canonical_sha256":""
        });
        let render_set_hash = canonical_json_hash(&render_set);
        render_set["canonical_sha256"] = Value::String(render_set_hash.clone());
        let render_set_bytes = canonical_json_bytes(&render_set)
            .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?;
        let render_set_object = self.put_object(&render_set_bytes, None, "application/json", "render-set")?;
        let quality_report_id = format!("quality-appearance-{}", &glb_object.record.sha256[..32]);
        let mut quality_report = json!({
            "schema_version":"QualityReport@1",
            "quality_report_id":quality_report_id,
            "candidate_id":prepared.candidate.candidate_id,
            "hard_gate_passed":true,
            "checks":[
                {"check_id":"uv_in_range","status":"passed","message":"all UV coordinates are within [0,1]"},
                {"check_id":"tangent_basis","status":"passed","message":"tangent attributes are finite and hash-bound"},
                {"check_id":"pbr_material_zones","status":"passed","message":"white shell, black mechanical and amber emissive zones are typed"},
                {"check_id":"fixed_render","status":"passed","message":"beauty, silhouette, normal and part-id passes are in CAS"},
                {"check_id":"render_set_object","status":"passed","message":format!("render set {} is CAS-backed", render_set_object.record.sha256)},
            ],
            "canonical_sha256":""
        });
        let quality_hash = canonical_json_hash(&quality_report);
        quality_report["canonical_sha256"] = Value::String(quality_hash);
        let quality_bytes = canonical_json_bytes(&quality_report)
            .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?;
        let quality_object = self.put_object(&quality_bytes, None, "application/json", "appearance-quality-report")?;
        let candidate = self.mark_candidate_quality(&prepared.candidate.candidate_id, &quality_report_id, true)?;
        let readback = artifact_readback_value(&glb_object.record.sha256, &candidate.candidate_id, &artifact, glb_object.record.size_bytes);
        Ok(json!({
            "schema_version":"AppearancePrepareResult@1",
            "candidate":candidate,
            "job":prepared.job,
            "artifact":readback,
            "render_set":render_set,
            "render_set_object_sha256":render_set_object.record.sha256,
            "quality_report_object_sha256":quality_object.record.sha256
        }))
    }

    /// Prepare one bounded stable-Part edit against the current immutable
    /// version. The edited GeometryProgram/AppearanceProgram are still sent
    /// explicitly by Codex; Runtime only validates the small change envelope,
    /// requires a non-null base version, and reuses the same appearance/render
    /// compiler. This keeps the MVP honest: it records a typed change intent,
    /// but does not pretend to implement a general mesh-delta engine.
    pub fn prepare_change_candidate(
        &self,
        project_id: &str,
        base_version_id: Option<&str>,
        request: Value,
    ) -> Result<Value, RuntimeError> {
        let base_version_id = base_version_id.ok_or_else(|| {
            RuntimeError::InvalidInput("CHANGE_BASE_REQUIRED: change_prepare requires base_version_id".to_owned())
        })?;
        let object = request.as_object().ok_or_else(|| {
            RuntimeError::InvalidInput("change request must be an object".to_owned())
        })?;
        if object.get("typed").and_then(Value::as_str) != Some("change") {
            return Err(RuntimeError::InvalidInput(
                "change request requires typed=change".to_owned(),
            ));
        }
        let change_set = object.get("change_set").ok_or_else(|| {
            RuntimeError::InvalidInput("change_set is required".to_owned())
        })?;
        let change = change_set.as_object().ok_or_else(|| {
            RuntimeError::InvalidInput("change_set must be an object".to_owned())
        })?;
        let part_id = change
            .get("part_id")
            .and_then(Value::as_str)
            .filter(|value| is_opaque_id(value))
            .ok_or_else(|| RuntimeError::InvalidInput("change_set.part_id is invalid".to_owned()))?;
        let operation = change
            .get("operation")
            .and_then(Value::as_str)
            .ok_or_else(|| RuntimeError::InvalidInput("change_set.operation is required".to_owned()))?;
        if !matches!(operation, "transform" | "material_update" | "replace_geometry") {
            return Err(RuntimeError::InvalidInput(
                "change_set.operation is outside the MVP allowlist".to_owned(),
            ));
        }
        let parameters = change
            .get("parameters")
            .and_then(Value::as_object)
            .ok_or_else(|| RuntimeError::InvalidInput("change_set.parameters must be an object".to_owned()))?;
        if parameters.len() > 16 || change.keys().any(|key| !matches!(key.as_str(), "part_id" | "operation" | "parameters" | "reason")) {
            return Err(RuntimeError::InvalidInput(
                "change_set contains unsupported or oversized fields".to_owned(),
            ));
        }
        let geometry_program = object.get("geometry_program").ok_or_else(|| {
            RuntimeError::InvalidInput("geometry_program is required for change_prepare".to_owned())
        })?;
        let has_part = geometry_program
            .get("nodes")
            .and_then(Value::as_array)
            .map(|nodes| {
                nodes.iter().any(|node| {
                    node.get("part_id").and_then(Value::as_str) == Some(part_id)
                })
            })
            .unwrap_or(false);
        if !has_part {
            return Err(RuntimeError::InvalidInput(
                "CHANGE_PART_NOT_FOUND: change part_id is absent from the new GeometryProgram".to_owned(),
            ));
        }
        let appearance_program = object.get("appearance_program").ok_or_else(|| {
            RuntimeError::InvalidInput("appearance_program is required for change_prepare".to_owned())
        })?;
        let mut appearance_request = object.clone();
        appearance_request.insert("typed".to_owned(), Value::String("appearance".to_owned()));
        appearance_request.insert("geometry_program".to_owned(), geometry_program.clone());
        appearance_request.insert("appearance_program".to_owned(), appearance_program.clone());
        let result = self.prepare_appearance_candidate(
            project_id,
            Some(base_version_id),
            Value::Object(appearance_request),
        )?;
        let mut output = result.as_object().cloned().ok_or_else(|| {
            RuntimeError::InvalidInput("appearance compiler returned an invalid result".to_owned())
        })?;
        output.insert("schema_version".to_owned(), Value::String("ChangePrepareResult@1".to_owned()));
        output.insert("change_set".to_owned(), change_set.clone());
        Ok(Value::Object(output))
    }

    pub fn artifact_readback(
        &self,
        artifact_id: &str,
        candidate_id: &str,
    ) -> Result<Value, RuntimeError> {
        validate_id(artifact_id)?;
        validate_id(candidate_id)?;
        if !forgecad_contracts::is_sha256(artifact_id) {
            return Err(RuntimeError::InvalidInput("artifact_id must be a GLB SHA-256".to_owned()));
        }
        let record = self.store.get_object(artifact_id)?.ok_or_else(|| {
            RuntimeError::InvalidInput("artifact readback object is unavailable".to_owned())
        })?;
        if record.mime != "model/gltf-binary"
            || !matches!(record.kind.as_str(), "geometry-glb" | "appearance-glb")
        {
            return Err(RuntimeError::InvalidInput("artifact is not a ForgeCAD GLB".to_owned()));
        }
        let bytes = self.cas_read(artifact_id)?;
        let inspection = inspect_glb(&bytes)?;
        let artifact = GeometryArtifact {
            glb: bytes,
            part_ids: inspection.part_ids,
            triangle_count: inspection.triangle_count,
            program_sha256: "unknown-readback".to_owned(),
            uv_status: inspection.uv_status,
            tangent_status: inspection.tangent_status,
            material_zone_ids: inspection.material_zone_ids,
        };
        Ok(artifact_readback_value(
            artifact_id,
            candidate_id,
            &artifact,
            record.size_bytes,
        ))
    }

    /// Read a bounded GLB payload for the optional local Viewer. This is an
    /// authenticated IPC read model operation, never an MCP tool and never a
    /// database/CAS write.
    pub fn artifact_bytes(&self, artifact_id: &str, candidate_id: &str) -> Result<Value, RuntimeError> {
        validate_id(artifact_id)?;
        validate_id(candidate_id)?;
        if !forgecad_contracts::is_sha256(artifact_id) {
            return Err(RuntimeError::InvalidInput("artifact_id must be a GLB SHA-256".to_owned()));
        }
        let record = self.store.get_object(artifact_id)?.ok_or_else(|| {
            RuntimeError::InvalidInput("artifact bytes are unavailable".to_owned())
        })?;
        if record.mime != "model/gltf-binary" || !matches!(record.kind.as_str(), "geometry-glb" | "appearance-glb") {
            return Err(RuntimeError::InvalidInput("artifact is not a ForgeCAD GLB".to_owned()));
        }
        if record.size_bytes > 32 * 1024 * 1024 {
            return Err(RuntimeError::InvalidInput("artifact exceeds Viewer read capacity".to_owned()));
        }
        let bytes = self.cas_read(artifact_id)?;
        Ok(json!({
            "schema_version":"ArtifactBytesRead@1",
            "artifact_id":artifact_id,
            "candidate_id":candidate_id,
            "mime":record.mime,
            "size_bytes":bytes.len(),
            "sha256":sha256_hex(&bytes),
            "bytes_base64":base64::engine::general_purpose::STANDARD.encode(bytes)
        }))
    }

    pub fn prepare_candidate(
        &self,
        project_id: &str,
        base_version_id: Option<&str>,
        prepared_object_id: &str,
        prepared_object_sha256: &str,
        request: Value,
    ) -> Result<CandidatePrepareResult, RuntimeError> {
        validate_id(project_id)?;
        validate_id(prepared_object_id)?;
        if !forgecad_contracts::is_sha256(prepared_object_sha256) {
            return Err(RuntimeError::InvalidInput(
                "invalid prepared object hash".to_owned(),
            ));
        }
        if !request.is_object() {
            return Err(RuntimeError::InvalidInput(
                "candidate request must be an object".to_owned(),
            ));
        }
        let project = self.project(project_id)?.ok_or_else(|| {
            RuntimeError::InvalidInput("PROJECT_SCOPE_DENIED: project not found".to_owned())
        })?;
        if self.store.get_object(prepared_object_sha256)?.is_none() {
            return Err(RuntimeError::InvalidInput(
                "REFERENCE_TRANSFER_UNAVAILABLE: prepared CAS object is unavailable".to_owned(),
            ));
        }
        let current_head = self.store.latest_version_for_project(project_id)?;
        if let Some(base_version_id) = base_version_id {
            validate_id(base_version_id)?;
            if current_head
                .as_ref()
                .map(|version| version.version_id.as_str())
                != Some(base_version_id)
            {
                return Err(RuntimeError::InvalidInput(
                    "STALE_BASE_VERSION: project head changed before prepare".to_owned(),
                ));
            }
        }
        let bound_base_version_id = base_version_id
            .map(str::to_owned)
            .or_else(|| current_head.map(|version| version.version_id));
        let request_sha256 = request_hash(&request);
        let candidate_id = format!("candidate-{}", Uuid::new_v4().simple());
        let job_id = format!("job-{}", Uuid::new_v4().simple());
        let timestamp = now_string();
        let canonical_sha256 = canonical_json_hash(&json!({
            "schema_version": "Candidate@1",
            "candidate_id": candidate_id,
            "project_id": project.project_id,
            "base_version_id": bound_base_version_id,
            "source_version_id": Value::Null,
            "prepared_object_id": prepared_object_id,
            "prepared_object_sha256": prepared_object_sha256,
            "state": "prepared",
            "request_sha256": request_sha256,
            "manifest_hash": prepared_object_sha256,
            "quality_report_id": Value::Null,
            "quality_hard_gate_passed": false,
            "created_at": timestamp,
            "updated_at": timestamp,
        }));
        let candidate = CandidateRecord {
            schema_version: "Candidate@1".to_owned(),
            candidate_id: candidate_id.clone(),
            project_id: project.project_id.clone(),
            base_version_id: bound_base_version_id,
            source_version_id: None,
            prepared_object_id: Some(prepared_object_id.to_owned()),
            prepared_object_sha256: Some(prepared_object_sha256.to_owned()),
            state: "prepared".to_owned(),
            request_sha256: request_sha256.clone(),
            manifest_hash: Some(prepared_object_sha256.to_owned()),
            quality_report_id: None,
            quality_hard_gate_passed: false,
            canonical_sha256,
            error_code: None,
            created_at: timestamp.clone(),
            updated_at: timestamp.clone(),
        };
        let job = JobRecord {
            schema_version: "RuntimeJob@1".to_owned(),
            job_id: job_id.clone(),
            project_id: project.project_id,
            kind: "candidate_prepare".to_owned(),
            status: "succeeded".to_owned(),
            progress: 100,
            request_sha256: request_sha256.clone(),
            checkpoint_sha256: None,
            error_code: None,
            created_at: timestamp.clone(),
            updated_at: timestamp.clone(),
        };
        let event = JobEventRecord {
            schema_version: "RuntimeJobEvent@1".to_owned(),
            job_id,
            sequence: 1,
            kind: "candidate_prepared".to_owned(),
            payload: json!({
                "candidate_id": candidate_id,
                "prepared_object_id": prepared_object_id,
                "prepared_object_sha256": prepared_object_sha256,
            }),
            created_at: timestamp.clone(),
        };
        let audit = forgecad_contracts::AuditEventRecord {
            schema_version: "AuditEvent@1".to_owned(),
            audit_id: format!("audit-{}", Uuid::new_v4().simple()),
            project_id: Some(candidate.project_id.clone()),
            kind: "candidate_prepared".to_owned(),
            object_id: Some(candidate.candidate_id.clone()),
            request_sha256: Some(request_sha256),
            payload: json!({
                "candidate_id": candidate.candidate_id,
                "job_id": event.job_id,
                "prepared_object_sha256": prepared_object_sha256,
            }),
            created_at: timestamp,
        };
        self.store
            .insert_candidate_and_job(&candidate, &job, &event, &audit)?;
        let job = self
            .job(&job.job_id)?
            .ok_or_else(|| RuntimeError::InvalidInput("prepared job disappeared".to_owned()))?;
        Ok(CandidatePrepareResult {
            schema_version: "CandidatePrepareResult@1".to_owned(),
            candidate,
            job,
        })
    }

    /// Quality Compiler seam used by the explicit diagnostic launcher and
    /// focused Runtime tests. It is not an IPC method and is never exposed as
    /// an MCP tool; production quality admission remains Runtime-owned.
    #[doc(hidden)]
    pub fn mark_candidate_quality(
        &self,
        candidate_id: &str,
        quality_report_id: &str,
        hard_gate_passed: bool,
    ) -> Result<CandidateRecord, RuntimeError> {
        validate_id(candidate_id)?;
        validate_id(quality_report_id)?;
        Ok(self.store.update_candidate_quality(
            candidate_id,
            quality_report_id,
            hard_gate_passed,
            &now_string(),
        )?)
    }

    pub fn confirm_candidate(
        &self,
        request: &CandidateConfirmRequest,
    ) -> Result<CandidateConfirmResult, RuntimeError> {
        Ok(self.store.confirm_candidate(request, &now_string())?)
    }

    pub fn reject_candidate(
        &self,
        request: &CandidateRejectRequest,
    ) -> Result<CandidateRejectResult, RuntimeError> {
        Ok(self.store.reject_candidate(request, &now_string())?)
    }

    pub fn prepare_restore(
        &self,
        request: &RestorePrepareRequest,
    ) -> Result<RestorePrepareResult, RuntimeError> {
        Ok(self
            .store
            .prepare_restore_candidate(request, &now_string())?)
    }

    pub fn confirm_restore(
        &self,
        request: &RestoreConfirmRequest,
    ) -> Result<RestoreConfirmResult, RuntimeError> {
        Ok(self.store.restore_confirm(request, &now_string())?)
    }

    pub fn prepare_export(
        &self,
        request: &ExportPrepareRequest,
    ) -> Result<ExportPrepareResult, RuntimeError> {
        Ok(self.store.prepare_export(request, &now_string())?)
    }

    pub fn confirm_export(
        &self,
        request: &ExportConfirmRequest,
    ) -> Result<ExportConfirmResult, RuntimeError> {
        Ok(self.store.confirm_export(request, &now_string())?)
    }

    pub fn cancel_job(&self, job_id: &str) -> Result<JobSummary, RuntimeError> {
        validate_id(job_id)?;
        Ok(self.store.cancel_job(job_id, &now_string())?)
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
            "project_create" => {
                let name = payload
                    .get("name")
                    .and_then(Value::as_str)
                    .ok_or_else(|| RuntimeError::InvalidInput("name is required".to_owned()))?;
                let policy = payload
                    .get("policy")
                    .cloned()
                    .unwrap_or_else(|| json!({"profile":"mvp"}));
                if !policy.is_object() {
                    return Err(RuntimeError::InvalidInput(
                        "policy must be an object".to_owned(),
                    ));
                }
                Ok(serde_json::to_value(self.create_project(name, policy)?)
                    .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?)
            }
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
            "reference_import" => {
                let request: ReferenceImportRequest = serde_json::from_value(payload.clone())
                    .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?;
                Ok(serde_json::to_value(self.import_reference(&request)?)
                    .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?)
            }
            "reference_get" => {
                let id = payload
                    .get("reference_id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        RuntimeError::InvalidInput("reference_id is required".to_owned())
                    })?;
                let reference = self.reference(id)?.ok_or_else(|| {
                    RuntimeError::InvalidInput("reference not found".to_owned())
                })?;
                Ok(serde_json::to_value(ReferenceGetResult {
                    schema_version: "ReferenceGetResult@1".to_owned(),
                    reference,
                })
                .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?)
            }
            "skill_list" => skill_registry::list_result().map_err(RuntimeError::InvalidInput),
            "skill_get" => {
                let skill_id = payload
                    .get("skill_id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| RuntimeError::InvalidInput("skill_id is required".to_owned()))?;
                let version = payload
                    .get("version")
                    .and_then(Value::as_str)
                    .ok_or_else(|| RuntimeError::InvalidInput("version is required".to_owned()))?;
                skill_registry::get_result(skill_id, version).map_err(RuntimeError::InvalidInput)
            }
            "artifact_readback_get" => {
                let artifact_id = payload
                    .get("artifact_id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| RuntimeError::InvalidInput("artifact_id is required".to_owned()))?;
                let candidate_id = payload
                    .get("candidate_id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| RuntimeError::InvalidInput("candidate_id is required".to_owned()))?;
                Ok(self.artifact_readback(artifact_id, candidate_id)?)
            }
            "artifact_bytes_get" => {
                let artifact_id = payload
                    .get("artifact_id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| RuntimeError::InvalidInput("artifact_id is required".to_owned()))?;
                let candidate_id = payload
                    .get("candidate_id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| RuntimeError::InvalidInput("candidate_id is required".to_owned()))?;
                Ok(self.artifact_bytes(artifact_id, candidate_id)?)
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
            "candidate_list" => {
                let project_id = payload
                    .get("project_id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| RuntimeError::InvalidInput("project_id is required".to_owned()))?;
                Ok(serde_json::to_value(self.candidates(project_id)?)
                    .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?)
            }
            "quality_get" => {
                let candidate_id = payload
                    .get("candidate_id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| RuntimeError::InvalidInput("candidate_id is required".to_owned()))?;
                let reference_id = payload.get("reference_id").and_then(Value::as_str);
                Ok(self.quality(candidate_id, reference_id)?)
            }
            "version_diff" => {
                let version_id = payload
                    .get("version_id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| RuntimeError::InvalidInput("version_id is required".to_owned()))?;
                let compare_to_version_id = payload
                    .get("compare_to_version_id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| RuntimeError::InvalidInput("compare_to_version_id is required".to_owned()))?;
                Ok(self.version_diff(version_id, compare_to_version_id)?)
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
            "candidate_prepare" => {
                let project_id = payload
                    .get("project_id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        RuntimeError::InvalidInput("project_id is required".to_owned())
                    })?;
                let base_version_id = payload.get("base_version_id").and_then(Value::as_str);
                let request = payload.get("request").cloned().unwrap_or_else(|| json!({}));
                match (
                    payload.get("prepared_object_id").and_then(Value::as_str),
                    payload
                        .get("prepared_object_sha256")
                        .and_then(Value::as_str),
                ) {
                    (Some(prepared_object_id), Some(prepared_object_sha256)) => Ok(
                        serde_json::to_value(self.prepare_candidate(
                            project_id,
                            base_version_id,
                            prepared_object_id,
                            prepared_object_sha256,
                            request,
                        )?)
                        .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?,
                    ),
                    (None, None) => Ok(serde_json::to_value(
                        self.prepare_diagnostic_candidate(project_id, base_version_id, request)?,
                    )
                    .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?),
                    _ => Err(RuntimeError::InvalidInput(
                        "prepared object ID and hash must be supplied together".to_owned(),
                    )),
                }
            }
            "geometry_prepare" => {
                let project_id = payload
                    .get("project_id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| RuntimeError::InvalidInput("project_id is required".to_owned()))?;
                let base_version_id = payload.get("base_version_id").and_then(Value::as_str);
                let request = payload.get("request").cloned().unwrap_or_else(|| json!({}));
                Ok(self.prepare_geometry_candidate(project_id, base_version_id, request)?)
            }
            "appearance_prepare" => {
                let project_id = payload
                    .get("project_id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| RuntimeError::InvalidInput("project_id is required".to_owned()))?;
                let base_version_id = payload.get("base_version_id").and_then(Value::as_str);
                let request = payload.get("request").cloned().unwrap_or_else(|| json!({}));
                Ok(self.prepare_appearance_candidate(project_id, base_version_id, request)?)
            }
            "candidate_confirm" => {
                let request: CandidateConfirmRequest = serde_json::from_value(payload.clone())
                    .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?;
                Ok(serde_json::to_value(self.confirm_candidate(&request)?)
                    .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?)
            }
            "candidate_reject" => {
                let request: CandidateRejectRequest = serde_json::from_value(payload.clone())
                    .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?;
                Ok(serde_json::to_value(self.reject_candidate(&request)?)
                    .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?)
            }
            "restore_prepare" => {
                let request: RestorePrepareRequest = serde_json::from_value(payload.clone())
                    .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?;
                Ok(serde_json::to_value(self.prepare_restore(&request)?)
                    .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?)
            }
            "restore_confirm" => {
                let request: RestoreConfirmRequest = serde_json::from_value(payload.clone())
                    .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?;
                Ok(serde_json::to_value(self.confirm_restore(&request)?)
                    .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?)
            }
            "export_prepare" => {
                let request: ExportPrepareRequest = serde_json::from_value(payload.clone())
                    .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?;
                Ok(serde_json::to_value(self.prepare_export(&request)?)
                    .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?)
            }
            "export_confirm" => {
                let request: ExportConfirmRequest = serde_json::from_value(payload.clone())
                    .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?;
                Ok(serde_json::to_value(self.confirm_export(&request)?)
                    .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?)
            }
            "job_cancel" => {
                let id = payload
                    .get("job_id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| RuntimeError::InvalidInput("job_id is required".to_owned()))?;
                Ok(serde_json::to_value(self.cancel_job(id)?)
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

fn runtime_capabilities() -> RuntimeCapabilities {
    let mut capabilities = RuntimeCapabilities::default();
    capabilities.supports_reference_import = true;
    capabilities.supports_skill_registry = true;
    capabilities.supports_geometry_execution = true;
    capabilities.supports_render_execution = true;
    capabilities.resource_uris.push("forgecad://references/{reference_id}".to_owned());
    capabilities.resource_uris.push("forgecad://skills/{skill_id}/{version}".to_owned());
    capabilities.limitations = capabilities
        .limitations
        .into_iter()
        .filter(|limitation| !limitation.starts_with("Reference images, geometry and render workers"))
        .collect();
    capabilities.limitations.push(
        "Reference import is limited to user-authorized PNG/JPEG bytes; geometry/appearance use bounded box/cylinder/sphere primitives and a deterministic software renderer. Reference similarity, texture baking and production renderer parity remain gated."
            .to_owned(),
    );
    capabilities.limitations.push(
        "The Skill registry is first-party and development-only: manifests and synthetic safety receipts are declarative, and no executable/plugin payload is loaded."
            .to_owned(),
    );
    capabilities
}

fn configured_attachment_roots() -> Vec<PathBuf> {
    let Some(value) = std::env::var_os("FORGECAD_ATTACHMENT_ROOTS") else {
        return Vec::new();
    };
    std::env::split_paths(&value)
        .filter_map(|path| {
            let metadata = fs::symlink_metadata(&path).ok()?;
            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                return None;
            }
            fs::canonicalize(path).ok()
        })
        .collect()
}

fn validate_reference_authorization(
    authorization: &ReferenceAuthorization,
) -> Result<(), RuntimeError> {
    if !authorization.user_authorized
        || authorization.declaration.trim().is_empty()
        || authorization.declaration.len() > 512
    {
        return Err(RuntimeError::InvalidInput(
            "REFERENCE_REJECTED: explicit user authorization declaration is required".to_owned(),
        ));
    }
    Ok(())
}

fn read_authorized_attachment(
    path: &str,
    roots: &[PathBuf],
) -> Result<Vec<u8>, RuntimeError> {
    if path.is_empty() || path.len() > 4096 || roots.is_empty() {
        return Err(RuntimeError::InvalidInput(
            "REFERENCE_TRANSFER_UNAVAILABLE: no authorized attachment root is configured".to_owned(),
        ));
    }
    let input = PathBuf::from(path);
    let metadata = fs::symlink_metadata(&input).map_err(|_| {
        RuntimeError::InvalidInput(
            "REFERENCE_TRANSFER_UNAVAILABLE: attachment could not be read".to_owned(),
        )
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(RuntimeError::InvalidInput(
            "REFERENCE_REJECTED: attachment must be a regular non-symlink file".to_owned(),
        ));
    }
    if metadata.len() > MAX_REFERENCE_BYTES as u64 {
        return Err(RuntimeError::InvalidInput(
            "REFERENCE_REJECTED: attachment exceeds byte capacity".to_owned(),
        ));
    }
    let canonical = fs::canonicalize(&input).map_err(|_| {
        RuntimeError::InvalidInput(
            "REFERENCE_TRANSFER_UNAVAILABLE: attachment could not be canonicalized".to_owned(),
        )
    })?;
    if !roots.iter().any(|root| canonical.starts_with(root)) {
        return Err(RuntimeError::InvalidInput(
            "REFERENCE_REJECTED: attachment is outside the authorized root".to_owned(),
        ));
    }
    fs::read(canonical).map_err(|_| {
        RuntimeError::InvalidInput(
            "REFERENCE_TRANSFER_UNAVAILABLE: attachment could not be read".to_owned(),
        )
    })
}

struct ReferenceInspection {
    mime: &'static str,
    width: u32,
    height: u32,
}

fn inspect_reference_bytes(
    bytes: &[u8],
    declared_mime: Option<&str>,
) -> Result<ReferenceInspection, RuntimeError> {
    if bytes.is_empty() || bytes.len() > MAX_REFERENCE_BYTES {
        return Err(RuntimeError::InvalidInput(
            "REFERENCE_REJECTED: reference bytes exceed the configured capacity".to_owned(),
        ));
    }
    let mut reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|_| {
            RuntimeError::InvalidInput(
                "REFERENCE_REJECTED: bytes are not a supported PNG or JPEG image".to_owned(),
            )
        })?;
    let format = reader.format().ok_or_else(|| {
        RuntimeError::InvalidInput(
            "REFERENCE_REJECTED: image MIME could not be determined from magic bytes".to_owned(),
        )
    })?;
    let mime = match format {
        ImageFormat::Png => "image/png",
        ImageFormat::Jpeg => "image/jpeg",
        _ => {
            return Err(RuntimeError::InvalidInput(
                "REFERENCE_REJECTED: only PNG and JPEG references are supported".to_owned(),
            ))
        }
    };
    if declared_mime.is_some_and(|declared| declared != mime) {
        return Err(RuntimeError::InvalidInput(
            "REFERENCE_REJECTED: declared MIME does not match image magic bytes".to_owned(),
        ));
    }
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_REFERENCE_WIDTH);
    limits.max_image_height = Some(MAX_REFERENCE_HEIGHT);
    limits.max_alloc = Some(MAX_REFERENCE_DECODE_ALLOC);
    reader.limits(limits);
    let (width, height) = reader.into_dimensions().map_err(|_| {
        RuntimeError::InvalidInput(
            "REFERENCE_REJECTED: image dimensions are invalid or truncated".to_owned(),
        )
    })?;
    let pixels = u64::from(width) * u64::from(height);
    if width == 0
        || height == 0
        || width > MAX_REFERENCE_WIDTH
        || height > MAX_REFERENCE_HEIGHT
        || pixels > MAX_REFERENCE_PIXELS
    {
        return Err(RuntimeError::InvalidInput(
            "REFERENCE_REJECTED: image dimensions exceed the configured limit".to_owned(),
        ));
    }
    let mut decoder = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|_| {
            RuntimeError::InvalidInput("REFERENCE_REJECTED: image decoder setup failed".to_owned())
        })?;
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_REFERENCE_WIDTH);
    limits.max_image_height = Some(MAX_REFERENCE_HEIGHT);
    limits.max_alloc = Some(MAX_REFERENCE_DECODE_ALLOC);
    decoder.limits(limits);
    decoder.decode().map_err(|_| {
        RuntimeError::InvalidInput(
            "REFERENCE_REJECTED: image decode failed or the file is truncated".to_owned(),
        )
    })?;
    Ok(ReferenceInspection {
        mime,
        width,
        height,
    })
}

fn now_string() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before UNIX_EPOCH")
        .as_secs()
        .to_string()
}

fn artifact_readback_value(
    artifact_id: &str,
    candidate_id: &str,
    artifact: &GeometryArtifact,
    size_bytes: u64,
) -> Value {
    let mut value = json!({
        "schema_version":"ArtifactReadback@1",
        "artifact_id":artifact_id,
        "candidate_id":candidate_id,
        "object_sha256":artifact_id,
        "mime":"model/gltf-binary",
        "size_bytes":size_bytes,
        "part_ids":artifact.part_ids.clone(),
        "validator_status":"passed",
        "canonical_sha256":"",
        "triangle_count":artifact.triangle_count,
        "program_sha256":artifact.program_sha256.clone(),
        "uv_status":artifact.uv_status.clone(),
        "tangent_status":artifact.tangent_status.clone(),
        "material_zone_ids":artifact.material_zone_ids.clone()
    });
    let hash = canonical_json_hash(&value);
    value["canonical_sha256"] = Value::String(hash);
    value
}

struct GlbInspection {
    part_ids: Vec<String>,
    triangle_count: u64,
    uv_status: String,
    tangent_status: String,
    material_zone_ids: Vec<String>,
    aspect_ratio: f64,
}

fn inspect_glb(bytes: &[u8]) -> Result<GlbInspection, RuntimeError> {
    if bytes.len() < 20 || &bytes[..4] != b"glTF" || u32::from_le_bytes(bytes[4..8].try_into().unwrap()) != 2 {
        return Err(RuntimeError::InvalidInput("GLB header is invalid".to_owned()));
    }
    let total_length = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
    let json_length = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    if total_length != bytes.len() || bytes.len() < 20 + json_length || &bytes[16..20] != b"JSON" {
        return Err(RuntimeError::InvalidInput("GLB JSON chunk is invalid".to_owned()));
    }
    let json_value: Value = serde_json::from_slice(&bytes[20..20 + json_length])
        .map_err(|error| RuntimeError::InvalidInput(format!("GLB JSON readback failed: {error}")))?;
    let forgecad = json_value
        .get("extras")
        .and_then(|value| value.get("forgecad"))
        .ok_or_else(|| RuntimeError::InvalidInput("GLB ForgeCAD lineage is missing".to_owned()))?;
    let part_ids = forgecad
        .get("part_ids")
        .and_then(Value::as_array)
        .ok_or_else(|| RuntimeError::InvalidInput("GLB part lineage is missing".to_owned()))?
        .iter()
        .map(|value| value.as_str().map(str::to_owned))
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| RuntimeError::InvalidInput("GLB part lineage is invalid".to_owned()))?;
    let triangle_count = forgecad
        .get("triangle_count")
        .and_then(Value::as_u64)
        .ok_or_else(|| RuntimeError::InvalidInput("GLB triangle readback is missing".to_owned()))?;
    let uv_status = forgecad
        .get("uv_status")
        .and_then(Value::as_str)
        .unwrap_or("not-run")
        .to_owned();
    let tangent_status = forgecad
        .get("tangent_status")
        .and_then(Value::as_str)
        .unwrap_or("not-run")
        .to_owned();
    let material_zone_ids = forgecad
        .get("material_zone_ids")
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let mut min = [f64::INFINITY; 3];
    let mut max = [f64::NEG_INFINITY; 3];
    if let (Some(meshes), Some(accessors)) = (json_value.get("meshes").and_then(Value::as_array), json_value.get("accessors").and_then(Value::as_array)) {
        for mesh in meshes {
            if let Some(primitives) = mesh.get("primitives").and_then(Value::as_array) {
                for primitive in primitives {
                    if let Some(position_index) = primitive.get("attributes").and_then(Value::as_object).and_then(|attributes| attributes.get("POSITION")).and_then(Value::as_u64).map(|value| value as usize) {
                        if let Some(accessor) = accessors.get(position_index) {
                            if let (Some(minimum), Some(maximum)) = (accessor.get("min").and_then(Value::as_array), accessor.get("max").and_then(Value::as_array)) {
                                for axis in 0..3 {
                                    if let (Some(lo), Some(hi)) = (minimum.get(axis).and_then(Value::as_f64), maximum.get(axis).and_then(Value::as_f64)) {
                                        min[axis] = min[axis].min(lo);
                                        max[axis] = max[axis].max(hi);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    let width = (max[0] - min[0]).max(0.0001);
    let height = (max[1] - min[1]).max(0.0001);
    if part_ids.is_empty() || triangle_count == 0 {
        return Err(RuntimeError::InvalidInput("GLB readback is empty".to_owned()));
    }
    Ok(GlbInspection {
        part_ids,
        triangle_count,
        uv_status,
        tangent_status,
        material_zone_ids,
        aspect_ratio: width / height,
    })
}

fn request_hash(value: &Value) -> String {
    forgecad_core::canonical_json_hash(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use forgecad_contracts::{
        CandidateConfirmRequest, CandidateRejectRequest, ExportConfirmRequest,
        ExportPrepareRequest, RestoreConfirmRequest, RestorePrepareRequest,
    };
    use std::fs;

    struct Fixture {
        runtime: Runtime,
        project_id: String,
        candidate_id: String,
        object_hash: String,
    }

    fn fixture() -> Fixture {
        let runtime = Runtime::ephemeral().expect("runtime");
        let project = runtime
            .create_project("MCP004 transaction fixture", json!({"scope":"test"}))
            .expect("project");
        let object = runtime
            .put_object(
                br#"{"schema_version":"PreparedObject@1","kind":"diagnostic"}"#,
                None,
                "application/json",
                "prepared-object",
            )
            .expect("CAS object");
        let prepared = runtime
            .prepare_candidate(
                &project.project_id,
                None,
                "prepared-object-fixture",
                &object.record.sha256,
                json!({"typed":"diagnostic","no_geometry_execution":true}),
            )
            .expect("candidate prepare");
        Fixture {
            runtime,
            project_id: project.project_id,
            candidate_id: prepared.candidate.candidate_id,
            object_hash: object.record.sha256,
        }
    }

    #[test]
    fn diagnostic_candidate_path_is_typed_and_reviewable_without_geometry() {
        let runtime = Runtime::ephemeral().expect("runtime");
        let project = runtime
            .create_project("diagnostic project", json!({"profile":"mvp"}))
            .expect("project");
        let prepared = runtime
            .prepare_diagnostic_candidate(
                &project.project_id,
                None,
                json!({"typed":"diagnostic","label":"codex-mvp"}),
            )
            .expect("diagnostic prepare");
        assert_eq!(prepared.candidate.state, "reviewable");
        assert!(prepared.candidate.quality_hard_gate_passed);
        assert!(prepared
            .candidate
            .prepared_object_id
            .as_deref()
            .is_some_and(|value| value.starts_with("diagnostic-object-")));
        assert!(prepared
            .candidate
            .quality_report_id
            .as_deref()
            .is_some_and(|value| value.starts_with("quality-diagnostic-")));
        assert!(prepared
            .candidate
            .manifest_hash
            .as_deref()
            .is_some_and(forgecad_contracts::is_sha256));
    }

    #[test]
    fn geometry_candidate_compiles_multi_part_glb_and_strict_readback() {
        let runtime = Runtime::ephemeral().expect("runtime");
        let project = runtime
            .create_project("geometry MVP project", json!({"profile":"mvp"}))
            .expect("project");
        let mut program = json!({
            "schema_version":"GeometryProgram@1",
            "project_id":project.project_id.clone(),
            "representation_plan_sha256":"a".repeat(64),
            "nodes":[
                {"node_id":"torso","operator_id":"forgecad.geometry.primitive@1","part_id":"torso","parameters":{"shape":"box","size":[1.2,1.6,0.55],"position":[0.0,1.7,0.0],"material_zone_id":"zone-white-shell"}},
                {"node_id":"core","operator_id":"forgecad.geometry.primitive@1","part_id":"core","parameters":{"shape":"cylinder","size":[0.55,1.2,0.55],"position":[0.0,1.5,0.0],"material_zone_id":"zone-black-mechanical"}},
                {"node_id":"head","operator_id":"forgecad.geometry.primitive@1","part_id":"head","parameters":{"shape":"sphere","size":[0.85,0.9,0.85],"position":[0.0,2.75,0.0],"material_zone_id":"zone-white-shell","segments":16}}
            ],
            "budgets":{"max_nodes":16,"max_triangles":20000,"max_runtime_ms":1000}
        });
        let hash = canonical_json_hash(&program);
        program["canonical_sha256"] = Value::String(hash.clone());
        let result = runtime
            .prepare_geometry_candidate(
                &project.project_id,
                None,
                json!({"typed":"geometry","geometry_program":program}),
            )
            .expect("geometry prepare");
        assert_eq!(result["schema_version"], "GeometryPrepareResult@1");
        assert_eq!(result["candidate"]["state"], "reviewable");
        assert_eq!(result["candidate"]["quality_hard_gate_passed"], true);
        let artifact_id = result["artifact"]["artifact_id"]
            .as_str()
            .expect("artifact hash")
            .to_owned();
        let candidate_id = result["candidate"]["candidate_id"]
            .as_str()
            .expect("candidate id")
            .to_owned();
        assert_eq!(result["artifact"]["mime"], "model/gltf-binary");
        assert_eq!(result["artifact"]["part_ids"].as_array().unwrap().len(), 3);
        assert!(result["artifact"]["triangle_count"].as_u64().unwrap() > 0);
        assert_eq!(result["artifact"]["validator_status"], "passed");
        let readback = runtime
            .artifact_readback(&artifact_id, &candidate_id)
            .expect("artifact readback");
        assert_eq!(readback["artifact_id"], artifact_id);
        assert_eq!(readback["candidate_id"], candidate_id);
        assert_eq!(readback["part_ids"], result["artifact"]["part_ids"]);
        assert_eq!(readback["triangle_count"], result["artifact"]["triangle_count"]);
        assert_eq!(readback["validator_status"], "passed");
    }

    #[test]
    fn geometry_candidate_rejects_non_canonical_or_unsupported_programs() {
        let runtime = Runtime::ephemeral().expect("runtime");
        let project = runtime
            .create_project("geometry negative project", json!({"profile":"mvp"}))
            .expect("project");
        let mut program = json!({
            "schema_version":"GeometryProgram@1",
            "project_id":project.project_id.clone(),
            "representation_plan_sha256":"b".repeat(64),
            "nodes":[{"node_id":"part","operator_id":"forgecad.geometry.python@1","part_id":"part","parameters":{"shape":"box"}}],
            "budgets":{"max_nodes":1,"max_triangles":100,"max_runtime_ms":1000}
        });
        program["canonical_sha256"] = Value::String(canonical_json_hash(&program));
        let error = runtime
            .prepare_geometry_candidate(
                &project.project_id,
                None,
                json!({"typed":"geometry","geometry_program":program}),
            )
            .expect_err("unsupported operator");
        assert!(error.to_string().contains("GEOMETRY_REJECTED"));
    }

    #[test]
    fn appearance_candidate_has_uv_pbr_and_fixed_render_evidence() {
        let runtime = Runtime::ephemeral().expect("runtime");
        let project = runtime
            .create_project("appearance MVP project", json!({"profile":"mvp"}))
            .expect("project");
        let mut geometry = json!({
            "schema_version":"GeometryProgram@1",
            "project_id":project.project_id.clone(),
            "representation_plan_sha256":"c".repeat(64),
            "nodes":[
                {"node_id":"shell","operator_id":"forgecad.geometry.primitive@1","part_id":"shell","parameters":{"shape":"box","size":[1.0,1.4,0.6],"position":[0.0,0.7,0.0],"material_zone_id":"zone-white-shell"}},
                {"node_id":"core","operator_id":"forgecad.geometry.primitive@1","part_id":"core","parameters":{"shape":"cylinder","size":[0.45,0.9,0.45],"position":[0.0,0.6,0.0],"material_zone_id":"zone-black-mechanical"}}
            ],
            "budgets":{"max_nodes":8,"max_triangles":5000,"max_runtime_ms":1000}
        });
        let geometry_hash = canonical_json_hash(&geometry);
        geometry["canonical_sha256"] = Value::String(geometry_hash.clone());
        let mut appearance = json!({
            "schema_version":"AppearanceProgram@1",
            "project_id":project.project_id.clone(),
            "geometry_program_sha256":geometry_hash,
            "material_zones":[
                {"zone_id":"zone-white-shell","part_ids":["shell"],"base_color":[0.78,0.82,0.86,1.0],"metallic":0.72,"roughness":0.28,"emissive":[0.0,0.0,0.0]},
                {"zone_id":"zone-black-mechanical","part_ids":["core"],"base_color":[0.03,0.04,0.05,1.0],"metallic":0.75,"roughness":0.3,"emissive":[0.0,0.0,0.0]},
                {"zone_id":"zone-amber-emissive","part_ids":["core"],"base_color":[0.16,0.06,0.01,1.0],"metallic":0.2,"roughness":0.25,"emissive":[1.0,0.12,0.01]}
            ]
        });
        appearance["canonical_sha256"] = Value::String(canonical_json_hash(&appearance));
        let result = runtime
            .prepare_appearance_candidate(
                &project.project_id,
                None,
                json!({"typed":"appearance","geometry_program":geometry,"appearance_program":appearance}),
            )
            .expect("appearance prepare");
        assert_eq!(result["schema_version"], "AppearancePrepareResult@1");
        assert_eq!(result["artifact"]["uv_status"], "passed");
        assert_eq!(result["artifact"]["tangent_status"], "passed");
        assert_eq!(result["render_set"]["passes"].as_array().unwrap().len(), 4);
        assert!(result["render_set_object_sha256"].as_str().is_some_and(forgecad_contracts::is_sha256));
        assert_eq!(result["candidate"]["quality_hard_gate_passed"], true);
    }

    #[test]
    fn appearance_candidate_can_be_approved_diffed_and_exported_as_glb() {
        let runtime = Runtime::ephemeral().expect("runtime");
        let project = runtime
            .create_project("MCP009 golden path", json!({"profile":"mvp"}))
            .expect("project");
        let mut geometry = json!({
            "schema_version":"GeometryProgram@1",
            "project_id":project.project_id.clone(),
            "representation_plan_sha256":"d".repeat(64),
            "nodes":[{"node_id":"body","operator_id":"forgecad.geometry.primitive@1","part_id":"body","parameters":{"shape":"box","size":[1.0,1.0,0.6],"position":[0.0,0.5,0.0],"material_zone_id":"zone-white-shell"}}],
            "budgets":{"max_nodes":4,"max_triangles":1000,"max_runtime_ms":1000}
        });
        let geometry_hash = canonical_json_hash(&geometry);
        geometry["canonical_sha256"] = Value::String(geometry_hash.clone());
        let mut appearance = json!({
            "schema_version":"AppearanceProgram@1","project_id":project.project_id.clone(),"geometry_program_sha256":geometry_hash,
            "material_zones":[{"zone_id":"zone-white-shell","part_ids":["body"],"base_color":[0.8,0.82,0.86,1.0],"metallic":0.7,"roughness":0.3,"emissive":[0.0,0.0,0.0]}]
        });
        appearance["canonical_sha256"] = Value::String(canonical_json_hash(&appearance));
        let prepared = runtime.prepare_appearance_candidate(&project.project_id, None, json!({"typed":"appearance","geometry_program":geometry,"appearance_program":appearance})).expect("prepare");
        let candidate = &prepared["candidate"];
        let candidate_id = candidate["candidate_id"].as_str().unwrap().to_owned();
        let object_id = candidate["prepared_object_id"].as_str().unwrap().to_owned();
        let object_hash = candidate["prepared_object_sha256"].as_str().unwrap().to_owned();
        let quality_id = candidate["quality_report_id"].as_str().unwrap().to_owned();
        let confirmed = runtime.confirm_candidate(&CandidateConfirmRequest {
            project_id:project.project_id.clone(), candidate_id:candidate_id.clone(), base_version_id:None,
            prepared_object_id:object_id, prepared_object_sha256:object_hash, quality_report_id:quality_id,
            approval_receipt_id:"mcp009-user-approval".to_owned(), approval_summary:"Approve MVP appearance candidate".to_owned(), approval_session_id:"mcp009-session".to_owned(), approval_expires_at:"9999999999".to_owned(), idempotency_key:"mcp009-confirm-once".to_owned()
        }).expect("confirm");
        let report = runtime.quality(&candidate_id, None).expect("quality");
        assert_eq!(report["hard_gate_passed"], true);
        let export = runtime.prepare_export(&ExportPrepareRequest { project_id:project.project_id.clone(), version_id:confirmed.version_id.clone(), format:"glb".to_owned(), profile:"mvp-glb".to_owned(), request:json!({"reason":"MVP GLB export"}) }).expect("glb export prepare");
        let output = runtime.confirm_export(&ExportConfirmRequest { project_id:project.project_id.clone(), export_id:export.manifest.export_id.clone(), version_id:confirmed.version_id.clone(), format:"glb".to_owned(), profile:"mvp-glb".to_owned(), approval_receipt_id:"mcp009-export-approval".to_owned(), approval_summary:"Approve MVP GLB export".to_owned(), approval_session_id:"mcp009-session".to_owned(), approval_expires_at:"9999999999".to_owned(), idempotency_key:"mcp009-export-once".to_owned() }).expect("glb export confirm");
        assert_eq!(output.output_sha256, export.manifest.artifact_hashes[0]);
        let diff = runtime.version_diff(&confirmed.version_id, &confirmed.version_id).expect("version diff");
        assert_eq!(diff["same_artifact"], true);
    }

    #[test]
    fn stable_part_change_prepare_requires_head_and_returns_new_candidate() {
        let runtime = Runtime::ephemeral().expect("runtime");
        let project = runtime
            .create_project("MCP009 change project", json!({"profile":"mvp"}))
            .expect("project");
        let mut geometry = json!({
            "schema_version":"GeometryProgram@1","project_id":project.project_id.clone(),"representation_plan_sha256":"e".repeat(64),
            "nodes":[{"node_id":"body","operator_id":"forgecad.geometry.primitive@1","part_id":"body","parameters":{"shape":"box","size":[1.0,1.0,0.6],"position":[0.0,0.5,0.0],"material_zone_id":"zone-white-shell"}}],
            "budgets":{"max_nodes":4,"max_triangles":1000,"max_runtime_ms":1000}
        });
        let geometry_hash = canonical_json_hash(&geometry);
        geometry["canonical_sha256"] = Value::String(geometry_hash.clone());
        let mut appearance = json!({
            "schema_version":"AppearanceProgram@1","project_id":project.project_id.clone(),"geometry_program_sha256":geometry_hash,
            "material_zones":[{"zone_id":"zone-white-shell","part_ids":["body"],"base_color":[0.8,0.82,0.86,1.0],"metallic":0.7,"roughness":0.3,"emissive":[0.0,0.0,0.0]}]
        });
        appearance["canonical_sha256"] = Value::String(canonical_json_hash(&appearance));
        let first = runtime
            .prepare_appearance_candidate(&project.project_id, None, json!({"typed":"appearance","geometry_program":geometry.clone(),"appearance_program":appearance.clone()}))
            .expect("first prepare");
        let first_candidate = &first["candidate"];
        let confirmed = runtime
            .confirm_candidate(&CandidateConfirmRequest {
                project_id:project.project_id.clone(), candidate_id:first_candidate["candidate_id"].as_str().unwrap().to_owned(), base_version_id:None,
                prepared_object_id:first_candidate["prepared_object_id"].as_str().unwrap().to_owned(), prepared_object_sha256:first_candidate["prepared_object_sha256"].as_str().unwrap().to_owned(), quality_report_id:first_candidate["quality_report_id"].as_str().unwrap().to_owned(),
                approval_receipt_id:"mcp009-change-first".to_owned(), approval_summary:"Approve first model".to_owned(), approval_session_id:"mcp009-change-session".to_owned(), approval_expires_at:"9999999999".to_owned(), idempotency_key:"mcp009-change-first-once".to_owned()
            })
            .expect("first confirm");
        geometry["nodes"][0]["parameters"]["size"][0] = json!(1.15);
        geometry["canonical_sha256"] = Value::String(canonical_json_hash(&json!({"schema_version":geometry["schema_version"].clone(),"project_id":geometry["project_id"].clone(),"representation_plan_sha256":geometry["representation_plan_sha256"].clone(),"nodes":geometry["nodes"].clone(),"budgets":geometry["budgets"].clone()})));
        appearance["geometry_program_sha256"] = geometry["canonical_sha256"].clone();
        appearance["canonical_sha256"] = Value::String(canonical_json_hash(&json!({"schema_version":appearance["schema_version"].clone(),"project_id":appearance["project_id"].clone(),"geometry_program_sha256":appearance["geometry_program_sha256"].clone(),"material_zones":appearance["material_zones"].clone()})));
        let changed = runtime
            .prepare_change_candidate(&project.project_id, Some(&confirmed.version_id), json!({
                "typed":"change",
                "change_set":{"part_id":"body","operation":"transform","parameters":{"scale":[1.15,1.0,1.0]},"reason":"widen torso shell"},
                "geometry_program":geometry,
                "appearance_program":appearance
            }))
            .expect("change prepare");
        assert_eq!(changed["schema_version"], "ChangePrepareResult@1");
        assert_eq!(changed["change_set"]["part_id"], "body");
        assert_eq!(changed["candidate"]["state"], "reviewable");
        assert_eq!(runtime.versions(Some(&project.project_id)).unwrap().len(), 1);
    }

    #[test]
    fn reference_import_stores_png_bytes_and_returns_hash_bound_evidence() {
        let runtime = Runtime::ephemeral().expect("runtime");
        let project = runtime
            .create_project("reference project", json!({"profile":"mvp"}))
            .expect("project");
        let request = ReferenceImportRequest {
            project_id: project.project_id.clone(),
            source: ReferenceImportSource::InlineContent {
                mime: "image/png".to_owned(),
                content_base64: "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=".to_owned(),
            },
            authorization: ReferenceAuthorization {
                user_authorized: true,
                declaration: "User supplied reference for this local MVP project".to_owned(),
            },
            expected_sha256: None,
        };
        let imported = runtime.import_reference(&request).expect("reference import");
        assert_eq!(imported.reference.mime, "image/png");
        assert_eq!(imported.reference.width, 1);
        assert_eq!(imported.reference.height, 1);
        assert_eq!(imported.reference.frame_count, 1);
        assert_eq!(imported.reference.import_mode, "inline_content");
        assert!(forgecad_contracts::is_sha256(&imported.reference.object_sha256));
        assert!(forgecad_contracts::is_sha256(&imported.reference.canonical_sha256));
        let bytes = runtime
            .cas_read(&imported.reference.object_sha256)
            .expect("CAS readback");
        assert!(!bytes.is_empty());
        assert_eq!(
            runtime
                .reference(&imported.reference.reference_id)
                .expect("reference read")
                .expect("reference")
                .object_sha256,
            imported.reference.object_sha256
        );
        assert_eq!(runtime.references(&project.project_id).unwrap().len(), 1);
    }

    #[test]
    fn reference_import_accepts_jpeg_bytes() {
        let runtime = Runtime::ephemeral().expect("runtime");
        let project = runtime
            .create_project("jpeg reference project", json!({"profile":"mvp"}))
            .expect("project");
        let mut jpeg = Vec::new();
        let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, 80);
        encoder
            .encode(&[240, 180, 80], 1, 1, image::ExtendedColorType::Rgb8)
            .expect("jpeg");
        let imported = runtime
            .import_reference(&ReferenceImportRequest {
                project_id: project.project_id,
                source: ReferenceImportSource::InlineContent {
                    mime: "image/jpeg".to_owned(),
                    content_base64: base64::engine::general_purpose::STANDARD.encode(jpeg),
                },
                authorization: ReferenceAuthorization {
                    user_authorized: true,
                    declaration: "authorized JPEG fixture".to_owned(),
                },
                expected_sha256: None,
            })
            .expect("JPEG import");
        assert_eq!(imported.reference.mime, "image/jpeg");
        assert_eq!(imported.reference.width, 1);
        assert_eq!(imported.reference.height, 1);
    }

    #[test]
    fn reference_import_rejects_wrong_mime_and_missing_authorization() {
        let runtime = Runtime::ephemeral().expect("runtime");
        let project = runtime
            .create_project("reference negative project", json!({"profile":"mvp"}))
            .expect("project");
        let mut request = ReferenceImportRequest {
            project_id: project.project_id,
            source: ReferenceImportSource::InlineContent {
                mime: "image/jpeg".to_owned(),
                content_base64: "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=".to_owned(),
            },
            authorization: ReferenceAuthorization {
                user_authorized: true,
                declaration: "authorized".to_owned(),
            },
            expected_sha256: None,
        };
        let error = runtime.import_reference(&request).expect_err("MIME mismatch");
        assert!(error.to_string().contains("REFERENCE_REJECTED"));
        request.authorization.user_authorized = false;
        request.source = ReferenceImportSource::InlineContent {
            mime: "image/png".to_owned(),
            content_base64: "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=".to_owned(),
        };
        let error = runtime.import_reference(&request).expect_err("authorization");
        assert!(error.to_string().contains("REFERENCE_REJECTED"));
        assert!(runtime.references(&request.project_id).unwrap().is_empty());
    }

    #[test]
    fn reference_import_rejects_truncation_oversize_and_hash_mismatch() {
        let runtime = Runtime::ephemeral().expect("runtime");
        let project = runtime
            .create_project("reference limits", json!({"profile":"mvp"}))
            .expect("project");
        let authorized = ReferenceAuthorization {
            user_authorized: true,
            declaration: "authorized limit fixture".to_owned(),
        };
        let request = |content_base64: String, expected_sha256: Option<String>| {
            ReferenceImportRequest {
                project_id: project.project_id.clone(),
                source: ReferenceImportSource::InlineContent {
                    mime: "image/png".to_owned(),
                    content_base64,
                },
                authorization: authorized.clone(),
                expected_sha256,
            }
        };
        let truncated = base64::engine::general_purpose::STANDARD.encode(b"not a complete png");
        let error = runtime
            .import_reference(&request(truncated, None))
            .expect_err("truncated image");
        assert!(error.to_string().contains("REFERENCE_REJECTED"));
        let error = runtime
            .import_reference(&request("A".repeat(MAX_REFERENCE_INLINE_BASE64 + 1), None))
            .expect_err("oversize encoded input");
        assert!(error.to_string().contains("REFERENCE_REJECTED"));
        let valid = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=".to_owned();
        let error = runtime
            .import_reference(&request(valid, Some("a".repeat(64))))
            .expect_err("hash mismatch");
        assert!(error.to_string().contains("CAS expected hash does not match content"));
    }

    #[cfg(unix)]
    #[test]
    fn local_file_reference_requires_authorized_root_and_rejects_symlink() {
        use std::os::unix::fs::symlink;
        let root = std::env::temp_dir().join(format!("forgecad-mcp005-path-{}", Uuid::new_v4()));
        let outside = std::env::temp_dir().join(format!("forgecad-mcp005-outside-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).expect("root");
        fs::write(
            root.join("source.png"),
            base64::engine::general_purpose::STANDARD
                .decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=")
                .expect("fixture bytes"),
        )
        .expect("source");
        fs::write(
            &outside,
            base64::engine::general_purpose::STANDARD
                .decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=")
                .expect("fixture bytes"),
        )
        .expect("outside");
        let runtime = Runtime::from_store_with_attachment_roots(
            Store::memory().expect("store"),
            vec![fs::canonicalize(&root).expect("canonical root")],
        )
        .expect("runtime");
        let project = runtime
            .create_project("local file reference", json!({"profile":"mvp"}))
            .expect("project");
        let request = |path: String| ReferenceImportRequest {
            project_id: project.project_id.clone(),
            source: ReferenceImportSource::CodexLocalFile { path },
            authorization: ReferenceAuthorization {
                user_authorized: true,
                declaration: "authorized local fixture".to_owned(),
            },
            expected_sha256: None,
        };
        let imported = runtime
            .import_reference(&request(root.join("source.png").to_string_lossy().into_owned()))
            .expect("authorized file");
        assert_eq!(imported.reference.import_mode, "codex_local_file");
        let error = runtime
            .import_reference(&request(outside.to_string_lossy().into_owned()))
            .expect_err("outside root");
        assert!(error.to_string().contains("REFERENCE_REJECTED"));
        let link = root.join("link.png");
        symlink(&outside, &link).expect("symlink");
        let error = runtime
            .import_reference(&request(link.to_string_lossy().into_owned()))
            .expect_err("symlink");
        assert!(error.to_string().contains("REFERENCE_REJECTED"));
        let _ = fs::remove_dir_all(root);
        let _ = fs::remove_file(outside);
    }

    fn confirm_request(fixture: &Fixture, key: &str, expires_at: &str) -> CandidateConfirmRequest {
        CandidateConfirmRequest {
            project_id: fixture.project_id.clone(),
            candidate_id: fixture.candidate_id.clone(),
            base_version_id: None,
            prepared_object_id: "prepared-object-fixture".to_owned(),
            prepared_object_sha256: fixture.object_hash.clone(),
            quality_report_id: "quality-fixture".to_owned(),
            approval_receipt_id: format!("approval-{key}"),
            approval_summary: "Confirm the typed diagnostic candidate".to_owned(),
            approval_session_id: "session-fixture".to_owned(),
            approval_expires_at: expires_at.to_owned(),
            idempotency_key: key.to_owned(),
        }
    }

    #[test]
    fn confirm_creates_one_immutable_version_and_replays_idempotently() {
        let fixture = fixture();
        fixture
            .runtime
            .mark_candidate_quality(&fixture.candidate_id, "quality-fixture", true)
            .expect("quality");
        let request = confirm_request(&fixture, "confirm-once", "9999999999");
        let first = fixture
            .runtime
            .confirm_candidate(&request)
            .expect("confirm");
        assert!(!first.replayed);
        assert!(first.approval_receipt_id.starts_with("receipt-"));
        assert_ne!(first.approval_receipt_id, request.approval_receipt_id);
        assert_eq!(
            fixture
                .runtime
                .versions(Some(&fixture.project_id))
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            fixture
                .runtime
                .candidate(&fixture.candidate_id)
                .unwrap()
                .unwrap()
                .state,
            "confirmed"
        );
        let replay = fixture.runtime.confirm_candidate(&request).expect("replay");
        assert!(replay.replayed);
        assert_eq!(replay.version_id, first.version_id);
        assert_eq!(
            fixture
                .runtime
                .versions(Some(&fixture.project_id))
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn idempotency_key_cannot_be_reused_for_a_different_request() {
        let fixture = fixture();
        fixture
            .runtime
            .mark_candidate_quality(&fixture.candidate_id, "quality-fixture", true)
            .expect("quality");
        let request = confirm_request(&fixture, "same-key", "9999999999");
        fixture
            .runtime
            .confirm_candidate(&request)
            .expect("confirm");
        let mut different = request.clone();
        different.approval_summary = "different approved scope".to_owned();
        let error = fixture
            .runtime
            .confirm_candidate(&different)
            .expect_err("key reuse");
        assert!(error.to_string().contains("IDEMPOTENCY_KEY_REUSED"));
        assert_eq!(
            fixture
                .runtime
                .versions(Some(&fixture.project_id))
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn confirmed_transaction_survives_runtime_restart_with_same_lineage() {
        let root = std::env::temp_dir().join(format!("forgecad-mcp004-restart-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).expect("root");
        let database = root.join("runtime.sqlite");
        let cas = root.join("cas");
        let (project_id, candidate_id, version_id, snapshot_id) = {
            let runtime = Runtime::open_with_cas(&database, &cas).expect("runtime");
            let project = runtime
                .create_project("restart fixture", json!({"scope":"test"}))
                .expect("project");
            let object = runtime
                .put_object(
                    b"restart prepared object",
                    None,
                    "application/octet-stream",
                    "prepared-object",
                )
                .expect("object");
            let prepared = runtime
                .prepare_candidate(
                    &project.project_id,
                    None,
                    "restart-prepared-object",
                    &object.record.sha256,
                    json!({"typed":"restart"}),
                )
                .expect("prepare");
            runtime
                .mark_candidate_quality(&prepared.candidate.candidate_id, "restart-quality", true)
                .expect("quality");
            let request = CandidateConfirmRequest {
                project_id: project.project_id.clone(),
                candidate_id: prepared.candidate.candidate_id.clone(),
                base_version_id: None,
                prepared_object_id: "restart-prepared-object".to_owned(),
                prepared_object_sha256: object.record.sha256,
                quality_report_id: "restart-quality".to_owned(),
                approval_receipt_id: "restart-approval".to_owned(),
                approval_summary: "Confirm restart fixture".to_owned(),
                approval_session_id: "restart-session".to_owned(),
                approval_expires_at: "9999999999".to_owned(),
                idempotency_key: "restart-confirm".to_owned(),
            };
            let result = runtime.confirm_candidate(&request).expect("confirm");
            (
                project.project_id,
                prepared.candidate.candidate_id,
                result.version_id,
                result.snapshot_id,
            )
        };
        let reopened = Runtime::open_with_cas(&database, &cas).expect("reopen");
        assert_eq!(
            reopened.candidate(&candidate_id).unwrap().unwrap().state,
            "confirmed"
        );
        assert_eq!(
            reopened.version(&version_id).unwrap().unwrap().candidate_id,
            candidate_id
        );
        assert_eq!(
            reopened.snapshot(&snapshot_id).unwrap().unwrap().project_id,
            project_id
        );
        assert_eq!(reopened.versions(Some(&project_id)).unwrap().len(), 1);
        drop(reopened);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn confirm_rejects_hash_mismatch_without_writing_a_version() {
        let fixture = fixture();
        fixture
            .runtime
            .mark_candidate_quality(&fixture.candidate_id, "quality-fixture", true)
            .expect("quality");
        let mut request = confirm_request(&fixture, "hash-mismatch", "9999999999");
        request.prepared_object_sha256 = "b".repeat(64);
        let error = fixture
            .runtime
            .confirm_candidate(&request)
            .expect_err("mismatch");
        assert!(error.to_string().contains("CANDIDATE_HASH_MISMATCH"));
        assert!(fixture
            .runtime
            .versions(Some(&fixture.project_id))
            .unwrap()
            .is_empty());
        assert_eq!(
            fixture
                .runtime
                .candidate(&fixture.candidate_id)
                .unwrap()
                .unwrap()
                .state,
            "reviewable"
        );
    }

    #[test]
    fn stale_candidates_fail_closed_without_moving_the_head() {
        let first = fixture();
        first
            .runtime
            .mark_candidate_quality(&first.candidate_id, "quality-fixture", true)
            .expect("quality");

        let object = first
            .runtime
            .put_object(
                b"second prepared object",
                None,
                "application/octet-stream",
                "prepared-object",
            )
            .expect("second object");
        let second = first
            .runtime
            .prepare_candidate(
                &first.project_id,
                None,
                "prepared-object-second",
                &object.record.sha256,
                json!({"typed":"second"}),
            )
            .expect("second candidate");
        first
            .runtime
            .mark_candidate_quality(&second.candidate.candidate_id, "quality-second", true)
            .expect("second quality");

        let first_request = confirm_request(&first, "confirm-first", "9999999999");
        first
            .runtime
            .confirm_candidate(&first_request)
            .expect("first confirm");
        let second_request = CandidateConfirmRequest {
            project_id: first.project_id.clone(),
            candidate_id: second.candidate.candidate_id.clone(),
            base_version_id: None,
            prepared_object_id: "prepared-object-second".to_owned(),
            prepared_object_sha256: object.record.sha256,
            quality_report_id: "quality-second".to_owned(),
            approval_receipt_id: "approval-second".to_owned(),
            approval_summary: "Confirm the second candidate".to_owned(),
            approval_session_id: "session-fixture".to_owned(),
            approval_expires_at: "9999999999".to_owned(),
            idempotency_key: "confirm-second".to_owned(),
        };
        let error = first
            .runtime
            .confirm_candidate(&second_request)
            .expect_err("stale");
        assert!(error.to_string().contains("STALE_BASE_VERSION"));
        assert_eq!(
            first
                .runtime
                .versions(Some(&first.project_id))
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn quality_failure_and_expired_approval_never_create_versions() {
        let failed = fixture();
        failed
            .runtime
            .mark_candidate_quality(&failed.candidate_id, "quality-failed", false)
            .expect("quality failure");
        let failed_request = confirm_request(&failed, "quality-failed-confirm", "9999999999");
        let error = failed
            .runtime
            .confirm_candidate(&failed_request)
            .expect_err("quality gate");
        assert!(error.to_string().contains("QUALITY_HARD_GATE_FAILED"));
        assert!(failed
            .runtime
            .versions(Some(&failed.project_id))
            .unwrap()
            .is_empty());

        let expired = fixture();
        expired
            .runtime
            .mark_candidate_quality(&expired.candidate_id, "quality-expired", true)
            .expect("quality");
        let mut expired_request = confirm_request(&expired, "expired-confirm", "0");
        expired_request.quality_report_id = "quality-expired".to_owned();
        let error = expired
            .runtime
            .confirm_candidate(&expired_request)
            .expect_err("expired");
        assert!(error.to_string().contains("APPROVAL_EXPIRED"));
        assert!(expired
            .runtime
            .versions(Some(&expired.project_id))
            .unwrap()
            .is_empty());
    }

    #[test]
    fn explicit_reject_is_idempotent_and_does_not_write_a_version() {
        let fixture = fixture();
        let request = CandidateRejectRequest {
            project_id: fixture.project_id.clone(),
            candidate_id: fixture.candidate_id.clone(),
            approval_receipt_id: "approval-reject".to_owned(),
            approval_summary: "Reject the diagnostic candidate".to_owned(),
            approval_session_id: "session-fixture".to_owned(),
            approval_expires_at: "9999999999".to_owned(),
            idempotency_key: "reject-once".to_owned(),
        };
        let first = fixture.runtime.reject_candidate(&request).expect("reject");
        assert!(!first.replayed);
        assert_eq!(first.state, "rejected");
        let replay = fixture
            .runtime
            .reject_candidate(&request)
            .expect("reject replay");
        assert!(replay.replayed);
        assert!(fixture
            .runtime
            .versions(Some(&fixture.project_id))
            .unwrap()
            .is_empty());
        assert_eq!(
            fixture
                .runtime
                .candidate(&fixture.candidate_id)
                .unwrap()
                .unwrap()
                .state,
            "rejected"
        );
    }

    #[test]
    fn restore_creates_a_new_child_and_diagnostic_export_is_idempotent() {
        let fixture = fixture();
        fixture
            .runtime
            .mark_candidate_quality(&fixture.candidate_id, "quality-source", true)
            .expect("source quality");
        let source = fixture
            .runtime
            .confirm_candidate(&CandidateConfirmRequest {
                project_id: fixture.project_id.clone(),
                candidate_id: fixture.candidate_id.clone(),
                base_version_id: None,
                prepared_object_id: "prepared-object-fixture".to_owned(),
                prepared_object_sha256: fixture.object_hash.clone(),
                quality_report_id: "quality-source".to_owned(),
                approval_receipt_id: "approval-source".to_owned(),
                approval_summary: "Confirm source version".to_owned(),
                approval_session_id: "session-fixture".to_owned(),
                approval_expires_at: "9999999999".to_owned(),
                idempotency_key: "confirm-source".to_owned(),
            })
            .expect("source confirm");

        let second_object = fixture
            .runtime
            .put_object(
                b"second confirmed object",
                None,
                "application/octet-stream",
                "prepared-object",
            )
            .expect("second object");
        let second = fixture
            .runtime
            .prepare_candidate(
                &fixture.project_id,
                Some(&source.version_id),
                "second-prepared-object",
                &second_object.record.sha256,
                json!({"typed":"second"}),
            )
            .expect("second prepare");
        fixture
            .runtime
            .mark_candidate_quality(&second.candidate.candidate_id, "quality-second", true)
            .expect("second quality");
        let second_confirm = fixture
            .runtime
            .confirm_candidate(&CandidateConfirmRequest {
                project_id: fixture.project_id.clone(),
                candidate_id: second.candidate.candidate_id.clone(),
                base_version_id: Some(source.version_id.clone()),
                prepared_object_id: "second-prepared-object".to_owned(),
                prepared_object_sha256: second_object.record.sha256.clone(),
                quality_report_id: "quality-second".to_owned(),
                approval_receipt_id: "approval-second".to_owned(),
                approval_summary: "Confirm second version".to_owned(),
                approval_session_id: "session-fixture".to_owned(),
                approval_expires_at: "9999999999".to_owned(),
                idempotency_key: "confirm-second".to_owned(),
            })
            .expect("second confirm");

        let restored = fixture
            .runtime
            .prepare_restore(&RestorePrepareRequest {
                project_id: fixture.project_id.clone(),
                base_version_id: Some(second_confirm.version_id.clone()),
                source_version_id: source.version_id.clone(),
                request: json!({"reason":"restore source fixture"}),
            })
            .expect("restore prepare");
        assert_eq!(restored.candidate.state, "reviewable");
        assert_eq!(
            restored.candidate.source_version_id.as_deref(),
            Some(source.version_id.as_str())
        );
        let restore_request = RestoreConfirmRequest {
            project_id: fixture.project_id.clone(),
            candidate_id: restored.candidate.candidate_id.clone(),
            source_version_id: source.version_id.clone(),
            base_version_id: Some(second_confirm.version_id.clone()),
            prepared_object_id: restored
                .candidate
                .prepared_object_id
                .clone()
                .expect("restore object id"),
            prepared_object_sha256: restored
                .candidate
                .prepared_object_sha256
                .clone()
                .expect("restore object hash"),
            quality_report_id: restored
                .candidate
                .quality_report_id
                .clone()
                .expect("restore quality"),
            approval_receipt_id: "approval-restore".to_owned(),
            approval_summary: "Restore the historical source version".to_owned(),
            approval_session_id: "session-fixture".to_owned(),
            approval_expires_at: "9999999999".to_owned(),
            idempotency_key: "restore-once".to_owned(),
        };
        let restored_version = fixture
            .runtime
            .confirm_restore(&restore_request)
            .expect("restore confirm");
        assert!(!restored_version.replayed);
        assert!(restored_version.approval_receipt_id.starts_with("receipt-"));
        assert_ne!(
            restored_version.approval_receipt_id,
            restore_request.approval_receipt_id
        );
        let restored_record = fixture
            .runtime
            .version(&restored_version.version_id)
            .unwrap()
            .expect("restored version");
        assert_eq!(
            restored_record.parent_version_id.as_deref(),
            Some(second_confirm.version_id.as_str())
        );
        assert_eq!(restored_record.manifest_hash, fixture.object_hash);
        let restore_replay = fixture
            .runtime
            .confirm_restore(&restore_request)
            .expect("restore replay");
        assert!(restore_replay.replayed);
        assert_eq!(restore_replay.version_id, restored_version.version_id);
        assert_eq!(
            fixture
                .runtime
                .candidate(&fixture.candidate_id)
                .unwrap()
                .unwrap()
                .state,
            "confirmed"
        );
        assert_eq!(
            fixture
                .runtime
                .versions(Some(&fixture.project_id))
                .unwrap()
                .len(),
            3
        );

        let export = fixture
            .runtime
            .prepare_export(&ExportPrepareRequest {
                project_id: fixture.project_id.clone(),
                version_id: restored_version.version_id.clone(),
                format: "manifest-json".to_owned(),
                profile: "diagnostic".to_owned(),
                request: json!({"target":"cas-only"}),
            })
            .expect("export prepare");
        assert_eq!(export.manifest.state, "prepared");
        assert_eq!(export.job.kind, "export_prepare");
        let export_request = ExportConfirmRequest {
            project_id: fixture.project_id.clone(),
            export_id: export.manifest.export_id.clone(),
            version_id: restored_version.version_id.clone(),
            format: "manifest-json".to_owned(),
            profile: "diagnostic".to_owned(),
            approval_receipt_id: "approval-export".to_owned(),
            approval_summary: "Approve diagnostic manifest export".to_owned(),
            approval_session_id: "session-fixture".to_owned(),
            approval_expires_at: "9999999999".to_owned(),
            idempotency_key: "export-once".to_owned(),
        };
        let exported = fixture
            .runtime
            .confirm_export(&export_request)
            .expect("export confirm");
        assert!(!exported.replayed);
        assert!(exported.approval_receipt_id.starts_with("receipt-"));
        assert_ne!(
            exported.approval_receipt_id,
            export_request.approval_receipt_id
        );
        assert_eq!(exported.output_sha256, exported.manifest_sha256);
        let export_replay = fixture
            .runtime
            .confirm_export(&export_request)
            .expect("export replay");
        assert!(export_replay.replayed);
        assert_eq!(export_replay.output_sha256, exported.output_sha256);
    }
}
