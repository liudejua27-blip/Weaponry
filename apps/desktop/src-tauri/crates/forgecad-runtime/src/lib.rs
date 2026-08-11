mod geometry_worker;
mod ipc;
mod process_lock;
mod skill_registry;

// The Runtime owns the final, strict GLB readback. Keeping the implementation
// compiled into Runtime ensures a worker's self-reported metadata can never
// replace JSON/BIN/accessor/topology inspection. The shared source is moved by
// the MCP010B cleanup only after the behavior is covered by both crates.
#[path = "../../../../../geometry-worker/src/integrity.rs"]
#[allow(dead_code)]
mod integrity;

#[derive(Debug, thiserror::Error)]
pub(crate) enum GeometryError {
    #[error("geometry readback is invalid: {0}")]
    Invalid(String),
}

use base64::Engine;
pub use forgecad_contracts::{
    build_cohort_sha256, is_opaque_id, supports_mcp_protocol, ReferenceAuthorization,
    ReferenceEvidenceRecord, ReferenceGetResult, ReferenceImportRequest, ReferenceImportResult,
    ReferenceImportSource, RuntimeCapabilities, RuntimeResourceContents, RuntimeResourceDescriptor,
    SelectionRecord, SkillBundleManifestRecord, SkillEvalReportRecord, SkillExecutionReceiptRecord,
    SkillGetResult, SkillListResult, CONTRACT_SET, MCP_PROTOCOL_COMPAT_VERSION,
    MCP_PROTOCOL_VERSION, MCP_PROTOCOL_VERSIONS,
};
pub use forgecad_core::{canonical_json_bytes, canonical_json_hash, sha256_hex};
pub use forgecad_store::{CasError, CasObject, CasStore, Store, StoreError, VisualEvidenceRecord};
use forgecad_worker_protocol::{operator_catalog, operator_catalog_sha256};
pub use ipc::{IpcError, LocalIpcClient, LocalIpcEndpoint, LocalIpcServer};

use forgecad_contracts::{
    CandidateConfirmRequest, CandidateConfirmResult, CandidatePrepareResult, CandidateRecord,
    CandidateRejectRequest, CandidateRejectResult, DesignAssetVersionRecord, ExportConfirmRequest,
    ExportConfirmResult, ExportPrepareRequest, ExportPrepareResult,
    GeometryCandidateEvidenceRecord, JobEventRecord, JobRecord, JobSummary, ProjectRecord,
    ProjectSummary, RestoreConfirmRequest, RestoreConfirmResult, RestorePrepareRequest,
    RestorePrepareResult, SnapshotRecord, SnapshotSummary,
};
use image::{imageops, ImageFormat, ImageReader, Limits, Rgba, RgbaImage};
use serde_json::{json, Value};
use std::collections::VecDeque;
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use uuid::Uuid;

// `integrity.rs` is shared verbatim with the Worker so its focused mutation
// tests also compile when Runtime includes it as the final readback authority.
// These test-only adapters are absent from product builds; normal Runtime
// execution still requires the fixed sibling Worker.
#[cfg(test)]
fn geometry_program_v2_draft_hash(
    draft: &Value,
) -> Result<String, forgecad_geometry_worker::GeometryError> {
    forgecad_geometry_worker::geometry_program_v2_draft_hash(draft)
}

