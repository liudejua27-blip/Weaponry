mod geometry_worker;
mod render_worker;
mod ipc;
mod agentic_design;
mod agentic_action;
mod agentic_session;
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
use forgecad_worker_protocol::{
    material_pack_manifest, operator_catalog, operator_catalog_sha256,
};
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
use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
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

// These are the product-level visible-view gates.  The earlier 0.72/0.75
// values were useful for exploratory comparison receipts, but were too weak
// to unlock downstream detail/material work.  Keep the thresholds in one
// Runtime-owned place so emitted reports and their validators cannot drift.
const VISIBLE_SILHOUETTE_IOU_MIN: f64 = 0.90;
const VISIBLE_BOUNDARY_F1_MIN: f64 = 0.90;
const VISIBLE_BBOX_EDGE_ERROR_MAX: f64 = 0.02;
const VISIBLE_CENTROID_ERROR_MAX: f64 = 0.02;
const VISIBLE_LANDMARK_COVERAGE_MIN: f64 = 0.80;
const VISIBLE_LANDMARK_NME_MAX: f64 = 0.03;
const VISIBLE_REGION_MEDIAN_IOU_MIN: f64 = 0.85;
const VISIBLE_CRITICAL_REGION_IOU_MIN: f64 = 0.85;
// A full 37+27 camera neighborhood is useful for offline research, but it is
// too slow for the product's single MCP request window on the fixed Worker.
// Keep the Runtime path deterministic and auditable with eight coarse probes;
// the probes are a balanced yaw/pitch/framing spread rather than an arbitrary
// prefix of the 37-row research list.
const CAMERA_FIT_RUNTIME_MAX_EVALUATIONS: usize = 8;
// The coarse camera search is intentionally low resolution, but the selected
// calibration is later bound to the 512x512 comparison/render set. Re-rank a
// small, deterministic top-k (plus the authored base camera) at the fixed
// resolution before exposing a CameraCalibrationRef. This prevents a
// low-resolution aliasing or landmark tie from becoming a durable camera
// binding while keeping the request bounded.
const CAMERA_FIT_FULL_RESOLUTION_MAX_EVALUATIONS: usize = 5;
// A Primary Form repair is one MCP action, so its nested Runtime-owned
// search stays within the same fixed 64-evaluation ceiling as the standalone
// fit. The action remains bounded and keeps continuous parameter search in
// Runtime/Workers rather than pushing it back into Codex.
const PRIMARY_FORM_REPAIR_MAX_EVALUATIONS: u64 = 64;
const PRIMARY_FORM_REPAIR_MAX_ITERATIONS: u64 = 1;

fn normalize_primary_form_repair_optimizer(optimizer: &mut serde_json::Map<String, Value>) {
    let requested_evaluations = optimizer
        .get("max_evaluations")
        .and_then(Value::as_u64)
        .unwrap_or(PRIMARY_FORM_REPAIR_MAX_EVALUATIONS);
    let requested_iterations = optimizer
        .get("max_iterations")
        .and_then(Value::as_u64)
        .unwrap_or(PRIMARY_FORM_REPAIR_MAX_ITERATIONS);
    optimizer.insert(
        "max_evaluations".to_owned(),
        Value::from(requested_evaluations.min(PRIMARY_FORM_REPAIR_MAX_EVALUATIONS)),
    );
    optimizer.insert(
        "max_iterations".to_owned(),
        Value::from(requested_iterations.min(PRIMARY_FORM_REPAIR_MAX_ITERATIONS)),
    );
}

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
    // CameraFitResult is deliberately cached only for the lifetime of this
    // Runtime process.  It is an optimisation for the immediately-following
    // CameraCalibrationRef -> SilhouetteFit handoff, never a source of truth:
    // the full result remains recomputable from the candidate/target hashes.
    camera_fit_cache: Mutex<HashMap<String, Value>>,
    _process_lock: Option<process_lock::ProcessLock>,
}

fn camera_fit_cache_key(project_id: &str, candidate_id: &str, target_sha256: &str) -> String {
    format!("{project_id}\n{candidate_id}\n{target_sha256}")
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
            camera_fit_cache: Mutex::new(HashMap::new()),
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
            camera_fit_cache: Mutex::new(HashMap::new()),
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

    /// Return the immutable, offline first-party MaterialPack manifest. The
    /// manifest is compiled into the Worker/Runtime cohort and contains only
    /// source/license/hash/provenance metadata; no URL is fetched at runtime.
    pub fn material_pack_manifest(&self) -> Value {
        material_pack_manifest()
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

    /// Return the canonical hash for a hash-free SilhouetteRig draft.  Luna
    /// should use this read-only Runtime-owned helper instead of reimplementing
    /// canonical JSON hashing in a prompt, script or local client.
    pub fn silhouette_rig_hash(
        &self,
        project_id: &str,
        request: &Value,
    ) -> Result<Value, RuntimeError> {
        validate_id(project_id)?;
        let object = request.as_object().ok_or_else(|| {
            RuntimeError::InvalidInput(
                "SILHOUETTE_RIG_HASH_INVALID: request must be an object".to_owned(),
            )
        })?;
        validate_request_keys(
            object,
            &["schema_version", "project_id", "candidate_id", "rig_draft"],
            "silhouette_rig_hash",
        )?;
        if object.get("schema_version").and_then(Value::as_str)
            != Some("SilhouetteRigHashRequest@1")
            || object.get("project_id").and_then(Value::as_str) != Some(project_id)
        {
            return Err(RuntimeError::InvalidInput(
                "SILHOUETTE_RIG_HASH_INVALID: schema or project binding".to_owned(),
            ));
        }
        let candidate_id = required_value_id(object.get("candidate_id"), "candidate_id")?;
        let candidate = self.candidate(candidate_id)?.ok_or_else(|| {
            RuntimeError::InvalidInput("NOT_FOUND: candidate not found".to_owned())
        })?;
        if candidate.project_id != project_id {
            return Err(RuntimeError::InvalidInput(
                "PROJECT_SCOPE_DENIED: candidate is outside the target project".to_owned(),
            ));
        }
        let draft = object.get("rig_draft").ok_or_else(|| {
            RuntimeError::InvalidInput(
                "SILHOUETTE_RIG_HASH_INVALID: rig_draft is required".to_owned(),
            )
        })?;
        let draft_object = draft.as_object().ok_or_else(|| {
            RuntimeError::InvalidInput(
                "SILHOUETTE_RIG_HASH_INVALID: rig_draft must be an object".to_owned(),
            )
        })?;
        if draft_object.contains_key("canonical_sha256") {
            return Err(RuntimeError::InvalidInput(
                "SILHOUETTE_RIG_HASH_INVALID: rig_draft must omit canonical_sha256".to_owned(),
            ));
        }
        let mut rig = draft.clone();
        rig["canonical_sha256"] = Value::String(String::new());
        let canonical_sha256 = canonical_json_hash(&rig);
        rig["canonical_sha256"] = Value::String(canonical_sha256.clone());
        validate_silhouette_rig(&rig, candidate_id)?;
        let result = json!({
            "schema_version":"SilhouetteRigHashResult@1",
            "silhouette_rig_schema_version":"SilhouetteRig@1",
            "canonical_sha256":canonical_sha256,
            "validation_status":"passed"
        });
        validate_silhouette_rig_hash_result_output(&result)?;
        Ok(result)
    }

    /// Materialize the reference silhouette as an immutable, hash-bound
    /// target.  A Codex contour, when supplied, is only a normalized polygon
    /// over the user-authorized reference; it never becomes geometry by
    /// itself.  The target and mask are stored in CAS so every later camera,
    /// comparison and correction call can bind to the same bytes.
    pub fn prepare_reference_mask(
        &self,
        project_id: &str,
        request: Value,
    ) -> Result<Value, RuntimeError> {
        validate_id(project_id)?;
        let object = request.as_object().ok_or_else(|| {
            RuntimeError::InvalidInput("REFERENCE_MASK_INVALID: request must be an object".to_owned())
        })?;
        validate_request_keys(object, &["project_id", "reference_id", "contour_points", "landmarks", "parts"], "reference_mask_prepare")?;
        if object.get("project_id").and_then(Value::as_str) != Some(project_id) {
            return Err(RuntimeError::InvalidInput(
                "PROJECT_SCOPE_DENIED: reference mask project differs".to_owned(),
            ));
        }
        let reference_id = required_value_id(object.get("reference_id"), "reference_id")?;
        let reference = self.reference(reference_id)?.ok_or_else(|| {
            RuntimeError::InvalidInput("NOT_FOUND: reference not found".to_owned())
        })?;
        if reference.project_id != project_id {
            return Err(RuntimeError::InvalidInput(
                "REFERENCE_SCOPE_DENIED: reference is outside the target project".to_owned(),
            ));
        }
        let contour_points = parse_contour_points(object.get("contour_points"))?;
        let landmarks = parse_target_landmarks(object.get("landmarks"))?;
        let parts = parse_target_parts(object.get("parts"))?;
        let automatic = reference_mask_png(&self.cas_read(&reference.object_sha256)?)?;
        self.store_silhouette_target(
            project_id,
            &reference,
            contour_points.as_deref(),
            landmarks,
            parts,
            automatic,
            contour_points.is_none(),
        )
    }

    /// Create a new target from a previous target and a corrected polygon.
    /// The old CAS objects remain immutable and usable for comparison.
    pub fn refine_reference_mask(
        &self,
        project_id: &str,
        request: Value,
    ) -> Result<Value, RuntimeError> {
        validate_id(project_id)?;
        let object = request.as_object().ok_or_else(|| {
            RuntimeError::InvalidInput("REFERENCE_MASK_INVALID: request must be an object".to_owned())
        })?;
        validate_request_keys(object, &["project_id", "base_target_sha256", "contour_points", "landmarks", "parts"], "reference_mask_refine_prepare")?;
        if object.get("project_id").and_then(Value::as_str) != Some(project_id) {
            return Err(RuntimeError::InvalidInput(
                "PROJECT_SCOPE_DENIED: reference mask project differs".to_owned(),
            ));
        }
        let base_sha = required_value_sha(object.get("base_target_sha256"), "base_target_sha256")?;
        let base = self.read_silhouette_target(base_sha)?;
        let reference_id = required_value_id(base.get("reference_id"), "reference_id")?;
        let reference = self.reference(reference_id)?.ok_or_else(|| {
            RuntimeError::InvalidInput("NOT_FOUND: reference not found".to_owned())
        })?;
        if reference.project_id != project_id {
            return Err(RuntimeError::InvalidInput(
                "REFERENCE_SCOPE_DENIED: base target is outside the target project".to_owned(),
            ));
        }
        let contour_points = parse_contour_points(object.get("contour_points"))?.ok_or_else(|| {
            RuntimeError::InvalidInput(
                "REFERENCE_MASK_INVALID: contour_points are required for refinement".to_owned(),
            )
        })?;
        let landmarks = match object.get("landmarks") {
            Some(value) => parse_target_landmarks(Some(value))?,
            None => base.get("landmarks").cloned().unwrap_or_else(|| json!([])),
        };
        let parts = match object.get("parts") {
            Some(value) => parse_target_parts(Some(value))?,
            None => base.get("parts").cloned().unwrap_or_else(|| json!([])),
        };
        let automatic = reference_mask_png(&self.cas_read(&reference.object_sha256)?)?;
        self.store_silhouette_target(
            project_id,
            &reference,
            Some(&contour_points),
            landmarks,
            parts,
            automatic,
            false,
        )
    }

    pub fn silhouette_target_get(&self, target_sha256: &str) -> Result<Value, RuntimeError> {
        let target = self.read_silhouette_target(target_sha256)?;
        Ok(target)
    }

    /// Search only a small deterministic camera neighborhood.  This is an
    /// alignment aid, not a hidden model optimizer: it renders the existing
    /// candidate and returns camera evidence without mutating the candidate.
    pub fn prepare_camera_fit(
        &self,
        project_id: &str,
        request: Value,
    ) -> Result<Value, RuntimeError> {
        validate_id(project_id)?;
        let object = request.as_object().ok_or_else(|| {
            RuntimeError::InvalidInput("CAMERA_FIT_INVALID: request must be an object".to_owned())
        })?;
        validate_request_keys(object, &["project_id", "candidate_id", "target_sha256", "camera"], "camera_fit_prepare")?;
        if object.get("project_id").and_then(Value::as_str) != Some(project_id) {
            return Err(RuntimeError::InvalidInput(
                "PROJECT_SCOPE_DENIED: camera fit project differs".to_owned(),
            ));
        }
        let candidate_id = required_value_id(object.get("candidate_id"), "candidate_id")?;
        let target_sha256 = required_value_sha(object.get("target_sha256"), "target_sha256")?;
        let target = self.read_silhouette_target(target_sha256)?;
        let target_reference = required_value_id(target.get("reference_id"), "reference_id")?;
        let reference = self.reference(target_reference)?.ok_or_else(|| {
            RuntimeError::InvalidInput("NOT_FOUND: target reference not found".to_owned())
        })?;
        let candidate = self.candidate(candidate_id)?.ok_or_else(|| {
            RuntimeError::InvalidInput("NOT_FOUND: candidate not found".to_owned())
        })?;
        if candidate.project_id != project_id || reference.project_id != project_id {
            return Err(RuntimeError::InvalidInput(
                "PROJECT_SCOPE_DENIED: camera fit input is outside the target project".to_owned(),
            ));
        }
        let artifact_sha256 = candidate
            .manifest_hash
            .clone()
            .or(candidate.prepared_object_sha256.clone())
            .ok_or_else(|| RuntimeError::InvalidInput("CANDIDATE_ARTIFACT_UNAVAILABLE".to_owned()))?;
        let glb = self.cas_read(&artifact_sha256)?;
        let inspection = strict_glb_inspection(&glb)?;
        if !inspection.hard_gate_passed {
            return Err(RuntimeError::InvalidInput(format!(
                "CAMERA_FIT_REJECTED: strict GLB readback failed: {}",
                inspection.failure_codes.join(",")
            )));
        }
        let reference_mask = self.target_mask(target_sha256, &target)?;
        let base_camera = object
            .get("camera")
            .filter(|value| !value.is_null())
            .cloned()
            .unwrap_or_else(default_camera_calibration);
        validate_camera_calibration(&base_camera)?;
        let all_coarse_variants = camera_fit_search_variants(&base_camera);
        // Prefer a spread of the deterministic yaw/pitch grid and retain the
        // base/offset probes. The complete 37-row list remains available to
        // offline tests; the product IPC path deliberately evaluates only a
        // fixed eight-row subset so the request cannot monopolise Runtime.
        // Reserve two slots for Runtime-owned framing candidates derived from
        // the authored base camera. Those candidates preserve the historical
        // height/extent calibration path without letting Codex search camera
        // parameters continuously.
        let coarse_indices = [0_usize, 5, 23, 11, 17, 13];
        let mut coarse_variants = coarse_indices
            .into_iter()
            .filter_map(|index| all_coarse_variants.get(index).cloned())
            .collect::<Vec<_>>();
        let base_full_passes = render_worker::render_glb_fit_batch_at_resolution(
            &glb,
            std::slice::from_ref(&base_camera),
            512,
        )
        .map_err(|error| {
            RuntimeError::InvalidInput(format!(
                "CAMERA_FIT_RENDER_FAILED: base framing verification failed: {error}"
            ))
        })?;
        let base_full_silhouette = base_full_passes
            .first()
            .and_then(|passes| passes.iter().find(|pass| pass.pass == "silhouette"))
            .ok_or_else(|| {
                RuntimeError::InvalidInput(
                    "CAMERA_FIT_RENDER_FAILED: base framing silhouette missing".to_owned(),
                )
            })?;
        let base_full_model_mask = decode_binary_mask(&base_full_silhouette.png).map_err(|error| {
            RuntimeError::InvalidInput(format!(
                "CAMERA_FIT_RENDER_FAILED: base framing silhouette decode failed: {error}"
            ))
        })?;
        for framing_camera in [
            calibrate_default_camera_height_only(
                &base_camera,
                &reference_mask.mask,
                &base_full_model_mask,
            ),
            calibrate_default_camera(
                &base_camera,
                &reference_mask.mask,
                &base_full_model_mask,
            ),
        ] {
            if framing_camera != base_camera && !coarse_variants.iter().any(|camera| camera == &framing_camera) {
                coarse_variants.push(framing_camera);
            }
        }
        let mut rows = Vec::with_capacity(CAMERA_FIT_RUNTIME_MAX_EVALUATIONS);
        let coarse_passes = render_glb_fit_batch_with_runtime_worker(&glb, &coarse_variants)
            .map_err(|error| {
                RuntimeError::InvalidInput(format!("CAMERA_FIT_RENDER_FAILED: {error}"))
            })?;
        if coarse_passes.len() != coarse_variants.len() {
            return Err(RuntimeError::InvalidInput(
                "CAMERA_FIT_RENDER_FAILED: coarse batch result count mismatch".to_owned(),
            ));
        }
        for (camera, passes) in coarse_variants.into_iter().zip(coarse_passes) {
            rows.push(camera_fit_row_from_passes(
                &reference_mask.mask,
                target.get("landmarks"),
                camera,
                &passes,
                &inspection.part_ids,
            )?);
        }
        rows.sort_by(|left, right| {
            left["loss"]
                .as_f64()
                .unwrap_or(f64::INFINITY)
                .partial_cmp(&right["loss"].as_f64().unwrap_or(f64::INFINITY))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let seeds = rows
            .iter()
            .take(3)
            .filter_map(|row| row.get("camera").cloned())
            .collect::<Vec<_>>();
        let mut refinement_cameras = Vec::new();
        for seed in seeds {
            for camera in camera_fit_refinement_variants(&seed) {
                if refinement_cameras.len() + rows.len() >= CAMERA_FIT_RUNTIME_MAX_EVALUATIONS {
                    break;
                }
                refinement_cameras.push(camera);
            }
            if refinement_cameras.len() + rows.len() >= CAMERA_FIT_RUNTIME_MAX_EVALUATIONS {
                break;
            }
        }
        if !refinement_cameras.is_empty() {
            let refinement_passes = render_glb_fit_batch_with_runtime_worker(
                &glb,
                &refinement_cameras,
            )
            .map_err(|error| {
                RuntimeError::InvalidInput(format!("CAMERA_FIT_RENDER_FAILED: {error}"))
            })?;
            if refinement_passes.len() != refinement_cameras.len() {
                return Err(RuntimeError::InvalidInput(
                    "CAMERA_FIT_RENDER_FAILED: refinement batch result count mismatch".to_owned(),
                ));
            }
            for (camera, passes) in refinement_cameras.into_iter().zip(refinement_passes) {
                rows.push(camera_fit_row_from_passes(
                    &reference_mask.mask,
                    target.get("landmarks"),
                    camera,
                    &passes,
                    &inspection.part_ids,
                )?);
            }
        }
        // The transient 128px ranking is only a bounded accelerator. The
        // selected camera is consumed by the 512px comparison path, so verify
        // the best coarse rows at that same fixed resolution before choosing
        // the Runtime-owned binding. Include the authored base camera even
        // when it did not make the coarse top-k; it is the fail-safe framing
        // fallback and must remain comparable to every proposed camera.
        rows.sort_by(|left, right| {
            left["loss"]
                .as_f64()
                .unwrap_or(f64::INFINITY)
                .partial_cmp(&right["loss"].as_f64().unwrap_or(f64::INFINITY))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let base_camera_index = rows
            .iter()
            .position(|row| row.get("camera") == Some(&base_camera));
        let mut full_resolution_indices = rows
            .iter()
            .take(CAMERA_FIT_FULL_RESOLUTION_MAX_EVALUATIONS)
            .enumerate()
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        if let Some(index) = base_camera_index {
            if !full_resolution_indices.contains(&index) {
                if full_resolution_indices.len() >= CAMERA_FIT_FULL_RESOLUTION_MAX_EVALUATIONS {
                    full_resolution_indices.pop();
                }
                full_resolution_indices.push(index);
            }
        }
        full_resolution_indices.sort_unstable();
        let full_resolution_cameras = full_resolution_indices
            .iter()
            .map(|index| {
                rows.get(*index)
                    .and_then(|row| row.get("camera"))
                    .cloned()
                    .ok_or_else(|| RuntimeError::InvalidInput("CAMERA_FIT_RENDER_FAILED: full-resolution row missing".to_owned()))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let full_resolution_passes = render_worker::render_glb_fit_batch_at_resolution(
            &glb,
            &full_resolution_cameras,
            512,
        )
        .map_err(|error| {
            RuntimeError::InvalidInput(format!("CAMERA_FIT_RENDER_FAILED: full-resolution verification failed: {error}"))
        })?;
        for (index, passes) in full_resolution_indices.into_iter().zip(full_resolution_passes) {
            let silhouette = passes
                .iter()
                .find(|pass| pass.pass == "silhouette")
                .ok_or_else(|| RuntimeError::InvalidInput("CAMERA_FIT_RENDER_FAILED: full-resolution silhouette missing".to_owned()))?;
            let model_mask = decode_binary_mask(&silhouette.png).map_err(|error| {
                RuntimeError::InvalidInput(format!("CAMERA_FIT_RENDER_FAILED: full-resolution silhouette decode failed: {error}"))
            })?;
            let full_metrics = extended_silhouette_metrics(&reference_mask.mask, &model_mask);
            let part_context = passes
                .iter()
                .find(|pass| pass.pass == "part-id")
                .map(|pass| (pass.png.as_slice(), inspection.part_ids.as_slice()));
            let loss_metrics = transient_loss_metrics_with_parts(
                &full_metrics,
                &model_mask,
                target.get("landmarks"),
                part_context,
            );
            let loss = camera_fit_loss(&loss_metrics);
            // CameraFitResult@1 intentionally exposes only the four compact
            // camera metrics. Keep the full-resolution SDF/landmark values in
            // the internal ranking loss, but never widen the public contract.
            let metrics = json!({
                "silhouette_iou": full_metrics["silhouette_iou"],
                "boundary_f1_4px": full_metrics["boundary_f1_4px"],
                "bbox_edge_error": full_metrics["bbox_edge_error"],
                "centroid_error": full_metrics["centroid_error"]
            });
            if let Some(row) = rows.get_mut(index) {
                row["loss"] = Value::from(stable_visual_metric(loss));
                row["metrics"] = metrics;
            }
        }
        // Keep the full deterministic evaluation budget internal, but return
        // only a compact ranked evidence set. Codex tool-result envelopes have
        // a finite payload budget; returning all 64 rows can cause a real
        // client to drop the selected camera even after Runtime completed the
        // search. Four complete calibration rows are enough for recovery and
        // review; preserve the base camera as a comparison row when it is not
        // in that ranked set.
        let base_row = rows
            .iter()
            .find(|row| row.get("camera") == Some(&base_camera))
            .cloned();
        rows.sort_by(|left, right| {
            left["loss"]
                .as_f64()
                .unwrap_or(f64::INFINITY)
                .partial_cmp(&right["loss"].as_f64().unwrap_or(f64::INFINITY))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        rows.truncate(4);
        if let Some(base_row) = base_row {
            if !rows.iter().any(|row| row.get("camera") == base_row.get("camera")) {
                rows.push(base_row);
                rows.sort_by(|left, right| {
                    left["loss"]
                        .as_f64()
                        .unwrap_or(f64::INFINITY)
                        .partial_cmp(&right["loss"].as_f64().unwrap_or(f64::INFINITY))
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
            }
        }
        let selected = rows
            .first()
            .and_then(|row| row.get("camera"))
            .cloned()
            .ok_or_else(|| RuntimeError::InvalidInput("CAMERA_FIT_UNAVAILABLE".to_owned()))?;
        let base_loss = rows
            .iter()
            .find(|row| row.get("camera") == Some(&base_camera))
            .and_then(|row| row.get("loss"))
            .and_then(Value::as_f64)
            .unwrap_or(f64::INFINITY);
        let selected_loss = rows[0]["loss"].as_f64().unwrap_or(f64::INFINITY);
        let mut result = json!({
            "schema_version":"CameraFitResult@1",
            "candidate_id":candidate_id,
            "target_sha256":target_sha256,
            "selected_camera":selected,
            "candidates":rows,
            "status":if selected_loss + 1e-12 < base_loss {"ready"} else {"no_improvement"},
            "canonical_sha256":""
        });
        result["canonical_sha256"] = Value::String(canonical_json_hash(&result));
        validate_camera_fit_result(&result)?;
        // A CameraCalibrationRef is normally consumed immediately by
        // silhouette_fit_prepare. Keep the just-produced full calibration in
        // a small process-local cache so that handoff does not rerun the
        // expensive Worker search. The result is still keyed by all source
        // hashes and can be recomputed after a Runtime restart.
        let cache_key = camera_fit_cache_key(project_id, candidate_id, target_sha256);
        if let Ok(mut cache) = self.camera_fit_cache.lock() {
            cache.insert(cache_key, result.clone());
            if cache.len() > 32 {
                if let Some(key) = cache.keys().next().cloned() {
                    cache.remove(&key);
                }
            }
        }
        Ok(result)
    }

    /// Return directional boundary error segments for Codex's next bounded
    /// edit.  The direction is radial (toward/away from the target centroid),
    /// deliberately conservative until a signed-distance field is added.
    pub fn boundary_error(
        &self,
        candidate_id: &str,
        target_sha256: &str,
        max_segments: Option<u64>,
    ) -> Result<Value, RuntimeError> {
        validate_id(candidate_id)?;
        let target = self.read_silhouette_target(target_sha256)?;
        let evidence = self.store.get_visual_evidence(candidate_id)?.ok_or_else(|| {
            RuntimeError::InvalidInput(
                "BOUNDARY_ERROR_UNAVAILABLE: run reference_compare_prepare first".to_owned(),
            )
        })?;
        let candidate = self.candidate(candidate_id)?.ok_or_else(|| {
            RuntimeError::InvalidInput("NOT_FOUND: candidate not found".to_owned())
        })?;
        if target.get("reference_id").and_then(Value::as_str) != Some(evidence.reference_id.as_str()) {
            return Err(RuntimeError::InvalidInput(
                "REFERENCE_BINDING_MISMATCH: target reference differs from candidate evidence".to_owned(),
            ));
        }
        let render_set: Value = serde_json::from_slice(&self.cas_read(&evidence.render_set_object_sha256)?)
            .map_err(|error| RuntimeError::InvalidInput(format!("BOUNDARY_ERROR_INVALID: RenderSet: {error}")))?;
        validate_render_set_v2_output(&render_set)?;
        let render_set_hash = evidence.render_set_object_sha256.clone();
        let target_mask = self.target_mask(target_sha256, &target)?;
        let silhouette_png = self.render_pass_bytes(&render_set, "silhouette")?;
        let model_mask = decode_binary_mask(&silhouette_png)?;
        let metrics = extended_silhouette_metrics(&target_mask.mask, &model_mask);
        let part_png = self.render_pass_bytes(&render_set, "part-id").ok();
        let inspection = candidate
            .manifest_hash
            .as_deref()
            .and_then(|hash| self.cas_read(hash).ok())
            .and_then(|bytes| strict_glb_inspection(&bytes).ok());
        let segments = Self::boundary_error_segments_for_masks(
            &target_mask.mask,
            &model_mask,
            part_png.as_deref(),
            inspection
                .as_ref()
                .map(|value| value.part_ids.as_slice())
                .unwrap_or(&[]),
            max_segments.unwrap_or(64).clamp(1, 64) as usize,
        );
        if segments.is_empty() {
            return Err(RuntimeError::InvalidInput(
                "BOUNDARY_ERROR_UNAVAILABLE: model silhouette is empty".to_owned(),
            ));
        }
        let mut result = json!({
            "schema_version":"BoundaryErrorResult@1",
            "candidate_id":candidate_id,
            "target_sha256":target_sha256,
            "render_set_hash":render_set_hash,
            "metrics":metrics,
            "segments":segments,
            "canonical_sha256":""
        });
        result["canonical_sha256"] = Value::String(canonical_json_hash(&result));
        validate_boundary_error_result(&result)?;
        Ok(result)
    }

/// Produce the same candidate-bound directional boundary evidence used by
/// `boundary_error_get`, but from an already-rendered transient Worker result.
/// Primary Form can therefore attribute local error to a typed Part without a
/// second Codex observation turn or a caller-owned image-space search.
fn boundary_error_segments_for_masks(
    reference: &[bool],
    model: &[bool],
    part_png: Option<&[u8]>,
    part_ids: &[String],
    max_segments: usize,
) -> Vec<Value> {
    let target_boundary = boundary_mask(reference);
    let model_boundary = boundary_mask(model);
    let model_points: Vec<(usize, usize)> = model_boundary
        .iter()
        .enumerate()
        .filter_map(|(index, value)| value.then_some((index % 512, index / 512)))
        .collect();
    if model_points.is_empty() {
        return Vec::new();
    }
    let (center_x, center_y) = mask_centroid(reference).unwrap_or((255.5, 255.5));
    let sample_stride = ((target_boundary.iter().filter(|value| **value).count() / 128).max(1)) as usize;
    let part_image = part_png.and_then(|png| {
        image::load_from_memory(png)
            .ok()
            .map(|image| image.resize_exact(512, 512, imageops::FilterType::Nearest).to_rgba8())
    });
    let mut segments = Vec::new();
    for (index, value) in target_boundary.iter().enumerate() {
        if !*value || index % sample_stride != 0 {
            continue;
        }
        let tx = index % 512;
        let ty = index / 512;
        let (mx, my, distance) = model_points
            .iter()
            .map(|(x, y)| {
                let dx = *x as f64 - tx as f64;
                let dy = *y as f64 - ty as f64;
                (*x, *y, (dx * dx + dy * dy).sqrt())
            })
            .min_by(|left, right| {
                left.2
                    .partial_cmp(&right.2)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .expect("model points is non-empty");
        let dx = mx as f64 - tx as f64;
        let dy = my as f64 - ty as f64;
        let radial_x = tx as f64 - center_x;
        let radial_y = ty as f64 - center_y;
        let direction = if distance <= 4.0 {
            "aligned"
        } else if dx * radial_x + dy * radial_y >= 0.0 {
            "outward"
        } else {
            "inward"
        };
        let part_id = part_image.as_ref().and_then(|image| {
            let pixel = image.get_pixel(mx as u32, my as u32).0;
            let index = part_color_index(pixel)?;
            part_ids.get(index).cloned()
        });
        segments.push(json!({
            "reference":[tx as f64 / 511.0, ty as f64 / 511.0],
            "model":[mx as f64 / 511.0, my as f64 / 511.0],
            "delta_px":[stable_visual_metric(dx), stable_visual_metric(dy)],
            "distance_px":stable_visual_metric(distance),
            "direction":direction,
            "part_id":part_id
        }));
    }
    segments.sort_by(|left, right| {
        right["distance_px"]
            .as_f64()
            .unwrap_or(0.0)
            .partial_cmp(&left["distance_px"].as_f64().unwrap_or(0.0))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    segments.truncate(max_segments.clamp(1, 64));
    segments
}

/// Project automatic silhouette boundary evidence onto one semantic Part.
///
/// An automatic target has no user-drawn Part contour.  We can still make a
/// bounded local proposal when the fixed Render Worker returned a Part-ID
/// pass: each reference boundary sample is attributed to the nearest visible
/// model boundary Part, and only those attributed samples are projected into a
/// local envelope.  This is evidence projection, not hidden-side inference.
fn projected_part_boundary_mask(segments: &[Value], part_id: &str) -> Option<Vec<bool>> {
    let mut mask = vec![false; 512 * 512];
    let mut count = 0usize;
    for segment in segments {
        if segment.get("part_id").and_then(Value::as_str) != Some(part_id) {
            continue;
        }
        let Some(point) = segment.get("reference").and_then(Value::as_array) else {
            continue;
        };
        let Some(x) = point.first().and_then(Value::as_f64) else {
            continue;
        };
        let Some(y) = point.get(1).and_then(Value::as_f64) else {
            continue;
        };
        let px = (x.clamp(0.0, 1.0) * 511.0).round() as usize;
        let py = (y.clamp(0.0, 1.0) * 511.0).round() as usize;
        if !mask[py * 512 + px] {
            mask[py * 512 + px] = true;
            count += 1;
        }
    }
    (count > 0).then_some(mask)
}

fn projected_part_boundary_error(segments: &[Value], part_id: &str) -> Option<f64> {
    let mut total = 0.0;
    let mut count = 0usize;
    for segment in segments {
        if segment.get("part_id").and_then(Value::as_str) != Some(part_id) {
            continue;
        }
        let Some(distance) = segment.get("distance_px").and_then(Value::as_f64) else {
            continue;
        };
        if distance.is_finite() {
            total += distance.clamp(0.0, 512.0);
            count += 1;
        }
    }
    (count > 0).then(|| (total / count as f64).clamp(0.0, 512.0))
}

    /// Return a deterministic per-Part contour error table for Luna's next
    /// bounded repair round.  This is deliberately read-only: it consumes the
    /// candidate's fixed RenderSet/Part-ID pass and the target's explicit
    /// contour slices, but never creates a candidate or changes Runtime state.
    pub fn silhouette_part_error(
        &self,
        project_id: &str,
        request: Value,
    ) -> Result<Value, RuntimeError> {
        validate_id(project_id)?;
        let object = request.as_object().ok_or_else(|| {
            RuntimeError::InvalidInput(
                "SILHOUETTE_PART_ERROR_INVALID: request must be an object".to_owned(),
            )
        })?;
        validate_request_keys(
            object,
            &["project_id", "candidate_id", "target_sha256"],
            "silhouette_part_error_get",
        )?;
        if object.get("project_id").and_then(Value::as_str) != Some(project_id) {
            return Err(RuntimeError::InvalidInput(
                "PROJECT_SCOPE_DENIED: Part error project differs".to_owned(),
            ));
        }
        let candidate_id = required_value_id(object.get("candidate_id"), "candidate_id")?;
        let target_sha256 = required_value_sha(object.get("target_sha256"), "target_sha256")?;
        let candidate = self.candidate(candidate_id)?.ok_or_else(|| {
            RuntimeError::InvalidInput("NOT_FOUND: candidate not found".to_owned())
        })?;
        if candidate.project_id != project_id {
            return Err(RuntimeError::InvalidInput(
                "PROJECT_SCOPE_DENIED: candidate is outside the target project".to_owned(),
            ));
        }
        let target = self.read_silhouette_target(target_sha256)?;
        let target_parts = target
            .get("parts")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                RuntimeError::InvalidInput(
                    "SILHOUETTE_PART_ERROR_UNAVAILABLE: target has no typed parts".to_owned(),
                )
            })?;
        if target_parts.is_empty() || target_parts.len() > 64 {
            return Err(RuntimeError::InvalidInput(
                "SILHOUETTE_PART_ERROR_UNAVAILABLE: target Part budget".to_owned(),
            ));
        }
        let target_mask = self.target_mask(target_sha256, &target)?;
        let evidence = self.store.get_visual_evidence(candidate_id)?.ok_or_else(|| {
            RuntimeError::InvalidInput(
                "SILHOUETTE_PART_ERROR_UNAVAILABLE: reference_compare_prepare required"
                    .to_owned(),
            )
        })?;
        if evidence.reference_id
            != target
                .get("reference_id")
                .and_then(Value::as_str)
                .unwrap_or_default()
        {
            return Err(RuntimeError::InvalidInput(
                "REFERENCE_BINDING_MISMATCH: target reference differs from candidate evidence"
                    .to_owned(),
            ));
        }
        let render_set: Value = serde_json::from_slice(&self.cas_read(&evidence.render_set_object_sha256)?)
            .map_err(|error| {
                RuntimeError::InvalidInput(format!(
                    "SILHOUETTE_PART_ERROR_INVALID: RenderSet: {error}"
                ))
            })?;
        validate_render_set_v2_output(&render_set)?;
        let silhouette_png = self.render_pass_bytes(&render_set, "silhouette")?;
        let model_mask = decode_binary_mask(&silhouette_png)?;
        let metrics = extended_silhouette_metrics(&target_mask.mask, &model_mask);
        let part_png = self.render_pass_bytes(&render_set, "part-id").ok();
        let part_ids = candidate
            .manifest_hash
            .as_deref()
            .and_then(|hash| self.cas_read(hash).ok())
            .and_then(|bytes| strict_glb_inspection(&bytes).ok())
            .map(|inspection| inspection.part_ids)
            .unwrap_or_default();
        let mut rows = Vec::with_capacity(target_parts.len());
        let mut ranked = Vec::<(f64, String)>::new();
        for part in target_parts {
            let part_object = exact_object(
                part,
                &["part_id", "start_index", "end_index", "visibility"],
                "SilhouetteTarget@1.part",
            )?;
            let part_id = required_contract_identifier(
                part_object,
                "part_id",
                "SilhouettePartErrorResult@1.part",
            )?;
            let visibility = part_object
                .get("visibility")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            let target_boundary = target_part_boundary_mask(&target, &part_id);
            let target_envelope = target_boundary.as_deref().and_then(mask_envelope);
            let target_boundary_pixels = target_boundary
                .as_deref()
                .map(|mask| mask.iter().filter(|value| **value).count())
                .unwrap_or(0);
            let model_part = part_png
                .as_deref()
                .and_then(|png| decode_part_mask(png, &part_id, &part_ids));
            let model_pixels = model_part
                .as_deref()
                .map(|mask| mask.iter().filter(|value| **value).count())
                .unwrap_or(0);
            let model_envelope = model_part.as_deref().and_then(mask_envelope);
            let ready = target_envelope.is_some() && model_envelope.is_some();
            let status = if target_envelope.is_none() {
                "empty_target_part"
            } else if model_envelope.is_none() {
                "missing_model_part"
            } else {
                "ready"
            };
            let target_bbox = target_envelope
                .map(mask_envelope_value)
                .unwrap_or_else(|| json!([0.0, 0.0, 0.0, 0.0]));
            let model_bbox = model_envelope
                .map(mask_envelope_value)
                .unwrap_or_else(|| json!([0.0, 0.0, 0.0, 0.0]));
            let (centroid_delta_x, centroid_delta_y, width_ratio, height_ratio, boundary_error_px) =
                if let (Some(target_envelope), Some(model_envelope)) =
                    (target_envelope, model_envelope)
                {
                    let target_width = (target_envelope.max_x - target_envelope.min_x + 1) as f64;
                    let model_width = (model_envelope.max_x - model_envelope.min_x + 1) as f64;
                    let target_height = (target_envelope.max_y - target_envelope.min_y + 1) as f64;
                    let model_height = (model_envelope.max_y - model_envelope.min_y + 1) as f64;
                    let boundary_error_px = part_png
                        .as_deref()
                        .map(|png| {
                            part_boundary_error(
                                png,
                                &target_mask.mask,
                                &target,
                                &part_id,
                                &part_ids,
                            )
                        })
                        .unwrap_or(512.0);
                    (
                        (target_envelope.centroid_x - model_envelope.centroid_x) * 511.0,
                        (target_envelope.centroid_y - model_envelope.centroid_y) * 511.0,
                        (target_width / model_width.max(1.0)).clamp(0.0, 4.0),
                        (target_height / model_height.max(1.0)).clamp(0.0, 4.0),
                        boundary_error_px.clamp(0.0, 512.0),
                    )
                } else {
                    (0.0, 0.0, 0.0, 0.0, 512.0)
                };
            let row = json!({
                "part_id": part_id,
                "visibility": visibility,
                "status": status,
                "target_boundary_pixels": target_boundary_pixels,
                "model_pixels": model_pixels,
                "target_bbox": target_bbox,
                "model_bbox": model_bbox,
                "centroid_delta_px": [stable_visual_metric(centroid_delta_x), stable_visual_metric(centroid_delta_y)],
                "width_ratio": stable_visual_metric(width_ratio),
                "height_ratio": stable_visual_metric(height_ratio),
                "boundary_error_px": stable_visual_metric(boundary_error_px)
            });
            if ready && visibility != "unknown" {
                ranked.push((boundary_error_px, part_id.to_owned()));
            }
            rows.push(row);
        }
        ranked.sort_by(|left, right| {
            right
                .0
                .partial_cmp(&left.0)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| left.1.cmp(&right.1))
        });
        let recommended_part_ids = ranked
            .into_iter()
            .take(16)
            .map(|(_, part_id)| Value::String(part_id))
            .collect::<Vec<_>>();
        let mut result = json!({
            "schema_version": "SilhouettePartErrorResult@1",
            "project_id": project_id,
            "candidate_id": candidate_id,
            "target_sha256": target_sha256,
            "render_set_hash": evidence.render_set_object_sha256,
            "metrics": metrics,
            "parts": rows,
            "recommended_part_ids": recommended_part_ids,
            "canonical_sha256": ""
        });
        result["canonical_sha256"] = Value::String(canonical_json_hash(&result));
        validate_silhouette_part_error_result(&result)?;
        Ok(result)
    }

    /// Run a bounded, deterministic contour fit.  The Runtime evaluates real
    /// fixed-render silhouette passes for a small camera neighborhood and
    /// derives bounded Rig parameter proposals from the target/model extent.
    /// It never mutates the candidate; Codex must turn the proposal into a new
    /// GeometryProgram and call geometry_prepare in a later step.
    pub fn silhouette_fit_prepare(
        &self,
        project_id: &str,
        request: Value,
    ) -> Result<Value, RuntimeError> {
        validate_id(project_id)?;
        let object = request.as_object().ok_or_else(|| {
            RuntimeError::InvalidInput("SILHOUETTE_FIT_INVALID: request must be an object".to_owned())
        })?;
        validate_request_keys(
            object,
            &["project_id", "candidate_id", "target_sha256", "rig", "base_camera", "optimizer", "canonical_sha256"],
            "silhouette_fit_prepare",
        )?;
        let intent_hash = required_value_sha(object.get("canonical_sha256"), "canonical_sha256")?;
        let mut intent_without_hash = request.clone();
        intent_without_hash["canonical_sha256"] = Value::String(String::new());
        if canonical_json_hash(&intent_without_hash) != intent_hash
            && canonical_json_hash(&normalize_json_numbers(&intent_without_hash)) != intent_hash
        {
            return Err(RuntimeError::InvalidInput("SILHOUETTE_FIT_INVALID: canonical_sha256 does not bind intent".to_owned()));
        }
        if object.get("project_id").and_then(Value::as_str) != Some(project_id) {
            return Err(RuntimeError::InvalidInput("PROJECT_SCOPE_DENIED: silhouette fit project differs".to_owned()));
        }
        let candidate_id = required_value_id(object.get("candidate_id"), "candidate_id")?;
        let target_sha256 = required_value_sha(object.get("target_sha256"), "target_sha256")?;
        let candidate = self.candidate(candidate_id)?.ok_or_else(|| RuntimeError::InvalidInput("NOT_FOUND: candidate not found".to_owned()))?;
        if candidate.project_id != project_id {
            return Err(RuntimeError::InvalidInput("PROJECT_SCOPE_DENIED: candidate is outside the target project".to_owned()));
        }
        let target = self.read_silhouette_target(target_sha256)?;
        let target_mask = self.target_mask(target_sha256, &target)?;
        let rig = object.get("rig").ok_or_else(|| RuntimeError::InvalidInput("SILHOUETTE_FIT_INVALID: rig is required".to_owned()))?;
        validate_silhouette_rig(rig, candidate_id).map_err(|error| {
            // The validator already emits a stable stage-prefixed error.  Keep
            // its reason intact so the thin MCP adapter can tell Codex which
            // typed field/hash failed instead of collapsing every failure to
            // a generic contract error.
            match error {
                RuntimeError::InvalidInput(detail)
                    if detail.starts_with("SILHOUETTE_RIG_INVALID:")
                        || detail.starts_with("CONTRACT_OUTPUT_INVALID:") =>
                {
                    RuntimeError::InvalidInput(detail)
                }
                other => RuntimeError::InvalidInput(format!("SILHOUETTE_RIG_INVALID: {other}")),
            }
        })?;
        let camera_input = object.get("base_camera").ok_or_else(|| RuntimeError::InvalidInput("SILHOUETTE_FIT_INVALID: base_camera is required".to_owned()))?;
        let camera = self.resolve_silhouette_fit_camera(
            project_id,
            candidate_id,
            target_sha256,
            camera_input,
        )?;
        validate_camera_calibration(&camera).map_err(|error| {
            // Preserve the concrete camera contract reason for diagnostics.
            match error {
                RuntimeError::InvalidInput(detail)
                    if detail.starts_with("CAMERA_CALIBRATION_INVALID:")
                        || detail.starts_with("CONTRACT_OUTPUT_INVALID:") =>
                {
                    RuntimeError::InvalidInput(detail)
                }
                other => RuntimeError::InvalidInput(format!("CAMERA_CALIBRATION_INVALID: {other}")),
            }
        })?;
        let optimizer = object.get("optimizer").and_then(Value::as_object).ok_or_else(|| RuntimeError::InvalidInput("SILHOUETTE_FIT_INVALID: optimizer is required".to_owned()))?;
        let algorithm = optimizer.get("algorithm").and_then(Value::as_str).ok_or_else(|| RuntimeError::InvalidInput("SILHOUETTE_FIT_INVALID: optimizer.algorithm is required".to_owned()))?;
        if !matches!(algorithm, "grid" | "coordinate_descent") {
            return Err(RuntimeError::InvalidInput("SILHOUETTE_FIT_INVALID: unsupported optimizer".to_owned()));
        }
        let max_iterations = optimizer.get("max_iterations").and_then(Value::as_u64).unwrap_or(1).clamp(1, 8);
        let max_evaluations = optimizer.get("max_evaluations").and_then(Value::as_u64).unwrap_or(16).clamp(1, 64) as usize;
        let step_fraction = optimizer
            .get("step_fraction")
            .and_then(Value::as_f64)
            .ok_or_else(|| RuntimeError::InvalidInput("SILHOUETTE_FIT_INVALID: optimizer.step_fraction is required".to_owned()))?;
        if !step_fraction.is_finite() || !(0.0..=0.5).contains(&step_fraction) || step_fraction == 0.0 {
            return Err(RuntimeError::InvalidInput("SILHOUETTE_FIT_INVALID: optimizer.step_fraction is outside (0,0.5]".to_owned()));
        }
        let artifact_sha256 = candidate.manifest_hash.clone().or(candidate.prepared_object_sha256.clone()).ok_or_else(|| RuntimeError::InvalidInput("CANDIDATE_ARTIFACT_UNAVAILABLE".to_owned()))?;
        let glb = self.cas_read(&artifact_sha256)?;
        let inspection = strict_glb_inspection(&glb).map_err(|error| {
            RuntimeError::InvalidInput(format!("SILHOUETTE_FIT_REJECTED: {error}"))
        })?;
        if !inspection.hard_gate_passed {
                return Err(RuntimeError::InvalidInput(format!("SILHOUETTE_FIT_REJECTED: {}", inspection.failure_codes.join(","))));
        }
        // A silhouette fit may only mutate a candidate-bound V2 program.  The
        // program is read from the durable evidence row, re-hashed through the
        // same Worker helper used by candidate_confirm, and never admitted to
        // CAS by this read-only proposal call.
        let geometry_program_draft = match self.store.get_geometry_candidate_evidence(candidate_id)? {
            Some(evidence) => {
                if evidence.project_id != project_id
                    || evidence.artifact_object_sha256 != artifact_sha256
                    || evidence.geometry_program_object_sha256.is_empty()
                {
                    return Err(RuntimeError::InvalidInput("SILHOUETTE_FIT_REJECTED: candidate geometry evidence is not bound".to_owned()));
                }
                let draft: Value = serde_json::from_slice(&self.cas_read(&evidence.geometry_program_object_sha256)?)
                    .map_err(|error| RuntimeError::InvalidInput(format!("SILHOUETTE_FIT_REJECTED: GeometryProgram CAS is invalid: {error}")))?;
                if draft.get("schema_version").and_then(Value::as_str) != Some("GeometryProgram@2")
                    || draft.get("project_id").and_then(Value::as_str) != Some(project_id)
                {
                    return Err(RuntimeError::InvalidInput("SILHOUETTE_FIT_REJECTED: persisted GeometryProgram scope is invalid".to_owned()));
                }
                let hash = hash_geometry_program_with_runtime_worker(&draft)
                    .map_err(|error| RuntimeError::InvalidInput(format!("SILHOUETTE_FIT_REJECTED: persisted GeometryProgram validation failed: {error}")))?;
                if hash.get("canonical_sha256").and_then(Value::as_str) != Some(evidence.geometry_program_sha256.as_str())
                    || hash.get("operator_catalog_sha256").and_then(Value::as_str) != Some(evidence.operator_catalog_sha256.as_str())
                {
                    return Err(RuntimeError::InvalidInput("SILHOUETTE_FIT_REJECTED: persisted GeometryProgram provenance drifted".to_owned()));
                }
                Some(draft)
            }
            None => None,
        };
        // Split the caller's bounded budget into initial camera search,
        // geometry trials and a final camera refit around the winning geometry.
        // The old independent per-phase caps could consume the request before
        // later Rig controls were reached and left the geometry winner at a
        // camera that had only been optimized for the authored mesh.  Every
        // probe below remains a real isolated Geometry/Render Worker call;
        // Codex supplies the typed Rig bounds once and never chooses a
        // continuous parameter trace.  The outer optimizer validation still
        // bounds the complete three-phase schedule to 64 evaluations.
        let (geometry_budget, camera_budget, camera_refit_budget) = primary_form_evaluation_budgets(
            max_evaluations,
            geometry_program_draft.is_some(),
        );
        let requested_iterations = if algorithm == "coordinate_descent" {
            max_iterations as usize
        } else {
            1
        };
        let mut current_camera = camera.clone();
        let mut remaining_budget = camera_budget;
        let mut completed_iterations = 0usize;
        let mut rows: Vec<(f64, Value, Value)> = Vec::new();
        // Keep the transient silhouette/Part-ID evidence alongside each
        // camera row.  Re-rendering the selected camera after the search
        // would consume an unreported evaluation and, more importantly,
        // could let the proposal use a different image than the row that
        // actually won the bounded camera loss.
        let mut camera_model_evidence: Vec<(Value, Vec<bool>, Option<Vec<u8>>)> = Vec::new();
        let mut best_overall: Option<(f64, Value, Value)> = None;
        let mut previous_best_loss = f64::INFINITY;
        let mut step_offset = 0usize;
        let step_variants = [
            (0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0),
            (0.08, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0),
            (-0.08, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0),
            (0.0, 0.08, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0),
            (0.0, -0.08, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0),
            (0.0, 0.0, 0.08, 0.0, 1.0, 0.0, 0.0, 1.0),
            (0.0, 0.0, -0.08, 0.0, 1.0, 0.0, 0.0, 1.0),
            (0.0, 0.0, 0.0, 4.0, 1.0, 0.0, 0.0, 1.0),
            (0.0, 0.0, 0.0, -4.0, 1.0, 0.0, 0.0, 1.0),
            (0.0, 0.0, 0.0, 0.0, 0.92, 0.0, 0.0, 1.0),
            (0.0, 0.0, 0.0, 0.0, 1.08, 0.0, 0.0, 1.0),
            (0.0, 0.0, 0.0, 0.0, 1.0, 0.06, 0.0, 1.0),
            (0.0, 0.0, 0.0, 0.0, 1.0, 0.0, -0.06, 1.0),
            (0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.94),
            (0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.06),
        ];
        for iteration in 0..requested_iterations {
            if remaining_budget == 0 {
                break;
            }
            let iterations_left = requested_iterations - iteration;
            let iteration_budget = (remaining_budget / iterations_left).max(1).min(step_variants.len());
            // Do not take the same prefix on every coordinate-descent round:
            // that used to repeat `base,+yaw,-yaw` and never test pitch/roll
            // when the request had two iterations.  Keep the first row as
            // the base camera (needed for the read-only baseline), then walk
            // the remaining deterministic neighborhood on the next round.
            let candidate_cameras = step_variants
                .iter()
                .skip(step_offset.min(step_variants.len().saturating_sub(1)))
                .take(iteration_budget)
                .map(|(yaw, pitch, roll, fov_delta, distance_scale, target_dx, target_dy, global_scale)| {
                    let candidate_camera = if *yaw == 0.0
                        && *pitch == 0.0
                        && *roll == 0.0
                        && *fov_delta == 0.0
                        && *distance_scale == 1.0
                        && *target_dx == 0.0
                        && *target_dy == 0.0
                        && *global_scale == 1.0
                    {
                        current_camera.clone()
                    } else {
                        camera_fit_variant_extended(
                            &current_camera,
                            *yaw,
                            *pitch,
                            *roll,
                            *fov_delta,
                            *distance_scale,
                            *target_dx,
                            *target_dy,
                            *global_scale,
                        )
                    };
                    validate_camera_calibration(&candidate_camera).map_err(|error| {
                        RuntimeError::InvalidInput(format!("SILHOUETTE_FIT_RENDER_FAILED: {error}"))
                    })?;
                    Ok::<Value, RuntimeError>(candidate_camera)
                })
                .collect::<Result<Vec<_>, _>>()?;
            let batch_passes = render_glb_fit_batch_with_runtime_worker(&glb, &candidate_cameras)
                .map_err(|error| RuntimeError::InvalidInput(format!("SILHOUETTE_FIT_RENDER_FAILED: {error}")))?;
            if batch_passes.len() != candidate_cameras.len() {
                return Err(RuntimeError::InvalidInput(
                    "SILHOUETTE_FIT_RENDER_FAILED: batch result count mismatch".to_owned(),
                ));
            }
            let mut iteration_best: Option<(f64, Value, Value)> = None;
            for (candidate_camera, passes) in candidate_cameras.into_iter().zip(batch_passes) {
                let silhouette = passes
                    .iter()
                    .find(|pass| pass.pass == "silhouette")
                    .ok_or_else(|| {
                        RuntimeError::InvalidInput(
                            "SILHOUETTE_FIT_RENDER_FAILED: silhouette pass missing".to_owned(),
                        )
                    })?;
                let model_mask = decode_binary_mask(&silhouette.png).map_err(|error| {
                    RuntimeError::InvalidInput(format!("SILHOUETTE_FIT_RENDER_FAILED: {error}"))
                })?;
                let metrics = extended_silhouette_metrics(&target_mask.mask, &model_mask);
                let part_png = passes
                    .iter()
                    .find(|pass| pass.pass == "part-id")
                    .map(|pass| pass.png.clone());
                camera_model_evidence.push((candidate_camera.clone(), model_mask.clone(), part_png));
                let part_context = passes
                    .iter()
                    .find(|pass| pass.pass == "part-id")
                    .map(|pass| (pass.png.as_slice(), inspection.part_ids.as_slice()));
                let loss_metrics = transient_loss_metrics_with_parts(
                    &metrics,
                    &model_mask,
                    target.get("landmarks"),
                    part_context,
                );
                let loss = camera_fit_loss(&loss_metrics);
                let row = (loss, candidate_camera, metrics);
                if iteration_best.as_ref().is_none_or(|best| loss < best.0) {
                    iteration_best = Some(row.clone());
                }
                rows.push(row);
            }
            remaining_budget = remaining_budget.saturating_sub(iteration_budget);
            step_offset = step_offset.saturating_add(iteration_budget);
            completed_iterations += 1;
            let Some(iteration_best) = iteration_best else { break; };
            let improved = iteration_best.0 + 1e-12 < previous_best_loss;
            if improved {
                previous_best_loss = iteration_best.0;
                current_camera = iteration_best.1.clone();
                if best_overall.as_ref().is_none_or(|best| iteration_best.0 < best.0) {
                    best_overall = Some(iteration_best);
                }
            } else {
                // A local camera batch can tie or lose while a later
                // deterministic neighborhood (roll/FOV/distance/target
                // offset/global scale) still contains a better view.  Stop
                // only after the declared bounded schedule is exhausted;
                // otherwise `max_evaluations` silently became a prefix
                // budget and Primary Form could never inspect those axes.
                continue;
            }
            if algorithm == "grid" {
                break;
            }
        }
        rows.sort_by(|left, right| left.0.partial_cmp(&right.0).unwrap_or(std::cmp::Ordering::Equal));
        let (mut best_loss, mut selected_camera, mut metrics) = best_overall
            .or_else(|| rows.first().cloned())
            .ok_or_else(|| RuntimeError::InvalidInput("SILHOUETTE_FIT_UNAVAILABLE".to_owned()))?;
        let base_loss = rows
            .iter()
            .find(|(_, value, _)| *value == camera)
            .map(|(loss, _, _)| *loss)
            .unwrap_or(best_loss);
        let (selected_model_mask, selected_part_png) = camera_model_evidence
            .iter()
            .find(|(candidate_camera, _, _)| candidate_camera == &selected_camera)
            .map(|(_, model_mask, part_png)| (model_mask.clone(), part_png.clone()))
            .ok_or_else(|| {
                RuntimeError::InvalidInput(
                    "SILHOUETTE_FIT_RENDER_FAILED: selected camera result missing".to_owned(),
                )
            })?;
        // A Rig proposal is only useful to Luna when a parameter is attributed
        // to the same visible Part that produced the boundary evidence.  The
        // coarse camera batch intentionally renders silhouette only; when the
        // target carries explicit observed/inferred Part slices, do one bounded
        // Part-ID readback at the selected camera and use local envelopes for
        // those parameters.  Automatic targets with no Part annotations keep
        // the conservative whole-body proposal instead of inventing ownership.
        let part_context = selected_part_png
            .as_deref()
            .map(|part_png| (part_png, inspection.part_ids.as_slice()));
        let mut selected_parameters = fit_rig_parameters_with_landmark_context(
            rig,
            &target,
            &target_mask.mask,
            &selected_model_mask,
            part_context,
            Some(&selected_camera),
        );
        // Automatic targets do not have caller-supplied contour Part ranges.
        // Reuse the selected transient Part-ID render to attribute the largest
        // visible boundary errors before geometry trials. This keeps the
        // repair local and Runtime-owned while preserving the target's
        // observed-only boundary semantics.
        let boundary_segments = Self::boundary_error_segments_for_masks(
            &target_mask.mask,
            &selected_model_mask,
            selected_part_png.as_deref(),
            &inspection.part_ids,
            64,
        );
        selected_parameters = apply_boundary_part_parameter_projection(
            rig,
            &selected_parameters,
            &boundary_segments,
            Some(&selected_camera),
        );
        // A Primary Form action must converge one semantic Part at a time.
        // The boundary projection can produce proposals for several Parts, but
        // feeding all of them into probe zero couples unrelated silhouette
        // errors and lets a large global loss hide which edit actually helped.
        // Keep the dominant candidate-bound Part as the only mutable scope for
        // this Runtime action; an unchanged result leaves the next action free
        // to select the next Part from a fresh observation.
        let focused_part_id = dominant_boundary_rig_part(rig, &boundary_segments);
        if let Some(part_id) = focused_part_id.as_deref() {
            selected_parameters = focus_rig_parameters_to_part(rig, &selected_parameters, part_id);
        }
        let mut geometry_evaluations = 0usize;
        let mut camera_refit_evaluations = 0usize;
        // Preserve the exact typed GeometryProgram that produced the winning
        // geometry trial. Returning only compact parameter deltas forced
        // Codex to reconstruct the program and could silently diverge from
        // the Runtime-owned Worker evaluation. This remains a read-only
        // proposal: the program is returned only when it strictly improves
        // the authored baseline and is never persisted or confirmed here.
        let mut selected_geometry_program: Option<Value> = None;
        if let Some(program) = geometry_program_draft.as_ref() {
            // Evaluate the evidence-attributed proposal once, then walk a
            // deterministic coordinate neighborhood around the best actual
            // mesh found so far.  If the joint proposal overshoots, the first
            // two bounded retries backtrack it toward the authored baseline
            // before spending the remaining budget on single-coordinate
            // probes. Every probe goes through the isolated Geometry Worker
            // and Render Worker; this is the Primary Form numeric loop, not
            // an image-space heuristic in Codex.
            let parameter_indices = ranked_rig_parameter_indices_with_boundary_context(
                rig,
                &selected_parameters,
                &boundary_segments,
            );
            let parameter_indices = if let Some(part_id) = focused_part_id.as_deref() {
                parameter_indices
                    .into_iter()
                    .filter(|index| {
                        rig.get("parameters")
                            .and_then(Value::as_array)
                            .and_then(|parameters| parameters.get(*index))
                            .and_then(|parameter| parameter.get("part_id"))
                            .and_then(Value::as_str)
                            .is_some_and(|candidate_part_id| {
                                rig_part_matches_observed_part(candidate_part_id, part_id)
                            })
                    })
                    .collect::<Vec<_>>()
            } else {
                parameter_indices
            };
            let definitions = rig
                .get("parameters")
                .and_then(Value::as_array)
                .ok_or_else(|| RuntimeError::InvalidInput("SILHOUETTE_FIT_GEOMETRY_FAILED: Rig parameters are missing".to_owned()))?;
            // Keep the current authored Rig as the immutable optimization
            // baseline.  An evidence-attributed proposal is a first trial,
            // not an unconditional new incumbent: if it is worse than the
            // current artifact, later coordinate probes must return to the
            // authored parameters instead of walking around a bad proposal.
            let baseline_parameters = definitions.clone();
            let mut best_geometry_loss = best_loss;
            let mut best_geometry_parameters = baseline_parameters.clone();
            let mut best_geometry_metrics = metrics.clone();
            let mut initial_proposal_improved = None;
            for probe_index in 0..geometry_budget {
                let backtrack_fraction = if initial_proposal_improved == Some(false) {
                    match probe_index {
                        1 => Some(0.5),
                        2 => Some(0.25),
                        _ => None,
                    }
                } else {
                    None
                };
                let mut parameter_values = if probe_index == 0 {
                    selected_parameters.clone()
                } else if let Some(fraction) = backtrack_fraction {
                    interpolate_rig_parameter_values(
                        definitions,
                        &selected_parameters,
                        fraction,
                    )
                } else {
                    best_geometry_parameters.clone()
                };
                if probe_index > 0 && backtrack_fraction.is_none() && !parameter_indices.is_empty() {
                    let coordinate_probe_index = if initial_proposal_improved == Some(false) {
                        // Probe 1 and 2 were reserved for proposal backtracking.
                        // Start the coordinate schedule at its first parameter
                        // only after those retries have been consumed.
                        probe_index.saturating_sub(2)
                    } else {
                        probe_index
                    };
                    let probe_slot = coordinate_probe_index - 1;
                    let coordinate = primary_form_probe_coordinate(&parameter_indices, coordinate_probe_index)
                        .ok_or_else(|| RuntimeError::InvalidInput("SILHOUETTE_FIT_GEOMETRY_FAILED: Rig probe schedule is empty".to_owned()))?;
                    let parameter = definitions.get(coordinate).ok_or_else(|| RuntimeError::InvalidInput("SILHOUETTE_FIT_GEOMETRY_FAILED: Rig parameter index drifted".to_owned()))?;
                    let value = parameter_values
                        .get(coordinate)
                        .and_then(|row| row.get("value"))
                        .and_then(Value::as_f64)
                        .unwrap_or_else(|| parameter.get("value").and_then(Value::as_f64).unwrap_or(0.0));
                    let min = parameter.get("min").and_then(Value::as_f64).unwrap_or(value);
                    let max = parameter.get("max").and_then(Value::as_f64).unwrap_or(value);
                    let step = parameter.get("step").and_then(Value::as_f64).unwrap_or(0.01).abs();
                    let span = (max - min).abs();
                    let delta = (step * step_fraction)
                        .max(span * step_fraction * 0.25)
                        .min(span * 0.5)
                        .max(1e-6);
                    // The first coordinate pass follows the direction of the
                    // evidence-attributed proposal, so a 16-probe default
                    // reaches every supplied Primary Form control whenever the
                    // Runtime budget has room for the complete first pass.
                    // Only a later pass tests the opposite direction.  The
                    // old +/- pair schedule spent two probes on each early
                    // parameter and left the rest of the Rig untouched.
                    let authored_value = parameter.get("value").and_then(Value::as_f64).unwrap_or(value);
                    let proposal_value = selected_parameters
                        .get(coordinate)
                        .and_then(|row| row.get("value"))
                        .and_then(Value::as_f64)
                        .unwrap_or(authored_value);
                    let proposal_direction = (proposal_value - authored_value).signum();
                    let fallback_direction = if coordinate % 2 == 0 { 1.0 } else { -1.0 };
                    let direction = (if proposal_direction == 0.0 {
                        fallback_direction
                    } else {
                        proposal_direction
                    }) * if probe_slot / parameter_indices.len() % 2 == 0 {
                        1.0
                    } else {
                        -1.0
                    };
                    if let Some(row) = parameter_values.get_mut(coordinate) {
                        row["value"] = Value::from(stable_visual_metric((value + direction * delta).clamp(min, max)));
                    }
                }
                let (draft, applied) = materialize_rig_geometry_program(
                    program,
                    rig,
                    &parameter_values,
                    Some(&selected_camera),
                )?;
                if applied == 0 {
                    if probe_index == 0 {
                        initial_proposal_improved = Some(false);
                    }
                    continue;
                }
                let finalized = finalize_v2_geometry_program(draft)?;
                let artifact = compile_geometry_with_runtime_worker(&finalized, None)
                    .map_err(|error| RuntimeError::InvalidInput(format!("SILHOUETTE_FIT_GEOMETRY_FAILED: {error}")))?;
                let variant_inspection = strict_glb_inspection(&artifact.glb).map_err(|error| {
                    RuntimeError::InvalidInput(format!("SILHOUETTE_FIT_GEOMETRY_FAILED: {error}"))
                })?;
                if !variant_inspection.hard_gate_passed {
                    return Err(RuntimeError::InvalidInput(format!(
                        "SILHOUETTE_FIT_GEOMETRY_FAILED: {}",
                        variant_inspection.failure_codes.join(",")
                    )));
                }
                let passes = render_glb_fit_batch_with_runtime_worker(&artifact.glb, &[selected_camera.clone()])
                    .map_err(|error| RuntimeError::InvalidInput(format!("SILHOUETTE_FIT_GEOMETRY_RENDER_FAILED: {error}")))?;
                let silhouette = passes
                    .first()
                    .and_then(|batch| batch.iter().find(|pass| pass.pass == "silhouette"))
                    .ok_or_else(|| RuntimeError::InvalidInput("SILHOUETTE_FIT_GEOMETRY_RENDER_FAILED: silhouette pass missing".to_owned()))?;
                let model_mask = decode_binary_mask(&silhouette.png).map_err(|error| {
                    RuntimeError::InvalidInput(format!("SILHOUETTE_FIT_GEOMETRY_RENDER_FAILED: {error}"))
                })?;
                let variant_metrics = extended_silhouette_metrics(&target_mask.mask, &model_mask);
                let part_context = passes
                    .first()
                    .and_then(|batch| batch.iter().find(|pass| pass.pass == "part-id"))
                    .map(|pass| (pass.png.as_slice(), variant_inspection.part_ids.as_slice()));
                let loss_metrics = transient_loss_metrics_with_parts(
                    &variant_metrics,
                    &model_mask,
                    target.get("landmarks"),
                    part_context,
                );
                // Geometry trials must use the same evidence-weighted loss as
                // the camera rows.  Comparing camera_fit_loss against the
                // contour-only extended_silhouette_loss allowed a geometry
                // variant to win by dropping landmark coverage, because the
                // two values were not comparable.
                let loss = camera_fit_loss(&loss_metrics);
                geometry_evaluations += 1;
                if probe_index == 0 {
                    initial_proposal_improved = Some(loss + 1e-12 < best_loss);
                }
                if loss + 1e-12 < best_geometry_loss {
                    best_geometry_loss = loss;
                    best_geometry_parameters = parameter_values;
                    best_geometry_metrics = variant_metrics;
                    selected_geometry_program = Some(finalized);
                }
            }
            if best_geometry_loss + 1e-12 < best_loss {
                best_loss = best_geometry_loss;
                selected_parameters = best_geometry_parameters;
                metrics = best_geometry_metrics;
            } else {
                // Do not return a visually worse evidence proposal as if it
                // were the Runtime winner.  A no-improvement result carries
                // the authored baseline and leaves the next repair round
                // free to choose a different typed Part/target intent.
                selected_parameters = baseline_parameters;
            }

            // Geometry and camera are coupled: changing the chest/limb
            // envelope changes the perspective projection that minimizes the
            // same target loss.  The old path accepted a geometry winner at
            // the authored camera and then sent that pair directly to
            // compare.  That allowed camera error to mask geometry progress
            // (or let a camera compensate for a bad form), so a Primary Form
            // action could report a local winner without actually converging.
            // Refit only a small deterministic neighborhood around the
            // geometry winner.  This is still one Runtime-owned bounded
            // schedule; Codex never sees or steers the continuous trace.
            if camera_refit_budget > 0 && selected_geometry_program.is_some() {
                let finalized = selected_geometry_program
                    .as_ref()
                    .expect("geometry program checked above");
                let artifact = compile_geometry_with_runtime_worker(finalized, None)
                    .map_err(|error| RuntimeError::InvalidInput(format!("SILHOUETTE_FIT_GEOMETRY_REFIT_FAILED: {error}")))?;
                let inspection = strict_glb_inspection(&artifact.glb).map_err(|error| {
                    RuntimeError::InvalidInput(format!("SILHOUETTE_FIT_GEOMETRY_REFIT_FAILED: {error}"))
                })?;
                if !inspection.hard_gate_passed {
                    return Err(RuntimeError::InvalidInput(format!(
                        "SILHOUETTE_FIT_GEOMETRY_REFIT_FAILED: {}",
                        inspection.failure_codes.join(",")
                    )));
                }
                let refit_cameras = primary_form_camera_refit_schedule(
                    &selected_camera,
                    camera_refit_budget,
                );
                if !refit_cameras.is_empty() {
                    let refit_passes = render_worker::render_glb_fit_batch_at_resolution(
                        &artifact.glb,
                        &refit_cameras,
                        512,
                    )
                    .map_err(|error| {
                        RuntimeError::InvalidInput(format!(
                            "SILHOUETTE_FIT_GEOMETRY_REFIT_RENDER_FAILED: {error}"
                        ))
                    })?;
                    if refit_passes.len() != refit_cameras.len() {
                        return Err(RuntimeError::InvalidInput(
                            "SILHOUETTE_FIT_GEOMETRY_REFIT_RENDER_FAILED: result count mismatch"
                                .to_owned(),
                        ));
                    }
                    for (camera_candidate, passes) in refit_cameras.into_iter().zip(refit_passes) {
                        let silhouette = passes
                            .iter()
                            .find(|pass| pass.pass == "silhouette")
                            .ok_or_else(|| {
                                RuntimeError::InvalidInput(
                                    "SILHOUETTE_FIT_GEOMETRY_REFIT_RENDER_FAILED: silhouette pass missing"
                                        .to_owned(),
                                )
                            })?;
                        let model_mask = decode_binary_mask(&silhouette.png).map_err(|error| {
                            RuntimeError::InvalidInput(format!(
                                "SILHOUETTE_FIT_GEOMETRY_REFIT_FAILED: {error}"
                            ))
                        })?;
                        let part_context = passes
                            .iter()
                            .find(|pass| pass.pass == "part-id")
                            .map(|pass| (pass.png.as_slice(), inspection.part_ids.as_slice()));
                        let candidate_metrics =
                            extended_silhouette_metrics(&target_mask.mask, &model_mask);
                        let loss_metrics = transient_loss_metrics_with_parts(
                            &candidate_metrics,
                            &model_mask,
                            target.get("landmarks"),
                            part_context,
                        );
                        let loss = camera_fit_loss(&loss_metrics);
                        camera_refit_evaluations += 1;
                        if loss + 1e-12 < best_loss {
                            best_loss = loss;
                            selected_camera = camera_candidate;
                            metrics = candidate_metrics;
                        }
                    }
                }
            }
        }
        // The geometry-search incumbent may still be represented by the full
        // SilhouetteRig parameter definitions when the authored baseline wins.
        // SilhouetteFitResult@1 exposes compact parameters plus an optional
        // Runtime-validated GeometryProgram proposal, so normalize the
        // parameter projection at this module boundary before calculating
        // deltas and validating the result.
        let selected_parameters = compact_rig_parameter_values(rig, &selected_parameters);
        let parameter_deltas = rig_parameter_deltas(rig, &selected_parameters);
        let total_evaluations = rows
            .len()
            .saturating_add(geometry_evaluations)
            .saturating_add(camera_refit_evaluations);
        let mut result = json!({
            "schema_version":"SilhouetteFitResult@1",
            "project_id":project_id,
            "candidate_id":candidate_id,
            "target_sha256":target_sha256,
            "selected_camera":selected_camera,
            "selected_parameters":selected_parameters,
            "parameter_deltas":parameter_deltas,
            "selected_geometry_program":selected_geometry_program,
            "geometry_evaluations":geometry_evaluations,
            "iterations":completed_iterations,
            "evaluations":total_evaluations,
            "metrics":metrics,
            "thresholds":{"silhouette_iou":0.9,"boundary_f1_4px":0.9},
            "status":if metrics["silhouette_iou"].as_f64().unwrap_or(0.0) >= 0.9 && metrics["boundary_f1_4px"].as_f64().unwrap_or(0.0) >= 0.9 {"ready"} else if best_loss + 1e-12 < base_loss {"quality_target_not_met"} else {"no_improvement"},
            "canonical_sha256":""
        });
        result["canonical_sha256"] = Value::String(canonical_json_hash(&result));
        validate_silhouette_fit_result(&result)?;
        // The fit may select a refined camera that was not part of the
        // initial camera-fit candidate batch.  Keep that Runtime-owned
        // calibration in the same process-local cache so the subsequent
        // CameraCalibrationRef handoff to reference comparison can resolve
        // the exact fit winner without re-running search or accepting a
        // caller-supplied camera payload.
        let cache_key = camera_fit_cache_key(project_id, candidate_id, target_sha256);
        if let Ok(mut cache) = self.camera_fit_cache.lock() {
            let entry = cache.entry(cache_key).or_insert_with(|| json!({"candidates":[]}));
            if let (Some(candidates), Some(fit_camera)) = (
                entry.get_mut("candidates").and_then(Value::as_array_mut),
                result.get("selected_camera").cloned(),
            ) {
                let already_present = candidates.iter().any(|row| row.get("camera") == Some(&fit_camera));
                if !already_present {
                    candidates.push(json!({"camera": fit_camera}));
                }
            }
        }
        Ok(result)
    }

    /// Execute one Runtime-owned Primary Form repair prepare.  Codex supplies
    /// one bounded fit intent; the Runtime owns the continuous search and
    /// chains the winning typed GeometryProgram through Geometry Worker,
    /// strict readback, isolated Render Worker and candidate-bound compare.
    /// This creates only a staged candidate.  It never confirms a version or
    /// exports an asset, and a failed visual gate remains a failed result.
    pub fn primary_form_repair_prepare(
        &self,
        project_id: &str,
        base_version_id: Option<&str>,
        request: Value,
    ) -> Result<Value, RuntimeError> {
        validate_id(project_id)?;
        let object = request.as_object().ok_or_else(|| {
            RuntimeError::InvalidInput(
                "PRIMARY_FORM_REPAIR_INVALID: request must be an object".to_owned(),
            )
        })?;
        validate_request_keys(
            object,
            &[
                "project_id",
                "candidate_id",
                "target_sha256",
                "rig",
                "base_camera",
                "optimizer",
                "base_version_id",
                "canonical_sha256",
            ],
            "primary_form_repair_prepare",
        )?;
        if object.get("project_id").and_then(Value::as_str) != Some(project_id) {
            return Err(RuntimeError::InvalidInput(
                "PROJECT_SCOPE_DENIED: Primary Form repair project differs".to_owned(),
            ));
        }
        let request_base_version_id = object
            .get("base_version_id")
            .and_then(Value::as_str);
        if request_base_version_id != base_version_id {
            return Err(RuntimeError::InvalidInput(
                "PRIMARY_FORM_REPAIR_INVALID: base_version_id argument is not bound to intent"
                    .to_owned(),
            ));
        }
        let intent_hash = required_value_sha(object.get("canonical_sha256"), "canonical_sha256")?;
        let mut intent_without_hash = request.clone();
        intent_without_hash["canonical_sha256"] = Value::String(String::new());
        if canonical_json_hash(&intent_without_hash) != intent_hash
            && canonical_json_hash(&normalize_json_numbers(&intent_without_hash)) != intent_hash
        {
            return Err(RuntimeError::InvalidInput(
                "PRIMARY_FORM_REPAIR_INVALID: canonical_sha256 does not bind intent".to_owned(),
            ));
        }
        if let Some(requested_base_version) = object.get("base_version_id") {
            if !requested_base_version.is_null() {
                let base_version = requested_base_version.as_str().ok_or_else(|| {
                    RuntimeError::InvalidInput(
                        "PRIMARY_FORM_REPAIR_INVALID: base_version_id must be an identifier or null"
                            .to_owned(),
                    )
                })?;
                validate_id(base_version)?;
            }
        }

        // Keep the existing SilhouetteFitResult@1 contract as the search
        // boundary.  The outer action hash includes base_version_id, while
        // the nested fit hash is deterministically re-derived after removing
        // that transaction concern.
        let mut fit_request = request.clone();
        fit_request
            .as_object_mut()
            .expect("Primary Form request object")
            .remove("base_version_id");
        if let Some(optimizer) = fit_request
            .get_mut("optimizer")
            .and_then(Value::as_object_mut)
        {
            normalize_primary_form_repair_optimizer(optimizer);
        }
        fit_request["canonical_sha256"] = Value::String(String::new());
        fit_request["canonical_sha256"] =
            Value::String(canonical_json_hash(&fit_request));
        let fit = self.silhouette_fit_prepare(project_id, fit_request)?;
        let target_sha256 = required_value_sha(fit.get("target_sha256"), "target_sha256")?;
        let target = self.read_silhouette_target(target_sha256)?;
        let reference_id = required_value_id(target.get("reference_id"), "reference_id")?;
        let reference = self.reference(reference_id)?.ok_or_else(|| {
            RuntimeError::InvalidInput("NOT_FOUND: Primary Form reference not found".to_owned())
        })?;
        if reference.project_id != project_id {
            return Err(RuntimeError::InvalidInput(
                "REFERENCE_SCOPE_DENIED: Primary Form target reference is outside the project"
                    .to_owned(),
            ));
        }

        let mut result = json!({
            "schema_version":"PrimaryFormRepairPrepareResult@1",
            "project_id":project_id,
            "source_candidate_id":fit["candidate_id"].clone(),
            "target_sha256":target_sha256,
            "reference_id":reference_id,
            "fit_result":fit,
            "prepared_candidate":Value::Null,
            "visual_evidence":Value::Null,
            "status":"no_improvement",
            "quality_status":"not-run",
            "candidate_state":"unchanged",
            "version_created":false,
            "canonical_sha256":""
        });
        let Some(program) = result["fit_result"]["selected_geometry_program"].as_object() else {
            result["canonical_sha256"] = Value::String(canonical_json_hash(&result));
            validate_primary_form_repair_prepare_result(&result)?;
            return Ok(result);
        };
        if program.get("project_id").and_then(Value::as_str) != Some(project_id) {
            return Err(RuntimeError::InvalidInput(
                "PRIMARY_FORM_REPAIR_REJECTED: selected GeometryProgram project differs"
                    .to_owned(),
            ));
        }

        let prepared = self
            .prepare_geometry_candidate(
                project_id,
                base_version_id.or_else(|| object.get("base_version_id").and_then(Value::as_str)),
                json!({
                    "typed":"geometry",
                    "reference_id":reference_id,
                    "geometry_program":Value::Object(program.clone())
                }),
            )
            .map_err(|error| match error {
                RuntimeError::Store(StoreError::Cas(CasError::HashMismatch { .. })) => {
                    RuntimeError::InvalidInput(
                        "PRIMARY_FORM_REPAIR_CAS_HASH_MISMATCH: staged GeometryProgram hash binding rejected"
                            .to_owned(),
                    )
                }
                RuntimeError::Store(StoreError::Cas(CasError::InvalidHash)) => {
                    RuntimeError::InvalidInput(
                        "PRIMARY_FORM_REPAIR_CAS_INVALID_HASH: staged GeometryProgram hash is invalid"
                            .to_owned(),
                    )
                }
                RuntimeError::Store(StoreError::Cas(CasError::Corrupt)) => {
                    RuntimeError::InvalidInput(
                        "PRIMARY_FORM_REPAIR_CAS_CORRUPT: staged CAS object is corrupt".to_owned(),
                    )
                }
                RuntimeError::Store(StoreError::Cas(CasError::CapacityExceeded)) => {
                    RuntimeError::InvalidInput(
                        "PRIMARY_FORM_REPAIR_CAS_CAPACITY_EXCEEDED: staged object exceeds the CAS limit"
                            .to_owned(),
                    )
                }
                RuntimeError::Store(StoreError::Cas(CasError::UnsafeRoot)) => {
                    RuntimeError::InvalidInput(
                        "PRIMARY_FORM_REPAIR_CAS_UNSAFE_ROOT: staged CAS root is unsafe".to_owned(),
                    )
                }
                RuntimeError::Store(StoreError::Cas(CasError::Io(_))) => {
                    RuntimeError::InvalidInput(
                        "PRIMARY_FORM_REPAIR_CAS_IO: staged CAS file operation failed".to_owned(),
                    )
                }
                other => other,
            })?;
        validate_geometry_prepare_result_v2_output(&prepared)?;
        let prepared_candidate_id = required_value_id(
            prepared.pointer("/candidate/candidate_id"),
            "candidate_id",
        )?
        .to_owned();
        let selected_camera = result["fit_result"]["selected_camera"].clone();
        // The fit winner is cached against the source candidate. The staged
        // GeometryProgram has a new candidate ID, so passing a
        // CameraCalibrationRef here would make comparison look up the old
        // candidate/target cache key and fail closed. Runtime already owns
        // this exact validated calibration; carry the complete object across
        // the internal staged-candidate boundary instead of asking Codex to
        // reconstruct or rebind it.
        let view_spec = primary_form_view_spec(&reference, &target, target_sha256)?;
        let comparison = self.prepare_reference_comparison(
            project_id,
            json!({
                "project_id":project_id,
                "candidate_id":prepared_candidate_id,
                "reference_id":reference_id,
                "view_spec":view_spec,
                "camera":selected_camera,
                "target_sha256":target_sha256
            }),
        )?;
        let quality_status = comparison
            .pointer("/quality_report/visual_status")
            .and_then(Value::as_str)
            .unwrap_or("not-run");
        result["prepared_candidate"] = prepared;
        result["visual_evidence"] = json!({
            "candidate_id":prepared_candidate_id,
            "reference_id":reference_id,
            "target_sha256":target_sha256,
            "camera_hash":comparison["camera"]["camera_hash"].clone(),
            "render_set_hash":comparison["render_set_object_sha256"].clone(),
            "comparison_report_hash":comparison["comparison_report_object_sha256"].clone(),
            "quality_report_hash":comparison["quality_report_object_sha256"].clone(),
            "render_set":comparison["render_set"].clone(),
            "comparison_report":comparison["comparison_report"].clone(),
            "quality_report":comparison["quality_report"].clone()
        });
        result["status"] = Value::String("prepared".to_owned());
        result["quality_status"] = Value::String(quality_status.to_owned());
        result["candidate_state"] = Value::String("staged_new_candidate".to_owned());
        result["canonical_sha256"] = Value::String(canonical_json_hash(&result));
        validate_primary_form_repair_prepare_result(&result)?;
        Ok(result)
    }

    /// Resolve either a complete CameraCalibration@1 or a compact
    /// CameraCalibrationRef@1.  The compact form is deliberately limited to
    /// the two Runtime-owned hashes; when a model round-trips a large camera
    /// object and changes float spelling/precision, the Runtime re-runs its
    /// deterministic bounded camera search and selects the exact matching
    /// calibration instead of accepting an altered payload.
    fn resolve_silhouette_fit_camera(
        &self,
        project_id: &str,
        candidate_id: &str,
        target_sha256: &str,
        input: &Value,
    ) -> Result<Value, RuntimeError> {
        if validate_camera_calibration(input).is_ok() {
            return Ok(input.clone());
        }
        let object = input.as_object().ok_or_else(|| {
            RuntimeError::InvalidInput(
                "CAMERA_CALIBRATION_INVALID: base_camera must be an object".to_owned(),
            )
        })?;
        if object.len() != 3
            || object.get("schema_version").and_then(Value::as_str)
                != Some("CameraCalibrationRef@1")
        {
            return Err(RuntimeError::InvalidInput(
                "CAMERA_CALIBRATION_INVALID: base_camera is neither a full calibration nor CameraCalibrationRef@1".to_owned(),
            ));
        }
        let camera_hash = required_value_sha(object.get("camera_hash"), "camera_hash")?;
        let canonical_sha256 = required_value_sha(object.get("canonical_sha256"), "canonical_sha256")?;
        let cache_key = camera_fit_cache_key(project_id, candidate_id, target_sha256);
        let cached = self
            .camera_fit_cache
            .lock()
            .ok()
            .and_then(|cache| cache.get(&cache_key).cloned());
        let result = match cached {
            Some(result) => result,
            None => self.prepare_camera_fit(
                project_id,
                json!({
                    "project_id": project_id,
                    "candidate_id": candidate_id,
                    "target_sha256": target_sha256,
                    "camera": null
                }),
            )?,
        };
        let matches = result
            .get("candidates")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|row| row.get("camera"))
            .find(|camera| {
                camera.get("camera_hash").and_then(Value::as_str) == Some(camera_hash)
                    && camera.get("canonical_sha256").and_then(Value::as_str)
                        == Some(canonical_sha256)
            })
            .cloned()
            .or_else(|| {
                result.get("selected_camera").filter(|camera| {
                    camera.get("camera_hash").and_then(Value::as_str) == Some(camera_hash)
                        && camera.get("canonical_sha256").and_then(Value::as_str)
                            == Some(canonical_sha256)
                }).cloned()
            });
        matches.ok_or_else(|| {
            RuntimeError::InvalidInput(
                "CAMERA_CALIBRATION_INVALID: CameraCalibrationRef@1 does not match candidate/target camera evidence".to_owned(),
            )
        })
    }

    /// Return a bounded one-Part adjustment proposal from signed contour
    /// evidence. This is intentionally read-only: no geometry program is
    /// edited and no candidate is replaced by this call.
    pub fn part_contour_fit_prepare(&self, project_id: &str, request: Value) -> Result<Value, RuntimeError> {
        validate_id(project_id)?;
        let object = request.as_object().ok_or_else(|| RuntimeError::InvalidInput("PART_CONTOUR_FIT_INVALID: request must be an object".to_owned()))?;
        validate_request_keys(object, &["project_id", "candidate_id", "target_sha256", "part_id", "rig"], "part_contour_fit_prepare")?;
        if object.get("project_id").and_then(Value::as_str) != Some(project_id) { return Err(RuntimeError::InvalidInput("PROJECT_SCOPE_DENIED: part fit project differs".to_owned())); }
        let candidate_id = required_value_id(object.get("candidate_id"), "candidate_id")?;
        let target_sha256 = required_value_sha(object.get("target_sha256"), "target_sha256")?;
        let part_id = required_value_id(object.get("part_id"), "part_id")?;
        let candidate = self.candidate(candidate_id)?.ok_or_else(|| RuntimeError::InvalidInput("NOT_FOUND: candidate not found".to_owned()))?;
        if candidate.project_id != project_id { return Err(RuntimeError::InvalidInput("PROJECT_SCOPE_DENIED: candidate is outside the target project".to_owned())); }
        validate_silhouette_rig(object.get("rig").ok_or_else(|| RuntimeError::InvalidInput("PART_CONTOUR_FIT_INVALID: rig is required".to_owned()))?, candidate_id)?;
        let target = self.read_silhouette_target(target_sha256)?;
        let target_mask = self.target_mask(target_sha256, &target)?;
        let evidence = self.store.get_visual_evidence(candidate_id)?.ok_or_else(|| RuntimeError::InvalidInput("PART_CONTOUR_FIT_UNAVAILABLE: reference_compare_prepare required".to_owned()))?;
        let render_set: Value = serde_json::from_slice(&self.cas_read(&evidence.render_set_object_sha256)?).map_err(|error| RuntimeError::InvalidInput(format!("PART_CONTOUR_FIT_INVALID: {error}")))?;
        validate_render_set_v2_output(&render_set)?;
        let model_mask = decode_binary_mask(&self.render_pass_bytes(&render_set, "silhouette")?)?;
        let metrics = extended_silhouette_metrics(&target_mask.mask, &model_mask);
        let part_png = self.render_pass_bytes(&render_set, "part-id").ok();
        let part_ids = candidate
            .manifest_hash
            .as_deref()
            .and_then(|hash| self.cas_read(hash).ok())
            .and_then(|bytes| strict_glb_inspection(&bytes).ok())
            .map(|inspection| inspection.part_ids)
            .unwrap_or_default();
        if !part_ids.iter().any(|value| value == part_id) {
            return Err(RuntimeError::InvalidInput(
                "PART_CONTOUR_FIT_INVALID: part_id is absent from candidate readback".to_owned(),
            ));
        }
        let decoded_part_mask = part_png
            .as_deref()
            .and_then(|bytes| decode_part_mask(bytes, part_id, &part_ids));
        let explicit_target_part_boundary = target_part_boundary_mask(&target, part_id);
        let automatic_boundary_segments = if explicit_target_part_boundary.is_none() {
            Self::boundary_error_segments_for_masks(
                &target_mask.mask,
                &model_mask,
                part_png.as_deref(),
                &part_ids,
                64,
            )
        } else {
            Vec::new()
        };
        let projected_target_part_boundary = if explicit_target_part_boundary.is_none() {
            Self::projected_part_boundary_mask(&automatic_boundary_segments, part_id)
        } else {
            None
        };
        let target_part_boundary = explicit_target_part_boundary
            .clone()
            .or(projected_target_part_boundary);
        let part_error = if explicit_target_part_boundary.is_some() {
            part_png
                .as_deref()
                .map(|bytes| part_boundary_error(bytes, &target_mask.mask, &target, part_id, &part_ids))
                .unwrap_or_else(|| metrics["sdf_chamfer_px"].as_f64().unwrap_or(0.0))
        } else {
            Self::projected_part_boundary_error(&automatic_boundary_segments, part_id)
                .or_else(|| {
                    part_png.as_deref().map(|bytes| {
                        part_boundary_error(bytes, &target_mask.mask, &target, part_id, &part_ids)
                    })
                })
                .unwrap_or_else(|| metrics["sdf_chamfer_px"].as_f64().unwrap_or(0.0))
        };
        let mut adjustments = Vec::new();
        if let Some(parameters) = object.get("rig").and_then(|rig| rig.get("parameters")).and_then(Value::as_array) {
            // A Part proposal must be driven by that Part's own projected
            // envelope.  Reusing the whole-body bbox here makes a chest or
            // shoulder correction inherit unrelated leg/head error and was
            // the main reason contour edits drifted away from the reference.
            if let (Some(model_part_mask), Some(target_part_mask)) = (
                decoded_part_mask.as_deref(),
                target_part_boundary.as_deref(),
            ) {
                if let (Some(target_envelope), Some(model_envelope)) = (
                    mask_envelope(&target_part_mask),
                    mask_envelope(model_part_mask),
                ) {
                    for parameter in parameters
                        .iter()
                        .filter(|value| value.get("part_id").and_then(Value::as_str) == Some(part_id))
                        .take(16)
                    {
                        let value = parameter.get("value").and_then(Value::as_f64).unwrap_or(0.0);
                        let min = parameter.get("min").and_then(Value::as_f64).unwrap_or(value);
                        let max = parameter.get("max").and_then(Value::as_f64).unwrap_or(value);
                        let delta = local_part_parameter_delta(parameter, target_envelope, model_envelope);
                        adjustments.push(json!({
                            "parameter_id":parameter.get("parameter_id").and_then(Value::as_str).unwrap_or("unknown"),
                            "delta":stable_visual_metric(delta),
                            "bounded_value":stable_visual_metric((value + delta).clamp(min, max))
                        }));
                    }
                }
            }
        }
        let mut result = json!({"schema_version":"PartContourFitResult@1","project_id":project_id,"candidate_id":candidate_id,"target_sha256":target_sha256,"part_id":part_id,"adjustments":adjustments,"metrics":{"silhouette_iou":metrics["silhouette_iou"],"boundary_f1_4px":metrics["boundary_f1_4px"],"sdf_chamfer_px":metrics["sdf_chamfer_px"],"part_boundary_error_px":stable_visual_metric(part_error)},"status":if part_error > 1.0 {"proposal_ready"} else {"no_action"},"canonical_sha256":""});
        result["canonical_sha256"] = Value::String(canonical_json_hash(&result));
        validate_part_contour_fit_result(&result)?;
        Ok(result)
    }

    /// Compare two or more candidate render evidences against the same target.
    pub fn silhouette_candidate_compare(&self, project_id: &str, request: Value) -> Result<Value, RuntimeError> {
        validate_id(project_id)?;
        let object = request.as_object().ok_or_else(|| RuntimeError::InvalidInput("SILHOUETTE_COMPARE_INVALID: request must be an object".to_owned()))?;
        validate_request_keys(object, &["project_id", "target_sha256", "candidate_ids"], "silhouette_candidate_compare")?;
        if object.get("project_id").and_then(Value::as_str) != Some(project_id) { return Err(RuntimeError::InvalidInput("PROJECT_SCOPE_DENIED: compare project differs".to_owned())); }
        let target_sha256 = required_value_sha(object.get("target_sha256"), "target_sha256")?;
        let ids = object.get("candidate_ids").and_then(Value::as_array).ok_or_else(|| RuntimeError::InvalidInput("SILHOUETTE_COMPARE_INVALID: candidate_ids is required".to_owned()))?;
        if !(2..=8).contains(&ids.len()) { return Err(RuntimeError::InvalidInput("SILHOUETTE_COMPARE_INVALID: candidate_ids must contain 2..8 ids".to_owned())); }
        let target = self.read_silhouette_target(target_sha256)?;
        let target_mask = self.target_mask(target_sha256, &target)?;
        let mut rows = Vec::new();
        for value in ids {
            let candidate_id = required_value_id(Some(value), "candidate_id")?;
            if rows.iter().any(|row: &Value| row["candidate_id"].as_str() == Some(candidate_id)) { return Err(RuntimeError::InvalidInput("SILHOUETTE_COMPARE_INVALID: duplicate candidate_id".to_owned())); }
            let candidate = self.candidate(candidate_id)?.ok_or_else(|| RuntimeError::InvalidInput("NOT_FOUND: candidate not found".to_owned()))?;
            if candidate.project_id != project_id { return Err(RuntimeError::InvalidInput("PROJECT_SCOPE_DENIED: candidate is outside the target project".to_owned())); }
            let evidence = self.store.get_visual_evidence(candidate_id)?.ok_or_else(|| RuntimeError::InvalidInput("SILHOUETTE_COMPARE_UNAVAILABLE: candidate has no visual evidence".to_owned()))?;
            let render_set: Value = serde_json::from_slice(&self.cas_read(&evidence.render_set_object_sha256)?).map_err(|error| RuntimeError::InvalidInput(format!("SILHOUETTE_COMPARE_INVALID: {error}")))?;
            validate_render_set_v2_output(&render_set)?;
            let model_mask = decode_binary_mask(&self.render_pass_bytes(&render_set, "silhouette")?)?;
            let part_context = self
                .render_pass_bytes(&render_set, "part-id")
                .ok()
                .and_then(|part_png| {
                    let part_ids = candidate
                        .manifest_hash
                        .as_deref()
                        .and_then(|hash| self.cas_read(hash).ok())
                        .and_then(|bytes| strict_glb_inspection(&bytes).ok())
                        .map(|inspection| inspection.part_ids)?;
                    Some((part_png, part_ids))
                });
            let metrics = extended_silhouette_metrics(&target_mask.mask, &model_mask);
            let loss_metrics = transient_loss_metrics_with_parts(
                &metrics,
                &model_mask,
                target.get("landmarks"),
                part_context
                    .as_ref()
                    .map(|(part_png, part_ids)| (part_png.as_slice(), part_ids.as_slice())),
            );
            let loss = extended_silhouette_loss(&loss_metrics);
            rows.push(json!({"candidate_id":candidate_id,"metrics":metrics,"loss":stable_visual_metric(loss)}));
        }
        rows.sort_by(|left, right| left["loss"].as_f64().unwrap_or(f64::INFINITY).partial_cmp(&right["loss"].as_f64().unwrap_or(f64::INFINITY)).unwrap_or(std::cmp::Ordering::Equal));
        let winner = rows.first().and_then(|row| row.get("candidate_id")).cloned().unwrap_or(Value::Null);
        let tie = rows.len() > 1 && (rows[0]["loss"].as_f64().unwrap_or(0.0) - rows[1]["loss"].as_f64().unwrap_or(0.0)).abs() < 1e-12;
        let first_metrics = rows.first().and_then(|row| row.get("metrics")).cloned().unwrap_or(Value::Null);
        let second_metrics = rows.get(1).and_then(|row| row.get("metrics")).cloned().unwrap_or(Value::Null);
        let mut result = json!({"schema_version":"SilhouetteCandidateCompareResult@1","target_sha256":target_sha256,"candidates":rows,"winner_candidate_id":if tie {Value::Null} else {winner},"delta":{"silhouette_iou":stable_visual_metric(second_metrics["silhouette_iou"].as_f64().unwrap_or(0.0)-first_metrics["silhouette_iou"].as_f64().unwrap_or(0.0)),"boundary_f1_4px":stable_visual_metric(second_metrics["boundary_f1_4px"].as_f64().unwrap_or(0.0)-first_metrics["boundary_f1_4px"].as_f64().unwrap_or(0.0)),"sdf_chamfer_px":stable_visual_metric(second_metrics["sdf_chamfer_px"].as_f64().unwrap_or(0.0)-first_metrics["sdf_chamfer_px"].as_f64().unwrap_or(0.0))},"status":if tie {"tie"} else {"ready"},"canonical_sha256":""});
        result["canonical_sha256"] = Value::String(canonical_json_hash(&result));
        validate_silhouette_candidate_compare_result(&result)?;
        Ok(result)
    }

    fn store_silhouette_target(
        &self,
        project_id: &str,
        reference: &ReferenceEvidenceRecord,
        contour_points: Option<&[[f64; 2]]>,
        landmarks: Value,
        parts: Value,
        automatic: ReferenceMask,
        automatic_source: bool,
    ) -> Result<Value, RuntimeError> {
        let mask = if let Some(points) = contour_points {
            rasterize_contour(points)
        } else {
            automatic.mask.clone()
        };
        let png = mask_to_png(&mask)?;
        let mask_object = self.put_object(&png, None, "image/png", "reference-silhouette-mask-v1")?;
        let points = contour_points
            .map(|points| points.iter().map(|[x, y]| json!([x, y])).collect::<Vec<_>>())
            .unwrap_or_else(|| contour_points_from_mask(&mask));
        if points.len() < 3 {
            return Err(RuntimeError::InvalidInput(
                "REFERENCE_MASK_INVALID: target contour has fewer than three points".to_owned(),
            ));
        }
        validate_target_part_ranges(&parts, points.len(), "REFERENCE_MASK_INVALID")?;
        let mut target = json!({
            "schema_version":"SilhouetteTarget@1",
            "target_id":format!("silhouette-target-{}", Uuid::new_v4().simple()),
            "reference_id":reference.reference_id,
            "reference_sha256":reference.object_sha256,
            "mask_sha256":mask_object.record.sha256,
            "width":512,
            "height":512,
            "coordinate_space":"normalized_reference_image",
            "source":if automatic_source {"automatic"} else {"user_refined"},
            "contour_points":points,
            "parts":parts,
            "landmarks":landmarks,
            "canonical_sha256":""
        });
        // serde_json may normalize a few floating-point spellings while
        // parsing the object back from CAS.  Hash the canonical round-trip
        // representation so an automatically sampled contour remains
        // readable through a fresh IPC process, not only in this stack.
        let mut hash_input = target.clone();
        hash_input["canonical_sha256"] = Value::String(String::new());
        let round_trip_bytes = canonical_json_bytes(&hash_input)
            .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?;
        let round_trip: Value = serde_json::from_slice(&round_trip_bytes)
            .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?;
        target = round_trip;
        target["canonical_sha256"] = Value::String(canonical_json_hash(&target));
        validate_silhouette_target(&target)?;
        let target_bytes = canonical_json_bytes(&target)
            .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?;
        let target_object = self.put_object(&target_bytes, None, "application/json", "silhouette-target-v1")?;
        let mut result = json!({
            "schema_version":"ReferenceMaskPrepareResult@1",
            "project_id":project_id,
            "reference_id":reference.reference_id,
            "target_sha256":target_object.record.sha256,
            "mask_sha256":mask_object.record.sha256,
            "target":target,
            "canonical_sha256":""
        });
        result["canonical_sha256"] = Value::String(canonical_json_hash(&result));
        validate_reference_mask_prepare_result(&result)?;
        Ok(result)
    }

    fn read_silhouette_target(&self, target_sha256: &str) -> Result<Value, RuntimeError> {
        if !forgecad_contracts::is_sha256(target_sha256) {
            return Err(RuntimeError::InvalidInput(
                "REFERENCE_MASK_INVALID: target_sha256 is not a SHA-256".to_owned(),
            ));
        }
        let target: Value = serde_json::from_slice(&self.cas_read(target_sha256)?)
            .map_err(|error| RuntimeError::InvalidInput(format!("REFERENCE_MASK_INVALID: {error}")))?;
        validate_silhouette_target(&target)?;
        Ok(target)
    }

    fn target_mask(&self, target_sha256: &str, target: &Value) -> Result<ReferenceMask, RuntimeError> {
        let _ = target_sha256;
        let mask_sha256 = required_value_sha(target.get("mask_sha256"), "mask_sha256")?;
        let bytes = self.cas_read(mask_sha256)?;
        let mask = decode_binary_mask(&bytes)?;
        Ok(ReferenceMask { mask, png: bytes })
    }

    fn render_pass_bytes(&self, render_set: &Value, pass: &str) -> Result<Vec<u8>, RuntimeError> {
        let hash = render_set
            .get("pass_artifacts")
            .and_then(|value| value.get(pass))
            .and_then(|value| value.get("sha256"))
            .and_then(Value::as_str)
            .ok_or_else(|| RuntimeError::InvalidInput(format!("BOUNDARY_ERROR_INVALID: {pass} pass missing")))?;
        self.cas_read(hash)
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

    pub fn skill_result(&self, skill_id: &str, version: &str) -> Result<Value, String> {
        if !is_opaque_id(skill_id) || !is_opaque_id(version) {
            return Err("invalid Skill identifier".to_owned());
        }
        skill_registry::get_result(skill_id, version)
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
        validate_request_keys(
            object,
            &[
                "project_id",
                "candidate_id",
                "reference_id",
                "view_spec",
                "camera",
                "target_sha256",
            ],
            "reference_compare_prepare",
        )?;
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
        let target_sha256 = object
            .get("target_sha256")
            .map(|value| required_value_sha(Some(value), "target_sha256"))
            .transpose()?
            .map(str::to_owned);
        if let Some(target_sha256) = target_sha256.as_deref() {
            let target = self.read_silhouette_target(target_sha256)?;
            if target.get("reference_id").and_then(Value::as_str) != Some(reference_id) {
                return Err(RuntimeError::InvalidInput(
                    "REFERENCE_SCOPE_DENIED: silhouette target is bound to another reference".to_owned(),
                ));
            }
        }
        let mut camera = match object.get("camera").filter(|value| !value.is_null()) {
            None => default_camera_calibration(),
            Some(value)
                if value.get("schema_version").and_then(Value::as_str)
                    == Some("CameraCalibrationRef@1") => {
                let target_sha256 = target_sha256.as_deref().ok_or_else(|| {
                    RuntimeError::InvalidInput(
                        "CAMERA_CALIBRATION_INVALID: CameraCalibrationRef@1 requires target_sha256".to_owned(),
                    )
                })?;
                self.resolve_silhouette_fit_camera(
                    project_id,
                    candidate_id,
                    target_sha256,
                    value,
                )?
            }
            Some(value) => value.clone(),
        };
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
            let initial_silhouette = render_passes
                .iter()
                .find(|pass| pass.pass == "silhouette")
                .map(|pass| decode_binary_mask(&pass.png))
                .transpose()?;
            if let Some(initial_silhouette) = initial_silhouette {
                // Compare a small deterministic set of framing candidates and
                // keep the one with the best combined silhouette/boundary/
                // extent/centroid score. This prevents a height-only fit from
                // improving one metric while making the overall reference
                // comparison worse. Only the winning render is persisted.
                let mut best_camera = camera.clone();
                let mut best_passes = std::mem::take(&mut render_passes);
                let mut best_score = camera_fit_score(&reference_mask.mask, &initial_silhouette);
                for candidate in [
                    calibrate_default_camera_height_only(
                        &camera,
                        &reference_mask.mask,
                        &initial_silhouette,
                    ),
                    calibrate_default_camera(&camera, &reference_mask.mask, &initial_silhouette),
                ] {
                    if candidate == camera {
                        continue;
                    }
                    validate_camera_calibration(&candidate)?;
                    let candidate_passes = render_glb_with_runtime_worker(&glb, &candidate)
                        .map_err(|error| {
                            RuntimeError::InvalidInput(format!("RENDER_REJECTED: {error}"))
                        })?;
                    let candidate_silhouette = candidate_passes
                        .iter()
                        .find(|pass| pass.pass == "silhouette")
                        .map(|pass| decode_binary_mask(&pass.png))
                        .transpose()?
                        .ok_or_else(|| {
                            RuntimeError::InvalidInput(
                                "RENDER_REJECTED: calibrated silhouette pass missing".to_owned(),
                            )
                        })?;
                    let score = camera_fit_score(&reference_mask.mask, &candidate_silhouette);
                    if score > best_score {
                        best_score = score;
                        best_camera = candidate;
                        best_passes = candidate_passes;
                    }
                }
                camera = best_camera;
                render_passes = best_passes;
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
            // reference_mask_prepare may already have admitted these exact
            // deterministic bytes. Reuse its stable CAS kind so the later
            // compare stage remains idempotent instead of colliding on
            // metadata for the same content hash.
            "reference-silhouette-mask-v1",
        )?;
        let model_mask = pass_bytes.get("silhouette").ok_or_else(|| {
            RuntimeError::InvalidInput("RENDER_REJECTED: silhouette pass missing".to_owned())
        })?;
        let metrics = compare_masks_with_parts(
            &reference_mask.mask,
            &decode_binary_mask(model_mask)?,
            view_spec,
            pass_bytes
                .get("part-id")
                .map(|bytes| (bytes.as_slice(), inspection.part_ids.as_slice())),
        );
        let visual_status = if visible_view_gate_passes(&metrics) {
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
            target_sha256: target_sha256.clone(),
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
        let candidate_artifact_sha256 = candidate
            .prepared_object_sha256
            .as_deref()
            .or(candidate.manifest_hash.as_deref())
            .ok_or_else(|| {
                RuntimeError::InvalidInput(
                    "VISUAL_EVIDENCE_UNAVAILABLE: candidate artifact is missing".to_owned(),
                )
            })?;
        if !forgecad_contracts::is_sha256(candidate_artifact_sha256)
            || candidate
                .prepared_object_sha256
                .as_deref()
                .is_some_and(|hash| !forgecad_contracts::is_sha256(hash))
            || candidate
                .manifest_hash
                .as_deref()
                .is_some_and(|hash| !forgecad_contracts::is_sha256(hash))
            || candidate
                .prepared_object_sha256
                .as_deref()
                .zip(candidate.manifest_hash.as_deref())
                .is_some_and(|(prepared, manifest)| prepared != manifest)
            || render_set.get("artifact_sha256").and_then(Value::as_str)
                != Some(candidate_artifact_sha256)
        {
            return Err(RuntimeError::InvalidInput(
                "VISUAL_EVIDENCE_BINDING_MISMATCH: RenderSet artifact differs from candidate"
                    .to_owned(),
            ));
        }
        if let Some(target_sha256) = evidence.target_sha256.as_deref() {
            let target = self.read_silhouette_target(target_sha256)?;
            if target.get("reference_id").and_then(Value::as_str)
                != Some(evidence.reference_id.as_str())
                || target.get("reference_sha256").and_then(Value::as_str)
                    != Some(reference.object_sha256.as_str())
            {
                return Err(RuntimeError::InvalidInput(
                    "VISUAL_EVIDENCE_BINDING_MISMATCH: silhouette target differs from reference"
                        .to_owned(),
                ));
            }
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
                || report.get("artifact_sha256").and_then(Value::as_str)
                    != Some(candidate_artifact_sha256)
                || report.get("reference_sha256").and_then(Value::as_str)
                    != Some(reference.object_sha256.as_str())
                || report.get("render_set_hash").and_then(Value::as_str)
                    != Some(evidence.render_set_object_sha256.as_str())
                || report.get("camera_hash") != render_set.get("camera_hash")
            {
                return Err(RuntimeError::InvalidInput(
                    "VISUAL_EVIDENCE_BINDING_MISMATCH: comparison report lineage differs"
                        .to_owned(),
                ));
            }
            Some(report)
        } else {
            return Err(RuntimeError::InvalidInput(
                "VISUAL_EVIDENCE_UNAVAILABLE: comparison report is missing".to_owned(),
            ));
        };
        let quality_report = self.quality(candidate_id, Some(&evidence.reference_id))?;
        validate_quality_report_v2_output(&quality_report)?;
        if quality_report.get("candidate_id").and_then(Value::as_str) != Some(candidate_id)
            || quality_report.get("artifact_sha256").and_then(Value::as_str)
                != Some(candidate_artifact_sha256)
            || quality_report.get("reference_id").and_then(Value::as_str)
                != Some(evidence.reference_id.as_str())
            || quality_report.get("reference_sha256").and_then(Value::as_str)
                != Some(reference.object_sha256.as_str())
            || quality_report.get("render_set_hash").and_then(Value::as_str)
                != Some(evidence.render_set_object_sha256.as_str())
            || quality_report.get("comparison_report_hash").and_then(Value::as_str)
                != evidence.comparison_report_object_sha256.as_deref()
        {
            return Err(RuntimeError::InvalidInput(
                "VISUAL_EVIDENCE_BINDING_MISMATCH: QualityReport lineage differs".to_owned(),
            ));
        }
        Ok(json!({
            "schema_version":"ViewerVisualEvidence@1",
            "candidate_id":candidate_id,
            "project_id":evidence.project_id,
            "reference_id":evidence.reference_id,
            "target_sha256":evidence.target_sha256,
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
        let appearance_program = object.get("appearance_program").ok_or_else(|| {
            RuntimeError::InvalidInput("appearance_program is required".to_owned())
        })?;
        let geometry_is_v2 = geometry_program
            .get("schema_version")
            .and_then(Value::as_str)
            == Some("GeometryProgram@2");
        let appearance_is_v2 = appearance_program
            .get("schema_version")
            .and_then(Value::as_str)
            == Some("AppearanceProgram@2");
        if geometry_is_v2 != appearance_is_v2 {
            return Err(RuntimeError::InvalidInput(
                "APPEARANCE_REJECTED: GeometryProgram@2 must use AppearanceProgram@2 and the legacy V1 pair must remain V1".to_owned(),
            ));
        }
        let artifact =
            compile_geometry_with_runtime_worker(geometry_program, Some(appearance_program))
                .map_err(|error| {
                    RuntimeError::InvalidInput(format!("APPEARANCE_REJECTED: {error}"))
                })?;
        if appearance_is_v2 {
            return self.prepare_appearance_candidate_v2(
                project_id,
                base_version_id,
                request.clone(),
                artifact,
            );
        }
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

    fn prepare_appearance_candidate_v2(
        &self,
        project_id: &str,
        base_version_id: Option<&str>,
        request: Value,
        artifact: geometry_worker::GeometryArtifact,
    ) -> Result<Value, RuntimeError> {
        let reference_id = request
            .get("reference_id")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                RuntimeError::InvalidInput(
                    "APPEARANCE_V2_REFERENCE_REQUIRED: supply reference_id for RenderSet@2 binding"
                        .to_owned(),
                )
            })?;
        let reference = self.reference(reference_id)?.ok_or_else(|| {
            RuntimeError::InvalidInput("NOT_FOUND: appearance reference not found".to_owned())
        })?;
        if reference.project_id != project_id {
            return Err(RuntimeError::InvalidInput(
                "REFERENCE_SCOPE_DENIED: appearance reference is outside the target project"
                    .to_owned(),
            ));
        }
        let inspection = strict_glb_inspection(&artifact.glb)?;
        validate_worker_metadata(&artifact, &inspection)?;
        if !inspection.hard_gate_passed {
            return Err(RuntimeError::InvalidInput(format!(
                "APPEARANCE_REJECTED: physical GLB readback failed: {}",
                inspection.failure_codes.join(",")
            )));
        }
        let glb_object = self.put_object(
            &artifact.glb,
            None,
            "model/gltf-binary",
            "appearance-v2-glb",
        )?;
        let prepared = self.prepare_candidate(
            project_id,
            base_version_id,
            &format!("appearance-v2-object-{}", &glb_object.record.sha256[..32]),
            &glb_object.record.sha256,
            request.clone(),
        )?;
        let readback = artifact_readback_v2_value(
            &glb_object.record.sha256,
            &prepared.candidate.candidate_id,
            &inspection,
            glb_object.record.size_bytes,
        );
        validate_artifact_readback_v2_output(&readback)?;
        let _readback_object = self.put_object(
            &canonical_json_bytes(&readback)
                .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?,
            None,
            "application/json",
            "appearance-v2-artifact-readback",
        )?;

        let camera = default_camera_calibration();
        let render_passes = render_glb_with_runtime_worker(&artifact.glb, &camera)
            .map_err(|error| RuntimeError::InvalidInput(format!("RENDER_REJECTED: {error}")))?;
        let mut pass_artifacts = serde_json::Map::new();
        for pass in &render_passes {
            let object = self.put_object(
                &pass.png,
                None,
                "image/png",
                &format!("appearance-v2-render-{}", pass.pass),
            )?;
            let color_space = match pass.pass.as_str() {
                "depth" | "normal" | "ao" | "uv-stretch" => "linear",
                "part-id" | "material-id" | "silhouette" | "wireframe" => "data",
                _ => "srgb",
            };
            pass_artifacts.insert(
                pass.pass.clone(),
                json!({"sha256":object.record.sha256,"mime":"image/png","size_bytes":object.record.size_bytes,"width":512,"height":512,"channels":"rgba8","color_space":color_space}),
            );
        }
        let camera_hash = canonical_json_hash(&camera);
        let renderer_hash = sha256_hex(b"forgecad-renderer-2");
        let mut render_set = json!({
            "schema_version":"RenderSet@2",
            "render_set_id":format!("render-set-v2-{}", &glb_object.record.sha256[..32]),
            "candidate_id":prepared.candidate.candidate_id,
            "artifact_sha256":glb_object.record.sha256,
            "program_sha256":inspection.program_sha256,
            "reference_id":reference_id,
            "camera_hash":camera_hash,
            "renderer_hash":renderer_hash,
            "width":512,
            "height":512,
            "passes":render_passes.iter().map(|pass| pass.pass.clone()).collect::<Vec<_>>(),
            "pass_artifacts":pass_artifacts,
            "canonical_sha256":""
        });
        render_set["canonical_sha256"] = Value::String(canonical_json_hash(&render_set));
        validate_render_set_v2_output(&render_set)?;
        let render_set_object = self.put_object(
            &canonical_json_bytes(&render_set)
                .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?,
            None,
            "application/json",
            "appearance-v2-render-set",
        )?;

        let manifest = material_pack_manifest();
        let manifest_hash = manifest
            .get("canonical_sha256")
            .and_then(Value::as_str)
            .ok_or_else(|| RuntimeError::InvalidInput("material pack manifest hash is missing".to_owned()))?;
        let _manifest_object = self.put_object(
            &canonical_json_bytes(&manifest)
                .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?,
            None,
            "application/json",
            "material-pack-manifest",
        )?;
        let receipt = json!({
            "schema_version":"TextureBuildReceipt@1",
            "pack_id":"forgecad-hard-surface-robot",
            "pack_version":"1.0.0",
            "recipe_id":"forgecad-hard-surface-robot-512-png@1",
            "toolchain":"forgecad-geometry-worker embedded offline PNG pack",
            "source_archive_policy":"raw archives remain in the local adoption cache",
            "color_management":"baseColor/emissive sRGB; normal/metallic/roughness/AO linear; OpenGL +Y",
            "external_uri":false,
            "network_at_runtime":false,
            "canonical_sha256":""
        });
        let mut receipt = receipt;
        receipt["canonical_sha256"] = Value::String(canonical_json_hash(&receipt));
        let receipt_object = self.put_object(
            &canonical_json_bytes(&receipt)
                .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?,
            None,
            "application/json",
            "texture-build-receipt",
        )?;
        let quality_id = format!("quality-appearance-v2-{}", &glb_object.record.sha256[..24]);
        let mut quality = json!({
            "schema_version":"QualityReport@2",
            "quality_report_id":quality_id,
            "candidate_id":prepared.candidate.candidate_id,
            "artifact_sha256":glb_object.record.sha256,
            "program_sha256":inspection.program_sha256,
            "reference_id":reference_id,
            "reference_sha256":reference.object_sha256,
            "render_set_hash":render_set_object.record.sha256,
            "comparison_report_hash":sha256_hex(b"reference-comparison-not-run"),
            "human_receipt_hash":Value::Null,
            "structural_status":"passed",
            "visual_status":"not-run",
            "hard_gate_passed":true,
            "limitations":["MCP010E material/UV/PBR structural gate; reference comparison and human visual review are separate MCP010C evidence."],
            "canonical_sha256":""
        });
        quality["canonical_sha256"] = Value::String(canonical_json_hash(&quality));
        validate_quality_report_v2_output(&quality)?;
        let quality_object = self.put_object(
            &canonical_json_bytes(&quality)
                .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?,
            None,
            "application/json",
            "appearance-v2-quality-report",
        )?;
        let candidate = self.mark_candidate_quality(
            &prepared.candidate.candidate_id,
            quality.get("quality_report_id").and_then(Value::as_str).unwrap_or("quality-appearance-v2"),
            true,
        )?;
        let result = json!({
            "schema_version":"AppearancePrepareResult@2",
            "candidate":candidate,
            "job":prepared.job,
            "artifact":readback,
            "render_set":render_set,
            "render_set_object_sha256":render_set_object.record.sha256,
            "quality_report_object_sha256":quality_object.record.sha256,
            "material_pack_manifest_sha256":manifest_hash,
            "texture_build_receipt_sha256":receipt_object.record.sha256
        });
        validate_appearance_prepare_result_v2_output(&result)?;
        Ok(result)
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
        self.revalidate_visual_evidence_for_confirmation(&candidate)?;
        Ok(self.store.confirm_candidate(request, &now_string())?)
    }

    /// If a candidate has entered the MCP010C visual-review path, confirmation
    /// must consume the same candidate-bound QualityReport@2 rather than
    /// falling back to the older geometry-only quality flag.  A failed visual
    /// target may remain reviewable for Codex revision, but it must not mint an
    /// immutable version.  Human evidence is optional for the MVP approval
    /// boundary; when present, an explicit `approved:false` is authoritative.
    fn revalidate_visual_evidence_for_confirmation(
        &self,
        candidate: &CandidateRecord,
    ) -> Result<(), RuntimeError> {
        let Some(evidence) = self.store.get_visual_evidence(&candidate.candidate_id)? else {
            return Ok(());
        };
        if evidence.project_id != candidate.project_id {
            return Err(RuntimeError::InvalidInput(
                "QUALITY_HARD_GATE_FAILED: visual evidence project binding drifted".to_owned(),
            ));
        }
        let quality: Value = serde_json::from_slice(&self.cas_read(
            &evidence.quality_report_object_sha256,
        )?)
        .map_err(|error| {
            RuntimeError::InvalidInput(format!(
                "QUALITY_HARD_GATE_FAILED: visual quality report is invalid: {error}"
            ))
        })?;
        validate_quality_report_v2_output(&quality)?;
        if quality.get("candidate_id").and_then(Value::as_str)
            != Some(candidate.candidate_id.as_str())
            || quality.get("artifact_sha256").and_then(Value::as_str)
                != candidate.prepared_object_sha256.as_deref()
            || quality.get("render_set_hash").and_then(Value::as_str)
                != Some(evidence.render_set_object_sha256.as_str())
            || quality.get("comparison_report_hash").and_then(Value::as_str)
                != evidence.comparison_report_object_sha256.as_deref()
        {
            return Err(RuntimeError::InvalidInput(
                "QUALITY_HARD_GATE_FAILED: visual quality report is not candidate-bound"
                    .to_owned(),
            ));
        }
        if quality.get("hard_gate_passed").and_then(Value::as_bool) != Some(true) {
            return Err(RuntimeError::InvalidInput(
                "QUALITY_TARGET_NOT_MET: candidate-bound visual quality gate has not passed"
                    .to_owned(),
            ));
        }
        if let Some(human_sha256) = evidence.human_receipt_object_sha256.as_deref() {
            let receipt: Value = serde_json::from_slice(&self.cas_read(human_sha256)?).map_err(
                |error| {
                    RuntimeError::InvalidInput(format!(
                        "HUMAN_REVIEW_INVALID: receipt is not valid JSON: {error}"
                    ))
                },
            )?;
            validate_human_review_receipt(&receipt)?;
            if receipt.get("candidate_id").and_then(Value::as_str)
                != Some(candidate.candidate_id.as_str())
                || receipt.get("reference_id").and_then(Value::as_str)
                    != Some(evidence.reference_id.as_str())
                || receipt.get("render_set_hash").and_then(Value::as_str)
                    != Some(evidence.render_set_object_sha256.as_str())
                || receipt.get("comparison_report_hash").and_then(Value::as_str)
                    != evidence.comparison_report_object_sha256.as_deref()
            {
                return Err(RuntimeError::InvalidInput(
                    "HUMAN_REVIEW_BINDING_MISMATCH: receipt is not candidate-bound".to_owned(),
                ));
            }
            if receipt.get("approved").and_then(Value::as_bool) != Some(true) {
                return Err(RuntimeError::InvalidInput(
                    "HUMAN_REVIEW_NOT_APPROVED: candidate requires an approved visual review"
                        .to_owned(),
                ));
            }
        }
        Ok(())
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
            "agentic_scene_observe" => {
                let project_id = payload
                    .get("project_id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| RuntimeError::InvalidInput("project_id is required".to_owned()))?;
                self.agentic_scene_observe(
                    project_id,
                    payload.get("candidate_id").and_then(Value::as_str),
                )
            }
            "agentic_stage_plan" => {
                let project_id = payload
                    .get("project_id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| RuntimeError::InvalidInput("project_id is required".to_owned()))?;
                self.agentic_stage_plan(
                    project_id,
                    payload.get("candidate_id").and_then(Value::as_str),
                )
            }
            "agentic_critic_projection" => {
                let project_id = payload
                    .get("project_id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| RuntimeError::InvalidInput("project_id is required".to_owned()))?;
                self.agentic_critic_projection(
                    project_id,
                    payload.get("candidate_id").and_then(Value::as_str),
                )
            }
            "agentic_session_lookup" => {
                let project_id = payload
                    .get("project_id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| RuntimeError::InvalidInput("project_id is required".to_owned()))?;
                let candidate_id = payload
                    .get("candidate_id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| RuntimeError::InvalidInput("candidate_id is required".to_owned()))?;
                self.agentic_session_lookup(project_id, candidate_id)
            }
            "session_create_or_resume"
            | "session_get"
            | "checkpoint_prepare"
            | "checkpoint_get"
            | "checkpoint_restore_prepare"
            | "design_action_run_prepare"
            | "design_action_run_get" => match method {
                "session_create_or_resume" => self.session_create_or_resume(payload.clone()),
                "session_get" => self.session_get(payload.clone()),
                "checkpoint_prepare" => self.checkpoint_prepare(payload.clone()),
                "checkpoint_get" => self.checkpoint_get(payload.clone()),
                "checkpoint_restore_prepare" => self.checkpoint_restore_prepare(payload.clone()),
                "design_action_run_prepare" => self.design_action_run_prepare(payload.clone()),
                "design_action_run_get" => self.design_action_run_get(payload.clone()),
                _ => unreachable!("Agentic session IPC method dispatch arm is exhaustive"),
            },
            "capabilities_get" => Ok(serde_json::to_value(self.capabilities())
                .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?),
            "operator_catalog_get" => Ok(self.active_operator_catalog()),
            "material_pack_get" => Ok(self.material_pack_manifest()),
            "geometry_program_hash" => self.geometry_program_hash(payload),
            "silhouette_rig_hash" => {
                let project_id = payload.get("project_id").and_then(Value::as_str).ok_or_else(|| RuntimeError::InvalidInput("project_id is required".to_owned()))?;
                self.silhouette_rig_hash(project_id, payload)
            }
            "silhouette_target_get" => {
                let target_sha256 = payload
                    .get("target_sha256")
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        RuntimeError::InvalidInput("target_sha256 is required".to_owned())
                    })?;
                self.silhouette_target_get(target_sha256)
            }
            "boundary_error_get" => {
                let candidate_id = payload
                    .get("candidate_id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        RuntimeError::InvalidInput("candidate_id is required".to_owned())
                    })?;
                let target_sha256 = payload
                    .get("target_sha256")
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        RuntimeError::InvalidInput("target_sha256 is required".to_owned())
                    })?;
                self.boundary_error(
                    candidate_id,
                    target_sha256,
                    payload.get("max_segments").and_then(Value::as_u64),
                )
            }
            "silhouette_part_error_get" => {
                let project_id = payload
                    .get("project_id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        RuntimeError::InvalidInput("project_id is required".to_owned())
                    })?;
                self.silhouette_part_error(project_id, payload.clone())
            }
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
            "reference_mask_prepare" => {
                let project_id = payload
                    .get("project_id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        RuntimeError::InvalidInput("project_id is required".to_owned())
                    })?;
                self.prepare_reference_mask(project_id, payload.clone())
            }
            "reference_mask_refine_prepare" => {
                let project_id = payload
                    .get("project_id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        RuntimeError::InvalidInput("project_id is required".to_owned())
                    })?;
                self.refine_reference_mask(project_id, payload.clone())
            }
            "camera_fit_prepare" => {
                let project_id = payload
                    .get("project_id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        RuntimeError::InvalidInput("project_id is required".to_owned())
                    })?;
                self.prepare_camera_fit(project_id, payload.clone())
            }
            "silhouette_fit_prepare" => {
                let project_id = payload
                    .get("project_id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| RuntimeError::InvalidInput("project_id is required".to_owned()))?;
                self.silhouette_fit_prepare(project_id, payload.clone())
            }
            "primary_form_repair_prepare" => {
                let project_id = payload
                    .get("project_id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| RuntimeError::InvalidInput("project_id is required".to_owned()))?;
                let base_version_id = payload.get("base_version_id").and_then(Value::as_str);
                self.primary_form_repair_prepare(project_id, base_version_id, payload.clone())
            }
            "part_contour_fit_prepare" => {
                let project_id = payload
                    .get("project_id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| RuntimeError::InvalidInput("project_id is required".to_owned()))?;
                self.part_contour_fit_prepare(project_id, payload.clone())
            }
            "silhouette_candidate_compare" => {
                let project_id = payload
                    .get("project_id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| RuntimeError::InvalidInput("project_id is required".to_owned()))?;
                self.silhouette_candidate_compare(project_id, payload.clone())
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

fn validate_request_keys(
    object: &serde_json::Map<String, Value>,
    allowed: &[&str],
    context: &str,
) -> Result<(), RuntimeError> {
    if object.keys().any(|key| !allowed.contains(&key.as_str())) {
        return Err(RuntimeError::InvalidInput(format!(
            "INVALID_INPUT: {context} contains an unknown field"
        )));
    }
    Ok(())
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

/// Preserve the original height-only framing candidate so the bounded search
/// can compare it with the width/centroid candidate below. This is deliberately
/// kept separate from explicit CameraCalibration input.
fn calibrate_default_camera_height_only(
    camera: &Value,
    reference: &[bool],
    model: &[bool],
) -> Value {
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
    let reference_width = (reference_bbox.2 - reference_bbox.0 + 1) as f64;
    let model_width = (model_bbox.2 - model_bbox.0 + 1) as f64;
    if reference_height <= 0.0
        || model_height <= 0.0
        || reference_width <= 0.0
        || model_width <= 0.0
    {
        return camera.clone();
    }
    // Fit the larger normalized extent so the framing cannot improve one
    // axis while pushing the other outside the reference crop.
    let scale = (model_height / reference_height)
        .max(model_width / reference_width)
        .clamp(0.55, 1.45);
    let Some(transform) = camera.get("transform").and_then(Value::as_object) else {
        return camera.clone();
    };
    let Some(position) = camera_vec3(transform.get("position_m")) else {
        return camera.clone();
    };
    let Some(target) = camera_vec3(transform.get("target_m")) else {
        return camera.clone();
    };
    let Some(up_input) = camera_vec3(transform.get("up")) else {
        return camera.clone();
    };
    let Some(fov_y_degrees) = camera.get("fov_y_degrees").and_then(Value::as_f64) else {
        return camera.clone();
    };
    let view = [
        target[0] - position[0],
        target[1] - position[1],
        target[2] - position[2],
    ];
    let view_length = (view[0] * view[0] + view[1] * view[1] + view[2] * view[2]).sqrt();
    if !view_length.is_finite() || view_length <= f64::EPSILON {
        return camera.clone();
    }
    let forward = [
        view[0] / view_length,
        view[1] / view_length,
        view[2] / view_length,
    ];
    let right_raw = [
        forward[1] * up_input[2] - forward[2] * up_input[1],
        forward[2] * up_input[0] - forward[0] * up_input[2],
        forward[0] * up_input[1] - forward[1] * up_input[0],
    ];
    let right_length =
        (right_raw[0] * right_raw[0] + right_raw[1] * right_raw[1] + right_raw[2] * right_raw[2])
            .sqrt();
    if !right_length.is_finite() || right_length <= f64::EPSILON {
        return camera.clone();
    }
    let right = [
        right_raw[0] / right_length,
        right_raw[1] / right_length,
        right_raw[2] / right_length,
    ];
    let up = [
        right[1] * forward[2] - right[2] * forward[1],
        right[2] * forward[0] - right[0] * forward[2],
        right[0] * forward[1] - right[1] * forward[0],
    ];
    let up_length = (up[0] * up[0] + up[1] * up[1] + up[2] * up[2]).sqrt();
    if !up_length.is_finite() || up_length <= f64::EPSILON {
        return camera.clone();
    }
    let up = [up[0] / up_length, up[1] / up_length, up[2] / up_length];
    let fov_half = (fov_y_degrees.to_radians() * 0.5).tan();
    if !fov_half.is_finite() || fov_half <= 0.0 {
        return camera.clone();
    }
    let adjusted_position = [
        target[0] + (position[0] - target[0]) * scale,
        target[1] + (position[1] - target[1]) * scale,
        target[2] + (position[2] - target[2]) * scale,
    ];
    let reference_center = [
        (reference_bbox.0 + reference_bbox.2) as f64 * 0.5,
        (reference_bbox.1 + reference_bbox.3) as f64 * 0.5,
    ];
    let model_center = [
        (model_bbox.0 + model_bbox.2) as f64 * 0.5,
        (model_bbox.1 + model_bbox.3) as f64 * 0.5,
    ];
    let delta_x = ((reference_center[0] - model_center[0]) / 512.0).clamp(-0.25, 0.25);
    let delta_y = ((reference_center[1] - model_center[1]) / 512.0).clamp(-0.25, 0.25);
    let adjusted_distance = view_length * scale;
    let half_height = adjusted_distance * fov_half;
    // C uses a square 512x512 render target.
    let half_width = half_height;
    // Shift the camera and target together, preserving the view ray while
    // moving the model toward the reference silhouette centroid.
    let camera_shift = [
        -right[0] * delta_x * half_width * 2.0 + up[0] * delta_y * half_height * 2.0,
        -right[1] * delta_x * half_width * 2.0 + up[1] * delta_y * half_height * 2.0,
        -right[2] * delta_x * half_width * 2.0 + up[2] * delta_y * half_height * 2.0,
    ];
    let adjusted_target = [
        target[0] + camera_shift[0],
        target[1] + camera_shift[1],
        target[2] + camera_shift[2],
    ];
    let adjusted_position = [
        adjusted_position[0] + camera_shift[0],
        adjusted_position[1] + camera_shift[1],
        adjusted_position[2] + camera_shift[2],
    ];
    let mut calibrated = camera.clone();
    let Some(calibrated_transform) = calibrated
        .get_mut("transform")
        .and_then(Value::as_object_mut)
    else {
        return camera.clone();
    };
    calibrated_transform.insert("position_m".to_owned(), json!(adjusted_position));
    calibrated_transform.insert("target_m".to_owned(), json!(adjusted_target));
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

/// Return the world-meter span represented by one normalized camera-plane
/// coordinate.  The fixed renderer uses a square target, so the horizontal
/// and vertical spans share the calibrated perspective distance/FOV.  Keeping
/// this conversion in Runtime prevents the Rig from treating image pixels as
/// arbitrary world-axis translations.
fn camera_plane_world_scales(camera: &Value) -> Option<(f64, f64)> {
    let transform = camera.get("transform").and_then(Value::as_object)?;
    let position = camera_vec3(transform.get("position_m"))?;
    let target = camera_vec3(transform.get("target_m"))?;
    let up_input = camera_vec3(transform.get("up"))?;
    let fov_y_degrees = camera.get("fov_y_degrees").and_then(Value::as_f64)?;
    let view = [
        target[0] - position[0],
        target[1] - position[1],
        target[2] - position[2],
    ];
    let view_length = (view[0] * view[0] + view[1] * view[1] + view[2] * view[2]).sqrt();
    let forward = [
        view[0] / view_length.max(1e-9),
        view[1] / view_length.max(1e-9),
        view[2] / view_length.max(1e-9),
    ];
    let right_raw = [
        forward[1] * up_input[2] - forward[2] * up_input[1],
        forward[2] * up_input[0] - forward[0] * up_input[2],
        forward[0] * up_input[1] - forward[1] * up_input[0],
    ];
    let right_length =
        (right_raw[0] * right_raw[0] + right_raw[1] * right_raw[1] + right_raw[2] * right_raw[2])
            .sqrt();
    let fov_half = (fov_y_degrees.to_radians() * 0.5).tan();
    if !view_length.is_finite()
        || view_length <= f64::EPSILON
        || !right_length.is_finite()
        || right_length <= f64::EPSILON
        || !fov_half.is_finite()
        || fov_half <= 0.0
    {
        return None;
    }
    let half_height = view_length * fov_half;
    let half_width = half_height;
    Some((half_width * 2.0, half_height * 2.0))
}

fn camera_plane_axes(camera: &Value) -> Option<([f64; 3], [f64; 3])> {
    let transform = camera.get("transform").and_then(Value::as_object)?;
    let position = camera_vec3(transform.get("position_m"))?;
    let target = camera_vec3(transform.get("target_m"))?;
    let up_input = camera_vec3(transform.get("up"))?;
    let view = [
        target[0] - position[0],
        target[1] - position[1],
        target[2] - position[2],
    ];
    let view_length = (view[0] * view[0] + view[1] * view[1] + view[2] * view[2]).sqrt();
    if !view_length.is_finite() || view_length <= f64::EPSILON {
        return None;
    }
    let forward = [
        view[0] / view_length,
        view[1] / view_length,
        view[2] / view_length,
    ];
    let right_raw = [
        forward[1] * up_input[2] - forward[2] * up_input[1],
        forward[2] * up_input[0] - forward[0] * up_input[2],
        forward[0] * up_input[1] - forward[1] * up_input[0],
    ];
    let right_length =
        (right_raw[0] * right_raw[0] + right_raw[1] * right_raw[1] + right_raw[2] * right_raw[2])
            .sqrt();
    if !right_length.is_finite() || right_length <= f64::EPSILON {
        return None;
    }
    let right = [
        right_raw[0] / right_length,
        right_raw[1] / right_length,
        right_raw[2] / right_length,
    ];
    let up_raw = [
        right[1] * forward[2] - right[2] * forward[1],
        right[2] * forward[0] - right[0] * forward[2],
        right[0] * forward[1] - right[1] * forward[0],
    ];
    let up_length = (up_raw[0] * up_raw[0] + up_raw[1] * up_raw[1] + up_raw[2] * up_raw[2]).sqrt();
    if !up_length.is_finite() || up_length <= f64::EPSILON {
        return None;
    }
    Some((right, [up_raw[0] / up_length, up_raw[1] / up_length, up_raw[2] / up_length]))
}

fn camera_fit_score(reference: &[bool], model: &[bool]) -> f64 {
    if reference.len() != 512 * 512 || model.len() != 512 * 512 {
        return f64::NEG_INFINITY;
    }
    let metrics = compare_masks(reference, model, &json!({"landmarks":[],"regions":[]}));
    let silhouette = metrics
        .get("silhouette_iou")
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    let boundary = metrics
        .get("boundary_f1_4px")
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    let bbox = metrics
        .get("bbox_edge_error")
        .and_then(Value::as_f64)
        .unwrap_or(1.0);
    let centroid = metrics
        .get("centroid_error")
        .and_then(Value::as_f64)
        .unwrap_or(1.0);
    // Equal bounded contributions keep a framing candidate from winning on
    // IoU alone while drifting the visible centroid or crop edges.
    silhouette + boundary + (1.0 - bbox) + (1.0 - centroid)
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
    verify_camera_canonical_hash(value)
}

fn verify_camera_canonical_hash(value: &Value) -> Result<(), RuntimeError> {
    let object = value.as_object().ok_or_else(|| {
        RuntimeError::InvalidInput(
            "CONTRACT_OUTPUT_INVALID: CameraCalibration@1 must be an object".to_owned(),
        )
    })?;
    let actual = object
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            RuntimeError::InvalidInput(
                "CONTRACT_OUTPUT_INVALID: CameraCalibration@1.canonical_sha256 missing"
                    .to_owned(),
            )
        })?;
    let mut input = value.clone();
    input["canonical_sha256"] = Value::String(String::new());
    if canonical_json_hash(&input) == actual
        || canonical_json_hash(&normalize_camera_numbers(&input)) == actual
    {
        return Ok(());
    }
    Err(RuntimeError::InvalidInput(
        "CONTRACT_OUTPUT_INVALID: CameraCalibration@1.canonical_sha256 does not bind the payload"
            .to_owned(),
    ))
}

fn normalize_camera_numbers(value: &Value) -> Value {
    match value {
        Value::Object(object) => Value::Object(
            object
                .iter()
                .map(|(key, child)| {
                    let normalized = match key.as_str() {
                        "resolution" => child.clone(),
                        "transform" => normalize_continuous_object(child),
                        "fov_y_degrees" | "near_m" | "far_m" => normalize_json_numbers(child),
                        _ => child.clone(),
                    };
                    (key.clone(), normalized)
                })
                .collect(),
        ),
        _ => value.clone(),
    }
}

fn normalize_continuous_object(value: &Value) -> Value {
    match value {
        Value::Object(object) => Value::Object(
            object
                .iter()
                .map(|(key, child)| (key.clone(), normalize_json_numbers(child)))
                .collect(),
        ),
        _ => value.clone(),
    }
}

fn validate_silhouette_target(value: &Value) -> Result<(), RuntimeError> {
    let object = exact_object(
        value,
        &[
            "schema_version",
            "target_id",
            "reference_id",
            "reference_sha256",
            "mask_sha256",
            "width",
            "height",
            "coordinate_space",
            "source",
            "contour_points",
            "parts",
            "landmarks",
            "canonical_sha256",
        ],
        "SilhouetteTarget@1",
    )?;
    if object.get("schema_version").and_then(Value::as_str) != Some("SilhouetteTarget@1")
        || object.get("width").and_then(Value::as_u64) != Some(512)
        || object.get("height").and_then(Value::as_u64) != Some(512)
        || object.get("coordinate_space").and_then(Value::as_str)
            != Some("normalized_reference_image")
        || !matches!(object.get("source").and_then(Value::as_str), Some("automatic" | "user_refined"))
    {
        return Err(RuntimeError::InvalidInput(
            "CONTRACT_OUTPUT_INVALID: SilhouetteTarget@1 constants drifted".to_owned(),
        ));
    }
    let target_id = required_contract_identifier(object, "target_id", "SilhouetteTarget@1")?;
    if !target_id.starts_with("silhouette-target-") {
        return Err(RuntimeError::InvalidInput("CONTRACT_OUTPUT_INVALID: SilhouetteTarget@1.target_id prefix".to_owned()));
    }
    required_contract_identifier(object, "reference_id", "SilhouetteTarget@1")?;
    required_contract_sha256(object, "reference_sha256", "SilhouetteTarget@1")?;
    required_contract_sha256(object, "mask_sha256", "SilhouetteTarget@1")?;
    let contour = object.get("contour_points").and_then(Value::as_array).ok_or_else(|| {
        RuntimeError::InvalidInput("CONTRACT_OUTPUT_INVALID: SilhouetteTarget@1.contour_points".to_owned())
    })?;
    if !(3..=512).contains(&contour.len()) {
        return Err(RuntimeError::InvalidInput("CONTRACT_OUTPUT_INVALID: contour point count".to_owned()));
    }
    for point in contour {
        let values = point.as_array().ok_or_else(|| RuntimeError::InvalidInput("CONTRACT_OUTPUT_INVALID: contour point".to_owned()))?;
        if values.len() != 2 || values.iter().any(|value| value.as_f64().is_none_or(|number| !number.is_finite() || !(0.0..=1.0).contains(&number))) {
            return Err(RuntimeError::InvalidInput("CONTRACT_OUTPUT_INVALID: contour point coordinates".to_owned()));
        }
    }
    let parts = object.get("parts").and_then(Value::as_array).ok_or_else(|| RuntimeError::InvalidInput("CONTRACT_OUTPUT_INVALID: target parts".to_owned()))?;
    if parts.len() > 64 {
        return Err(RuntimeError::InvalidInput("CONTRACT_OUTPUT_INVALID: target annotation limits".to_owned()));
    }
    validate_target_part_ranges(&Value::Array(parts.clone()), contour.len(), "CONTRACT_OUTPUT_INVALID")?;
    let landmarks = object.get("landmarks").and_then(Value::as_array).ok_or_else(|| RuntimeError::InvalidInput("CONTRACT_OUTPUT_INVALID: target landmarks".to_owned()))?;
    if landmarks.len() > 128 {
        return Err(RuntimeError::InvalidInput("CONTRACT_OUTPUT_INVALID: target annotation limits".to_owned()));
    }
    for landmark in landmarks {
        let landmark = exact_object(landmark, &["landmark_id", "x", "y", "visibility"], "SilhouetteTarget@1.landmark")?;
        required_contract_identifier(landmark, "landmark_id", "SilhouetteTarget@1.landmark")?;
        for key in ["x", "y"] {
            if landmark.get(key).and_then(Value::as_f64).is_none_or(|coordinate| !coordinate.is_finite() || !(0.0..=1.0).contains(&coordinate)) {
                return Err(RuntimeError::InvalidInput("CONTRACT_OUTPUT_INVALID: target landmark coordinate".to_owned()));
            }
        }
        if !matches!(landmark.get("visibility").and_then(Value::as_str), Some("observed" | "inferred" | "unknown")) {
            return Err(RuntimeError::InvalidInput("CONTRACT_OUTPUT_INVALID: target landmark visibility".to_owned()));
        }
    }
    required_contract_sha256(object, "canonical_sha256", "SilhouetteTarget@1")?;
    verify_output_canonical_hash(value, "SilhouetteTarget@1")
}

fn validate_reference_mask_prepare_result(value: &Value) -> Result<(), RuntimeError> {
    let object = exact_object(
        value,
        &[
            "schema_version",
            "project_id",
            "reference_id",
            "target_sha256",
            "mask_sha256",
            "target",
            "canonical_sha256",
        ],
        "ReferenceMaskPrepareResult@1",
    )?;
    if object.get("schema_version").and_then(Value::as_str) != Some("ReferenceMaskPrepareResult@1") {
        return Err(RuntimeError::InvalidInput("CONTRACT_OUTPUT_INVALID: ReferenceMaskPrepareResult@1 schema_version".to_owned()));
    }
    required_contract_identifier(object, "project_id", "ReferenceMaskPrepareResult@1")?;
    required_contract_identifier(object, "reference_id", "ReferenceMaskPrepareResult@1")?;
    required_contract_sha256(object, "target_sha256", "ReferenceMaskPrepareResult@1")?;
    required_contract_sha256(object, "mask_sha256", "ReferenceMaskPrepareResult@1")?;
    validate_silhouette_target(object.get("target").ok_or_else(|| RuntimeError::InvalidInput("CONTRACT_OUTPUT_INVALID: target missing".to_owned()))?)?;
    required_contract_sha256(object, "canonical_sha256", "ReferenceMaskPrepareResult@1")?;
    verify_output_canonical_hash(value, "ReferenceMaskPrepareResult@1")
}

fn validate_camera_fit_result(value: &Value) -> Result<(), RuntimeError> {
    let object = exact_object(
        value,
        &[
            "schema_version",
            "candidate_id",
            "target_sha256",
            "selected_camera",
            "candidates",
            "status",
            "canonical_sha256",
        ],
        "CameraFitResult@1",
    )?;
    if object.get("schema_version").and_then(Value::as_str) != Some("CameraFitResult@1")
        || !matches!(object.get("status").and_then(Value::as_str), Some("ready" | "no_improvement" | "unavailable"))
    {
        return Err(RuntimeError::InvalidInput("CONTRACT_OUTPUT_INVALID: CameraFitResult@1 constants".to_owned()));
    }
    required_contract_identifier(object, "candidate_id", "CameraFitResult@1")?;
    required_contract_sha256(object, "target_sha256", "CameraFitResult@1")?;
    validate_camera_calibration(object.get("selected_camera").ok_or_else(|| RuntimeError::InvalidInput("CONTRACT_OUTPUT_INVALID: selected camera missing".to_owned()))?)?;
    let candidates = object.get("candidates").and_then(Value::as_array).ok_or_else(|| RuntimeError::InvalidInput("CONTRACT_OUTPUT_INVALID: candidates missing".to_owned()))?;
    if candidates.is_empty() || candidates.len() > 128 {
        return Err(RuntimeError::InvalidInput("CONTRACT_OUTPUT_INVALID: candidate camera count".to_owned()));
    }
    for candidate in candidates {
        let entry = exact_object(candidate, &["camera", "loss", "metrics"], "CameraFitResult@1.candidate")?;
        validate_camera_calibration(entry.get("camera").ok_or_else(|| RuntimeError::InvalidInput("CONTRACT_OUTPUT_INVALID: candidate camera missing".to_owned()))?)?;
        let loss = entry.get("loss").and_then(Value::as_f64).ok_or_else(|| RuntimeError::InvalidInput("CONTRACT_OUTPUT_INVALID: camera loss missing".to_owned()))?;
        if !loss.is_finite() || loss < 0.0 {
            return Err(RuntimeError::InvalidInput("CONTRACT_OUTPUT_INVALID: camera loss invalid".to_owned()));
        }
        validate_camera_fit_metrics(entry.get("metrics").ok_or_else(|| RuntimeError::InvalidInput("CONTRACT_OUTPUT_INVALID: camera metrics missing".to_owned()))?)?;
    }
    required_contract_sha256(object, "canonical_sha256", "CameraFitResult@1")?;
    verify_output_canonical_hash(value, "CameraFitResult@1")
}

fn validate_camera_fit_metrics(value: &Value) -> Result<(), RuntimeError> {
    let object = exact_object(value, &["silhouette_iou", "boundary_f1_4px", "bbox_edge_error", "centroid_error"], "CameraFitResult@1.metrics")?;
    for (key, maximum) in [("silhouette_iou", 1.0), ("boundary_f1_4px", 1.0), ("bbox_edge_error", 1.0), ("centroid_error", 1.0)] {
        let number = object.get(key).and_then(Value::as_f64).ok_or_else(|| RuntimeError::InvalidInput(format!("CONTRACT_OUTPUT_INVALID: camera metric {key}")))?;
        if !number.is_finite() || number < 0.0 || number > maximum {
            return Err(RuntimeError::InvalidInput(format!("CONTRACT_OUTPUT_INVALID: camera metric {key}")));
        }
    }
    Ok(())
}

fn validate_boundary_error_result(value: &Value) -> Result<(), RuntimeError> {
    let object = exact_object(value, &["schema_version", "candidate_id", "target_sha256", "render_set_hash", "metrics", "segments", "canonical_sha256"], "BoundaryErrorResult@1")?;
    if object.get("schema_version").and_then(Value::as_str) != Some("BoundaryErrorResult@1") {
        return Err(RuntimeError::InvalidInput("CONTRACT_OUTPUT_INVALID: BoundaryErrorResult@1 schema_version".to_owned()));
    }
    required_contract_identifier(object, "candidate_id", "BoundaryErrorResult@1")?;
    required_contract_sha256(object, "target_sha256", "BoundaryErrorResult@1")?;
    required_contract_sha256(object, "render_set_hash", "BoundaryErrorResult@1")?;
    validate_extended_silhouette_metrics(object.get("metrics").ok_or_else(|| RuntimeError::InvalidInput("CONTRACT_OUTPUT_INVALID: boundary metrics missing".to_owned()))?, "BoundaryErrorResult@1.metrics")?;
    let segments = object.get("segments").and_then(Value::as_array).ok_or_else(|| RuntimeError::InvalidInput("CONTRACT_OUTPUT_INVALID: boundary segments missing".to_owned()))?;
    if segments.len() > 64 {
        return Err(RuntimeError::InvalidInput("CONTRACT_OUTPUT_INVALID: too many boundary segments".to_owned()));
    }
    for segment in segments {
        let entry = exact_object(segment, &["reference", "model", "delta_px", "distance_px", "direction", "part_id"], "BoundaryErrorResult@1.segment")?;
        for key in ["reference", "model", "delta_px"] {
            let point = entry.get(key).and_then(Value::as_array).ok_or_else(|| RuntimeError::InvalidInput("CONTRACT_OUTPUT_INVALID: boundary point".to_owned()))?;
            if point.len() != 2 || point.iter().any(|value| value.as_f64().is_none_or(|number| !number.is_finite())) {
                return Err(RuntimeError::InvalidInput("CONTRACT_OUTPUT_INVALID: boundary point".to_owned()));
            }
        }
        let distance = entry.get("distance_px").and_then(Value::as_f64).ok_or_else(|| RuntimeError::InvalidInput("CONTRACT_OUTPUT_INVALID: boundary distance".to_owned()))?;
        if !distance.is_finite() || distance < 0.0 || !matches!(entry.get("direction").and_then(Value::as_str), Some("inward" | "outward" | "aligned")) {
            return Err(RuntimeError::InvalidInput("CONTRACT_OUTPUT_INVALID: boundary segment".to_owned()));
        }
        if let Some(part_id) = entry.get("part_id").and_then(Value::as_str) {
            if !is_opaque_id(part_id) {
                return Err(RuntimeError::InvalidInput("CONTRACT_OUTPUT_INVALID: boundary part_id".to_owned()));
            }
        } else if !entry.get("part_id").is_some_and(Value::is_null) {
            return Err(RuntimeError::InvalidInput("CONTRACT_OUTPUT_INVALID: boundary part_id".to_owned()));
        }
    }
    required_contract_sha256(object, "canonical_sha256", "BoundaryErrorResult@1")?;
    verify_output_canonical_hash(value, "BoundaryErrorResult@1")
}

fn validate_extended_silhouette_metrics(value: &Value, context: &str) -> Result<(), RuntimeError> {
    let object = exact_object(value, &["silhouette_iou", "boundary_f1_4px", "bbox_edge_error", "centroid_error", "sdf_chamfer_px"], context)?;
    for key in ["silhouette_iou", "boundary_f1_4px"] {
        let number = object.get(key).and_then(Value::as_f64).ok_or_else(|| RuntimeError::InvalidInput(format!("CONTRACT_OUTPUT_INVALID: {context}.{key}")))?;
        if !number.is_finite() || !(0.0..=1.0).contains(&number) { return Err(RuntimeError::InvalidInput(format!("CONTRACT_OUTPUT_INVALID: {context}.{key}"))); }
    }
    for key in ["bbox_edge_error", "centroid_error", "sdf_chamfer_px"] {
        let number = object.get(key).and_then(Value::as_f64).ok_or_else(|| RuntimeError::InvalidInput(format!("CONTRACT_OUTPUT_INVALID: {context}.{key}")))?;
        if !number.is_finite() || number < 0.0 { return Err(RuntimeError::InvalidInput(format!("CONTRACT_OUTPUT_INVALID: {context}.{key}"))); }
    }
    Ok(())
}

fn primary_form_view_spec(
    reference: &ReferenceEvidenceRecord,
    target: &Value,
    target_sha256: &str,
) -> Result<Value, RuntimeError> {
    let landmarks = target
        .get("landmarks")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            RuntimeError::InvalidInput(
                "PRIMARY_FORM_REPAIR_INVALID: target landmarks are missing".to_owned(),
            )
        })?
        .iter()
        .map(|landmark| {
            let object = exact_object(
                landmark,
                &["landmark_id", "x", "y", "visibility"],
                "PrimaryFormRepair target landmark",
            )?;
            Ok(json!({
                "landmark_id":object["landmark_id"].clone(),
                "x":object["x"].clone(),
                "y":object["y"].clone(),
                "visibility":object["visibility"].clone(),
                "confidence":1.0
            }))
        })
        .collect::<Result<Vec<_>, RuntimeError>>()?;
    let mut view_spec = json!({
        "schema_version":"ReferenceViewSpec@1",
        "reference_id":reference.reference_id,
        "reference_sha256":reference.object_sha256,
        "view_id":format!("primary-form-target-{}", &target_sha256[..24.min(target_sha256.len())]),
        "source_view":"unknown",
        "image":{"width":reference.width,"height":reference.height,"rotation_degrees":0.0,"crop":{"x":0.0,"y":0.0,"width":1.0,"height":1.0}},
        "landmarks":landmarks,
        "regions":[],
        "canonical_sha256":""
    });
    view_spec["canonical_sha256"] = Value::String(canonical_json_hash(&view_spec));
    validate_reference_view_spec(&view_spec, reference)?;
    Ok(view_spec)
}

fn validate_primary_form_repair_prepare_result(value: &Value) -> Result<(), RuntimeError> {
    let object = exact_object(
        value,
        &[
            "schema_version",
            "project_id",
            "source_candidate_id",
            "target_sha256",
            "reference_id",
            "fit_result",
            "prepared_candidate",
            "visual_evidence",
            "status",
            "quality_status",
            "candidate_state",
            "version_created",
            "canonical_sha256",
        ],
        "PrimaryFormRepairPrepareResult@1",
    )?;
    if object.get("schema_version").and_then(Value::as_str)
        != Some("PrimaryFormRepairPrepareResult@1")
        || !matches!(
            object.get("status").and_then(Value::as_str),
            Some("no_improvement" | "prepared")
        )
        || object.get("version_created") != Some(&Value::Bool(false))
    {
        return Err(RuntimeError::InvalidInput(
            "CONTRACT_OUTPUT_INVALID: PrimaryFormRepairPrepareResult@1 constants drifted"
                .to_owned(),
        ));
    }
    required_contract_identifier(object, "project_id", "PrimaryFormRepairPrepareResult@1")?;
    required_contract_identifier(
        object,
        "source_candidate_id",
        "PrimaryFormRepairPrepareResult@1",
    )?;
    required_contract_identifier(object, "reference_id", "PrimaryFormRepairPrepareResult@1")?;
    required_contract_sha256(object, "target_sha256", "PrimaryFormRepairPrepareResult@1")?;
    validate_silhouette_fit_result(
        object
            .get("fit_result")
            .ok_or_else(|| RuntimeError::InvalidInput("CONTRACT_OUTPUT_INVALID: fit_result".to_owned()))?,
    )?;
    let prepared = object
        .get("prepared_candidate")
        .ok_or_else(|| RuntimeError::InvalidInput("CONTRACT_OUTPUT_INVALID: prepared_candidate".to_owned()))?;
    let visual = object
        .get("visual_evidence")
        .ok_or_else(|| RuntimeError::InvalidInput("CONTRACT_OUTPUT_INVALID: visual_evidence".to_owned()))?;
    if object.get("status").and_then(Value::as_str) == Some("no_improvement") {
        if !prepared.is_null()
            || !visual.is_null()
            || object.get("quality_status").and_then(Value::as_str) != Some("not-run")
            || object.get("candidate_state").and_then(Value::as_str) != Some("unchanged")
        {
            return Err(RuntimeError::InvalidInput(
                "CONTRACT_OUTPUT_INVALID: no-improvement result must not contain a staged candidate"
                    .to_owned(),
            ));
        }
    } else {
        validate_geometry_prepare_result_v2_output(prepared)?;
        let visual = exact_object(
            visual,
            &[
                "candidate_id",
                "reference_id",
                "target_sha256",
                "camera_hash",
                "render_set_hash",
                "comparison_report_hash",
                "quality_report_hash",
                "render_set",
                "comparison_report",
                "quality_report",
            ],
            "PrimaryFormRepairPrepareResult@1.visual_evidence",
        )?;
        required_contract_identifier(visual, "candidate_id", "PrimaryFormRepair visual evidence")?;
        required_contract_identifier(visual, "reference_id", "PrimaryFormRepair visual evidence")?;
        required_contract_sha256(visual, "target_sha256", "PrimaryFormRepair visual evidence")?;
        for key in [
            "camera_hash",
            "render_set_hash",
            "comparison_report_hash",
            "quality_report_hash",
        ] {
            required_contract_sha256(visual, key, "PrimaryFormRepair visual evidence")?;
        }
        validate_render_set_v2_output(visual.get("render_set").ok_or_else(|| {
            RuntimeError::InvalidInput("CONTRACT_OUTPUT_INVALID: render_set".to_owned())
        })?)?;
        validate_reference_comparison_report(visual.get("comparison_report").ok_or_else(|| {
            RuntimeError::InvalidInput("CONTRACT_OUTPUT_INVALID: comparison_report".to_owned())
        })?)?;
        validate_quality_report_v2_output(visual.get("quality_report").ok_or_else(|| {
            RuntimeError::InvalidInput("CONTRACT_OUTPUT_INVALID: quality_report".to_owned())
        })?)?;
        if object.get("quality_status")
            != visual.get("quality_report").and_then(|report| report.get("visual_status"))
        {
            return Err(RuntimeError::InvalidInput(
                "CONTRACT_OUTPUT_INVALID: quality status is not bound to the Runtime report"
                    .to_owned(),
            ));
        }
        if object.get("candidate_state").and_then(Value::as_str)
            != Some("staged_new_candidate")
        {
            return Err(RuntimeError::InvalidInput(
                "CONTRACT_OUTPUT_INVALID: prepared result candidate state drifted".to_owned(),
            ));
        }
    }
    required_contract_sha256(object, "canonical_sha256", "PrimaryFormRepairPrepareResult@1")?;
    verify_output_canonical_hash(value, "PrimaryFormRepairPrepareResult@1")
}

fn validate_silhouette_fit_result(value: &Value) -> Result<(), RuntimeError> {
    let object = exact_object(value, &["schema_version", "project_id", "candidate_id", "target_sha256", "selected_camera", "selected_parameters", "parameter_deltas", "selected_geometry_program", "geometry_evaluations", "iterations", "evaluations", "metrics", "thresholds", "status", "canonical_sha256"], "SilhouetteFitResult@1")?;
    if object.get("schema_version").and_then(Value::as_str) != Some("SilhouetteFitResult@1") { return Err(RuntimeError::InvalidInput("CONTRACT_OUTPUT_INVALID: SilhouetteFitResult@1 schema_version".to_owned())); }
    required_contract_identifier(object, "project_id", "SilhouetteFitResult@1")?;
    required_contract_identifier(object, "candidate_id", "SilhouetteFitResult@1")?;
    required_contract_sha256(object, "target_sha256", "SilhouetteFitResult@1")?;
    validate_camera_calibration(object.get("selected_camera").ok_or_else(|| RuntimeError::InvalidInput("CONTRACT_OUTPUT_INVALID: selected camera".to_owned()))?)?;
    let parameters = object.get("selected_parameters").and_then(Value::as_array).ok_or_else(|| RuntimeError::InvalidInput("CONTRACT_OUTPUT_INVALID: fit parameters".to_owned()))?;
    if parameters.len() > 64 || parameters.iter().any(|parameter| {
        let Some(parameter) = parameter.as_object() else { return true; };
        parameter.len() != 3
            || required_contract_identifier(parameter, "parameter_id", "SilhouetteFitResult@1.selected_parameters").is_err()
            || required_contract_identifier(parameter, "part_id", "SilhouetteFitResult@1.selected_parameters").is_err()
            || parameter.get("value").and_then(Value::as_f64).is_none_or(|value| !value.is_finite())
    }) {
        return Err(RuntimeError::InvalidInput("CONTRACT_OUTPUT_INVALID: fit parameters".to_owned()));
    }
    if let Some(deltas) = object.get("parameter_deltas") {
        let deltas = deltas.as_array().ok_or_else(|| RuntimeError::InvalidInput("CONTRACT_OUTPUT_INVALID: parameter_deltas".to_owned()))?;
        if deltas.len() > 64 || deltas.iter().any(|delta| {
            let Some(delta) = delta.as_object() else { return true; };
            delta.len() != 5
                || required_contract_identifier(delta, "parameter_id", "SilhouetteFitResult@1.parameter_deltas").is_err()
                || required_contract_identifier(delta, "part_id", "SilhouetteFitResult@1.parameter_deltas").is_err()
                || ["from", "to", "delta"].iter().any(|key| delta.get(*key).and_then(Value::as_f64).is_none_or(|value| !value.is_finite()))
        }) {
            return Err(RuntimeError::InvalidInput("CONTRACT_OUTPUT_INVALID: parameter_deltas".to_owned()));
        }
    }
    let project_id = required_contract_identifier(object, "project_id", "SilhouetteFitResult@1")?;
    let selected_geometry_program = object
        .get("selected_geometry_program")
        .ok_or_else(|| RuntimeError::InvalidInput("CONTRACT_OUTPUT_INVALID: selected_geometry_program".to_owned()))?;
    if !selected_geometry_program.is_null() {
        let program = selected_geometry_program.as_object().ok_or_else(|| {
            RuntimeError::InvalidInput("CONTRACT_OUTPUT_INVALID: selected_geometry_program".to_owned())
        })?;
        if program.get("schema_version").and_then(Value::as_str) != Some("GeometryProgram@2")
            || program.get("project_id").and_then(Value::as_str) != Some(project_id.as_str())
        {
            return Err(RuntimeError::InvalidInput(
                "CONTRACT_OUTPUT_INVALID: selected_geometry_program project binding".to_owned(),
            ));
        }
        let mut draft = Value::Object(program.clone());
        draft
            .as_object_mut()
            .expect("selected GeometryProgram is an object")
            .remove("canonical_sha256");
        let hash = hash_geometry_program_with_runtime_worker(&draft).map_err(|error| {
            RuntimeError::InvalidInput(format!(
                "CONTRACT_OUTPUT_INVALID: selected_geometry_program validation failed: {error}"
            ))
        })?;
        if hash.get("canonical_sha256").and_then(Value::as_str)
            != program.get("canonical_sha256").and_then(Value::as_str)
            || hash.get("operator_catalog_sha256").and_then(Value::as_str).is_none()
        {
            return Err(RuntimeError::InvalidInput(
                "CONTRACT_OUTPUT_INVALID: selected_geometry_program hash binding".to_owned(),
            ));
        }
    }
    if let Some(geometry_evaluations) = object.get("geometry_evaluations") {
        if geometry_evaluations.as_u64().is_none_or(|value| value > 64) {
            return Err(RuntimeError::InvalidInput("CONTRACT_OUTPUT_INVALID: geometry_evaluations".to_owned()));
        }
    }
    validate_extended_silhouette_metrics(object.get("metrics").ok_or_else(|| RuntimeError::InvalidInput("CONTRACT_OUTPUT_INVALID: fit metrics".to_owned()))?, "SilhouetteFitResult@1.metrics")?;
    let iterations = object.get("iterations").and_then(Value::as_u64).unwrap_or(99);
    let evaluations = object.get("evaluations").and_then(Value::as_u64).unwrap_or(99);
    if iterations > 8 || evaluations == 0 || evaluations > 64 || !matches!(object.get("status").and_then(Value::as_str), Some("ready" | "no_improvement" | "quality_target_not_met" | "unavailable")) { return Err(RuntimeError::InvalidInput("CONTRACT_OUTPUT_INVALID: fit bounds/status".to_owned())); }
    let thresholds = exact_object(object.get("thresholds").ok_or_else(|| RuntimeError::InvalidInput("CONTRACT_OUTPUT_INVALID: fit thresholds".to_owned()))?, &["silhouette_iou", "boundary_f1_4px"], "SilhouetteFitResult@1.thresholds")?;
    if thresholds.get("silhouette_iou").and_then(Value::as_f64) != Some(0.9) || thresholds.get("boundary_f1_4px").and_then(Value::as_f64) != Some(0.9) { return Err(RuntimeError::InvalidInput("CONTRACT_OUTPUT_INVALID: strict silhouette thresholds drifted".to_owned())); }
    required_contract_sha256(object, "canonical_sha256", "SilhouetteFitResult@1")?;
    verify_output_canonical_hash(value, "SilhouetteFitResult@1")
}

fn validate_silhouette_part_error_result(value: &Value) -> Result<(), RuntimeError> {
    let object = exact_object(
        value,
        &[
            "schema_version",
            "project_id",
            "candidate_id",
            "target_sha256",
            "render_set_hash",
            "metrics",
            "parts",
            "recommended_part_ids",
            "canonical_sha256",
        ],
        "SilhouettePartErrorResult@1",
    )?;
    if object.get("schema_version").and_then(Value::as_str)
        != Some("SilhouettePartErrorResult@1")
    {
        return Err(RuntimeError::InvalidInput(
            "CONTRACT_OUTPUT_INVALID: SilhouettePartErrorResult@1 schema_version".to_owned(),
        ));
    }
    for key in ["project_id", "candidate_id"] {
        required_contract_identifier(object, key, "SilhouettePartErrorResult@1")?;
    }
    for key in ["target_sha256", "render_set_hash"] {
        required_contract_sha256(object, key, "SilhouettePartErrorResult@1")?;
    }
    validate_extended_silhouette_metrics(
        object
            .get("metrics")
            .ok_or_else(|| RuntimeError::InvalidInput("CONTRACT_OUTPUT_INVALID: Part metrics".to_owned()))?,
        "SilhouettePartErrorResult@1.metrics",
    )?;
    let parts = object
        .get("parts")
        .and_then(Value::as_array)
        .ok_or_else(|| RuntimeError::InvalidInput("CONTRACT_OUTPUT_INVALID: Part rows".to_owned()))?;
    if parts.is_empty() || parts.len() > 64 {
        return Err(RuntimeError::InvalidInput(
            "CONTRACT_OUTPUT_INVALID: Part row budget".to_owned(),
        ));
    }
    let mut part_ids = std::collections::HashSet::new();
    for part in parts {
        let entry = exact_object(
            part,
            &[
                "part_id",
                "visibility",
                "status",
                "target_boundary_pixels",
                "model_pixels",
                "target_bbox",
                "model_bbox",
                "centroid_delta_px",
                "width_ratio",
                "height_ratio",
                "boundary_error_px",
            ],
            "SilhouettePartErrorResult@1.part",
        )?;
        let part_id = required_contract_identifier(
            entry,
            "part_id",
            "SilhouettePartErrorResult@1.part",
        )?;
        if !part_ids.insert(part_id) {
            return Err(RuntimeError::InvalidInput(
                "CONTRACT_OUTPUT_INVALID: duplicate Part row".to_owned(),
            ));
        }
        if !matches!(
            entry.get("visibility").and_then(Value::as_str),
            Some("observed" | "inferred" | "unknown")
        ) || !matches!(
            entry.get("status").and_then(Value::as_str),
            Some("ready" | "missing_model_part" | "empty_target_part")
        ) {
            return Err(RuntimeError::InvalidInput(
                "CONTRACT_OUTPUT_INVALID: Part visibility/status".to_owned(),
            ));
        }
        for key in ["target_boundary_pixels", "model_pixels"] {
            let number = entry.get(key).and_then(Value::as_u64).ok_or_else(|| {
                RuntimeError::InvalidInput("CONTRACT_OUTPUT_INVALID: Part pixel count".to_owned())
            })?;
            if number > 262_144 {
                return Err(RuntimeError::InvalidInput(
                    "CONTRACT_OUTPUT_INVALID: Part pixel count".to_owned(),
                ));
            }
        }
        for key in ["target_bbox", "model_bbox"] {
            let bbox = entry.get(key).and_then(Value::as_array).ok_or_else(|| {
                RuntimeError::InvalidInput("CONTRACT_OUTPUT_INVALID: Part bbox".to_owned())
            })?;
            if bbox.len() != 4
                || bbox.iter().any(|value| {
                    value
                        .as_f64()
                        .is_none_or(|number| !number.is_finite() || !(0.0..=1.0).contains(&number))
                })
            {
                return Err(RuntimeError::InvalidInput(
                    "CONTRACT_OUTPUT_INVALID: Part bbox".to_owned(),
                ));
            }
        }
        let centroid = entry
            .get("centroid_delta_px")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                RuntimeError::InvalidInput("CONTRACT_OUTPUT_INVALID: Part centroid".to_owned())
            })?;
        if centroid.len() != 2
            || centroid.iter().any(|value| {
                value
                    .as_f64()
                    .is_none_or(|number| !number.is_finite() || !(-512.0..=512.0).contains(&number))
            })
        {
            return Err(RuntimeError::InvalidInput(
                "CONTRACT_OUTPUT_INVALID: Part centroid".to_owned(),
            ));
        }
        for key in ["width_ratio", "height_ratio"] {
            if entry.get(key).and_then(Value::as_f64).is_none_or(|number| {
                !number.is_finite() || !(0.0..=4.0).contains(&number)
            }) {
                return Err(RuntimeError::InvalidInput(
                    "CONTRACT_OUTPUT_INVALID: Part ratio".to_owned(),
                ));
            }
        }
        if entry
            .get("boundary_error_px")
            .and_then(Value::as_f64)
            .is_none_or(|number| !number.is_finite() || !(0.0..=512.0).contains(&number))
        {
            return Err(RuntimeError::InvalidInput(
                "CONTRACT_OUTPUT_INVALID: Part boundary error".to_owned(),
            ));
        }
    }
    let recommended = object
        .get("recommended_part_ids")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            RuntimeError::InvalidInput("CONTRACT_OUTPUT_INVALID: recommended Parts".to_owned())
        })?;
    if recommended.len() > 16 {
        return Err(RuntimeError::InvalidInput(
            "CONTRACT_OUTPUT_INVALID: recommended Part budget".to_owned(),
        ));
    }
    let mut recommended_ids = std::collections::HashSet::new();
    for value in recommended {
        let id = required_contract_identifier(
            &serde_json::Map::from_iter([("part_id".to_owned(), value.clone())]),
            "part_id",
            "SilhouettePartErrorResult@1.recommended_part_ids",
        )?;
        if !recommended_ids.insert(id.clone()) || !part_ids.contains(&id) {
            return Err(RuntimeError::InvalidInput(
                "CONTRACT_OUTPUT_INVALID: recommended Part id".to_owned(),
            ));
        }
    }
    required_contract_sha256(object, "canonical_sha256", "SilhouettePartErrorResult@1")?;
    verify_output_canonical_hash(value, "SilhouettePartErrorResult@1")
}

fn validate_part_contour_fit_result(value: &Value) -> Result<(), RuntimeError> {
    let object = exact_object(value, &["schema_version", "project_id", "candidate_id", "target_sha256", "part_id", "adjustments", "metrics", "status", "canonical_sha256"], "PartContourFitResult@1")?;
    if object.get("schema_version").and_then(Value::as_str) != Some("PartContourFitResult@1") { return Err(RuntimeError::InvalidInput("CONTRACT_OUTPUT_INVALID: PartContourFitResult@1 schema_version".to_owned())); }
    for key in ["project_id", "candidate_id", "part_id"] { required_contract_identifier(object, key, "PartContourFitResult@1")?; }
    required_contract_sha256(object, "target_sha256", "PartContourFitResult@1")?;
    let metrics = exact_object(object.get("metrics").ok_or_else(|| RuntimeError::InvalidInput("CONTRACT_OUTPUT_INVALID: part metrics".to_owned()))?, &["silhouette_iou", "boundary_f1_4px", "sdf_chamfer_px", "part_boundary_error_px"], "PartContourFitResult@1.metrics")?;
    for key in ["silhouette_iou", "boundary_f1_4px"] {
        let number = metrics.get(key).and_then(Value::as_f64).ok_or_else(|| RuntimeError::InvalidInput("CONTRACT_OUTPUT_INVALID: part metric".to_owned()))?;
        if !number.is_finite() || !(0.0..=1.0).contains(&number) { return Err(RuntimeError::InvalidInput("CONTRACT_OUTPUT_INVALID: part metric".to_owned())); }
    }
    for key in ["sdf_chamfer_px", "part_boundary_error_px"] {
        if metrics.get(key).and_then(Value::as_f64).is_none_or(|number| !number.is_finite() || number < 0.0) { return Err(RuntimeError::InvalidInput("CONTRACT_OUTPUT_INVALID: part metric".to_owned())); }
    }
    let adjustments = object.get("adjustments").and_then(Value::as_array).ok_or_else(|| RuntimeError::InvalidInput("CONTRACT_OUTPUT_INVALID: part adjustments".to_owned()))?;
    if adjustments.len() > 16 { return Err(RuntimeError::InvalidInput("CONTRACT_OUTPUT_INVALID: part adjustment budget".to_owned())); }
    for adjustment in adjustments {
        let entry = exact_object(adjustment, &["parameter_id", "delta", "bounded_value"], "PartContourFitResult@1.adjustment")?;
        required_contract_identifier(entry, "parameter_id", "PartContourFitResult@1.adjustment")?;
        for key in ["delta", "bounded_value"] { if entry.get(key).and_then(Value::as_f64).is_none_or(|number| !number.is_finite()) { return Err(RuntimeError::InvalidInput("CONTRACT_OUTPUT_INVALID: part adjustment value".to_owned())); } }
    }
    if !matches!(object.get("status").and_then(Value::as_str), Some("proposal_ready" | "no_action" | "unavailable")) { return Err(RuntimeError::InvalidInput("CONTRACT_OUTPUT_INVALID: part status".to_owned())); }
    required_contract_sha256(object, "canonical_sha256", "PartContourFitResult@1")?;
    verify_output_canonical_hash(value, "PartContourFitResult@1")
}

fn validate_silhouette_candidate_compare_result(value: &Value) -> Result<(), RuntimeError> {
    let object = exact_object(value, &["schema_version", "target_sha256", "candidates", "winner_candidate_id", "delta", "status", "canonical_sha256"], "SilhouetteCandidateCompareResult@1")?;
    if object.get("schema_version").and_then(Value::as_str) != Some("SilhouetteCandidateCompareResult@1") { return Err(RuntimeError::InvalidInput("CONTRACT_OUTPUT_INVALID: compare schema_version".to_owned())); }
    required_contract_sha256(object, "target_sha256", "SilhouetteCandidateCompareResult@1")?;
    let candidates = object.get("candidates").and_then(Value::as_array).ok_or_else(|| RuntimeError::InvalidInput("CONTRACT_OUTPUT_INVALID: compare candidates".to_owned()))?;
    if !(2..=8).contains(&candidates.len()) { return Err(RuntimeError::InvalidInput("CONTRACT_OUTPUT_INVALID: compare candidate budget".to_owned())); }
    let mut ids = std::collections::HashSet::new();
    for candidate in candidates {
        let entry = exact_object(candidate, &["candidate_id", "metrics", "loss"], "SilhouetteCandidateCompareResult@1.candidate")?;
        let id = required_contract_identifier(entry, "candidate_id", "SilhouetteCandidateCompareResult@1.candidate")?;
        if !ids.insert(id) || entry.get("loss").and_then(Value::as_f64).is_none_or(|loss| !loss.is_finite() || loss < 0.0) { return Err(RuntimeError::InvalidInput("CONTRACT_OUTPUT_INVALID: compare candidate".to_owned())); }
        validate_extended_silhouette_metrics(entry.get("metrics").ok_or_else(|| RuntimeError::InvalidInput("CONTRACT_OUTPUT_INVALID: compare metrics".to_owned()))?, "SilhouetteCandidateCompareResult@1.metrics")?;
    }
    if let Some(winner) = object.get("winner_candidate_id").and_then(Value::as_str) { if !ids.contains(winner) { return Err(RuntimeError::InvalidInput("CONTRACT_OUTPUT_INVALID: compare winner".to_owned())); } } else if !object.get("winner_candidate_id").is_some_and(Value::is_null) { return Err(RuntimeError::InvalidInput("CONTRACT_OUTPUT_INVALID: compare winner".to_owned())); }
    let delta = exact_object(object.get("delta").ok_or_else(|| RuntimeError::InvalidInput("CONTRACT_OUTPUT_INVALID: compare delta".to_owned()))?, &["silhouette_iou", "boundary_f1_4px", "sdf_chamfer_px"], "SilhouetteCandidateCompareResult@1.delta")?;
    for key in ["silhouette_iou", "boundary_f1_4px", "sdf_chamfer_px"] { if delta.get(key).and_then(Value::as_f64).is_none_or(|number| !number.is_finite()) { return Err(RuntimeError::InvalidInput("CONTRACT_OUTPUT_INVALID: compare delta value".to_owned())); } }
    if !matches!(object.get("status").and_then(Value::as_str), Some("ready" | "tie" | "unavailable")) { return Err(RuntimeError::InvalidInput("CONTRACT_OUTPUT_INVALID: compare status".to_owned())); }
    required_contract_sha256(object, "canonical_sha256", "SilhouetteCandidateCompareResult@1")?;
    verify_output_canonical_hash(value, "SilhouetteCandidateCompareResult@1")
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
    if object.get("status").and_then(Value::as_str) == Some("PARTIAL_VISIBLE_VIEW_PASS")
        && !visible_view_gate_passes(&Value::Object(metrics.clone()))
    {
        return Err(RuntimeError::InvalidInput(
            "CONTRACT_OUTPUT_INVALID: visible-view pass does not meet strict contour gates"
                .to_owned(),
        ));
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
    let visual_status = object
        .get("visual_status")
        .and_then(Value::as_str)
        .unwrap_or("not-run");
    let hard_gate_passed = object
        .get("hard_gate_passed")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if (visual_status == "PARTIAL_VISIBLE_VIEW_PASS") != hard_gate_passed {
        return Err(RuntimeError::InvalidInput(
            "CONTRACT_OUTPUT_INVALID: QualityReport@2 visual gate/status mismatch".to_owned(),
        ));
    }
    verify_output_canonical_hash(value, "QualityReport@2")
}

fn validate_appearance_prepare_result_v2_output(value: &Value) -> Result<(), RuntimeError> {
    let object = exact_object(
        value,
        &[
            "schema_version",
            "candidate",
            "job",
            "artifact",
            "render_set",
            "render_set_object_sha256",
            "quality_report_object_sha256",
            "material_pack_manifest_sha256",
            "texture_build_receipt_sha256",
        ],
        "AppearancePrepareResult@2",
    )?;
    if object.get("schema_version").and_then(Value::as_str) != Some("AppearancePrepareResult@2") {
        return Err(RuntimeError::InvalidInput(
            "CONTRACT_OUTPUT_INVALID: AppearancePrepareResult@2 schema_version".to_owned(),
        ));
    }
    validate_artifact_readback_v2_output(
        object
            .get("artifact")
            .ok_or_else(|| RuntimeError::InvalidInput("CONTRACT_OUTPUT_INVALID: appearance artifact".to_owned()))?,
    )?;
    validate_render_set_v2_output(
        object
            .get("render_set")
            .ok_or_else(|| RuntimeError::InvalidInput("CONTRACT_OUTPUT_INVALID: appearance render_set".to_owned()))?,
    )?;
    for key in [
        "render_set_object_sha256",
        "quality_report_object_sha256",
        "material_pack_manifest_sha256",
        "texture_build_receipt_sha256",
    ] {
        required_contract_sha256(object, key, "AppearancePrepareResult@2")?;
    }
    Ok(())
}

struct ReferenceMask {
    mask: Vec<bool>,
    png: Vec<u8>,
}

fn parse_contour_points(value: Option<&Value>) -> Result<Option<Vec<[f64; 2]>>, RuntimeError> {
    let Some(value) = value else {
        return Ok(None);
    };
    if value.is_null() {
        return Ok(None);
    }
    let points = value.as_array().ok_or_else(|| {
        RuntimeError::InvalidInput("REFERENCE_MASK_INVALID: contour_points must be an array or null".to_owned())
    })?;
    if !(3..=512).contains(&points.len()) {
        return Err(RuntimeError::InvalidInput(
            "REFERENCE_MASK_INVALID: contour_points must contain 3..512 points".to_owned(),
        ));
    }
    let mut parsed = Vec::with_capacity(points.len());
    for point in points {
        let values = point.as_array().ok_or_else(|| {
            RuntimeError::InvalidInput("REFERENCE_MASK_INVALID: contour point must be [x,y]".to_owned())
        })?;
        if values.len() != 2 {
            return Err(RuntimeError::InvalidInput(
                "REFERENCE_MASK_INVALID: contour point must contain two coordinates".to_owned(),
            ));
        }
        let x = values[0].as_f64().ok_or_else(|| {
            RuntimeError::InvalidInput("REFERENCE_MASK_INVALID: contour x is not finite".to_owned())
        })?;
        let y = values[1].as_f64().ok_or_else(|| {
            RuntimeError::InvalidInput("REFERENCE_MASK_INVALID: contour y is not finite".to_owned())
        })?;
        if !x.is_finite() || !y.is_finite() || !(0.0..=1.0).contains(&x) || !(0.0..=1.0).contains(&y) {
            return Err(RuntimeError::InvalidInput(
                "REFERENCE_MASK_INVALID: contour coordinates must be finite normalized values".to_owned(),
            ));
        }
        parsed.push([x, y]);
    }
    if contour_self_intersects(&parsed) {
        return Err(RuntimeError::InvalidInput(
            "REFERENCE_MASK_INVALID: contour self-intersects".to_owned(),
        ));
    }
    Ok(Some(parsed))
}

fn parse_target_landmarks(value: Option<&Value>) -> Result<Value, RuntimeError> {
    let Some(value) = value else {
        return Ok(json!([]));
    };
    if value.is_null() {
        return Ok(json!([]));
    }
    let values = value.as_array().ok_or_else(|| {
        RuntimeError::InvalidInput("REFERENCE_MASK_INVALID: landmarks must be an array or null".to_owned())
    })?;
    if values.len() > 128 {
        return Err(RuntimeError::InvalidInput("REFERENCE_MASK_INVALID: too many landmarks".to_owned()));
    }
    for landmark in values {
        let object = landmark.as_object().ok_or_else(|| RuntimeError::InvalidInput("REFERENCE_MASK_INVALID: landmark must be an object".to_owned()))?;
        if object.len() != 4 || ["landmark_id", "x", "y", "visibility"].iter().any(|key| !object.contains_key(*key)) || object.keys().any(|key| !["landmark_id", "x", "y", "visibility"].contains(&key.as_str())) {
            return Err(RuntimeError::InvalidInput("REFERENCE_MASK_INVALID: landmark field set is not closed".to_owned()));
        }
        if !is_opaque_id(object.get("landmark_id").and_then(Value::as_str).unwrap_or_default()) || !matches!(object.get("visibility").and_then(Value::as_str), Some("observed" | "inferred" | "unknown")) {
            return Err(RuntimeError::InvalidInput("REFERENCE_MASK_INVALID: landmark identity or visibility is invalid".to_owned()));
        }
        let x = object.get("x").and_then(Value::as_f64).ok_or_else(|| {
            RuntimeError::InvalidInput("REFERENCE_MASK_INVALID: landmark x is required".to_owned())
        })?;
        let y = object.get("y").and_then(Value::as_f64).ok_or_else(|| {
            RuntimeError::InvalidInput("REFERENCE_MASK_INVALID: landmark y is required".to_owned())
        })?;
        if !x.is_finite() || !y.is_finite() || !(0.0..=1.0).contains(&x) || !(0.0..=1.0).contains(&y) {
            return Err(RuntimeError::InvalidInput("REFERENCE_MASK_INVALID: landmark coordinates are invalid".to_owned()));
        }
    }
    Ok(value.clone())
}

fn parse_target_parts(value: Option<&Value>) -> Result<Value, RuntimeError> {
    let Some(value) = value else {
        return Ok(json!([]));
    };
    if value.is_null() {
        return Ok(json!([]));
    }
    let values = value.as_array().ok_or_else(|| {
        RuntimeError::InvalidInput("REFERENCE_MASK_INVALID: parts must be an array or null".to_owned())
    })?;
    if values.len() > 64 {
        return Err(RuntimeError::InvalidInput("REFERENCE_MASK_INVALID: too many parts".to_owned()));
    }
    for part in values {
        let object = part.as_object().ok_or_else(|| RuntimeError::InvalidInput("REFERENCE_MASK_INVALID: part must be an object".to_owned()))?;
        if object.len() != 4 || ["part_id", "start_index", "end_index", "visibility"].iter().any(|key| !object.contains_key(*key)) || object.keys().any(|key| !["part_id", "start_index", "end_index", "visibility"].contains(&key.as_str())) {
            return Err(RuntimeError::InvalidInput("REFERENCE_MASK_INVALID: part field set is not closed".to_owned()));
        }
        let part_id = object.get("part_id").and_then(Value::as_str).unwrap_or_default();
        if !is_opaque_id(part_id) || object.get("start_index").and_then(Value::as_u64).is_none_or(|index| index > 511) || object.get("end_index").and_then(Value::as_u64).is_none_or(|index| index > 511) || !matches!(object.get("visibility").and_then(Value::as_str), Some("observed" | "inferred" | "unknown")) {
            return Err(RuntimeError::InvalidInput("REFERENCE_MASK_INVALID: part fields are invalid".to_owned()));
        }
    }
    Ok(value.clone())
}

fn contour_self_intersects(points: &[[f64; 2]]) -> bool {
    fn orientation(a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> f64 {
        (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
    }
    fn intersects(a: [f64; 2], b: [f64; 2], c: [f64; 2], d: [f64; 2]) -> bool {
        let ab_c = orientation(a, b, c);
        let ab_d = orientation(a, b, d);
        let cd_a = orientation(c, d, a);
        let cd_b = orientation(c, d, b);
        let eps = 1e-10;
        ((ab_c > eps && ab_d < -eps) || (ab_c < -eps && ab_d > eps))
            && ((cd_a > eps && cd_b < -eps) || (cd_a < -eps && cd_b > eps))
    }
    let n = points.len();
    for i in 0..n {
        let a = points[i];
        let b = points[(i + 1) % n];
        for j in (i + 1)..n {
            if j == i + 1 || (i == 0 && j == n - 1) {
                continue;
            }
            if intersects(a, b, points[j], points[(j + 1) % n]) {
                return true;
            }
        }
    }
    false
}

fn point_in_polygon(x: f64, y: f64, points: &[[f64; 2]]) -> bool {
    let mut inside = false;
    let mut previous = points.len() - 1;
    for current in 0..points.len() {
        let [xi, yi] = points[current];
        let [xj, yj] = points[previous];
        let crosses = (yi > y) != (yj > y)
            && x < (xj - xi) * (y - yi) / ((yj - yi).abs().max(1e-12) * (if yj >= yi { 1.0 } else { -1.0 })) + xi;
        if crosses {
            inside = !inside;
        }
        previous = current;
    }
    inside
}

fn rasterize_contour(points: &[[f64; 2]]) -> Vec<bool> {
    let mut mask = vec![false; 512 * 512];
    for y in 0..512usize {
        for x in 0..512usize {
            let px = (x as f64 + 0.5) / 512.0;
            let py = (y as f64 + 0.5) / 512.0;
            mask[y * 512 + x] = point_in_polygon(px, py, points);
        }
    }
    close_mask(&mask)
}

fn mask_to_png(mask: &[bool]) -> Result<Vec<u8>, RuntimeError> {
    if mask.len() != 512 * 512 {
        return Err(RuntimeError::InvalidInput("REFERENCE_MASK_INVALID: mask dimensions are not 512x512".to_owned()));
    }
    let mut image = RgbaImage::from_pixel(512, 512, Rgba([0, 0, 0, 255]));
    for (index, value) in mask.iter().enumerate() {
        if *value {
            image.put_pixel((index % 512) as u32, (index / 512) as u32, Rgba([255, 255, 255, 255]));
        }
    }
    let mut png = Vec::new();
    image::DynamicImage::ImageRgba8(image)
        .write_to(&mut Cursor::new(&mut png), ImageFormat::Png)
        .map_err(|error| RuntimeError::InvalidInput(format!("REFERENCE_MASK_FAILED: {error}")))?;
    Ok(png)
}

fn contour_points_from_mask(mask: &[bool]) -> Vec<Value> {
    // The old implementation sorted every boundary pixel by its angle around
    // the global centroid.  That does not produce a contour: concave regions
    // and disconnected foreground components can be interleaved, yielding a
    // self-crossing polygon that cannot safely drive a Part slice.  Build the
    // actual pixel boundary as directed grid edges instead.  The foreground
    // stays on the right of each edge, so every closed component is a stable
    // clockwise loop.  We keep the largest loop as the automatic outer contour;
    // the exact binary mask remains the comparison truth and small detached
    // components must be explicitly annotated by the user before local repair.
    if mask.len() != 512 * 512 {
        return Vec::new();
    }
    type GridPoint = (i32, i32);
    type GridEdge = (GridPoint, GridPoint);
    let mut outgoing: HashMap<GridPoint, Vec<GridPoint>> = HashMap::new();
    let mut edges = Vec::<GridEdge>::new();
    let mut add_edge = |start: GridPoint, end: GridPoint| {
        edges.push((start, end));
        outgoing.entry(start).or_default().push(end);
    };
    for y in 0..512usize {
        for x in 0..512usize {
            if !mask[y * 512 + x] {
                continue;
            }
            let x = x as i32;
            let y = y as i32;
            if y == 0 || !mask[(y as usize - 1) * 512 + x as usize] {
                add_edge((x, y), (x + 1, y));
            }
            if x == 511 || !mask[y as usize * 512 + (x as usize + 1)] {
                add_edge((x + 1, y), (x + 1, y + 1));
            }
            if y == 511 || !mask[(y as usize + 1) * 512 + x as usize] {
                add_edge((x + 1, y + 1), (x, y + 1));
            }
            if x == 0 || !mask[y as usize * 512 + (x as usize - 1)] {
                add_edge((x, y + 1), (x, y));
            }
        }
    }
    for ends in outgoing.values_mut() {
        ends.sort_unstable();
    }
    edges.sort_unstable();
    let mut used = HashSet::<GridEdge>::new();
    let mut loops = Vec::<Vec<GridPoint>>::new();
    for (start, first_end) in edges {
        if used.contains(&(start, first_end)) {
            continue;
        }
        let mut points = Vec::new();
        let mut current = start;
        let mut previous_direction = (first_end.0 - start.0, first_end.1 - start.1);
        let mut next = first_end;
        let mut guard = 0usize;
        loop {
            if !used.insert((current, next)) {
                break;
            }
            points.push(current);
            current = next;
            if current == start {
                break;
            }
            let Some(candidates) = outgoing.get(&current) else {
                break;
            };
            let mut available = candidates
                .iter()
                .copied()
                .filter(|candidate| !used.contains(&(current, *candidate)))
                .collect::<Vec<_>>();
            if available.is_empty() {
                break;
            }
            // At a normal pixel corner there is one continuation.  If two
            // components touch at a grid vertex, prefer the clockwise/right
            // turn, then straight, then left, with lexicographic tie-breaks.
            available.sort_by_key(|candidate| {
                let direction = (candidate.0 - current.0, candidate.1 - current.1);
                let previous_index = grid_direction_index(previous_direction);
                let candidate_index = grid_direction_index(direction);
                let turn = (candidate_index - previous_index + 4) % 4;
                let preference = match turn {
                    1 => 0,
                    0 => 1,
                    3 => 2,
                    _ => 3,
                };
                (preference, candidate.1, candidate.0)
            });
            next = available[0];
            previous_direction = (next.0 - current.0, next.1 - current.1);
            guard += 1;
            if guard > 512 * 512 * 4 {
                break;
            }
        }
        if current == start && points.len() >= 4 {
            loops.push(points);
        }
    }
    let mut points = loops
        .into_iter()
        .max_by(|left, right| {
            contour_area(left)
                .abs()
                .partial_cmp(&contour_area(right).abs())
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .unwrap_or_default();
    if points.len() > 256 {
        let original = std::mem::take(&mut points);
        let count = 256usize;
        points = (0..count)
            .map(|index| original[index * original.len() / count])
            .collect();
    }
    points
        .into_iter()
        .map(|(x, y)| json!([(x as f64 / 512.0).clamp(0.0, 1.0), (y as f64 / 512.0).clamp(0.0, 1.0)]))
        .collect()
}

fn grid_direction_index(direction: (i32, i32)) -> i32 {
    match direction {
        (1, 0) => 0,
        (0, 1) => 1,
        (-1, 0) => 2,
        (0, -1) => 3,
        _ => 0,
    }
}

fn contour_area(points: &[(i32, i32)]) -> f64 {
    points
        .iter()
        .enumerate()
        .map(|(index, &(x0, y0))| {
            let (x1, y1) = points[(index + 1) % points.len()];
            x0 as f64 * y1 as f64 - x1 as f64 * y0 as f64
        })
        .sum::<f64>()
        * 0.5
}

fn mask_centroid(mask: &[bool]) -> Option<(f64, f64)> {
    let mut count = 0.0;
    let mut x = 0.0;
    let mut y = 0.0;
    for (index, value) in mask.iter().enumerate() {
        if *value {
            count += 1.0;
            x += (index % 512) as f64;
            y += (index / 512) as f64;
        }
    }
    (count > 0.0).then_some((x / count, y / count))
}

fn camera_orbit_variant(base: &Value, yaw: f64, pitch: f64, distance_scale: f64) -> Value {
    let Some(transform) = base.get("transform").and_then(Value::as_object) else {
        return base.clone();
    };
    let Some(position) = camera_vec3(transform.get("position_m")) else {
        return base.clone();
    };
    let Some(target) = camera_vec3(transform.get("target_m")) else {
        return base.clone();
    };
    let relative = [position[0] - target[0], position[1] - target[1], position[2] - target[2]];
    let horizontal = (relative[0] * relative[0] + relative[2] * relative[2]).sqrt().max(1e-6);
    let base_yaw = relative[0].atan2(relative[2]);
    let base_pitch = (relative[1] / horizontal).atan();
    let yaw = base_yaw + yaw;
    let pitch = (base_pitch + pitch).clamp(-1.25, 1.25);
    let radius = (relative[0] * relative[0] + relative[1] * relative[1] + relative[2] * relative[2]).sqrt() * distance_scale;
    let horizontal = radius * pitch.cos();
    let new_position = [target[0] + horizontal * yaw.sin(), target[1] + radius * pitch.sin(), target[2] + horizontal * yaw.cos()];
    let mut camera = base.clone();
    if let Some(transform) = camera.get_mut("transform").and_then(Value::as_object_mut) {
        transform.insert("position_m".to_owned(), json!(new_position));
    }
    camera["camera_hash"] = Value::String(String::new());
    camera["canonical_sha256"] = Value::String(String::new());
    camera["camera_hash"] = Value::String(canonical_json_hash(&camera));
    camera["canonical_sha256"] = Value::String(canonical_json_hash(&camera));
    camera
}

/// Build the deterministic coarse stage of the camera search. The first
/// stage covers the dominant view variables (yaw, pitch, FOV and distance),
/// plus explicit roll and target-offset probes. The caller ranks these real
/// renders before asking for local refinements.
fn camera_fit_search_variants(base: &Value) -> Vec<Value> {
    let mut coarse = Vec::new();
    let direction_offsets = [-0.18_f64, 0.0, 0.18];
    let framing_offsets = [(-6.0_f64, 0.94_f64), (0.0, 1.0), (6.0, 1.06)];
    for yaw in direction_offsets {
        for pitch in direction_offsets {
            for (fov_delta, distance_scale) in framing_offsets {
                coarse.push(camera_fit_variant_extended(
                    base,
                    yaw,
                    pitch,
                    0.0,
                    fov_delta,
                    distance_scale,
                    0.0,
                    0.0,
                    1.0,
                ));
            }
        }
    }
    // Add roll, target-offset and global-scale probes around the unmodified
    // camera. These are intentionally separate from the direction grid so
    // their effects remain attributable in the returned candidate list.
    for roll in [-0.05_f64, 0.05] {
        coarse.push(camera_fit_variant_extended(
            base, 0.0, 0.0, roll, 0.0, 1.0, 0.0, 0.0, 1.0,
        ));
    }
    for (target_dx, target_dy) in [
        (-0.04_f64, 0.0),
        (0.04, 0.0),
        (0.0, -0.04),
        (0.0, 0.04),
        (-0.04, -0.04),
    ] {
        coarse.push(camera_fit_variant_extended(
            base,
            0.0,
            0.0,
            0.0,
            0.0,
            1.0,
            target_dx,
            target_dy,
            1.0,
        ));
    }
    for global_scale in [0.96_f64, 1.04] {
        coarse.push(camera_fit_variant_extended(
            base,
            0.0,
            0.0,
            0.0,
            0.0,
            1.0,
            0.0,
            0.0,
            global_scale,
        ));
    }
    let mut variants = Vec::with_capacity(37);
    variants.push(base.clone());
    variants.extend(coarse);
    variants.truncate(37);
    variants
}

/// Return nine one-variable probes around a ranked coarse camera. This keeps
/// the second stage attributable and guarantees a hard 64-render ceiling
/// when three seeds are refined (37 coarse + 27 local).
fn camera_fit_refinement_variants(seed: &Value) -> Vec<Value> {
    [
        (0.04_f64, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0),
        (-0.04, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0),
        (0.0, 0.03, 0.0, 0.0, 1.0, 0.0, 0.0),
        (0.0, -0.03, 0.0, 0.0, 1.0, 0.0, 0.0),
        (0.0, 0.0, 0.02, 0.0, 1.0, 0.0, 0.0),
        (0.0, 0.0, -0.02, 0.0, 1.0, 0.0, 0.0),
        (0.0, 0.0, 0.0, 3.0, 1.03, 0.0, 0.0),
        (0.0, 0.0, 0.0, -3.0, 0.97, 0.0, 0.0),
        (0.0, 0.0, 0.0, 0.0, 1.0, 0.02, 0.0),
    ]
    .into_iter()
    .map(|(yaw, pitch, roll, fov_delta, distance_scale, dx, dy)| {
        camera_fit_variant_extended(
            seed,
            yaw,
            pitch,
            roll,
            fov_delta,
            distance_scale,
            dx,
            dy,
            1.0,
        )
    })
    .collect()
}

fn camera_fit_row_from_passes(
    reference_mask: &[bool],
    landmarks: Option<&Value>,
    camera: Value,
    passes: &[render_worker::RenderPass],
    part_ids: &[String],
) -> Result<Value, RuntimeError> {
    validate_camera_calibration(&camera)?;
    let silhouette = passes
        .iter()
        .find(|pass| pass.pass == "silhouette")
        .ok_or_else(|| {
            RuntimeError::InvalidInput(
                "CAMERA_FIT_RENDER_FAILED: silhouette pass missing".to_owned(),
            )
        })?;
    // Camera search is an ordering operation, not the final quality gate.
    // Fit passes are deliberately rendered at the small binary resolution;
    // compare them at that same resolution instead of expanding every row to
    // 512² and repeatedly calculating the full review metric set.  The
    // selected camera is re-rendered through the normal 512² nine-AOV path by
    // reference_compare_prepare, so this cannot lower the product gate.
    const FIT_RESOLUTION: usize = 128;
    let model_mask = decode_binary_mask_at_resolution(&silhouette.png, FIT_RESOLUTION)?;
    let reference_fit_mask = downsample_mask(reference_mask, 512, FIT_RESOLUTION);
    let metrics = camera_fit_metrics_at_resolution(
        &reference_fit_mask,
        &model_mask,
        FIT_RESOLUTION,
    );
    let part_context = passes
        .iter()
        .find(|pass| pass.pass == "part-id")
        .map(|pass| (pass.png.as_slice(), part_ids));
    let mut loss_base = metrics.clone();
    loss_base["sdf_chamfer_px"] = Value::from(stable_visual_metric(
        sdf_chamfer_px_at_resolution(&reference_fit_mask, &model_mask, FIT_RESOLUTION)
            * (512.0 / FIT_RESOLUTION as f64),
    ));
    let loss_metrics = transient_loss_metrics_at_resolution(
        &loss_base,
        &model_mask,
        FIT_RESOLUTION,
        landmarks,
        part_context,
    );
    let loss = camera_fit_loss(&loss_metrics);
    Ok(json!({
        "camera": camera,
        "loss": stable_visual_metric(loss),
        "metrics": metrics
    }))
}

fn camera_fit_variant_extended(
    base: &Value,
    yaw: f64,
    pitch: f64,
    roll: f64,
    fov_delta: f64,
    distance_scale: f64,
    target_dx: f64,
    target_dy: f64,
    global_scale: f64,
) -> Value {
    let mut camera = camera_orbit_variant(base, yaw, pitch, distance_scale * global_scale);
    let Some(transform) = camera.get("transform").and_then(Value::as_object).cloned() else { return camera; };
    let Some(mut position) = camera_vec3(transform.get("position_m")) else { return camera; };
    let Some(mut target) = camera_vec3(transform.get("target_m")) else { return camera; };
    let Some(up) = camera_vec3(transform.get("up")) else { return camera; };
    let view = [target[0] - position[0], target[1] - position[1], target[2] - position[2]];
    let length = (view[0] * view[0] + view[1] * view[1] + view[2] * view[2]).sqrt().max(1e-6);
    let forward = [view[0] / length, view[1] / length, view[2] / length];
    let right_raw = [forward[1] * up[2] - forward[2] * up[1], forward[2] * up[0] - forward[0] * up[2], forward[0] * up[1] - forward[1] * up[0]];
    let right_len = (right_raw[0] * right_raw[0] + right_raw[1] * right_raw[1] + right_raw[2] * right_raw[2]).sqrt().max(1e-6);
    let right = [right_raw[0] / right_len, right_raw[1] / right_len, right_raw[2] / right_len];
    let corrected_up = [right[1] * forward[2] - right[2] * forward[1], right[2] * forward[0] - right[0] * forward[2], right[0] * forward[1] - right[1] * forward[0]];
    let cos = roll.clamp(-0.5, 0.5).cos();
    let sin = roll.clamp(-0.5, 0.5).sin();
    let rolled_up = [corrected_up[0] * cos + right[0] * sin, corrected_up[1] * cos + right[1] * sin, corrected_up[2] * cos + right[2] * sin];
    let shift = [right[0] * target_dx + corrected_up[0] * target_dy, right[1] * target_dx + corrected_up[1] * target_dy, right[2] * target_dx + corrected_up[2] * target_dy];
    for index in 0..3 { target[index] += shift[index]; position[index] += shift[index]; }
    if let Some(transform) = camera.get_mut("transform").and_then(Value::as_object_mut) {
        transform.insert("position_m".to_owned(), json!(position));
        transform.insert("target_m".to_owned(), json!(target));
        transform.insert("up".to_owned(), json!(rolled_up));
    }
    if let Some(fov) = camera.get("fov_y_degrees").and_then(Value::as_f64) {
        camera["fov_y_degrees"] = json!((fov + fov_delta).clamp(15.0, 100.0));
    }
    camera["camera_hash"] = Value::String(String::new());
    camera["canonical_sha256"] = Value::String(String::new());
    camera["camera_hash"] = Value::String(canonical_json_hash(&camera));
    camera["canonical_sha256"] = Value::String(canonical_json_hash(&camera));
    camera
}

fn extended_silhouette_metrics(reference: &[bool], model: &[bool]) -> Value {
    let base = compare_masks(reference, model, &json!({"landmarks":[],"regions":[]}));
    json!({
        "silhouette_iou":base["silhouette_iou"],
        "boundary_f1_4px":base["boundary_f1_4px"],
        "bbox_edge_error":base["bbox_edge_error"],
        "centroid_error":base["centroid_error"],
        "sdf_chamfer_px":stable_visual_metric(sdf_chamfer_px(reference, model))
    })
}

fn extended_silhouette_loss(metrics: &Value) -> f64 {
    weighted_contour_loss(metrics)
}

/// Deterministic image-space loss used by camera and candidate ranking.
/// Chamfer and IoU dominate the fit, while boundary, framing and centroid
/// keep a candidate from winning by matching only its occupied area.  Landmark
/// and semantic-Part terms are intentionally optional: the transient camera
/// batch does not invent them when a target has no typed annotations, but a
/// future annotated fit can add those bounded penalties without changing the
/// ranking contract.
fn weighted_contour_loss(metrics: &Value) -> f64 {
    let chamfer = (metrics
        .get("sdf_chamfer_px")
        .and_then(Value::as_f64)
        .unwrap_or(512.0)
        / 512.0)
        .clamp(0.0, 1.0);
    let iou = (1.0 - metrics
        .get("silhouette_iou")
        .and_then(Value::as_f64)
        .unwrap_or(0.0))
        .clamp(0.0, 1.0);
    let boundary = (1.0 - metrics
        .get("boundary_f1_4px")
        .and_then(Value::as_f64)
        .unwrap_or(0.0))
        .clamp(0.0, 1.0);
    let bbox = metrics
        .get("bbox_edge_error")
        .and_then(Value::as_f64)
        .unwrap_or(1.0)
        .clamp(0.0, 1.0);
    let centroid = metrics
        .get("centroid_error")
        .and_then(Value::as_f64)
        .unwrap_or(1.0)
        .clamp(0.0, 1.0);
    // The 5% regularization reserve is zero for the current fixed camera
    // batch.  Keeping it explicit makes the weighting auditable and leaves a
    // bounded slot for symmetry/connection regularization when Rig fitting
    // supplies that typed value.
    let regularization = metrics
        .get("regularization_penalty")
        .and_then(Value::as_f64)
        .unwrap_or(0.0)
        .clamp(0.0, 1.0);
    if let Some(landmark_nme) = metrics.get("landmark_nme").and_then(Value::as_f64) {
        // When Luna supplies image-derived landmarks, reserve an auditable
        // 10% of the bounded objective for landmark evidence.  Split that
        // weight between reprojection miss and coverage: a camera that puts
        // one landmark in an excellent position while dropping the other
        // observed anchors must not beat a framing that covers the whole
        // observed body.  Targets without landmarks retain the contour-only
        // objective below.
        let landmark_nme = landmark_nme.clamp(0.0, 1.0);
        let landmark_coverage = metrics
            .get("landmark_coverage")
            .and_then(Value::as_f64)
            .unwrap_or(0.0)
            .clamp(0.0, 1.0);
        0.315 * chamfer
            + 0.225 * iou
            + 0.135 * boundary
            + 0.09 * bbox
            + 0.09 * centroid
            + 0.05 * landmark_nme
            + 0.05 * (1.0 - landmark_coverage)
            + 0.045 * regularization
    } else {
        0.35 * chamfer
            + 0.25 * iou
            + 0.15 * boundary
            + 0.10 * bbox
            + 0.10 * centroid
            + 0.05 * regularization
    }
}

fn transient_loss_metrics_with_parts(
    base: &Value,
    model: &[bool],
    landmarks: Option<&Value>,
    part_context: Option<(&[u8], &[String])>,
) -> Value {
    let Some(values) = landmarks.and_then(Value::as_array) else {
        return base.clone();
    };
    if values.is_empty() {
        return base.clone();
    }
    let view_spec = json!({"landmarks": values});
    let (coverage, nme) = landmark_metrics_with_parts(model, &view_spec, part_context);
    let mut metrics = base.clone();
    if let Some(object) = metrics.as_object_mut() {
        object.insert("landmark_coverage".to_owned(), Value::from(stable_visual_metric(coverage)));
        object.insert("landmark_nme".to_owned(), Value::from(stable_visual_metric(nme)));
    }
    metrics
}

#[cfg(test)]
fn contour_loss_metrics(base: &Value, model: &[bool], landmarks: Option<&Value>) -> Value {
    transient_loss_metrics_with_parts(base, model, landmarks, None)
}

fn sdf_chamfer_px(reference: &[bool], model: &[bool]) -> f64 {
    let reference_boundary = boundary_mask(reference);
    let model_boundary = boundary_mask(model);
    let reference_field = boundary_distance_field(&reference_boundary);
    let model_field = boundary_distance_field(&model_boundary);
    let mut left = 0.0;
    let mut right = 0.0;
    let mut left_count = 0.0;
    let mut right_count = 0.0;
    for index in 0..reference_boundary.len() {
        if reference_boundary[index] { left += model_field[index]; left_count += 1.0; }
        if model_boundary[index] { right += reference_field[index]; right_count += 1.0; }
    }
    if left_count == 0.0 || right_count == 0.0 { return 512.0; }
    ((left / left_count) + (right / right_count)) * 0.5
}

fn boundary_distance_field(boundary: &[bool]) -> Vec<f64> {
    const INF: f64 = 1.0e9;
    let mut field = boundary.iter().map(|value| if *value { 0.0 } else { INF }).collect::<Vec<_>>();
    for y in 0..512usize {
        for x in 0..512usize {
            let index = y * 512 + x;
            let mut value = field[index];
            for (dx, dy, weight) in [(-1_i32, 0_i32, 1.0), (0, -1, 1.0), (-1, -1, 1.4142), (1, -1, 1.4142)] {
                let nx = x as i32 + dx; let ny = y as i32 + dy;
                if nx >= 0 && ny >= 0 && nx < 512 && ny < 512 { value = value.min(field[ny as usize * 512 + nx as usize] + weight); }
            }
            field[index] = value;
        }
    }
    for y in (0..512usize).rev() {
        for x in (0..512usize).rev() {
            let index = y * 512 + x;
            let mut value = field[index];
            for (dx, dy, weight) in [(1_i32, 0_i32, 1.0), (0, 1, 1.0), (1, 1, 1.4142), (-1, 1, 1.4142)] {
                let nx = x as i32 + dx; let ny = y as i32 + dy;
                if nx >= 0 && ny >= 0 && nx < 512 && ny < 512 { value = value.min(field[ny as usize * 512 + nx as usize] + weight); }
            }
            field[index] = value;
        }
    }
    field
}

fn validate_silhouette_rig(value: &Value, candidate_id: &str) -> Result<(), RuntimeError> {
    let object = exact_object(value, &["schema_version", "rig_id", "candidate_id", "parameters", "canonical_sha256"], "SilhouetteRig@1")?;
    if object.get("schema_version").and_then(Value::as_str) != Some("SilhouetteRig@1") || object.get("candidate_id").and_then(Value::as_str) != Some(candidate_id) { return Err(RuntimeError::InvalidInput("SILHOUETTE_RIG_INVALID: candidate binding".to_owned())); }
    required_contract_identifier(object, "rig_id", "SilhouetteRig@1")?;
    let parameters = object.get("parameters").and_then(Value::as_array).ok_or_else(|| RuntimeError::InvalidInput("SILHOUETTE_RIG_INVALID: parameters".to_owned()))?;
    if parameters.is_empty() || parameters.len() > 64 { return Err(RuntimeError::InvalidInput("SILHOUETTE_RIG_INVALID: parameter budget".to_owned())); }
    let mut ids = std::collections::HashSet::new();
    for parameter in parameters {
        let entry = exact_object(parameter, &["parameter_id", "part_id", "semantic", "value", "min", "max", "step", "unit"], "SilhouetteRig@1.parameter")?;
        let parameter_id = required_contract_identifier(entry, "parameter_id", "SilhouetteRig@1.parameter")?;
        required_contract_identifier(entry, "part_id", "SilhouetteRig@1.parameter")?;
        if !ids.insert(parameter_id) { return Err(RuntimeError::InvalidInput("SILHOUETTE_RIG_INVALID: duplicate parameter_id".to_owned())); }
        let value = entry.get("value").and_then(Value::as_f64).unwrap_or(f64::NAN);
        let min = entry.get("min").and_then(Value::as_f64).unwrap_or(f64::NAN);
        let max = entry.get("max").and_then(Value::as_f64).unwrap_or(f64::NAN);
        let step = entry.get("step").and_then(Value::as_f64).unwrap_or(f64::NAN);
        if !value.is_finite() || !min.is_finite() || !max.is_finite() || !step.is_finite() || min > value || value > max || step <= 0.0 || min >= max { return Err(RuntimeError::InvalidInput("SILHOUETTE_RIG_INVALID: parameter bounds".to_owned())); }
        if !matches!(entry.get("semantic").and_then(Value::as_str), Some("width" | "height" | "depth" | "offset_x" | "offset_y" | "offset_z" | "scale")) { return Err(RuntimeError::InvalidInput("SILHOUETTE_RIG_INVALID: semantic".to_owned())); }
        if !matches!(entry.get("unit").and_then(Value::as_str), Some("meter" | "ratio")) { return Err(RuntimeError::InvalidInput("SILHOUETTE_RIG_INVALID: unit".to_owned())); }
    }
    required_contract_sha256(object, "canonical_sha256", "SilhouetteRig@1")?;
    verify_output_canonical_hash(value, "SilhouetteRig@1")
}

fn fit_rig_parameters(rig: &Value, target: &[bool], model: &[bool]) -> Vec<Value> {
    let (width_ratio, height_ratio) = bbox_axis_ratios(target, model);
    rig.get("parameters").and_then(Value::as_array).map(|parameters| parameters.iter().map(|parameter| {
        let value = parameter.get("value").and_then(Value::as_f64).unwrap_or(0.0);
        let min = parameter.get("min").and_then(Value::as_f64).unwrap_or(value);
        let max = parameter.get("max").and_then(Value::as_f64).unwrap_or(value);
        let semantic = parameter.get("semantic").and_then(Value::as_str).unwrap_or("");
        let multiplier = match semantic {
            "width" => width_ratio,
            "height" => height_ratio,
            "scale" => (width_ratio * height_ratio).sqrt().clamp(0.5, 1.5),
            _ => 1.0,
        };
        json!({"parameter_id":parameter.get("parameter_id").and_then(Value::as_str).unwrap_or("unknown"),"part_id":parameter.get("part_id").and_then(Value::as_str).unwrap_or("unknown"),"value":stable_visual_metric((value * multiplier).clamp(min, max))})
    }).collect()).unwrap_or_default()
}

/// Produce bounded Rig values using local Part envelopes when the target has
/// explicit contour slices and the selected render has a Part-ID pass.  A
/// missing slice or Part-ID readback deliberately falls back to the legacy
/// whole-body proposal; it never guesses a semantic owner from a free-form
/// region or from hidden geometry.
fn fit_rig_parameters_with_part_context(
    rig: &Value,
    target: &Value,
    target_mask: &[bool],
    model_mask: &[bool],
    part_context: Option<(&[u8], &[String])>,
) -> Vec<Value> {
    let Some(parameters) = rig.get("parameters").and_then(Value::as_array) else {
        return Vec::new();
    };
    let global = fit_rig_parameters(rig, target_mask, model_mask);
    let Some((part_png, part_ids)) = part_context else {
        return global;
    };
    let mut local_envelopes: HashMap<String, (MaskEnvelope, MaskEnvelope)> = HashMap::new();
    if let Some(parts) = target.get("parts").and_then(Value::as_array) {
        for part in parts {
            let Some(part_id) = part.get("part_id").and_then(Value::as_str) else {
                continue;
            };
            let Some(target_part) = target_part_boundary_mask(target, part_id)
                .and_then(|mask| mask_envelope(&mask))
            else {
                continue;
            };
            let Some(model_part) = decode_part_mask(part_png, part_id, part_ids)
                .and_then(|mask| mask_envelope(&mask))
            else {
                continue;
            };
            local_envelopes.insert(part_id.to_owned(), (target_part, model_part));
        }
    }
    if local_envelopes.is_empty() {
        return global;
    }
    parameters
        .iter()
        .enumerate()
        .map(|(index, parameter)| {
            let part_id = parameter.get("part_id").and_then(Value::as_str);
            let value = parameter.get("value").and_then(Value::as_f64).unwrap_or(0.0);
            let min = parameter.get("min").and_then(Value::as_f64).unwrap_or(value);
            let max = parameter.get("max").and_then(Value::as_f64).unwrap_or(value);
            let proposed = part_id
                .and_then(|id| local_envelopes.get(id))
                .map(|(target_envelope, model_envelope)| {
                    (value + local_part_parameter_delta(parameter, *target_envelope, *model_envelope))
                        .clamp(min, max)
                })
                .unwrap_or_else(|| {
                    global
                        .get(index)
                        .and_then(|row| row.get("value"))
                        .and_then(Value::as_f64)
                        .unwrap_or(value)
                });
            json!({
                "parameter_id":parameter.get("parameter_id").and_then(Value::as_str).unwrap_or("unknown"),
                "part_id":part_id.unwrap_or("unknown"),
                "value":stable_visual_metric(proposed)
            })
        })
        .collect()
}

/// Refine the bounded Part-envelope proposal with image-derived landmark
/// offsets.  This is intentionally a second, typed evidence projection: the
/// target landmark vocabulary is mapped to a Runtime-owned semantic Part,
/// the selected Render Worker Part-ID mask supplies the current anchor, and
/// the calibrated camera converts the normalized image error to a camera-plane
/// meter offset.  No free-form region or hidden geometry is guessed.
fn fit_rig_parameters_with_landmark_context(
    rig: &Value,
    target: &Value,
    target_mask: &[bool],
    model_mask: &[bool],
    part_context: Option<(&[u8], &[String])>,
    camera: Option<&Value>,
) -> Vec<Value> {
    let mut selected = fit_rig_parameters_with_part_context(
        rig,
        target,
        target_mask,
        model_mask,
        part_context,
    );
    let Some(camera) = camera else {
        return selected;
    };
    let Some((part_png, part_ids)) = part_context else {
        return selected;
    };
    let Some(parameters) = rig.get("parameters").and_then(Value::as_array) else {
        return selected;
    };
    let Some(landmarks) = target.get("landmarks").and_then(Value::as_array) else {
        return selected;
    };
    let Some((world_per_screen_x, world_per_screen_y)) = camera_plane_world_scales(camera) else {
        return selected;
    };
    for (index, parameter) in parameters.iter().enumerate() {
        let semantic = parameter.get("semantic").and_then(Value::as_str).unwrap_or("");
        if !matches!(semantic, "offset_x" | "offset_y") {
            continue;
        }
        let Some(part_id) = parameter.get("part_id").and_then(Value::as_str) else {
            continue;
        };
        let Some(part_mask) = decode_part_mask(part_png, part_id, part_ids) else {
            continue;
        };
        let mut target_x = 0.0;
        let mut target_y = 0.0;
        let mut model_x = 0.0;
        let mut model_y = 0.0;
        let mut weight_total = 0.0;
        for landmark in landmarks {
            if landmark.get("visibility").and_then(Value::as_str) == Some("unknown") {
                continue;
            }
            let Some(landmark_id) = landmark.get("landmark_id").and_then(Value::as_str) else {
                continue;
            };
            let Some((landmark_part_id, anchor)) = landmark_part_hint(landmark_id) else {
                continue;
            };
            if !rig_part_matches_output(part_id, landmark_part_id)
                && !rig_part_matches_output(landmark_part_id, part_id)
            {
                continue;
            }
            let target_point = (
                landmark.get("x").and_then(Value::as_f64).unwrap_or(-1.0),
                landmark.get("y").and_then(Value::as_f64).unwrap_or(-1.0),
            );
            if !(0.0..=1.0).contains(&target_point.0)
                || !(0.0..=1.0).contains(&target_point.1)
            {
                continue;
            }
            let Some(model_point) = landmark_anchor_point(&part_mask, anchor) else {
                continue;
            };
            let confidence = landmark
                .get("confidence")
                .and_then(Value::as_f64)
                .unwrap_or(1.0)
                .clamp(0.25, 1.0);
            target_x += target_point.0 * confidence;
            target_y += target_point.1 * confidence;
            model_x += model_point.0 * confidence;
            model_y += model_point.1 * confidence;
            weight_total += confidence;
        }
        if weight_total == 0.0 {
            continue;
        }
        let delta = if semantic == "offset_x" {
            (target_x / weight_total - model_x / weight_total)
                * world_per_screen_x
        } else {
            // Image Y grows downward while the calibrated camera-plane up
            // basis grows upward.
            (model_y / weight_total - target_y / weight_total)
                * world_per_screen_y
        };
        let value = parameter.get("value").and_then(Value::as_f64).unwrap_or(0.0);
        let min = parameter.get("min").and_then(Value::as_f64).unwrap_or(value);
        let max = parameter.get("max").and_then(Value::as_f64).unwrap_or(value);
        let Some(selected_row) = selected.get_mut(index) else {
            continue;
        };
        selected_row["value"] = Value::from(stable_visual_metric(
            (value + delta.clamp(-0.35, 0.35)).clamp(min, max),
        ));
    }
    selected
}

/// Project visible boundary correspondences onto the Rig controls owned by
/// the corresponding Part. This is deliberately a proposal projection, not
/// an optimizer: it uses only the bounded Runtime boundary sample and at most
/// one value per typed parameter. Width/height use the observed screen-space
/// envelope ratio; offsets use the calibrated camera plane when the Rig uses
/// meters. A Part with no attributed boundary evidence is left unchanged.
fn apply_boundary_part_parameter_projection(
    rig: &Value,
    selected_parameters: &[Value],
    segments: &[Value],
    camera: Option<&Value>,
) -> Vec<Value> {
    let mut grouped: HashMap<String, Vec<(f64, f64, f64, f64)>> = HashMap::new();
    for segment in segments {
        let Some(part_id) = segment.get("part_id").and_then(Value::as_str) else {
            continue;
        };
        let distance = segment
            .get("distance_px")
            .and_then(Value::as_f64)
            .unwrap_or(0.0);
        if distance <= 4.0 {
            continue;
        }
        let Some(reference) = segment.get("reference").and_then(Value::as_array) else {
            continue;
        };
        let Some(model) = segment.get("model").and_then(Value::as_array) else {
            continue;
        };
        let (Some(reference_x), Some(reference_y), Some(model_x), Some(model_y)) = (
            reference.first().and_then(Value::as_f64),
            reference.get(1).and_then(Value::as_f64),
            model.first().and_then(Value::as_f64),
            model.get(1).and_then(Value::as_f64),
        ) else {
            continue;
        };
        if [reference_x, reference_y, model_x, model_y]
            .iter()
            .any(|value| !value.is_finite())
        {
            continue;
        }
        grouped
            .entry(part_id.to_owned())
            .or_default()
            .push((reference_x, reference_y, model_x, model_y));
    }
    if grouped.is_empty() {
        return selected_parameters.to_vec();
    }
    let camera_scales = camera.and_then(camera_plane_world_scales);
    let Some(definitions) = rig.get("parameters").and_then(Value::as_array) else {
        return selected_parameters.to_vec();
    };
    let mut projected = selected_parameters.to_vec();
    for (index, definition) in definitions.iter().enumerate() {
        let Some(part_id) = definition.get("part_id").and_then(Value::as_str) else {
            continue;
        };
        // The typed Rig may own a bilateral `*-pair` while the Render Worker
        // exposes the visible AOV as fixed `*-left`/`*-right` Part IDs. Merge
        // those aliases before computing the local envelope; otherwise the
        // dominant Part can be selected correctly but receives no boundary
        // width/height/offset proposal.
        let points = grouped
            .iter()
            .filter(|(observed_part_id, _)| {
                rig_part_matches_observed_part(part_id, observed_part_id)
            })
            .flat_map(|(_, points)| points.iter().copied())
            .collect::<Vec<_>>();
        if points.len() < 2 {
            continue;
        }
        let target_min_x = points.iter().map(|point| point.0).fold(1.0, f64::min);
        let target_max_x = points.iter().map(|point| point.0).fold(0.0, f64::max);
        let target_min_y = points.iter().map(|point| point.1).fold(1.0, f64::min);
        let target_max_y = points.iter().map(|point| point.1).fold(0.0, f64::max);
        let model_min_x = points.iter().map(|point| point.2).fold(1.0, f64::min);
        let model_max_x = points.iter().map(|point| point.2).fold(0.0, f64::max);
        let model_min_y = points.iter().map(|point| point.3).fold(1.0, f64::min);
        let model_max_y = points.iter().map(|point| point.3).fold(0.0, f64::max);
        let target_width = (target_max_x - target_min_x).max(1.0 / 511.0);
        let model_width = (model_max_x - model_min_x).max(1.0 / 511.0);
        let target_height = (target_max_y - target_min_y).max(1.0 / 511.0);
        let model_height = (model_max_y - model_min_y).max(1.0 / 511.0);
        let width_ratio = (target_width / model_width).clamp(0.5, 1.5);
        let height_ratio = (target_height / model_height).clamp(0.5, 1.5);
        let target_center_x = (target_min_x + target_max_x) * 0.5;
        let model_center_x = (model_min_x + model_max_x) * 0.5;
        let target_center_y = (target_min_y + target_max_y) * 0.5;
        let model_center_y = (model_min_y + model_max_y) * 0.5;
        let center_delta_x = (target_center_x - model_center_x).clamp(-0.5, 0.5);
        let center_delta_y = (model_center_y - target_center_y).clamp(-0.5, 0.5);
        let semantic = definition.get("semantic").and_then(Value::as_str).unwrap_or("");
        let unit = definition.get("unit").and_then(Value::as_str).unwrap_or("meter");
        let base_value = definition.get("value").and_then(Value::as_f64).unwrap_or(0.0);
        let min = definition.get("min").and_then(Value::as_f64).unwrap_or(base_value);
        let max = definition.get("max").and_then(Value::as_f64).unwrap_or(base_value);
        let desired = match semantic {
            "width" => Some(base_value * width_ratio),
            "height" => Some(base_value * height_ratio),
            "offset_x" => {
                let delta = if unit == "ratio" {
                    center_delta_x
                } else {
                    let Some((world_per_screen_x, _)) = camera_scales else {
                        continue;
                    };
                    center_delta_x * world_per_screen_x
                };
                Some(base_value + delta)
            }
            "offset_y" => {
                let delta = if unit == "ratio" {
                    center_delta_y
                } else {
                    let Some((_, world_per_screen_y)) = camera_scales else {
                        continue;
                    };
                    center_delta_y * world_per_screen_y
                };
                Some(base_value + delta)
            }
            _ => None,
        };
        let Some(desired) = desired else {
            continue;
        };
        let Some(selected) = projected.get_mut(index) else {
            continue;
        };
        if selected.get("parameter_id") != definition.get("parameter_id") {
            continue;
        }
        selected["value"] = Value::from(stable_visual_metric(desired.clamp(min, max)));
    }
    projected
}

/// Apply a typed, candidate-bound Rig proposal to the corresponding V2
/// primitive nodes.  This is deliberately a small projection layer rather
/// than a general expression evaluator: only dimensions and image-plane
/// offsets on the fixed operator parameter objects are writable.  Unknown
/// Parts/operators are left untouched and reported through the applied count.
fn materialize_rig_geometry_program(
    program: &Value,
    rig: &Value,
    selected_parameters: &[Value],
    camera: Option<&Value>,
) -> Result<(Value, usize), RuntimeError> {
    let mut materialized = program.clone();
    let outputs = materialized
        .get("part_outputs")
        .and_then(Value::as_array)
        .ok_or_else(|| RuntimeError::InvalidInput("SILHOUETTE_FIT_GEOMETRY_FAILED: GeometryProgram part_outputs are missing".to_owned()))?
        .clone();
    let nodes = materialized
        .get_mut("nodes")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| RuntimeError::InvalidInput("SILHOUETTE_FIT_GEOMETRY_FAILED: GeometryProgram nodes are missing".to_owned()))?;
    let definitions = rig
        .get("parameters")
        .and_then(Value::as_array)
        .ok_or_else(|| RuntimeError::InvalidInput("SILHOUETTE_FIT_GEOMETRY_FAILED: Rig parameters are missing".to_owned()))?;
    let mut applied = 0usize;
    for (index, definition) in definitions.iter().enumerate() {
        let Some(selected) = selected_parameters.get(index) else { continue; };
        let Some(parameter_id) = definition.get("parameter_id").and_then(Value::as_str) else { continue; };
        if selected.get("parameter_id").and_then(Value::as_str) != Some(parameter_id) {
            return Err(RuntimeError::InvalidInput("SILHOUETTE_FIT_GEOMETRY_FAILED: Rig parameter order drifted".to_owned()));
        }
        let Some(part_id) = definition.get("part_id").and_then(Value::as_str) else { continue; };
        let Some(value) = selected.get("value").and_then(Value::as_f64) else { continue; };
        let base_value = definition.get("value").and_then(Value::as_f64).unwrap_or(value);
        let semantic = definition.get("semantic").and_then(Value::as_str).unwrap_or_default();
        let unit = definition.get("unit").and_then(Value::as_str).unwrap_or("meter");
        let mut changed = false;
        // A Rig uses stable semantic pair IDs (for example
        // `shoulder-armor-pair`) while a primitive blockout may expose the
        // two visible sides as `shoulder-left` and `shoulder-right`.  Resolve
        // all matching sinks instead of silently applying zero geometry
        // trials when the authoring route chose explicit left/right Parts.
        for output in outputs.iter().filter(|output| {
            output
                .get("part_id")
                .and_then(Value::as_str)
                .is_some_and(|output_part_id| rig_part_matches_output(part_id, output_part_id))
        }) {
            let Some(input_node_ids) = output.get("input_node_ids").and_then(Value::as_array) else { continue; };
            let mut affected = Vec::new();
            let mut visited = HashSet::new();
            for input_node_id in input_node_ids.iter().filter_map(Value::as_str) {
                collect_geometry_node_indices(&nodes, input_node_id, &mut visited, &mut affected);
            }
            let has_transform = affected.iter().any(|index| {
                nodes
                    .get(*index)
                    .and_then(|node| node.get("operator_id"))
                    .and_then(Value::as_str)
                    == Some("forgecad.geometry.transform@2")
            });
            for node_index in affected {
                let Some(node) = nodes.get_mut(node_index) else { continue; };
                let operator_id = node
                    .get("operator_id")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned();
                let Some(parameters) = node.get_mut("parameters").and_then(Value::as_object_mut) else { continue; };
                // Dimensions belong to the first-party source geometry node.
                // An offset/scale belongs to the final transform when one
                // exists; tracing the DAG keeps mirror/array/part-output
                // sinks editable without applying the same delta twice.
                let is_transform = operator_id == "forgecad.geometry.transform@2";
                let is_source_geometry = !matches!(
                    operator_id.as_str(),
                    "forgecad.geometry.transform@2"
                        | "forgecad.geometry.mirror@1"
                        | "forgecad.geometry.array@1"
                        | "forgecad.geometry.part-output@1"
                );
                let should_apply = match semantic {
                    "offset_x" | "offset_y" | "offset_z" | "scale" => {
                        if has_transform { is_transform } else { is_source_geometry }
                    }
                    "width" | "height" | "depth" => is_source_geometry,
                    _ => false,
                };
                if should_apply
                    && apply_rig_parameter_to_node(
                        parameters,
                        semantic,
                        unit,
                        value,
                        base_value,
                        camera,
                    )
                {
                    changed = true;
                }
            }
        }
        if changed { applied += 1; }
    }
    Ok((materialized, applied))
}

/// Return whether a typed Rig parameter owns a rendered semantic Part.  The
/// canonical detail route uses pair sinks directly, while the primitive
/// blockout route keeps explicit left/right outputs so it can remain a simple
/// closed leaf program.  This aliasing is deliberately limited to the fixed
/// bilateral naming convention; it is not a free-form fuzzy matcher.
fn rig_part_matches_output(rig_part_id: &str, output_part_id: &str) -> bool {
    if rig_part_id == output_part_id {
        return true;
    }
    let Some(stem) = rig_part_id.strip_suffix("-pair") else {
        return false;
    };
    let mut bases = vec![stem.to_owned()];
    if let Some(base) = stem.strip_suffix("-armor") {
        bases.push(base.to_owned());
    }
    bases.iter().any(|base| {
        output_part_id == format!("{base}-left") || output_part_id == format!("{base}-right")
    })
}

fn collect_geometry_node_indices(
    nodes: &[Value],
    node_id: &str,
    visited: &mut HashSet<String>,
    indices: &mut Vec<usize>,
) {
    if !visited.insert(node_id.to_owned()) {
        return;
    }
    let Some((index, node)) = nodes
        .iter()
        .enumerate()
        .find(|(_, node)| node.get("node_id").and_then(Value::as_str) == Some(node_id))
    else {
        return;
    };
    indices.push(index);
    let inputs = node
        .get("inputs")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    for input in inputs.iter().filter_map(Value::as_str) {
        collect_geometry_node_indices(nodes, input, visited, indices);
    }
}

fn apply_rig_parameter_to_node(
    parameters: &mut serde_json::Map<String, Value>,
    semantic: &str,
    unit: &str,
    value: f64,
    base_value: f64,
    camera: Option<&Value>,
) -> bool {
    if !value.is_finite() || !base_value.is_finite() {
        return false;
    }
    let ratio = if unit == "ratio" {
        value / base_value.abs().max(1e-6)
    } else {
        1.0
    };
    let delta = if unit == "ratio" { value - base_value } else { value };
    if !ratio.is_finite() || !delta.is_finite() || ratio <= 0.0 {
        return false;
    }
    match semantic {
        "offset_x" | "offset_y" | "offset_z" => {
            let key = if parameters.contains_key("translation_m") {
                "translation_m"
            } else {
                "position_m"
            };
            let Some(vector) = parameters.get_mut(key).and_then(Value::as_array_mut) else {
                return false;
            };
            if vector.len() != 3 {
                return false;
            }
            if let Some(camera) = camera.filter(|_| matches!(semantic, "offset_x" | "offset_y")) {
                let Some((right, up)) = camera_plane_axes(camera) else {
                    return false;
                };
                let basis = if semantic == "offset_x" { right } else { up };
                for axis in 0..3 {
                    let old = vector[axis].as_f64().unwrap_or(0.0);
                    let next = old + basis[axis] * delta;
                    if !next.is_finite() {
                        return false;
                    }
                    vector[axis] = Value::from(next);
                }
                return true;
            }
            let axis = match semantic {
                "offset_x" => 0,
                "offset_y" => 1,
                _ => 2,
            };
            let old = vector[axis].as_f64().unwrap_or(0.0);
            let next = old + delta;
            if !next.is_finite() {
                return false;
            }
            vector[axis] = Value::from(next);
            true
        }
        "width" | "height" | "depth" => {
            let axis = match semantic {
                "width" => 0,
                "height" => 1,
                _ => 2,
            };
            let scale_dimension = |old: f64| {
                if unit == "ratio" { old * ratio } else { value }
            };
            let mut changed = false;
            for key in ["size_m", "radii_m"] {
                if let Some(vector) = parameters.get_mut(key).and_then(Value::as_array_mut) {
                    if vector.len() == 3 {
                        let old = vector[axis].as_f64().unwrap_or(0.0);
                        let next = scale_dimension(old);
                        if next.is_finite() && next > 0.0 {
                            vector[axis] = Value::from(next);
                            changed = true;
                        }
                    }
                }
            }
            let scalar_key = match semantic {
                "width" => "width_m",
                "height" => "height_m",
                _ => "depth_m",
            };
            if let Some(old) = parameters.get(scalar_key).and_then(Value::as_f64) {
                let next = scale_dimension(old);
                if next.is_finite() && next > 0.0 {
                    parameters.insert(scalar_key.to_owned(), Value::from(next));
                    changed = true;
                }
            }
            if semantic == "depth" {
                for key in ["thickness_m", "radius_m"] {
                    if let Some(old) = parameters.get(key).and_then(Value::as_f64) {
                        let next = scale_dimension(old);
                        if next.is_finite() && next > 0.0 {
                            parameters.insert(key.to_owned(), Value::from(next));
                            changed = true;
                        }
                    }
                }
            } else if parameters.get("radius_m").is_some() {
                if let Some(old) = parameters.get("radius_m").and_then(Value::as_f64) {
                    let next = scale_dimension(old);
                    if next.is_finite() && next > 0.0 {
                        parameters.insert("radius_m".to_owned(), Value::from(next));
                        changed = true;
                    }
                }
            }
            // Profile-based operators expose their silhouette in point arrays,
            // not size_m. Scale only the requested image-plane axis; depth is
            // handled by depth_m/thickness_m above.
            if semantic != "depth" {
                for key in ["profile", "path"] {
                    if let Some(points) = parameters.get_mut(key).and_then(Value::as_array_mut) {
                        for point in points {
                            if let Some(pair) = point.as_array_mut() {
                                if pair.len() >= 2 {
                                    let coordinate = pair[axis].as_f64().unwrap_or(0.0);
                                    pair[axis] = Value::from(coordinate * ratio);
                                    changed = true;
                                }
                            }
                        }
                    }
                }
                if let Some(profiles) = parameters.get_mut("profiles").and_then(Value::as_array_mut) {
                    for profile in profiles {
                        if let Some(points) = profile.get_mut("points").and_then(Value::as_array_mut) {
                            for point in points {
                                if let Some(pair) = point.as_array_mut() {
                                    if pair.len() >= 2 {
                                        let coordinate = pair[axis].as_f64().unwrap_or(0.0);
                                        pair[axis] = Value::from(coordinate * ratio);
                                        changed = true;
                                    }
                                }
                            }
                        }
                    }
                }
            }
            changed
        }
        "scale" => {
            let mut changed = false;
            if let Some(vector) = parameters.get_mut("scale").and_then(Value::as_array_mut) {
                for item in vector.iter_mut() {
                    if let Some(old) = item.as_f64() {
                        let next = old * ratio;
                        if next.is_finite() && next > 0.0 {
                            *item = Value::from(next);
                            changed = true;
                        }
                    }
                }
            }
            for key in ["size_m", "radii_m"] {
                if let Some(vector) = parameters.get_mut(key).and_then(Value::as_array_mut) {
                    for item in vector.iter_mut() {
                        if let Some(old) = item.as_f64() {
                            let next = old * ratio;
                            if next.is_finite() && next > 0.0 {
                                *item = Value::from(next);
                                changed = true;
                            }
                        }
                    }
                }
            }
            for key in ["radius_m", "width_m", "height_m", "depth_m", "thickness_m"] {
                if let Some(old) = parameters.get(key).and_then(Value::as_f64) {
                    let next = old * ratio;
                    if next.is_finite() && next > 0.0 {
                        parameters.insert(key.to_owned(), Value::from(next));
                        changed = true;
                    }
                }
            }
            changed
        }
        _ => false,
    }
}

fn finalize_v2_geometry_program(mut draft: Value) -> Result<Value, RuntimeError> {
    // Runtime-created float Values have not crossed the JSON IPC boundary
    // yet. Serialize and parse once before hashing so the in-memory draft has
    // exactly the same serde_json number representation that the isolated
    // Geometry Worker receives. Without this normalization, the Worker can
    // validate one canonical hash while Runtime stores a different byte hash
    // for the same logical Rig edit.
    draft = serde_json::from_slice(
        &serde_json::to_vec(&draft)
            .map_err(|error| RuntimeError::InvalidInput(format!(
                "SILHOUETTE_FIT_GEOMETRY_FAILED: program serialization failed: {error}"
            )))?,
    )
    .map_err(|error| {
        RuntimeError::InvalidInput(format!(
            "SILHOUETTE_FIT_GEOMETRY_FAILED: program round-trip failed: {error}"
        ))
    })?;
    draft
        .as_object_mut()
        .ok_or_else(|| RuntimeError::InvalidInput("SILHOUETTE_FIT_GEOMETRY_FAILED: program is not an object".to_owned()))?
        .remove("canonical_sha256");
    let hash = hash_geometry_program_with_runtime_worker(&draft)
        .map_err(|error| RuntimeError::InvalidInput(format!("SILHOUETTE_FIT_GEOMETRY_FAILED: {error}")))?;
    let canonical = hash
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| RuntimeError::InvalidInput("SILHOUETTE_FIT_GEOMETRY_FAILED: GeometryProgram hash is missing".to_owned()))?
        .to_owned();
    draft["canonical_sha256"] = Value::String(canonical);
    Ok(draft)
}

fn rig_parameter_deltas(rig: &Value, selected_parameters: &[Value]) -> Vec<Value> {
    rig.get("parameters")
        .and_then(Value::as_array)
        .map(|parameters| {
            parameters.iter().enumerate().filter_map(|(index, parameter)| {
                let selected = selected_parameters.get(index)?;
                let parameter_id = parameter.get("parameter_id").and_then(Value::as_str)?;
                let part_id = parameter.get("part_id").and_then(Value::as_str)?;
                let from = parameter.get("value").and_then(Value::as_f64)?;
                let to = selected.get("value").and_then(Value::as_f64)?;
                Some(json!({
                    "parameter_id": parameter_id,
                    "part_id": part_id,
                    "from": stable_visual_metric(from),
                    "to": stable_visual_metric(to),
                    "delta": stable_visual_metric(to - from)
                }))
            }).collect()
        })
        .unwrap_or_default()
}

fn compact_rig_parameter_values(rig: &Value, selected_parameters: &[Value]) -> Vec<Value> {
    rig.get("parameters")
        .and_then(Value::as_array)
        .map(|parameters| {
            parameters
                .iter()
                .enumerate()
                .filter_map(|(index, parameter)| {
                    let selected = selected_parameters.get(index)?;
                    let parameter_id = parameter.get("parameter_id").and_then(Value::as_str)?;
                    let part_id = parameter.get("part_id").and_then(Value::as_str)?;
                    let value = selected.get("value").and_then(Value::as_f64)?;
                    Some(json!({
                        "parameter_id": parameter_id,
                        "part_id": part_id,
                        "value": stable_visual_metric(value)
                    }))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Rank primary-form controls by the size of the evidence-attributed proposal.
/// This keeps the bounded Runtime search focused on the Parts that are furthest
/// from the current visible target, while preserving a stable parameter-id tie
/// break.  Codex does not receive or steer this ordering.
fn ranked_rig_parameter_indices(rig: &Value, selected_parameters: &[Value]) -> Vec<usize> {
    let Some(parameters) = rig.get("parameters").and_then(Value::as_array) else {
        return Vec::new();
    };
    let mut indices: Vec<usize> = (0..parameters.len()).collect();
    indices.sort_by(|left, right| {
        let delta = |index: usize| {
            let from = parameters
                .get(index)
                .and_then(|parameter| parameter.get("value"))
                .and_then(Value::as_f64)
                .unwrap_or(0.0);
            let to = selected_parameters
                .get(index)
                .and_then(|parameter| parameter.get("value"))
                .and_then(Value::as_f64)
                .unwrap_or(from);
            (to - from).abs()
        };
        delta(*right)
            .partial_cmp(&delta(*left))
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                let id = |index: usize| {
                    parameters
                        .get(index)
                        .and_then(|parameter| parameter.get("parameter_id"))
                        .and_then(Value::as_str)
                        .unwrap_or("")
                };
                id(*left).cmp(id(*right))
            })
    });
    indices
}

/// Rank the bounded Primary Form coordinates by the candidate-bound boundary
/// evidence first, then by the size of the Runtime-owned proposal.  The
/// ordinary ranking is still useful for callers without Part-ID evidence, but
/// the repair path has already rendered a Part-ID pass and must spend its
/// small first coordinate pass on the Parts producing the largest visible
/// error.  This is a priority projection only: it does not choose values,
/// widen bounds, or expose a continuous search trace to Codex.
fn ranked_rig_parameter_indices_with_boundary_context(
    rig: &Value,
    selected_parameters: &[Value],
    segments: &[Value],
) -> Vec<usize> {
    let mut part_scores: HashMap<String, f64> = HashMap::new();
    for segment in segments {
        let Some(part_id) = segment.get("part_id").and_then(Value::as_str) else {
            continue;
        };
        let distance = segment
            .get("distance_px")
            .and_then(Value::as_f64)
            .unwrap_or(0.0);
        if distance <= 4.0 || !distance.is_finite() {
            continue;
        }
        // The squared distance makes a few dominant, clearly separated
        // contour errors win over many near-aligned samples from another
        // Part, while the fixed segment cap keeps this aggregation bounded.
        *part_scores.entry(part_id.to_owned()).or_default() += distance * distance;
    }
    if part_scores.is_empty() {
        return ranked_rig_parameter_indices(rig, selected_parameters);
    }
    let Some(parameters) = rig.get("parameters").and_then(Value::as_array) else {
        return Vec::new();
    };
    let mut indices: Vec<usize> = (0..parameters.len()).collect();
    indices.sort_by(|left, right| {
        let part_score = |index: usize| {
            parameters
                .get(index)
                .and_then(|parameter| parameter.get("part_id"))
                .and_then(Value::as_str)
                .and_then(|part_id| part_scores.get(part_id))
                .copied()
                .unwrap_or(0.0)
        };
        part_score(*right)
            .partial_cmp(&part_score(*left))
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                let delta = |index: usize| {
                    let from = parameters
                        .get(index)
                        .and_then(|parameter| parameter.get("value"))
                        .and_then(Value::as_f64)
                        .unwrap_or(0.0);
                    let to = selected_parameters
                        .get(index)
                        .and_then(|parameter| parameter.get("value"))
                        .and_then(Value::as_f64)
                        .unwrap_or(from);
                    (to - from).abs()
                };
                delta(*right)
                    .partial_cmp(&delta(*left))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .then_with(|| {
                let id = |index: usize| {
                    parameters
                        .get(index)
                        .and_then(|parameter| parameter.get("parameter_id"))
                        .and_then(Value::as_str)
                        .unwrap_or("")
                };
                id(*left).cmp(id(*right))
            })
    });
    indices
}

/// Select the single Rig Part that owns the largest candidate-bound visible
/// boundary error.  The score is intentionally the same squared-distance
/// aggregation used by the coordinate ranking so the action scope and probe
/// order cannot disagree.  A `*-pair` Rig Part is allowed to own its fixed
/// left/right output aliases; no fuzzy or user-defined name matching is used.
fn dominant_boundary_rig_part(rig: &Value, segments: &[Value]) -> Option<String> {
    let parameters = rig.get("parameters").and_then(Value::as_array)?;
    let mut rig_parts = HashSet::new();
    for parameter in parameters {
        if let Some(part_id) = parameter.get("part_id").and_then(Value::as_str) {
            rig_parts.insert(part_id.to_owned());
        }
    }
    let mut scores = HashMap::<String, f64>::new();
    for rig_part_id in rig_parts {
        let score = segments
            .iter()
            .filter_map(|segment| {
                let observed_part_id = segment.get("part_id").and_then(Value::as_str)?;
                if !rig_part_matches_observed_part(rig_part_id.as_str(), observed_part_id) {
                    return None;
                }
                let distance = segment.get("distance_px").and_then(Value::as_f64)?;
                if distance <= 4.0 || !distance.is_finite() {
                    return None;
                }
                Some(distance * distance)
            })
            .sum::<f64>();
        if score > 0.0 {
            scores.insert(rig_part_id, score);
        }
    }
    let mut ranked = scores.into_iter().collect::<Vec<_>>();
    ranked.sort_by(|left, right| {
        right
            .1
            .partial_cmp(&left.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left.0.cmp(&right.0))
    });
    ranked.into_iter().next().map(|(part_id, _)| part_id)
}

/// Keep the Runtime-owned proposal shape intact while restoring every
/// non-focused Part to its authored value.  The GeometryProgram materializer
/// expects parameter order to remain stable, so this returns a full ordered
/// compact proposal rather than a sparse patch map.
fn focus_rig_parameters_to_part(
    rig: &Value,
    selected_parameters: &[Value],
    focused_part_id: &str,
) -> Vec<Value> {
    rig.get("parameters")
        .and_then(Value::as_array)
        .map(|parameters| {
            parameters
                .iter()
                .enumerate()
                .map(|(index, parameter)| {
                    let parameter_id = parameter
                        .get("parameter_id")
                        .and_then(Value::as_str)
                        .unwrap_or("unknown");
                    let part_id = parameter
                        .get("part_id")
                        .and_then(Value::as_str)
                        .unwrap_or("unknown");
                    let authored_value = parameter
                        .get("value")
                        .and_then(Value::as_f64)
                        .unwrap_or(0.0);
                    let value = if rig_part_matches_observed_part(part_id, focused_part_id) {
                        selected_parameters
                            .get(index)
                            .and_then(|row| row.get("value"))
                            .and_then(Value::as_f64)
                            .unwrap_or(authored_value)
                    } else {
                        authored_value
                    };
                    json!({
                        "parameter_id":parameter_id,
                        "part_id":part_id,
                        "value":stable_visual_metric(value)
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn rig_part_matches_observed_part(rig_part_id: &str, observed_part_id: &str) -> bool {
    rig_part_id == observed_part_id
        || rig_part_matches_output(rig_part_id, observed_part_id)
        || rig_part_matches_output(observed_part_id, rig_part_id)
}

/// Return the coordinate used by a bounded Primary Form probe.
///
/// Probe zero is the complete evidence-attributed proposal.  Subsequent
/// probes make one deterministic pass over every ranked Rig coordinate before
/// a second pass tests the opposite direction.  Keeping this schedule as a
/// small pure function makes the coverage guarantee testable without starting
/// a Geometry/Render Worker.
fn primary_form_probe_coordinate(parameter_indices: &[usize], probe_index: usize) -> Option<usize> {
    if probe_index == 0 || parameter_indices.is_empty() {
        return None;
    }
    parameter_indices.get((probe_index - 1) % parameter_indices.len()).copied()
}

/// Backtrack one evidence-attributed joint proposal toward the authored Rig
/// baseline without widening any typed bound. This is used only inside the
/// Runtime-owned Primary Form search: it gives coupled width/height/offset
/// controls a deterministic chance to recover from an over-large projection
/// before the search falls back to independent coordinate probes.
fn interpolate_rig_parameter_values(
    definitions: &[Value],
    selected_parameters: &[Value],
    fraction: f64,
) -> Vec<Value> {
    let fraction = fraction.clamp(0.0, 1.0);
    definitions
        .iter()
        .enumerate()
        .map(|(index, definition)| {
            let authored = definition
                .get("value")
                .and_then(Value::as_f64)
                .unwrap_or(0.0);
            let proposed = selected_parameters
                .get(index)
                .and_then(|row| row.get("value"))
                .and_then(Value::as_f64)
                .unwrap_or(authored);
            let min = definition
                .get("min")
                .and_then(Value::as_f64)
                .unwrap_or(authored);
            let max = definition
                .get("max")
                .and_then(Value::as_f64)
                .unwrap_or(authored);
            let mut value = definition.clone();
            value["value"] = Value::from(stable_visual_metric(
                (authored + (proposed - authored) * fraction).clamp(min, max),
            ));
            value
        })
        .collect()
}

fn primary_form_evaluation_budgets(
    max_evaluations: usize,
    has_geometry_program: bool,
) -> (usize, usize, usize) {
    if !has_geometry_program {
        return (0, max_evaluations.clamp(1, 64), 0);
    }
    if max_evaluations < 3 {
        return (0, max_evaluations.clamp(1, 64), 0);
    }
    // Reserve a first-class budget for the coupled geometry -> camera
    // convergence pass.  A geometry-only 2/3 split made the final camera
    // implicit and could spend every evaluation before the winner was
    // re-framed.  The three budgets always sum to the caller's hard cap.
    let geometry_budget = (max_evaluations / 2).clamp(1, 40);
    let camera_budget = (max_evaluations / 4).clamp(1, 24);
    let camera_refit_budget = max_evaluations
        .saturating_sub(geometry_budget)
        .saturating_sub(camera_budget)
        .clamp(1, 24);
    (geometry_budget, camera_budget, camera_refit_budget)
}

fn primary_form_camera_refit_schedule(base: &Value, budget: usize) -> Vec<Value> {
    if budget == 0 {
        return Vec::new();
    }
    let mut cameras = Vec::with_capacity(budget);
    cameras.push(base.clone());
    for candidate in camera_fit_refinement_variants(base)
        .into_iter()
        .chain(camera_fit_search_variants(base).into_iter())
    {
        if cameras.len() >= budget {
            break;
        }
        if !cameras.iter().any(|existing| existing == &candidate) {
            cameras.push(candidate);
        }
    }
    cameras
}

fn bbox_axis_ratios(target: &[bool], model: &[bool]) -> (f64, f64) {
    let Some(target_box) = bbox(target) else { return (1.0, 1.0); };
    let Some(model_box) = bbox(model) else { return (1.0, 1.0); };
    let target_width = (target_box.2 - target_box.0 + 1) as f64;
    let target_height = (target_box.3 - target_box.1 + 1) as f64;
    let model_width = (model_box.2 - model_box.0 + 1) as f64;
    let model_height = (model_box.3 - model_box.1 + 1) as f64;
    (
        (target_width / model_width.max(1.0)).clamp(0.5, 1.5),
        (target_height / model_height.max(1.0)).clamp(0.5, 1.5),
    )
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct MaskEnvelope {
    min_x: usize,
    min_y: usize,
    max_x: usize,
    max_y: usize,
    centroid_x: f64,
    centroid_y: f64,
}

fn mask_envelope_value(envelope: MaskEnvelope) -> Value {
    json!([
        stable_visual_metric(envelope.min_x as f64 / 511.0),
        stable_visual_metric(envelope.min_y as f64 / 511.0),
        stable_visual_metric(envelope.max_x as f64 / 511.0),
        stable_visual_metric(envelope.max_y as f64 / 511.0)
    ])
}

fn mask_envelope(mask: &[bool]) -> Option<MaskEnvelope> {
    let mut min_x = 512usize;
    let mut min_y = 512usize;
    let mut max_x = 0usize;
    let mut max_y = 0usize;
    let mut count = 0.0;
    let mut centroid_x = 0.0;
    let mut centroid_y = 0.0;
    for y in 0..512usize {
        for x in 0..512usize {
            if !mask.get(y * 512 + x).copied().unwrap_or(false) {
                continue;
            }
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x);
            max_y = max_y.max(y);
            centroid_x += x as f64;
            centroid_y += y as f64;
            count += 1.0;
        }
    }
    (count > 0.0).then_some(MaskEnvelope {
        min_x,
        min_y,
        max_x,
        max_y,
        centroid_x: centroid_x / count / 511.0,
        centroid_y: centroid_y / count / 511.0,
    })
}

fn local_part_parameter_delta(parameter: &Value, target: MaskEnvelope, model: MaskEnvelope) -> f64 {
    let semantic = parameter.get("semantic").and_then(Value::as_str).unwrap_or("");
    let unit = parameter.get("unit").and_then(Value::as_str).unwrap_or("meter");
    let value = parameter.get("value").and_then(Value::as_f64).unwrap_or(0.0);
    let step = parameter.get("step").and_then(Value::as_f64).unwrap_or(0.01);
    let parameter_scale = value.abs().max(step.abs()).max(0.001);
    let target_width = (target.max_x.saturating_sub(target.min_x) + 1) as f64;
    let target_height = (target.max_y.saturating_sub(target.min_y) + 1) as f64;
    let model_width = (model.max_x.saturating_sub(model.min_x) + 1) as f64;
    let model_height = (model.max_y.saturating_sub(model.min_y) + 1) as f64;
    let width_ratio = (target_width / model_width.max(1.0)).clamp(0.5, 1.5);
    let height_ratio = (target_height / model_height.max(1.0)).clamp(0.5, 1.5);
    let center_delta_x = (target.centroid_x - model.centroid_x).clamp(-0.5, 0.5);
    let center_delta_y = (target.centroid_y - model.centroid_y).clamp(-0.5, 0.5);
    let raw_delta = match semantic {
        "width" => (width_ratio - 1.0) * parameter_scale,
        "height" => (height_ratio - 1.0) * parameter_scale,
        "scale" => (((width_ratio * height_ratio).sqrt()) - 1.0) * parameter_scale,
        "offset_x" => center_delta_x * if unit == "ratio" { 1.0 } else { parameter_scale },
        "offset_y" => center_delta_y * if unit == "ratio" { 1.0 } else { parameter_scale },
        // Depth and Z offsets cannot be inferred from one view.  Returning a
        // neutral bounded proposal is safer than inventing hidden geometry.
        "depth" | "offset_z" => 0.0,
        _ => 0.0,
    };
    raw_delta.clamp(-0.25, 0.25)
}

fn decode_part_mask(part_png: &[u8], part_id: &str, part_ids: &[String]) -> Option<Vec<bool>> {
    let image = image::load_from_memory(part_png)
        .ok()?
        .resize_exact(512, 512, imageops::FilterType::Nearest)
        .to_rgba8();
    if part_ids.is_empty() {
        return None;
    }
    let mut part_mask = vec![false; 512 * 512];
    for (index, value) in part_mask.iter_mut().enumerate() {
        let pixel = image.get_pixel((index % 512) as u32, (index / 512) as u32).0;
        let Some(part_index) = part_color_index(pixel) else { continue; };
        *value = part_ids.get(part_index).is_some_and(|candidate| candidate == part_id);
    }
    part_mask.iter().any(|value| *value).then_some(part_mask)
}

/// Decode the Part-ID image once for all landmark-relevant Parts.  The old
/// path resized and scanned the complete 512x512 image once per Part, which
/// made a fit score quadratic in the number of semantic Parts.  This helper
/// keeps the same palette/readback semantics while doing one image pass.
fn decode_relevant_part_masks(
    part_png: &[u8],
    part_ids: &[String],
    wanted_part_ids: &HashSet<String>,
    resolution: usize,
) -> Option<HashMap<String, Vec<bool>>> {
    if part_ids.is_empty() || wanted_part_ids.is_empty() || resolution == 0 {
        return Some(HashMap::new());
    }
    let image = image::load_from_memory(part_png)
        .ok()?
        .resize_exact(
            resolution as u32,
            resolution as u32,
            imageops::FilterType::Nearest,
        )
        .to_rgba8();
    let wanted_indices = part_ids
        .iter()
        .enumerate()
        .filter_map(|(index, part_id)| {
            wanted_part_ids
                .contains(part_id)
                .then_some((index, part_id.clone()))
        })
        .collect::<HashMap<usize, String>>();
    if wanted_indices.is_empty() {
        return Some(HashMap::new());
    }
    let mut masks = wanted_indices
        .values()
        .map(|part_id| (part_id.clone(), vec![false; resolution * resolution]))
        .collect::<HashMap<_, _>>();
    for index in 0..resolution * resolution {
        let pixel = image
            .get_pixel((index % resolution) as u32, (index / resolution) as u32)
            .0;
        let Some(part_index) = part_color_index(pixel) else {
            continue;
        };
        let Some(part_id) = wanted_indices.get(&part_index) else {
            continue;
        };
        if let Some(mask) = masks.get_mut(part_id) {
            mask[index] = true;
        }
    }
    Some(
        masks
            .into_iter()
            .filter(|(_, mask)| mask.iter().any(|value| *value))
            .collect(),
    )
}

fn part_boundary_error(part_png: &[u8], target_mask: &[bool], target: &Value, part_id: &str, part_ids: &[String]) -> f64 {
    let Some(part_mask) = decode_part_mask(part_png, part_id, part_ids) else { return 512.0; };
    let selected_boundary = boundary_mask(&part_mask);
    if !selected_boundary.iter().any(|value| *value) { return 512.0; }
    let target_boundary = target_part_boundary_mask(target, part_id)
        .unwrap_or_else(|| boundary_mask(target_mask));
    let target_field = boundary_distance_field(&target_boundary);
    let mut total = 0.0;
    let mut count = 0.0;
    for (index, value) in selected_boundary.iter().enumerate() {
        if *value {
            total += target_field[index].min(512.0);
            count += 1.0;
        }
    }
    if count == 0.0 { 512.0 } else { (total / count).clamp(0.0, 512.0) }
}

/// Validate the contour slices used to attribute a target boundary to one
/// semantic Part.  A slice is an inclusive, non-wrapping range over the
/// canonical contour point array; ranges must be unique and non-overlapping
/// so a boundary pixel cannot silently belong to two Parts.
fn validate_target_part_ranges(parts: &Value, contour_len: usize, context: &str) -> Result<(), RuntimeError> {
    let values = parts.as_array().ok_or_else(|| {
        RuntimeError::InvalidInput(format!("{context}: target parts must be an array"))
    })?;
    if values.len() > 64 {
        return Err(RuntimeError::InvalidInput(format!("{context}: too many target parts")));
    }
    let mut ids = std::collections::HashSet::new();
    let mut ranges: Vec<(usize, usize)> = Vec::with_capacity(values.len());
    for part in values {
        let object = exact_object(
            part,
            &["part_id", "start_index", "end_index", "visibility"],
            "SilhouetteTarget@1.part",
        )?;
        let part_id = required_contract_identifier(object, "part_id", "SilhouetteTarget@1.part")?;
        if !ids.insert(part_id.to_owned()) {
            return Err(RuntimeError::InvalidInput(format!("{context}: duplicate target part_id")));
        }
        let start = object
            .get("start_index")
            .and_then(Value::as_u64)
            .ok_or_else(|| RuntimeError::InvalidInput(format!("{context}: target part start_index")))?
            as usize;
        let end = object
            .get("end_index")
            .and_then(Value::as_u64)
            .ok_or_else(|| RuntimeError::InvalidInput(format!("{context}: target part end_index")))?
            as usize;
        if contour_len < 2 || start >= end || end >= contour_len {
            return Err(RuntimeError::InvalidInput(format!(
                "{context}: target part range is outside contour"
            )));
        }
        if !matches!(
            object.get("visibility").and_then(Value::as_str),
            Some("observed" | "inferred" | "unknown")
        ) {
            return Err(RuntimeError::InvalidInput(format!("{context}: target part visibility")));
        }
        if ranges
            .iter()
            .any(|(other_start, other_end)| start <= *other_end && *other_start <= end)
        {
            return Err(RuntimeError::InvalidInput(format!(
                "{context}: target part ranges overlap"
            )));
        }
        ranges.push((start, end));
    }
    Ok(())
}

/// Convert one visible target Part's contour slice into a thin 512px boundary
/// mask.  This keeps local Part proposals tied to the user-declared contour
/// segment instead of comparing a chest/shoulder Part against the full-body
/// silhouette.  Unknown slices deliberately fall back to the full target in
/// the caller rather than inventing a hidden contour.
fn target_part_boundary_mask(target: &Value, part_id: &str) -> Option<Vec<bool>> {
    let contour = target.get("contour_points")?.as_array()?;
    let part = target
        .get("parts")?
        .as_array()?
        .iter()
        .find(|value| value.get("part_id").and_then(Value::as_str) == Some(part_id))?;
    if part.get("visibility").and_then(Value::as_str) == Some("unknown") {
        return None;
    }
    let start = part.get("start_index")?.as_u64()? as usize;
    let end = part.get("end_index")?.as_u64()? as usize;
    if start >= end || end >= contour.len() {
        return None;
    }
    let mut points = Vec::with_capacity(end - start + 1);
    for value in &contour[start..=end] {
        let point = value.as_array()?;
        points.push([point.first()?.as_f64()?, point.get(1)?.as_f64()?]);
    }
    let mut mask = vec![false; 512 * 512];
    for pair in points.windows(2) {
        rasterize_boundary_segment(&mut mask, pair[0], pair[1]);
    }
    // A target made from one user-drawn Part is a closed polygon.  Multi-Part
    // annotations remain open contour chains so adjacent semantic regions do
    // not gain an invented closing edge.
    if start == 0 && end + 1 == contour.len() {
        if let (Some(first), Some(last)) = (points.first().copied(), points.last().copied()) {
            rasterize_boundary_segment(&mut mask, last, first);
        }
    }
    mask.iter().any(|value| *value).then_some(mask)
}

fn rasterize_boundary_segment(mask: &mut [bool], start: [f64; 2], end: [f64; 2]) {
    let x0 = (start[0].clamp(0.0, 1.0) * 511.0).round() as i32;
    let y0 = (start[1].clamp(0.0, 1.0) * 511.0).round() as i32;
    let x1 = (end[0].clamp(0.0, 1.0) * 511.0).round() as i32;
    let y1 = (end[1].clamp(0.0, 1.0) * 511.0).round() as i32;
    let steps = (x1 - x0).abs().max((y1 - y0).abs()).max(1);
    for step in 0..=steps {
        let t = step as f64 / steps as f64;
        let x = (x0 as f64 + (x1 - x0) as f64 * t)
            .round()
            .clamp(0.0, 511.0) as usize;
        let y = (y0 as f64 + (y1 - y0) as f64 * t)
            .round()
            .clamp(0.0, 511.0) as usize;
        mask[y * 512 + x] = true;
    }
}

fn camera_fit_metrics_at_resolution(
    reference: &[bool],
    model: &[bool],
    resolution: usize,
) -> Value {
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
    let reference_bbox = bbox_at_resolution(reference, resolution);
    let model_bbox = bbox_at_resolution(model, resolution);
    let iou = if union == 0 {
        0.0
    } else {
        intersection as f64 / union as f64
    };
    let boundary_f1 = boundary_f1_at_resolution(reference, model, resolution, 1);
    json!({
        "silhouette_iou": stable_visual_metric(iou),
        "boundary_f1_4px": stable_visual_metric(boundary_f1),
        "bbox_edge_error": stable_visual_metric(bbox_edge_error_at_resolution(reference_bbox, model_bbox, resolution)),
        "centroid_error": stable_visual_metric(centroid_error_at_resolution(reference, model, resolution))
    })
}

fn transient_loss_metrics_at_resolution(
    base: &Value,
    model: &[bool],
    resolution: usize,
    landmarks: Option<&Value>,
    part_context: Option<(&[u8], &[String])>,
) -> Value {
    let Some(values) = landmarks.and_then(Value::as_array) else {
        return base.clone();
    };
    if values.is_empty() {
        return base.clone();
    }
    let (covered, nme) = landmark_metrics_at_resolution_with_parts(
        model,
        resolution,
        &json!({"landmarks": values}),
        part_context,
    );
    if values.iter().all(|value| value.get("visibility").and_then(Value::as_str) == Some("unknown")) {
        return base.clone();
    }
    let mut metrics = base.clone();
    if let Some(object) = metrics.as_object_mut() {
        object.insert(
            "landmark_coverage".to_owned(),
            Value::from(stable_visual_metric(covered)),
        );
        object.insert(
            "landmark_nme".to_owned(),
            Value::from(stable_visual_metric(nme)),
        );
    }
    metrics
}

fn camera_fit_loss(metrics: &Value) -> f64 {
    weighted_contour_loss(metrics)
}

fn part_color_index(pixel: [u8; 4]) -> Option<usize> {
    // Part-ID colors are generated from a fixed 256-entry palette.  The
    // previous decoder searched that palette linearly for every pixel, which
    // made a single fit score O(pixels * 256) and dominated silhouette-fit
    // requests.  Keep the palette definition in one place, but reverse it
    // once so every readback pixel is an O(1) lookup.  `OnceLock` keeps this
    // deterministic and avoids a per-call allocation while remaining safe
    // for the Runtime's test and IPC threads.
    static PALETTE_INDEX: OnceLock<HashMap<u32, usize>> = OnceLock::new();
    let lookup = PALETTE_INDEX.get_or_init(|| {
        let mut table = HashMap::with_capacity(256);
        for index in 0usize..256usize {
            let rgba = [
                (((index.wrapping_mul(97) + 53) % 220 + 20) as u8),
                (((index.wrapping_mul(53) + 79) % 170 + 40) as u8),
                (((index.wrapping_mul(31) + 131) % 120 + 80) as u8),
                255_u8,
            ];
            let key = u32::from_be_bytes(rgba);
            table.insert(key, index);
        }
        table
    });
    lookup.get(&u32::from_be_bytes(pixel)).copied()
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

fn decode_binary_mask_at_resolution(bytes: &[u8], resolution: usize) -> Result<Vec<bool>, RuntimeError> {
    if !(16..=512).contains(&resolution) {
        return Err(RuntimeError::InvalidInput(
            "RENDER_PASS_INVALID: fit resolution out of bounds".to_owned(),
        ));
    }
    let image = image::load_from_memory(bytes)
        .map_err(|error| RuntimeError::InvalidInput(format!("RENDER_PASS_INVALID: {error}")))?
        .resize_exact(resolution as u32, resolution as u32, imageops::FilterType::Nearest)
        .to_rgba8();
    Ok(image
        .pixels()
        .map(|pixel| {
            let [r, g, b, _] = pixel.0;
            (r as u16 + g as u16 + b as u16) > 96
        })
        .collect())
}

fn downsample_mask(mask: &[bool], source_resolution: usize, resolution: usize) -> Vec<bool> {
    (0..resolution * resolution)
        .map(|index| {
            let x = index % resolution;
            let y = index / resolution;
            let sx = ((x * source_resolution + source_resolution / (resolution * 2)) / resolution)
                .min(source_resolution.saturating_sub(1));
            let sy = ((y * source_resolution + source_resolution / (resolution * 2)) / resolution)
                .min(source_resolution.saturating_sub(1));
            mask[sy * source_resolution + sx]
        })
        .collect()
}

fn bbox_at_resolution(mask: &[bool], resolution: usize) -> Option<(usize, usize, usize, usize)> {
    let mut min_x = resolution;
    let mut min_y = resolution;
    let mut max_x = 0;
    let mut max_y = 0;
    let mut any = false;
    for y in 0..resolution {
        for x in 0..resolution {
            if mask[y * resolution + x] {
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

fn bbox_edge_error_at_resolution(
    left: Option<(usize, usize, usize, usize)>,
    right: Option<(usize, usize, usize, usize)>,
    resolution: usize,
) -> f64 {
    match (left, right) {
        (Some(a), Some(b)) => [
            a.0 as f64 / resolution as f64 - b.0 as f64 / resolution as f64,
            a.1 as f64 / resolution as f64 - b.1 as f64 / resolution as f64,
            a.2 as f64 / resolution as f64 - b.2 as f64 / resolution as f64,
            a.3 as f64 / resolution as f64 - b.3 as f64 / resolution as f64,
        ]
        .iter()
        .map(|value| value.abs())
        .fold(0.0, f64::max),
        _ => 1.0,
    }
}

fn centroid_error_at_resolution(reference: &[bool], model: &[bool], resolution: usize) -> f64 {
    fn center(mask: &[bool], resolution: usize) -> Option<(f64, f64)> {
        let mut count = 0.0;
        let mut x = 0.0;
        let mut y = 0.0;
        for py in 0..resolution {
            for px in 0..resolution {
                if mask[py * resolution + px] {
                    count += 1.0;
                    x += px as f64 / resolution as f64;
                    y += py as f64 / resolution as f64;
                }
            }
        }
        (count > 0.0).then_some((x / count, y / count))
    }
    match (center(reference, resolution), center(model, resolution)) {
        (Some(left), Some(right)) =>
            ((left.0 - right.0).powi(2) + (left.1 - right.1).powi(2)).sqrt().min(1.0),
        _ => 1.0,
    }
}

fn boundary_mask_at_resolution(mask: &[bool], resolution: usize) -> Vec<bool> {
    let mut output = vec![false; mask.len()];
    for y in 0..resolution {
        for x in 0..resolution {
            if !mask[y * resolution + x] {
                continue;
            }
            for (ny, nx) in [
                (y.wrapping_sub(1), x),
                (y + 1, x),
                (y, x.wrapping_sub(1)),
                (y, x + 1),
            ] {
                if ny >= resolution || nx >= resolution || !mask[ny * resolution + nx] {
                    output[y * resolution + x] = true;
                    break;
                }
            }
        }
    }
    output
}

fn boundary_f1_at_resolution(
    reference: &[bool],
    model: &[bool],
    resolution: usize,
    radius: i32,
) -> f64 {
    let left = boundary_mask_at_resolution(reference, resolution);
    let right = boundary_mask_at_resolution(model, resolution);
    fn score(left: &[bool], right: &[bool], resolution: usize, radius: i32) -> f64 {
        let mut total = 0usize;
        let mut hit = 0usize;
        for y in 0..resolution {
            for x in 0..resolution {
                if !left[y * resolution + x] {
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
                            && nx < resolution as i32
                            && ny < resolution as i32
                            && right[ny as usize * resolution + nx as usize]
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
        if total == 0 { 0.0 } else { hit as f64 / total as f64 }
    }
    let precision = score(&left, &right, resolution, radius);
    let recall = score(&right, &left, resolution, radius);
    if precision + recall == 0.0 {
        0.0
    } else {
        2.0 * precision * recall / (precision + recall)
    }
}

fn sdf_chamfer_px_at_resolution(reference: &[bool], model: &[bool], resolution: usize) -> f64 {
    let left = boundary_mask_at_resolution(reference, resolution);
    let right = boundary_mask_at_resolution(model, resolution);
    let left_field = boundary_distance_field_at_resolution(&left, resolution);
    let right_field = boundary_distance_field_at_resolution(&right, resolution);
    let mut forward = 0.0;
    let mut backward = 0.0;
    let mut forward_count = 0.0;
    let mut backward_count = 0.0;
    for index in 0..left.len() {
        if left[index] {
            forward += right_field[index];
            forward_count += 1.0;
        }
        if right[index] {
            backward += left_field[index];
            backward_count += 1.0;
        }
    }
    if forward_count == 0.0 || backward_count == 0.0 {
        resolution as f64
    } else {
        ((forward / forward_count) + (backward / backward_count)) * 0.5
    }
}

fn boundary_distance_field_at_resolution(boundary: &[bool], resolution: usize) -> Vec<f64> {
    const INF: f64 = 1.0e9;
    let mut field = boundary
        .iter()
        .map(|value| if *value { 0.0 } else { INF })
        .collect::<Vec<_>>();
    for y in 0..resolution {
        for x in 0..resolution {
            let index = y * resolution + x;
            let mut value = field[index];
            for (dx, dy, weight) in [(-1_i32, 0_i32, 1.0), (0, -1, 1.0), (-1, -1, 1.4142), (1, -1, 1.4142)] {
                let nx = x as i32 + dx;
                let ny = y as i32 + dy;
                if nx >= 0 && ny >= 0 && nx < resolution as i32 && ny < resolution as i32 {
                    value = value.min(field[ny as usize * resolution + nx as usize] + weight);
                }
            }
            field[index] = value;
        }
    }
    for y in (0..resolution).rev() {
        for x in (0..resolution).rev() {
            let index = y * resolution + x;
            let mut value = field[index];
            for (dx, dy, weight) in [(1_i32, 0_i32, 1.0), (0, 1, 1.0), (1, 1, 1.4142), (-1, 1, 1.4142)] {
                let nx = x as i32 + dx;
                let ny = y as i32 + dy;
                if nx >= 0 && ny >= 0 && nx < resolution as i32 && ny < resolution as i32 {
                    value = value.min(field[ny as usize * resolution + nx as usize] + weight);
                }
            }
            field[index] = value;
        }
    }
    field
}

fn landmark_metrics_at_resolution_with_parts(
    model: &[bool],
    resolution: usize,
    view_spec: &Value,
    part_context: Option<(&[u8], &[String])>,
) -> (f64, f64) {
    let Some(values) = view_spec.get("landmarks").and_then(Value::as_array) else {
        return (0.0, 1.0);
    };
    let wanted_part_ids = values
        .iter()
        .filter_map(|value| value.get("landmark_id").and_then(Value::as_str))
        .filter_map(landmark_part_hint)
        .map(|(part_id, _)| part_id.to_owned())
        .collect::<HashSet<_>>();
    let part_masks = part_context
        .and_then(|(part_png, part_ids)| {
            decode_relevant_part_masks(part_png, part_ids, &wanted_part_ids, resolution)
        })
        .unwrap_or_default();
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
        let confidence = value.get("confidence").and_then(Value::as_f64).unwrap_or(1.0).clamp(0.25, 1.0);
        total += confidence;
        let px = ((x * (resolution.saturating_sub(1)) as f64).round() as usize).min(resolution.saturating_sub(1));
        let py = ((y * (resolution.saturating_sub(1)) as f64).round() as usize).min(resolution.saturating_sub(1));
        let anchor = value
            .get("landmark_id")
            .and_then(Value::as_str)
            .and_then(landmark_part_hint)
            .and_then(|(part_id, anchor)| part_masks.get(part_id).and_then(|mask| landmark_anchor_point_at_resolution(mask, anchor, resolution)));
        if let Some((anchor_x, anchor_y)) = anchor {
            let distance = ((anchor_x - x).powi(2) + (anchor_y - y).powi(2)).sqrt();
            if distance <= 12.0 / 512.0 { covered += confidence; }
            error += distance.min(1.0) * confidence;
        } else if model[py * resolution + px] {
            covered += confidence;
        } else {
            let mut best: f64 = 1.0;
            for dy in -3i32..=3 {
                for dx in -3i32..=3 {
                    let nx = px as i32 + dx;
                    let ny = py as i32 + dy;
                    if nx >= 0 && ny >= 0 && nx < resolution as i32 && ny < resolution as i32 && model[ny as usize * resolution + nx as usize] {
                        best = best.min(((dx * dx + dy * dy) as f64).sqrt() / resolution as f64);
                    }
                }
            }
            error += best * confidence;
        }
    }
    if total == 0.0 { (0.0, 1.0) } else { (covered / total, (error / total).min(1.0)) }
}

fn landmark_anchor_point_at_resolution(mask: &[bool], anchor: LandmarkAnchor, resolution: usize) -> Option<(f64, f64)> {
    let pixels = mask.iter().enumerate().filter_map(|(index, value)| value.then_some((index % resolution, index / resolution))).collect::<Vec<_>>();
    if pixels.is_empty() { return None; }
    let min_x = pixels.iter().map(|(x, _)| *x).min()?;
    let max_x = pixels.iter().map(|(x, _)| *x).max()?;
    let split = (min_x + max_x) / 2;
    let selected = pixels.iter().copied().filter(|(x, _)| match anchor {
        LandmarkAnchor::Left | LandmarkAnchor::LowerLeft => *x <= split,
        LandmarkAnchor::Right => *x >= split,
        _ => true,
    }).collect::<Vec<_>>();
    let selected = if selected.is_empty() { pixels } else { selected };
    let extreme = match anchor {
        LandmarkAnchor::Top => selected.iter().map(|(_, y)| *y).min(),
        LandmarkAnchor::Bottom => selected.iter().map(|(_, y)| *y).max(),
        LandmarkAnchor::Left | LandmarkAnchor::LowerLeft => selected.iter().map(|(x, _)| *x).min(),
        LandmarkAnchor::Right => selected.iter().map(|(x, _)| *x).max(),
        LandmarkAnchor::Center => None,
    };
    let chosen = if let Some(extreme) = extreme {
        selected.into_iter().filter(|(x, y)| match anchor {
            LandmarkAnchor::Top | LandmarkAnchor::Bottom => *y == extreme,
            LandmarkAnchor::Left | LandmarkAnchor::LowerLeft | LandmarkAnchor::Right => *x == extreme,
            LandmarkAnchor::Center => true,
        }).collect::<Vec<_>>()
    } else { selected };
    let (sum_x, sum_y) = chosen.iter().fold((0.0, 0.0), |(sx, sy), (x, y)| (sx + *x as f64, sy + *y as f64));
    Some((sum_x / chosen.len() as f64 / resolution.saturating_sub(1) as f64, sum_y / chosen.len() as f64 / resolution.saturating_sub(1) as f64))
}

/// Quantize persisted visual metrics before hashing them. `serde_json::Value`
/// stores renderer measurements as binary `f64`; without a bounded decimal
/// representation, serializing and reading a report can turn values such as
/// `0.24176079827981134` into `0.24176079827981137`, invalidating the report's
/// own canonical hash. Twelve fractional digits are well below the C quality
/// thresholds and make the JSON/CAS round trip deterministic.
fn stable_visual_metric(value: f64) -> f64 {
    if value.is_finite() {
        (value * 1_000_000_000_000.0).round() / 1_000_000_000_000.0
    } else {
        value
    }
}

fn visible_view_gate_passes(metrics: &Value) -> bool {
    let at_least = |key: &str, minimum: f64| {
        metrics
            .get(key)
            .and_then(Value::as_f64)
            .is_some_and(|value| value.is_finite() && value >= minimum)
    };
    let at_most = |key: &str, maximum: f64| {
        metrics
            .get(key)
            .and_then(Value::as_f64)
            .is_some_and(|value| value.is_finite() && value <= maximum)
    };
    at_least("silhouette_iou", VISIBLE_SILHOUETTE_IOU_MIN)
        && at_least("boundary_f1_4px", VISIBLE_BOUNDARY_F1_MIN)
        && at_most("bbox_edge_error", VISIBLE_BBOX_EDGE_ERROR_MAX)
        && at_most("centroid_error", VISIBLE_CENTROID_ERROR_MAX)
        && at_least("landmark_coverage", VISIBLE_LANDMARK_COVERAGE_MIN)
        && at_most("landmark_nme", VISIBLE_LANDMARK_NME_MAX)
        && at_least("region_median_iou", VISIBLE_REGION_MEDIAN_IOU_MIN)
        && at_least("critical_region_min_iou", VISIBLE_CRITICAL_REGION_IOU_MIN)
}

fn compare_masks(reference: &[bool], model: &[bool], view_spec: &Value) -> Value {
    compare_masks_with_parts(reference, model, view_spec, None)
}

/// Compare a reference/model silhouette pair and, when the fixed renderer's
/// Part-ID pass is available, use semantic Part anchors for known image
/// landmarks.  The public ReferenceViewSpec stays intentionally small: the
/// mapping is a product-owned, deterministic robot vocabulary, not a free-form
/// user-supplied Part selector.  Unknown landmark IDs fall back to the global
/// silhouette test so older clients remain compatible.
fn compare_masks_with_parts(
    reference: &[bool],
    model: &[bool],
    view_spec: &Value,
    part_context: Option<(&[u8], &[String])>,
) -> Value {
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
    let (landmark_coverage, landmark_nme) =
        landmark_metrics_with_parts(model, view_spec, part_context);
    let region_scores = region_metrics(reference, model, view_spec);
    let region_median = if region_scores.is_empty() {
        0.0
    } else {
        let mut sorted = region_scores.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        sorted[sorted.len() / 2]
    };
    let critical = region_scores.iter().copied().fold(1.0, f64::min);
    json!({
        "silhouette_iou":stable_visual_metric(silhouette_iou),
        "boundary_f1_4px":stable_visual_metric(boundary_f1),
        "bbox_edge_error":stable_visual_metric(bbox_edge_error),
        "centroid_error":stable_visual_metric(centroid_error),
        "landmark_coverage":stable_visual_metric(landmark_coverage),
        "landmark_nme":stable_visual_metric(landmark_nme),
        "region_median_iou":stable_visual_metric(region_median),
        "critical_region_min_iou":stable_visual_metric(critical)
    })
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
#[derive(Clone, Copy)]
enum LandmarkAnchor {
    Top,
    Bottom,
    Left,
    Right,
    Center,
    LowerLeft,
}

fn landmark_part_hint(landmark_id: &str) -> Option<(&'static str, LandmarkAnchor)> {
    Some(match landmark_id {
        "crown" => ("head-shell", LandmarkAnchor::Top),
        "visor-front-tip" => ("visor", LandmarkAnchor::Left),
        "visor-lower-front" => ("visor", LandmarkAnchor::LowerLeft),
        "neck-base" => ("neck", LandmarkAnchor::Bottom),
        "left-shoulder-outer" => ("shoulder-armor-pair", LandmarkAnchor::Left),
        "right-shoulder-outer" => ("shoulder-armor-pair", LandmarkAnchor::Right),
        "chest-center" => ("chest-shell", LandmarkAnchor::Center),
        "chest-lower" => ("chest-shell", LandmarkAnchor::Bottom),
        "left-elbow" => ("elbow-pair", LandmarkAnchor::Left),
        "right-elbow" => ("elbow-pair", LandmarkAnchor::Right),
        "pelvis-center" => ("pelvis", LandmarkAnchor::Center),
        "left-knee" => ("knee-pair", LandmarkAnchor::Left),
        "right-knee" => ("knee-pair", LandmarkAnchor::Right),
        "left-hand" => ("hand-pair", LandmarkAnchor::Left),
        "right-hand" => ("hand-pair", LandmarkAnchor::Right),
        _ => return None,
    })
}

fn landmark_anchor_point(mask: &[bool], anchor: LandmarkAnchor) -> Option<(f64, f64)> {
    let pixels = mask
        .iter()
        .enumerate()
        .filter_map(|(index, value)| value.then_some((index % 512, index / 512)))
        .collect::<Vec<_>>();
    if pixels.is_empty() {
        return None;
    }
    let (min_x, max_x, min_y, max_y) = pixels.iter().fold(
        (512usize, 0usize, 512usize, 0usize),
        |(min_x, max_x, min_y, max_y), (x, y)| {
            (min_x.min(*x), max_x.max(*x), min_y.min(*y), max_y.max(*y))
        },
    );
    let side = match anchor {
        LandmarkAnchor::Left | LandmarkAnchor::LowerLeft => Some((min_x + max_x) / 2),
        LandmarkAnchor::Right => Some((min_x + max_x) / 2),
        _ => None,
    };
    let filtered = pixels.iter().copied().filter(|(x, _y)| match anchor {
        LandmarkAnchor::Left | LandmarkAnchor::LowerLeft => side.is_none_or(|split| *x <= split),
        LandmarkAnchor::Right => side.is_none_or(|split| *x >= split),
        _ => true,
    });
    let selected = filtered.collect::<Vec<_>>();
    let selected = if selected.is_empty() { pixels } else { selected };
    let (sum_x, sum_y) = selected.iter().fold((0.0, 0.0), |(x, y), (px, py)| {
        (x + *px as f64, y + *py as f64)
    });
    let center = || {
        (
            sum_x / selected.len() as f64 / 511.0,
            sum_y / selected.len() as f64 / 511.0,
        )
    };
    let mean_pixels = |pixels: Vec<(usize, usize)>| -> Option<(f64, f64)> {
        if pixels.is_empty() {
            return None;
        }
        let (sum_x, sum_y) = pixels.iter().fold((0.0, 0.0), |(x, y), (px, py)| {
            (x + *px as f64, y + *py as f64)
        });
        Some((
            sum_x / pixels.len() as f64 / 511.0,
            sum_y / pixels.len() as f64 / 511.0,
        ))
    };
    let (x, y) = match anchor {
        LandmarkAnchor::Top => {
            let extreme = selected.iter().map(|(_, y)| *y).min()?;
            mean_pixels(selected.iter().copied().filter(|(_, y)| *y == extreme).collect())?
        }
        LandmarkAnchor::Bottom => {
            let extreme = selected.iter().map(|(_, y)| *y).max()?;
            mean_pixels(selected.iter().copied().filter(|(_, y)| *y == extreme).collect())?
        }
        LandmarkAnchor::Left => {
            let extreme = selected.iter().map(|(x, _)| *x).min()?;
            mean_pixels(selected.iter().copied().filter(|(x, _)| *x == extreme).collect())?
        }
        LandmarkAnchor::Right => {
            let extreme = selected.iter().map(|(x, _)| *x).max()?;
            mean_pixels(selected.iter().copied().filter(|(x, _)| *x == extreme).collect())?
        }
        LandmarkAnchor::LowerLeft => {
            let bottom_band = max_y.saturating_sub(((max_y - min_y) / 5).max(4));
            let bottom = selected
                .iter()
                .copied()
                .filter(|(_, y)| *y >= bottom_band)
                .collect::<Vec<_>>();
            let bottom = if bottom.is_empty() { selected.clone() } else { bottom };
            let extreme = match anchor {
                LandmarkAnchor::LowerLeft => bottom.iter().map(|(x, _)| *x).min()?,
                _ => unreachable!(),
            };
            mean_pixels(bottom.into_iter().filter(|(x, _)| *x == extreme).collect())?
        }
        LandmarkAnchor::Center => center(),
    };
    let _ = (min_y, max_y);
    Some((x, y))
}

fn landmark_metrics_with_parts(
    model: &[bool],
    view_spec: &Value,
    part_context: Option<(&[u8], &[String])>,
) -> (f64, f64) {
    let Some(values) = view_spec.get("landmarks").and_then(Value::as_array) else {
        return (0.0, 1.0);
    };
    let wanted_part_ids = values
        .iter()
        .filter_map(|value| value.get("landmark_id").and_then(Value::as_str))
        .filter_map(landmark_part_hint)
        .map(|(part_id, _)| part_id.to_owned())
        .collect::<HashSet<_>>();
    let part_masks = part_context
        .and_then(|(part_png, part_ids)| {
            decode_relevant_part_masks(part_png, part_ids, &wanted_part_ids, 512)
        })
        .unwrap_or_default();
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
        let confidence = value
            .get("confidence")
            .and_then(Value::as_f64)
            .unwrap_or(1.0)
            .clamp(0.25, 1.0);
        total += confidence;
        let px = (x * 511.0) as usize;
        let py = (y * 511.0) as usize;
        let hint = value
            .get("landmark_id")
            .and_then(Value::as_str)
            .and_then(landmark_part_hint);
        let anchor = hint.and_then(|(part_id, anchor)| {
            part_masks
                .get(part_id)
                .and_then(|mask| landmark_anchor_point(mask, anchor))
        });
        if let Some((anchor_x, anchor_y)) = anchor {
            let distance = ((anchor_x - x).powi(2) + (anchor_y - y).powi(2)).sqrt();
            if distance <= 12.0 / 512.0 {
                covered += confidence;
            }
            error += distance.min(1.0) * confidence;
        } else if model[py * 512 + px] {
            covered += confidence;
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
            error += best * confidence;
        }
    }
    if total == 0.0 {
        (0.0, 1.0)
    } else {
        (covered / total, (error / total).min(1.0))
    }
}

/// Compare each declared visible region using the reference and model masks
/// restricted to that region. The previous implementation compared the whole
/// model mask against the rectangle itself, which made the score depend on
/// unrelated geometry elsewhere in the frame and understated good local
/// matches. Unknown regions remain excluded from the quality aggregate.
fn region_metrics(reference: &[bool], model: &[bool], view_spec: &Value) -> Vec<f64> {
    let Some(values) = view_spec.get("regions").and_then(Value::as_array) else {
        return Vec::new();
    };
    values
        .iter()
        .filter_map(|value| {
            if value.get("visibility").and_then(Value::as_str) == Some("unknown") {
                return None;
            }
            let x = value.get("x").and_then(Value::as_f64)?;
            let y = value.get("y").and_then(Value::as_f64)?;
            let w = value.get("width").and_then(Value::as_f64)?;
            let h = value.get("height").and_then(Value::as_f64)?;
            let mut inter = 0usize;
            let mut union = 0usize;
            for py in 0..512 {
                for px in 0..512 {
                    let in_region = px as f64 / 512.0 >= x
                        && px as f64 / 512.0 <= x + w
                        && py as f64 / 512.0 >= y
                        && py as f64 / 512.0 <= y + h;
                    if !in_region {
                        continue;
                    }
                    let index = py * 512 + px;
                    let in_reference = reference[index];
                    let in_model = model[index];
                    if in_reference && in_model {
                        inter += 1;
                    }
                    if in_reference || in_model {
                        union += 1;
                    }
                }
            }
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
) -> Result<Vec<render_worker::RenderPass>, geometry_worker::GeometryWorkerError> {
    let artifact = match geometry_worker::compile_geometry(geometry_program, Some(appearance_program)) {
        Ok(artifact) => artifact,
        #[cfg(any(test, feature = "test-geometry-worker-fallback"))]
        Err(geometry_worker::GeometryWorkerError::Unavailable) => {
            geometry_worker::compile_geometry_test_fallback(geometry_program, Some(appearance_program))?
        }
        Err(error) => return Err(error),
    };
    match render_worker::render_fixed_glb(&artifact.glb) {
        Ok(passes) => Ok(passes),
        #[cfg(any(test, feature = "test-geometry-worker-fallback"))]
        Err(geometry_worker::GeometryWorkerError::Unavailable) => {
            forgecad_render_core::render_fixed_glb(&artifact.glb)
                .map(|passes| {
                    passes
                        .into_iter()
                        .map(|pass| render_worker::RenderPass {
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
) -> Result<Vec<render_worker::RenderPass>, geometry_worker::GeometryWorkerError> {
    match render_worker::render_glb(glb, camera) {
        Ok(passes) => Ok(passes),
        #[cfg(any(test, feature = "test-geometry-worker-fallback"))]
        Err(geometry_worker::GeometryWorkerError::Unavailable) => {
            forgecad_render_core::render_perspective_glb(glb, camera)
                .map(|passes| {
                    passes
                        .into_iter()
                        .map(|pass| render_worker::RenderPass {
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

fn render_glb_fit_batch_with_runtime_worker(
    glb: &[u8],
    cameras: &[Value],
) -> Result<Vec<Vec<render_worker::RenderPass>>, geometry_worker::GeometryWorkerError> {
    render_worker::render_glb_fit_batch(glb, cameras)
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
    if canonical_json_hash(&input) != actual
        && canonical_json_hash(&normalize_json_numbers(&input)) != actual
    {
        return Err(RuntimeError::InvalidInput(format!(
            "CONTRACT_OUTPUT_INVALID: {context}.canonical_sha256 does not bind the payload"
        )));
    }
    Ok(())
}

/// JSON clients may round-trip `1.0` as `1` (and `0.0` as `0`) while
/// preserving the same typed numeric value. Keep the ordinary canonical hash
/// as the first choice, then accept this deterministic numeric representation
/// normalization as a compatibility path. Strings, identifiers, and object
/// structure are never coerced.
fn normalize_json_numbers(value: &Value) -> Value {
    match value {
        Value::Null | Value::Bool(_) | Value::String(_) => value.clone(),
        Value::Number(number) => number
            .as_f64()
            .and_then(serde_json::Number::from_f64)
            .map(Value::Number)
            .unwrap_or_else(|| value.clone()),
        Value::Array(values) => Value::Array(values.iter().map(normalize_json_numbers).collect()),
        Value::Object(object) => Value::Object(
            object
                .iter()
                .map(|(key, child)| (key.clone(), normalize_json_numbers(child)))
                .collect(),
        ),
    }
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

fn validate_silhouette_rig_hash_result_output(value: &Value) -> Result<(), RuntimeError> {
    let object = exact_object(
        value,
        &[
            "schema_version",
            "silhouette_rig_schema_version",
            "canonical_sha256",
            "validation_status",
        ],
        "SilhouetteRigHashResult@1",
    )?;
    if object.get("schema_version").and_then(Value::as_str)
        != Some("SilhouetteRigHashResult@1")
        || object
            .get("silhouette_rig_schema_version")
            .and_then(Value::as_str)
            != Some("SilhouetteRig@1")
        || object.get("validation_status").and_then(Value::as_str) != Some("passed")
    {
        return Err(RuntimeError::InvalidInput(
            "CONTRACT_OUTPUT_INVALID: SilhouetteRigHashResult@1 constants drifted".to_owned(),
        ));
    }
    required_contract_sha256(object, "canonical_sha256", "SilhouetteRigHashResult@1")?;
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
        program
            .as_object_mut()
            .expect("V2 program object")
            .remove("canonical_sha256");
        let program_hash = canonical_json_hash(&program);
        program["canonical_sha256"] = Value::String(program_hash);
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
    fn comparison_metrics_keep_a_stable_canonical_hash_after_cas_round_trip() {
        let mut reference = vec![false; 512 * 512];
        let mut model = vec![false; 512 * 512];
        for y in 96..416 {
            for x in 128..320 {
                reference[y * 512 + x] = true;
            }
        }
        for y in 101..411 {
            for x in 136..328 {
                model[y * 512 + x] = true;
            }
        }
        let metrics = compare_masks(&reference, &model, &json!({"landmarks":[],"regions":[]}));
        let bytes = canonical_json_bytes(&metrics).expect("metrics canonical bytes");
        let round_tripped: Value = serde_json::from_slice(&bytes).expect("metrics JSON round trip");
        assert_eq!(
            canonical_json_hash(&metrics),
            canonical_json_hash(&round_tripped)
        );
    }

    #[test]
    fn semantic_landmark_anchors_use_part_id_and_do_not_accept_any_inside_pixel() {
        let mut model = vec![false; 512 * 512];
        let mut part_image = RgbaImage::from_pixel(512, 512, Rgba([0, 0, 0, 0]));
        let part_color = Rgba([73, 119, 91, 255]);
        for y in 100..201 {
            for x in 200..301 {
                model[y * 512 + x] = true;
                part_image.put_pixel(x as u32, y as u32, part_color);
            }
        }
        let mut part_png = Vec::new();
        part_image
            .write_to(&mut Cursor::new(&mut part_png), ImageFormat::Png)
            .expect("part-id png");
        let part_ids = ["head-shell".to_owned()];
        let exact = json!({
            "landmarks":[{"landmark_id":"crown","x":250.0/511.0,"y":100.0/511.0,"visibility":"observed","confidence":1.0}]
        });
        let (coverage, nme) = landmark_metrics_with_parts(
            &model,
            &exact,
            Some((&part_png, &part_ids)),
        );
        assert_eq!(coverage, 1.0);
        assert!(nme < 0.001);

        // The point is still inside the overall silhouette, but it is not the
        // semantic crown anchor. The old global-membership implementation
        // would have incorrectly reported zero error here.
        let wrong = json!({
            "landmarks":[{"landmark_id":"crown","x":220.0/511.0,"y":180.0/511.0,"visibility":"observed","confidence":1.0}]
        });
        let (coverage, nme) = landmark_metrics_with_parts(
            &model,
            &wrong,
            Some((&part_png, &part_ids)),
        );
        assert_eq!(coverage, 0.0);
        assert!(nme > 0.05);
    }

    #[test]
    fn visible_view_gate_rejects_exploratory_thresholds_and_accepts_strict_metrics() {
        let exploratory = json!({
            "silhouette_iou": 0.80,
            "boundary_f1_4px": 0.80,
            "bbox_edge_error": 0.03,
            "centroid_error": 0.03,
            "landmark_coverage": 0.80,
            "landmark_nme": 0.03,
            "region_median_iou": 0.85,
            "critical_region_min_iou": 0.85
        });
        assert!(!visible_view_gate_passes(&exploratory));

        let strict = json!({
            "silhouette_iou": 0.90,
            "boundary_f1_4px": 0.90,
            "bbox_edge_error": 0.02,
            "centroid_error": 0.02,
            "landmark_coverage": 0.80,
            "landmark_nme": 0.03,
            "region_median_iou": 0.85,
            "critical_region_min_iou": 0.85
        });
        assert!(visible_view_gate_passes(&strict));
    }

    #[test]
    fn region_metrics_compare_reference_and_model_inside_each_region() {
        let mut reference = vec![false; 512 * 512];
        let mut model = vec![false; 512 * 512];
        for y in 128..256 {
            for x in 128..256 {
                reference[y * 512 + x] = true;
                model[y * 512 + x] = true;
            }
        }
        // Geometry outside the declared region must not lower a local match.
        for y in 360..420 {
            for x in 360..420 {
                model[y * 512 + x] = true;
            }
        }
        let exact = compare_masks(
            &reference,
            &model,
            &json!({"landmarks":[],"regions":[{"region_id":"core","x":0.25,"y":0.25,"width":0.25,"height":0.25,"visibility":"observed","confidence":1.0}]}),
        );
        assert_eq!(exact["region_median_iou"], 1.0);
        assert_eq!(exact["critical_region_min_iou"], 1.0);

        let mut shifted = vec![false; 512 * 512];
        for y in 128..256 {
            for x in 160..288 {
                shifted[y * 512 + x] = true;
            }
        }
        let partial = compare_masks(
            &reference,
            &shifted,
            &json!({"landmarks":[],"regions":[{"region_id":"core","x":0.25,"y":0.25,"width":0.25,"height":0.25,"visibility":"observed","confidence":1.0}]}),
        );
        assert!(partial["region_median_iou"].as_f64().unwrap() < 1.0);
        assert!(partial["region_median_iou"].as_f64().unwrap() > 0.0);
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
        let _baseline_visual = runtime
            .prepare_reference_comparison(
                &project.project_id,
                json!({"candidate_id":candidate_id,"reference_id":reference.reference_id,"view_spec":view_spec}),
            )
            .expect("fixed renderer comparison");
        let target = runtime
            .prepare_reference_mask(
                &project.project_id,
                json!({
                    "project_id":project.project_id.clone(),
                    "reference_id":reference.reference_id.clone(),
                    "contour_points":[[0.1,0.1],[0.9,0.1],[0.9,0.9],[0.1,0.9]]
                }),
            )
            .expect("camera reference target");
        let target_sha256 = target["target_sha256"].as_str().unwrap().to_owned();
        let camera_fit = runtime
            .prepare_camera_fit(
                &project.project_id,
                json!({
                    "project_id":project.project_id.clone(),
                    "candidate_id":candidate_id.clone(),
                    "target_sha256":target_sha256.clone(),
                    "camera":null
                }),
            )
            .expect("camera fit for reference");
        let selected_camera = camera_fit["selected_camera"].as_object().unwrap();
        let camera_ref = json!({
            "schema_version":"CameraCalibrationRef@1",
            "camera_hash":selected_camera["camera_hash"].clone(),
            "canonical_sha256":selected_camera["canonical_sha256"].clone()
        });
        let ref_bound_visual = runtime
            .prepare_reference_comparison(
                &project.project_id,
                json!({
                    "candidate_id":candidate_id.clone(),
                    "reference_id":reference.reference_id.clone(),
                    "view_spec":view_spec.clone(),
                    "camera":camera_ref,
                    "target_sha256":target_sha256
                }),
            )
            .expect("hash-bound camera comparison");
        assert_eq!(
            ref_bound_visual["render_set"]["camera_hash"],
            selected_camera["camera_hash"]
        );
        let target_bound_viewer = runtime
            .visual_evidence(&candidate_id)
            .expect("target-bound Viewer evidence");
        assert_eq!(target_bound_viewer["target_sha256"], target_sha256);
        let target_bound_observation = runtime
            .agentic_scene_observe(&project.project_id, Some(&candidate_id))
            .expect("target-bound Agentic observation");
        assert_eq!(
            target_bound_observation["visual_evidence_bundle"]["hashes"]["target_sha256"],
            target_sha256
        );
        assert_eq!(
            target_bound_observation["design_critic_report"]["primary_form_directive"]["owner"],
            "runtime"
        );
        assert_eq!(
            target_bound_observation["design_critic_report"]["primary_form_directive"]["target_sha256"],
            target_sha256
        );
        // Restore the ordinary compatibility comparison as the active review
        // evidence for the remainder of this legacy renderer fixture. The
        // preceding ref-bound call is the focused assertion for the new
        // CameraCalibrationRef path; a later self-rendered reference must use
        // the same default framing as the existing fixture.
        let prepared_visual = runtime
            .prepare_reference_comparison(
                &project.project_id,
                json!({
                    "candidate_id":candidate_id.clone(),
                    "reference_id":reference.reference_id.clone(),
                    "view_spec":view_spec.clone()
                }),
            )
            .expect("restore default camera comparison");
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
                "issues":[{"issue_id":"primitive-blockout","pass":"silhouette","region_id":"whole-body","claim":"Primitive-only candidate is a structural blockout and does not yet reproduce the panel, vent, cable and joint detail visible in the reference.","confidence":0.98,"visibility":"observed","action":"Keep this candidate as comparison evidence; activate supported hard-surface detail operators in a later MCP010D goal before claiming likeness."}],
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
        let candidate_record = runtime
            .candidate(&candidate_id)
            .expect("candidate query")
            .expect("candidate record");
        let visual_failure = runtime
            .confirm_candidate(&CandidateConfirmRequest {
                project_id: project.project_id.clone(),
                candidate_id: candidate_id.clone(),
                base_version_id: None,
                prepared_object_id: candidate_record
                    .prepared_object_id
                    .clone()
                    .expect("prepared object ID"),
                prepared_object_sha256: candidate_record
                    .prepared_object_sha256
                    .clone()
                    .expect("prepared object hash"),
                quality_report_id: candidate_record
                    .quality_report_id
                    .clone()
                    .expect("quality report ID"),
                approval_receipt_id: "mcp010c-visual-failure-approval".to_owned(),
                approval_summary: "Attempt to confirm a visual target failure".to_owned(),
                approval_session_id: "mcp010c-visual-failure-session".to_owned(),
                approval_expires_at: "9999999999".to_owned(),
                idempotency_key: "mcp010c-visual-failure-confirm".to_owned(),
            })
            .expect_err("QUALITY_TARGET_NOT_MET must not create a version");
        assert!(visual_failure
            .to_string()
            .contains("QUALITY_TARGET_NOT_MET"));
        assert!(runtime
            .versions(Some(&project.project_id))
            .expect("versions after visual gate")
            .is_empty());

        // A self-rendered silhouette is a deterministic structural fixture for
        // the positive confirmation/export path.  It is not evidence that the
        // user robot reference passed likeness; that separate run remains
        // QUALITY_TARGET_NOT_MET.
        let silhouette_base64 = runtime
            .render_pass_get(render_set_hash, "silhouette")
            .expect("silhouette pass")
            .get("png_base64")
            .and_then(Value::as_str)
            .expect("silhouette bytes")
            .to_owned();
        let silhouette_bytes = base64::engine::general_purpose::STANDARD
            .decode(silhouette_base64)
            .expect("silhouette base64");
        let mut silhouette_image = image::load_from_memory(&silhouette_bytes)
            .expect("silhouette PNG")
            .to_rgba8();
        // Keep the thresholded RGB mask identical while changing the CAS
        // bytes, because the original render pass already occupies the same
        // content hash with a different immutable object kind.
        for pixel in silhouette_image.pixels_mut() {
            pixel.0[3] = 254;
        }
        let mut matched_reference_bytes = Vec::new();
        image::DynamicImage::ImageRgba8(silhouette_image)
            .write_to(
                &mut Cursor::new(&mut matched_reference_bytes),
                ImageFormat::Png,
            )
            .expect("matched reference PNG");
        let matched_reference = runtime
            .import_reference(&ReferenceImportRequest {
                project_id: project.project_id.clone(),
                source: ReferenceImportSource::InlineContent {
                    mime: "image/png".to_owned(),
                    content_base64: base64::engine::general_purpose::STANDARD
                        .encode(matched_reference_bytes),
                },
                authorization: ReferenceAuthorization {
                    user_authorized: true,
                    declaration: "MCP010C deterministic renderer fixture reference".to_owned(),
                },
                expected_sha256: None,
            })
            .expect("matched reference import")
            .reference;
        let mut matched_view_spec = json!({
            "schema_version":"ReferenceViewSpec@1",
            "reference_id":matched_reference.reference_id,
            "reference_sha256":matched_reference.object_sha256,
            "view_id":"renderer-self-match",
            "source_view":"three-quarter",
            "image":{"width":512,"height":512,"rotation_degrees":0.0,"crop":{"x":0.0,"y":0.0,"width":1.0,"height":1.0}},
            "landmarks":[{"landmark_id":"fixture-center","x":0.5,"y":0.5,"visibility":"observed","confidence":1.0}],
            "regions":[{"region_id":"fixture-core","x":0.40234375,"y":0.466796875,"width":0.197265625,"height":0.248046875,"visibility":"observed","confidence":1.0}],
            "canonical_sha256":""
        });
        matched_view_spec["canonical_sha256"] =
            Value::String(canonical_json_hash(&matched_view_spec));
        let matched = runtime
            .prepare_reference_comparison(
                &project.project_id,
                json!({"candidate_id":candidate_id,"reference_id":matched_reference.reference_id,"view_spec":matched_view_spec}),
            )
            .expect("self-rendered reference comparison");
        assert_eq!(
            matched["quality_report"]["visual_status"],
            "PARTIAL_VISIBLE_VIEW_PASS"
        );
        let matched_render_set_hash = matched["render_set_object_sha256"]
            .as_str()
            .expect("matched RenderSet hash")
            .to_owned();
        let matched_comparison_hash = matched["comparison_report_object_sha256"]
            .as_str()
            .expect("matched comparison hash")
            .to_owned();
        let approved_human = runtime
            .submit_human_visual_review(json!({
                "candidate_id":candidate_id,
                "reference_id":matched_reference.reference_id,
                "render_set_hash":matched_render_set_hash,
                "comparison_report_hash":matched_comparison_hash,
                "scores":{"likeness":5,"geometry_detail":5,"material_fidelity":5,"editability":5},
                "approved":true
            }))
            .expect("approved human fixture review");
        assert_eq!(approved_human["receipt"]["approved"], true);
        let confirmed_candidate = runtime
            .candidate(&candidate_id)
            .expect("candidate after matched review")
            .expect("candidate after matched review record");
        let confirmed = runtime
            .confirm_candidate(&CandidateConfirmRequest {
                project_id: project.project_id.clone(),
                candidate_id: candidate_id.clone(),
                base_version_id: None,
                prepared_object_id: confirmed_candidate
                    .prepared_object_id
                    .clone()
                    .expect("confirmed prepared object ID"),
                prepared_object_sha256: confirmed_candidate
                    .prepared_object_sha256
                    .clone()
                    .expect("confirmed prepared object hash"),
                quality_report_id: confirmed_candidate
                    .quality_report_id
                    .clone()
                    .expect("confirmed quality report ID"),
                approval_receipt_id: "mcp010c-positive-approval".to_owned(),
                approval_summary: "Approve the deterministic visual fixture".to_owned(),
                approval_session_id: "mcp010c-positive-session".to_owned(),
                approval_expires_at: "9999999999".to_owned(),
                idempotency_key: "mcp010c-positive-confirm".to_owned(),
            })
            .expect("visual hard gate should allow confirmation");
        let export = runtime
            .prepare_export(&ExportPrepareRequest {
                project_id: project.project_id.clone(),
                version_id: confirmed.version_id.clone(),
                format: "glb".to_owned(),
                profile: "mvp-glb".to_owned(),
                request: json!({"reason":"MCP010C export hash fixture"}),
            })
            .expect("C export prepare");
        let exported = runtime
            .confirm_export(&ExportConfirmRequest {
                project_id: project.project_id.clone(),
                export_id: export.manifest.export_id.clone(),
                version_id: confirmed.version_id,
                format: "glb".to_owned(),
                profile: "mvp-glb".to_owned(),
                approval_receipt_id: "mcp010c-export-approval".to_owned(),
                approval_summary: "Approve C structural export".to_owned(),
                approval_session_id: "mcp010c-positive-session".to_owned(),
                approval_expires_at: "9999999999".to_owned(),
                idempotency_key: "mcp010c-export-once".to_owned(),
            })
            .expect("C export confirm");
        assert_eq!(exported.output_sha256, export.manifest.artifact_hashes[0]);

        // Viewer evidence must fail closed in Runtime when a valid RenderSet
        // is relinked to an unrelated artifact.  Client-side binding checks
        // are useful defense in depth, but they cannot be the quality truth
        // boundary because a Viewer payload may be stale or tampered with.
        let mut mismatched_render_set = matched["render_set"].clone();
        mismatched_render_set["artifact_sha256"] = Value::String("d".repeat(64));
        mismatched_render_set["canonical_sha256"] = Value::String(String::new());
        mismatched_render_set["canonical_sha256"] =
            Value::String(canonical_json_hash(&mismatched_render_set));
        let mismatched_render_set_object = runtime
            .put_object(
                &canonical_json_bytes(&mismatched_render_set).expect("mismatched RenderSet JSON"),
                None,
                "application/json",
                "render-set-tamper-fixture",
            )
            .expect("mismatched RenderSet object");
        let evidence = runtime
            .store
            .get_visual_evidence(&candidate_id)
            .expect("visual evidence query")
            .expect("visual evidence record");
        runtime
            .store
            .upsert_visual_evidence(&VisualEvidenceRecord {
                render_set_object_sha256: mismatched_render_set_object.record.sha256,
                ..evidence
            })
            .expect("tamper fixture evidence");
        let binding_error = runtime
            .visual_evidence(&candidate_id)
            .expect_err("Viewer evidence must reject mismatched artifact lineage");
        assert!(binding_error
            .to_string()
            .contains("RenderSet artifact differs from candidate"));
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
                    "project_id":project.project_id.clone(),
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
    fn silhouette_target_is_hash_bound_and_refinement_is_immutable() {
        let runtime = Runtime::ephemeral().expect("runtime");
        let project = runtime
            .create_project("silhouette target", json!({"profile":"mvp"}))
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
                    declaration: "silhouette target test".to_owned(),
                },
                expected_sha256: None,
            })
            .expect("reference")
            .reference;
        let first = runtime
            .prepare_reference_mask(
                &project.project_id,
                json!({
                    "project_id":project.project_id.clone(),
                    "reference_id":reference.reference_id,
                    "contour_points":[[0.2,0.15],[0.8,0.15],[0.85,0.85],[0.15,0.85]],
                    "landmarks":[],
                    "parts":[]
                }),
            )
            .expect("target");
        assert_eq!(first["schema_version"], "ReferenceMaskPrepareResult@1");
        assert_eq!(first["target"]["source"], "user_refined");
        validate_reference_mask_prepare_result(&first).expect("target contract");
        let first_hash = first["target_sha256"].as_str().unwrap().to_owned();
        let refined = runtime
            .refine_reference_mask(
                &project.project_id,
                json!({
                    "project_id":project.project_id.clone(),
                    "base_target_sha256":first_hash,
                    "contour_points":[[0.25,0.2],[0.75,0.2],[0.9,0.8],[0.1,0.8]]
                }),
            )
            .expect("refined target");
        assert_ne!(refined["target_sha256"], first["target_sha256"]);
        assert_eq!(runtime.silhouette_target_get(&first_hash).unwrap()["target_id"], first["target"]["target_id"]);
        assert_eq!(refined["target"]["source"], "user_refined");
        assert!(runtime
            .refine_reference_mask(
                &project.project_id,
                json!({
                    "project_id":project.project_id.clone(),
                    "base_target_sha256":"f".repeat(64),
                    "contour_points":[[0.1,0.1],[0.9,0.1],[0.9,0.9]]
                }),
            )
            .is_err());
    }

    #[test]
    fn automatic_silhouette_target_round_trips_float_contour_hash() {
        let runtime = Runtime::ephemeral().expect("runtime");
        let project = runtime
            .create_project("automatic silhouette target", json!({"profile":"mvp"}))
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
                    declaration: "automatic silhouette target test".to_owned(),
                },
                expected_sha256: None,
            })
            .expect("reference")
            .reference;
        let mut mask = vec![false; 512 * 512];
        for y in 32..480 {
            for x in 96..416 {
                mask[y * 512 + x] = true;
            }
        }
        let reference_record = runtime
            .reference(&reference.reference_id)
            .expect("reference lookup")
            .expect("reference record");
        let prepared = runtime
            .store_silhouette_target(
                &project.project_id,
                &reference_record,
                None,
                json!([]),
                json!([]),
                ReferenceMask {
                    mask,
                    png: Vec::new(),
                },
                true,
            )
            .expect("automatic target");
        let target_sha = prepared["target_sha256"]
            .as_str()
            .expect("target hash");
        let readback = runtime
            .silhouette_target_get(target_sha)
            .expect("automatic target readback");
        assert_eq!(readback["schema_version"], "SilhouetteTarget@1");
        assert_eq!(readback["source"], "automatic");
    }

    #[test]
    fn automatic_contour_points_are_ordered_and_follow_outer_mask_boundary() {
        let mut mask = vec![false; 512 * 512];
        for y in 32..480 {
            for x in 96..416 {
                mask[y * 512 + x] = true;
            }
        }
        // A detached foreground island must not be interleaved into the main
        // loop.  The binary mask remains the full comparison truth, while the
        // single automatic contour represents the largest outer component.
        for y in 440..470 {
            for x in 450..480 {
                mask[y * 512 + x] = true;
            }
        }
        let values = contour_points_from_mask(&mask);
        assert!(values.len() >= 4);
        let points = values
            .iter()
            .map(|value| {
                let point = value.as_array().expect("point array");
                [point[0].as_f64().unwrap(), point[1].as_f64().unwrap()]
            })
            .collect::<Vec<_>>();
        assert!(!contour_self_intersects(&points));
        assert!(points.iter().all(|[x, y]| (0.0..=1.0).contains(x) && (0.0..=1.0).contains(y)));
        let reconstructed = rasterize_contour(&points);
        let metrics = compare_masks(&mask, &reconstructed, &json!({"landmarks":[],"regions":[]}));
        assert!(metrics["silhouette_iou"].as_f64().unwrap() > 0.94);
    }

    #[test]
    fn landmark_annotations_change_only_the_transient_contour_ranking_loss() {
        let mut mask = vec![false; 512 * 512];
        for y in 96..416 {
            for x in 128..384 {
                mask[y * 512 + x] = true;
            }
        }
        let base = json!({
            "silhouette_iou": 0.8,
            "boundary_f1_4px": 0.8,
            "bbox_edge_error": 0.1,
            "centroid_error": 0.1,
            "sdf_chamfer_px": 10.0
        });
        let inside_landmarks = json!([
            {"landmark_id":"chest-center","x":0.5,"y":0.5,"visibility":"observed"}
        ]);
        let outside_landmarks = json!([
            {"landmark_id":"crown","x":0.05,"y":0.05,"visibility":"observed"}
        ]);
        let inside = contour_loss_metrics(&base, &mask, Some(&inside_landmarks));
        let outside = contour_loss_metrics(&base, &mask, Some(&outside_landmarks));
        assert_eq!(inside["landmark_coverage"].as_f64(), Some(1.0));
        assert_eq!(inside["landmark_nme"].as_f64(), Some(0.0));
        assert_eq!(outside["landmark_coverage"].as_f64(), Some(0.0));
        assert!(outside["landmark_nme"].as_f64().unwrap() > 0.0);
        assert!(weighted_contour_loss(&outside) > weighted_contour_loss(&inside));
        assert!(base.get("landmark_nme").is_none());
    }

    #[test]
    fn camera_loss_rejects_low_landmark_coverage_even_with_low_nme() {
        let mut covered = json!({
            "silhouette_iou": 0.74,
            "boundary_f1_4px": 0.33,
            "bbox_edge_error": 0.01,
            "centroid_error": 0.01,
            "sdf_chamfer_px": 17.0,
            "landmark_coverage": 0.733333333333,
            "landmark_nme": 0.1345
        });
        let sparse = json!({
            "silhouette_iou": 0.69,
            "boundary_f1_4px": 0.25,
            "bbox_edge_error": 0.035,
            "centroid_error": 0.042,
            "sdf_chamfer_px": 17.0,
            "landmark_coverage": 0.067398119122,
            "landmark_nme": 0.0914
        });
        assert!(weighted_contour_loss(&sparse) > weighted_contour_loss(&covered));
        covered["landmark_coverage"] = json!(1.0);
        assert!(weighted_contour_loss(&covered) < weighted_contour_loss(&sparse));
    }

    #[test]
    fn camera_fit_returns_bounded_hash_bound_candidates_without_mutating_candidate() {
        let runtime = Runtime::ephemeral().expect("runtime");
        let project = runtime
            .create_project("camera fit", json!({"profile":"mvp"}))
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
                    declaration: "camera fit test".to_owned(),
                },
                expected_sha256: None,
            })
            .expect("reference")
            .reference;
        let target = runtime
            .prepare_reference_mask(
                &project.project_id,
                json!({
                    "project_id":project.project_id.clone(),
                    "reference_id":reference.reference_id,
                    "contour_points":[[0.2,0.1],[0.8,0.1],[0.85,0.9],[0.15,0.9]]
                }),
            )
            .expect("target");
        let prepared = runtime
            .prepare_geometry_candidate(
                &project.project_id,
                None,
                json!({"typed":"geometry","geometry_program":v2_restore_program(&project.project_id)}),
            )
            .expect("candidate");
        let candidate_id = prepared["candidate"]["candidate_id"].as_str().unwrap().to_owned();
        let result = runtime
            .prepare_camera_fit(
                &project.project_id,
                json!({
                    "project_id":project.project_id.clone(),
                    "candidate_id":candidate_id,
                    "target_sha256":target["target_sha256"].clone()
                }),
            )
            .expect("camera fit");
        validate_camera_fit_result(&result).expect("camera fit contract");
        assert_eq!(result["candidate_id"], prepared["candidate"]["candidate_id"]);
        assert!(result["candidates"].as_array().unwrap().len() <= 64);
        assert_eq!(runtime.candidates(&project.project_id).unwrap().len(), 1);
        assert_eq!(runtime.versions(Some(&project.project_id)).unwrap().len(), 0);
        let selected = result["selected_camera"].clone();
        let camera_ref = json!({
            "schema_version":"CameraCalibrationRef@1",
            "camera_hash":selected["camera_hash"].clone(),
            "canonical_sha256":selected["canonical_sha256"].clone()
        });
        let resolved = runtime
            .resolve_silhouette_fit_camera(
                &project.project_id,
                &candidate_id,
                target["target_sha256"].as_str().unwrap(),
                &camera_ref,
            )
            .expect("compact camera reference resolves");
        assert_eq!(resolved, selected);
    }

    #[test]
    fn camera_fit_search_covers_global_scale_with_deterministic_budget() {
        let base = default_camera_calibration();
        let coarse = camera_fit_search_variants(&base);
        assert_eq!(coarse.len(), 37);
        assert!(coarse.iter().all(|camera| validate_camera_calibration(camera).is_ok()));
        let coarse_hashes = coarse
            .iter()
            .filter_map(|camera| camera.get("camera_hash").and_then(Value::as_str))
            .collect::<std::collections::HashSet<_>>();
        assert_eq!(coarse_hashes.len(), coarse.len());
        for global_scale in [0.96_f64, 1.04] {
            let expected = camera_fit_variant_extended(
                &base, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, global_scale,
            );
            assert!(coarse.iter().any(|camera| camera == &expected));
            assert_ne!(expected["camera_hash"], base["camera_hash"]);
        }
        let refinement = camera_fit_refinement_variants(&coarse[0]);
        assert_eq!(refinement.len(), 9);
        assert!(refinement.iter().all(|camera| validate_camera_calibration(camera).is_ok()));
        assert!(coarse.len() + 3 * refinement.len() <= 64);
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
    fn silhouette_rig_hash_is_runtime_owned_and_candidate_bound() {
        let runtime = Runtime::ephemeral().expect("runtime");
        let project = runtime
            .create_project("MCP010F rig hash project", json!({"profile":"mvp"}))
            .expect("project");
        let program = v2_restore_program(&project.project_id);
        let prepared = runtime
            .prepare_geometry_candidate(
                &project.project_id,
                None,
                json!({"typed":"geometry","geometry_program":program}),
            )
            .expect("candidate");
        let candidate_id = prepared["candidate"]["candidate_id"]
            .as_str()
            .expect("candidate id")
            .to_owned();
        let rig_draft = json!({
            "schema_version":"SilhouetteRig@1",
            "rig_id":"robot-silhouette-rig",
            "candidate_id":candidate_id.clone(),
            "parameters":[
                {"parameter_id":"chest-width","part_id":"shell","semantic":"width","value":1.0,"min":0.7,"max":1.3,"step":0.05,"unit":"meter"},
                {"parameter_id":"chest-height","part_id":"shell","semantic":"height","value":1.2,"min":0.9,"max":1.5,"step":0.05,"unit":"meter"}
            ]
        });
        let request = json!({
            "schema_version":"SilhouetteRigHashRequest@1",
            "project_id":project.project_id.clone(),
            "candidate_id":candidate_id.clone(),
            "rig_draft":rig_draft.clone()
        });
        let before_version_count = runtime
            .versions(Some(&project.project_id))
            .expect("versions")
            .len();
        let result = runtime
            .silhouette_rig_hash(&project.project_id, &request)
            .expect("runtime rig hash");
        assert_eq!(result["schema_version"], "SilhouetteRigHashResult@1");
        assert_eq!(result["validation_status"], "passed");
        let canonical = result["canonical_sha256"]
            .as_str()
            .expect("rig hash")
            .to_owned();
        assert!(forgecad_contracts::is_sha256(&canonical));
        let mut rig = rig_draft;
        rig["canonical_sha256"] = Value::String(canonical.clone());
        validate_silhouette_rig(&rig, &candidate_id).expect("hashed rig contract");
        assert_eq!(
            runtime
                .versions(Some(&project.project_id))
                .expect("versions")
                .len(),
            before_version_count
        );

        let mut prefilled = request.clone();
        prefilled["rig_draft"]["canonical_sha256"] = Value::String(canonical);
        assert!(runtime
            .silhouette_rig_hash(&project.project_id, &prefilled)
            .is_err());
        let mut wrong_project = request;
        wrong_project["project_id"] = json!("project-outside-scope");
        assert!(runtime
            .silhouette_rig_hash(&project.project_id, &wrong_project)
            .is_err());
    }

    #[test]
    fn contour_fit_part_proposal_and_candidate_compare_are_bounded_and_read_only() {
        let runtime = Runtime::ephemeral().expect("runtime");
        let project = runtime
            .create_project("MCP010F contour solver", json!({"profile":"mvp"}))
            .expect("project");
        let reference = runtime
            .import_reference(&ReferenceImportRequest {
                project_id: project.project_id.clone(),
                source: ReferenceImportSource::InlineContent {
                    mime: "image/png".to_owned(),
                    content_base64: "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=".to_owned(),
                },
                authorization: ReferenceAuthorization { user_authorized: true, declaration: "MCP010F contour solver fixture".to_owned() },
                expected_sha256: None,
            })
            .expect("reference")
            .reference;
        let mut program = v2_restore_program(&project.project_id);
        program["budgets"]["max_nodes"] = json!(2);
        program["nodes"]
            .as_array_mut()
            .expect("V2 nodes")
            .push(json!({
                "node_id":"visor",
                "operator_id":"forgecad.geometry.primitive@2",
                "inputs":[],
                "parameters":{"shape":"box","size_m":[0.4,0.25,0.2],"position_m":[0.0,1.8,0.0],"rotation_rad":[0.0,0.0,0.0]}
            }));
        program["part_outputs"]
            .as_array_mut()
            .expect("V2 Part outputs")
            .push(json!({
                "part_id":"visor",
                "input_node_ids":["visor"],
                "material_zone_id":"zone-white-shell",
                "solid":true
            }));
        program
            .as_object_mut()
            .expect("V2 program object")
            .remove("canonical_sha256");
        let program_hash = canonical_json_hash(&program);
        program["canonical_sha256"] = Value::String(program_hash);
        let prepare = |runtime: &Runtime| {
            runtime
                .prepare_geometry_candidate(&project.project_id, None, json!({"typed":"geometry","geometry_program":program.clone()}))
                .expect("geometry candidate")
        };
        let first = prepare(&runtime);
        let first_id = first["candidate"]["candidate_id"].as_str().unwrap().to_owned();
        // The silhouette-first product path stores the automatic mask before
        // comparison. Keep that ordering here so repeated CAS admission of
        // the same mask bytes is covered by the focused regression.
        let target = runtime
            .prepare_reference_mask(&project.project_id, json!({"project_id":project.project_id.clone(),"reference_id":reference.reference_id,"contour_points":[[0.1,0.1],[0.5,0.1],[0.9,0.1],[0.9,0.5],[0.9,0.9],[0.5,0.9],[0.1,0.9],[0.1,0.5]],"parts":[{"part_id":"shell","start_index":0,"end_index":3,"visibility":"observed"},{"part_id":"visor","start_index":4,"end_index":7,"visibility":"observed"}]}))
            .expect("target");
        let mut view_spec = json!({
            "schema_version":"ReferenceViewSpec@1","reference_id":reference.reference_id,"reference_sha256":reference.object_sha256,
            "view_id":"contour-fit-view","source_view":"three-quarter",
            "image":{"width":1,"height":1,"rotation_degrees":0.0,"crop":{"x":0.0,"y":0.0,"width":1.0,"height":1.0}},
            "landmarks":[],"regions":[],"canonical_sha256":""
        });
        view_spec["canonical_sha256"] = Value::String(canonical_json_hash(&view_spec));
        runtime
            .prepare_reference_comparison(&project.project_id, json!({"candidate_id":first_id,"reference_id":reference.reference_id,"view_spec":view_spec.clone()}))
            .expect("first comparison");
        let mut rig = json!({"schema_version":"SilhouetteRig@1","rig_id":"robot-rig","candidate_id":first_id.clone(),"parameters":[{"parameter_id":"shell-width","part_id":"shell","semantic":"width","value":1.0,"min":0.5,"max":1.5,"step":0.05,"unit":"meter"}],"canonical_sha256":""});
        rig["canonical_sha256"] = Value::String(canonical_json_hash(&rig));
        let mut fit_request = json!({"project_id":project.project_id.clone(),"candidate_id":first_id.clone(),"target_sha256":target["target_sha256"].clone(),"rig":rig.clone(),"base_camera":default_camera_calibration(),"optimizer":{"algorithm":"coordinate_descent","max_iterations":2,"max_evaluations":8,"step_fraction":0.1},"canonical_sha256":""});
        fit_request["canonical_sha256"] = Value::String(canonical_json_hash(&fit_request));
        let fit = runtime.silhouette_fit_prepare(&project.project_id, fit_request).expect("fit");
        validate_silhouette_fit_result(&fit).expect("fit contract");
        let before_primary_form_versions = runtime.versions(Some(&project.project_id)).unwrap().len();
        let mut primary_form_request = json!({
            "project_id":project.project_id.clone(),
            "candidate_id":first_id.clone(),
            "target_sha256":target["target_sha256"].clone(),
            "rig":rig.clone(),
            "base_camera":default_camera_calibration(),
            "optimizer":{"algorithm":"coordinate_descent","max_iterations":2,"max_evaluations":64,"step_fraction":0.1},
            "canonical_sha256":""
        });
        primary_form_request["canonical_sha256"] =
            Value::String(canonical_json_hash(&primary_form_request));
        let primary_form = runtime
            .primary_form_repair_prepare(&project.project_id, None, primary_form_request)
            .expect("Primary Form repair prepare");
        validate_primary_form_repair_prepare_result(&primary_form)
            .expect("Primary Form repair contract");
        assert_eq!(primary_form["source_candidate_id"], first_id);
        assert_eq!(primary_form["target_sha256"], target["target_sha256"]);
        assert_eq!(primary_form["version_created"], false);
        assert_eq!(
            runtime.versions(Some(&project.project_id)).unwrap().len(),
            before_primary_form_versions
        );
        if primary_form["status"] == "prepared" {
            assert_eq!(primary_form["candidate_state"], "staged_new_candidate");
            assert_eq!(
                primary_form["visual_evidence"]["target_sha256"],
                target["target_sha256"]
            );
            assert_eq!(
                primary_form["visual_evidence"]["quality_report"]["candidate_id"],
                primary_form["visual_evidence"]["candidate_id"]
            );
        } else {
            assert_eq!(primary_form["status"], "no_improvement");
            assert_eq!(primary_form["candidate_state"], "unchanged");
        }
        let primary_form_evaluations = primary_form["fit_result"]["evaluations"]
            .as_u64()
            .expect("Primary Form fit evaluation count");
        assert!((63..=64).contains(&primary_form_evaluations));
        let fit_camera = fit["selected_camera"].clone();
        let fit_camera_ref = json!({
            "schema_version": "CameraCalibrationRef@1",
            "camera_hash": fit_camera["camera_hash"].clone(),
            "canonical_sha256": fit_camera["canonical_sha256"].clone()
        });
        let resolved_fit_camera = runtime
            .resolve_silhouette_fit_camera(
                &project.project_id,
                &first_id,
                target["target_sha256"].as_str().unwrap(),
                &fit_camera_ref,
            )
            .expect("fit winner camera ref resolves");
        assert_eq!(resolved_fit_camera, fit_camera);
        let comparison = runtime
            .prepare_reference_comparison(
                &project.project_id,
                json!({
                    "project_id":project.project_id.clone(),
                    "candidate_id":first_id.clone(),
                    "reference_id":reference.reference_id.clone(),
                    "view_spec":view_spec,
                    "camera":fit_camera_ref,
                    "target_sha256":target["target_sha256"].clone()
                }),
            )
            .expect("fit winner comparison");
        assert_eq!(comparison["camera"]["camera_hash"], fit_camera["camera_hash"]);
        assert_eq!(comparison["camera"]["canonical_sha256"], fit_camera["canonical_sha256"]);
        // The optimizer may retain the authored camera as the incumbent, but
        // it must still consume the declared bounded camera schedule instead
        // of stopping after the first non-improving batch and hiding later
        // axes from the proposal.
        assert_eq!(fit["iterations"].as_u64(), Some(2));
        assert_eq!(fit["evaluations"].as_u64(), Some(8));
        assert!(fit["geometry_evaluations"].as_u64().unwrap() <= 5);
        assert_eq!(fit["parameter_deltas"].as_array().map(Vec::len), Some(1));
        assert!(fit["parameter_deltas"][0]["delta"].as_f64().unwrap().is_finite());
        assert!(fit.get("selected_geometry_program").is_some());
        if let Some(program) = fit["selected_geometry_program"].as_object() {
            assert_eq!(program["schema_version"], "GeometryProgram@2");
            assert_eq!(program["project_id"], project.project_id);
            assert!(program["canonical_sha256"].as_str().is_some());
        }
        assert!(matches!(fit["status"].as_str(), Some("ready" | "quality_target_not_met" | "no_improvement")));
        let part = runtime.part_contour_fit_prepare(&project.project_id, json!({"project_id":project.project_id.clone(),"candidate_id":first_id.clone(),"target_sha256":target["target_sha256"].clone(),"part_id":"shell","rig":rig.clone()})).expect("part proposal");
        validate_part_contour_fit_result(&part).expect("part contract");
        assert_eq!(part["adjustments"].as_array().map(Vec::len), Some(1));
        assert!(part["adjustments"][0]["delta"].as_f64().unwrap().is_finite());
        let part_errors = runtime
            .silhouette_part_error(
                &project.project_id,
                json!({
                    "project_id":project.project_id.clone(),
                    "candidate_id":first_id.clone(),
                    "target_sha256":target["target_sha256"].clone()
                }),
            )
            .expect("per-Part contour error");
        validate_silhouette_part_error_result(&part_errors).expect("per-Part contour contract");
        assert_eq!(part_errors["parts"].as_array().map(Vec::len), Some(2));
        assert_eq!(part_errors["parts"][0]["part_id"], "shell");
        let recommended = part_errors["recommended_part_ids"]
            .as_array()
            .expect("recommended Part IDs");
        assert!(recommended.iter().any(|id| id == "shell"));
        assert!(recommended.iter().any(|id| id == "visor"));
        assert_eq!(part_errors["parts"][1]["part_id"], "visor");
        assert!(part_errors["parts"][1]["status"] == "ready");
        let observation = runtime
            .agentic_scene_observe(&project.project_id, Some(&first_id))
            .expect("Agentic observation includes Runtime Part error context");
        let directive = &observation["design_critic_report"]["primary_form_directive"];
        assert_eq!(directive["part_error"], part_errors);
        assert_eq!(
            directive["focus_part_id"],
            part_errors["recommended_part_ids"][0]
        );
        assert_eq!(directive["focus_part_status"], "observed");
        assert_eq!(
            observation["design_critic_report"]["repair_intents"][0]["scope"]["part_id"],
            directive["focus_part_id"]
        );
        assert!(runtime
            .part_contour_fit_prepare(&project.project_id, json!({"project_id":project.project_id.clone(),"candidate_id":first_id.clone(),"target_sha256":target["target_sha256"].clone(),"part_id":"unknown-part","rig":rig.clone()}))
            .is_err());
        let second = prepare(&runtime);
        let second_id = second["candidate"]["candidate_id"].as_str().unwrap().to_owned();
        runtime
            .prepare_reference_comparison(&project.project_id, json!({"candidate_id":second_id,"reference_id":reference.reference_id,"view_spec":view_spec}))
            .expect("second comparison");
        let compared = runtime.silhouette_candidate_compare(&project.project_id, json!({"project_id":project.project_id.clone(),"target_sha256":target["target_sha256"].clone(),"candidate_ids":[first_id,second_id]})).expect("compare");
        validate_silhouette_candidate_compare_result(&compared).expect("compare contract");
        assert_eq!(compared["candidates"].as_array().unwrap().len(), 2);
        assert_eq!(runtime.versions(Some(&project.project_id)).unwrap().len(), 0);
    }

    #[test]
    fn target_part_ranges_are_closed_and_part_boundary_uses_declared_slice() {
        let target = json!({
            "contour_points":[[0.1,0.1],[0.5,0.1],[0.9,0.1],[0.9,0.9],[0.5,0.9],[0.1,0.9]],
            "parts":[
                {"part_id":"left","start_index":0,"end_index":2,"visibility":"observed"},
                {"part_id":"right","start_index":3,"end_index":5,"visibility":"observed"}
            ]
        });
        validate_target_part_ranges(target.get("parts").unwrap(), 6, "test").expect("valid disjoint ranges");
        let left = target_part_boundary_mask(&target, "left").expect("left boundary");
        let right = target_part_boundary_mask(&target, "right").expect("right boundary");
        assert!(left.iter().any(|value| *value));
        assert!(right.iter().any(|value| *value));
        assert_ne!(left, right);
        assert!(target_part_boundary_mask(&target, "missing").is_none());
        let overlapping = json!([
            {"part_id":"left","start_index":0,"end_index":2,"visibility":"observed"},
            {"part_id":"right","start_index":2,"end_index":5,"visibility":"observed"}
        ]);
        assert!(validate_target_part_ranges(&overlapping, 6, "test").is_err());
        let outside = json!([
            {"part_id":"left","start_index":0,"end_index":6,"visibility":"observed"}
        ]);
        assert!(validate_target_part_ranges(&outside, 6, "test").is_err());
    }

    #[test]
    fn primary_form_probe_schedule_covers_all_ranked_parameters_before_repeat() {
        let ranked = (0..26).collect::<Vec<_>>();
        let first_pass = (1..=ranked.len())
            .map(|probe_index| primary_form_probe_coordinate(&ranked, probe_index).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(first_pass, ranked);
        assert_eq!(primary_form_probe_coordinate(&ranked, ranked.len() + 1), Some(0));
        assert_eq!(primary_form_probe_coordinate(&[], 1), None);
        assert_eq!(primary_form_probe_coordinate(&ranked, 0), None);
    }

    #[test]
    fn primary_form_repair_optimizer_preserves_bounded_detail_budget() {
        let mut optimizer = json!({
            "max_evaluations": 64,
            "max_iterations": 2
        });
        normalize_primary_form_repair_optimizer(optimizer.as_object_mut().unwrap());
        assert_eq!(optimizer["max_evaluations"], 64);
        assert_eq!(optimizer["max_iterations"], 1);

        let mut oversized = json!({
            "max_evaluations": 128,
            "max_iterations": 8
        });
        normalize_primary_form_repair_optimizer(oversized.as_object_mut().unwrap());
        assert_eq!(oversized["max_evaluations"], 64);
        assert_eq!(oversized["max_iterations"], 1);

        let mut defaults = json!({});
        normalize_primary_form_repair_optimizer(defaults.as_object_mut().unwrap());
        assert_eq!(defaults["max_evaluations"], 64);
        assert_eq!(defaults["max_iterations"], 1);
    }

    #[test]
    fn primary_form_joint_proposal_backtracks_inside_authored_bounds() {
        let definitions = vec![
            json!({
                "parameter_id":"chest-width",
                "part_id":"chest-shell",
                "semantic":"width",
                "value":1.0,
                "min":0.5,
                "max":1.5,
                "step":0.05,
                "unit":"ratio"
            }),
            json!({
                "parameter_id":"chest-offset-x",
                "part_id":"chest-shell",
                "semantic":"offset_x",
                "value":0.0,
                "min":-0.4,
                "max":0.4,
                "step":0.05,
                "unit":"meter"
            }),
        ];
        let proposal = vec![
            json!({"parameter_id":"chest-width","part_id":"chest-shell","value":1.4}),
            json!({"parameter_id":"chest-offset-x","part_id":"chest-shell","value":0.3}),
        ];
        let half = interpolate_rig_parameter_values(&definitions, &proposal, 0.5);
        let quarter = interpolate_rig_parameter_values(&definitions, &proposal, 0.25);
        assert_eq!(half[0]["value"], 1.2);
        assert_eq!(half[1]["value"], 0.15);
        assert_eq!(quarter[0]["value"], 1.1);
        assert_eq!(quarter[1]["value"], 0.075);
        assert!(half.iter().zip(&definitions).all(|(value, definition)| {
            let candidate = value["value"].as_f64().unwrap();
            let min = definition["min"].as_f64().unwrap();
            let max = definition["max"].as_f64().unwrap();
            (min..=max).contains(&candidate)
        }));
    }

    #[test]
    fn primary_form_budget_honors_declared_bound_with_geometry_priority() {
        assert_eq!(primary_form_evaluation_budgets(24, true), (12, 6, 6));
        assert_eq!(primary_form_evaluation_budgets(8, true), (4, 2, 2));
        assert_eq!(primary_form_evaluation_budgets(1, true), (0, 1, 0));
        assert_eq!(primary_form_evaluation_budgets(64, true), (32, 16, 16));
        assert_eq!(primary_form_evaluation_budgets(24, false), (0, 24, 0));
        let detail_rig_parameter_count = 26;
        let detail_budgets = primary_form_evaluation_budgets(64, true);
        assert!(detail_budgets.0 >= detail_rig_parameter_count + 1);
        for max_evaluations in 1..=64 {
            let budgets = primary_form_evaluation_budgets(max_evaluations, true);
            assert!(budgets.0 + budgets.1 + budgets.2 <= max_evaluations);
            assert!(budgets.0 <= 40 && budgets.1 <= 24 && budgets.2 <= 24);
        }
    }

    #[test]
    fn primary_form_camera_refit_schedule_is_bounded_and_keeps_winner_first() {
        let base = default_camera_calibration();
        let schedule = primary_form_camera_refit_schedule(&base, 6);
        assert_eq!(schedule.len(), 6);
        assert_eq!(schedule.first(), Some(&base));
        assert!(schedule
            .iter()
            .all(|camera| validate_camera_calibration(camera).is_ok()));
        let hashes = schedule
            .iter()
            .filter_map(|camera| camera.get("camera_hash").and_then(Value::as_str))
            .collect::<std::collections::HashSet<_>>();
        assert_eq!(hashes.len(), schedule.len());
        assert!(primary_form_camera_refit_schedule(&base, 64).len() <= 64);
    }

    #[test]
    fn part_contour_adjustment_uses_local_width_height_and_centroid() {
        let mut target_mask = vec![false; 512 * 512];
        let mut model_mask = vec![false; 512 * 512];
        for y in 100..200 {
            for x in 100..200 {
                target_mask[y * 512 + x] = true;
            }
        }
        for y in 110..160 {
            for x in 110..160 {
                model_mask[y * 512 + x] = true;
            }
        }
        let target = mask_envelope(&target_mask).expect("target envelope");
        let model = mask_envelope(&model_mask).expect("model envelope");
        let width = json!({
            "parameter_id":"shell-width","part_id":"shell","semantic":"width",
            "value":1.0,"min":0.5,"max":1.5,"step":0.05,"unit":"meter"
        });
        let height = json!({
            "parameter_id":"shell-height","part_id":"shell","semantic":"height",
            "value":1.0,"min":0.5,"max":1.5,"step":0.05,"unit":"meter"
        });
        let offset = json!({
            "parameter_id":"shell-offset-x","part_id":"shell","semantic":"offset_x",
            "value":0.0,"min":-0.5,"max":0.5,"step":0.05,"unit":"ratio"
        });
        assert!(local_part_parameter_delta(&width, target, model) > 0.0);
        assert!(local_part_parameter_delta(&height, target, model) > 0.0);
        assert!(local_part_parameter_delta(&offset, target, model) > 0.0);
        let depth = json!({
            "parameter_id":"shell-depth","part_id":"shell","semantic":"depth",
            "value":1.0,"min":0.5,"max":1.5,"step":0.05,"unit":"meter"
        });
        assert_eq!(local_part_parameter_delta(&depth, target, model), 0.0);
    }

    #[test]
    fn automatic_part_boundary_projection_uses_only_attributed_samples() {
        let segments = vec![
            json!({"part_id":"shin-pair","reference":[0.20,0.80],"distance_px":48.0}),
            json!({"part_id":"shin-pair","reference":[0.62,0.90],"distance_px":32.0}),
            json!({"part_id":"head-shell","reference":[0.56,0.15],"distance_px":42.0}),
        ];
        let mask = Runtime::projected_part_boundary_mask(&segments, "shin-pair").expect("projected mask");
        let envelope = mask_envelope(&mask).expect("projected envelope");
        assert!(envelope.min_x < envelope.max_x);
        assert!(envelope.min_y < envelope.max_y);
        assert_eq!(Runtime::projected_part_boundary_error(&segments, "shin-pair"), Some(40.0));
        assert!(Runtime::projected_part_boundary_mask(&segments, "missing").is_none());
        assert!(Runtime::projected_part_boundary_error(&segments, "missing").is_none());
    }

    #[test]
    fn global_rig_proposal_uses_axis_specific_bbox_ratios() {
        let mut target_mask = vec![false; 512 * 512];
        let mut model_mask = vec![false; 512 * 512];
        for y in 100..220 {
            for x in 100..250 {
                target_mask[y * 512 + x] = true;
            }
        }
        for y in 100..200 {
            for x in 100..200 {
                model_mask[y * 512 + x] = true;
            }
        }
        let rig = json!({"parameters":[
            {"parameter_id":"body-width","part_id":"body","semantic":"width","value":1.0,"min":0.5,"max":2.0,"step":0.05,"unit":"ratio"},
            {"parameter_id":"body-height","part_id":"body","semantic":"height","value":1.0,"min":0.5,"max":2.0,"step":0.05,"unit":"ratio"},
            {"parameter_id":"body-scale","part_id":"body","semantic":"scale","value":1.0,"min":0.5,"max":2.0,"step":0.05,"unit":"ratio"}
        ]});
        let selected = fit_rig_parameters(&rig, &target_mask, &model_mask);
        assert_eq!(selected[0]["value"], 1.5);
        assert_eq!(selected[1]["value"], 1.2);
        assert!((selected[2]["value"].as_f64().unwrap() - 1.3416407865).abs() < 1e-10);
    }

    #[test]
    fn silhouette_fit_compacts_authored_baseline_parameters() {
        let rig = json!({"parameters":[
            {"parameter_id":"body-width","part_id":"body","semantic":"width","value":1.0,"min":0.5,"max":2.0,"step":0.05,"unit":"ratio"}
        ]});
        let authored_baseline = vec![rig["parameters"][0].clone()];
        let compact = compact_rig_parameter_values(&rig, &authored_baseline);
        assert_eq!(compact, vec![json!({
            "parameter_id":"body-width",
            "part_id":"body",
            "value":1.0
        })]);
        assert_eq!(compact[0].as_object().unwrap().len(), 3);
    }

    #[test]
    fn silhouette_rig_proposal_prefers_part_envelope_over_global_envelope() {
        let mut global_mask = vec![false; 512 * 512];
        for y in 100..200 {
            for x in 100..200 {
                global_mask[y * 512 + x] = true;
            }
        }
        let target = json!({
            "contour_points":[[0.2,0.2],[0.8,0.2],[0.8,0.8],[0.2,0.8]],
            "parts":[{"part_id":"shell","start_index":0,"end_index":3,"visibility":"observed"}]
        });
        let mut part_image = RgbaImage::from_pixel(512, 512, Rgba([0, 0, 0, 0]));
        let part_color = Rgba([73, 119, 91, 255]);
        for y in 100..200 {
            for x in 100..200 {
                part_image.put_pixel(x, y, part_color);
            }
        }
        let mut part_png = Vec::new();
        part_image
            .write_to(&mut Cursor::new(&mut part_png), ImageFormat::Png)
            .expect("part-id png");
        assert!(decode_part_mask(&part_png, "shell", &["shell".to_owned()]).is_some());
        assert!(target_part_boundary_mask(&target, "shell")
            .and_then(|mask| mask_envelope(&mask))
            .is_some());
        let rig = json!({
            "parameters":[{"parameter_id":"shell-width","part_id":"shell","semantic":"width","value":1.0,"min":0.5,"max":1.5,"step":0.05,"unit":"meter"}]
        });
        let selected = fit_rig_parameters_with_part_context(
            &rig,
            &target,
            &global_mask,
            &global_mask,
            Some((&part_png, &["shell".to_owned()])),
        );
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0]["part_id"], "shell");
        assert_eq!(selected[0]["value"], 1.25);
    }

    #[test]
    fn landmark_offsets_are_camera_calibrated_and_part_owned() {
        let mut part_image = RgbaImage::from_pixel(512, 512, Rgba([0, 0, 0, 0]));
        let part_color = Rgba([73, 119, 91, 255]);
        for y in 120..220 {
            for x in 120..220 {
                part_image.put_pixel(x, y, part_color);
            }
        }
        let mut part_png = Vec::new();
        part_image
            .write_to(&mut Cursor::new(&mut part_png), ImageFormat::Png)
            .expect("part-id png");
        let target = json!({
            "landmarks":[
                {"landmark_id":"chest-center","x":0.38,"y":0.26,"visibility":"observed","confidence":1.0}
            ]
        });
        let rig = json!({
            "parameters":[
                {"parameter_id":"chest-offset-x","part_id":"chest-shell","semantic":"offset_x","value":0.0,"min":-0.35,"max":0.35,"step":0.05,"unit":"meter"},
                {"parameter_id":"chest-offset-y","part_id":"chest-shell","semantic":"offset_y","value":0.0,"min":-0.35,"max":0.35,"step":0.05,"unit":"meter"}
            ]
        });
        let model_mask = vec![false; 512 * 512];
        let selected = fit_rig_parameters_with_landmark_context(
            &rig,
            &target,
            &model_mask,
            &model_mask,
            Some((&part_png, &["chest-shell".to_owned()])),
            Some(&default_camera_calibration()),
        );
        assert_eq!(selected.len(), 2);
        assert!(selected[0]["value"].as_f64().unwrap() > 0.0);
        assert!(selected[1]["value"].as_f64().unwrap() > 0.0);
        assert!(selected[0]["value"].as_f64().unwrap() <= 0.35);
        assert!(selected[1]["value"].as_f64().unwrap() <= 0.35);
    }

    #[test]
    fn primary_form_geometry_objective_keeps_landmark_penalty() {
        let mut model = vec![false; 512 * 512];
        model[255 * 512 + 255] = true;
        let base = json!({
            "silhouette_iou":0.75,
            "boundary_f1_4px":0.34,
            "bbox_edge_error":0.01,
            "centroid_error":0.003,
            "sdf_chamfer_px":15.0
        });
        let aligned = json!([
            {"landmark_id":"chest-center","x":255.0/511.0,"y":255.0/511.0,"visibility":"observed","confidence":1.0}
        ]);
        let misaligned = json!([
            {"landmark_id":"chest-center","x":0.9,"y":0.9,"visibility":"observed","confidence":1.0}
        ]);
        let contour_only = camera_fit_loss(&base);
        let aligned_loss = camera_fit_loss(&transient_loss_metrics_with_parts(
            &base,
            &model,
            Some(&aligned),
            None,
        ));
        let misaligned_loss = camera_fit_loss(&transient_loss_metrics_with_parts(
            &base,
            &model,
            Some(&misaligned),
            None,
        ));
        assert!(aligned_loss < contour_only);
        assert!(misaligned_loss > aligned_loss);
        assert!(misaligned_loss > contour_only);
    }

    #[test]
    fn boundary_projection_prioritizes_attributed_local_part_controls() {
        let rig = json!({"parameters":[
            {"parameter_id":"shin-width","part_id":"shin-pair","semantic":"width","value":1.0,"min":0.82,"max":1.18,"step":0.04,"unit":"ratio"},
            {"parameter_id":"shin-offset-x","part_id":"shin-pair","semantic":"offset_x","value":0.0,"min":-0.35,"max":0.35,"step":0.05,"unit":"meter"},
            {"parameter_id":"chest-width","part_id":"chest-shell","semantic":"width","value":1.0,"min":0.82,"max":1.18,"step":0.04,"unit":"ratio"}
        ]});
        let selected = rig["parameters"].as_array().unwrap().clone();
        let segments = vec![
            json!({"reference":[0.24,0.89],"model":[0.33,0.89],"distance_px":46.0,"part_id":"shin-pair"}),
            json!({"reference":[0.59,0.89],"model":[0.66,0.89],"distance_px":36.0,"part_id":"shin-pair"}),
        ];
        let projected = apply_boundary_part_parameter_projection(
            &rig,
            &selected,
            &segments,
            Some(&default_camera_calibration()),
        );
        assert!(projected[0]["value"].as_f64().unwrap() > 1.0);
        assert!(projected[1]["value"].as_f64().unwrap() < 0.0);
        assert_eq!(projected[2]["value"], 1.0);
    }

    #[test]
    fn boundary_projection_merges_bilateral_worker_part_ids_for_pair_rig() {
        let rig = json!({"parameters":[
            {"parameter_id":"shin-width","part_id":"shin-pair","semantic":"width","value":1.0,"min":0.82,"max":1.5,"step":0.04,"unit":"ratio"},
            {"parameter_id":"shin-offset-x","part_id":"shin-pair","semantic":"offset_x","value":0.0,"min":-0.35,"max":0.35,"step":0.05,"unit":"meter"},
            {"parameter_id":"chest-width","part_id":"chest-shell","semantic":"width","value":1.0,"min":0.82,"max":1.18,"step":0.04,"unit":"ratio"}
        ]});
        let selected = rig["parameters"].as_array().unwrap().clone();
        let segments = vec![
            json!({"reference":[0.20,0.82],"model":[0.30,0.82],"distance_px":48.0,"part_id":"shin-left"}),
            json!({"reference":[0.80,0.82],"model":[0.70,0.82],"distance_px":38.0,"part_id":"shin-right"}),
        ];
        let projected = apply_boundary_part_parameter_projection(
            &rig,
            &selected,
            &segments,
            Some(&default_camera_calibration()),
        );
        assert!(projected[0]["value"].as_f64().unwrap() > 1.0);
        assert!(projected[1]["value"].as_f64().unwrap().abs() < 1e-9);
        assert_eq!(projected[2]["value"], 1.0);
    }

    #[test]
    fn primary_form_ranking_prioritizes_dominant_boundary_part_before_proposal_delta() {
        let rig = json!({"parameters":[
            {"parameter_id":"chest-width","part_id":"chest-shell","semantic":"width","value":1.0,"min":0.8,"max":1.2,"step":0.04,"unit":"ratio"},
            {"parameter_id":"shin-width","part_id":"shin-pair","semantic":"width","value":1.0,"min":0.8,"max":1.2,"step":0.04,"unit":"ratio"},
            {"parameter_id":"shin-offset-x","part_id":"shin-pair","semantic":"offset_x","value":0.0,"min":-0.35,"max":0.35,"step":0.05,"unit":"meter"}
        ]});
        // The chest proposal is larger, but the candidate-bound contour shows
        // that the shin is the dominant visible failure.  The bounded search
        // must spend its first coordinate slots on that Part instead of using
        // proposal magnitude as a proxy for visual priority.
        let selected = vec![
            json!({"parameter_id":"chest-width","part_id":"chest-shell","value":1.18}),
            json!({"parameter_id":"shin-width","part_id":"shin-pair","value":1.04}),
            json!({"parameter_id":"shin-offset-x","part_id":"shin-pair","value":-0.08}),
        ];
        let segments = vec![
            json!({"part_id":"shin-pair","distance_px":48.0}),
            json!({"part_id":"shin-pair","distance_px":38.0}),
            json!({"part_id":"chest-shell","distance_px":12.0}),
        ];
        let ranked = ranked_rig_parameter_indices_with_boundary_context(
            &rig,
            &selected,
            &segments,
        );
        assert_eq!(ranked, vec![2, 1, 0]);
    }

    #[test]
    fn primary_form_scope_locks_one_dominant_part_and_restores_other_proposals() {
        let rig = json!({"parameters":[
            {"parameter_id":"chest-width","part_id":"chest-shell","semantic":"width","value":1.0,"min":0.8,"max":1.2,"step":0.04,"unit":"ratio"},
            {"parameter_id":"shin-width","part_id":"shin-pair","semantic":"width","value":1.0,"min":0.8,"max":1.2,"step":0.04,"unit":"ratio"},
            {"parameter_id":"shin-offset-x","part_id":"shin-pair","semantic":"offset_x","value":0.0,"min":-0.35,"max":0.35,"step":0.05,"unit":"meter"}
        ]});
        let selected = vec![
            json!({"parameter_id":"chest-width","part_id":"chest-shell","value":1.18}),
            json!({"parameter_id":"shin-width","part_id":"shin-pair","value":1.08}),
            json!({"parameter_id":"shin-offset-x","part_id":"shin-pair","value":-0.08}),
        ];
        let segments = vec![
            json!({"part_id":"shin-left","distance_px":48.0}),
            json!({"part_id":"shin-right","distance_px":38.0}),
            json!({"part_id":"chest-shell","distance_px":12.0}),
        ];
        let focused = dominant_boundary_rig_part(&rig, &segments).expect("dominant Part");
        assert_eq!(focused, "shin-pair");
        let scoped = focus_rig_parameters_to_part(&rig, &selected, &focused);
        assert_eq!(scoped[0]["value"], 1.0);
        assert_eq!(scoped[1]["value"], 1.08);
        assert_eq!(scoped[2]["value"], -0.08);
        assert!(rig_part_matches_observed_part("shin-pair", "shin-left"));
        assert!(rig_part_matches_observed_part("shin-pair", "shin-right"));
        assert!(!rig_part_matches_observed_part("chest-shell", "shin-left"));
    }

    #[test]
    fn rig_materialization_traces_mirror_transform_and_profile_sources() {
        let program = json!({
            "schema_version":"GeometryProgram@2",
            "project_id":"project-rig-test",
            "nodes":[
                {"node_id":"shell-left","operator_id":"forgecad.geometry.panel@1","inputs":[],"parameters":{"shape":"panel","size_m":[1.0,2.0,0.4],"thickness_m":0.2,"bevel_m":0.05,"position_m":[-1.0,0.0,0.0],"rotation_rad":[0.0,0.0,0.0]}},
                {"node_id":"shell-shaped","operator_id":"forgecad.geometry.transform@2","inputs":["shell-left"],"parameters":{"shape":"transform","translation_m":[0.0,0.0,0.0],"rotation_rad":[0.0,0.0,0.0],"scale":[1.0,1.0,1.0]}},
                {"node_id":"shell-pair","operator_id":"forgecad.geometry.mirror@1","inputs":["shell-shaped"],"parameters":{"shape":"mirror","axis":"x","offset_m":0.0}}
            ],
            "part_outputs":[{"part_id":"shoulder-pair","input_node_ids":["shell-pair"],"material_zone_id":"zone-white-shell","solid":true}],
            "canonical_sha256":""
        });
        let rig = json!({"parameters":[
            {"parameter_id":"shoulder-width","part_id":"shoulder-pair","semantic":"width","value":1.0,"min":0.5,"max":1.5,"step":0.05,"unit":"ratio"},
            {"parameter_id":"shoulder-height","part_id":"shoulder-pair","semantic":"height","value":1.0,"min":0.5,"max":1.5,"step":0.05,"unit":"ratio"},
            {"parameter_id":"shoulder-offset-x","part_id":"shoulder-pair","semantic":"offset_x","value":0.0,"min":-0.5,"max":0.5,"step":0.05,"unit":"meter"},
            {"parameter_id":"shoulder-offset-y","part_id":"shoulder-pair","semantic":"offset_y","value":0.0,"min":-0.5,"max":0.5,"step":0.05,"unit":"meter"}
        ]});
        let selected = vec![
            json!({"parameter_id":"shoulder-width","part_id":"shoulder-pair","value":1.2}),
            json!({"parameter_id":"shoulder-height","part_id":"shoulder-pair","value":1.1}),
            json!({"parameter_id":"shoulder-offset-x","part_id":"shoulder-pair","value":0.1}),
            json!({"parameter_id":"shoulder-offset-y","part_id":"shoulder-pair","value":0.2}),
        ];
        let (materialized, applied) = materialize_rig_geometry_program(&program, &rig, &selected, None).expect("materialize");
        assert_eq!(applied, 4);
        let source = materialized["nodes"].as_array().unwrap().iter().find(|node| node["node_id"] == "shell-left").unwrap();
        assert_eq!(source["parameters"]["size_m"][0], 1.2);
        assert_eq!(source["parameters"]["size_m"][1], 2.2);
        let transform = materialized["nodes"].as_array().unwrap().iter().find(|node| node["node_id"] == "shell-shaped").unwrap();
        assert_eq!(transform["parameters"]["translation_m"][0], 0.1);
        assert_eq!(transform["parameters"]["translation_m"][1], 0.2);
        assert_eq!(materialized["nodes"][2]["parameters"]["axis"], "x");
    }

    #[test]
    fn camera_plane_offset_materialization_preserves_bounded_distance() {
        let program = json!({
            "schema_version":"GeometryProgram@2",
            "project_id":"project-camera-offset-test",
            "nodes":[
                {"node_id":"shell","operator_id":"forgecad.geometry.primitive@2","inputs":[],"parameters":{"shape":"box","size_m":[1.0,1.0,1.0],"position_m":[0.0,0.0,0.0],"rotation_rad":[0.0,0.0,0.0]}}
            ],
            "part_outputs":[{"part_id":"chest-shell","input_node_ids":["shell"],"material_zone_id":"zone-white-shell","solid":true}],
            "canonical_sha256":""
        });
        let rig = json!({"parameters":[
            {"parameter_id":"chest-offset-x","part_id":"chest-shell","semantic":"offset_x","value":0.0,"min":-0.35,"max":0.35,"step":0.05,"unit":"meter"}
        ]});
        let selected = vec![json!({"parameter_id":"chest-offset-x","part_id":"chest-shell","value":0.2})];
        let (materialized, applied) = materialize_rig_geometry_program(
            &program,
            &rig,
            &selected,
            Some(&default_camera_calibration()),
        )
        .expect("camera-plane materialize");
        assert_eq!(applied, 1);
        let translation = materialized["nodes"][0]["parameters"]["position_m"]
            .as_array()
            .expect("position");
        let distance = translation
            .iter()
            .map(|value| value.as_f64().unwrap().powi(2))
            .sum::<f64>()
            .sqrt();
        assert!((distance - 0.2).abs() < 1e-9);
        assert_ne!(translation[0], 0.2);
    }

    #[test]
    fn rig_materialization_scales_profile_and_operator_specific_dimensions() {
        let program = json!({
            "schema_version":"GeometryProgram@2",
            "project_id":"project-rig-test",
            "nodes":[
                {"node_id":"vent","operator_id":"forgecad.geometry.vent-array@1","inputs":[],"parameters":{"shape":"vent-array","width_m":1.0,"height_m":0.5,"depth_m":0.2,"slot_count":3,"slot_width_m":0.1,"slot_spacing_m":0.1,"position_m":[0.0,0.0,0.0],"rotation_rad":[0.0,0.0,0.0]}},
                {"node_id":"profile","operator_id":"forgecad.geometry.profile-extrude@1","inputs":[],"parameters":{"shape":"profile-extrude","profile":[[-1.0,-0.5],[1.0,-0.5],[0.0,0.75]],"depth_m":0.4,"position_m":[0.0,0.0,0.0],"rotation_rad":[0.0,0.0,0.0]}}
            ],
            "part_outputs":[
                {"part_id":"vent","input_node_ids":["vent"],"material_zone_id":"zone-black-mechanical","solid":true},
                {"part_id":"profile","input_node_ids":["profile"],"material_zone_id":"zone-white-shell","solid":true}
            ],
            "canonical_sha256":""
        });
        let rig = json!({"parameters":[
            {"parameter_id":"vent-width","part_id":"vent","semantic":"width","value":1.0,"min":0.5,"max":1.5,"step":0.05,"unit":"ratio"},
            {"parameter_id":"profile-height","part_id":"profile","semantic":"height","value":1.0,"min":0.5,"max":1.5,"step":0.05,"unit":"ratio"}
        ]});
        let selected = vec![
            json!({"parameter_id":"vent-width","part_id":"vent","value":1.1}),
            json!({"parameter_id":"profile-height","part_id":"profile","value":0.8}),
        ];
        let (materialized, applied) = materialize_rig_geometry_program(&program, &rig, &selected, None).expect("materialize");
        assert_eq!(applied, 2);
        assert_eq!(materialized["nodes"][0]["parameters"]["width_m"], 1.1);
        let profile = materialized["nodes"][1]["parameters"]["profile"].as_array().unwrap();
        let y0 = profile[0][1].as_f64().unwrap();
        let y2 = profile[2][1].as_f64().unwrap();
        assert!((y0 + 0.4).abs() < 1e-9);
        assert!((y2 - 0.6).abs() < 1e-9);
    }

    #[test]
    fn rig_materialization_maps_bilateral_parameter_to_explicit_primitive_sides() {
        let program = json!({
            "schema_version":"GeometryProgram@2",
            "project_id":"project-rig-alias",
            "nodes":[
                {"node_id":"shoulder-left","operator_id":"forgecad.geometry.primitive@2","inputs":[],"parameters":{"shape":"sphere","radius_m":0.3,"longitude_segments":12,"latitude_segments":8,"position_m":[-0.8,2.0,0.0],"rotation_rad":[0.0,0.0,0.0]}},
                {"node_id":"shoulder-right","operator_id":"forgecad.geometry.primitive@2","inputs":[],"parameters":{"shape":"sphere","radius_m":0.3,"longitude_segments":12,"latitude_segments":8,"position_m":[0.8,2.0,0.0],"rotation_rad":[0.0,0.0,0.0]}}
            ],
            "part_outputs":[
                {"part_id":"shoulder-left","input_node_ids":["shoulder-left"],"material_zone_id":"zone-white-shell","solid":true},
                {"part_id":"shoulder-right","input_node_ids":["shoulder-right"],"material_zone_id":"zone-white-shell","solid":true}
            ],
            "canonical_sha256":""
        });
        let rig = json!({"parameters":[
            {"parameter_id":"shoulder-width","part_id":"shoulder-armor-pair","semantic":"width","value":1.0,"min":0.5,"max":1.5,"step":0.05,"unit":"ratio"}
        ]});
        let selected = vec![json!({"parameter_id":"shoulder-width","part_id":"shoulder-armor-pair","value":1.2})];
        let (materialized, applied) = materialize_rig_geometry_program(&program, &rig, &selected, None).expect("materialize");
        assert_eq!(applied, 1);
        assert_eq!(materialized["nodes"][0]["parameters"]["radius_m"], 0.36);
        assert_eq!(materialized["nodes"][1]["parameters"]["radius_m"], 0.36);
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
    fn c_visual_evidence_export_and_restart_keep_hashes_stable() {
        let root = std::env::temp_dir().join(format!("forgecad-mcp010c-restart-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).expect("root");
        let database = root.join("runtime.sqlite");
        let cas = root.join("cas");
        let (candidate_id, version_id, artifact_sha256, reference_id, reference_sha256, render_set_sha256, comparison_sha256, quality_sha256, export_request, export_output_sha256) = {
            let runtime = Runtime::open_with_cas(&database, &cas).expect("runtime");
            let project = runtime
                .create_project("MCP010C restart evidence", json!({"profile":"mvp"}))
                .expect("project");
            let mut program = json!({
                "schema_version":"GeometryProgram@2",
                "project_id":project.project_id,
                "representation_plan_sha256":"f".repeat(64),
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
            let candidate = prepared["candidate"].clone();
            let candidate_id = candidate["candidate_id"].as_str().unwrap().to_owned();
            let artifact_sha256 = candidate["prepared_object_sha256"].as_str().unwrap().to_owned();
            let confirmed = runtime
                .confirm_candidate(&CandidateConfirmRequest {
                    project_id: project.project_id.clone(),
                    candidate_id: candidate_id.clone(),
                    base_version_id: None,
                    prepared_object_id: candidate["prepared_object_id"].as_str().unwrap().to_owned(),
                    prepared_object_sha256: artifact_sha256.clone(),
                    quality_report_id: candidate["quality_report_id"].as_str().unwrap().to_owned(),
                    approval_receipt_id: "mcp010c-restart-confirm".to_owned(),
                    approval_summary: "Confirm structural restart fixture".to_owned(),
                    approval_session_id: "mcp010c-restart-session".to_owned(),
                    approval_expires_at: "9999999999".to_owned(),
                    idempotency_key: "mcp010c-restart-confirm-once".to_owned(),
                })
                .expect("confirm structural fixture");
            let reference = runtime
                .import_reference(&ReferenceImportRequest {
                    project_id: project.project_id.clone(),
                    source: ReferenceImportSource::InlineContent {
                        mime: "image/png".to_owned(),
                        content_base64: "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=".to_owned(),
                    },
                    authorization: ReferenceAuthorization {
                        user_authorized: true,
                        declaration: "MCP010C restart hash fixture reference".to_owned(),
                    },
                    expected_sha256: None,
                })
                .expect("reference import")
                .reference;
            let mut view_spec = json!({
                "schema_version":"ReferenceViewSpec@1",
                "reference_id":reference.reference_id,
                "reference_sha256":reference.object_sha256,
                "view_id":"restart-hash-fixture",
                "source_view":"three-quarter",
                "image":{"width":1,"height":1,"rotation_degrees":0.0,"crop":{"x":0.0,"y":0.0,"width":1.0,"height":1.0}},
                "landmarks":[],
                "regions":[],
                "canonical_sha256":""
            });
            view_spec["canonical_sha256"] = Value::String(canonical_json_hash(&view_spec));
            let visual = runtime
                .prepare_reference_comparison(
                    &project.project_id,
                    json!({"candidate_id":candidate_id,"reference_id":reference.reference_id,"view_spec":view_spec}),
                )
                .expect("reference comparison");
            let target = runtime
                .prepare_reference_mask(
                    &project.project_id,
                    json!({
                        "project_id":project.project_id.clone(),
                        "reference_id":reference.reference_id.clone(),
                        "contour_points":[[0.1,0.1],[0.9,0.1],[0.9,0.9],[0.1,0.9]]
                    }),
                )
                .expect("silhouette target");
            let boundary = runtime
                .boundary_error(
                    &candidate_id,
                    target["target_sha256"].as_str().unwrap(),
                    Some(8),
                )
                .expect("directional boundary error");
            validate_boundary_error_result(&boundary).expect("boundary contract");
            assert_eq!(boundary["candidate_id"], candidate_id);
            assert!(boundary["segments"].as_array().unwrap().len() <= 8);
            let render_set_sha256 = visual["render_set_object_sha256"].as_str().unwrap().to_owned();
            let comparison_sha256 = visual["comparison_report_object_sha256"].as_str().unwrap().to_owned();
            let quality_sha256 = visual["quality_report_object_sha256"].as_str().unwrap().to_owned();
            let export = runtime
                .prepare_export(&ExportPrepareRequest {
                    project_id: project.project_id.clone(),
                    version_id: confirmed.version_id.clone(),
                    format: "glb".to_owned(),
                    profile: "mvp-glb".to_owned(),
                    request: json!({"reason":"MCP010C restart hash fixture"}),
                })
                .expect("export prepare");
            let export_request = ExportConfirmRequest {
                project_id: project.project_id.clone(),
                export_id: export.manifest.export_id.clone(),
                version_id: confirmed.version_id.clone(),
                format: "glb".to_owned(),
                profile: "mvp-glb".to_owned(),
                approval_receipt_id: "mcp010c-restart-export".to_owned(),
                approval_summary: "Approve structural restart export".to_owned(),
                approval_session_id: "mcp010c-restart-session".to_owned(),
                approval_expires_at: "9999999999".to_owned(),
                idempotency_key: "mcp010c-restart-export-once".to_owned(),
            };
            let exported = runtime
                .confirm_export(&export_request)
                .expect("export confirm");
            (
                candidate_id,
                confirmed.version_id,
                artifact_sha256,
                reference.reference_id,
                reference.object_sha256,
                render_set_sha256,
                comparison_sha256,
                quality_sha256,
                export_request,
                exported.output_sha256,
            )
        };

        let reopened = Runtime::open_with_cas(&database, &cas).expect("reopen");
        let candidate = reopened
            .candidate(&candidate_id)
            .expect("candidate query")
            .expect("candidate after restart");
        assert_eq!(candidate.prepared_object_sha256.as_deref(), Some(artifact_sha256.as_str()));
        let version = reopened
            .version(&version_id)
            .expect("version query")
            .expect("version after restart");
        assert_eq!(version.manifest_hash, artifact_sha256);
        let evidence = reopened
            .visual_evidence(&candidate_id)
            .expect("visual evidence after restart");
        assert_eq!(evidence["reference_id"], reference_id);
        assert_eq!(evidence["render_set_hash"], render_set_sha256);
        assert_eq!(evidence["comparison_report_hash"], comparison_sha256);
        assert_eq!(evidence["quality_report_hash"], quality_sha256);
        let quality = reopened
            .quality(&candidate_id, Some(&reference_id))
            .expect("quality after restart");
        assert_eq!(quality["artifact_sha256"], artifact_sha256);
        assert_eq!(quality["render_set_hash"], render_set_sha256);
        assert_eq!(quality["comparison_report_hash"], comparison_sha256);
        assert_eq!(reopened.reference(&reference_id).unwrap().unwrap().object_sha256, reference_sha256);
        let pass = reopened
            .render_pass_get(&render_set_sha256, "beauty")
            .expect("render pass after restart");
        assert_eq!(pass["render_set_hash"], render_set_sha256);
        let replay = reopened
            .confirm_export(&export_request)
            .expect("export replay after restart");
        assert!(replay.replayed);
        assert_eq!(replay.output_sha256, export_output_sha256);
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

    #[test]
    fn bounded_agentic_action_run_executes_primary_form_and_round_trips_immutably() {
        let runtime = Runtime::ephemeral().expect("runtime");
        let project = runtime
            .create_project("MCP010F bounded action run", json!({"profile":"mvp"}))
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
                    declaration: "MCP010F bounded action fixture".to_owned(),
                },
                expected_sha256: None,
            })
            .expect("reference")
            .reference;
        let program = v2_restore_program(&project.project_id);
        let prepared = runtime
            .prepare_geometry_candidate(
                &project.project_id,
                None,
                json!({"typed":"geometry","geometry_program":program}),
            )
            .expect("candidate");
        let candidate_id = prepared["candidate"]["candidate_id"]
            .as_str()
            .expect("candidate id")
            .to_owned();
        let target = runtime
            .prepare_reference_mask(
                &project.project_id,
                json!({
                    "project_id":project.project_id.clone(),
                    "reference_id":reference.reference_id.clone(),
                    "contour_points":[[0.1,0.1],[0.9,0.1],[0.9,0.9],[0.1,0.9]],
                    "parts":[{"part_id":"shell","start_index":0,"end_index":3,"visibility":"observed"}]
                }),
            )
            .expect("target");
        let mut view_spec = json!({
            "schema_version":"ReferenceViewSpec@1",
            "reference_id":reference.reference_id.clone(),
            "reference_sha256":reference.object_sha256.clone(),
            "view_id":"bounded-action-view",
            "source_view":"three-quarter",
            "image":{"width":1,"height":1,"rotation_degrees":0.0,"crop":{"x":0.0,"y":0.0,"width":1.0,"height":1.0}},
            "landmarks":[],
            "regions":[],
            "canonical_sha256":""
        });
        view_spec["canonical_sha256"] = Value::String(canonical_json_hash(&view_spec));
        runtime
            .prepare_reference_comparison(
                &project.project_id,
                json!({
                    "project_id":project.project_id.clone(),
                    "candidate_id":candidate_id.clone(),
                    "reference_id":reference.reference_id.clone(),
                    "view_spec":view_spec,
                    "target_sha256":target["target_sha256"].clone()
                }),
            )
            .expect("comparison");
        let visual = runtime.visual_evidence(&candidate_id).expect("visual evidence");
        let camera_hash = visual["render_set"]["camera_hash"]
            .as_str()
            .expect("camera hash")
            .to_owned();
        let evidence_sha256 = visual["quality_report_hash"]
            .as_str()
            .expect("quality evidence hash")
            .to_owned();
        let session_id = "design-session-bounded-action";
        let session = runtime
            .session_create_or_resume(json!({
                "session_id":session_id,
                "project_id":project.project_id.clone(),
                "candidate_id":candidate_id.clone(),
                "idempotency_key":"bounded-action-session",
                "reference_id":reference.reference_id.clone(),
                "design_spec_id":"design-spec-bounded-action",
                "reference_canvas_id":"reference-canvas-bounded-action",
                "camera_hash":camera_hash,
                "evidence_sha256":evidence_sha256,
                "approved":true,
                "approval_receipt_id":"bounded-action-session-approval",
                "approval_summary":"Approve bounded action session"
            }))
            .expect("session");
        assert_eq!(session["session"]["current_stage"], "primary-form");
        let action = json!({
            "action_id":"bounded-primary-form-adjustment",
            "action_kind":"bounded-repair",
            "scope_kind":"part",
            "target_id":"shell",
            "operator_id":"forgecad.geometry.transform@2",
            "parameter_changes":[{"parameter_id":"shell-width","before":1.0,"after":1.05,"minimum":0.5,"maximum":1.5,"unit":"meter"}],
            "bounded":true,
            "description":"Adjust one bounded Primary Form width parameter"
        });
        let run_id = "action-run-bounded-primary-form";
        let input_sha256 = canonical_json_hash(&json!({
            "project_id":project.project_id.clone(),
            "session_id":session_id,
            "candidate_id":candidate_id.clone(),
            "run_id":run_id,
            "action":action,
            "requested_stage":"primary-form"
        }));
        let before_versions = runtime.versions(Some(&project.project_id)).expect("versions").len();
        let request = json!({
            "project_id":project.project_id.clone(),
            "session_id":session_id,
            "candidate_id":candidate_id.clone(),
            "run_id":run_id,
            "action":action,
            "input_sha256":input_sha256,
            "requested_stage":"primary-form",
            "approved":true,
            "approval_receipt_id":"bounded-action-run-approval",
            "approval_summary":"Approve one bounded Primary Form repair",
            "approval_session_id":session_id,
            "idempotency_key":"bounded-action-run-once"
        });
        let first = runtime
            .design_action_run_prepare(request.clone())
            .expect("action run");
        assert_eq!(first["schema_version"], "DesignActionRun@1");
        assert!(matches!(first["status"].as_str(), Some("completed" | "blocked")));
        assert_eq!(first["runtime_write"], false);
        assert_eq!(first["persistent_user_data_touched"], false);
        assert!(first["locked_actions"].as_array().unwrap().iter().any(|value| value == "confirm"));
        assert!(first["locked_actions"].as_array().unwrap().iter().any(|value| value == "export"));
        let second = runtime
            .design_action_run_prepare(request)
            .expect("idempotent action run");
        assert_eq!(first, second);
        let read = runtime
            .design_action_run_get(json!({
                "project_id":project.project_id,
                "session_id":session_id,
                "candidate_id":candidate_id,
                "run_id":run_id
            }))
            .expect("action readback");
        assert_eq!(read, first);
        assert_eq!(runtime.versions(Some(session["project_id"].as_str().unwrap())).unwrap().len(), before_versions);
    }
}