#[cfg(test)]
fn compile_geometry_program(
    program: &Value,
) -> Result<forgecad_geometry_worker::GeometryArtifact, forgecad_geometry_worker::GeometryError> {
    forgecad_geometry_worker::compile_geometry_program(program)
}

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

    /// Return the closed, product-owned OperatorCatalog@1 through an explicit
    /// read path for MCP clients that cannot directly consume MCP resources.
    /// This is the same value exposed at forgecad://operators/catalog.
    pub fn active_operator_catalog(&self) -> Value {
        operator_catalog()
    }

    /// Validate a hash-free GeometryProgram@2 draft and return the one
    /// canonical hash that the bounded compiler will accept. This is a
    /// deliberately read-only authoring aid: it does not compile geometry,
    /// create a candidate or Job, or touch SQLite/CAS state.
    pub fn geometry_program_hash(&self, request: &Value) -> Result<Value, RuntimeError> {
        let object = request.as_object().ok_or_else(|| {
            RuntimeError::InvalidInput(
                "GEOMETRY_PROGRAM_HASH_REJECTED: request must be an object".to_owned(),
            )
        })?;
        let allowed = ["schema_version", "geometry_program_draft"];
        if object.len() != allowed.len()
            || !allowed.iter().all(|key| object.contains_key(*key))
            || object.keys().any(|key| !allowed.contains(&key.as_str()))
        {
            return Err(RuntimeError::InvalidInput(
                "GEOMETRY_PROGRAM_HASH_REJECTED: request must contain only schema_version and geometry_program_draft".to_owned(),
            ));
        }
        if object.get("schema_version").and_then(Value::as_str)
            != Some("GeometryProgramHashRequest@1")
        {
            return Err(RuntimeError::InvalidInput(
                "GEOMETRY_PROGRAM_HASH_REJECTED: schema_version must be GeometryProgramHashRequest@1"
                    .to_owned(),
            ));
        }
        let draft = object
            .get("geometry_program_draft")
            .expect("required key was checked");
        let result = hash_geometry_program_with_runtime_worker(draft).map_err(|error| {
            RuntimeError::InvalidInput(format!("GEOMETRY_PROGRAM_HASH_REJECTED: {error}"))
        })?;
        if result
            .get("operator_catalog_sha256")
            .and_then(Value::as_str)
            != Some(operator_catalog_sha256().as_str())
        {
            return Err(RuntimeError::InvalidInput(
                "GEOMETRY_PROGRAM_HASH_REJECTED: GEOMETRY_WORKER_PROTOCOL".to_owned(),
            ));
        }
        validate_geometry_program_hash_result_output(&result)?;
        Ok(result)
    }

    pub fn skills(&self) -> Result<Vec<SkillBundleManifestRecord>, RuntimeError> {
        skill_registry::list().map_err(RuntimeError::InvalidInput)
    }

    pub fn skill(
        &self,
        skill_id: &str,
        version: &str,
    ) -> Result<Option<SkillBundleManifestRecord>, RuntimeError> {
        if !is_opaque_id(skill_id) || !is_opaque_id(version) {
            return Err(RuntimeError::InvalidInput(
                "invalid Skill identifier".to_owned(),
            ));
        }
        skill_registry::get(skill_id, version).map_err(RuntimeError::InvalidInput)
    }

    pub fn projects(&self) -> Result<Vec<ProjectSummary>, RuntimeError> {
        Ok(self.store.list_projects()?)
    }

    pub fn reference(&self, id: &str) -> Result<Option<ReferenceEvidenceRecord>, RuntimeError> {
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
    pub fn quality(
        &self,
        candidate_id: &str,
        reference_id: Option<&str>,
    ) -> Result<Value, RuntimeError> {
        let candidate = self.candidate(candidate_id)?.ok_or_else(|| {
            RuntimeError::InvalidInput("NOT_FOUND: candidate not found".to_owned())
        })?;
        if let Some(evidence) = self.store.get_visual_evidence(candidate_id)? {
            let report: Value =
                serde_json::from_slice(&self.cas_read(&evidence.quality_report_object_sha256)?)
                    .map_err(|error| {
                        RuntimeError::InvalidInput(format!(
                            "QUALITY_REPORT_INVALID: visual report JSON is invalid: {error}"
                        ))
                    })?;
            if report.get("schema_version").and_then(Value::as_str) == Some("QualityReport@2") {
                if report.get("candidate_id").and_then(Value::as_str) != Some(candidate_id) {
                    return Err(RuntimeError::InvalidInput(
                        "QUALITY_REPORT_BINDING_MISMATCH: candidate hash is not bound".to_owned(),
                    ));
                }
                if let Some(reference_id) = reference_id {
                    if report.get("reference_id").and_then(Value::as_str) != Some(reference_id) {
                        return Err(RuntimeError::InvalidInput(
                            "REFERENCE_BINDING_MISMATCH: visual quality report uses another reference".to_owned(),
                        ));
                    }
                }
                validate_quality_report_v2_output(&report)?;
                return Ok(report);
            }
        }
        let quality_report_id = candidate.quality_report_id.as_deref().ok_or_else(|| {
            RuntimeError::InvalidInput(
                "QUALITY_REPORT_UNAVAILABLE: candidate has not completed Runtime quality evaluation"
                    .to_owned(),
            )
        })?;
        let mut checks = Vec::new();
        let mut artifact = Value::Null;
        if let Some(hash) = candidate
            .manifest_hash
            .as_deref()
            .filter(|hash| forgecad_contracts::is_sha256(hash))
        {
            artifact = self
                .artifact_readback(hash, candidate_id)
                .unwrap_or(Value::Null);
        }
        let artifact_valid =
            artifact.get("validator_status").and_then(Value::as_str) == Some("passed");
        let artifact_present = artifact.is_object();
        let v2_integrity = artifact.get("integrity");
        let v2_uv_tangent_passed = v2_integrity.is_some_and(|integrity| {
            [
                "uv_non_finite_count",
                "zero_area_uv_triangle_count",
                "tangent_non_finite_count",
                "tangent_orthogonality_error_count",
                "tangent_handedness_error_count",
            ]
            .iter()
            .all(|key| integrity.get(*key).and_then(Value::as_u64) == Some(0))
        });
        let legacy_uv_tangent_passed = artifact.get("uv_status").and_then(Value::as_str)
            == Some("passed")
            && artifact.get("tangent_status").and_then(Value::as_str) == Some("passed");
        checks.push(json!({"check_id":"candidate_state","status":if candidate.state == "reviewable" || candidate.state == "confirmed" {"passed"} else {"failed"},"message":format!("candidate state is {}", candidate.state)}));
        checks.push(json!({"check_id":"glb_readback","status":if artifact_valid {"passed"} else if artifact_present {"failed"} else {"not-run"},"message":"Runtime artifact readback is hash-bound"}));
        checks.push(json!({"check_id":"uv_tangent","status":if v2_integrity.is_some() && v2_uv_tangent_passed || v2_integrity.is_none() && legacy_uv_tangent_passed {"passed"} else if artifact_present {"failed"} else {"not-run"},"message":"UV and tangent attributes are read back from the GLB BIN"}));
        checks.push(json!({"check_id":"pbr_material_zones","status":if artifact.get("material_zone_ids").and_then(Value::as_array).is_some_and(|zones| !zones.is_empty()) {"passed"} else if artifact_present {"failed"} else {"not-run"},"message":"typed material zones are bound in GLB lineage"}));
        let mut compare = Value::Null;
        if let Some(reference_id) = reference_id {
            let reference = self.reference(reference_id)?.ok_or_else(|| {
                RuntimeError::InvalidInput("NOT_FOUND: reference not found".to_owned())
            })?;
            if reference.project_id != candidate.project_id {
                return Err(RuntimeError::InvalidInput(
                    "REFERENCE_SCOPE_DENIED: reference is outside the candidate project".to_owned(),
                ));
            }
            if let Some(evidence) = self.store.get_geometry_candidate_evidence(candidate_id)? {
                if evidence.reference_id.as_deref() != Some(reference_id)
                    || evidence.reference_sha256.as_deref()
                        != Some(reference.object_sha256.as_str())
                {
                    return Err(RuntimeError::InvalidInput(
                        "REFERENCE_BINDING_MISMATCH: quality comparison must use the candidate-bound reference"
                            .to_owned(),
                    ));
                }
            }
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
        let hard_gate_passed = candidate.quality_hard_gate_passed
            && checks
                .iter()
                .all(|check| check.get("status").and_then(Value::as_str) != Some("failed"));
        let mut report = json!({
            "schema_version":"QualityReport@1",
            "quality_report_id":quality_report_id,
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

    /// Render a candidate against one user-authorized reference and persist a
    /// temporary RenderSet@2, mask, comparison report and QualityReport@2.
    /// No immutable version is created by this method.
    pub fn prepare_reference_comparison(
        &self,
        project_id: &str,
        request: Value,
    ) -> Result<Value, RuntimeError> {
        validate_id(project_id)?;
        let object = request.as_object().ok_or_else(|| {
            RuntimeError::InvalidInput("reference comparison request must be an object".to_owned())
        })?;
        let candidate_id = required_value_id(object.get("candidate_id"), "candidate_id")?;
        let reference_id = required_value_id(object.get("reference_id"), "reference_id")?;
        let candidate = self.candidate(candidate_id)?.ok_or_else(|| {
            RuntimeError::InvalidInput("NOT_FOUND: candidate not found".to_owned())
        })?;
        if candidate.project_id != project_id {
            return Err(RuntimeError::InvalidInput(
                "PROJECT_SCOPE_DENIED: candidate is outside the target project".to_owned(),
            ));
        }
        let reference = self.reference(reference_id)?.ok_or_else(|| {
            RuntimeError::InvalidInput("NOT_FOUND: reference not found".to_owned())
        })?;
        if reference.project_id != project_id {
            return Err(RuntimeError::InvalidInput(
                "REFERENCE_SCOPE_DENIED: reference is outside the target project".to_owned(),
            ));
        }
        let view_spec = object
            .get("view_spec")
            .ok_or_else(|| RuntimeError::InvalidInput("REFERENCE_VIEW_SPEC_REQUIRED".to_owned()))?;
        validate_reference_view_spec(view_spec, &reference)?;
        let explicit_camera = object.get("camera").is_some_and(|value| !value.is_null());
        let mut camera = object
            .get("camera")
            .filter(|value| !value.is_null())
            .cloned()
            .unwrap_or_else(default_camera_calibration);
        validate_camera_calibration(&camera)?;
        let artifact_sha256 = candidate
            .manifest_hash
            .clone()
            .or(candidate.prepared_object_sha256.clone())
            .ok_or_else(|| {
                RuntimeError::InvalidInput("CANDIDATE_ARTIFACT_UNAVAILABLE".to_owned())
            })?;
        let glb = self.cas_read(&artifact_sha256)?;
        let inspection = strict_glb_inspection(&glb)?;
        if !inspection.hard_gate_passed {
            return Err(RuntimeError::InvalidInput(format!(
                "RENDER_REJECTED: strict GLB readback failed: {}",
                inspection.failure_codes.join(",")
            )));
        }
        let mut render_passes = render_glb_with_runtime_worker(&glb, &camera)
            .map_err(|error| RuntimeError::InvalidInput(format!("RENDER_REJECTED: {error}")))?;
        let reference_bytes = self.cas_read(&reference.object_sha256)?;
        let reference_mask = reference_mask_png(&reference_bytes)?;
        if !explicit_camera {
            if let Some(initial_silhouette) = render_passes
                .iter()
                .find(|pass| pass.pass == "silhouette")
                .map(|pass| decode_binary_mask(&pass.png))
            {
                let initial_silhouette = initial_silhouette?;
                let calibrated =
                    calibrate_default_camera(&camera, &reference_mask.mask, &initial_silhouette);
                if calibrated != camera {
                    validate_camera_calibration(&calibrated)?;
                    camera = calibrated;
                    render_passes =
                        render_glb_with_runtime_worker(&glb, &camera).map_err(|error| {
                            RuntimeError::InvalidInput(format!("RENDER_REJECTED: {error}"))
                        })?;
                }
            }
        }
        let camera_bytes = canonical_json_bytes(&camera)
            .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?;
        let camera_object = self.put_object(
            &camera_bytes,
            None,
            "application/json",
            "camera-calibration",
        )?;
        if render_passes.len() != 9
            || render_passes
                .iter()
                .any(|pass| pass.width != 512 || pass.height != 512)
        {
            return Err(RuntimeError::InvalidInput(
                "RENDER_REJECTED: fixed renderer did not return nine 512x512 passes".to_owned(),
            ));
        }
        let mut pass_artifacts = serde_json::Map::new();
        let mut pass_bytes = std::collections::HashMap::new();
        for pass in &render_passes {
            let stored = self.put_object(
                &pass.png,
                None,
                "image/png",
                &format!("render-pass-{}", pass.pass),
            )?;
            pass_bytes.insert(pass.pass.clone(), pass.png.clone());
            pass_artifacts.insert(
                pass.pass.clone(),
                json!({
                    "sha256":stored.record.sha256,
                    "mime":"image/png",
                    "size_bytes":stored.record.size_bytes,
                    "width":512,
                    "height":512,
                    "channels":"rgba8",
                    "color_space":if pass.pass == "depth" || pass.pass == "normal" || pass.pass == "ao" || pass.pass == "uv-stretch" {"linear"} else if pass.pass == "part-id" || pass.pass == "material-id" || pass.pass == "wireframe" || pass.pass == "silhouette" {"data"} else {"srgb"}
                }),
            );
        }
        let mut render_set = json!({
            "schema_version":"RenderSet@2",
            "render_set_id":format!("render-set-{}", &artifact_sha256[..32]),
            "candidate_id":candidate_id,
            "artifact_sha256":artifact_sha256,
            "program_sha256":inspection.program_sha256,
            "reference_id":reference_id,
            "camera_hash":camera["camera_hash"].clone(),
            "renderer_hash":sha256_hex(b"forgecad-renderer@2-fixed-perspective-ggx-aov"),
            "width":512,
            "height":512,
            "passes":["beauty","silhouette","depth","normal","ao","part-id","material-id","wireframe","uv-stretch"],
            "pass_artifacts":pass_artifacts,
            "canonical_sha256":""
        });
        render_set["canonical_sha256"] = Value::String(canonical_json_hash(&render_set));
        validate_render_set_v2_output(&render_set)?;
        let render_set_bytes = canonical_json_bytes(&render_set)
            .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?;
        let render_set_object =
            self.put_object(&render_set_bytes, None, "application/json", "render-set-v2")?;
        let render_set_hash = render_set_object.record.sha256.clone();
        let mask_object = self.put_object(
            &reference_mask.png,
            None,
            "image/png",
            "reference-silhouette-mask",
        )?;
        let model_mask = pass_bytes.get("silhouette").ok_or_else(|| {
            RuntimeError::InvalidInput("RENDER_REJECTED: silhouette pass missing".to_owned())
        })?;
        let metrics = compare_masks(
            &reference_mask.mask,
            &decode_binary_mask(model_mask)?,
            view_spec,
        );
        let visual_status = if metrics
            .get("silhouette_iou")
            .and_then(Value::as_f64)
            .unwrap_or(0.0)
            >= 0.72
            && metrics
                .get("boundary_f1_4px")
                .and_then(Value::as_f64)
                .unwrap_or(0.0)
                >= 0.75
            && metrics
                .get("bbox_edge_error")
                .and_then(Value::as_f64)
                .unwrap_or(1.0)
                <= 0.05
            && metrics
                .get("centroid_error")
                .and_then(Value::as_f64)
                .unwrap_or(1.0)
                <= 0.04
            && metrics
                .get("landmark_coverage")
                .and_then(Value::as_f64)
                .unwrap_or(0.0)
                >= 0.80
            && metrics
                .get("landmark_nme")
                .and_then(Value::as_f64)
                .unwrap_or(1.0)
                <= 0.08
            && metrics
                .get("region_median_iou")
                .and_then(Value::as_f64)
                .unwrap_or(0.0)
                >= 0.50
            && metrics
                .get("critical_region_min_iou")
                .and_then(Value::as_f64)
                .unwrap_or(0.0)
                >= 0.30
        {
            "PARTIAL_VISIBLE_VIEW_PASS"
        } else {
            "QUALITY_TARGET_NOT_MET"
        };
        let mut comparison = json!({
            "schema_version":"ReferenceComparisonReport@1",
            "report_id":format!("comparison-{}", &render_set_hash[..32]),
            "candidate_id":candidate_id,
            "artifact_sha256":artifact_sha256,
            "reference_id":reference_id,
            "reference_sha256":reference.object_sha256,
            "render_set_hash":render_set_hash,
            "camera_hash":camera["camera_hash"].clone(),
            "mask":{"method":"local-border-flood-fill-morphology","revision":"mask-2","sha256":mask_object.record.sha256,"width":512,"height":512},
            "metrics":metrics,
            "status":visual_status,
            "canonical_sha256":""
        });
        comparison["canonical_sha256"] = Value::String(canonical_json_hash(&comparison));
        validate_reference_comparison_report(&comparison)?;
        let comparison_object = self.put_object(
            &canonical_json_bytes(&comparison)
                .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?,
            None,
            "application/json",
            "reference-comparison-report",
        )?;
        let comparison_hash = comparison_object.record.sha256.clone();
        let quality_id = format!("quality-c-{}", Uuid::new_v4().simple());
        let mut quality = json!({
            "schema_version":"QualityReport@2",
            "quality_report_id":quality_id,
            "candidate_id":candidate_id,
            "artifact_sha256":artifact_sha256,
            "program_sha256":inspection.program_sha256,
            "reference_id":reference_id,
            "reference_sha256":reference.object_sha256,
            "render_set_hash":render_set_hash,
            "comparison_report_hash":comparison_hash,
            "human_receipt_hash":Value::Null,
            "structural_status":"passed",
            "visual_status":visual_status,
            "hard_gate_passed":visual_status == "PARTIAL_VISIBLE_VIEW_PASS",
            "limitations":["human_visual_review_not_run","single_reference_view_only","HQ_360_PASS_BLOCKED_REFERENCE_COVERAGE"],
            "canonical_sha256":""
        });
        quality["canonical_sha256"] = Value::String(canonical_json_hash(&quality));
        validate_quality_report_v2_output(&quality)?;
        let quality_object = self.put_object(
            &canonical_json_bytes(&quality)
                .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?,
            None,
            "application/json",
            "quality-report-v2",
        )?;
        let now = now_string();
        self.store.upsert_visual_evidence(&VisualEvidenceRecord {
            candidate_id: candidate_id.to_owned(),
            project_id: project_id.to_owned(),
            reference_id: reference_id.to_owned(),
            render_set_object_sha256: render_set_object.record.sha256.clone(),
            comparison_report_object_sha256: Some(comparison_object.record.sha256.clone()),
            visual_review_object_sha256: None,
            quality_report_object_sha256: quality_object.record.sha256.clone(),
            human_receipt_object_sha256: None,
            created_at: now.clone(),
            updated_at: now,
        })?;
        Ok(json!({
            "schema_version":"ReferenceComparisonPrepareResult@1",
            "candidate_id":candidate_id,
            "reference_id":reference_id,
            "camera":camera,
            "camera_object_sha256":camera_object.record.sha256,
            "render_set":render_set,
            "render_set_hash":render_set_object.record.sha256,
            "render_set_object_sha256":render_set_object.record.sha256,
            "comparison_report":comparison,
            "comparison_report_hash":comparison_object.record.sha256,
            "comparison_report_object_sha256":comparison_object.record.sha256,
            "quality_report":quality,
            "quality_report_object_sha256":quality_object.record.sha256
        }))
    }

    pub fn render_pass_get(
        &self,
        render_set_hash: &str,
        pass_name: &str,
    ) -> Result<Value, RuntimeError> {
        if !forgecad_contracts::is_sha256(render_set_hash) {
            return Err(RuntimeError::InvalidInput(
                "RENDER_PASS_INVALID: render_set_hash is invalid".to_owned(),
            ));
        }
        let render_set: Value = serde_json::from_slice(&self.cas_read(render_set_hash)?)
            .map_err(|error| RuntimeError::InvalidInput(format!("RENDER_PASS_INVALID: {error}")))?;
        validate_render_set_v2_output(&render_set)?;
        let artifact = render_set
            .get("pass_artifacts")
            .and_then(|value| value.get(pass_name))
            .ok_or_else(|| RuntimeError::InvalidInput("RENDER_PASS_NOT_FOUND".to_owned()))?;
        let pass_hash = artifact
            .get("sha256")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                RuntimeError::InvalidInput("RENDER_PASS_INVALID: pass hash is missing".to_owned())
            })?;
        let png = self.cas_read(pass_hash)?;
        if !png.starts_with(b"\x89PNG\r\n\x1a\n") {
            return Err(RuntimeError::InvalidInput(
                "RENDER_PASS_INVALID: CAS bytes are not PNG".to_owned(),
            ));
        }
        Ok(
            json!({"schema_version":"RenderPassGet@1","render_set_hash":render_set_hash,"candidate_id":render_set["candidate_id"],"pass":pass_name,"mime":"image/png","width":512,"height":512,"sha256":pass_hash,"png_base64":base64::engine::general_purpose::STANDARD.encode(png)}),
        )
    }

    /// Return the candidate-bound visual evidence needed by the optional
    /// Viewer. Reports stay in CAS and are re-read/validated here; the
    /// projection contains no image bytes and performs no writes.
    pub fn visual_evidence(&self, candidate_id: &str) -> Result<Value, RuntimeError> {
        validate_id(candidate_id)?;
        let candidate = self.candidate(candidate_id)?.ok_or_else(|| {
            RuntimeError::InvalidInput(
                "VISUAL_EVIDENCE_UNAVAILABLE: candidate not found".to_owned(),
            )
        })?;
        let evidence = self
            .store
            .get_visual_evidence(candidate_id)?
            .ok_or_else(|| RuntimeError::InvalidInput("VISUAL_EVIDENCE_UNAVAILABLE".to_owned()))?;
        let reference = self.reference(&evidence.reference_id)?.ok_or_else(|| {
            RuntimeError::InvalidInput(
                "VISUAL_EVIDENCE_UNAVAILABLE: reference not found".to_owned(),
            )
        })?;
        if reference.project_id != candidate.project_id
            || evidence.project_id != candidate.project_id
        {
            return Err(RuntimeError::InvalidInput(
                "VISUAL_EVIDENCE_BINDING_MISMATCH: project differs".to_owned(),
            ));
        }
        let render_set: Value = serde_json::from_slice(
            &self.cas_read(&evidence.render_set_object_sha256)?,
        )
        .map_err(|error| {
            RuntimeError::InvalidInput(format!("VISUAL_EVIDENCE_INVALID: RenderSet: {error}"))
        })?;
        validate_render_set_v2_output(&render_set)?;
        if render_set.get("candidate_id").and_then(Value::as_str) != Some(candidate_id)
            || render_set.get("reference_id").and_then(Value::as_str)
                != Some(evidence.reference_id.as_str())
        {
            return Err(RuntimeError::InvalidInput(
                "VISUAL_EVIDENCE_BINDING_MISMATCH: RenderSet candidate differs".to_owned(),
            ));
        }
        let comparison_report = if let Some(hash) =
            evidence.comparison_report_object_sha256.as_deref()
        {
            let report: Value = serde_json::from_slice(&self.cas_read(hash)?).map_err(|error| {
                RuntimeError::InvalidInput(format!(
                    "VISUAL_EVIDENCE_INVALID: comparison report: {error}"
                ))
            })?;
            validate_reference_comparison_report(&report)?;
            if report.get("candidate_id").and_then(Value::as_str) != Some(candidate_id)
                || report.get("reference_id").and_then(Value::as_str)
                    != Some(evidence.reference_id.as_str())
            {
                return Err(RuntimeError::InvalidInput(
                    "VISUAL_EVIDENCE_BINDING_MISMATCH: comparison report differs".to_owned(),
                ));
            }
            Some(report)
        } else {
            None
        };
        let quality_report = self.quality(candidate_id, Some(&evidence.reference_id))?;
        Ok(json!({
            "schema_version":"ViewerVisualEvidence@1",
            "candidate_id":candidate_id,
            "project_id":evidence.project_id,
            "reference_id":evidence.reference_id,
            "render_set_hash":evidence.render_set_object_sha256,
            "comparison_report_hash":evidence.comparison_report_object_sha256,
            "quality_report_hash":evidence.quality_report_object_sha256,
            "render_set":render_set,
            "comparison_report":comparison_report,
            "quality_report":quality_report
        }))
    }

    pub fn submit_visual_review(&self, request: Value) -> Result<Value, RuntimeError> {
        let object = request.as_object().ok_or_else(|| {
            RuntimeError::InvalidInput("visual review request must be an object".to_owned())
        })?;
        let candidate_id = required_value_id(object.get("candidate_id"), "candidate_id")?;
        let reference_id = required_value_id(object.get("reference_id"), "reference_id")?;
        let evidence = self
            .store
            .get_visual_evidence(candidate_id)?
            .ok_or_else(|| {
                RuntimeError::InvalidInput(
                    "VISUAL_REVIEW_UNAVAILABLE: run reference_compare_prepare first".to_owned(),
                )
            })?;
        if evidence.reference_id != reference_id {
            return Err(RuntimeError::InvalidInput(
                "REFERENCE_BINDING_MISMATCH: review reference differs from candidate evidence"
                    .to_owned(),
            ));
        }
        let render_set_hash = required_value_sha(object.get("render_set_hash"), "render_set_hash")?;
        let render_set: Value = serde_json::from_slice(
            &self.cas_read(&evidence.render_set_object_sha256)?,
        )
        .map_err(|error| {
            RuntimeError::InvalidInput(format!(
                "VISUAL_REVIEW_UNAVAILABLE: RenderSet is invalid: {error}"
            ))
        })?;
        validate_render_set_v2_output(&render_set)?;
        if evidence.render_set_object_sha256 != render_set_hash {
            return Err(RuntimeError::InvalidInput(
                "VISUAL_REVIEW_BINDING_MISMATCH: RenderSet hash is not candidate-bound".to_owned(),
            ));
        }
        let comparison_hash = required_value_sha(
            object.get("comparison_report_hash"),
            "comparison_report_hash",
        )?;
        let comparison_object_sha = evidence
            .comparison_report_object_sha256
            .as_deref()
            .ok_or_else(|| {
                RuntimeError::InvalidInput(
                    "VISUAL_REVIEW_UNAVAILABLE: comparison report is missing".to_owned(),
                )
            })?;
        let comparison: Value = serde_json::from_slice(&self.cas_read(comparison_object_sha)?)
            .map_err(|error| {
                RuntimeError::InvalidInput(format!(
                    "VISUAL_REVIEW_UNAVAILABLE: comparison report is invalid: {error}"
                ))
            })?;
        validate_reference_comparison_report(&comparison)?;
        if evidence.comparison_report_object_sha256.as_deref() != Some(comparison_hash) {
            return Err(RuntimeError::InvalidInput(
                "VISUAL_REVIEW_BINDING_MISMATCH: comparison report is not candidate-bound"
                    .to_owned(),
            ));
        }
        let mut report = json!({"schema_version":"VisualReviewReport@1","review_id":format!("review-{}",Uuid::new_v4().simple()),"candidate_id":candidate_id,"reference_id":reference_id,"render_set_hash":render_set_hash,"comparison_report_hash":comparison_hash,"round":object.get("round").cloned().unwrap_or(Value::Null),"stage":object.get("stage").cloned().unwrap_or(Value::Null),"issues":object.get("issues").cloned().unwrap_or(Value::Array(Vec::new())),"status":object.get("status").cloned().unwrap_or(Value::String("submitted".to_owned())),"canonical_sha256":""});
        report["canonical_sha256"] = Value::String(canonical_json_hash(&report));
        validate_visual_review_report(&report)?;
        let report_object = self.put_object(
            &canonical_json_bytes(&report)
                .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?,
            None,
            "application/json",
            "visual-review-report",
        )?;
        let now = now_string();
        self.store.upsert_visual_evidence(&VisualEvidenceRecord {
            visual_review_object_sha256: Some(report_object.record.sha256.clone()),
            updated_at: now.clone(),
            ..evidence
        })?;
        Ok(
            json!({"schema_version":"VisualReviewSubmitResult@1","review":report,"review_object_sha256":report_object.record.sha256}),
        )
    }

    pub fn submit_human_visual_review(&self, request: Value) -> Result<Value, RuntimeError> {
        let object = request.as_object().ok_or_else(|| {
            RuntimeError::InvalidInput("human visual review request must be an object".to_owned())
        })?;
        let candidate_id = required_value_id(object.get("candidate_id"), "candidate_id")?;
        let reference_id = required_value_id(object.get("reference_id"), "reference_id")?;
        let evidence = self
            .store
            .get_visual_evidence(candidate_id)?
            .ok_or_else(|| {
                RuntimeError::InvalidInput(
                    "HUMAN_REVIEW_UNAVAILABLE: run reference_compare_prepare first".to_owned(),
                )
            })?;
        if evidence.reference_id != reference_id {
            return Err(RuntimeError::InvalidInput("REFERENCE_BINDING_MISMATCH: human review reference differs from candidate evidence".to_owned()));
        }
        let render_set_hash = required_value_sha(object.get("render_set_hash"), "render_set_hash")?;
        let comparison_hash = required_value_sha(
            object.get("comparison_report_hash"),
            "comparison_report_hash",
        )?;
        let render_set: Value = serde_json::from_slice(
            &self.cas_read(&evidence.render_set_object_sha256)?,
        )
        .map_err(|error| {
            RuntimeError::InvalidInput(format!(
                "HUMAN_REVIEW_UNAVAILABLE: RenderSet is invalid: {error}"
            ))
        })?;
        validate_render_set_v2_output(&render_set)?;
        if evidence.render_set_object_sha256 != render_set_hash {
            return Err(RuntimeError::InvalidInput(
                "HUMAN_REVIEW_BINDING_MISMATCH: RenderSet hash is not candidate-bound".to_owned(),
            ));
        }
        let comparison_object_sha = evidence
            .comparison_report_object_sha256
            .as_deref()
            .ok_or_else(|| {
                RuntimeError::InvalidInput(
                    "HUMAN_REVIEW_UNAVAILABLE: comparison report is missing".to_owned(),
                )
            })?;
        let comparison: Value = serde_json::from_slice(&self.cas_read(comparison_object_sha)?)
            .map_err(|error| {
                RuntimeError::InvalidInput(format!(
                    "HUMAN_REVIEW_UNAVAILABLE: comparison report is invalid: {error}"
                ))
            })?;
        validate_reference_comparison_report(&comparison)?;
        if evidence.comparison_report_object_sha256.as_deref() != Some(comparison_hash) {
            return Err(RuntimeError::InvalidInput(
                "HUMAN_REVIEW_BINDING_MISMATCH: comparison report is not candidate-bound"
                    .to_owned(),
            ));
        }
        let scores = object
            .get("scores")
            .cloned()
            .ok_or_else(|| RuntimeError::InvalidInput("HUMAN_REVIEW_SCORES_REQUIRED".to_owned()))?;
        let mut receipt = json!({"schema_version":"HumanVisualReviewReceipt@1","receipt_id":format!("human-review-{}",Uuid::new_v4().simple()),"candidate_id":candidate_id,"reference_id":reference_id,"render_set_hash":render_set_hash,"comparison_report_hash":comparison_hash,"scores":scores,"approved":object.get("approved").cloned().unwrap_or(Value::Bool(false)),"recorded_at":now_string(),"canonical_sha256":""});
        receipt["canonical_sha256"] = Value::String(canonical_json_hash(&receipt));
        validate_human_review_receipt(&receipt)?;
        let receipt_object = self.put_object(
            &canonical_json_bytes(&receipt)
                .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?,
            None,
            "application/json",
            "human-visual-review-receipt",
        )?;
        let mut quality: Value =
            serde_json::from_slice(&self.cas_read(&evidence.quality_report_object_sha256)?)
                .map_err(|error| {
                    RuntimeError::InvalidInput(format!("QUALITY_REPORT_INVALID: {error}"))
                })?;
        quality["human_receipt_hash"] = Value::String(receipt_object.record.sha256.clone());
        quality["canonical_sha256"] = Value::String(String::new());
        quality["canonical_sha256"] = Value::String(canonical_json_hash(&quality));
        validate_quality_report_v2_output(&quality)?;
        let quality_object = self.put_object(
            &canonical_json_bytes(&quality)
                .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?,
            None,
            "application/json",
            "quality-report-v2",
        )?;
        let now = now_string();
        self.store.upsert_visual_evidence(&VisualEvidenceRecord {
            human_receipt_object_sha256: Some(receipt_object.record.sha256.clone()),
            quality_report_object_sha256: quality_object.record.sha256.clone(),
            updated_at: now,
            ..evidence
        })?;
        Ok(
            json!({"schema_version":"HumanVisualReviewSubmitResult@1","receipt":receipt,"receipt_object_sha256":receipt_object.record.sha256,"quality_report":quality,"quality_report_object_sha256":quality_object.record.sha256}),
        )
    }

    pub fn version_diff(
        &self,
        version_id: &str,
        compare_to_version_id: &str,
    ) -> Result<Value, RuntimeError> {
        validate_id(version_id)?;
        validate_id(compare_to_version_id)?;
        let version = self
            .version(version_id)?
            .ok_or_else(|| RuntimeError::InvalidInput("NOT_FOUND: version not found".to_owned()))?;
        let compare = self
            .version(compare_to_version_id)?
            .ok_or_else(|| RuntimeError::InvalidInput("NOT_FOUND: version not found".to_owned()))?;
        if version.project_id != compare.project_id {
            return Err(RuntimeError::InvalidInput(
                "PROJECT_SCOPE_DENIED: versions are from different projects".to_owned(),
            ));
        }
        let left = self.candidate(&version.candidate_id)?.ok_or_else(|| {
            RuntimeError::InvalidInput("NOT_FOUND: version candidate not found".to_owned())
        })?;
        let right = self.candidate(&compare.candidate_id)?.ok_or_else(|| {
            RuntimeError::InvalidInput("NOT_FOUND: comparison candidate not found".to_owned())
        })?;
        let left_parts = left
            .manifest_hash
            .as_deref()
            .and_then(|hash| self.cas_read(hash).ok())
            .and_then(|bytes| inspect_glb(&bytes).ok())
            .map(|inspection| inspection.part_ids)
            .unwrap_or_default();
        let right_parts = right
            .manifest_hash
            .as_deref()
            .and_then(|hash| self.cas_read(hash).ok())
            .and_then(|bytes| inspect_glb(&bytes).ok())
            .map(|inspection| inspection.part_ids)
            .unwrap_or_default();
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
        }, RuntimeResourceDescriptor {
            schema_version: "RuntimeResource@1".to_owned(),
            uri: "forgecad://operators/catalog".to_owned(),
            name: "Geometry operator catalog".to_owned(),
            description: "Exact product-owned GeometryProgram@2 operators and canonical hash; unavailable operators are omitted".to_owned(),
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
                    description:
                        "Hash-bound reference image evidence; original path is not retained"
                            .to_owned(),
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
                description: "First-party development-only Skill manifest; no executable payload"
                    .to_owned(),
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
            ["operators", "catalog"] => operator_catalog(),
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
            ["references", reference_id] => serde_json::to_value(ReferenceGetResult {
                schema_version: "ReferenceGetResult@1".to_owned(),
                reference: self
                    .reference(reference_id)?
                    .ok_or_else(|| RuntimeError::InvalidInput("reference not found".to_owned()))?,
            })
            .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?,
            ["skills", skill_id, version] => {
                skill_registry::get_result(skill_id, version).map_err(RuntimeError::InvalidInput)?
            }
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
        self.store
            .append_audit(&forgecad_contracts::AuditEventRecord {
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
        if object.keys().any(|key| key != "typed" && key != "label") {
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
        let prepared_object_id =
            format!("diagnostic-object-{}", &artifact_object.record.sha256[..32]);
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
        let quality_report_id =
            format!("quality-diagnostic-{}", &quality_object.record.sha256[..32]);
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
    /// enter the normal candidate/quality transaction. The compiler is a
    /// fixed, one-shot product-owned sibling Worker; no arbitrary code,
    /// command, environment, or path is accepted from MCP input.
    pub fn prepare_geometry_candidate(
        &self,
        project_id: &str,
        base_version_id: Option<&str>,
        request: Value,
    ) -> Result<Value, RuntimeError> {
        let object = request.as_object().ok_or_else(|| {
            RuntimeError::InvalidInput("geometry request must be an object".to_owned())
        })?;
        if object
            .keys()
            .any(|key| !matches!(key.as_str(), "typed" | "reference_id" | "geometry_program"))
        {
            return Err(RuntimeError::InvalidInput(
                "GEOMETRY_REJECTED: geometry request contains an unsupported field".to_owned(),
            ));
        }
        if object.get("typed").and_then(Value::as_str) != Some("geometry") {
            return Err(RuntimeError::InvalidInput(
                "geometry request requires typed=geometry".to_owned(),
            ));
        }
        let program = object
            .get("geometry_program")
            .ok_or_else(|| RuntimeError::InvalidInput("geometry_program is required".to_owned()))?;
        if program.get("project_id").and_then(Value::as_str) != Some(project_id) {
            return Err(RuntimeError::InvalidInput(
                "GEOMETRY_REJECTED: GeometryProgram project_id must match the target project"
                    .to_owned(),
            ));
        }
        let (reference_id, reference_sha256) = match object.get("reference_id") {
            None => (None, None),
            Some(Value::String(reference_id)) => {
                let reference = self.reference(reference_id)?.ok_or_else(|| {
                    RuntimeError::InvalidInput("NOT_FOUND: reference not found".to_owned())
                })?;
                if reference.project_id != project_id {
                    return Err(RuntimeError::InvalidInput(
                        "REFERENCE_SCOPE_DENIED: geometry reference is outside the target project"
                            .to_owned(),
                    ));
                }
                (Some(reference.reference_id), Some(reference.object_sha256))
            }
            Some(_) => {
                return Err(RuntimeError::InvalidInput(
                    "GEOMETRY_REJECTED: reference_id must be an identifier".to_owned(),
                ));
            }
        };
        let is_v2 =
            program.get("schema_version").and_then(Value::as_str) == Some("GeometryProgram@2");
        let artifact = compile_geometry_with_runtime_worker(program, None)
            .map_err(|error| RuntimeError::InvalidInput(format!("GEOMETRY_REJECTED: {error}")))?;
        let inspection = strict_glb_inspection(&artifact.glb)?;
        validate_worker_metadata(&artifact, &inspection)?;
        let hard_gate_passed = if is_v2 {
            inspection.hard_gate_passed
        } else {
            physical_geometry_passed(&inspection)
        };
        if !hard_gate_passed {
            return Err(RuntimeError::InvalidInput(format!(
                "GEOMETRY_REJECTED: strict GLB readback failed: {}",
                inspection.failure_codes.join(",")
            )));
        }
        let glb_object =
            self.put_object(&artifact.glb, None, "model/gltf-binary", "geometry-glb")?;
        let geometry_program_object = if is_v2 {
            let mut draft = program.clone();
            draft
                .as_object_mut()
                .expect("GeometryProgram was checked as an object")
                .remove("canonical_sha256");
            let bytes = canonical_json_bytes(&draft)
                .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?;
            let object = self.put_object(
                &bytes,
                Some(&inspection.program_sha256),
                "application/json",
                "geometry-program-v2",
            )?;
            if object.record.sha256 != inspection.program_sha256 {
                return Err(RuntimeError::InvalidInput(
                    "GEOMETRY_REJECTED: canonical GeometryProgram hash does not match CAS"
                        .to_owned(),
                ));
            }
            Some(object)
        } else {
            None
        };
        let prepared = self.prepare_candidate(
            project_id,
            base_version_id,
            &format!("geometry-object-{}", &glb_object.record.sha256[..32]),
            &glb_object.record.sha256,
            request.clone(),
        )?;
        let readback = if is_v2 {
            artifact_readback_v2_value(
                &glb_object.record.sha256,
                &prepared.candidate.candidate_id,
                &inspection,
                glb_object.record.size_bytes,
            )
        } else {
            artifact_readback_v1_value(
                &glb_object.record.sha256,
                &prepared.candidate.candidate_id,
                &inspection,
                glb_object.record.size_bytes,
            )
        };
        if is_v2 {
            validate_artifact_readback_v2_output(&readback)?;
        }
        let readback_object = if is_v2 {
            let bytes = canonical_json_bytes(&readback)
                .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?;
            Some(self.put_object(
                &bytes,
                None,
                "application/json",
                "geometry-artifact-readback-v2",
            )?)
        } else {
            None
        };
        let quality_report_id = if is_v2 {
            format!("quality-geometry-{}", Uuid::new_v4().simple())
        } else {
            String::new()
        };
        let mut quality_report = if is_v2 {
            json!({
                "schema_version":"GeometryQualityReport@2",
                "scope":"mcp010b-strict-glb-bin-accessor-hard-gates",
                "quality_report_id":quality_report_id,
                "candidate_id":prepared.candidate.candidate_id,
                "artifact_sha256":glb_object.record.sha256,
                "program_sha256":inspection.program_sha256,
                "operator_catalog_sha256":inspection.operator_catalog_sha256,
                "readback_config_sha256":inspection.readback_config_sha256,
                "artifact_readback_object_sha256":readback_object.as_ref().expect("V2 readback object").record.sha256,
                "integrity":strict_integrity_value(&inspection),
                "hard_gate_passed":hard_gate_passed,
                "canonical_sha256":""
            })
        } else {
            json!({
                "schema_version":"GeometryQualityReport@1",
                "scope":"legacy-compatible-physical-glb-readback",
                "candidate_id":prepared.candidate.candidate_id,
                "artifact_sha256":glb_object.record.sha256,
                "checks":{"non_empty_glb":inspection.triangle_count > 0,"finite_positions":inspection.non_finite_count == 0,"indices_in_bounds":inspection.invalid_index_count == 0,"no_degenerate_triangles":inspection.degenerate_triangle_count == 0,"part_lineage":inspection.part_coverage == 1.0,"budget":artifact.triangle_count > 0},
                "triangle_count":inspection.triangle_count,
                "part_ids":inspection.part_ids,
                "hard_gate_passed":hard_gate_passed
            })
        };
        if is_v2 {
            quality_report["canonical_sha256"] =
                Value::String(canonical_json_hash(&quality_report));
            validate_geometry_quality_report_v2_output(&quality_report)?;
        }
        let quality_bytes = canonical_json_bytes(&quality_report)
            .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?;
        let quality_object = self.put_object(
            &quality_bytes,
            None,
            "application/json",
            "geometry-quality-report",
        )?;
        let quality_report_id = if is_v2 {
            quality_report_id
        } else {
            format!("quality-geometry-{}", &quality_object.record.sha256[..32])
        };
        let candidate = if is_v2 {
            let program_object = geometry_program_object.as_ref().expect("V2 program object");
            let evidence = geometry_candidate_evidence_value(
                &prepared.candidate,
                reference_id.as_deref(),
                reference_sha256.as_deref(),
                &inspection,
                &program_object.record.sha256,
                &glb_object.record.sha256,
                &readback_object
                    .as_ref()
                    .expect("V2 readback object")
                    .record
                    .sha256,
                &quality_object.record.sha256,
                &quality_report_id,
            );
            validate_geometry_candidate_evidence_output(&evidence)?;
            let evidence: GeometryCandidateEvidenceRecord = serde_json::from_value(evidence)
                .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?;
            self.store
                .record_geometry_candidate_evidence_and_mark_quality(
                    &evidence,
                    hard_gate_passed,
                    &now_string(),
                )?
        } else {
            self.mark_candidate_quality(
                &prepared.candidate.candidate_id,
                &quality_report_id,
                hard_gate_passed,
            )?
        };
        if is_v2 {
            let result = json!({
                "schema_version":"GeometryPrepareResult@2",
                "candidate":candidate,
                "job":prepared.job,
                "operator_catalog":operator_catalog(),
                "artifact":readback
            });
            validate_geometry_prepare_result_v2_output(&result)?;
            Ok(result)
        } else {
            Ok(json!({
                "schema_version":"GeometryPrepareResult@1",
                "candidate":candidate,
                "job":prepared.job,
                "artifact":readback
            }))
        }
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
        let geometry_program = object
            .get("geometry_program")
            .ok_or_else(|| RuntimeError::InvalidInput("geometry_program is required".to_owned()))?;
        if geometry_program
            .get("schema_version")
            .and_then(Value::as_str)
            == Some("GeometryProgram@2")
        {
            return Err(RuntimeError::InvalidInput(
                "APPEARANCE_V2_UNAVAILABLE: AppearanceProgram@2, atlas and PBR texture receipts are scheduled for MCP010E".to_owned(),
            ));
        }
        let appearance_program = object.get("appearance_program").ok_or_else(|| {
            RuntimeError::InvalidInput("appearance_program is required".to_owned())
        })?;
        let artifact =
            compile_geometry_with_runtime_worker(geometry_program, Some(appearance_program))
                .map_err(|error| {
                    RuntimeError::InvalidInput(format!("APPEARANCE_REJECTED: {error}"))
                })?;
        let inspection = strict_glb_inspection(&artifact.glb)?;
        validate_worker_metadata(&artifact, &inspection)?;
        let hard_gate_passed = physical_geometry_passed(&inspection);
        if !hard_gate_passed {
            return Err(RuntimeError::InvalidInput(format!(
                "APPEARANCE_REJECTED: physical GLB readback failed: {}",
                inspection.failure_codes.join(",")
            )));
        }
        // Rendering remains inside the isolated Worker and runs before any
        // candidate/GLB/render CAS object is persisted. A timeout, crash or
        // malformed response therefore leaves no candidate/version/CAS write.
        let render_passes = render_fixed_with_runtime_worker(geometry_program, appearance_program)
            .map_err(|error| RuntimeError::InvalidInput(format!("RENDER_REJECTED: {error}")))?;
        let glb_object =
            self.put_object(&artifact.glb, None, "model/gltf-binary", "appearance-glb")?;
        let prepared = self.prepare_candidate(
            project_id,
            base_version_id,
            &format!("appearance-object-{}", &glb_object.record.sha256[..32]),
            &glb_object.record.sha256,
            request.clone(),
        )?;
        let mut pass_artifacts = serde_json::Map::new();
        for pass in &render_passes {
            let object = self.put_object(
                &pass.png,
                None,
                "image/png",
                &format!("fixed-render-{}", pass.pass),
            )?;
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
        let render_set_object =
            self.put_object(&render_set_bytes, None, "application/json", "render-set")?;
        let quality_report_id = format!("quality-appearance-{}", &glb_object.record.sha256[..32]);
        let mut quality_report = json!({
            "schema_version":"QualityReport@1",
            "quality_report_id":quality_report_id,
            "candidate_id":prepared.candidate.candidate_id,
            "hard_gate_passed":hard_gate_passed,
            "checks":[
                {"check_id":"uv_in_range","status":if inspection.uv_non_finite_count == 0 && inspection.zero_area_uv_triangle_count == 0 {"passed"} else {"failed"},"message":"UV values are read back from the GLB BIN"},
                {"check_id":"tangent_basis","status":if inspection.tangent_non_finite_count == 0 && inspection.tangent_orthogonality_error_count == 0 && inspection.tangent_handedness_error_count == 0 {"passed"} else {"failed"},"message":"tangent attributes are read back from the GLB BIN"},
                {"check_id":"pbr_material_zones","status":if !inspection.material_zone_ids.is_empty() {"passed"} else {"failed"},"message":"typed material zones are present in GLB lineage"},
                {"check_id":"fixed_render","status":"passed","message":"beauty, silhouette, normal and part-id passes are in CAS"},
                {"check_id":"render_set_object","status":"passed","message":format!("render set {} is CAS-backed", render_set_object.record.sha256)},
            ],
            "canonical_sha256":""
        });
        let quality_hash = canonical_json_hash(&quality_report);
        quality_report["canonical_sha256"] = Value::String(quality_hash);
        let quality_bytes = canonical_json_bytes(&quality_report)
            .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?;
        let quality_object = self.put_object(
            &quality_bytes,
            None,
            "application/json",
            "appearance-quality-report",
        )?;
        let candidate = self.mark_candidate_quality(
            &prepared.candidate.candidate_id,
            &quality_report_id,
            hard_gate_passed,
        )?;
        let readback = artifact_readback_v1_value(
            &glb_object.record.sha256,
            &candidate.candidate_id,
            &inspection,
            glb_object.record.size_bytes,
        );
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
            RuntimeError::InvalidInput(
                "CHANGE_BASE_REQUIRED: change_prepare requires base_version_id".to_owned(),
            )
        })?;
        let object = request.as_object().ok_or_else(|| {
            RuntimeError::InvalidInput("change request must be an object".to_owned())
        })?;
        if object.get("typed").and_then(Value::as_str) != Some("change") {
            return Err(RuntimeError::InvalidInput(
                "change request requires typed=change".to_owned(),
            ));
        }
        let change_set = object
            .get("change_set")
            .ok_or_else(|| RuntimeError::InvalidInput("change_set is required".to_owned()))?;
        let change = change_set
            .as_object()
            .ok_or_else(|| RuntimeError::InvalidInput("change_set must be an object".to_owned()))?;
        let part_id = change
            .get("part_id")
            .and_then(Value::as_str)
            .filter(|value| is_opaque_id(value))
            .ok_or_else(|| {
                RuntimeError::InvalidInput("change_set.part_id is invalid".to_owned())
            })?;
        let operation = change
            .get("operation")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                RuntimeError::InvalidInput("change_set.operation is required".to_owned())
            })?;
        if !matches!(
            operation,
            "transform" | "material_update" | "replace_geometry"
        ) {
            return Err(RuntimeError::InvalidInput(
                "change_set.operation is outside the MVP allowlist".to_owned(),
            ));
        }
        let parameters = change
            .get("parameters")
            .and_then(Value::as_object)
            .ok_or_else(|| {
                RuntimeError::InvalidInput("change_set.parameters must be an object".to_owned())
            })?;
        if parameters.len() > 16
            || change.keys().any(|key| {
                !matches!(
                    key.as_str(),
                    "part_id" | "operation" | "parameters" | "reason"
                )
            })
        {
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
                nodes
                    .iter()
                    .any(|node| node.get("part_id").and_then(Value::as_str) == Some(part_id))
            })
            .unwrap_or(false);
        if !has_part {
            return Err(RuntimeError::InvalidInput(
                "CHANGE_PART_NOT_FOUND: change part_id is absent from the new GeometryProgram"
                    .to_owned(),
            ));
        }
        let appearance_program = object.get("appearance_program").ok_or_else(|| {
            RuntimeError::InvalidInput(
                "appearance_program is required for change_prepare".to_owned(),
            )
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
        output.insert(
            "schema_version".to_owned(),
            Value::String("ChangePrepareResult@1".to_owned()),
        );
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
            return Err(RuntimeError::InvalidInput(
                "artifact_id must be a GLB SHA-256".to_owned(),
            ));
        }
        self.ensure_candidate_artifact_binding(candidate_id, artifact_id)?;
        let record = self.store.get_object(artifact_id)?.ok_or_else(|| {
            RuntimeError::InvalidInput("artifact readback object is unavailable".to_owned())
        })?;
        if record.mime != "model/gltf-binary"
            || !matches!(record.kind.as_str(), "geometry-glb" | "appearance-glb")
        {
            return Err(RuntimeError::InvalidInput(
                "artifact is not a ForgeCAD GLB".to_owned(),
            ));
        }
        let bytes = self.cas_read(artifact_id)?;
        let inspection = strict_glb_inspection(&bytes)?;
        if inspection.artifact_schema_version == "ArtifactReadback@2" {
            if !v2_readback_shape_is_serializable(&inspection) {
                return Err(RuntimeError::InvalidInput(
                    "STRICT_GLB_READBACK_FAILED: V2 artifact lineage or catalog binding is not contract-safe"
                        .to_owned(),
                ));
            }
            Ok(artifact_readback_v2_value(
                artifact_id,
                candidate_id,
                &inspection,
                record.size_bytes,
            ))
        } else {
            Ok(artifact_readback_v1_value(
                artifact_id,
                candidate_id,
                &inspection,
                record.size_bytes,
            ))
        }
    }

    /// Read a bounded GLB payload for the optional local Viewer. This is an
    /// authenticated IPC read model operation, never an MCP tool and never a
    /// database/CAS write.
    pub fn artifact_bytes(
        &self,
        artifact_id: &str,
        candidate_id: &str,
    ) -> Result<Value, RuntimeError> {
        validate_id(artifact_id)?;
        validate_id(candidate_id)?;
        if !forgecad_contracts::is_sha256(artifact_id) {
            return Err(RuntimeError::InvalidInput(
                "artifact_id must be a GLB SHA-256".to_owned(),
            ));
        }
        self.ensure_candidate_artifact_binding(candidate_id, artifact_id)?;
        let record = self.store.get_object(artifact_id)?.ok_or_else(|| {
            RuntimeError::InvalidInput("artifact bytes are unavailable".to_owned())
        })?;
        if record.mime != "model/gltf-binary"
            || !matches!(record.kind.as_str(), "geometry-glb" | "appearance-glb")
        {
            return Err(RuntimeError::InvalidInput(
                "artifact is not a ForgeCAD GLB".to_owned(),
            ));
        }
        if record.size_bytes > 32 * 1024 * 1024 {
            return Err(RuntimeError::InvalidInput(
                "artifact exceeds Viewer read capacity".to_owned(),
            ));
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

    /// Read a bounded reference image for the optional local Viewer. The
    /// reference remains owned by Runtime/CAS; this method only exposes a
    /// candidate/project-bound read projection and never mutates state.
    pub fn reference_bytes(
        &self,
        reference_id: &str,
        project_id: &str,
    ) -> Result<Value, RuntimeError> {
        validate_id(reference_id)?;
        validate_id(project_id)?;
        let reference = self.reference(reference_id)?.ok_or_else(|| {
            RuntimeError::InvalidInput("reference bytes are unavailable".to_owned())
        })?;
        if reference.project_id != project_id {
            return Err(RuntimeError::InvalidInput(
                "REFERENCE_PROJECT_MISMATCH: reference is not in this project".to_owned(),
            ));
        }
        if !matches!(reference.mime.as_str(), "image/png" | "image/jpeg") {
            return Err(RuntimeError::InvalidInput(
                "reference is not a supported image".to_owned(),
            ));
        }
        if reference.size_bytes > 32 * 1024 * 1024 {
            return Err(RuntimeError::InvalidInput(
                "reference exceeds Viewer read capacity".to_owned(),
            ));
        }
        let record = self
            .store
            .get_object(&reference.object_sha256)?
            .ok_or_else(|| {
                RuntimeError::InvalidInput("reference CAS object is unavailable".to_owned())
            })?;
        if record.mime != reference.mime || record.size_bytes != reference.size_bytes {
            return Err(RuntimeError::InvalidInput(
                "REFERENCE_INTEGRITY_FAILED: CAS metadata differs from ReferenceEvidence"
                    .to_owned(),
            ));
        }
        let bytes = self.cas_read(&reference.object_sha256)?;
        if bytes.len() as u64 != reference.size_bytes
            || sha256_hex(&bytes) != reference.object_sha256
        {
            return Err(RuntimeError::InvalidInput(
                "REFERENCE_INTEGRITY_FAILED: CAS bytes do not match ReferenceEvidence".to_owned(),
            ));
        }
        Ok(json!({
            "schema_version":"ReferenceBytesRead@1",
            "reference_id":reference.reference_id,
            "project_id":reference.project_id,
            "mime":reference.mime,
            "width":reference.width,
            "height":reference.height,
            "size_bytes":bytes.len(),
            "sha256":sha256_hex(&bytes),
            "bytes_base64":base64::engine::general_purpose::STANDARD.encode(bytes)
        }))
    }

    fn ensure_candidate_artifact_binding(
        &self,
        candidate_id: &str,
        artifact_id: &str,
    ) -> Result<CandidateRecord, RuntimeError> {
        let candidate = self.candidate(candidate_id)?.ok_or_else(|| {
            RuntimeError::InvalidInput(
                "CANDIDATE_ARTIFACT_MISMATCH: candidate is unavailable".to_owned(),
            )
        })?;
        if candidate.manifest_hash.as_deref() != Some(artifact_id)
            || candidate.prepared_object_sha256.as_deref() != Some(artifact_id)
        {
            return Err(RuntimeError::InvalidInput(
                "CANDIDATE_ARTIFACT_MISMATCH: artifact is not bound to this candidate".to_owned(),
            ));
        }
        Ok(candidate)
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
        let stored_candidate = self.candidate(&request.candidate_id)?.ok_or_else(|| {
            RuntimeError::InvalidInput("NOT_FOUND: candidate not found".to_owned())
        })?;
        if stored_candidate.prepared_object_sha256.as_deref()
            != Some(request.prepared_object_sha256.as_str())
        {
            return Err(RuntimeError::InvalidInput(
                "CANDIDATE_HASH_MISMATCH: confirmation hash does not match the prepared candidate"
                    .to_owned(),
            ));
        }
        let candidate = self.ensure_candidate_artifact_binding(
            &request.candidate_id,
            &request.prepared_object_sha256,
        )?;
        self.revalidate_candidate_for_confirmation(&candidate, &request.prepared_object_sha256)?;
        Ok(self.store.confirm_candidate(request, &now_string())?)
    }

    /// Re-read the immutable CAS object and, for V2 GLBs, the complete
    /// candidate-bound provenance chain before any Store confirmation path can
    /// create a version. Restore confirmation uses the same gate rather than
    /// trusting the source candidate's former reviewable state.
    fn revalidate_candidate_for_confirmation(
        &self,
        candidate: &CandidateRecord,
        prepared_object_sha256: &str,
    ) -> Result<(), RuntimeError> {
        let record = self
            .store
            .get_object(prepared_object_sha256)?
            .ok_or_else(|| {
                RuntimeError::InvalidInput(
                    "QUALITY_HARD_GATE_FAILED: candidate GLB is unavailable".to_owned(),
                )
            })?;
        let bytes = self.cas_read(prepared_object_sha256)?;
        if record.mime == "model/gltf-binary" {
            let inspection = strict_glb_inspection(&bytes)?;
            match self
                .store
                .get_geometry_candidate_evidence(&candidate.candidate_id)?
            {
                Some(evidence) => {
                    self.revalidate_v2_geometry_evidence(&candidate, &inspection, &evidence)?
                }
                None if inspection.artifact_schema_version == "ArtifactReadback@2" => {
                    return Err(RuntimeError::InvalidInput(
                        "QUALITY_HARD_GATE_FAILED: V2 candidate is missing durable geometry evidence"
                            .to_owned(),
                    ));
                }
                None if !candidate.quality_hard_gate_passed => {
                    return Err(RuntimeError::InvalidInput(
                        "QUALITY_HARD_GATE_FAILED: candidate did not pass Runtime quality"
                            .to_owned(),
                    ));
                }
                None => {}
            }
        } else if !candidate.quality_hard_gate_passed {
            return Err(RuntimeError::InvalidInput(
                "QUALITY_HARD_GATE_FAILED: candidate did not pass Runtime quality".to_owned(),
            ));
        }
        Ok(())
    }

    fn revalidate_v2_geometry_evidence(
        &self,
        candidate: &CandidateRecord,
        inspection: &integrity::GlbIntegrity,
        evidence: &GeometryCandidateEvidenceRecord,
    ) -> Result<(), RuntimeError> {
        if evidence.project_id != candidate.project_id
            || evidence.candidate_id != candidate.candidate_id
            || candidate.prepared_object_sha256.as_deref()
                != Some(evidence.artifact_object_sha256.as_str())
            || candidate.quality_report_id.as_deref() != Some(evidence.quality_report_id.as_str())
            || !candidate.quality_hard_gate_passed
            || inspection.artifact_schema_version != "ArtifactReadback@2"
            || !inspection.hard_gate_passed
            || inspection.program_sha256 != evidence.geometry_program_sha256
            || inspection.operator_catalog_sha256.as_deref()
                != Some(evidence.operator_catalog_sha256.as_str())
            || inspection.readback_config_sha256 != evidence.readback_config_sha256
            || evidence.operator_catalog_sha256 != operator_catalog_sha256()
        {
            return Err(RuntimeError::InvalidInput(
                "QUALITY_HARD_GATE_FAILED: V2 geometry evidence does not match candidate readback"
                    .to_owned(),
            ));
        }
        if let Some(reference_id) = evidence.reference_id.as_deref() {
            let reference = self.reference(reference_id)?.ok_or_else(|| {
                RuntimeError::InvalidInput(
                    "QUALITY_HARD_GATE_FAILED: candidate-bound reference is unavailable".to_owned(),
                )
            })?;
            if reference.project_id != candidate.project_id
                || evidence.reference_sha256.as_deref() != Some(reference.object_sha256.as_str())
            {
                return Err(RuntimeError::InvalidInput(
                    "QUALITY_HARD_GATE_FAILED: candidate-bound reference drifted".to_owned(),
                ));
            }
        } else if evidence.reference_sha256.is_some() {
            return Err(RuntimeError::InvalidInput(
                "QUALITY_HARD_GATE_FAILED: unbound candidate has reference hash".to_owned(),
            ));
        }

        let program_draft: Value =
            serde_json::from_slice(&self.cas_read(&evidence.geometry_program_object_sha256)?)
                .map_err(|_| {
                    RuntimeError::InvalidInput(
                        "QUALITY_HARD_GATE_FAILED: persisted GeometryProgram is not JSON"
                            .to_owned(),
                    )
                })?;
        let hash = hash_geometry_program_with_runtime_worker(&program_draft).map_err(|error| {
            RuntimeError::InvalidInput(format!(
                "QUALITY_HARD_GATE_FAILED: persisted GeometryProgram validation failed: {error}"
            ))
        })?;
        if hash.get("canonical_sha256").and_then(Value::as_str)
            != Some(evidence.geometry_program_sha256.as_str())
            || hash.get("operator_catalog_sha256").and_then(Value::as_str)
                != Some(evidence.operator_catalog_sha256.as_str())
        {
            return Err(RuntimeError::InvalidInput(
                "QUALITY_HARD_GATE_FAILED: persisted GeometryProgram provenance drifted".to_owned(),
            ));
        }

        let expected_readback = artifact_readback_v2_value(
            &evidence.artifact_object_sha256,
            &candidate.candidate_id,
            inspection,
            self.store
                .get_object(&evidence.artifact_object_sha256)?
                .ok_or_else(|| {
                    RuntimeError::InvalidInput(
                        "QUALITY_HARD_GATE_FAILED: candidate GLB object is unavailable".to_owned(),
                    )
                })?
                .size_bytes,
        );
        validate_artifact_readback_v2_output(&expected_readback)?;
        let stored_readback: Value =
            serde_json::from_slice(&self.cas_read(&evidence.artifact_readback_object_sha256)?)
                .map_err(|_| {
                    RuntimeError::InvalidInput(
                        "QUALITY_HARD_GATE_FAILED: stored ArtifactReadback is not JSON".to_owned(),
                    )
                })?;
        validate_artifact_readback_v2_output(&stored_readback)?;
        let stored_readback_bytes = canonical_json_bytes(&stored_readback)
            .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?;
        let expected_readback_bytes = canonical_json_bytes(&expected_readback)
            .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?;
        if stored_readback_bytes != expected_readback_bytes {
            return Err(RuntimeError::InvalidInput(
                "QUALITY_HARD_GATE_FAILED: stored ArtifactReadback does not match GLB".to_owned(),
            ));
        }

        let quality_report: Value =
            serde_json::from_slice(&self.cas_read(&evidence.quality_report_object_sha256)?)
                .map_err(|_| {
                    RuntimeError::InvalidInput(
                        "QUALITY_HARD_GATE_FAILED: stored geometry quality report is not JSON"
                            .to_owned(),
                    )
                })?;
        validate_geometry_quality_report_v2_output(&quality_report)?;
        if quality_report
            .get("quality_report_id")
            .and_then(Value::as_str)
            != Some(evidence.quality_report_id.as_str())
            || quality_report.get("candidate_id").and_then(Value::as_str)
                != Some(candidate.candidate_id.as_str())
            || quality_report
                .get("artifact_sha256")
                .and_then(Value::as_str)
                != Some(evidence.artifact_object_sha256.as_str())
            || quality_report.get("program_sha256").and_then(Value::as_str)
                != Some(evidence.geometry_program_sha256.as_str())
            || quality_report
                .get("operator_catalog_sha256")
                .and_then(Value::as_str)
                != Some(evidence.operator_catalog_sha256.as_str())
            || quality_report
                .get("readback_config_sha256")
                .and_then(Value::as_str)
                != Some(evidence.readback_config_sha256.as_str())
            || quality_report
                .get("artifact_readback_object_sha256")
                .and_then(Value::as_str)
                != Some(evidence.artifact_readback_object_sha256.as_str())
        {
            return Err(RuntimeError::InvalidInput(
                "QUALITY_HARD_GATE_FAILED: stored geometry quality report provenance drifted"
                    .to_owned(),
            ));
        }
        Ok(())
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
        let mut prepared = self
            .store
            .prepare_restore_candidate(request, &now_string())?;
        let source_version = self.version(&request.source_version_id)?.ok_or_else(|| {
            RuntimeError::InvalidInput(
                "NOT_FOUND: restore source version disappeared during preparation".to_owned(),
            )
        })?;
        if source_version.project_id != request.project_id {
            return Err(RuntimeError::InvalidInput(
                "PROJECT_SCOPE_DENIED: restore source version is outside the project".to_owned(),
            ));
        }
        let source_candidate = self
            .candidate(&source_version.candidate_id)?
            .ok_or_else(|| {
                RuntimeError::InvalidInput(
                    "NOT_FOUND: restore source candidate disappeared during preparation".to_owned(),
                )
            })?;
        prepared.candidate =
            self.materialize_restore_candidate(&prepared.candidate, &source_candidate)?;
        Ok(prepared)
    }

    pub fn confirm_restore(
        &self,
        request: &RestoreConfirmRequest,
    ) -> Result<RestoreConfirmResult, RuntimeError> {
        let candidate = self.candidate(&request.candidate_id)?.ok_or_else(|| {
            RuntimeError::InvalidInput("NOT_FOUND: restore candidate not found".to_owned())
        })?;
        if candidate.project_id != request.project_id {
            return Err(RuntimeError::InvalidInput(
                "PROJECT_SCOPE_DENIED: restore candidate is outside the project".to_owned(),
            ));
        }
        if candidate.source_version_id.as_deref() != Some(request.source_version_id.as_str()) {
            return Err(RuntimeError::InvalidInput(
                "RESTORE_SOURCE_MISMATCH: restore candidate is not bound to the requested source version"
                    .to_owned(),
            ));
        }
        if candidate.prepared_object_sha256.as_deref()
            != Some(request.prepared_object_sha256.as_str())
        {
            return Err(RuntimeError::InvalidInput(
                "CANDIDATE_HASH_MISMATCH: confirmation hash does not match the prepared candidate"
                    .to_owned(),
            ));
        }
        self.revalidate_candidate_for_confirmation(&candidate, &request.prepared_object_sha256)?;
        Ok(self.store.restore_confirm(request, &now_string())?)
    }

    /// A restore starts with a `prepared` candidate. For legacy candidates we
    /// retain the historical quality receipt after verifying the source CAS
    /// bytes. V2 candidates instead receive fresh candidate-bound readback,
    /// quality and evidence records before becoming reviewable.
    fn materialize_restore_candidate(
        &self,
        restored_candidate: &CandidateRecord,
        source_candidate: &CandidateRecord,
    ) -> Result<CandidateRecord, RuntimeError> {
        if source_candidate.state != "confirmed" || !source_candidate.quality_hard_gate_passed {
            return Err(RuntimeError::InvalidInput(
                "RESTORE_SOURCE_UNCONFIRMED: restore source must be confirmed and quality-passing"
                    .to_owned(),
            ));
        }
        let artifact_sha256 = restored_candidate
            .prepared_object_sha256
            .as_deref()
            .ok_or_else(|| {
                RuntimeError::InvalidInput(
                    "CANDIDATE_HASH_MISMATCH: restore candidate has no prepared object hash"
                        .to_owned(),
                )
            })?;
        let object = self.store.get_object(artifact_sha256)?.ok_or_else(|| {
            RuntimeError::InvalidInput(
                "REFERENCE_TRANSFER_UNAVAILABLE: restore source CAS object is unavailable"
                    .to_owned(),
            )
        })?;
        let bytes = self.cas_read(artifact_sha256)?;
        if object.mime == "model/gltf-binary" {
            let inspection = strict_glb_inspection(&bytes)?;
            if inspection.artifact_schema_version == "ArtifactReadback@2" {
                let source_evidence = self
                    .store
                    .get_geometry_candidate_evidence(&source_candidate.candidate_id)?
                    .ok_or_else(|| {
                        RuntimeError::InvalidInput(
                            "QUALITY_HARD_GATE_FAILED: V2 restore source is missing durable geometry evidence"
                                .to_owned(),
                        )
                    })?;
                self.revalidate_v2_geometry_evidence(
                    source_candidate,
                    &inspection,
                    &source_evidence,
                )?;
                return self.record_restored_v2_geometry_evidence(
                    restored_candidate,
                    &source_evidence,
                    &inspection,
                    object.size_bytes,
                );
            }
        }
        let quality_report_id = source_candidate
            .quality_report_id
            .as_deref()
            .ok_or_else(|| {
                RuntimeError::InvalidInput(
                    "QUALITY_HARD_GATE_FAILED: restore source has no quality report".to_owned(),
                )
            })?;
        self.mark_candidate_quality(&restored_candidate.candidate_id, quality_report_id, true)
    }

    fn record_restored_v2_geometry_evidence(
        &self,
        restored_candidate: &CandidateRecord,
        source_evidence: &GeometryCandidateEvidenceRecord,
        inspection: &integrity::GlbIntegrity,
        artifact_size_bytes: u64,
    ) -> Result<CandidateRecord, RuntimeError> {
        let artifact_sha256 = restored_candidate
            .prepared_object_sha256
            .as_deref()
            .ok_or_else(|| {
                RuntimeError::InvalidInput(
                    "CANDIDATE_HASH_MISMATCH: restore candidate has no prepared object hash"
                        .to_owned(),
                )
            })?;
        if artifact_sha256 != source_evidence.artifact_object_sha256 {
            return Err(RuntimeError::InvalidInput(
                "QUALITY_HARD_GATE_FAILED: restored V2 artifact does not match source evidence"
                    .to_owned(),
            ));
        }
        let readback = artifact_readback_v2_value(
            artifact_sha256,
            &restored_candidate.candidate_id,
            inspection,
            artifact_size_bytes,
        );
        validate_artifact_readback_v2_output(&readback)?;
        let readback_bytes = canonical_json_bytes(&readback)
            .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?;
        let readback_object = self.put_object(
            &readback_bytes,
            None,
            "application/json",
            "geometry-artifact-readback-v2",
        )?;
        let quality_report_id = format!("quality-geometry-{}", Uuid::new_v4().simple());
        let mut quality_report = json!({
            "schema_version":"GeometryQualityReport@2",
            "scope":"mcp010b-strict-glb-bin-accessor-hard-gates",
            "quality_report_id":quality_report_id,
            "candidate_id":restored_candidate.candidate_id,
            "artifact_sha256":artifact_sha256,
            "program_sha256":inspection.program_sha256,
            "operator_catalog_sha256":inspection.operator_catalog_sha256,
            "readback_config_sha256":inspection.readback_config_sha256,
            "artifact_readback_object_sha256":readback_object.record.sha256,
            "integrity":strict_integrity_value(inspection),
            "hard_gate_passed":inspection.hard_gate_passed,
            "canonical_sha256":""
        });
        quality_report["canonical_sha256"] = Value::String(canonical_json_hash(&quality_report));
        validate_geometry_quality_report_v2_output(&quality_report)?;
        let quality_bytes = canonical_json_bytes(&quality_report)
            .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?;
        let quality_object = self.put_object(
            &quality_bytes,
            None,
            "application/json",
            "geometry-quality-report",
        )?;
        let evidence = geometry_candidate_evidence_value(
            restored_candidate,
            source_evidence.reference_id.as_deref(),
            source_evidence.reference_sha256.as_deref(),
            inspection,
            &source_evidence.geometry_program_object_sha256,
            artifact_sha256,
            &readback_object.record.sha256,
            &quality_object.record.sha256,
            &quality_report_id,
        );
        validate_geometry_candidate_evidence_output(&evidence)?;
        let evidence: GeometryCandidateEvidenceRecord = serde_json::from_value(evidence)
            .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?;
        self.store
            .record_geometry_candidate_evidence_and_mark_quality(&evidence, true, &now_string())
            .map_err(RuntimeError::from)
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
            "operator_catalog_get" => Ok(self.active_operator_catalog()),
            "geometry_program_hash" => self.geometry_program_hash(payload),
            "render_pass_get" => {
                let render_set_hash = payload
                    .get("render_set_hash")
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        RuntimeError::InvalidInput("render_set_hash is required".to_owned())
                    })?;
                let pass = payload
                    .get("pass")
                    .and_then(Value::as_str)
                    .ok_or_else(|| RuntimeError::InvalidInput("pass is required".to_owned()))?;
                self.render_pass_get(render_set_hash, pass)
            }
            "visual_evidence_get" => {
                let candidate_id = payload
                    .get("candidate_id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        RuntimeError::InvalidInput("candidate_id is required".to_owned())
                    })?;
                self.visual_evidence(candidate_id)
            }
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
                let reference = self
                    .reference(id)?
                    .ok_or_else(|| RuntimeError::InvalidInput("reference not found".to_owned()))?;
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
                    .ok_or_else(|| {
                        RuntimeError::InvalidInput("artifact_id is required".to_owned())
                    })?;
                let candidate_id = payload
                    .get("candidate_id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        RuntimeError::InvalidInput("candidate_id is required".to_owned())
                    })?;
                Ok(self.artifact_readback(artifact_id, candidate_id)?)
            }
            "artifact_bytes_get" => {
                let artifact_id = payload
                    .get("artifact_id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        RuntimeError::InvalidInput("artifact_id is required".to_owned())
                    })?;
                let candidate_id = payload
                    .get("candidate_id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        RuntimeError::InvalidInput("candidate_id is required".to_owned())
                    })?;
                Ok(self.artifact_bytes(artifact_id, candidate_id)?)
            }
            "reference_bytes_get" => {
                let reference_id = payload
                    .get("reference_id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        RuntimeError::InvalidInput("reference_id is required".to_owned())
                    })?;
                let project_id = payload
                    .get("project_id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        RuntimeError::InvalidInput("project_id is required".to_owned())
                    })?;
                Ok(self.reference_bytes(reference_id, project_id)?)
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
                    .ok_or_else(|| {
                        RuntimeError::InvalidInput("project_id is required".to_owned())
                    })?;
                Ok(serde_json::to_value(self.candidates(project_id)?)
                    .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?)
            }
            "quality_get" => {
                let candidate_id = payload
                    .get("candidate_id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        RuntimeError::InvalidInput("candidate_id is required".to_owned())
                    })?;
                let reference_id = payload.get("reference_id").and_then(Value::as_str);
                Ok(self.quality(candidate_id, reference_id)?)
            }
            "version_diff" => {
                let version_id = payload
                    .get("version_id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        RuntimeError::InvalidInput("version_id is required".to_owned())
                    })?;
                let compare_to_version_id = payload
                    .get("compare_to_version_id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        RuntimeError::InvalidInput("compare_to_version_id is required".to_owned())
                    })?;
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
                    (Some(prepared_object_id), Some(prepared_object_sha256)) => {
                        Ok(serde_json::to_value(self.prepare_candidate(
                            project_id,
                            base_version_id,
                            prepared_object_id,
                            prepared_object_sha256,
                            request,
                        )?)
                        .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?)
                    }
                    (None, None) => Ok(serde_json::to_value(self.prepare_diagnostic_candidate(
                        project_id,
                        base_version_id,
                        request,
                    )?)
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
                    .ok_or_else(|| {
                        RuntimeError::InvalidInput("project_id is required".to_owned())
                    })?;
                let base_version_id = payload.get("base_version_id").and_then(Value::as_str);
                let request = payload.get("request").cloned().unwrap_or_else(|| json!({}));
                Ok(self.prepare_geometry_candidate(project_id, base_version_id, request)?)
            }
            "reference_compare_prepare" => {
                let project_id = payload
                    .get("project_id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        RuntimeError::InvalidInput("project_id is required".to_owned())
                    })?;
                self.prepare_reference_comparison(project_id, payload.clone())
            }
            "visual_review_submit" => self.submit_visual_review(payload.clone()),
            "human_visual_review_submit" => self.submit_human_visual_review(payload.clone()),
            "appearance_prepare" => {
                let project_id = payload
                    .get("project_id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        RuntimeError::InvalidInput("project_id is required".to_owned())
                    })?;
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
    capabilities.operator_catalog_sha256 = Some(operator_catalog_sha256());
    capabilities
        .resource_uris
        .push("forgecad://references/{reference_id}".to_owned());
    capabilities
        .resource_uris
        .push("forgecad://skills/{skill_id}/{version}".to_owned());
    capabilities
        .resource_uris
        .push("forgecad://operators/catalog".to_owned());
    capabilities.limitations = capabilities
        .limitations
        .into_iter()
        .filter(|limitation| {
            !limitation.starts_with("Reference images, geometry and render workers")
        })
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

fn read_authorized_attachment(path: &str, roots: &[PathBuf]) -> Result<Vec<u8>, RuntimeError> {
    if path.is_empty() || path.len() > 4096 || roots.is_empty() {
        return Err(RuntimeError::InvalidInput(
            "REFERENCE_TRANSFER_UNAVAILABLE: no authorized attachment root is configured"
                .to_owned(),
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

fn required_value_id<'a>(value: Option<&'a Value>, key: &str) -> Result<&'a str, RuntimeError> {
    let value = value.and_then(Value::as_str).ok_or_else(|| {
        RuntimeError::InvalidInput(format!("{key} is required and must be an opaque id"))
    })?;
    if !is_opaque_id(value) {
        return Err(RuntimeError::InvalidInput(format!(
            "{key} is required and must be an opaque id"
        )));
    }
    Ok(value)
}

fn required_value_sha<'a>(value: Option<&'a Value>, key: &str) -> Result<&'a str, RuntimeError> {
    let value = value.and_then(Value::as_str).ok_or_else(|| {
        RuntimeError::InvalidInput(format!("{key} is required and must be a SHA-256"))
    })?;
    if !forgecad_contracts::is_sha256(value) {
        return Err(RuntimeError::InvalidInput(format!(
            "{key} is required and must be a SHA-256"
        )));
    }
    Ok(value)
}

fn default_camera_calibration() -> Value {
    let mut camera = json!({
        "schema_version":"CameraCalibration@1",
        "camera_hash":"",
        "projection":"perspective",
        "transform":{"position_m":[4.0,3.0,6.0],"target_m":[0.0,1.5,0.0],"up":[0.0,1.0,0.0]},
        "fov_y_degrees":42.0,
        "near_m":0.05,
        "far_m":20.0,
        "resolution":{"width":512,"height":512},
        "coordinate_system":"right-handed-y-up-meter",
        "renderer_revision":"forgecad-renderer-2",
        "canonical_sha256":""
    });
    let hash = canonical_json_hash(&camera);
    camera["camera_hash"] = Value::String(hash.clone());
    camera["canonical_sha256"] = Value::String(canonical_json_hash(&camera));
    // The camera_hash is the hash of the complete calibration with its
    // canonical field blank; the canonical receipt binds the final object.
    camera
}

/// Fit the product-owned default camera to the visible reference silhouette.
///
/// This is deliberately a framing-only calibration: it changes the camera
/// distance along the existing view ray, never the model, camera direction or
/// hidden geometry. A caller-supplied CameraCalibration remains authoritative
/// and bypasses this helper. The bounded mask heuristic keeps the comparison
/// useful for a close-cropped single image without introducing a segmentation
/// model or a second source of geometry truth.
fn calibrate_default_camera(camera: &Value, reference: &[bool], model: &[bool]) -> Value {
    let Some(reference_bbox) = bbox(reference) else {
        return camera.clone();
    };
    let Some(model_bbox) = bbox(model) else {
        return camera.clone();
    };
    let reference_height = (reference_bbox.3 - reference_bbox.1 + 1) as f64;
    let model_height = (model_bbox.3 - model_bbox.1 + 1) as f64;
    if reference_height <= 0.0 || model_height <= 0.0 {
        return camera.clone();
    }
    let scale = (model_height / reference_height).clamp(0.55, 1.45);
    let Some(transform) = camera.get("transform").and_then(Value::as_object) else {
        return camera.clone();
    };
    let Some(position) = camera_vec3(transform.get("position_m")) else {
        return camera.clone();
    };
    let Some(target) = camera_vec3(transform.get("target_m")) else {
        return camera.clone();
    };
    let adjusted_position = [
        target[0] + (position[0] - target[0]) * scale,
        target[1] + (position[1] - target[1]) * scale,
        target[2] + (position[2] - target[2]) * scale,
    ];
    let mut calibrated = camera.clone();
    let Some(calibrated_transform) = calibrated
        .get_mut("transform")
        .and_then(Value::as_object_mut)
    else {
        return camera.clone();
    };
    calibrated_transform.insert("position_m".to_owned(), json!(adjusted_position));
    calibrated["camera_hash"] = Value::String(String::new());
    calibrated["canonical_sha256"] = Value::String(String::new());
    calibrated["camera_hash"] = Value::String(canonical_json_hash(&calibrated));
    calibrated["canonical_sha256"] = Value::String(canonical_json_hash(&calibrated));
    calibrated
}

fn camera_vec3(value: Option<&Value>) -> Option<[f64; 3]> {
    let values = value?.as_array()?;
    if values.len() != 3 {
        return None;
    }
    Some([
        values[0].as_f64()?,
        values[1].as_f64()?,
        values[2].as_f64()?,
    ])
}

fn validate_reference_view_spec(
    value: &Value,
    reference: &ReferenceEvidenceRecord,
) -> Result<(), RuntimeError> {
    let object = exact_object(
        value,
        &[
            "schema_version",
            "reference_id",
            "reference_sha256",
            "view_id",
            "source_view",
            "image",
            "landmarks",
            "regions",
            "canonical_sha256",
        ],
        "ReferenceViewSpec@1",
    )?;
    if object.get("schema_version").and_then(Value::as_str) != Some("ReferenceViewSpec@1")
        || object.get("reference_id").and_then(Value::as_str)
            != Some(reference.reference_id.as_str())
        || object.get("reference_sha256").and_then(Value::as_str)
            != Some(reference.object_sha256.as_str())
    {
        return Err(RuntimeError::InvalidInput(
            "REFERENCE_VIEW_BINDING_MISMATCH".to_owned(),
        ));
    }
    required_contract_identifier(object, "view_id", "ReferenceViewSpec@1")?;
    required_contract_sha256(object, "reference_sha256", "ReferenceViewSpec@1")?;
    required_contract_sha256(object, "canonical_sha256", "ReferenceViewSpec@1")?;
    let image = object
        .get("image")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            RuntimeError::InvalidInput("ReferenceViewSpec@1.image is invalid".to_owned())
        })?;
    let image_value = Value::Object(image.clone());
    let image = exact_object(
        &image_value,
        &["width", "height", "rotation_degrees", "crop"],
        "ReferenceViewSpec@1.image",
    )?;
    let width = image.get("width").and_then(Value::as_u64).unwrap_or(0);
    let height = image.get("height").and_then(Value::as_u64).unwrap_or(0);
    if width != reference.width as u64 || height != reference.height as u64 {
        return Err(RuntimeError::InvalidInput(
            "REFERENCE_VIEW_BINDING_MISMATCH: image dimensions differ from ReferenceEvidence"
                .to_owned(),
        ));
    }
    let rotation = image
        .get("rotation_degrees")
        .and_then(Value::as_f64)
        .ok_or_else(|| {
            RuntimeError::InvalidInput(
                "ReferenceViewSpec@1.image.rotation_degrees is invalid".to_owned(),
            )
        })?;
    if !rotation.is_finite() || !(-180.0..=180.0).contains(&rotation) {
        return Err(RuntimeError::InvalidInput(
            "ReferenceViewSpec@1.image.rotation_degrees is out of range".to_owned(),
        ));
    }
    let crop = exact_object(
        image.get("crop").ok_or_else(|| {
            RuntimeError::InvalidInput("ReferenceViewSpec@1.image.crop is missing".to_owned())
        })?,
        &["x", "y", "width", "height"],
        "ReferenceViewSpec@1.image.crop",
    )?;
    for key in ["x", "y", "width", "height"] {
        let value = crop.get(key).and_then(Value::as_f64).ok_or_else(|| {
            RuntimeError::InvalidInput(format!("ReferenceViewSpec@1.image.crop.{key} is invalid"))
        })?;
        if !value.is_finite()
            || !(0.0..=1.0).contains(&value)
            || ((key == "width" || key == "height") && value <= 0.0)
        {
            return Err(RuntimeError::InvalidInput(format!(
                "ReferenceViewSpec@1.image.crop.{key} is out of range"
            )));
        }
    }
    let landmarks = object
        .get("landmarks")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            RuntimeError::InvalidInput("ReferenceViewSpec@1.landmarks is invalid".to_owned())
        })?;
    if landmarks.len() > 128 {
        return Err(RuntimeError::InvalidInput(
            "ReferenceViewSpec@1.landmarks exceeds 128 items".to_owned(),
        ));
    }
    for landmark in landmarks {
        let landmark = exact_object(
            landmark,
            &["landmark_id", "x", "y", "visibility", "confidence"],
            "ReferenceViewSpec@1.landmark",
        )?;
        required_contract_identifier(landmark, "landmark_id", "ReferenceViewSpec@1.landmark")?;
        validate_normalized_coordinate(landmark, "x", "ReferenceViewSpec@1.landmark")?;
        validate_normalized_coordinate(landmark, "y", "ReferenceViewSpec@1.landmark")?;
        validate_visibility(landmark, "visibility", "ReferenceViewSpec@1.landmark")?;
        validate_unit_number(landmark, "confidence", "ReferenceViewSpec@1.landmark")?;
    }
    let regions = object
        .get("regions")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            RuntimeError::InvalidInput("ReferenceViewSpec@1.regions is invalid".to_owned())
        })?;
    if regions.len() > 64 {
        return Err(RuntimeError::InvalidInput(
            "ReferenceViewSpec@1.regions exceeds 64 items".to_owned(),
        ));
    }
    for region in regions {
        let region = exact_object(
            region,
            &[
                "region_id",
                "x",
                "y",
                "width",
                "height",
                "visibility",
                "confidence",
            ],
            "ReferenceViewSpec@1.region",
        )?;
        required_contract_identifier(region, "region_id", "ReferenceViewSpec@1.region")?;
        for key in ["x", "y", "width", "height", "confidence"] {
            validate_unit_number(region, key, "ReferenceViewSpec@1.region")?;
        }
        if region.get("width").and_then(Value::as_f64).unwrap_or(0.0) <= 0.0
            || region.get("height").and_then(Value::as_f64).unwrap_or(0.0) <= 0.0
        {
            return Err(RuntimeError::InvalidInput(
                "ReferenceViewSpec@1.region dimensions must be positive".to_owned(),
            ));
        }
        validate_visibility(region, "visibility", "ReferenceViewSpec@1.region")?;
    }
    verify_output_canonical_hash(value, "ReferenceViewSpec@1")
}

fn validate_unit_number(
    object: &serde_json::Map<String, Value>,
    key: &str,
    context: &str,
) -> Result<f64, RuntimeError> {
    let value = object.get(key).and_then(Value::as_f64).ok_or_else(|| {
        RuntimeError::InvalidInput(format!(
            "CONTRACT_OUTPUT_INVALID: {context}.{key} is invalid"
        ))
    })?;
    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
        return Err(RuntimeError::InvalidInput(format!(
            "CONTRACT_OUTPUT_INVALID: {context}.{key} is out of range"
        )));
    }
    Ok(value)
}

fn validate_normalized_coordinate(
    object: &serde_json::Map<String, Value>,
    key: &str,
    context: &str,
) -> Result<(), RuntimeError> {
    validate_unit_number(object, key, context).map(|_| ())
}

fn validate_visibility(
    object: &serde_json::Map<String, Value>,
    key: &str,
    context: &str,
) -> Result<(), RuntimeError> {
    if !matches!(
        object.get(key).and_then(Value::as_str),
        Some("observed" | "inferred" | "unknown")
    ) {
        return Err(RuntimeError::InvalidInput(format!(
            "CONTRACT_OUTPUT_INVALID: {context}.{key} is invalid"
        )));
    }
    Ok(())
}

fn validate_vec3(
    object: &serde_json::Map<String, Value>,
    key: &str,
    context: &str,
) -> Result<(), RuntimeError> {
    let values = object.get(key).and_then(Value::as_array).ok_or_else(|| {
        RuntimeError::InvalidInput(format!(
            "CONTRACT_OUTPUT_INVALID: {context}.{key} is invalid"
        ))
    })?;
    if values.len() != 3
        || values
            .iter()
            .any(|value| value.as_f64().is_none_or(|value| !value.is_finite()))
    {
        return Err(RuntimeError::InvalidInput(format!(
            "CONTRACT_OUTPUT_INVALID: {context}.{key} must be a finite vec3"
        )));
    }
    Ok(())
}

fn validate_camera_calibration(value: &Value) -> Result<(), RuntimeError> {
    let object = exact_object(
        value,
        &[
            "schema_version",
            "camera_hash",
            "projection",
            "transform",
            "fov_y_degrees",
            "near_m",
            "far_m",
            "resolution",
            "coordinate_system",
            "renderer_revision",
            "canonical_sha256",
        ],
        "CameraCalibration@1",
    )?;
    if object.get("schema_version").and_then(Value::as_str) != Some("CameraCalibration@1")
        || object.get("projection").and_then(Value::as_str) != Some("perspective")
        || object.get("coordinate_system").and_then(Value::as_str)
            != Some("right-handed-y-up-meter")
        || object
            .get("resolution")
            .and_then(|value| value.get("width"))
            .and_then(Value::as_u64)
            != Some(512)
        || object
            .get("resolution")
            .and_then(|value| value.get("height"))
            .and_then(Value::as_u64)
            != Some(512)
        || object.get("far_m").and_then(Value::as_f64).unwrap_or(0.0)
            <= object.get("near_m").and_then(Value::as_f64).unwrap_or(0.0)
    {
        return Err(RuntimeError::InvalidInput(
            "CAMERA_CALIBRATION_INVALID: fixed perspective requirements are not met".to_owned(),
        ));
    }
    let transform = exact_object(
        object.get("transform").ok_or_else(|| {
            RuntimeError::InvalidInput(
                "CAMERA_CALIBRATION_INVALID: transform is missing".to_owned(),
            )
        })?,
        &["position_m", "target_m", "up"],
        "CameraCalibration@1.transform",
    )?;
    for key in ["position_m", "target_m", "up"] {
        validate_vec3(transform, key, "CameraCalibration@1.transform")?;
    }
    let resolution = exact_object(
        object.get("resolution").ok_or_else(|| {
            RuntimeError::InvalidInput(
                "CAMERA_CALIBRATION_INVALID: resolution is missing".to_owned(),
            )
        })?,
        &["width", "height"],
        "CameraCalibration@1.resolution",
    )?;
    if resolution.get("width").and_then(Value::as_u64) != Some(512)
        || resolution.get("height").and_then(Value::as_u64) != Some(512)
    {
        return Err(RuntimeError::InvalidInput(
            "CAMERA_CALIBRATION_INVALID: resolution must be 512x512".to_owned(),
        ));
    }
    for key in ["fov_y_degrees", "near_m", "far_m"] {
        let number = object.get(key).and_then(Value::as_f64).ok_or_else(|| {
            RuntimeError::InvalidInput(format!("CAMERA_CALIBRATION_INVALID: {key} is missing"))
        })?;
        if !number.is_finite() || number <= 0.0 {
            return Err(RuntimeError::InvalidInput(format!(
                "CAMERA_CALIBRATION_INVALID: {key} is invalid"
            )));
        }
    }
    let fov = object["fov_y_degrees"].as_f64().unwrap_or(0.0);
    if !(1.0..179.0).contains(&fov) {
        return Err(RuntimeError::InvalidInput(
            "CAMERA_CALIBRATION_INVALID: fov is out of range".to_owned(),
        ));
    }
    required_contract_sha256(object, "camera_hash", "CameraCalibration@1")?;
    required_contract_sha256(object, "canonical_sha256", "CameraCalibration@1")?;
    required_contract_identifier(object, "renderer_revision", "CameraCalibration@1")?;
    verify_output_canonical_hash(value, "CameraCalibration@1")
}

fn validate_render_set_v2_output(value: &Value) -> Result<(), RuntimeError> {
    let object = exact_object(
        value,
        &[
            "schema_version",
            "render_set_id",
            "candidate_id",
            "artifact_sha256",
            "program_sha256",
            "reference_id",
            "camera_hash",
            "renderer_hash",
            "width",
            "height",
            "passes",
            "pass_artifacts",
            "canonical_sha256",
        ],
        "RenderSet@2",
    )?;
    if object.get("schema_version").and_then(Value::as_str) != Some("RenderSet@2")
        || object.get("width").and_then(Value::as_u64) != Some(512)
        || object.get("height").and_then(Value::as_u64) != Some(512)
    {
        return Err(RuntimeError::InvalidInput(
            "CONTRACT_OUTPUT_INVALID: RenderSet@2 constants drifted".to_owned(),
        ));
    }
    for key in ["render_set_id", "candidate_id", "reference_id"] {
        required_contract_identifier(object, key, "RenderSet@2")?;
    }
    for key in [
        "artifact_sha256",
        "program_sha256",
        "camera_hash",
        "renderer_hash",
    ] {
        required_contract_sha256(object, key, "RenderSet@2")?;
    }
    let expected = [
        "beauty",
        "silhouette",
        "depth",
        "normal",
        "ao",
        "part-id",
        "material-id",
        "wireframe",
        "uv-stretch",
    ];
    let passes = object
        .get("passes")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            RuntimeError::InvalidInput(
                "CONTRACT_OUTPUT_INVALID: RenderSet@2.passes is invalid".to_owned(),
            )
        })?;
    if passes.len() != expected.len()
        || passes.iter().map(Value::as_str).collect::<Option<Vec<_>>>() != Some(expected.to_vec())
    {
        return Err(RuntimeError::InvalidInput(
            "CONTRACT_OUTPUT_INVALID: RenderSet@2 pass order is not fixed".to_owned(),
        ));
    }
    let artifacts = object
        .get("pass_artifacts")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            RuntimeError::InvalidInput(
                "CONTRACT_OUTPUT_INVALID: RenderSet@2.pass_artifacts is invalid".to_owned(),
            )
        })?;
    if artifacts.len() != expected.len()
        || expected.iter().any(|pass| !artifacts.contains_key(*pass))
    {
        return Err(RuntimeError::InvalidInput(
            "CONTRACT_OUTPUT_INVALID: RenderSet@2 pass artifacts are incomplete".to_owned(),
        ));
    }
    for pass in expected {
        let artifact = exact_object(
            artifacts.get(pass).ok_or_else(|| {
                RuntimeError::InvalidInput(
                    "CONTRACT_OUTPUT_INVALID: RenderSet@2 pass artifact is missing".to_owned(),
                )
            })?,
            &[
                "sha256",
                "mime",
                "size_bytes",
                "width",
                "height",
                "channels",
                "color_space",
            ],
            "RenderSet@2.pass_artifact",
        )?;
        required_contract_sha256(artifact, "sha256", "RenderSet@2.pass_artifact")?;
        if artifact.get("mime").and_then(Value::as_str) != Some("image/png")
            || artifact.get("width").and_then(Value::as_u64) != Some(512)
            || artifact.get("height").and_then(Value::as_u64) != Some(512)
            || artifact
                .get("size_bytes")
                .and_then(Value::as_u64)
                .unwrap_or(0)
                == 0
            || !matches!(
                artifact.get("channels").and_then(Value::as_str),
                Some("rgba8")
            )
            || !matches!(
                artifact.get("color_space").and_then(Value::as_str),
                Some("srgb" | "linear" | "data")
            )
        {
            return Err(RuntimeError::InvalidInput(
                "CONTRACT_OUTPUT_INVALID: RenderSet@2 pass PNG metadata is invalid".to_owned(),
            ));
        }
    }
    verify_output_canonical_hash(value, "RenderSet@2")
}

fn validate_reference_comparison_report(value: &Value) -> Result<(), RuntimeError> {
    let object = exact_object(
        value,
        &[
            "schema_version",
            "report_id",
            "candidate_id",
            "artifact_sha256",
            "reference_id",
            "reference_sha256",
            "render_set_hash",
            "camera_hash",
            "mask",
            "metrics",
            "status",
            "canonical_sha256",
        ],
        "ReferenceComparisonReport@1",
    )?;
    if object.get("schema_version").and_then(Value::as_str) != Some("ReferenceComparisonReport@1")
        || !matches!(
            object.get("status").and_then(Value::as_str),
            Some(
                "PARTIAL_VISIBLE_VIEW_PASS"
                    | "QUALITY_TARGET_NOT_MET"
                    | "BLOCKED_REFERENCE_COVERAGE"
            )
        )
    {
        return Err(RuntimeError::InvalidInput(
            "CONTRACT_OUTPUT_INVALID: ReferenceComparisonReport@1 constants drifted".to_owned(),
        ));
    }
    for key in ["report_id", "candidate_id", "reference_id"] {
        required_contract_identifier(object, key, "ReferenceComparisonReport@1")?;
    }
    for key in [
        "artifact_sha256",
        "reference_sha256",
        "render_set_hash",
        "camera_hash",
    ] {
        required_contract_sha256(object, key, "ReferenceComparisonReport@1")?;
    }
    let mask = exact_object(
        object.get("mask").ok_or_else(|| {
            RuntimeError::InvalidInput(
                "CONTRACT_OUTPUT_INVALID: comparison mask missing".to_owned(),
            )
        })?,
        &["method", "revision", "sha256", "width", "height"],
        "ReferenceComparisonReport@1.mask",
    )?;
    if mask.get("method").and_then(Value::as_str) != Some("local-border-flood-fill-morphology")
        || mask.get("width").and_then(Value::as_u64) != Some(512)
        || mask.get("height").and_then(Value::as_u64) != Some(512)
    {
        return Err(RuntimeError::InvalidInput(
            "CONTRACT_OUTPUT_INVALID: comparison mask metadata is invalid".to_owned(),
        ));
    }
    required_contract_identifier(mask, "revision", "ReferenceComparisonReport@1.mask")?;
    required_contract_sha256(mask, "sha256", "ReferenceComparisonReport@1.mask")?;
    let metrics = exact_object(
        object.get("metrics").ok_or_else(|| {
            RuntimeError::InvalidInput(
                "CONTRACT_OUTPUT_INVALID: comparison metrics missing".to_owned(),
            )
        })?,
        &[
            "silhouette_iou",
            "boundary_f1_4px",
            "bbox_edge_error",
            "centroid_error",
            "landmark_coverage",
            "landmark_nme",
            "region_median_iou",
            "critical_region_min_iou",
        ],
        "ReferenceComparisonReport@1.metrics",
    )?;
    for key in [
        "silhouette_iou",
        "boundary_f1_4px",
        "bbox_edge_error",
        "centroid_error",
        "landmark_coverage",
        "landmark_nme",
        "region_median_iou",
        "critical_region_min_iou",
    ] {
        let metric = metrics.get(key).and_then(Value::as_f64).ok_or_else(|| {
            RuntimeError::InvalidInput(format!(
                "CONTRACT_OUTPUT_INVALID: comparison metric {key} missing"
            ))
        })?;
        if !metric.is_finite() || !(0.0..=1.0).contains(&metric) {
            return Err(RuntimeError::InvalidInput(format!(
                "CONTRACT_OUTPUT_INVALID: comparison metric {key} out of range"
            )));
        }
    }
    verify_output_canonical_hash(value, "ReferenceComparisonReport@1")
}

fn validate_visual_review_report(value: &Value) -> Result<(), RuntimeError> {
    let object = exact_object(
        value,
        &[
            "schema_version",
            "review_id",
            "candidate_id",
            "reference_id",
            "render_set_hash",
            "comparison_report_hash",
            "round",
            "stage",
            "issues",
            "status",
            "canonical_sha256",
        ],
        "VisualReviewReport@1",
    )?;
    if object.get("schema_version").and_then(Value::as_str) != Some("VisualReviewReport@1")
        || object
            .get("round")
            .and_then(Value::as_u64)
            .is_none_or(|round| !(1..=5).contains(&round))
        || !matches!(
            object.get("stage").and_then(Value::as_str),
            Some("silhouette" | "structure" | "form" | "material-surface" | "final")
        )
    {
        return Err(RuntimeError::InvalidInput(
            "CONTRACT_OUTPUT_INVALID: VisualReviewReport@1 stage/round is invalid".to_owned(),
        ));
    }
    for key in ["review_id", "candidate_id", "reference_id"] {
        required_contract_identifier(object, key, "VisualReviewReport@1")?;
    }
    for key in ["render_set_hash", "comparison_report_hash"] {
        required_contract_sha256(object, key, "VisualReviewReport@1")?;
    }
    if !matches!(
        object.get("status").and_then(Value::as_str),
        Some("submitted" | "needs_revision" | "accepted")
    ) {
        return Err(RuntimeError::InvalidInput(
            "CONTRACT_OUTPUT_INVALID: VisualReviewReport@1 status is invalid".to_owned(),
        ));
    }
    let issues = object
        .get("issues")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            RuntimeError::InvalidInput(
                "CONTRACT_OUTPUT_INVALID: VisualReviewReport@1.issues is invalid".to_owned(),
            )
        })?;
    if issues.len() > 128 {
        return Err(RuntimeError::InvalidInput(
            "CONTRACT_OUTPUT_INVALID: VisualReviewReport@1.issues exceeds 128 items".to_owned(),
        ));
    }
    for issue in issues {
        let issue = exact_object(
            issue,
            &[
                "issue_id",
                "pass",
                "region_id",
                "claim",
                "confidence",
                "visibility",
                "action",
            ],
            "VisualReviewReport@1.issue",
        )?;
        required_contract_identifier(issue, "issue_id", "VisualReviewReport@1.issue")?;
        required_contract_identifier(issue, "region_id", "VisualReviewReport@1.issue")?;
        if !matches!(
            issue.get("pass").and_then(Value::as_str),
            Some(
                "beauty"
                    | "silhouette"
                    | "depth"
                    | "normal"
                    | "ao"
                    | "part-id"
                    | "material-id"
                    | "wireframe"
                    | "uv-stretch"
            )
        ) {
            return Err(RuntimeError::InvalidInput(
                "CONTRACT_OUTPUT_INVALID: VisualReviewReport@1.issue.pass is invalid".to_owned(),
            ));
        }
        validate_bounded_text(issue, "claim", 512, "VisualReviewReport@1.issue")?;
        validate_bounded_text(issue, "action", 512, "VisualReviewReport@1.issue")?;
        validate_unit_number(issue, "confidence", "VisualReviewReport@1.issue")?;
        validate_visibility(issue, "visibility", "VisualReviewReport@1.issue")?;
    }
    verify_output_canonical_hash(value, "VisualReviewReport@1")
}

fn validate_human_review_receipt(value: &Value) -> Result<(), RuntimeError> {
    let object = exact_object(
        value,
        &[
            "schema_version",
            "receipt_id",
            "candidate_id",
            "reference_id",
            "render_set_hash",
            "comparison_report_hash",
            "scores",
            "approved",
            "recorded_at",
            "canonical_sha256",
        ],
        "HumanVisualReviewReceipt@1",
    )?;
    if object.get("schema_version").and_then(Value::as_str) != Some("HumanVisualReviewReceipt@1") {
        return Err(RuntimeError::InvalidInput(
            "CONTRACT_OUTPUT_INVALID: HumanVisualReviewReceipt@1 version drifted".to_owned(),
        ));
    }
    for key in ["receipt_id", "candidate_id", "reference_id"] {
        required_contract_identifier(object, key, "HumanVisualReviewReceipt@1")?;
    }
    for key in ["render_set_hash", "comparison_report_hash"] {
        required_contract_sha256(object, key, "HumanVisualReviewReceipt@1")?;
    }
    let scores = exact_object(
        object.get("scores").ok_or_else(|| {
            RuntimeError::InvalidInput(
                "CONTRACT_OUTPUT_INVALID: human scores are missing".to_owned(),
            )
        })?,
        &[
            "likeness",
            "geometry_detail",
            "material_fidelity",
            "editability",
        ],
        "HumanVisualReviewReceipt@1.scores",
    )?;
    for key in [
        "likeness",
        "geometry_detail",
        "material_fidelity",
        "editability",
    ] {
        let score = scores.get(key).and_then(Value::as_u64).ok_or_else(|| {
            RuntimeError::InvalidInput(format!(
                "CONTRACT_OUTPUT_INVALID: human score {key} is missing"
            ))
        })?;
        if !(1..=5).contains(&score) {
            return Err(RuntimeError::InvalidInput(format!(
                "CONTRACT_OUTPUT_INVALID: human score {key} is out of range"
            )));
        }
    }
    if object.get("approved").and_then(Value::as_bool).is_none() {
        return Err(RuntimeError::InvalidInput(
            "CONTRACT_OUTPUT_INVALID: human approved is invalid".to_owned(),
        ));
    }
    let recorded_at = object
        .get("recorded_at")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            RuntimeError::InvalidInput(
                "CONTRACT_OUTPUT_INVALID: human recorded_at is invalid".to_owned(),
            )
        })?;
    if recorded_at.is_empty() || recorded_at.len() > 64 {
        return Err(RuntimeError::InvalidInput(
            "CONTRACT_OUTPUT_INVALID: human recorded_at is out of range".to_owned(),
        ));
    }
    verify_output_canonical_hash(value, "HumanVisualReviewReceipt@1")
}

fn validate_bounded_text(
    object: &serde_json::Map<String, Value>,
    key: &str,
    max: usize,
    context: &str,
) -> Result<(), RuntimeError> {
    let value = object.get(key).and_then(Value::as_str).ok_or_else(|| {
        RuntimeError::InvalidInput(format!(
            "CONTRACT_OUTPUT_INVALID: {context}.{key} is invalid"
        ))
    })?;
    if value.is_empty() || value.len() > max {
        return Err(RuntimeError::InvalidInput(format!(
            "CONTRACT_OUTPUT_INVALID: {context}.{key} is out of range"
        )));
    }
    Ok(())
}

fn validate_quality_report_v2_output(value: &Value) -> Result<(), RuntimeError> {
    let object = exact_object(
        value,
        &[
            "schema_version",
            "quality_report_id",
            "candidate_id",
            "artifact_sha256",
            "program_sha256",
            "reference_id",
            "reference_sha256",
            "render_set_hash",
            "comparison_report_hash",
            "human_receipt_hash",
            "structural_status",
            "visual_status",
            "hard_gate_passed",
            "limitations",
            "canonical_sha256",
        ],
        "QualityReport@2",
    )?;
    if object.get("schema_version").and_then(Value::as_str) != Some("QualityReport@2")
        || !matches!(
            object.get("structural_status").and_then(Value::as_str),
            Some("passed" | "failed" | "not-run")
        )
        || !matches!(
            object.get("visual_status").and_then(Value::as_str),
            Some(
                "PARTIAL_VISIBLE_VIEW_PASS"
                    | "QUALITY_TARGET_NOT_MET"
                    | "BLOCKED_REFERENCE_COVERAGE"
                    | "not-run"
            )
        )
    {
        return Err(RuntimeError::InvalidInput(
            "CONTRACT_OUTPUT_INVALID: QualityReport@2 status is invalid".to_owned(),
        ));
    }
    for key in ["quality_report_id", "candidate_id"] {
        required_contract_identifier(object, key, "QualityReport@2")?;
    }
    for key in [
        "artifact_sha256",
        "program_sha256",
        "render_set_hash",
        "comparison_report_hash",
    ] {
        required_contract_sha256(object, key, "QualityReport@2")?;
    }
    for key in ["reference_id", "reference_sha256", "human_receipt_hash"] {
        if !object.get(key).is_some_and(|value| {
            value.is_null() || value.as_str().is_some_and(forgecad_contracts::is_sha256)
        }) && key != "reference_id"
        {
            return Err(RuntimeError::InvalidInput(format!(
                "CONTRACT_OUTPUT_INVALID: QualityReport@2.{key} is invalid"
            )));
        }
    }
    if object
        .get("reference_id")
        .and_then(Value::as_str)
        .is_some_and(|_| object.get("reference_sha256").is_some_and(Value::is_null))
    {
        return Err(RuntimeError::InvalidInput(
            "CONTRACT_OUTPUT_INVALID: QualityReport@2 reference ID/hash pairing is invalid"
                .to_owned(),
        ));
    }
    verify_output_canonical_hash(value, "QualityReport@2")
}

struct ReferenceMask {
    mask: Vec<bool>,
    png: Vec<u8>,
}

fn reference_mask_png(bytes: &[u8]) -> Result<ReferenceMask, RuntimeError> {
    let decoded = image::load_from_memory(bytes)
        .map_err(|error| RuntimeError::InvalidInput(format!("REFERENCE_MASK_FAILED: {error}")))?
        .resize_exact(512, 512, imageops::FilterType::Triangle)
        .to_rgba8();
    let mut background = vec![false; 512 * 512];
    let mut queue = VecDeque::new();
    for x in 0..512usize {
        queue.push_back((x, 0));
        queue.push_back((x, 511));
    }
    for y in 0..512usize {
        queue.push_back((0, y));
        queue.push_back((511, y));
    }
    // Studio references often have a dark-to-gray lighting gradient.  A
    // fixed distance from the top-left pixel incorrectly turns the far wall
    // or floor into foreground.  Seed every border pixel, then walk only
    // across small local colour changes so a smooth background is traversed
    // while the high-contrast subject boundary remains a stop edge.  This is
    // still a deterministic, product-owned mask heuristic; it is not a
    // remote segmentation model and does not claim semantic segmentation.
    const LOCAL_BACKGROUND_EDGE_THRESHOLD: i32 = 18;
    while let Some((x, y)) = queue.pop_front() {
        let index = y * 512 + x;
        if background[index] {
            continue;
        }
        background[index] = true;
        let current = decoded.get_pixel(x as u32, y as u32).0;
        for (nx, ny) in [
            (x.wrapping_sub(1), y),
            (x + 1, y),
            (x, y.wrapping_sub(1)),
            (x, y + 1),
        ] {
            if nx < 512 && ny < 512 && !background[ny * 512 + nx] {
                let next = decoded.get_pixel(nx as u32, ny as u32).0;
                let distance = (0..3)
                    .map(|channel| (current[channel] as i32 - next[channel] as i32).abs())
                    .sum::<i32>();
                if distance <= LOCAL_BACKGROUND_EDGE_THRESHOLD {
                    queue.push_back((nx, ny));
                }
            }
        }
    }
    let mut mask = background
        .into_iter()
        .map(|value| !value)
        .collect::<Vec<_>>();
    if !mask.iter().any(|value| *value) {
        mask = decoded
            .pixels()
            .map(|pixel| {
                let [r, g, b, _] = pixel.0;
                (r as u16 + g as u16 + b as u16) > 48
            })
            .collect();
    }
    let mask = close_mask(&mask);
    let mut image = RgbaImage::from_pixel(512, 512, Rgba([0, 0, 0, 255]));
    for (index, value) in mask.iter().enumerate() {
        if *value {
            image.put_pixel(
                (index % 512) as u32,
                (index / 512) as u32,
                Rgba([255, 255, 255, 255]),
            );
        }
    }
    let mut png = Vec::new();
    image::DynamicImage::ImageRgba8(image)
        .write_to(&mut Cursor::new(&mut png), ImageFormat::Png)
        .map_err(|error| RuntimeError::InvalidInput(format!("REFERENCE_MASK_FAILED: {error}")))?;
    Ok(ReferenceMask { mask, png })
}

fn close_mask(mask: &[bool]) -> Vec<bool> {
    let mut dilated = vec![false; mask.len()];
    for y in 0..512usize {
        for x in 0..512usize {
            let mut value = false;
            for dy in -1i32..=1 {
                for dx in -1i32..=1 {
                    let nx = x as i32 + dx;
                    let ny = y as i32 + dy;
                    if nx >= 0
                        && ny >= 0
                        && nx < 512
                        && ny < 512
                        && mask[ny as usize * 512 + nx as usize]
                    {
                        value = true;
                    }
                }
            }
            dilated[y * 512 + x] = value;
        }
    }
    let mut eroded = vec![false; mask.len()];
    for y in 0..512usize {
        for x in 0..512usize {
            let mut value = true;
            for dy in -1i32..=1 {
                for dx in -1i32..=1 {
                    let nx = x as i32 + dx;
                    let ny = y as i32 + dy;
                    if nx < 0
                        || ny < 0
                        || nx >= 512
                        || ny >= 512
                        || !dilated[ny as usize * 512 + nx as usize]
                    {
                        value = false;
                    }
                }
            }
            eroded[y * 512 + x] = value;
        }
    }
    eroded
}

fn decode_binary_mask(bytes: &[u8]) -> Result<Vec<bool>, RuntimeError> {
    let image = image::load_from_memory(bytes)
        .map_err(|error| RuntimeError::InvalidInput(format!("RENDER_PASS_INVALID: {error}")))?
        .resize_exact(512, 512, imageops::FilterType::Nearest)
        .to_rgba8();
    Ok(image
        .pixels()
        .map(|pixel| {
            let [r, g, b, _] = pixel.0;
            (r as u16 + g as u16 + b as u16) > 96
        })
        .collect())
}

fn compare_masks(reference: &[bool], model: &[bool], view_spec: &Value) -> Value {
    let mut intersection = 0usize;
    let mut union = 0usize;
    for (left, right) in reference.iter().zip(model) {
        if *left && *right {
            intersection += 1;
        }
        if *left || *right {
            union += 1;
        }
    }
    let reference_bbox = bbox(reference);
    let model_bbox = bbox(model);
    let silhouette_iou = if union == 0 {
        0.0
    } else {
        intersection as f64 / union as f64
    };
    let bbox_edge_error = bbox_edge_error(reference_bbox, model_bbox);
    let centroid_error = centroid_error(reference, model);
    let boundary_f1 = boundary_f1(reference, model, 4);
    let (landmark_coverage, landmark_nme) = landmark_metrics(model, view_spec);
    let region_scores = region_metrics(model, view_spec);
    let region_median = if region_scores.is_empty() {
        0.0
    } else {
        let mut sorted = region_scores.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        sorted[sorted.len() / 2]
    };
    let critical = region_scores.iter().copied().fold(1.0, f64::min);
    json!({"silhouette_iou":silhouette_iou,"boundary_f1_4px":boundary_f1,"bbox_edge_error":bbox_edge_error,"centroid_error":centroid_error,"landmark_coverage":landmark_coverage,"landmark_nme":landmark_nme,"region_median_iou":region_median,"critical_region_min_iou":critical})
}

fn bbox(mask: &[bool]) -> Option<(usize, usize, usize, usize)> {
    let mut min_x = 512;
    let mut min_y = 512;
    let mut max_x = 0;
    let mut max_y = 0;
    let mut any = false;
    for y in 0..512 {
        for x in 0..512 {
            if mask[y * 512 + x] {
                any = true;
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x);
                max_y = max_y.max(y);
            }
        }
    }
    any.then_some((min_x, min_y, max_x, max_y))
}
fn bbox_edge_error(
    left: Option<(usize, usize, usize, usize)>,
    right: Option<(usize, usize, usize, usize)>,
) -> f64 {
    match (left, right) {
        (Some(a), Some(b)) => [
            a.0 as f64 / 512.0 - b.0 as f64 / 512.0,
            a.1 as f64 / 512.0 - b.1 as f64 / 512.0,
            a.2 as f64 / 512.0 - b.2 as f64 / 512.0,
            a.3 as f64 / 512.0 - b.3 as f64 / 512.0,
        ]
        .iter()
        .map(|v| v.abs())
        .fold(0.0, f64::max),
        _ => 1.0,
    }
}
fn centroid_error(reference: &[bool], model: &[bool]) -> f64 {
    fn center(mask: &[bool]) -> Option<(f64, f64)> {
        let mut n = 0.0;
        let mut x = 0.0;
        let mut y = 0.0;
        for j in 0..512 {
            for i in 0..512 {
                if mask[j * 512 + i] {
                    n += 1.0;
                    x += i as f64 / 512.0;
                    y += j as f64 / 512.0;
                }
            }
        }
        (n > 0.0).then_some((x / n, y / n))
    }
    match (center(reference), center(model)) {
        (Some(a), Some(b)) => ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt().min(1.0),
        _ => 1.0,
    }
}
fn boundary_mask(mask: &[bool]) -> Vec<bool> {
    let mut out = vec![false; mask.len()];
    for y in 0..512 {
        for x in 0..512 {
            if !mask[y * 512 + x] {
                continue;
            }
            for (ny, nx) in [
                (y.wrapping_sub(1), x),
                (y + 1, x),
                (y, x.wrapping_sub(1)),
                (y, x + 1),
            ] {
                if ny >= 512 || nx >= 512 || !mask[ny * 512 + nx] {
                    out[y * 512 + x] = true;
                    break;
                }
            }
        }
    }
    out
}
fn boundary_f1(reference: &[bool], model: &[bool], radius: i32) -> f64 {
    let a = boundary_mask(reference);
    let b = boundary_mask(model);
    fn score(left: &[bool], right: &[bool], radius: i32) -> f64 {
        let mut total = 0;
        let mut hit = 0;
        for y in 0..512 {
            for x in 0..512 {
                if !left[y * 512 + x] {
                    continue;
                }
                total += 1;
                let mut found = false;
                for dy in -radius..=radius {
                    for dx in -radius..=radius {
                        let nx = x as i32 + dx;
                        let ny = y as i32 + dy;
                        if nx >= 0
                            && ny >= 0
                            && nx < 512
                            && ny < 512
                            && right[ny as usize * 512 + nx as usize]
                        {
                            found = true;
                        }
                    }
                }
                if found {
                    hit += 1;
                }
            }
        }
        if total == 0 {
            0.0
        } else {
            hit as f64 / total as f64
        }
    }
    let p = score(&a, &b, radius);
    let r = score(&b, &a, radius);
    if p + r == 0.0 {
        0.0
    } else {
        2.0 * p * r / (p + r)
    }
}
fn landmark_metrics(model: &[bool], view_spec: &Value) -> (f64, f64) {
    let Some(values) = view_spec.get("landmarks").and_then(Value::as_array) else {
        return (0.0, 1.0);
    };
    let mut total = 0.0;
    let mut covered = 0.0;
    let mut error = 0.0;
    for value in values {
        if value.get("visibility").and_then(Value::as_str) == Some("unknown") {
            continue;
        }
        let x = value.get("x").and_then(Value::as_f64).unwrap_or(-1.0);
        let y = value.get("y").and_then(Value::as_f64).unwrap_or(-1.0);
        if !(0.0..=1.0).contains(&x) || !(0.0..=1.0).contains(&y) {
            continue;
        }
        total += 1.0;
        let px = (x * 511.0) as usize;
        let py = (y * 511.0) as usize;
        if model[py * 512 + px] {
            covered += 1.0;
            error += 0.0;
        } else {
            let mut best: f64 = 1.0;
            for dy in -12i32..=12 {
                for dx in -12i32..=12 {
                    let nx = px as i32 + dx;
                    let ny = py as i32 + dy;
                    if nx >= 0
                        && ny >= 0
                        && nx < 512
                        && ny < 512
                        && model[ny as usize * 512 + nx as usize]
                    {
                        best = best.min(((dx * dx + dy * dy) as f64).sqrt() / 512.0);
                    }
                }
            }
            error += best;
        }
    }
    if total == 0.0 {
        (0.0, 1.0)
    } else {
        (covered / total, (error / total).min(1.0))
    }
}
fn region_metrics(model: &[bool], view_spec: &Value) -> Vec<f64> {
    let Some(values) = view_spec.get("regions").and_then(Value::as_array) else {
        return Vec::new();
    };
    values
        .iter()
        .filter_map(|value| {
            let x = value.get("x").and_then(Value::as_f64)?;
            let y = value.get("y").and_then(Value::as_f64)?;
            let w = value.get("width").and_then(Value::as_f64)?;
            let h = value.get("height").and_then(Value::as_f64)?;
            let mut inter = 0usize;
            let mut region = 0usize;
            let mut model_total = 0usize;
            for py in 0..512 {
                for px in 0..512 {
                    let in_region = px as f64 / 512.0 >= x
                        && px as f64 / 512.0 <= x + w
                        && py as f64 / 512.0 >= y
                        && py as f64 / 512.0 <= y + h;
                    let in_model = model[py * 512 + px];
                    if in_region {
                        region += 1;
                    }
                    if in_model {
                        model_total += 1;
                    }
                    if in_region && in_model {
                        inter += 1;
                    }
                }
            }
            let union = region + model_total - inter;
            (union > 0).then_some(inter as f64 / union as f64)
        })
        .collect()
}

fn now_string() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before UNIX_EPOCH")
        .as_secs()
        .to_string()
}

fn compile_geometry_with_runtime_worker(
    geometry_program: &Value,
    appearance_program: Option<&Value>,
) -> Result<geometry_worker::GeometryArtifact, geometry_worker::GeometryWorkerError> {
    match geometry_worker::compile_geometry(geometry_program, appearance_program) {
        Ok(artifact) => Ok(artifact),
        #[cfg(any(test, feature = "test-geometry-worker-fallback"))]
        Err(geometry_worker::GeometryWorkerError::Unavailable) => {
            // This branch exists only in Runtime unit tests and in a consumer
            // test target that explicitly enables the internal test feature.
            // It is absent from product builds and dedicated isolation gates.
            geometry_worker::compile_geometry_test_fallback(geometry_program, appearance_program)
        }
        Err(error) => Err(error),
    }
}

fn hash_geometry_program_with_runtime_worker(
    draft: &Value,
) -> Result<Value, geometry_worker::GeometryWorkerError> {
    match geometry_worker::geometry_program_hash(draft) {
        Ok(result) => Ok(result),
        #[cfg(any(test, feature = "test-geometry-worker-fallback"))]
        Err(geometry_worker::GeometryWorkerError::Unavailable) => {
            let canonical_sha256 = forgecad_geometry_worker::geometry_program_v2_draft_hash(draft)
                .map_err(|_| geometry_worker::GeometryWorkerError::Rejected)?;
            Ok(json!({
                "schema_version":"GeometryProgramHashResult@1",
                "geometry_program_schema_version":"GeometryProgram@2",
                "canonical_sha256":canonical_sha256,
                "operator_catalog_sha256":operator_catalog_sha256(),
                "validation_status":"passed"
            }))
        }
        Err(error) => Err(error),
    }
}

fn render_fixed_with_runtime_worker(
    geometry_program: &Value,
    appearance_program: &Value,
) -> Result<Vec<geometry_worker::RenderPass>, geometry_worker::GeometryWorkerError> {
    match geometry_worker::render_fixed(geometry_program, appearance_program) {
        Ok(passes) => Ok(passes),
        #[cfg(any(test, feature = "test-geometry-worker-fallback"))]
        Err(geometry_worker::GeometryWorkerError::Unavailable) => {
            let artifact = geometry_worker::compile_geometry_test_fallback(
                geometry_program,
                Some(appearance_program),
            )?;
            forgecad_geometry_worker::render_fixed_glb(&artifact.glb)
                .map(|passes| {
                    passes
                        .into_iter()
                        .map(|pass| geometry_worker::RenderPass {
                            pass: pass.pass,
                            png: pass.png,
                            width: pass.width,
                            height: pass.height,
                        })
                        .collect()
                })
                .map_err(|_| geometry_worker::GeometryWorkerError::Rejected)
        }
        Err(error) => Err(error),
    }
}

fn render_glb_with_runtime_worker(
    glb: &[u8],
    camera: &Value,
) -> Result<Vec<geometry_worker::RenderPass>, geometry_worker::GeometryWorkerError> {
    match geometry_worker::render_glb(glb, camera) {
        Ok(passes) => Ok(passes),
        #[cfg(any(test, feature = "test-geometry-worker-fallback"))]
        Err(geometry_worker::GeometryWorkerError::Unavailable) => {
            forgecad_geometry_worker::render_perspective_glb(glb, camera)
                .map(|passes| {
                    passes
                        .into_iter()
                        .map(|pass| geometry_worker::RenderPass {
                            pass: pass.pass,
                            png: pass.png,
                            width: pass.width,
                            height: pass.height,
                        })
                        .collect()
                })
                .map_err(|_| geometry_worker::GeometryWorkerError::Rejected)
        }
        Err(error) => Err(error),
    }
}

fn strict_glb_inspection(bytes: &[u8]) -> Result<integrity::GlbIntegrity, RuntimeError> {
    integrity::inspect_glb(bytes)
        .map_err(|error| RuntimeError::InvalidInput(format!("STRICT_GLB_READBACK_FAILED: {error}")))
}

fn validate_worker_metadata(
    artifact: &geometry_worker::GeometryArtifact,
    inspection: &integrity::GlbIntegrity,
) -> Result<(), RuntimeError> {
    let mut worker_parts = artifact.part_ids.clone();
    let mut readback_parts = inspection.part_ids.clone();
    worker_parts.sort_unstable();
    readback_parts.sort_unstable();
    let mut worker_zones = artifact.material_zone_ids.clone();
    let mut readback_zones = inspection.material_zone_ids.clone();
    worker_zones.sort_unstable();
    readback_zones.sort_unstable();
    let expected_uv =
        if inspection.uv_non_finite_count == 0 && inspection.zero_area_uv_triangle_count == 0 {
            "passed"
        } else {
            "failed"
        };
    let expected_tangent = if inspection.tangent_non_finite_count == 0
        && inspection.tangent_orthogonality_error_count == 0
        && inspection.tangent_handedness_error_count == 0
    {
        "passed"
    } else {
        "failed"
    };
    if artifact.program_sha256 != inspection.program_sha256
        || artifact.triangle_count != inspection.triangle_count
        || worker_parts != readback_parts
        || worker_zones != readback_zones
        || artifact.uv_status != expected_uv
        || artifact.tangent_status != expected_tangent
    {
        return Err(RuntimeError::InvalidInput(
            "GEOMETRY_WORKER_PROTOCOL: Worker metadata does not match Runtime GLB readback"
                .to_owned(),
        ));
    }
    Ok(())
}

fn physical_geometry_passed(inspection: &integrity::GlbIntegrity) -> bool {
    inspection.triangle_count > 0
        && inspection.invalid_index_count == 0
        && inspection.non_finite_count == 0
        && inspection.degenerate_triangle_count == 0
        && inspection.boundary_edge_count == 0
        && inspection.non_manifold_edge_count == 0
        && inspection.winding_error_count == 0
        && inspection.uv_non_finite_count == 0
        && inspection.zero_area_uv_triangle_count == 0
        && inspection.tangent_non_finite_count == 0
        && inspection.tangent_orthogonality_error_count == 0
        && inspection.tangent_handedness_error_count == 0
        && inspection.external_uri_count == 0
        && inspection.metadata_mismatch_count == 0
        && (inspection.part_coverage - 1.0).abs() <= f64::EPSILON
        && (inspection.source_coverage - 1.0).abs() <= f64::EPSILON
        && (inspection.material_zone_coverage - 1.0).abs() <= f64::EPSILON
}

fn strict_integrity_value(inspection: &integrity::GlbIntegrity) -> Value {
    json!({
        "glb_parse_status":inspection.glb_parse_status,
        "invalid_index_count":inspection.invalid_index_count,
        "non_finite_count":inspection.non_finite_count,
        "degenerate_triangle_count":inspection.degenerate_triangle_count,
        "boundary_edge_count":inspection.boundary_edge_count,
        "non_manifold_edge_count":inspection.non_manifold_edge_count,
        "winding_error_count":inspection.winding_error_count,
        "uv_non_finite_count":inspection.uv_non_finite_count,
        "zero_area_uv_triangle_count":inspection.zero_area_uv_triangle_count,
        "tangent_non_finite_count":inspection.tangent_non_finite_count,
        "tangent_orthogonality_error_count":inspection.tangent_orthogonality_error_count,
        "tangent_handedness_error_count":inspection.tangent_handedness_error_count,
        "metadata_mismatch_count":inspection.metadata_mismatch_count,
        "external_uri_count":inspection.external_uri_count,
        "part_coverage":inspection.part_coverage,
        "source_coverage":inspection.source_coverage,
        "material_zone_coverage":inspection.material_zone_coverage,
    })
}

fn v2_readback_shape_is_serializable(inspection: &integrity::GlbIntegrity) -> bool {
    inspection.triangle_count <= 250_000
        && inspection.part_ids.len() <= 512
        && inspection.source_node_ids.len() <= 512
        && inspection.material_zone_ids.len() <= 512
        && inspection.part_bindings.len() <= 512
        && inspection
            .operator_catalog_sha256
            .as_deref()
            .is_some_and(forgecad_contracts::is_sha256)
        && inspection.part_ids.iter().all(|id| is_opaque_id(id))
        && inspection.source_node_ids.iter().all(|id| is_opaque_id(id))
        && inspection
            .material_zone_ids
            .iter()
            .all(|id| is_opaque_id(id))
        && inspection.part_bindings.iter().all(|binding| {
            is_opaque_id(&binding.part_id)
                && is_opaque_id(&binding.source_node_id)
                && is_opaque_id(&binding.material_zone_id)
                && binding.triangle_count <= 250_000
        })
}

fn artifact_readback_v1_value(
    artifact_id: &str,
    candidate_id: &str,
    inspection: &integrity::GlbIntegrity,
    size_bytes: u64,
) -> Value {
    let physical_passed = physical_geometry_passed(inspection);
    let mut value = json!({
        "schema_version":"ArtifactReadback@1",
        "artifact_id":artifact_id,
        "candidate_id":candidate_id,
        "object_sha256":artifact_id,
        "mime":"model/gltf-binary",
        "size_bytes":size_bytes,
        "part_ids":inspection.part_ids,
        "validator_status":if physical_passed {"passed"} else {"failed"},
        "canonical_sha256":"",
        "triangle_count":inspection.triangle_count,
        "program_sha256":inspection.program_sha256,
        "uv_status":if inspection.uv_non_finite_count == 0 && inspection.zero_area_uv_triangle_count == 0 {"passed"} else {"failed"},
        "tangent_status":if inspection.tangent_non_finite_count == 0 && inspection.tangent_orthogonality_error_count == 0 && inspection.tangent_handedness_error_count == 0 {"passed"} else {"failed"},
        "material_zone_ids":inspection.material_zone_ids
    });
    let hash = canonical_json_hash(&value);
    value["canonical_sha256"] = Value::String(hash);
    value
}

fn artifact_readback_v2_value(
    artifact_id: &str,
    candidate_id: &str,
    inspection: &integrity::GlbIntegrity,
    size_bytes: u64,
) -> Value {
    let mut value = json!({
        "schema_version":"ArtifactReadback@2",
        "artifact_id":artifact_id,
        "candidate_id":candidate_id,
        "object_sha256":artifact_id,
        "mime":"model/gltf-binary",
        "size_bytes":size_bytes,
        "program_sha256":inspection.program_sha256,
        "operator_catalog_sha256":inspection.operator_catalog_sha256,
        "readback_config_sha256":inspection.readback_config_sha256,
        "triangle_count":inspection.triangle_count,
        "part_ids":inspection.part_ids,
        "source_node_ids":inspection.source_node_ids,
        "material_zone_ids":inspection.material_zone_ids,
        "part_bindings":inspection.part_bindings.iter().map(|binding| json!({
            "part_id":binding.part_id,
            "source_node_id":binding.source_node_id,
            "material_zone_id":binding.material_zone_id,
            "solid":binding.solid,
            "triangle_count":binding.triangle_count,
        })).collect::<Vec<_>>(),
        "validator_status":inspection.validator_status,
        "hard_gate_passed":inspection.hard_gate_passed,
        "integrity":strict_integrity_value(inspection),
        "canonical_sha256":""
    });
    let hash = canonical_json_hash(&value);
    value["canonical_sha256"] = Value::String(hash);
    value
}

fn geometry_candidate_evidence_value(
    candidate: &CandidateRecord,
    reference_id: Option<&str>,
    reference_sha256: Option<&str>,
    inspection: &integrity::GlbIntegrity,
    geometry_program_object_sha256: &str,
    artifact_object_sha256: &str,
    artifact_readback_object_sha256: &str,
    quality_report_object_sha256: &str,
    quality_report_id: &str,
) -> Value {
    let mut value = json!({
        "schema_version":"GeometryCandidateEvidence@1",
        "candidate_id":candidate.candidate_id,
        "project_id":candidate.project_id,
        "reference_id":reference_id,
        "reference_sha256":reference_sha256,
        "geometry_program_sha256":inspection.program_sha256,
        "geometry_program_object_sha256":geometry_program_object_sha256,
        "operator_catalog_sha256":inspection.operator_catalog_sha256,
        "readback_config_sha256":inspection.readback_config_sha256,
        "artifact_object_sha256":artifact_object_sha256,
        "artifact_readback_object_sha256":artifact_readback_object_sha256,
        "quality_report_object_sha256":quality_report_object_sha256,
        "quality_report_id":quality_report_id,
        "canonical_sha256":"",
        "created_at":now_string()
    });
    value["canonical_sha256"] = Value::String(canonical_json_hash(&value));
    value
}

fn exact_object<'a>(
    value: &'a Value,
    required: &[&str],
    context: &str,
) -> Result<&'a serde_json::Map<String, Value>, RuntimeError> {
    let object = value.as_object().ok_or_else(|| {
        RuntimeError::InvalidInput(format!(
            "CONTRACT_OUTPUT_INVALID: {context} must be an object"
        ))
    })?;
    if object.len() != required.len()
        || required.iter().any(|key| !object.contains_key(*key))
        || object.keys().any(|key| !required.contains(&key.as_str()))
    {
        return Err(RuntimeError::InvalidInput(format!(
            "CONTRACT_OUTPUT_INVALID: {context} has an unexpected field set"
        )));
    }
    Ok(object)
}

fn required_contract_identifier(
    object: &serde_json::Map<String, Value>,
    key: &str,
    context: &str,
) -> Result<String, RuntimeError> {
    let value = object.get(key).and_then(Value::as_str).ok_or_else(|| {
        RuntimeError::InvalidInput(format!(
            "CONTRACT_OUTPUT_INVALID: {context}.{key} is missing"
        ))
    })?;
    if !is_opaque_id(value) {
        return Err(RuntimeError::InvalidInput(format!(
            "CONTRACT_OUTPUT_INVALID: {context}.{key} is not an identifier"
        )));
    }
    Ok(value.to_owned())
}

fn required_contract_sha256(
    object: &serde_json::Map<String, Value>,
    key: &str,
    context: &str,
) -> Result<String, RuntimeError> {
    let value = object.get(key).and_then(Value::as_str).ok_or_else(|| {
        RuntimeError::InvalidInput(format!(
            "CONTRACT_OUTPUT_INVALID: {context}.{key} is missing"
        ))
    })?;
    if !forgecad_contracts::is_sha256(value) {
        return Err(RuntimeError::InvalidInput(format!(
            "CONTRACT_OUTPUT_INVALID: {context}.{key} is not SHA-256"
        )));
    }
    Ok(value.to_owned())
}

fn verify_output_canonical_hash(value: &Value, context: &str) -> Result<(), RuntimeError> {
    let object = value.as_object().ok_or_else(|| {
        RuntimeError::InvalidInput(format!(
            "CONTRACT_OUTPUT_INVALID: {context} must be an object"
        ))
    })?;
    let actual = object
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .filter(|hash| forgecad_contracts::is_sha256(hash))
        .ok_or_else(|| {
            RuntimeError::InvalidInput(format!(
                "CONTRACT_OUTPUT_INVALID: {context}.canonical_sha256 is invalid"
            ))
        })?;
    let mut input = value.clone();
    input["canonical_sha256"] = Value::String(String::new());
    if canonical_json_hash(&input) != actual {
        return Err(RuntimeError::InvalidInput(format!(
            "CONTRACT_OUTPUT_INVALID: {context}.canonical_sha256 does not bind the payload"
        )));
    }
    Ok(())
}

fn validate_passing_integrity(value: &Value, context: &str) -> Result<(), RuntimeError> {
    let fields = [
        "glb_parse_status",
        "invalid_index_count",
        "non_finite_count",
        "degenerate_triangle_count",
        "boundary_edge_count",
        "non_manifold_edge_count",
        "winding_error_count",
        "uv_non_finite_count",
        "zero_area_uv_triangle_count",
        "tangent_non_finite_count",
        "tangent_orthogonality_error_count",
        "tangent_handedness_error_count",
        "metadata_mismatch_count",
        "external_uri_count",
        "part_coverage",
        "source_coverage",
        "material_zone_coverage",
    ];
    let object = exact_object(value, &fields, context)?;
    if object.get("glb_parse_status").and_then(Value::as_str) != Some("passed") {
        return Err(RuntimeError::InvalidInput(format!(
            "CONTRACT_OUTPUT_INVALID: {context}.glb_parse_status must be passed"
        )));
    }
    for key in [
        "invalid_index_count",
        "non_finite_count",
        "degenerate_triangle_count",
        "boundary_edge_count",
        "non_manifold_edge_count",
        "winding_error_count",
        "uv_non_finite_count",
        "zero_area_uv_triangle_count",
        "tangent_non_finite_count",
        "tangent_orthogonality_error_count",
        "tangent_handedness_error_count",
        "metadata_mismatch_count",
        "external_uri_count",
    ] {
        if object.get(key).and_then(Value::as_u64) != Some(0) {
            return Err(RuntimeError::InvalidInput(format!(
                "CONTRACT_OUTPUT_INVALID: {context}.{key} must be zero"
            )));
        }
    }
    for key in ["part_coverage", "source_coverage", "material_zone_coverage"] {
        if object.get(key).and_then(Value::as_f64) != Some(1.0) {
            return Err(RuntimeError::InvalidInput(format!(
                "CONTRACT_OUTPUT_INVALID: {context}.{key} must be one"
            )));
        }
    }
    Ok(())
}

fn validate_geometry_program_hash_result_output(value: &Value) -> Result<(), RuntimeError> {
    let object = exact_object(
        value,
        &[
            "schema_version",
            "geometry_program_schema_version",
            "canonical_sha256",
            "operator_catalog_sha256",
            "validation_status",
        ],
        "GeometryProgramHashResult@1",
    )?;
    if object.get("schema_version").and_then(Value::as_str) != Some("GeometryProgramHashResult@1")
        || object
            .get("geometry_program_schema_version")
            .and_then(Value::as_str)
            != Some("GeometryProgram@2")
        || object.get("validation_status").and_then(Value::as_str) != Some("passed")
    {
        return Err(RuntimeError::InvalidInput(
            "CONTRACT_OUTPUT_INVALID: GeometryProgramHashResult@1 constants drifted".to_owned(),
        ));
    }
    required_contract_sha256(object, "canonical_sha256", "GeometryProgramHashResult@1")?;
    required_contract_sha256(
        object,
        "operator_catalog_sha256",
        "GeometryProgramHashResult@1",
    )?;
    Ok(())
}

fn validate_artifact_readback_v2_output(value: &Value) -> Result<(), RuntimeError> {
    let fields = [
        "schema_version",
        "artifact_id",
        "candidate_id",
        "object_sha256",
        "mime",
        "size_bytes",
        "program_sha256",
        "operator_catalog_sha256",
        "readback_config_sha256",
        "triangle_count",
        "part_ids",
        "source_node_ids",
        "material_zone_ids",
        "part_bindings",
        "validator_status",
        "hard_gate_passed",
        "integrity",
        "canonical_sha256",
    ];
    let object = exact_object(value, &fields, "ArtifactReadback@2")?;
    if object.get("schema_version").and_then(Value::as_str) != Some("ArtifactReadback@2")
        || object.get("mime").and_then(Value::as_str) != Some("model/gltf-binary")
        || object.get("validator_status").and_then(Value::as_str) != Some("passed")
        || object.get("hard_gate_passed").and_then(Value::as_bool) != Some(true)
        || object
            .get("size_bytes")
            .and_then(Value::as_u64)
            .filter(|size| *size > 0)
            .is_none()
        || object
            .get("triangle_count")
            .and_then(Value::as_u64)
            .is_none()
    {
        return Err(RuntimeError::InvalidInput(
            "CONTRACT_OUTPUT_INVALID: ArtifactReadback@2 constants or scalar values drifted"
                .to_owned(),
        ));
    }
    for key in ["artifact_id", "candidate_id"] {
        required_contract_identifier(object, key, "ArtifactReadback@2")?;
    }
    for key in [
        "object_sha256",
        "program_sha256",
        "operator_catalog_sha256",
        "readback_config_sha256",
    ] {
        required_contract_sha256(object, key, "ArtifactReadback@2")?;
    }
    for key in ["part_ids", "source_node_ids", "material_zone_ids"] {
        let values = object.get(key).and_then(Value::as_array).ok_or_else(|| {
            RuntimeError::InvalidInput(format!("CONTRACT_OUTPUT_INVALID: ArtifactReadback@2.{key}"))
        })?;
        if values.len() > 512
            || values
                .iter()
                .any(|value| !value.as_str().is_some_and(is_opaque_id))
        {
            return Err(RuntimeError::InvalidInput(format!(
                "CONTRACT_OUTPUT_INVALID: ArtifactReadback@2.{key} is invalid"
            )));
        }
    }
    let bindings = object
        .get("part_bindings")
        .and_then(Value::as_array)
        .filter(|values| values.len() <= 512)
        .ok_or_else(|| {
            RuntimeError::InvalidInput(
                "CONTRACT_OUTPUT_INVALID: ArtifactReadback@2.part_bindings is invalid".to_owned(),
            )
        })?;
    for binding in bindings {
        let binding = exact_object(
            binding,
            &[
                "part_id",
                "source_node_id",
                "material_zone_id",
                "solid",
                "triangle_count",
            ],
            "ArtifactReadback@2.part_binding",
        )?;
        for key in ["part_id", "source_node_id", "material_zone_id"] {
            required_contract_identifier(binding, key, "ArtifactReadback@2.part_binding")?;
        }
        if binding.get("solid").and_then(Value::as_bool).is_none()
            || binding
                .get("triangle_count")
                .and_then(Value::as_u64)
                .is_none()
        {
            return Err(RuntimeError::InvalidInput(
                "CONTRACT_OUTPUT_INVALID: ArtifactReadback@2.part_binding values drifted"
                    .to_owned(),
            ));
        }
    }
    validate_passing_integrity(
        object
            .get("integrity")
            .expect("integrity field was required"),
        "ArtifactReadback@2.integrity",
    )?;
    verify_output_canonical_hash(value, "ArtifactReadback@2")
}

fn validate_geometry_quality_report_v2_output(value: &Value) -> Result<(), RuntimeError> {
    let object = exact_object(
        value,
        &[
            "schema_version",
            "scope",
            "quality_report_id",
            "candidate_id",
            "artifact_sha256",
            "program_sha256",
            "operator_catalog_sha256",
            "readback_config_sha256",
            "artifact_readback_object_sha256",
            "integrity",
            "hard_gate_passed",
            "canonical_sha256",
        ],
        "GeometryQualityReport@2",
    )?;
    if object.get("schema_version").and_then(Value::as_str) != Some("GeometryQualityReport@2")
        || object.get("scope").and_then(Value::as_str)
            != Some("mcp010b-strict-glb-bin-accessor-hard-gates")
        || object.get("hard_gate_passed").and_then(Value::as_bool) != Some(true)
    {
        return Err(RuntimeError::InvalidInput(
            "CONTRACT_OUTPUT_INVALID: GeometryQualityReport@2 constants drifted".to_owned(),
        ));
    }
    for key in ["quality_report_id", "candidate_id"] {
        required_contract_identifier(object, key, "GeometryQualityReport@2")?;
    }
    for key in [
        "artifact_sha256",
        "program_sha256",
        "operator_catalog_sha256",
        "readback_config_sha256",
        "artifact_readback_object_sha256",
    ] {
        required_contract_sha256(object, key, "GeometryQualityReport@2")?;
    }
    validate_passing_integrity(
        object
            .get("integrity")
            .expect("integrity field was required"),
        "GeometryQualityReport@2.integrity",
    )?;
    verify_output_canonical_hash(value, "GeometryQualityReport@2")
}

fn validate_geometry_candidate_evidence_output(value: &Value) -> Result<(), RuntimeError> {
    let object = exact_object(
        value,
        &[
            "schema_version",
            "candidate_id",
            "project_id",
            "reference_id",
            "reference_sha256",
            "geometry_program_sha256",
            "geometry_program_object_sha256",
            "operator_catalog_sha256",
            "readback_config_sha256",
            "artifact_object_sha256",
            "artifact_readback_object_sha256",
            "quality_report_object_sha256",
            "quality_report_id",
            "canonical_sha256",
            "created_at",
        ],
        "GeometryCandidateEvidence@1",
    )?;
    if object.get("schema_version").and_then(Value::as_str) != Some("GeometryCandidateEvidence@1") {
        return Err(RuntimeError::InvalidInput(
            "CONTRACT_OUTPUT_INVALID: GeometryCandidateEvidence@1 schema version drifted"
                .to_owned(),
        ));
    }
    for key in ["candidate_id", "project_id", "quality_report_id"] {
        required_contract_identifier(object, key, "GeometryCandidateEvidence@1")?;
    }
    for key in [
        "geometry_program_sha256",
        "geometry_program_object_sha256",
        "operator_catalog_sha256",
        "readback_config_sha256",
        "artifact_object_sha256",
        "artifact_readback_object_sha256",
        "quality_report_object_sha256",
    ] {
        required_contract_sha256(object, key, "GeometryCandidateEvidence@1")?;
    }
    match (object.get("reference_id"), object.get("reference_sha256")) {
        (Some(Value::Null), Some(Value::Null)) => {}
        (Some(Value::String(reference_id)), Some(Value::String(reference_sha256)))
            if is_opaque_id(reference_id) && forgecad_contracts::is_sha256(reference_sha256) => {}
        _ => {
            return Err(RuntimeError::InvalidInput(
                "CONTRACT_OUTPUT_INVALID: GeometryCandidateEvidence@1 reference binding drifted"
                    .to_owned(),
            ));
        }
    }
    if object
        .get("created_at")
        .and_then(Value::as_str)
        .is_none_or(|value| value.is_empty() || value.len() > 64)
    {
        return Err(RuntimeError::InvalidInput(
            "CONTRACT_OUTPUT_INVALID: GeometryCandidateEvidence@1 created_at is invalid".to_owned(),
        ));
    }
    verify_output_canonical_hash(value, "GeometryCandidateEvidence@1")
}

fn validate_geometry_prepare_result_v2_output(value: &Value) -> Result<(), RuntimeError> {
    let object = exact_object(
        value,
        &[
            "schema_version",
            "candidate",
            "job",
            "operator_catalog",
            "artifact",
        ],
        "GeometryPrepareResult@2",
    )?;
    if object.get("schema_version").and_then(Value::as_str) != Some("GeometryPrepareResult@2")
        || object.get("operator_catalog") != Some(&operator_catalog())
    {
        return Err(RuntimeError::InvalidInput(
            "CONTRACT_OUTPUT_INVALID: GeometryPrepareResult@2 constants drifted".to_owned(),
        ));
    }
    let candidate: CandidateRecord =
        serde_json::from_value(object.get("candidate").cloned().ok_or_else(|| {
            RuntimeError::InvalidInput("CONTRACT_OUTPUT_INVALID: candidate".to_owned())
        })?)
        .map_err(|_| RuntimeError::InvalidInput("CONTRACT_OUTPUT_INVALID: candidate".to_owned()))?;
    if candidate.schema_version != "Candidate@1" || !is_opaque_id(&candidate.candidate_id) {
        return Err(RuntimeError::InvalidInput(
            "CONTRACT_OUTPUT_INVALID: GeometryPrepareResult@2 candidate drifted".to_owned(),
        ));
    }
    let job_value = object
        .get("job")
        .ok_or_else(|| RuntimeError::InvalidInput("CONTRACT_OUTPUT_INVALID: job".to_owned()))?;
    exact_object(
        job_value,
        &[
            "job_id",
            "project_id",
            "kind",
            "status",
            "progress",
            "error_code",
            "created_at",
            "updated_at",
        ],
        "GeometryPrepareResult@2.job",
    )?;
    let job: JobSummary = serde_json::from_value(job_value.clone())
        .map_err(|_| RuntimeError::InvalidInput("CONTRACT_OUTPUT_INVALID: job".to_owned()))?;
    if !is_opaque_id(&job.job_id) {
        return Err(RuntimeError::InvalidInput(
            "CONTRACT_OUTPUT_INVALID: GeometryPrepareResult@2 job drifted".to_owned(),
        ));
    }
    validate_artifact_readback_v2_output(
        object.get("artifact").expect("artifact field was required"),
    )
}

struct GlbInspection {
    part_ids: Vec<String>,
    aspect_ratio: f64,
}

fn inspect_glb(bytes: &[u8]) -> Result<GlbInspection, RuntimeError> {
    let inspection = strict_glb_inspection(bytes)?;
    if inspection.part_ids.is_empty() || inspection.triangle_count == 0 {
        return Err(RuntimeError::InvalidInput(
            "GLB readback is empty".to_owned(),
        ));
    }
    Ok(GlbInspection {
        part_ids: inspection.part_ids,
        aspect_ratio: inspection.aspect_ratio,
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

    fn v2_restore_program(project_id: &str) -> Value {
        let mut program = json!({
            "schema_version":"GeometryProgram@2",
            "project_id":project_id,
            "representation_plan_sha256":"9".repeat(64),
            "operator_catalog_sha256":operator_catalog_sha256(),
            "units":{"length":"meter","angle":"radian","coordinate_system":"right-handed-y-up"},
            "budgets":{"max_nodes":1,"max_triangles":1000,"max_glb_bytes":1048576,"max_worker_memory_bytes":536870912,"max_runtime_ms":10000},
            "nodes":[{"node_id":"shell","operator_id":"forgecad.geometry.primitive@2","inputs":[],"parameters":{"shape":"box","size_m":[1.0,1.2,0.5],"position_m":[0.0,1.0,0.0],"rotation_rad":[0.0,0.0,0.0]}}],
            "part_outputs":[{"part_id":"shell","input_node_ids":["shell"],"material_zone_id":"zone-white-shell","solid":true}]
        });
        program["canonical_sha256"] = Value::String(canonical_json_hash(&program));
        program
    }

    fn prepare_and_confirm_v2_restore_source(
        runtime: &Runtime,
        project_id: &str,
        idempotency_suffix: &str,
    ) -> (Value, CandidateConfirmResult) {
        let prepared = runtime
            .prepare_geometry_candidate(
                project_id,
                None,
                json!({"typed":"geometry","geometry_program":v2_restore_program(project_id)}),
            )
            .expect("V2 source prepare");
        let candidate = &prepared["candidate"];
        let confirmed = runtime
            .confirm_candidate(&CandidateConfirmRequest {
                project_id: project_id.to_owned(),
                candidate_id: candidate["candidate_id"]
                    .as_str()
                    .expect("candidate ID")
                    .to_owned(),
                base_version_id: None,
                prepared_object_id: candidate["prepared_object_id"]
                    .as_str()
                    .expect("prepared object ID")
                    .to_owned(),
                prepared_object_sha256: candidate["prepared_object_sha256"]
                    .as_str()
                    .expect("prepared object SHA-256")
                    .to_owned(),
                quality_report_id: candidate["quality_report_id"]
                    .as_str()
                    .expect("quality report ID")
                    .to_owned(),
                approval_receipt_id: format!("restore-v2-source-approval-{idempotency_suffix}"),
                approval_summary: "Approve V2 restore source".to_owned(),
                approval_session_id: format!("restore-v2-source-session-{idempotency_suffix}"),
                approval_expires_at: "9999999999".to_owned(),
                idempotency_key: format!("restore-v2-source-confirm-{idempotency_suffix}"),
            })
            .expect("V2 source confirm");
        (prepared, confirmed)
    }

    fn restore_confirm_request(
        project_id: &str,
        source_version_id: &str,
        restored_candidate: &CandidateRecord,
        idempotency_suffix: &str,
    ) -> RestoreConfirmRequest {
        RestoreConfirmRequest {
            project_id: project_id.to_owned(),
            candidate_id: restored_candidate.candidate_id.clone(),
            source_version_id: source_version_id.to_owned(),
            base_version_id: restored_candidate.base_version_id.clone(),
            prepared_object_id: restored_candidate
                .prepared_object_id
                .clone()
                .expect("restore prepared object ID"),
            prepared_object_sha256: restored_candidate
                .prepared_object_sha256
                .clone()
                .expect("restore prepared object SHA-256"),
            quality_report_id: restored_candidate
                .quality_report_id
                .clone()
                .expect("restore quality report ID"),
            approval_receipt_id: format!("restore-v2-approval-{idempotency_suffix}"),
            approval_summary: "Approve V2 restore".to_owned(),
            approval_session_id: format!("restore-v2-session-{idempotency_suffix}"),
            approval_expires_at: "9999999999".to_owned(),
            idempotency_key: format!("restore-v2-confirm-{idempotency_suffix}"),
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
        assert_eq!(
            readback["triangle_count"],
            result["artifact"]["triangle_count"]
        );
        assert_eq!(readback["validator_status"], "passed");
    }

    #[test]
    fn v2_geometry_candidate_returns_candidate_bound_strict_readback() {
        let runtime = Runtime::ephemeral().expect("runtime");
        let project = runtime
            .create_project("MCP010B V2 geometry project", json!({"profile":"mvp"}))
            .expect("project");
        let mut program = json!({
            "schema_version":"GeometryProgram@2",
            "project_id":project.project_id.clone(),
            "representation_plan_sha256":"f".repeat(64),
            "operator_catalog_sha256":operator_catalog_sha256(),
            "units":{"length":"meter","angle":"radian","coordinate_system":"right-handed-y-up"},
            "budgets":{"max_nodes":4,"max_triangles":10000,"max_glb_bytes":1048576,"max_worker_memory_bytes":536870912,"max_runtime_ms":10000},
            "nodes":[
                {"node_id":"shell","operator_id":"forgecad.geometry.primitive@2","inputs":[],"parameters":{"shape":"box","size_m":[1.2,1.6,0.55],"position_m":[0.0,1.7,0.0],"rotation_rad":[0.0,0.0,0.0]}},
                {"node_id":"head","operator_id":"forgecad.geometry.primitive@2","inputs":[],"parameters":{"shape":"ellipsoid","radii_m":[0.42,0.46,0.42],"longitude_segments":16,"latitude_segments":8,"position_m":[0.0,2.75,0.0],"rotation_rad":[0.0,0.0,0.0]}}
            ],
            "part_outputs":[
                {"part_id":"shell","input_node_ids":["shell"],"material_zone_id":"zone-white-shell","solid":true},
                {"part_id":"head","input_node_ids":["head"],"material_zone_id":"zone-white-shell","solid":true}
            ]
        });
        program["canonical_sha256"] = Value::String(canonical_json_hash(&program));
        let result = runtime
            .prepare_geometry_candidate(
                &project.project_id,
                None,
                json!({"typed":"geometry","geometry_program":program}),
            )
            .expect("V2 geometry prepare");
        assert_eq!(result["schema_version"], "GeometryPrepareResult@2");
        assert_eq!(result["artifact"]["schema_version"], "ArtifactReadback@2");
        assert_eq!(result["artifact"]["hard_gate_passed"], true);
        assert_eq!(
            result["artifact"]["integrity"]["degenerate_triangle_count"],
            0
        );
        assert_eq!(result["artifact"]["integrity"]["boundary_edge_count"], 0);
        assert_eq!(result["artifact"]["integrity"]["winding_error_count"], 0);
        assert_eq!(
            result["artifact"]["part_bindings"]
                .as_array()
                .expect("bindings")
                .len(),
            2
        );
        assert_eq!(
            result["operator_catalog"]["canonical_sha256"],
            result["artifact"]["operator_catalog_sha256"]
        );
        let artifact_id = result["artifact"]["artifact_id"]
            .as_str()
            .expect("artifact");
        let candidate_id = result["candidate"]["candidate_id"]
            .as_str()
            .expect("candidate");
        let reread = runtime
            .artifact_readback(artifact_id, candidate_id)
            .expect("candidate-bound readback");
        assert_eq!(
            reread["canonical_sha256"],
            result["artifact"]["canonical_sha256"]
        );
        let quality = runtime.quality(candidate_id, None).expect("V2 quality");
        assert_eq!(
            quality["checks"]
                .as_array()
                .expect("checks")
                .iter()
                .find(|check| check["check_id"] == "uv_tangent")
                .expect("UV/tangent check")["status"],
            "passed"
        );
        assert!(runtime
            .artifact_readback(artifact_id, "candidate-not-bound")
            .is_err());
    }

    #[test]
    fn default_camera_framing_uses_reference_mask_only_when_camera_is_omitted() {
        let camera = default_camera_calibration();
        let mut reference = vec![false; 512 * 512];
        let mut model = vec![false; 512 * 512];
        for y in 48..464 {
            for x in 180..332 {
                reference[y * 512 + x] = true;
            }
        }
        for y in 120..328 {
            for x in 200..312 {
                model[y * 512 + x] = true;
            }
        }
        let calibrated = calibrate_default_camera(&camera, &reference, &model);
        assert_ne!(calibrated, camera);
        let original_position = camera["transform"]["position_m"][1]
            .as_f64()
            .expect("original camera position");
        let calibrated_position = calibrated["transform"]["position_m"][1]
            .as_f64()
            .expect("calibrated camera position");
        assert!(calibrated_position < original_position);
        assert!(validate_camera_calibration(&calibrated).is_ok());
        let explicit = camera.clone();
        assert_eq!(explicit["camera_hash"], camera["camera_hash"]);
    }

    #[test]
    fn c_fixed_renderer_persists_nine_aovs_and_review_chain() {
        let runtime = Runtime::ephemeral().expect("runtime");
        let project = runtime
            .create_project("MCP010C renderer project", json!({"profile":"mvp"}))
            .expect("project");
        let reference = runtime
            .import_reference(&ReferenceImportRequest {
                project_id: project.project_id.clone(),
                source: ReferenceImportSource::InlineContent {
                    mime: "image/png".to_owned(),
                    content_base64: "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=".to_owned(),
                },
                authorization: ReferenceAuthorization {
                    user_authorized: true,
                    declaration: "MCP010C fixed renderer test reference".to_owned(),
                },
                expected_sha256: None,
            })
            .expect("reference import")
            .reference;
        let mut program = json!({
            "schema_version":"GeometryProgram@2",
            "project_id":project.project_id,
            "representation_plan_sha256":"e".repeat(64),
            "operator_catalog_sha256":operator_catalog_sha256(),
            "units":{"length":"meter","angle":"radian","coordinate_system":"right-handed-y-up"},
            "budgets":{"max_nodes":1,"max_triangles":1000,"max_glb_bytes":1048576,"max_worker_memory_bytes":536870912,"max_runtime_ms":10000},
            "nodes":[{"node_id":"shell","operator_id":"forgecad.geometry.primitive@2","inputs":[],"parameters":{"shape":"box","size_m":[1.0,1.2,0.5],"position_m":[0.0,1.0,0.0],"rotation_rad":[0.0,0.0,0.0]}}],
            "part_outputs":[{"part_id":"shell","input_node_ids":["shell"],"material_zone_id":"zone-white-shell","solid":true}]
        });
        program["canonical_sha256"] = Value::String(canonical_json_hash(&program));
        let prepared = runtime
            .prepare_geometry_candidate(
                &project.project_id,
                None,
                json!({"typed":"geometry","geometry_program":program}),
            )
            .expect("geometry prepare");
        let candidate_id = prepared["candidate"]["candidate_id"]
            .as_str()
            .expect("candidate")
            .to_owned();
        let mut view_spec = json!({
            "schema_version":"ReferenceViewSpec@1",
            "reference_id":reference.reference_id,
            "reference_sha256":reference.object_sha256,
            "view_id":"three-quarter-test",
            "source_view":"three-quarter",
            "image":{"width":1,"height":1,"rotation_degrees":0.0,"crop":{"x":0.0,"y":0.0,"width":1.0,"height":1.0}},
            "landmarks":[],
            "regions":[],
            "canonical_sha256":""
        });
        view_spec["canonical_sha256"] = Value::String(canonical_json_hash(&view_spec));
        let prepared_visual = runtime
            .prepare_reference_comparison(
                &project.project_id,
                json!({"candidate_id":candidate_id,"reference_id":reference.reference_id,"view_spec":view_spec}),
            )
            .expect("fixed renderer comparison");
        let render_set = &prepared_visual["render_set"];
        assert_eq!(render_set["schema_version"], "RenderSet@2");
        assert_eq!(render_set["passes"].as_array().unwrap().len(), 9);
        let render_set_hash = prepared_visual["render_set_object_sha256"]
            .as_str()
            .unwrap();
        let pass = runtime
            .render_pass_get(render_set_hash, "beauty")
            .expect("beauty pass");
        assert_eq!(pass["width"], 512);
        assert!(pass["png_base64"]
            .as_str()
            .is_some_and(|value| !value.is_empty()));
        let comparison_hash = prepared_visual["comparison_report_object_sha256"]
            .as_str()
            .unwrap()
            .to_owned();
        let review = runtime
            .submit_visual_review(json!({
                "candidate_id":candidate_id,
                "reference_id":reference.reference_id,
                "render_set_hash":render_set_hash,
                "comparison_report_hash":comparison_hash,
                "round":1,
                "stage":"silhouette",
                "issues":[],
                "status":"needs_revision"
            }))
            .expect("Codex visual review");
        assert_eq!(review["review"]["schema_version"], "VisualReviewReport@1");
        let human = runtime
            .submit_human_visual_review(json!({
                "candidate_id":candidate_id,
                "reference_id":reference.reference_id,
                "render_set_hash":render_set_hash,
                "comparison_report_hash":comparison_hash,
                "scores":{"likeness":3,"geometry_detail":3,"material_fidelity":2,"editability":5},
                "approved":false
            }))
            .expect("human visual review");
        assert_eq!(
            human["receipt"]["schema_version"],
            "HumanVisualReviewReceipt@1"
        );
        assert_eq!(
            runtime.quality(&candidate_id, None).unwrap()["schema_version"],
            "QualityReport@2"
        );
        let viewer_evidence = runtime
            .visual_evidence(&candidate_id)
            .expect("viewer visual evidence");
        assert_eq!(viewer_evidence["schema_version"], "ViewerVisualEvidence@1");
        assert_eq!(viewer_evidence["candidate_id"], candidate_id);
        assert_eq!(viewer_evidence["reference_id"], reference.reference_id);
        assert_eq!(
            viewer_evidence["render_set_hash"],
            prepared_visual["render_set_object_sha256"]
        );
        assert_eq!(
            viewer_evidence["comparison_report"]["schema_version"],
            "ReferenceComparisonReport@1"
        );
    }

    #[test]
    fn v2_candidate_evidence_binds_reference_and_is_revalidated_on_confirm() {
        fn import_png(
            runtime: &Runtime,
            project_id: &str,
            declaration: &str,
        ) -> ReferenceEvidenceRecord {
            runtime
                .import_reference(&ReferenceImportRequest {
                    project_id: project_id.to_owned(),
                    source: ReferenceImportSource::InlineContent {
                        mime: "image/png".to_owned(),
                        content_base64: "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=".to_owned(),
                    },
                    authorization: ReferenceAuthorization {
                        user_authorized: true,
                        declaration: declaration.to_owned(),
                    },
                    expected_sha256: None,
                })
                .expect("reference import")
                .reference
        }

        fn v2_program(project_id: &str) -> Value {
            let mut program = json!({
                "schema_version":"GeometryProgram@2",
                "project_id":project_id,
                "representation_plan_sha256":"c".repeat(64),
                "operator_catalog_sha256":operator_catalog_sha256(),
                "units":{"length":"meter","angle":"radian","coordinate_system":"right-handed-y-up"},
                "budgets":{"max_nodes":1,"max_triangles":1000,"max_glb_bytes":1048576,"max_worker_memory_bytes":536870912,"max_runtime_ms":10000},
                "nodes":[{"node_id":"shell","operator_id":"forgecad.geometry.primitive@2","inputs":[],"parameters":{"shape":"box","size_m":[1.0,1.2,0.5],"position_m":[0.0,1.0,0.0],"rotation_rad":[0.0,0.0,0.0]}}],
                "part_outputs":[{"part_id":"shell","input_node_ids":["shell"],"material_zone_id":"zone-white-shell","solid":true}]
            });
            program["canonical_sha256"] = Value::String(canonical_json_hash(&program));
            program
        }

        let runtime = Runtime::ephemeral().expect("runtime");
        let project = runtime
            .create_project("V2 evidence project", json!({"profile":"mvp"}))
            .expect("project");
        let foreign_project = runtime
            .create_project("foreign reference project", json!({"profile":"mvp"}))
            .expect("foreign project");
        let reference = import_png(&runtime, &project.project_id, "authorized local reference");
        let foreign_reference = import_png(
            &runtime,
            &foreign_project.project_id,
            "authorized foreign fixture reference",
        );

        let foreign_error = runtime
            .prepare_geometry_candidate(
                &project.project_id,
                None,
                json!({
                    "typed":"geometry",
                    "reference_id":foreign_reference.reference_id,
                    "geometry_program":v2_program(&project.project_id)
                }),
            )
            .expect_err("foreign reference must fail before candidate persistence");
        assert!(foreign_error.to_string().contains("REFERENCE_SCOPE_DENIED"));
        assert!(runtime.candidates(&project.project_id).unwrap().is_empty());

        let result = runtime
            .prepare_geometry_candidate(
                &project.project_id,
                None,
                json!({
                    "typed":"geometry",
                    "reference_id":reference.reference_id,
                    "geometry_program":v2_program(&project.project_id)
                }),
            )
            .expect("bound V2 geometry prepare");
        let candidate = result["candidate"].clone();
        let candidate_id = candidate["candidate_id"].as_str().expect("candidate ID");
        let evidence = runtime
            .store
            .get_geometry_candidate_evidence(candidate_id)
            .expect("evidence query")
            .expect("V2 evidence record");
        assert_eq!(evidence.project_id, project.project_id);
        assert_eq!(
            evidence.reference_id.as_deref(),
            Some(reference.reference_id.as_str())
        );
        assert_eq!(
            evidence.reference_sha256.as_deref(),
            Some(reference.object_sha256.as_str())
        );
        assert_eq!(
            evidence.geometry_program_sha256,
            result["artifact"]["program_sha256"]
                .as_str()
                .expect("program hash")
        );
        assert!(runtime
            .quality(candidate_id, Some(&foreign_reference.reference_id))
            .is_err());
        assert_eq!(
            runtime
                .quality(candidate_id, Some(&reference.reference_id))
                .expect("bound quality")["reference_compare"]["reference_id"],
            reference.reference_id
        );

        let confirmed = runtime
            .confirm_candidate(&CandidateConfirmRequest {
                project_id: project.project_id.clone(),
                candidate_id: candidate_id.to_owned(),
                base_version_id: None,
                prepared_object_id: candidate["prepared_object_id"]
                    .as_str()
                    .expect("object ID")
                    .to_owned(),
                prepared_object_sha256: candidate["prepared_object_sha256"]
                    .as_str()
                    .expect("object hash")
                    .to_owned(),
                quality_report_id: candidate["quality_report_id"]
                    .as_str()
                    .expect("quality ID")
                    .to_owned(),
                approval_receipt_id: "mcp010b-evidence-approval".to_owned(),
                approval_summary: "Approve hash-bound V2 geometry evidence".to_owned(),
                approval_session_id: "mcp010b-evidence-session".to_owned(),
                approval_expires_at: "9999999999".to_owned(),
                idempotency_key: "mcp010b-evidence-confirm-once".to_owned(),
            })
            .expect("V2 confirmation revalidates evidence");
        assert_eq!(confirmed.project_id, project.project_id);
    }

    #[test]
    fn v2_restore_rebuilds_candidate_bound_evidence_before_becoming_reviewable() {
        let runtime = Runtime::ephemeral().expect("runtime");
        let project = runtime
            .create_project("V2 restore provenance project", json!({"profile":"mvp"}))
            .expect("project");
        let (source_prepared, source_version) =
            prepare_and_confirm_v2_restore_source(&runtime, &project.project_id, "fresh-evidence");
        let source_candidate_id = source_prepared["candidate"]["candidate_id"]
            .as_str()
            .expect("source candidate ID");
        let source_evidence = runtime
            .store
            .get_geometry_candidate_evidence(source_candidate_id)
            .expect("source evidence query")
            .expect("source V2 evidence");

        let restored = runtime
            .prepare_restore(&RestorePrepareRequest {
                project_id: project.project_id.clone(),
                base_version_id: Some(source_version.version_id.clone()),
                source_version_id: source_version.version_id.clone(),
                request: json!({"reason":"rebuild V2 provenance"}),
            })
            .expect("V2 restore prepare");
        assert_eq!(restored.candidate.state, "reviewable");
        assert!(restored.candidate.quality_hard_gate_passed);
        let restored_evidence = runtime
            .store
            .get_geometry_candidate_evidence(&restored.candidate.candidate_id)
            .expect("restored evidence query")
            .expect("restored V2 evidence");
        assert_eq!(
            restored_evidence.candidate_id,
            restored.candidate.candidate_id
        );
        assert_eq!(
            restored.candidate.quality_report_id.as_deref(),
            Some(restored_evidence.quality_report_id.as_str())
        );
        assert_eq!(
            restored_evidence.geometry_program_object_sha256,
            source_evidence.geometry_program_object_sha256
        );
        assert_eq!(
            restored_evidence.artifact_object_sha256,
            source_evidence.artifact_object_sha256
        );
        assert_ne!(
            restored_evidence.artifact_readback_object_sha256,
            source_evidence.artifact_readback_object_sha256
        );
        assert_ne!(
            restored_evidence.quality_report_object_sha256,
            source_evidence.quality_report_object_sha256
        );

        let confirmed = runtime
            .confirm_restore(&restore_confirm_request(
                &project.project_id,
                &source_version.version_id,
                &restored.candidate,
                "fresh-evidence",
            ))
            .expect("V2 restore confirm");
        assert_eq!(confirmed.source_version_id, source_version.version_id);
        assert_eq!(
            runtime
                .candidate(&restored.candidate.candidate_id)
                .expect("restored candidate query")
                .expect("restored candidate")
                .state,
            "confirmed"
        );
        assert_eq!(
            runtime
                .versions(Some(&project.project_id))
                .expect("versions")
                .len(),
            2
        );
    }

    #[test]
    fn v2_restore_without_source_evidence_never_becomes_reviewable_or_creates_version() {
        let runtime = Runtime::ephemeral().expect("runtime");
        let project = runtime
            .create_project(
                "V2 restore missing evidence project",
                json!({"profile":"mvp"}),
            )
            .expect("project");
        let program = v2_restore_program(&project.project_id);
        let artifact = forgecad_geometry_worker::compile_geometry_program(&program)
            .expect("direct V2 artifact");
        let object = runtime
            .put_object(&artifact.glb, None, "model/gltf-binary", "geometry-glb")
            .expect("V2 GLB object");
        let source_prepared = runtime
            .prepare_candidate(
                &project.project_id,
                None,
                "direct-v2-source-object",
                &object.record.sha256,
                json!({"typed":"direct-v2-source"}),
            )
            .expect("direct V2 source prepare");
        let source_candidate = runtime
            .mark_candidate_quality(
                &source_prepared.candidate.candidate_id,
                "quality-direct-v2-source",
                true,
            )
            .expect("direct V2 quality");
        let source_version = runtime
            .store
            .confirm_candidate(
                &CandidateConfirmRequest {
                    project_id: project.project_id.clone(),
                    candidate_id: source_candidate.candidate_id.clone(),
                    base_version_id: None,
                    prepared_object_id: "direct-v2-source-object".to_owned(),
                    prepared_object_sha256: object.record.sha256.clone(),
                    quality_report_id: "quality-direct-v2-source".to_owned(),
                    approval_receipt_id: "direct-v2-source-approval".to_owned(),
                    approval_summary: "Create malformed persisted V2 source".to_owned(),
                    approval_session_id: "direct-v2-source-session".to_owned(),
                    approval_expires_at: "9999999999".to_owned(),
                    idempotency_key: "direct-v2-source-confirm".to_owned(),
                },
                &now_string(),
            )
            .expect("direct Store confirm models legacy malformed state");
        assert!(runtime
            .store
            .get_geometry_candidate_evidence(&source_candidate.candidate_id)
            .expect("source evidence query")
            .is_none());
        let version_count = runtime
            .versions(Some(&project.project_id))
            .expect("versions before restore")
            .len();

        let error = runtime
            .prepare_restore(&RestorePrepareRequest {
                project_id: project.project_id.clone(),
                base_version_id: Some(source_version.version_id.clone()),
                source_version_id: source_version.version_id.clone(),
                request: json!({"reason":"must reject missing source evidence"}),
            })
            .expect_err("V2 restore source without evidence must fail closed");
        assert!(error
            .to_string()
            .contains("V2 restore source is missing durable geometry evidence"));
        assert_eq!(
            runtime
                .versions(Some(&project.project_id))
                .expect("versions after rejected restore")
                .len(),
            version_count
        );
        let attempted_restore = runtime
            .candidates(&project.project_id)
            .expect("candidates")
            .into_iter()
            .find(|candidate| {
                candidate.source_version_id.as_deref() == Some(source_version.version_id.as_str())
            })
            .expect("prepared restore candidate");
        assert_eq!(attempted_restore.state, "prepared");
        assert!(!attempted_restore.quality_hard_gate_passed);
        assert!(runtime
            .store
            .get_geometry_candidate_evidence(&attempted_restore.candidate_id)
            .expect("restore evidence query")
            .is_none());
    }

    #[test]
    fn v2_restore_confirmation_revalidates_cas_and_does_not_create_a_version() {
        let runtime = Runtime::ephemeral().expect("runtime");
        let project = runtime
            .create_project(
                "V2 restore CAS corruption project",
                json!({"profile":"mvp"}),
            )
            .expect("project");
        let (_source_prepared, source_version) =
            prepare_and_confirm_v2_restore_source(&runtime, &project.project_id, "cas-corrupt");
        let restored = runtime
            .prepare_restore(&RestorePrepareRequest {
                project_id: project.project_id.clone(),
                base_version_id: Some(source_version.version_id.clone()),
                source_version_id: source_version.version_id.clone(),
                request: json!({"reason":"verify confirmation readback"}),
            })
            .expect("V2 restore prepare");
        let artifact_sha256 = restored
            .candidate
            .prepared_object_sha256
            .clone()
            .expect("restore artifact SHA-256");
        let object_path = runtime
            .store
            .cas()
            .root()
            .join("objects")
            .join(&artifact_sha256[..2])
            .join(&artifact_sha256);
        fs::write(object_path, b"corrupt restored V2 artifact")
            .expect("corrupt test-only CAS object");
        let version_count = runtime
            .versions(Some(&project.project_id))
            .expect("versions before corrupted confirmation")
            .len();

        let error = runtime
            .confirm_restore(&restore_confirm_request(
                &project.project_id,
                &source_version.version_id,
                &restored.candidate,
                "cas-corrupt",
            ))
            .expect_err("corrupt restore artifact must not confirm");
        assert!(error.to_string().contains("CAS"));
        assert_eq!(
            runtime
                .versions(Some(&project.project_id))
                .expect("versions after corrupted confirmation")
                .len(),
            version_count
        );
        assert_eq!(
            runtime
                .candidate(&restored.candidate.candidate_id)
                .expect("restored candidate query")
                .expect("restored candidate")
                .state,
            "reviewable"
        );
    }

    #[test]
    fn v2_runtime_output_validators_fail_closed_on_mutated_receipts() {
        let runtime = Runtime::ephemeral().expect("runtime");
        let project = runtime
            .create_project("V2 output validator project", json!({"profile":"mvp"}))
            .expect("project");
        let result = runtime
            .prepare_geometry_candidate(
                &project.project_id,
                None,
                json!({"typed":"geometry","geometry_program":{
                    "schema_version":"GeometryProgram@2",
                    "project_id":project.project_id,
                    "representation_plan_sha256":"d".repeat(64),
                    "operator_catalog_sha256":operator_catalog_sha256(),
                    "units":{"length":"meter","angle":"radian","coordinate_system":"right-handed-y-up"},
                    "budgets":{"max_nodes":1,"max_triangles":1000,"max_glb_bytes":1048576,"max_worker_memory_bytes":536870912,"max_runtime_ms":10000},
                    "nodes":[{"node_id":"shell","operator_id":"forgecad.geometry.primitive@2","inputs":[],"parameters":{"shape":"box","size_m":[1.0,1.2,0.5],"position_m":[0.0,1.0,0.0],"rotation_rad":[0.0,0.0,0.0]}}],
                    "part_outputs":[{"part_id":"shell","input_node_ids":["shell"],"material_zone_id":"zone-white-shell","solid":true}],
                    "canonical_sha256":""
                }}),
            );
        assert!(
            result.is_err(),
            "a noncanonical V2 program never reaches output validation"
        );

        let mut artifact = json!({
            "schema_version":"ArtifactReadback@2",
            "artifact_id":"artifact-test",
            "candidate_id":"candidate-test",
            "object_sha256":"a".repeat(64),
            "mime":"model/gltf-binary",
            "size_bytes":1,
            "program_sha256":"b".repeat(64),
            "operator_catalog_sha256":"c".repeat(64),
            "readback_config_sha256":"d".repeat(64),
            "triangle_count":1,
            "part_ids":[],"source_node_ids":[],"material_zone_ids":[],"part_bindings":[],
            "validator_status":"passed","hard_gate_passed":true,
            "integrity":{
                "glb_parse_status":"passed","invalid_index_count":0,"non_finite_count":0,"degenerate_triangle_count":0,"boundary_edge_count":0,"non_manifold_edge_count":0,"winding_error_count":0,"uv_non_finite_count":0,"zero_area_uv_triangle_count":0,"tangent_non_finite_count":0,"tangent_orthogonality_error_count":0,"tangent_handedness_error_count":0,"metadata_mismatch_count":0,"external_uri_count":0,"part_coverage":1.0,"source_coverage":1.0,"material_zone_coverage":1.0
            },"canonical_sha256":""
        });
        artifact["canonical_sha256"] = Value::String(canonical_json_hash(&artifact));
        validate_artifact_readback_v2_output(&artifact).expect("valid canonical receipt shape");
        artifact["integrity"]["winding_error_count"] = json!(1);
        assert!(validate_artifact_readback_v2_output(&artifact).is_err());

        let mut review = json!({
            "schema_version":"VisualReviewReport@1",
            "review_id":"review-test",
            "candidate_id":"candidate-test",
            "reference_id":"reference-test",
            "render_set_hash":"e".repeat(64),
            "comparison_report_hash":"f".repeat(64),
            "round":1,
            "stage":"silhouette",
            "issues":[{"issue_id":"issue-test","pass":"silhouette","region_id":"torso","claim":"silhouette is too narrow","confidence":0.9,"visibility":"observed","action":"widen torso shell"}],
            "status":"needs_revision",
            "canonical_sha256":""
        });
        review["canonical_sha256"] = Value::String(canonical_json_hash(&review));
        validate_visual_review_report(&review).expect("valid visual review receipt shape");
        review["issues"][0]["unexpected"] = json!(true);
        assert!(validate_visual_review_report(&review).is_err());

        let mut human = json!({
            "schema_version":"HumanVisualReviewReceipt@1",
            "receipt_id":"human-test",
            "candidate_id":"candidate-test",
            "reference_id":"reference-test",
            "render_set_hash":"e".repeat(64),
            "comparison_report_hash":"f".repeat(64),
            "scores":{"likeness":4,"geometry_detail":4,"material_fidelity":3,"editability":5},
            "approved":false,
            "recorded_at":"2026-08-10T00:00:00Z",
            "canonical_sha256":""
        });
        human["canonical_sha256"] = Value::String(canonical_json_hash(&human));
        validate_human_review_receipt(&human).expect("valid human review receipt shape");
        human["scores"]["unexpected"] = json!(1);
        assert!(validate_human_review_receipt(&human).is_err());
    }

    #[test]
    fn geometry_program_hash_is_read_only_and_compiler_compatible() {
        let runtime = Runtime::ephemeral().expect("runtime");
        let project = runtime
            .create_project("MCP010B canonical hash project", json!({"profile":"mvp"}))
            .expect("project");
        let draft = json!({
            "schema_version":"GeometryProgram@2",
            "project_id":project.project_id.clone(),
            "representation_plan_sha256":"c".repeat(64),
            "operator_catalog_sha256":operator_catalog_sha256(),
            "units":{"length":"meter","angle":"radian","coordinate_system":"right-handed-y-up"},
            "budgets":{"max_nodes":1,"max_triangles":1000,"max_glb_bytes":1048576,"max_worker_memory_bytes":536870912,"max_runtime_ms":10000},
            "nodes":[{
                "node_id":"shell",
                "operator_id":"forgecad.geometry.primitive@2",
                "inputs":[],
                "parameters":{"shape":"box","size_m":[1.2,1.6,0.55],"position_m":[0.0,1.7,0.0],"rotation_rad":[0.0,0.0,0.0]}
            }],
            "part_outputs":[{
                "part_id":"shell",
                "input_node_ids":["shell"],
                "material_zone_id":"zone-white-shell",
                "solid":true
            }]
        });
        let before = serde_json::to_value(json!({
            "projects":runtime.projects().expect("projects"),
            "candidates":runtime.candidates(&project.project_id).expect("candidates"),
            "versions":runtime.versions(Some(&project.project_id)).expect("versions")
        }))
        .expect("state is serializable");
        let request = json!({
            "schema_version":"GeometryProgramHashRequest@1",
            "geometry_program_draft":draft
        });
        let result = runtime.geometry_program_hash(&request).expect("draft hash");
        assert_eq!(result["schema_version"], "GeometryProgramHashResult@1");
        assert_eq!(
            result["geometry_program_schema_version"],
            "GeometryProgram@2"
        );
        assert_eq!(result["operator_catalog_sha256"], operator_catalog_sha256());
        assert_eq!(result["validation_status"], "passed");
        assert!(result["canonical_sha256"]
            .as_str()
            .is_some_and(forgecad_contracts::is_sha256));
        assert_eq!(
            runtime
                .dispatch_ipc("geometry_program_hash", &request)
                .expect("IPC hash dispatch"),
            result
        );
        let after = serde_json::to_value(json!({
            "projects":runtime.projects().expect("projects"),
            "candidates":runtime.candidates(&project.project_id).expect("candidates"),
            "versions":runtime.versions(Some(&project.project_id)).expect("versions")
        }))
        .expect("state is serializable");
        assert_eq!(before, after, "hashing must not persist Runtime state");

        let mut program = request["geometry_program_draft"].clone();
        program["canonical_sha256"] = result["canonical_sha256"].clone();
        let prepared = runtime
            .prepare_geometry_candidate(
                &project.project_id,
                None,
                json!({"typed":"geometry","geometry_program":program}),
            )
            .expect("compiler accepts Runtime-owned canonical hash");
        assert_eq!(prepared["schema_version"], "GeometryPrepareResult@2");
    }

    #[test]
    fn geometry_program_hash_rejects_noncanonical_or_invalid_requests() {
        let runtime = Runtime::ephemeral().expect("runtime");
        let project = runtime
            .create_project("MCP010B hash negative project", json!({"profile":"mvp"}))
            .expect("project");
        let draft = json!({
            "schema_version":"GeometryProgram@2",
            "project_id":project.project_id.clone(),
            "representation_plan_sha256":"d".repeat(64),
            "operator_catalog_sha256":operator_catalog_sha256(),
            "units":{"length":"meter","angle":"radian","coordinate_system":"right-handed-y-up"},
            "budgets":{"max_nodes":1,"max_triangles":1000,"max_glb_bytes":1048576,"max_worker_memory_bytes":536870912,"max_runtime_ms":10000},
            "nodes":[{
                "node_id":"solid",
                "operator_id":"forgecad.geometry.primitive@2",
                "inputs":[],
                "parameters":{"shape":"sphere","radius_m":0.4,"longitude_segments":16,"latitude_segments":8,"position_m":[0.0,0.0,0.0],"rotation_rad":[0.0,0.0,0.0]}
            }],
            "part_outputs":[{"part_id":"solid","input_node_ids":["solid"],"material_zone_id":"zone-white-shell","solid":true}]
        });
        let valid_request = json!({
            "schema_version":"GeometryProgramHashRequest@1",
            "geometry_program_draft":draft
        });
        let mut prefilled = valid_request.clone();
        prefilled["geometry_program_draft"]["canonical_sha256"] = json!("0".repeat(64));
        let mut catalog_mismatch = valid_request.clone();
        catalog_mismatch["geometry_program_draft"]["operator_catalog_sha256"] =
            json!("0".repeat(64));
        let mut unknown_draft_field = valid_request.clone();
        unknown_draft_field["geometry_program_draft"]["untrusted_extension"] = json!(true);
        for invalid in [
            json!({"schema_version":"GeometryProgramHashRequest@0","geometry_program_draft":valid_request["geometry_program_draft"]}),
            json!({"schema_version":"GeometryProgramHashRequest@1","geometry_program_draft":valid_request["geometry_program_draft"],"untrusted_extension":true}),
            prefilled,
            catalog_mismatch,
            unknown_draft_field,
        ] {
            let error = runtime
                .geometry_program_hash(&invalid)
                .expect_err("invalid hash request must fail closed");
            assert!(error
                .to_string()
                .starts_with("invalid runtime input: GEOMETRY_PROGRAM_HASH_REJECTED:"));
        }
    }

    #[test]
    fn v2_confirmation_revalidates_cas_bytes_and_rejects_corruption() {
        let runtime = Runtime::ephemeral().expect("runtime");
        let project = runtime
            .create_project("MCP010B confirmation project", json!({"profile":"mvp"}))
            .expect("project");
        let mut program = json!({
            "schema_version":"GeometryProgram@2",
            "project_id":project.project_id.clone(),
            "representation_plan_sha256":"e".repeat(64),
            "operator_catalog_sha256":operator_catalog_sha256(),
            "units":{"length":"meter","angle":"radian","coordinate_system":"right-handed-y-up"},
            "budgets":{"max_nodes":1,"max_triangles":1000,"max_glb_bytes":1048576,"max_worker_memory_bytes":536870912,"max_runtime_ms":10000},
            "nodes":[{"node_id":"solid","operator_id":"forgecad.geometry.primitive@2","inputs":[],"parameters":{"shape":"sphere","radius_m":0.4,"longitude_segments":16,"latitude_segments":8,"position_m":[0.0,0.0,0.0],"rotation_rad":[0.0,0.0,0.0]}}],
            "part_outputs":[{"part_id":"solid","input_node_ids":["solid"],"material_zone_id":"zone-white-shell","solid":true}]
        });
        program["canonical_sha256"] = Value::String(canonical_json_hash(&program));
        let valid =
            forgecad_geometry_worker::compile_geometry_program(&program).expect("valid V2 GLB");
        let mut corrupt = valid.glb;
        let json_length =
            u32::from_le_bytes(corrupt[12..16].try_into().expect("json length")) as usize;
        let root: Value = serde_json::from_slice(&corrupt[20..20 + json_length]).expect("root");
        let primitive = &root["meshes"][0]["primitives"][0];
        let index_accessor = primitive["indices"].as_u64().expect("index accessor") as usize;
        let view_index = root["accessors"][index_accessor]["bufferView"]
            .as_u64()
            .expect("view") as usize;
        let offset = root["bufferViews"][view_index]["byteOffset"]
            .as_u64()
            .unwrap_or(0) as usize;
        let bin_offset = 20 + json_length + 8;
        corrupt[bin_offset + offset..bin_offset + offset + 4]
            .copy_from_slice(&u32::MAX.to_le_bytes());
        let corrupt_object = runtime
            .put_object(&corrupt, None, "model/gltf-binary", "geometry-glb")
            .expect("corrupt test object");
        let prepared = runtime
            .prepare_candidate(
                &project.project_id,
                None,
                "corrupt-v2-glb",
                &corrupt_object.record.sha256,
                json!({"typed":"geometry-test"}),
            )
            .expect("prepared candidate");
        let candidate = runtime
            .mark_candidate_quality(&prepared.candidate.candidate_id, "quality-corrupt-v2", true)
            .expect("quality mark");
        let error = runtime
            .confirm_candidate(&CandidateConfirmRequest {
                project_id: project.project_id.clone(),
                candidate_id: candidate.candidate_id.clone(),
                base_version_id: None,
                prepared_object_id: candidate.prepared_object_id.clone().expect("object id"),
                prepared_object_sha256: corrupt_object.record.sha256,
                quality_report_id: "quality-corrupt-v2".to_owned(),
                approval_receipt_id: "mcp010b-corrupt-approval".to_owned(),
                approval_summary: "Reject corrupt V2 geometry".to_owned(),
                approval_session_id: "mcp010b-corrupt-session".to_owned(),
                approval_expires_at: "9999999999".to_owned(),
                idempotency_key: "mcp010b-corrupt-once".to_owned(),
            })
            .expect_err("corrupt V2 GLB cannot confirm");
        assert!(error.to_string().contains("QUALITY_HARD_GATE_FAILED"));
        assert!(runtime
            .versions(Some(&project.project_id))
            .expect("versions")
            .is_empty());
    }

    #[test]
    fn operator_catalog_resource_matches_live_runtime_capabilities() {
        let runtime = Runtime::ephemeral().expect("runtime");
        let descriptors = runtime.resource_descriptors().expect("resources");
        assert!(descriptors
            .iter()
            .any(|resource| resource.uri == "forgecad://operators/catalog" && resource.read_only));
        let contents = runtime
            .read_resource("forgecad://operators/catalog")
            .expect("operator catalog resource");
        let catalog: Value = serde_json::from_str(&contents.text).expect("catalog JSON");
        assert_eq!(catalog["schema_version"], "OperatorCatalog@1");
        assert_eq!(catalog, runtime.active_operator_catalog());
        assert_eq!(
            runtime
                .dispatch_ipc("operator_catalog_get", &json!({}))
                .expect("operator catalog IPC dispatch"),
            catalog
        );
        assert_eq!(
            catalog["canonical_sha256"],
            runtime
                .capabilities()
                .operator_catalog_sha256
                .as_deref()
                .expect("capability catalog hash")
        );
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

        let mut cross_project_program = json!({
            "schema_version":"GeometryProgram@1",
            "project_id":"project-other",
            "representation_plan_sha256":"b".repeat(64),
            "nodes":[{"node_id":"part","operator_id":"forgecad.geometry.primitive@1","part_id":"part","parameters":{"shape":"box","size":[1.0,1.0,1.0],"position":[0.0,0.0,0.0],"material_zone_id":"zone-white-shell"}}],
            "budgets":{"max_nodes":1,"max_triangles":100,"max_runtime_ms":1000}
        });
        cross_project_program["canonical_sha256"] =
            Value::String(canonical_json_hash(&cross_project_program));
        let error = runtime
            .prepare_geometry_candidate(
                &project.project_id,
                None,
                json!({"typed":"geometry","geometry_program":cross_project_program}),
            )
            .expect_err("cross-project geometry program");
        assert!(error.to_string().contains("project_id must match"));
        assert!(runtime
            .candidates(&project.project_id)
            .expect("candidates")
            .is_empty());
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
        assert!(result["render_set_object_sha256"]
            .as_str()
            .is_some_and(forgecad_contracts::is_sha256));
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
        let object_hash = candidate["prepared_object_sha256"]
            .as_str()
            .unwrap()
            .to_owned();
        let quality_id = candidate["quality_report_id"].as_str().unwrap().to_owned();
        let confirmed = runtime
            .confirm_candidate(&CandidateConfirmRequest {
                project_id: project.project_id.clone(),
                candidate_id: candidate_id.clone(),
                base_version_id: None,
                prepared_object_id: object_id,
                prepared_object_sha256: object_hash,
                quality_report_id: quality_id,
                approval_receipt_id: "mcp009-user-approval".to_owned(),
                approval_summary: "Approve MVP appearance candidate".to_owned(),
                approval_session_id: "mcp009-session".to_owned(),
                approval_expires_at: "9999999999".to_owned(),
                idempotency_key: "mcp009-confirm-once".to_owned(),
            })
            .expect("confirm");
        let report = runtime.quality(&candidate_id, None).expect("quality");
        assert_eq!(report["hard_gate_passed"], true);
        let export = runtime
            .prepare_export(&ExportPrepareRequest {
                project_id: project.project_id.clone(),
                version_id: confirmed.version_id.clone(),
                format: "glb".to_owned(),
                profile: "mvp-glb".to_owned(),
                request: json!({"reason":"MVP GLB export"}),
            })
            .expect("glb export prepare");
        let output = runtime
            .confirm_export(&ExportConfirmRequest {
                project_id: project.project_id.clone(),
                export_id: export.manifest.export_id.clone(),
                version_id: confirmed.version_id.clone(),
                format: "glb".to_owned(),
                profile: "mvp-glb".to_owned(),
                approval_receipt_id: "mcp009-export-approval".to_owned(),
                approval_summary: "Approve MVP GLB export".to_owned(),
                approval_session_id: "mcp009-session".to_owned(),
                approval_expires_at: "9999999999".to_owned(),
                idempotency_key: "mcp009-export-once".to_owned(),
            })
            .expect("glb export confirm");
        assert_eq!(output.output_sha256, export.manifest.artifact_hashes[0]);
        let diff = runtime
            .version_diff(&confirmed.version_id, &confirmed.version_id)
            .expect("version diff");
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
                project_id: project.project_id.clone(),
                candidate_id: first_candidate["candidate_id"].as_str().unwrap().to_owned(),
                base_version_id: None,
                prepared_object_id: first_candidate["prepared_object_id"]
                    .as_str()
                    .unwrap()
                    .to_owned(),
                prepared_object_sha256: first_candidate["prepared_object_sha256"]
                    .as_str()
                    .unwrap()
                    .to_owned(),
                quality_report_id: first_candidate["quality_report_id"]
                    .as_str()
                    .unwrap()
                    .to_owned(),
                approval_receipt_id: "mcp009-change-first".to_owned(),
                approval_summary: "Approve first model".to_owned(),
                approval_session_id: "mcp009-change-session".to_owned(),
                approval_expires_at: "9999999999".to_owned(),
                idempotency_key: "mcp009-change-first-once".to_owned(),
            })
            .expect("first confirm");
        geometry["nodes"][0]["parameters"]["size"][0] = json!(1.15);
        geometry["canonical_sha256"] = Value::String(canonical_json_hash(
            &json!({"schema_version":geometry["schema_version"].clone(),"project_id":geometry["project_id"].clone(),"representation_plan_sha256":geometry["representation_plan_sha256"].clone(),"nodes":geometry["nodes"].clone(),"budgets":geometry["budgets"].clone()}),
        ));
        appearance["geometry_program_sha256"] = geometry["canonical_sha256"].clone();
        appearance["canonical_sha256"] = Value::String(canonical_json_hash(
            &json!({"schema_version":appearance["schema_version"].clone(),"project_id":appearance["project_id"].clone(),"geometry_program_sha256":appearance["geometry_program_sha256"].clone(),"material_zones":appearance["material_zones"].clone()}),
        ));
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
        assert_eq!(
            runtime.versions(Some(&project.project_id)).unwrap().len(),
            1
        );
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
        let imported = runtime
            .import_reference(&request)
            .expect("reference import");
        assert_eq!(imported.reference.mime, "image/png");
        assert_eq!(imported.reference.width, 1);
        assert_eq!(imported.reference.height, 1);
        assert_eq!(imported.reference.frame_count, 1);
        assert_eq!(imported.reference.import_mode, "inline_content");
        assert!(forgecad_contracts::is_sha256(
            &imported.reference.object_sha256
        ));
        assert!(forgecad_contracts::is_sha256(
            &imported.reference.canonical_sha256
        ));
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
    fn viewer_reference_bytes_are_hash_bound_and_project_scoped() {
        let runtime = Runtime::ephemeral().expect("runtime");
        let project = runtime
            .create_project("Viewer reference bytes", json!({"profile":"mvp"}))
            .expect("project");
        let other_project = runtime
            .create_project("Other project", json!({"profile":"mvp"}))
            .expect("other project");
        let reference = runtime
            .import_reference(&ReferenceImportRequest {
                project_id: project.project_id.clone(),
                source: ReferenceImportSource::InlineContent {
                    mime: "image/png".to_owned(),
                    content_base64: "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=".to_owned(),
                },
                authorization: ReferenceAuthorization {
                    user_authorized: true,
                    declaration: "viewer read test".to_owned(),
                },
                expected_sha256: None,
            })
            .expect("reference")
            .reference;
        let payload = runtime
            .reference_bytes(&reference.reference_id, &project.project_id)
            .expect("reference bytes");
        assert_eq!(payload["schema_version"], "ReferenceBytesRead@1");
        assert_eq!(payload["sha256"], reference.object_sha256);
        assert_eq!(payload["size_bytes"], reference.size_bytes);
        assert!(payload["bytes_base64"]
            .as_str()
            .is_some_and(|value| !value.is_empty()));
        assert!(runtime
            .reference_bytes(&reference.reference_id, &other_project.project_id)
            .is_err());
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
        let error = runtime
            .import_reference(&request)
            .expect_err("MIME mismatch");
        assert!(error.to_string().contains("REFERENCE_REJECTED"));
        request.authorization.user_authorized = false;
        request.source = ReferenceImportSource::InlineContent {
            mime: "image/png".to_owned(),
            content_base64: "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=".to_owned(),
        };
        let error = runtime
            .import_reference(&request)
            .expect_err("authorization");
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
        let request =
            |content_base64: String, expected_sha256: Option<String>| ReferenceImportRequest {
                project_id: project.project_id.clone(),
                source: ReferenceImportSource::InlineContent {
                    mime: "image/png".to_owned(),
                    content_base64,
                },
                authorization: authorized.clone(),
                expected_sha256,
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
        assert!(error
            .to_string()
            .contains("CAS expected hash does not match content"));
    }

    #[cfg(unix)]
    #[test]
    fn local_file_reference_requires_authorized_root_and_rejects_symlink() {
        use std::os::unix::fs::symlink;
        let root = std::env::temp_dir().join(format!("forgecad-mcp005-path-{}", Uuid::new_v4()));
        let outside =
            std::env::temp_dir().join(format!("forgecad-mcp005-outside-{}", Uuid::new_v4()));
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
            .import_reference(&request(
                root.join("source.png").to_string_lossy().into_owned(),
            ))
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
