//! Runtime-owned, read-only projections for the first Agentic Design Runtime slice.
//!
//! These projections are deliberately derived on demand from the existing Runtime
//! read model.  They do not create checkpoints, candidates, versions, CAS objects,
//! or database rows.  Every conclusion is either bound to an existing evidence
//! hash or explicitly labelled as inferred/unknown.

use super::{canonical_json_hash, Runtime, RuntimeError};
use forgecad_contracts::{
    is_sha256, CandidateRecord, GeometryCandidateEvidenceRecord, ProjectRecord,
    ReferenceEvidenceRecord, SnapshotRecord,
};
use image::GenericImageView;
use serde_json::{json, Value};
use std::collections::BTreeMap;

const AGENTIC_PROJECTION_STATUS: &str = "projection/read-only";
const STAGES: [&str; 6] = [
    "reference-canvas",
    "primary-form",
    "secondary-structure",
    "tertiary-detail",
    "uv-pbr",
    "final-review",
];

#[derive(Debug, Clone)]
struct GeometryContext {
    evidence: Option<GeometryCandidateEvidenceRecord>,
    program: Option<Value>,
    artifact: Option<Value>,
}

#[derive(Debug, Clone)]
struct VisualContext {
    bundle: Value,
    reference_id: Option<String>,
    quality_report: Option<Value>,
    quality_report_hash: Option<String>,
    comparison_status: String,
    part_error: Option<Value>,
    surface_readback: Value,
}

#[derive(Debug, Clone)]
struct QualityContext {
    projection: Value,
    report_hash: Option<String>,
    report_id: Option<String>,
    structural_status: String,
    visual_status: String,
    strict_visual_gate: String,
}

#[derive(Debug, Clone)]
struct ProjectionContext {
    project: ProjectRecord,
    snapshot: Option<SnapshotRecord>,
    candidate: Option<CandidateRecord>,
    candidate_selection: &'static str,
    geometry: GeometryContext,
    visual: VisualContext,
    quality: QualityContext,
    reference_canvas: Value,
    lineage: Value,
    projection_key: String,
}

impl Runtime {
    /// Build the complete Agentic observation surface from Runtime-owned data.
    /// This method never persists the returned DesignSession or any child object.
    pub fn agentic_scene_observe(
        &self,
        project_id: &str,
        candidate_id: Option<&str>,
    ) -> Result<Value, RuntimeError> {
        let context = build_context(self, project_id, candidate_id)?;
        let scene_graph = build_scene_graph(&context);
        let model_bundle = build_model_understanding_bundle(&context, &scene_graph);
        let stage_plan = build_stage_plan(&context);
        let critic = build_critic_report(&context, &stage_plan);
        let session = build_design_session(&context, &scene_graph, &stage_plan, &critic);

        Ok(canonicalize(json!({
            "schema_version":"AgenticSceneObserveResult@1",
            "projection_status":AGENTIC_PROJECTION_STATUS,
            "read_only":true,
            "project_id":context.project.project_id,
            "candidate_id":context.candidate.as_ref().map(|candidate| candidate.candidate_id.clone()),
            "candidate_selection":context.candidate_selection,
            "project":context.project,
            "snapshot":context.snapshot,
            "candidate":context.candidate,
            "semantic_scene_graph":scene_graph,
            "model_understanding_bundle":model_bundle,
            "reference_canvas":context.reference_canvas,
            "visual_evidence_bundle":context.visual.bundle,
            "quality":context.quality.projection,
            "design_session":session,
            "design_stage_plan":stage_plan,
            "design_critic_report":critic,
            "lineage":context.lineage,
            "canonical_sha256":""
        })))
    }

    /// Return only the derived stage plan.  Stage advancement and unlocks are
    /// fail-closed on the same strict visual evidence used by scene observe.
    pub fn agentic_stage_plan(
        &self,
        project_id: &str,
        candidate_id: Option<&str>,
    ) -> Result<Value, RuntimeError> {
        let context = build_context(self, project_id, candidate_id)?;
        Ok(build_stage_plan(&context))
    }

    /// Return the optional evidence-bound critic projection without executing a
    /// repair.  RepairIntent is a bounded suggestion, never a write command.
    pub fn agentic_critic_projection(
        &self,
        project_id: &str,
        candidate_id: Option<&str>,
        target_sha256: Option<&str>,
    ) -> Result<Value, RuntimeError> {
        let mut context = build_context(self, project_id, candidate_id)?;
        if let Some(target_sha256) = target_sha256 {
            let candidate = context.candidate.as_ref().ok_or_else(|| {
                RuntimeError::InvalidInput(
                    "AGENTIC_LOCAL_PART_ERROR_UNAVAILABLE: candidate is required".to_owned(),
                )
            })?;
            let part_error = self.silhouette_part_error(
                project_id,
                json!({
                    "project_id":project_id,
                    "candidate_id":candidate.candidate_id,
                    "target_sha256":target_sha256
                }),
            )?;
            context.visual.part_error = Some(part_error);
        }
        let stage_plan = build_stage_plan(&context);
        Ok(build_critic_report(&context, &stage_plan))
    }

    /// Return the same Runtime-owned VisualEvidenceBundle used by
    /// `scene_observe_get`, without exposing the legacy ViewerVisualEvidence
    /// shape. When a durable multi-view ReferenceCanvas exists, the bundle
    /// includes its per-view evidence inventory; only a reference match with
    /// an existing candidate-bound render receives hashes, and the remaining
    /// views stay explicitly `not-run`.
    pub fn visual_evidence_bundle_get(
        &self,
        project_id: &str,
        candidate_id: Option<&str>,
    ) -> Result<Value, RuntimeError> {
        let context = build_context(self, project_id, candidate_id)?;
        Ok(context.visual.bundle)
    }

    /// Return a candidate-bound visual-surface diagnostic projection.  Fixed
    /// AOV/boundary signals and a bounded mesh-derived curvature/feature-line
    /// summary are read back from the same candidate GLB.  The summary is an
    /// evidence adapter, not a SubD/NURBS surface executor or a quality gate.
    pub fn visual_surface_get(&self, request: Value) -> Result<Value, RuntimeError> {
        let request = parse_visual_surface_request(&request)?;
        let mut context = build_context(self, &request.project_id, Some(&request.candidate_id))?;
        if let Some(target_sha256) = request.target_sha256.as_deref() {
            let part_error = self.silhouette_part_error(
                &request.project_id,
                json!({
                    "project_id":request.project_id,
                    "candidate_id":request.candidate_id,
                    "target_sha256":target_sha256
                }),
            )?;
            context.visual.part_error = Some(part_error);
        }
        let result = build_visual_surface_result(&context, &request)?;
        validate_visual_surface_result(&result)?;
        Ok(result)
    }
}

const VISUAL_SURFACE_SIGNAL_NAMES: [&str; 8] = [
    "silhouette",
    "boundary",
    "depth",
    "normal",
    "part-id",
    "material-id",
    "curvature",
    "feature-line",
];

const VISUAL_SURFACE_BINDING_KEYS: [&str; 7] = [
    "reference_id",
    "reference_sha256",
    "artifact_sha256",
    "render_set_hash",
    "camera_hash",
    "comparison_report_hash",
    "quality_report_hash",
];

#[derive(Debug, Clone)]
struct VisualSurfaceRequestInput {
    project_id: String,
    candidate_id: String,
    requested_signals: Vec<String>,
    expected_binding: Value,
    target_sha256: Option<String>,
    max_part_errors: usize,
}

fn parse_visual_surface_request(value: &Value) -> Result<VisualSurfaceRequestInput, RuntimeError> {
    let object = value
        .as_object()
        .ok_or_else(|| visual_surface_request_error("request must be an object"))?;
    let expected_keys = [
        "schema_version",
        "project_id",
        "candidate_id",
        "requested_signals",
        "expected_binding",
        "target_sha256",
        "max_part_errors",
        "canonical_sha256",
    ];
    if object.len() != expected_keys.len()
        || expected_keys.iter().any(|key| !object.contains_key(*key))
        || object
            .keys()
            .any(|key| !expected_keys.contains(&key.as_str()))
    {
        return Err(visual_surface_request_error(
            "request has an unexpected field set",
        ));
    }
    if object.get("schema_version").and_then(Value::as_str) != Some("VisualSurfaceRequest@1") {
        return Err(visual_surface_request_error(
            "schema_version must be VisualSurfaceRequest@1",
        ));
    }
    let project_id = required_visual_surface_id(object, "project_id")?;
    let candidate_id = required_visual_surface_id(object, "candidate_id")?;
    let requested_signals = object
        .get("requested_signals")
        .and_then(Value::as_array)
        .ok_or_else(|| visual_surface_request_error("requested_signals must be an array"))?;
    if requested_signals.is_empty() || requested_signals.len() > 8 {
        return Err(visual_surface_request_error(
            "requested_signals must contain one to eight signals",
        ));
    }
    let mut signals = Vec::with_capacity(requested_signals.len());
    for signal in requested_signals {
        let signal = signal.as_str().ok_or_else(|| {
            visual_surface_request_error("requested_signals contains a non-string")
        })?;
        if !VISUAL_SURFACE_SIGNAL_NAMES.contains(&signal) {
            return Err(visual_surface_request_error(
                "requested_signals contains an unsupported signal name",
            ));
        }
        if signals.iter().any(|existing| existing == signal) {
            return Err(visual_surface_request_error(
                "requested_signals must not contain duplicates",
            ));
        }
        signals.push(signal.to_owned());
    }
    let expected_binding = object
        .get("expected_binding")
        .ok_or_else(|| visual_surface_request_error("expected_binding is missing"))?;
    validate_visual_surface_binding(expected_binding, "VisualSurfaceRequest@1.expected_binding")?;
    let target_sha256 = match object.get("target_sha256") {
        Some(Value::Null) => None,
        Some(Value::String(value)) if is_sha256(value) => Some(value.to_owned()),
        _ => {
            return Err(visual_surface_request_error(
                "target_sha256 must be null or a lowercase SHA-256",
            ))
        }
    };
    let max_part_errors = object
        .get("max_part_errors")
        .and_then(Value::as_u64)
        .filter(|value| (1..=64).contains(value))
        .map(|value| value as usize)
        .ok_or_else(|| visual_surface_request_error("max_part_errors must be between 1 and 64"))?;
    let declared_hash = object
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .filter(|hash| is_sha256(hash))
        .ok_or_else(|| visual_surface_request_error("canonical_sha256 is invalid"))?;
    let mut canonical_input = value.clone();
    canonical_input["canonical_sha256"] = Value::String(String::new());
    if canonical_json_hash(&canonical_input) != declared_hash {
        return Err(visual_surface_request_error(
            "canonical_sha256 does not bind the request",
        ));
    }
    Ok(VisualSurfaceRequestInput {
        project_id,
        candidate_id,
        requested_signals: signals,
        expected_binding: expected_binding.clone(),
        target_sha256,
        max_part_errors,
    })
}

fn required_visual_surface_id(
    object: &serde_json::Map<String, Value>,
    key: &str,
) -> Result<String, RuntimeError> {
    let value = object
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| forgecad_contracts::is_opaque_id(value))
        .ok_or_else(|| visual_surface_request_error(&format!("{key} is not an identifier")))?;
    Ok(value.to_owned())
}

fn validate_visual_surface_binding(value: &Value, context: &str) -> Result<(), RuntimeError> {
    let object = value
        .as_object()
        .ok_or_else(|| visual_surface_request_error(&format!("{context} must be an object")))?;
    if object.len() != VISUAL_SURFACE_BINDING_KEYS.len()
        || VISUAL_SURFACE_BINDING_KEYS
            .iter()
            .any(|key| !object.contains_key(*key))
        || object
            .keys()
            .any(|key| !VISUAL_SURFACE_BINDING_KEYS.contains(&key.as_str()))
    {
        return Err(visual_surface_request_error(&format!(
            "{context} has an unexpected field set"
        )));
    }
    for key in VISUAL_SURFACE_BINDING_KEYS {
        let value = object
            .get(key)
            .ok_or_else(|| visual_surface_request_error(&format!("{context}.{key} is missing")))?;
        if value.is_null() {
            continue;
        }
        if key == "reference_id" {
            if !value.as_str().is_some_and(forgecad_contracts::is_opaque_id) {
                return Err(visual_surface_request_error(&format!(
                    "{context}.{key} is not an identifier"
                )));
            }
        } else if !value.as_str().is_some_and(is_sha256) {
            return Err(visual_surface_request_error(&format!(
                "{context}.{key} is not a lowercase SHA-256"
            )));
        }
    }
    Ok(())
}

fn visual_surface_request_error(detail: &str) -> RuntimeError {
    RuntimeError::InvalidInput(format!("VISUAL_SURFACE_REQUEST_INVALID: {detail}"))
}

fn build_visual_surface_result(
    context: &ProjectionContext,
    request: &VisualSurfaceRequestInput,
) -> Result<Value, RuntimeError> {
    let binding = visual_surface_binding(&context.visual.bundle);
    let expected_binding = request
        .expected_binding
        .as_object()
        .expect("visual surface request binding was validated");
    for key in VISUAL_SURFACE_BINDING_KEYS {
        let expected = expected_binding
            .get(key)
            .expect("visual surface request binding key was validated");
        if !expected.is_null() && binding.get(key) != Some(expected) {
            return Err(binding_error(&format!(
                "visual surface {key} does not match the candidate-bound evidence"
            )));
        }
    }
    let comparison = context
        .visual
        .bundle
        .get("comparison_report")
        .filter(|value| value.is_object());
    let readback = context.visual.surface_readback.clone();
    let aov_passes = readback
        .pointer("/aov/passes")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let edge_ready = readback.pointer("/edge/status").and_then(Value::as_str) == Some("ready");
    let readback_ready = readback.get("status").and_then(Value::as_str) == Some("ready");
    let surface_ready =
        readback.pointer("/surface/status").and_then(Value::as_str) == Some("ready");
    let mut available_signals = Vec::new();
    let mut unsupported_signals = Vec::new();
    let mut unknowns = Vec::new();
    for signal in &request.requested_signals {
        let available = match signal.as_str() {
            "boundary" => edge_ready,
            "curvature" | "feature-line" => surface_ready,
            signal => aov_passes.iter().any(|pass| {
                pass.get("pass").and_then(Value::as_str) == Some(signal)
                    && pass.get("status").and_then(Value::as_str) == Some("decoded")
            }),
        };
        if available {
            available_signals.push(signal.clone());
        } else if matches!(signal.as_str(), "curvature" | "feature-line") {
            unsupported_signals.push(signal.clone());
            unknowns.push(format!("unsupported-{signal}"));
        } else {
            unknowns.push(format!("unavailable-{signal}"));
        }
    }
    let complete_binding = VISUAL_SURFACE_BINDING_KEYS
        .iter()
        .all(|key| !binding.get(*key).is_none_or(Value::is_null));
    let status = if complete_binding
        && readback_ready
        && unsupported_signals.is_empty()
        && unknowns.is_empty()
    {
        "ready"
    } else if context.candidate.is_some() {
        "blocked"
    } else {
        "not-run"
    };
    if !complete_binding {
        for key in VISUAL_SURFACE_BINDING_KEYS {
            if binding.get(key).is_none_or(Value::is_null) {
                unknowns.push(format!("binding-{key}"));
            }
        }
    }
    if context
        .visual
        .bundle
        .get("available")
        .and_then(Value::as_bool)
        != Some(true)
    {
        unknowns.push("candidate-bound-visual-evidence".to_owned());
    }
    if !surface_ready {
        unknowns.push("surface-program-not-run".to_owned());
    }
    unknowns.sort();
    unknowns.dedup();
    let metrics = visual_surface_metrics(comparison);
    let part_errors =
        visual_surface_part_errors(context.visual.part_error.as_ref(), request.max_part_errors);
    let result = canonicalize(json!({
        "schema_version":"VisualSurfaceResult@1",
        "projection_status":AGENTIC_PROJECTION_STATUS,
        "read_only":true,
        "project_id":request.project_id,
        "candidate_id":request.candidate_id,
        "target_sha256":request.target_sha256,
        "status":status,
        "backend":if surface_ready {"candidate-bound-surface-analysis@1"} else {"candidate-bound-aov-diagnostics@1"},
        "surface_program_status":if surface_ready {"ready"} else {"not-run"},
        "requested_signals":request.requested_signals,
        "available_signals":available_signals,
        "unsupported_signals":unsupported_signals,
        "binding":binding.clone(),
        "metrics":metrics,
        "part_errors":part_errors,
        "readback":readback,
        "unknowns":unknowns,
        "lineage":{
            "project_id":request.project_id,
            "candidate_id":request.candidate_id,
            "target_sha256":request.target_sha256,
            "reference_id":binding["reference_id"],
            "reference_sha256":binding["reference_sha256"],
            "artifact_sha256":binding["artifact_sha256"],
            "render_set_hash":binding["render_set_hash"],
            "camera_hash":binding["camera_hash"],
            "comparison_report_hash":binding["comparison_report_hash"],
            "quality_report_hash":binding["quality_report_hash"]
        },
        "canonical_sha256":""
    }));
    Ok(result)
}

const VISUAL_SURFACE_AOV_PASSES: [&str; 9] = [
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

fn build_visual_surface_readback(
    runtime: &Runtime,
    context: &ProjectionContext,
) -> Result<Value, RuntimeError> {
    let Some(render_set) = context
        .visual
        .bundle
        .get("render_set")
        .filter(|value| value.is_object())
    else {
        return Ok(empty_visual_surface_readback());
    };
    let pass_artifacts = render_set
        .get("pass_artifacts")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let mut aov_rows = Vec::new();
    let mut missing_passes = Vec::new();
    for pass in VISUAL_SURFACE_AOV_PASSES {
        let Some(hash) = pass_artifacts
            .get(pass)
            .and_then(|value| value.get("sha256"))
            .and_then(Value::as_str)
            .filter(|hash| is_sha256(hash))
        else {
            missing_passes.push(pass.to_owned());
            continue;
        };
        let Ok(bytes) = runtime.cas_read(hash) else {
            missing_passes.push(pass.to_owned());
            continue;
        };
        let Ok(decoded) = image::load_from_memory(&bytes) else {
            missing_passes.push(pass.to_owned());
            continue;
        };
        let (width, height) = decoded.dimensions();
        if width != 512 || height != 512 {
            missing_passes.push(pass.to_owned());
            continue;
        }
        let rgba = decoded.to_rgba8();
        let mut sums = [0u64; 4];
        let mut nonzero_pixel_count = 0u64;
        for pixel in rgba.pixels() {
            for (index, channel) in pixel.0.iter().enumerate() {
                sums[index] += u64::from(*channel);
            }
            if pixel.0[..3].iter().any(|channel| *channel > 0) {
                nonzero_pixel_count += 1;
            }
        }
        aov_rows.push(json!({
            "pass":pass,
            "sha256":hash,
            "status":"decoded",
            "pixel_count":262144,
            "nonzero_pixel_count":nonzero_pixel_count,
            "mean_rgba":sums.map(|sum| (sum / 262144) as u8)
        }));
    }
    let aov_status = if aov_rows.len() == VISUAL_SURFACE_AOV_PASSES.len() {
        "ready"
    } else if aov_rows.is_empty() {
        "not-run"
    } else {
        "partial"
    };
    let aov = json!({
        "status":aov_status,
        "source":"RenderSet@2/pass_artifacts",
        "passes":aov_rows,
        "missing_passes":missing_passes
    });

    let candidate_mask_sha256 = pass_artifacts
        .get("silhouette")
        .and_then(|value| value.get("sha256"))
        .and_then(Value::as_str)
        .filter(|hash| is_sha256(hash))
        .map(str::to_owned);
    let reference_mask_sha256 = context
        .visual
        .bundle
        .pointer("/comparison_report/mask/sha256")
        .and_then(Value::as_str)
        .filter(|hash| is_sha256(hash))
        .map(str::to_owned);
    let reference_mask = read_visual_surface_mask_stats(runtime, reference_mask_sha256.as_deref());
    let candidate_mask = read_visual_surface_mask_stats(runtime, candidate_mask_sha256.as_deref());
    let reference_mask_values = reference_mask.1.as_deref();
    let candidate_mask_values = candidate_mask.1.as_deref();
    let edge = build_visual_surface_edge_readback(reference_mask_values, candidate_mask_values);

    let part_id_sha256 = pass_artifacts
        .get("part-id")
        .and_then(|value| value.get("sha256"))
        .and_then(Value::as_str)
        .filter(|hash| is_sha256(hash))
        .map(str::to_owned);
    let material_id_sha256 = pass_artifacts
        .get("material-id")
        .and_then(|value| value.get("sha256"))
        .and_then(Value::as_str)
        .filter(|hash| is_sha256(hash))
        .map(str::to_owned);
    let part_ids = context
        .geometry
        .artifact
        .as_ref()
        .and_then(|artifact| artifact.get("part_ids"))
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let parts = part_id_sha256
        .as_deref()
        .and_then(|hash| runtime.cas_read(hash).ok())
        .and_then(|bytes| visual_surface_palette_rows(&bytes, &part_ids));
    let regions =
        visual_surface_reference_regions(context, reference_mask_values, candidate_mask_values);
    let mut roi_unknowns = Vec::new();
    let roi_status = if parts.is_some() {
        if regions.is_empty() {
            roi_unknowns.push("reference-regions-not-bound".to_owned());
            "partial"
        } else {
            "ready"
        }
    } else {
        roi_unknowns.push("part-id-readback-unavailable".to_owned());
        "not-run"
    };
    let roi_source = if parts.is_some() {
        if regions.is_empty() {
            "part-id"
        } else {
            "part-id+reference-regions"
        }
    } else {
        "not-run"
    };
    let roi = json!({
        "status":roi_status,
        "source":roi_source,
        "part_id_sha256":part_id_sha256,
        "material_id_sha256":material_id_sha256,
        "parts":parts.unwrap_or_default(),
        "regions":regions,
        "unknowns":roi_unknowns
    });
    let artifact_sha256 = context
        .visual
        .bundle
        .pointer("/lineage/artifact_sha256")
        .and_then(Value::as_str)
        .filter(|hash| is_sha256(hash))
        .or_else(|| {
            context
                .geometry
                .evidence
                .as_ref()
                .map(|evidence| evidence.artifact_object_sha256.as_str())
                .filter(|hash| is_sha256(hash))
        });
    let surface = build_surface_signal_readback(runtime, artifact_sha256);
    let status = if reference_mask.0 && candidate_mask.0 && aov_status == "ready" {
        "ready"
    } else if context
        .visual
        .bundle
        .get("available")
        .and_then(Value::as_bool)
        == Some(true)
    {
        "blocked"
    } else {
        "not-run"
    };
    Ok(canonicalize(json!({
        "schema_version":"VisualSurfaceReadback@1",
        "status":status,
        "resolution":[512,512],
        "reference_mask":reference_mask.2,
        "candidate_mask":candidate_mask.2,
        "edge":edge,
        "roi":roi,
        "aov":aov,
        "surface":surface,
        "canonical_sha256":""
    })))
}

#[derive(Debug, Clone, Copy)]
struct SurfaceEdgeIncident {
    triangle_index: usize,
    normal: [f32; 3],
}

fn build_surface_signal_readback(runtime: &Runtime, artifact_sha256: Option<&str>) -> Value {
    let not_run = || {
        canonicalize(json!({
            "schema_version":"SurfaceSignalReadback@1",
            "status":"not-run",
            "artifact_sha256":Value::Null,
            "triangle_count":Value::Null,
            "vertex_count":Value::Null,
            "edge_count":Value::Null,
            "non_manifold_edge_count":Value::Null,
            "curvature":{
                "status":"not-run",
                "method":"not-run",
                "mean_abs_dihedral_rad":Value::Null,
                "max_abs_dihedral_rad":Value::Null,
                "curved_triangle_count":Value::Null
            },
            "feature_line":{
                "status":"not-run",
                "method":"not-run",
                "threshold_rad":Value::Null,
                "edge_count":Value::Null,
                "boundary_edge_count":Value::Null,
                "crease_edge_count":Value::Null
            },
            "canonical_sha256":""
        }))
    };
    let Some(artifact_sha256) = artifact_sha256 else {
        return not_run();
    };
    let Ok(glb) = runtime.cas_read(artifact_sha256) else {
        return not_run();
    };
    let Ok(mesh) = super::integrity::extract_surface_mesh(&glb) else {
        return not_run();
    };

    const WELD_SCALE: f32 = 1_000_000.0;
    const CURVATURE_EPSILON_RAD: f64 = 0.01;
    const FEATURE_LINE_THRESHOLD_RAD: f64 = std::f64::consts::FRAC_PI_6;
    let mut edges = BTreeMap::<([i64; 3], [i64; 3]), Vec<SurfaceEdgeIncident>>::new();
    let mut curvature_sums = vec![0.0_f64; mesh.triangles.len()];
    let mut curvature_counts = vec![0_u32; mesh.triangles.len()];
    for (triangle_index, triangle) in mesh.triangles.iter().enumerate() {
        let keys = triangle
            .positions
            .map(|position| position.map(|component| (component * WELD_SCALE).round() as i64));
        for (left, right) in [(keys[0], keys[1]), (keys[1], keys[2]), (keys[2], keys[0])] {
            let edge = if left <= right {
                (left, right)
            } else {
                (right, left)
            };
            edges.entry(edge).or_default().push(SurfaceEdgeIncident {
                triangle_index,
                normal: triangle.normal,
            });
        }
    }

    let mut boundary_edge_count = 0_u64;
    let mut crease_edge_count = 0_u64;
    let mut non_manifold_edge_count = 0_u64;
    let mut total_dihedral = 0.0_f64;
    let mut paired_edge_count = 0_u64;
    let mut max_dihedral = 0.0_f64;
    for incidents in edges.values() {
        match incidents.as_slice() {
            [_] => boundary_edge_count += 1,
            [left, right] => {
                let dot = f64::from(
                    left.normal[0] * right.normal[0]
                        + left.normal[1] * right.normal[1]
                        + left.normal[2] * right.normal[2],
                )
                .clamp(-1.0, 1.0);
                let dihedral = dot.acos();
                total_dihedral += dihedral;
                paired_edge_count += 1;
                max_dihedral = max_dihedral.max(dihedral);
                for triangle_index in [left.triangle_index, right.triangle_index] {
                    curvature_sums[triangle_index] += dihedral;
                    curvature_counts[triangle_index] += 1;
                }
                if dihedral >= FEATURE_LINE_THRESHOLD_RAD {
                    crease_edge_count += 1;
                }
            }
            _ => non_manifold_edge_count += 1,
        }
    }
    let curved_triangle_count = curvature_sums
        .iter()
        .zip(curvature_counts.iter())
        .filter(|(sum, count)| **count > 0 && (**sum / f64::from(**count)) >= CURVATURE_EPSILON_RAD)
        .count() as u64;
    let feature_edge_count = boundary_edge_count + crease_edge_count;
    let ready = non_manifold_edge_count == 0;
    canonicalize(json!({
        "schema_version":"SurfaceSignalReadback@1",
        "status":if ready {"ready"} else {"blocked"},
        "artifact_sha256":artifact_sha256,
        "triangle_count":mesh.triangles.len() as u64,
        "vertex_count":mesh.vertex_count as u64,
        "edge_count":edges.len() as u64,
        "non_manifold_edge_count":non_manifold_edge_count,
        "curvature":{
            "status":if ready {"ready"} else {"not-run"},
            "method":if ready {"triangle-dihedral@1"} else {"not-run"},
            "mean_abs_dihedral_rad":if ready && paired_edge_count > 0 {Value::from(total_dihedral / paired_edge_count as f64)} else {Value::Null},
            "max_abs_dihedral_rad":if ready && paired_edge_count > 0 {Value::from(max_dihedral)} else {Value::Null},
            "curved_triangle_count":if ready {Value::from(curved_triangle_count)} else {Value::Null}
        },
        "feature_line":{
            "status":if ready {"ready"} else {"not-run"},
            "method":if ready {"boundary-and-crease-edge@1"} else {"not-run"},
            "threshold_rad":if ready {Value::from(FEATURE_LINE_THRESHOLD_RAD)} else {Value::Null},
            "edge_count":if ready {Value::from(feature_edge_count)} else {Value::Null},
            "boundary_edge_count":if ready {Value::from(boundary_edge_count)} else {Value::Null},
            "crease_edge_count":if ready {Value::from(crease_edge_count)} else {Value::Null}
        },
        "canonical_sha256":""
    }))
}

fn empty_visual_surface_readback() -> Value {
    canonicalize(json!({
        "schema_version":"VisualSurfaceReadback@1",
        "status":"not-run",
        "resolution":[512,512],
        "reference_mask":visual_surface_empty_mask_stats(),
        "candidate_mask":visual_surface_empty_mask_stats(),
        "edge":json!({
            "status":"not-run",
            "radius_px":4,
            "reference_edge_pixels":Value::Null,
            "candidate_edge_pixels":Value::Null,
            "matched_reference_edge_pixels":Value::Null,
            "matched_candidate_edge_pixels":Value::Null,
            "f1":Value::Null,
            "sdf_chamfer_px":Value::Null
        }),
        "roi":json!({
            "status":"not-run",
            "source":"not-run",
            "part_id_sha256":Value::Null,
            "material_id_sha256":Value::Null,
            "parts":[],
            "regions":[],
            "unknowns":["render-set-not-run"]
        }),
        "aov":json!({
            "status":"not-run",
            "source":"RenderSet@2/pass_artifacts",
            "passes":[],
            "missing_passes":VISUAL_SURFACE_AOV_PASSES
        }),
        "surface":json!({
            "schema_version":"SurfaceSignalReadback@1",
            "status":"not-run",
            "artifact_sha256":Value::Null,
            "triangle_count":Value::Null,
            "vertex_count":Value::Null,
            "edge_count":Value::Null,
            "non_manifold_edge_count":Value::Null,
            "curvature":{
                "status":"not-run",
                "method":"not-run",
                "mean_abs_dihedral_rad":Value::Null,
                "max_abs_dihedral_rad":Value::Null,
                "curved_triangle_count":Value::Null
            },
            "feature_line":{
                "status":"not-run",
                "method":"not-run",
                "threshold_rad":Value::Null,
                "edge_count":Value::Null,
                "boundary_edge_count":Value::Null,
                "crease_edge_count":Value::Null
            },
            "canonical_sha256":""
        }),
        "canonical_sha256":""
    }))
}

fn visual_surface_empty_mask_stats() -> Value {
    json!({
        "sha256":Value::Null,
        "decoded":false,
        "foreground_pixels":Value::Null,
        "edge_pixels":Value::Null,
        "bbox":Value::Null
    })
}

fn read_visual_surface_mask_stats(
    runtime: &Runtime,
    hash: Option<&str>,
) -> (bool, Option<Vec<bool>>, Value) {
    let Some(hash) = hash else {
        return (false, None, visual_surface_empty_mask_stats());
    };
    let Ok(bytes) = runtime.cas_read(hash) else {
        return (false, None, visual_surface_empty_mask_stats());
    };
    let Ok(mask) = super::decode_binary_mask(&bytes) else {
        return (false, None, visual_surface_empty_mask_stats());
    };
    let boundary = super::boundary_mask(&mask);
    let foreground_pixels = mask.iter().filter(|value| **value).count();
    let edge_pixels = boundary.iter().filter(|value| **value).count();
    let bbox = visual_surface_bbox(&mask);
    (
        true,
        Some(mask),
        json!({
            "sha256":hash,
            "decoded":true,
            "foreground_pixels":foreground_pixels,
            "edge_pixels":edge_pixels,
            "bbox":bbox
        }),
    )
}

fn visual_surface_bbox(mask: &[bool]) -> Value {
    let Some((min_x, min_y, max_x, max_y)) = super::bbox(mask) else {
        return Value::Null;
    };
    json!([min_x, min_y, max_x, max_y])
}

fn build_visual_surface_edge_readback(
    reference: Option<&[bool]>,
    candidate: Option<&[bool]>,
) -> Value {
    let Some(reference) = reference else {
        return json!({
            "status":"not-run",
            "radius_px":4,
            "reference_edge_pixels":Value::Null,
            "candidate_edge_pixels":Value::Null,
            "matched_reference_edge_pixels":Value::Null,
            "matched_candidate_edge_pixels":Value::Null,
            "f1":Value::Null,
            "sdf_chamfer_px":Value::Null
        });
    };
    let Some(candidate) = candidate else {
        return json!({
            "status":"not-run",
            "radius_px":4,
            "reference_edge_pixels":super::boundary_mask(reference).iter().filter(|value| **value).count(),
            "candidate_edge_pixels":Value::Null,
            "matched_reference_edge_pixels":Value::Null,
            "matched_candidate_edge_pixels":Value::Null,
            "f1":Value::Null,
            "sdf_chamfer_px":Value::Null
        });
    };
    let reference_edge = super::boundary_mask(reference);
    let candidate_edge = super::boundary_mask(candidate);
    let matched_reference = visual_surface_matched_edges(&reference_edge, &candidate_edge, 4);
    let matched_candidate = visual_surface_matched_edges(&candidate_edge, &reference_edge, 4);
    json!({
        "status":"ready",
        "radius_px":4,
        "reference_edge_pixels":reference_edge.iter().filter(|value| **value).count(),
        "candidate_edge_pixels":candidate_edge.iter().filter(|value| **value).count(),
        "matched_reference_edge_pixels":matched_reference,
        "matched_candidate_edge_pixels":matched_candidate,
        "f1":stable_surface_unit(super::boundary_f1(reference, candidate, 4)),
        "sdf_chamfer_px":stable_surface_metric(super::sdf_chamfer_px_at_resolution(reference, candidate, 512))
    })
}

fn visual_surface_matched_edges(left: &[bool], right: &[bool], radius: i32) -> usize {
    let mut matched = 0usize;
    for (index, value) in left.iter().enumerate() {
        if !*value {
            continue;
        }
        let x = (index % 512) as i32;
        let y = (index / 512) as i32;
        let found = (-radius..=radius).any(|dy| {
            (-radius..=radius).any(|dx| {
                let nx = x + dx;
                let ny = y + dy;
                nx >= 0 && ny >= 0 && nx < 512 && ny < 512 && right[ny as usize * 512 + nx as usize]
            })
        });
        if found {
            matched += 1;
        }
    }
    matched
}

fn stable_surface_unit(value: f64) -> Value {
    if value.is_finite() {
        Value::from(value.clamp(0.0, 1.0))
    } else {
        Value::Null
    }
}

fn stable_surface_metric(value: f64) -> Value {
    if value.is_finite() {
        Value::from(value.clamp(0.0, 512.0))
    } else {
        Value::Null
    }
}

fn visual_surface_palette_rows(bytes: &[u8], ids: &[String]) -> Option<Vec<Value>> {
    if ids.is_empty() {
        return None;
    }
    let image = image::load_from_memory(bytes)
        .ok()?
        .resize_exact(512, 512, image::imageops::FilterType::Nearest)
        .to_rgba8();
    let mut counts = vec![0usize; ids.len()];
    let mut bounds = vec![(512usize, 512usize, 0usize, 0usize); ids.len()];
    for y in 0..512usize {
        for x in 0..512usize {
            let Some(index) = super::part_color_index(image.get_pixel(x as u32, y as u32).0) else {
                continue;
            };
            if index >= ids.len() {
                continue;
            }
            counts[index] += 1;
            let bound = &mut bounds[index];
            bound.0 = bound.0.min(x);
            bound.1 = bound.1.min(y);
            bound.2 = bound.2.max(x);
            bound.3 = bound.3.max(y);
        }
    }
    Some(
        ids.iter()
            .enumerate()
            .filter_map(|(index, part_id)| {
                let count = counts[index];
                if count == 0 {
                    return None;
                }
                let bound = bounds[index];
                Some(json!({
                    "part_id":part_id,
                    "pixel_count":count,
                    "normalized_area":stable_surface_unit(count as f64 / 262144.0),
                    "bbox":[bound.0,bound.1,bound.2,bound.3]
                }))
            })
            .collect(),
    )
}

fn visual_surface_reference_regions(
    context: &ProjectionContext,
    reference: Option<&[bool]>,
    candidate: Option<&[bool]>,
) -> Vec<Value> {
    let Some(reference) = reference else {
        return Vec::new();
    };
    let Some(candidate) = candidate else {
        return Vec::new();
    };
    let Some(view_id) = context
        .visual
        .bundle
        .pointer("/render_set/view_id")
        .and_then(Value::as_str)
    else {
        return Vec::new();
    };
    let Some(views) = context
        .reference_canvas
        .pointer("/authoring_context/canvas/views")
        .and_then(Value::as_array)
    else {
        return Vec::new();
    };
    let Some(view_spec) = views
        .iter()
        .find(|view| view.get("view_id").and_then(Value::as_str) == Some(view_id))
        .and_then(|view| view.get("view_spec"))
        .and_then(Value::as_object)
    else {
        return Vec::new();
    };
    let Some(regions) = view_spec.get("regions").and_then(Value::as_array) else {
        return Vec::new();
    };
    regions
        .iter()
        .filter_map(|region| {
            let region_id = region.get("region_id").and_then(Value::as_str)?;
            let visibility = region
                .get("visibility")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            let x = region.get("x").and_then(Value::as_f64)?;
            let y = region.get("y").and_then(Value::as_f64)?;
            let width = region.get("width").and_then(Value::as_f64)?;
            let height = region.get("height").and_then(Value::as_f64)?;
            let x0 = (x.clamp(0.0, 1.0) * 512.0).floor() as usize;
            let y0 = (y.clamp(0.0, 1.0) * 512.0).floor() as usize;
            let x1 = ((x + width).clamp(0.0, 1.0) * 512.0).ceil() as usize;
            let y1 = ((y + height).clamp(0.0, 1.0) * 512.0).ceil() as usize;
            let mut reference_pixels = 0usize;
            let mut candidate_pixels = 0usize;
            let mut intersection = 0usize;
            let mut union = 0usize;
            for py in y0.min(512)..y1.min(512) {
                for px in x0.min(512)..x1.min(512) {
                    let index = py * 512 + px;
                    if reference[index] {
                        reference_pixels += 1;
                    }
                    if candidate[index] {
                        candidate_pixels += 1;
                    }
                    if reference[index] && candidate[index] {
                        intersection += 1;
                    }
                    if reference[index] || candidate[index] {
                        union += 1;
                    }
                }
            }
            Some(json!({
                "region_id":region_id,
                "visibility":visibility,
                "bbox":[x,y,width,height],
                "reference_pixels":reference_pixels,
                "candidate_pixels":candidate_pixels,
                "iou":if union == 0 {Value::Null} else {stable_surface_unit(intersection as f64 / union as f64)}
            }))
        })
        .collect()
}

fn visual_surface_binding(bundle: &Value) -> Value {
    json!({
        "reference_id":bundle.get("reference_id").cloned().unwrap_or(Value::Null),
        "reference_sha256":bundle.pointer("/reference/reference_sha256").cloned().unwrap_or(Value::Null),
        "artifact_sha256":bundle.pointer("/lineage/artifact_sha256").cloned().unwrap_or(Value::Null),
        "render_set_hash":bundle.pointer("/hashes/render_set_hash").cloned().unwrap_or(Value::Null),
        "camera_hash":bundle.pointer("/camera/camera_hash").cloned().unwrap_or(Value::Null),
        "comparison_report_hash":bundle.pointer("/hashes/comparison_report_hash").cloned().unwrap_or(Value::Null),
        "quality_report_hash":bundle.pointer("/hashes/quality_report_hash").cloned().unwrap_or(Value::Null)
    })
}

fn visual_surface_metrics(comparison: Option<&Value>) -> Value {
    let metric = |name: &str| {
        comparison
            .and_then(|report| report.get("metrics"))
            .and_then(|metrics| metrics.get(name))
            .and_then(Value::as_f64)
            .filter(|value| value.is_finite() && (0.0..=1.0).contains(value))
            .map(Value::from)
            .unwrap_or(Value::Null)
    };
    json!({
        "silhouette_iou":metric("silhouette_iou"),
        "boundary_f1_4px":metric("boundary_f1_4px"),
        "bbox_edge_error":metric("bbox_edge_error"),
        "centroid_error":metric("centroid_error"),
        "landmark_coverage":metric("landmark_coverage"),
        "landmark_nme":metric("landmark_nme"),
        "region_median_iou":metric("region_median_iou"),
        "critical_region_min_iou":metric("critical_region_min_iou")
    })
}

fn visual_surface_part_errors(part_error: Option<&Value>, max_items: usize) -> Vec<Value> {
    let Some(part_error) = part_error else {
        return Vec::new();
    };
    let evidence_hash = part_error
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .filter(|hash| is_sha256(hash))
        .map(str::to_owned)
        .map(Value::String)
        .unwrap_or(Value::Null);
    part_error
        .get("parts")
        .and_then(Value::as_array)
        .map(|parts| {
            parts
                .iter()
                .filter_map(|part| {
                    let part_id = part.get("part_id").and_then(Value::as_str)?;
                    if !forgecad_contracts::is_opaque_id(part_id) {
                        return None;
                    }
                    let status = if part.get("status").and_then(Value::as_str) == Some("ready") {
                        "ready"
                    } else {
                        "unknown"
                    };
                    let boundary_error_px = part
                        .get("boundary_error_px")
                        .and_then(Value::as_f64)
                        .filter(|value| value.is_finite() && (0.0..=512.0).contains(value));
                    Some(json!({
                        "part_id":part_id,
                        "status":status,
                        "boundary_error_px":boundary_error_px,
                        "boundary_error_normalized":boundary_error_px.map(|value| Value::from(value / 512.0)).unwrap_or(Value::Null),
                        "evidence_hash":evidence_hash.clone()
                    }))
                })
                .take(max_items)
                .collect()
        })
        .unwrap_or_default()
}

fn validate_visual_surface_readback(value: &Value) -> Result<(), RuntimeError> {
    let object = value.as_object().ok_or_else(|| {
        RuntimeError::InvalidInput(
            "CONTRACT_OUTPUT_INVALID: VisualSurfaceReadback@1 must be an object".to_owned(),
        )
    })?;
    let required = [
        "schema_version",
        "status",
        "resolution",
        "reference_mask",
        "candidate_mask",
        "edge",
        "roi",
        "aov",
        "surface",
        "canonical_sha256",
    ];
    if object.len() != required.len()
        || required.iter().any(|key| !object.contains_key(*key))
        || object.keys().any(|key| !required.contains(&key.as_str()))
    {
        return Err(visual_surface_readback_error("field set is not closed"));
    }
    if object.get("schema_version").and_then(Value::as_str) != Some("VisualSurfaceReadback@1")
        || !matches!(
            object.get("status").and_then(Value::as_str),
            Some("ready" | "blocked" | "not-run")
        )
    {
        return Err(visual_surface_readback_error("constants drifted"));
    }
    let resolution = object
        .get("resolution")
        .and_then(Value::as_array)
        .ok_or_else(|| visual_surface_readback_error("resolution is invalid"))?;
    if resolution.len() != 2
        || resolution.first().and_then(Value::as_u64) != Some(512)
        || resolution.get(1).and_then(Value::as_u64) != Some(512)
    {
        return Err(visual_surface_readback_error("resolution must be 512x512"));
    }
    for key in ["reference_mask", "candidate_mask"] {
        validate_visual_surface_mask_stats(object.get(key).expect("mask required"), key)?;
    }
    validate_visual_surface_edge_stats(object.get("edge").expect("edge required"))?;
    validate_visual_surface_roi_stats(object.get("roi").expect("roi required"))?;
    validate_visual_surface_aov_stats(object.get("aov").expect("aov required"))?;
    validate_visual_surface_surface_stats(object.get("surface").expect("surface required"))?;
    super::verify_output_canonical_hash(value, "VisualSurfaceReadback@1")
}

fn visual_surface_readback_error(detail: &str) -> RuntimeError {
    RuntimeError::InvalidInput(format!(
        "CONTRACT_OUTPUT_INVALID: VisualSurfaceReadback@1 {detail}"
    ))
}

fn validate_visual_surface_sha(
    value: &Value,
    label: &str,
    nullable: bool,
) -> Result<(), RuntimeError> {
    if nullable && value.is_null() {
        return Ok(());
    }
    if value.as_str().is_some_and(is_sha256) {
        Ok(())
    } else {
        Err(visual_surface_readback_error(&format!(
            "{label} is not a SHA-256"
        )))
    }
}

fn validate_visual_surface_bbox(value: &Value, label: &str) -> Result<(), RuntimeError> {
    if value.is_null() {
        return Ok(());
    }
    let values = value
        .as_array()
        .ok_or_else(|| visual_surface_readback_error(&format!("{label} is invalid")))?;
    if values.len() != 4
        || values
            .iter()
            .any(|value| value.as_u64().is_none_or(|value| value > 511))
    {
        return Err(visual_surface_readback_error(&format!(
            "{label} is invalid"
        )));
    }
    Ok(())
}

fn validate_visual_surface_mask_stats(value: &Value, label: &str) -> Result<(), RuntimeError> {
    let object = value
        .as_object()
        .ok_or_else(|| visual_surface_readback_error(&format!("{label} is invalid")))?;
    let required = [
        "sha256",
        "decoded",
        "foreground_pixels",
        "edge_pixels",
        "bbox",
    ];
    if object.len() != required.len()
        || required.iter().any(|key| !object.contains_key(*key))
        || object.keys().any(|key| !required.contains(&key.as_str()))
    {
        return Err(visual_surface_readback_error(&format!(
            "{label} field set is invalid"
        )));
    }
    validate_visual_surface_sha(object.get("sha256").expect("sha required"), label, true)?;
    if object.get("decoded").and_then(Value::as_bool).is_none() {
        return Err(visual_surface_readback_error(&format!(
            "{label}.decoded is invalid"
        )));
    }
    for key in ["foreground_pixels", "edge_pixels"] {
        let value = object.get(key).expect("mask count required");
        if !value.is_null() && value.as_u64().is_none_or(|value| value > 262_144) {
            return Err(visual_surface_readback_error(&format!(
                "{label}.{key} is invalid"
            )));
        }
    }
    validate_visual_surface_bbox(
        object.get("bbox").expect("bbox required"),
        &format!("{label}.bbox"),
    )
}

fn validate_visual_surface_edge_stats(value: &Value) -> Result<(), RuntimeError> {
    let object = value
        .as_object()
        .ok_or_else(|| visual_surface_readback_error("edge is invalid"))?;
    let required = [
        "status",
        "radius_px",
        "reference_edge_pixels",
        "candidate_edge_pixels",
        "matched_reference_edge_pixels",
        "matched_candidate_edge_pixels",
        "f1",
        "sdf_chamfer_px",
    ];
    if object.len() != required.len()
        || required.iter().any(|key| !object.contains_key(*key))
        || object.keys().any(|key| !required.contains(&key.as_str()))
        || !matches!(
            object.get("status").and_then(Value::as_str),
            Some("ready" | "not-run")
        )
        || object.get("radius_px").and_then(Value::as_u64) != Some(4)
    {
        return Err(visual_surface_readback_error("edge field set is invalid"));
    }
    for key in [
        "reference_edge_pixels",
        "candidate_edge_pixels",
        "matched_reference_edge_pixels",
        "matched_candidate_edge_pixels",
    ] {
        let value = object.get(key).expect("edge count required");
        if !value.is_null() && value.as_u64().is_none_or(|value| value > 262_144) {
            return Err(visual_surface_readback_error(&format!(
                "edge.{key} is invalid"
            )));
        }
    }
    if let Some(value) = object.get("f1").and_then(Value::as_f64) {
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err(visual_surface_readback_error("edge.f1 is invalid"));
        }
    } else if !object.get("f1").is_some_and(Value::is_null) {
        return Err(visual_surface_readback_error("edge.f1 is invalid"));
    }
    if let Some(value) = object.get("sdf_chamfer_px").and_then(Value::as_f64) {
        if !value.is_finite() || !(0.0..=512.0).contains(&value) {
            return Err(visual_surface_readback_error(
                "edge.sdf_chamfer_px is invalid",
            ));
        }
    } else if !object.get("sdf_chamfer_px").is_some_and(Value::is_null) {
        return Err(visual_surface_readback_error(
            "edge.sdf_chamfer_px is invalid",
        ));
    }
    Ok(())
}

fn validate_visual_surface_roi_stats(value: &Value) -> Result<(), RuntimeError> {
    let object = value
        .as_object()
        .ok_or_else(|| visual_surface_readback_error("roi is invalid"))?;
    let required = [
        "status",
        "source",
        "part_id_sha256",
        "material_id_sha256",
        "parts",
        "regions",
        "unknowns",
    ];
    if object.len() != required.len()
        || required.iter().any(|key| !object.contains_key(*key))
        || object.keys().any(|key| !required.contains(&key.as_str()))
        || !matches!(
            object.get("status").and_then(Value::as_str),
            Some("ready" | "partial" | "not-run")
        )
        || !matches!(
            object.get("source").and_then(Value::as_str),
            Some("part-id+reference-regions" | "part-id" | "not-run")
        )
    {
        return Err(visual_surface_readback_error("roi field set is invalid"));
    }
    validate_visual_surface_sha(
        object.get("part_id_sha256").expect("part sha required"),
        "roi.part_id_sha256",
        true,
    )?;
    validate_visual_surface_sha(
        object
            .get("material_id_sha256")
            .expect("material sha required"),
        "roi.material_id_sha256",
        true,
    )?;
    let parts = object
        .get("parts")
        .and_then(Value::as_array)
        .ok_or_else(|| visual_surface_readback_error("roi.parts is invalid"))?;
    let mut seen_parts = Vec::new();
    for part in parts {
        let part = part
            .as_object()
            .ok_or_else(|| visual_surface_readback_error("roi.part is invalid"))?;
        let required = ["part_id", "pixel_count", "normalized_area", "bbox"];
        if part.len() != required.len()
            || required.iter().any(|key| !part.contains_key(*key))
            || part.keys().any(|key| !required.contains(&key.as_str()))
        {
            return Err(visual_surface_readback_error(
                "roi.part field set is invalid",
            ));
        }
        let part_id = part
            .get("part_id")
            .and_then(Value::as_str)
            .filter(|value| forgecad_contracts::is_opaque_id(value))
            .ok_or_else(|| visual_surface_readback_error("roi.part_id is invalid"))?;
        if seen_parts.iter().any(|value| value == part_id) {
            return Err(visual_surface_readback_error("roi.part_id is duplicated"));
        }
        seen_parts.push(part_id.to_owned());
        if part
            .get("pixel_count")
            .and_then(Value::as_u64)
            .is_none_or(|value| value > 262_144)
        {
            return Err(visual_surface_readback_error("roi.pixel_count is invalid"));
        }
        validate_surface_unit_value(
            part.get("normalized_area").expect("area required"),
            "roi.normalized_area",
        )?;
        validate_visual_surface_bbox(
            part.get("bbox").expect("part bbox required"),
            "roi.part.bbox",
        )?;
    }
    let regions = object
        .get("regions")
        .and_then(Value::as_array)
        .ok_or_else(|| visual_surface_readback_error("roi.regions is invalid"))?;
    for region in regions {
        let region = region
            .as_object()
            .ok_or_else(|| visual_surface_readback_error("roi.region is invalid"))?;
        let required = [
            "region_id",
            "visibility",
            "bbox",
            "reference_pixels",
            "candidate_pixels",
            "iou",
        ];
        if region.len() != required.len()
            || required.iter().any(|key| !region.contains_key(*key))
            || region.keys().any(|key| !required.contains(&key.as_str()))
            || region
                .get("region_id")
                .and_then(Value::as_str)
                .is_none_or(|value| !forgecad_contracts::is_opaque_id(value))
            || !matches!(
                region.get("visibility").and_then(Value::as_str),
                Some("observed" | "inferred" | "unknown")
            )
        {
            return Err(visual_surface_readback_error(
                "roi.region field set is invalid",
            ));
        }
        let bbox = region
            .get("bbox")
            .and_then(Value::as_array)
            .ok_or_else(|| visual_surface_readback_error("roi.region.bbox is invalid"))?;
        if bbox.len() != 4
            || bbox.iter().any(|value| {
                value
                    .as_f64()
                    .is_none_or(|value| !value.is_finite() || !(0.0..=1.0).contains(&value))
            })
        {
            return Err(visual_surface_readback_error("roi.region.bbox is invalid"));
        }
        for key in ["reference_pixels", "candidate_pixels"] {
            if region
                .get(key)
                .and_then(Value::as_u64)
                .is_none_or(|value| value > 262_144)
            {
                return Err(visual_surface_readback_error(&format!(
                    "roi.region.{key} is invalid"
                )));
            }
        }
        validate_surface_nullable_unit_value(
            region.get("iou").expect("region iou required"),
            "roi.region.iou",
        )?;
    }
    let unknowns = object
        .get("unknowns")
        .and_then(Value::as_array)
        .ok_or_else(|| visual_surface_readback_error("roi.unknowns is invalid"))?;
    if unknowns.iter().any(|value| {
        !value.as_str().is_some_and(|value| {
            !value.is_empty()
                && value.len() <= 128
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
        })
    }) {
        return Err(visual_surface_readback_error("roi.unknowns is invalid"));
    }
    Ok(())
}

fn validate_visual_surface_aov_stats(value: &Value) -> Result<(), RuntimeError> {
    let object = value
        .as_object()
        .ok_or_else(|| visual_surface_readback_error("aov is invalid"))?;
    let required = ["status", "source", "passes", "missing_passes"];
    if object.len() != required.len()
        || required.iter().any(|key| !object.contains_key(*key))
        || object.keys().any(|key| !required.contains(&key.as_str()))
        || !matches!(
            object.get("status").and_then(Value::as_str),
            Some("ready" | "partial" | "not-run")
        )
        || object.get("source").and_then(Value::as_str) != Some("RenderSet@2/pass_artifacts")
    {
        return Err(visual_surface_readback_error("aov field set is invalid"));
    }
    let passes = object
        .get("passes")
        .and_then(Value::as_array)
        .ok_or_else(|| visual_surface_readback_error("aov.passes is invalid"))?;
    let mut seen = Vec::new();
    for pass in passes {
        let pass = pass
            .as_object()
            .ok_or_else(|| visual_surface_readback_error("aov.pass is invalid"))?;
        let required = [
            "pass",
            "sha256",
            "status",
            "pixel_count",
            "nonzero_pixel_count",
            "mean_rgba",
        ];
        if pass.len() != required.len()
            || required.iter().any(|key| !pass.contains_key(*key))
            || pass.keys().any(|key| !required.contains(&key.as_str()))
            || pass.get("status").and_then(Value::as_str) != Some("decoded")
        {
            return Err(visual_surface_readback_error(
                "aov.pass field set is invalid",
            ));
        }
        let pass_name = pass
            .get("pass")
            .and_then(Value::as_str)
            .filter(|value| VISUAL_SURFACE_AOV_PASSES.contains(value))
            .ok_or_else(|| visual_surface_readback_error("aov.pass name is invalid"))?;
        if seen.iter().any(|value| value == pass_name) {
            return Err(visual_surface_readback_error("aov.pass is duplicated"));
        }
        seen.push(pass_name.to_owned());
        validate_visual_surface_sha(
            pass.get("sha256").expect("aov sha required"),
            "aov.pass.sha256",
            false,
        )?;
        if pass.get("pixel_count").and_then(Value::as_u64) != Some(262_144)
            || pass
                .get("nonzero_pixel_count")
                .and_then(Value::as_u64)
                .is_none_or(|value| value > 262_144)
        {
            return Err(visual_surface_readback_error(
                "aov pixel counts are invalid",
            ));
        }
        let mean = pass
            .get("mean_rgba")
            .and_then(Value::as_array)
            .ok_or_else(|| visual_surface_readback_error("aov.mean_rgba is invalid"))?;
        if mean.len() != 4
            || mean
                .iter()
                .any(|value| value.as_u64().is_none_or(|value| value > 255))
        {
            return Err(visual_surface_readback_error("aov.mean_rgba is invalid"));
        }
    }
    let missing = object
        .get("missing_passes")
        .and_then(Value::as_array)
        .ok_or_else(|| visual_surface_readback_error("aov.missing_passes is invalid"))?;
    if missing.iter().any(|value| {
        !value
            .as_str()
            .is_some_and(|value| VISUAL_SURFACE_AOV_PASSES.contains(&value))
    }) {
        return Err(visual_surface_readback_error(
            "aov.missing_passes is invalid",
        ));
    }
    Ok(())
}

fn validate_visual_surface_surface_stats(value: &Value) -> Result<(), RuntimeError> {
    let object = value
        .as_object()
        .ok_or_else(|| visual_surface_readback_error("surface is invalid"))?;
    let required = [
        "schema_version",
        "status",
        "artifact_sha256",
        "triangle_count",
        "vertex_count",
        "edge_count",
        "non_manifold_edge_count",
        "curvature",
        "feature_line",
        "canonical_sha256",
    ];
    if object.len() != required.len()
        || required.iter().any(|key| !object.contains_key(*key))
        || object.keys().any(|key| !required.contains(&key.as_str()))
        || object.get("schema_version").and_then(Value::as_str) != Some("SurfaceSignalReadback@1")
        || !matches!(
            object.get("status").and_then(Value::as_str),
            Some("ready" | "blocked" | "not-run")
        )
    {
        return Err(visual_surface_readback_error(
            "surface field set is invalid",
        ));
    }
    validate_visual_surface_sha(
        object
            .get("artifact_sha256")
            .expect("surface artifact required"),
        "surface.artifact_sha256",
        true,
    )?;
    for key in [
        "triangle_count",
        "vertex_count",
        "edge_count",
        "non_manifold_edge_count",
    ] {
        let value = object.get(key).expect("surface count required");
        if !value.is_null() && value.as_u64().is_none() {
            return Err(visual_surface_readback_error(&format!(
                "surface.{key} is invalid"
            )));
        }
    }
    let curvature = object
        .get("curvature")
        .and_then(Value::as_object)
        .ok_or_else(|| visual_surface_readback_error("surface.curvature is invalid"))?;
    let curvature_required = [
        "status",
        "method",
        "mean_abs_dihedral_rad",
        "max_abs_dihedral_rad",
        "curved_triangle_count",
    ];
    if curvature.len() != curvature_required.len()
        || curvature_required
            .iter()
            .any(|key| !curvature.contains_key(*key))
        || curvature
            .keys()
            .any(|key| !curvature_required.contains(&key.as_str()))
        || !matches!(
            curvature.get("status").and_then(Value::as_str),
            Some("ready" | "not-run")
        )
        || !matches!(
            curvature.get("method").and_then(Value::as_str),
            Some("triangle-dihedral@1" | "not-run")
        )
    {
        return Err(visual_surface_readback_error(
            "surface.curvature field set is invalid",
        ));
    }
    for key in ["mean_abs_dihedral_rad", "max_abs_dihedral_rad"] {
        if let Some(value) = curvature.get(key).and_then(Value::as_f64) {
            if !value.is_finite() || !(0.0..=std::f64::consts::PI).contains(&value) {
                return Err(visual_surface_readback_error(&format!(
                    "surface.curvature.{key} is invalid"
                )));
            }
        } else if !curvature.get(key).is_some_and(Value::is_null) {
            return Err(visual_surface_readback_error(&format!(
                "surface.curvature.{key} is invalid"
            )));
        }
    }
    if !curvature
        .get("curved_triangle_count")
        .is_some_and(|value| value.is_null() || value.as_u64().is_some())
    {
        return Err(visual_surface_readback_error(
            "surface.curvature.curved_triangle_count is invalid",
        ));
    }
    let feature_line = object
        .get("feature_line")
        .and_then(Value::as_object)
        .ok_or_else(|| visual_surface_readback_error("surface.feature_line is invalid"))?;
    let feature_required = [
        "status",
        "method",
        "threshold_rad",
        "edge_count",
        "boundary_edge_count",
        "crease_edge_count",
    ];
    if feature_line.len() != feature_required.len()
        || feature_required
            .iter()
            .any(|key| !feature_line.contains_key(*key))
        || feature_line
            .keys()
            .any(|key| !feature_required.contains(&key.as_str()))
        || !matches!(
            feature_line.get("status").and_then(Value::as_str),
            Some("ready" | "not-run")
        )
        || !matches!(
            feature_line.get("method").and_then(Value::as_str),
            Some("boundary-and-crease-edge@1" | "not-run")
        )
    {
        return Err(visual_surface_readback_error(
            "surface.feature_line field set is invalid",
        ));
    }
    if let Some(value) = feature_line.get("threshold_rad").and_then(Value::as_f64) {
        if !value.is_finite() || !(0.0..=std::f64::consts::PI).contains(&value) {
            return Err(visual_surface_readback_error(
                "surface.feature_line.threshold_rad is invalid",
            ));
        }
    } else if !feature_line
        .get("threshold_rad")
        .is_some_and(Value::is_null)
    {
        return Err(visual_surface_readback_error(
            "surface.feature_line.threshold_rad is invalid",
        ));
    }
    for key in ["edge_count", "boundary_edge_count", "crease_edge_count"] {
        if !feature_line
            .get(key)
            .is_some_and(|value| value.is_null() || value.as_u64().is_some())
        {
            return Err(visual_surface_readback_error(&format!(
                "surface.feature_line.{key} is invalid"
            )));
        }
    }
    Ok(())
}

fn validate_surface_unit_value(value: &Value, label: &str) -> Result<(), RuntimeError> {
    if value
        .as_f64()
        .is_some_and(|value| value.is_finite() && (0.0..=1.0).contains(&value))
    {
        Ok(())
    } else {
        Err(visual_surface_readback_error(&format!(
            "{label} is invalid"
        )))
    }
}

fn validate_surface_nullable_unit_value(value: &Value, label: &str) -> Result<(), RuntimeError> {
    if value.is_null() {
        Ok(())
    } else {
        validate_surface_unit_value(value, label)
    }
}

fn validate_visual_surface_result(value: &Value) -> Result<(), RuntimeError> {
    let object = value.as_object().ok_or_else(|| {
        RuntimeError::InvalidInput(
            "CONTRACT_OUTPUT_INVALID: VisualSurfaceResult@1 must be an object".to_owned(),
        )
    })?;
    let required = [
        "schema_version",
        "projection_status",
        "read_only",
        "project_id",
        "candidate_id",
        "target_sha256",
        "status",
        "backend",
        "surface_program_status",
        "requested_signals",
        "available_signals",
        "unsupported_signals",
        "binding",
        "metrics",
        "part_errors",
        "readback",
        "unknowns",
        "lineage",
        "canonical_sha256",
    ];
    if object.len() != required.len()
        || required.iter().any(|key| !object.contains_key(*key))
        || object.keys().any(|key| !required.contains(&key.as_str()))
    {
        return Err(RuntimeError::InvalidInput(
            "CONTRACT_OUTPUT_INVALID: VisualSurfaceResult@1 field set is not closed".to_owned(),
        ));
    }
    if object.get("schema_version").and_then(Value::as_str) != Some("VisualSurfaceResult@1")
        || object.get("projection_status").and_then(Value::as_str)
            != Some(AGENTIC_PROJECTION_STATUS)
        || object.get("read_only") != Some(&Value::Bool(true))
        || !matches!(
            object.get("backend").and_then(Value::as_str),
            Some("candidate-bound-aov-diagnostics@1" | "candidate-bound-surface-analysis@1")
        )
        || !matches!(
            object.get("status").and_then(Value::as_str),
            Some("ready" | "blocked" | "not-run")
        )
        || !matches!(
            object.get("surface_program_status").and_then(Value::as_str),
            Some("ready" | "not-run" | "unavailable")
        )
    {
        return Err(RuntimeError::InvalidInput(
            "CONTRACT_OUTPUT_INVALID: VisualSurfaceResult@1 constants drifted".to_owned(),
        ));
    }
    for key in ["project_id", "candidate_id"] {
        let value = object
            .get(key)
            .and_then(Value::as_str)
            .filter(|value| forgecad_contracts::is_opaque_id(value));
        if value.is_none() {
            return Err(RuntimeError::InvalidInput(format!(
                "CONTRACT_OUTPUT_INVALID: VisualSurfaceResult@1.{key} is invalid"
            )));
        }
    }
    if let Some(target_sha256) = object.get("target_sha256").and_then(Value::as_str) {
        if !is_sha256(target_sha256) {
            return Err(RuntimeError::InvalidInput(
                "CONTRACT_OUTPUT_INVALID: VisualSurfaceResult@1.target_sha256 is invalid"
                    .to_owned(),
            ));
        }
    } else if !object.get("target_sha256").is_some_and(Value::is_null) {
        return Err(RuntimeError::InvalidInput(
            "CONTRACT_OUTPUT_INVALID: VisualSurfaceResult@1.target_sha256 is not string or null"
                .to_owned(),
        ));
    }
    for key in [
        "requested_signals",
        "available_signals",
        "unsupported_signals",
    ] {
        let values = object.get(key).and_then(Value::as_array).ok_or_else(|| {
            RuntimeError::InvalidInput(format!(
                "CONTRACT_OUTPUT_INVALID: VisualSurfaceResult@1.{key} is not an array"
            ))
        })?;
        let mut seen = Vec::new();
        for value in values {
            let signal = value.as_str().ok_or_else(|| {
                RuntimeError::InvalidInput(format!(
                    "CONTRACT_OUTPUT_INVALID: VisualSurfaceResult@1.{key} contains a non-string"
                ))
            })?;
            if !VISUAL_SURFACE_SIGNAL_NAMES.contains(&signal)
                || seen.iter().any(|existing| *existing == signal)
            {
                return Err(RuntimeError::InvalidInput(format!(
                    "CONTRACT_OUTPUT_INVALID: VisualSurfaceResult@1.{key} contains an invalid or duplicate signal"
                )));
            }
            seen.push(signal);
        }
    }
    let requested = object["requested_signals"]
        .as_array()
        .expect("array checked");
    let available = object["available_signals"]
        .as_array()
        .expect("array checked");
    let unsupported = object["unsupported_signals"]
        .as_array()
        .expect("array checked");
    if available.iter().any(|value| !requested.contains(value))
        || unsupported.iter().any(|value| !requested.contains(value))
        || available.iter().any(|value| unsupported.contains(value))
    {
        return Err(RuntimeError::InvalidInput(
            "CONTRACT_OUTPUT_INVALID: VisualSurfaceResult@1 signal partition is invalid".to_owned(),
        ));
    }
    validate_visual_surface_binding(
        object.get("binding").expect("binding required"),
        "VisualSurfaceResult@1.binding",
    )?;
    let metrics = object
        .get("metrics")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            RuntimeError::InvalidInput(
                "CONTRACT_OUTPUT_INVALID: VisualSurfaceResult@1.metrics is invalid".to_owned(),
            )
        })?;
    let metric_names = [
        "silhouette_iou",
        "boundary_f1_4px",
        "bbox_edge_error",
        "centroid_error",
        "landmark_coverage",
        "landmark_nme",
        "region_median_iou",
        "critical_region_min_iou",
    ];
    if metrics.len() != metric_names.len()
        || metric_names.iter().any(|key| !metrics.contains_key(*key))
        || metrics
            .keys()
            .any(|key| !metric_names.contains(&key.as_str()))
    {
        return Err(RuntimeError::InvalidInput(
            "CONTRACT_OUTPUT_INVALID: VisualSurfaceResult@1.metrics field set is invalid"
                .to_owned(),
        ));
    }
    for key in metric_names {
        if let Some(value) = metrics.get(key).and_then(Value::as_f64) {
            if !value.is_finite() || !(0.0..=1.0).contains(&value) {
                return Err(RuntimeError::InvalidInput(format!(
                    "CONTRACT_OUTPUT_INVALID: VisualSurfaceResult@1 metric {key} is out of range"
                )));
            }
        } else if !metrics.get(key).is_some_and(Value::is_null) {
            return Err(RuntimeError::InvalidInput(format!(
                "CONTRACT_OUTPUT_INVALID: VisualSurfaceResult@1 metric {key} is not numeric or null"
            )));
        }
    }
    let part_errors = object
        .get("part_errors")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            RuntimeError::InvalidInput(
                "CONTRACT_OUTPUT_INVALID: VisualSurfaceResult@1.part_errors is invalid".to_owned(),
            )
        })?;
    for part in part_errors {
        let part = part.as_object().ok_or_else(|| {
            RuntimeError::InvalidInput(
                "CONTRACT_OUTPUT_INVALID: VisualSurfaceResult@1 part error is not an object"
                    .to_owned(),
            )
        })?;
        let required = [
            "part_id",
            "status",
            "boundary_error_px",
            "boundary_error_normalized",
            "evidence_hash",
        ];
        if part.len() != required.len()
            || required.iter().any(|key| !part.contains_key(*key))
            || part.keys().any(|key| !required.contains(&key.as_str()))
            || !part
                .get("part_id")
                .and_then(Value::as_str)
                .is_some_and(forgecad_contracts::is_opaque_id)
            || !matches!(
                part.get("status").and_then(Value::as_str),
                Some("ready" | "unknown" | "not-run")
            )
        {
            return Err(RuntimeError::InvalidInput(
                "CONTRACT_OUTPUT_INVALID: VisualSurfaceResult@1 part error fields are invalid"
                    .to_owned(),
            ));
        }
        if let Some(value) = part.get("boundary_error_px").and_then(Value::as_f64) {
            if !value.is_finite() || !(0.0..=512.0).contains(&value) {
                return Err(RuntimeError::InvalidInput(
                    "CONTRACT_OUTPUT_INVALID: VisualSurfaceResult@1 part pixel error is invalid"
                        .to_owned(),
                ));
            }
        } else if !part.get("boundary_error_px").is_some_and(Value::is_null) {
            return Err(RuntimeError::InvalidInput(
                "CONTRACT_OUTPUT_INVALID: VisualSurfaceResult@1 part pixel error is not numeric or null".to_owned(),
            ));
        }
        if let Some(value) = part
            .get("boundary_error_normalized")
            .and_then(Value::as_f64)
        {
            if !value.is_finite() || !(0.0..=1.0).contains(&value) {
                return Err(RuntimeError::InvalidInput(
                    "CONTRACT_OUTPUT_INVALID: VisualSurfaceResult@1 normalized part error is invalid".to_owned(),
                ));
            }
        } else if !part
            .get("boundary_error_normalized")
            .is_some_and(Value::is_null)
        {
            return Err(RuntimeError::InvalidInput(
                "CONTRACT_OUTPUT_INVALID: VisualSurfaceResult@1 normalized part error is not numeric or null".to_owned(),
            ));
        }
        if let Some(value) = part.get("evidence_hash").and_then(Value::as_str) {
            if !is_sha256(value) {
                return Err(RuntimeError::InvalidInput(
                    "CONTRACT_OUTPUT_INVALID: VisualSurfaceResult@1 part evidence hash is invalid"
                        .to_owned(),
                ));
            }
        } else if !part.get("evidence_hash").is_some_and(Value::is_null) {
            return Err(RuntimeError::InvalidInput(
                "CONTRACT_OUTPUT_INVALID: VisualSurfaceResult@1 part evidence hash is not string or null".to_owned(),
            ));
        }
    }
    validate_visual_surface_readback(object.get("readback").expect("readback required"))?;
    let unknowns = object
        .get("unknowns")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            RuntimeError::InvalidInput(
                "CONTRACT_OUTPUT_INVALID: VisualSurfaceResult@1.unknowns is invalid".to_owned(),
            )
        })?;
    if unknowns.iter().any(|value| {
        !value.as_str().is_some_and(|value| {
            !value.is_empty()
                && value.len() <= 128
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
        })
    }) {
        return Err(RuntimeError::InvalidInput(
            "CONTRACT_OUTPUT_INVALID: VisualSurfaceResult@1.unknowns contains unsafe text"
                .to_owned(),
        ));
    }
    let lineage = object
        .get("lineage")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            RuntimeError::InvalidInput(
                "CONTRACT_OUTPUT_INVALID: VisualSurfaceResult@1.lineage is invalid".to_owned(),
            )
        })?;
    let lineage_required = [
        "project_id",
        "candidate_id",
        "target_sha256",
        "reference_id",
        "reference_sha256",
        "artifact_sha256",
        "render_set_hash",
        "camera_hash",
        "comparison_report_hash",
        "quality_report_hash",
    ];
    if lineage.len() != lineage_required.len()
        || lineage_required
            .iter()
            .any(|key| !lineage.contains_key(*key))
        || lineage
            .keys()
            .any(|key| !lineage_required.contains(&key.as_str()))
        || lineage.get("project_id") != object.get("project_id")
        || lineage.get("candidate_id") != object.get("candidate_id")
        || lineage.get("target_sha256") != object.get("target_sha256")
    {
        return Err(RuntimeError::InvalidInput(
            "CONTRACT_OUTPUT_INVALID: VisualSurfaceResult@1 lineage is not candidate-bound"
                .to_owned(),
        ));
    }
    validate_visual_surface_binding(
        &Value::Object(
            lineage
                .iter()
                .filter(|(key, _)| VISUAL_SURFACE_BINDING_KEYS.contains(&key.as_str()))
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect(),
        ),
        "VisualSurfaceResult@1.lineage",
    )?;
    super::verify_output_canonical_hash(value, "VisualSurfaceResult@1")
}

fn build_context(
    runtime: &Runtime,
    project_id: &str,
    requested_candidate_id: Option<&str>,
) -> Result<ProjectionContext, RuntimeError> {
    super::validate_id(project_id)?;
    let project = runtime.project(project_id)?.ok_or_else(|| {
        RuntimeError::InvalidInput("PROJECT_SCOPE_DENIED: project not found".to_owned())
    })?;
    let snapshot = project
        .head_snapshot_id
        .as_deref()
        .map(|snapshot_id| runtime.snapshot_record(snapshot_id))
        .transpose()?
        .flatten();
    if snapshot
        .as_ref()
        .is_some_and(|snapshot| snapshot.project_id != project_id)
    {
        return Err(binding_error("active snapshot project differs"));
    }

    let (candidate, candidate_selection) = select_candidate(
        runtime,
        project_id,
        requested_candidate_id,
        snapshot.as_ref(),
    )?;
    let geometry = match candidate.as_ref() {
        Some(candidate) => read_geometry_context(runtime, candidate)?,
        None => GeometryContext {
            evidence: None,
            program: None,
            artifact: None,
        },
    };
    let visual = match candidate.as_ref() {
        Some(candidate) => read_visual_context(runtime, candidate, &geometry)?,
        None => unknown_visual_context(None, None, None),
    };
    let reference_id = visual.reference_id.clone().or_else(|| {
        geometry
            .evidence
            .as_ref()
            .and_then(|evidence| evidence.reference_id.clone())
    });
    let reference_canvas = build_reference_canvas(
        runtime,
        project_id,
        reference_id.as_deref(),
        candidate
            .as_ref()
            .map(|candidate| candidate.candidate_id.as_str()),
    )?;
    let mut visual = visual;
    enrich_visual_context(
        runtime,
        &mut visual,
        &reference_canvas,
        candidate
            .as_ref()
            .map(|candidate| candidate.candidate_id.as_str()),
    )?;
    let quality = build_quality_context(runtime, candidate.as_ref(), &geometry, &visual)?;
    let lineage = build_lineage(
        &project,
        snapshot.as_ref(),
        candidate.as_ref(),
        &geometry,
        &visual,
        &quality,
        reference_canvas
            .get("selected_reference_id")
            .and_then(Value::as_str),
        runtime,
    )?;
    let projection_key = canonical_json_hash(&json!({
        "project_id":project.project_id,
        "project_canonical_sha256":project.canonical_sha256,
        "snapshot_id":snapshot.as_ref().map(|snapshot| snapshot.snapshot_id.clone()),
        "snapshot_revision":snapshot.as_ref().map(|snapshot| snapshot.revision),
        "candidate_id":candidate.as_ref().map(|candidate| candidate.candidate_id.clone()),
        "candidate_canonical_sha256":candidate.as_ref().map(|candidate| candidate.canonical_sha256.clone()),
        "lineage":lineage
    }));

    let mut context = ProjectionContext {
        project,
        snapshot,
        candidate,
        candidate_selection,
        geometry,
        visual,
        quality,
        reference_canvas,
        lineage,
        projection_key,
    };
    context.visual.surface_readback = build_visual_surface_readback(runtime, &context)?;
    Ok(context)
}

fn select_candidate(
    runtime: &Runtime,
    project_id: &str,
    requested_candidate_id: Option<&str>,
    snapshot: Option<&SnapshotRecord>,
) -> Result<(Option<CandidateRecord>, &'static str), RuntimeError> {
    if let Some(candidate_id) = requested_candidate_id {
        super::validate_id(candidate_id)?;
        let candidate = runtime.candidate(candidate_id)?.ok_or_else(|| {
            RuntimeError::InvalidInput("NOT_FOUND: candidate not found".to_owned())
        })?;
        if candidate.project_id != project_id {
            return Err(binding_error("candidate is outside the requested project"));
        }
        return Ok((Some(candidate), "explicit_candidate_id"));
    }

    if let Some(candidate_id) = snapshot.and_then(|snapshot| snapshot.candidate_id.as_deref()) {
        let candidate = runtime
            .candidate(candidate_id)?
            .ok_or_else(|| binding_error("active snapshot candidate is unavailable"))?;
        if candidate.project_id != project_id {
            return Err(binding_error(
                "active snapshot candidate is outside the project",
            ));
        }
        return Ok((Some(candidate), "active_snapshot"));
    }

    let candidate = runtime.candidates(project_id)?.into_iter().next();
    Ok((candidate, "latest_candidate_when_no_active_snapshot"))
}

fn read_geometry_context(
    runtime: &Runtime,
    candidate: &CandidateRecord,
) -> Result<GeometryContext, RuntimeError> {
    let evidence = runtime
        .store
        .get_geometry_candidate_evidence(&candidate.candidate_id)?;
    if let Some(evidence) = evidence {
        validate_geometry_binding(runtime, candidate, &evidence)?;
        let program = read_json_object(
            runtime,
            &evidence.geometry_program_object_sha256,
            "geometry program",
        )?;
        if canonical_json_hash(&program) != evidence.geometry_program_sha256 {
            return Err(binding_error("geometry program canonical hash differs"));
        }
        let artifact =
            runtime.artifact_readback(&evidence.artifact_object_sha256, &candidate.candidate_id)?;
        return Ok(GeometryContext {
            evidence: Some(evidence),
            program: Some(program),
            artifact: Some(artifact),
        });
    }

    let artifact_hash = candidate
        .manifest_hash
        .as_deref()
        .or(candidate.prepared_object_sha256.as_deref());
    let Some(artifact_hash) = artifact_hash else {
        return Ok(GeometryContext {
            evidence: None,
            program: None,
            artifact: None,
        });
    };
    if !is_sha256(artifact_hash) {
        return Err(binding_error("candidate artifact hash is invalid"));
    }
    let Some(object) = runtime.store.get_object(artifact_hash)? else {
        return Err(binding_error("candidate artifact object is unavailable"));
    };
    if object.mime != "model/gltf-binary" {
        return Ok(GeometryContext {
            evidence: None,
            program: None,
            artifact: None,
        });
    }
    let artifact = runtime.artifact_readback(artifact_hash, &candidate.candidate_id)?;
    Ok(GeometryContext {
        evidence: None,
        program: None,
        artifact: Some(artifact),
    })
}

fn validate_geometry_binding(
    runtime: &Runtime,
    candidate: &CandidateRecord,
    evidence: &GeometryCandidateEvidenceRecord,
) -> Result<(), RuntimeError> {
    if evidence.candidate_id != candidate.candidate_id
        || evidence.project_id != candidate.project_id
        || candidate.prepared_object_sha256.as_deref()
            != Some(evidence.artifact_object_sha256.as_str())
        || candidate
            .manifest_hash
            .as_deref()
            .is_some_and(|hash| hash != evidence.artifact_object_sha256)
        || candidate.quality_report_id.as_deref() != Some(evidence.quality_report_id.as_str())
    {
        return Err(binding_error(
            "geometry evidence is bound to another candidate",
        ));
    }
    let hashes = [
        evidence.reference_sha256.clone(),
        Some(evidence.geometry_program_sha256.clone()),
        Some(evidence.geometry_program_object_sha256.clone()),
        Some(evidence.operator_catalog_sha256.clone()),
        Some(evidence.readback_config_sha256.clone()),
        Some(evidence.artifact_object_sha256.clone()),
        Some(evidence.artifact_readback_object_sha256.clone()),
        Some(evidence.quality_report_object_sha256.clone()),
    ];
    for hash in hashes.iter() {
        if hash.as_deref().is_some_and(|hash| !is_sha256(hash)) {
            return Err(binding_error("geometry evidence contains an invalid hash"));
        }
    }
    if let Some(reference_id) = evidence.reference_id.as_deref() {
        let reference = runtime
            .reference(reference_id)?
            .ok_or_else(|| binding_error("geometry evidence reference is unavailable"))?;
        if reference.project_id != candidate.project_id
            || evidence.reference_sha256.as_deref() != Some(reference.object_sha256.as_str())
        {
            return Err(binding_error(
                "geometry evidence reference is not project-bound",
            ));
        }
    } else if evidence.reference_sha256.is_some() {
        return Err(binding_error(
            "geometry evidence has a hash without a reference",
        ));
    }
    Ok(())
}

fn read_visual_context(
    runtime: &Runtime,
    candidate: &CandidateRecord,
    geometry: &GeometryContext,
) -> Result<VisualContext, RuntimeError> {
    let Some(evidence) = runtime.store.get_visual_evidence(&candidate.candidate_id)? else {
        return Ok(unknown_visual_context(
            Some(candidate.candidate_id.clone()),
            geometry
                .evidence
                .as_ref()
                .and_then(|evidence| evidence.reference_id.clone()),
            None,
        ));
    };
    if evidence.candidate_id != candidate.candidate_id
        || evidence.project_id != candidate.project_id
    {
        return Err(binding_error(
            "visual evidence is bound to another candidate or project",
        ));
    }
    let reference = runtime
        .reference(&evidence.reference_id)?
        .ok_or_else(|| binding_error("visual evidence reference is unavailable"))?;
    if reference.project_id != candidate.project_id {
        return Err(binding_error(
            "visual evidence reference is outside the candidate project",
        ));
    }
    if let Some(geometry_reference_id) = geometry
        .evidence
        .as_ref()
        .and_then(|evidence| evidence.reference_id.as_deref())
    {
        if geometry_reference_id != evidence.reference_id {
            return Err(binding_error("visual and geometry references differ"));
        }
    }

    let render_set = read_json_object(runtime, &evidence.render_set_object_sha256, "RenderSet")?;
    super::validate_render_set_v2_output(&render_set)?;
    if render_set.get("candidate_id").and_then(Value::as_str)
        != Some(candidate.candidate_id.as_str())
        || render_set.get("reference_id").and_then(Value::as_str)
            != Some(evidence.reference_id.as_str())
        || render_set.get("reference_id").and_then(Value::as_str)
            != Some(reference.reference_id.as_str())
        || render_set
            .get("camera_hash")
            .and_then(Value::as_str)
            .is_none_or(|hash| !is_sha256(hash))
    {
        return Err(binding_error(
            "RenderSet candidate/reference/camera binding differs",
        ));
    }
    if let Some(artifact) = geometry.artifact.as_ref() {
        if render_set.get("artifact_sha256").and_then(Value::as_str)
            != artifact.get("artifact_id").and_then(Value::as_str)
        {
            return Err(binding_error(
                "RenderSet artifact differs from candidate readback",
            ));
        }
    }

    let comparison = evidence
        .comparison_report_object_sha256
        .as_deref()
        .map(|hash| {
            let comparison = read_json_object(runtime, hash, "comparison report")?;
            super::validate_reference_comparison_report(&comparison)?;
            if comparison.get("candidate_id").and_then(Value::as_str)
                != Some(candidate.candidate_id.as_str())
                || comparison.get("reference_id").and_then(Value::as_str)
                    != Some(evidence.reference_id.as_str())
                || comparison.get("render_set_hash").and_then(Value::as_str)
                    != Some(evidence.render_set_object_sha256.as_str())
            {
                return Err(binding_error(
                    "comparison report is bound to another candidate",
                ));
            }
            Ok(comparison)
        })
        .transpose()?;

    let quality_report = read_json_object(
        runtime,
        &evidence.quality_report_object_sha256,
        "quality report",
    )?;
    if quality_report.get("schema_version").and_then(Value::as_str) != Some("QualityReport@2") {
        return Err(binding_error(
            "visual evidence quality report is not QualityReport@2",
        ));
    }
    super::validate_quality_report_v2_output(&quality_report)?;
    if quality_report.get("candidate_id").and_then(Value::as_str)
        != Some(candidate.candidate_id.as_str())
        || quality_report.get("reference_id").and_then(Value::as_str)
            != Some(evidence.reference_id.as_str())
        || quality_report
            .get("reference_sha256")
            .and_then(Value::as_str)
            != Some(reference.object_sha256.as_str())
        || quality_report
            .get("render_set_hash")
            .and_then(Value::as_str)
            != Some(evidence.render_set_object_sha256.as_str())
        || quality_report
            .get("comparison_report_hash")
            .and_then(Value::as_str)
            != evidence.comparison_report_object_sha256.as_deref()
    {
        return Err(binding_error(
            "quality report is bound to another candidate/evidence",
        ));
    }

    let camera_hash = render_set["camera_hash"].as_str().unwrap_or_default();
    let comparison_status = comparison
        .as_ref()
        .and_then(|comparison| comparison.get("status"))
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_owned();
    let mut bundle = canonicalize(json!({
        "schema_version":"VisualEvidenceBundle@1",
        "projection_status":AGENTIC_PROJECTION_STATUS,
        "read_only":true,
        "available":true,
        "status":quality_report.get("visual_status").and_then(Value::as_str).unwrap_or("unknown"),
        "project_id":candidate.project_id,
        "candidate_id":candidate.candidate_id,
        "reference_id":evidence.reference_id,
        "reference":reference_projection(&reference),
        "camera":{"camera_hash":camera_hash,"status":"observed","source":"RenderSet@2"},
        "render_set":render_set,
        "comparison_report":comparison,
        "quality_report":quality_report,
        "hashes":{
            "render_set_hash":evidence.render_set_object_sha256,
            "comparison_report_hash":evidence.comparison_report_object_sha256,
            "quality_report_hash":evidence.quality_report_object_sha256,
            "visual_review_hash":evidence.visual_review_object_sha256,
            "human_receipt_hash":evidence.human_receipt_object_sha256
        },
        "lineage":{
            "candidate_id":candidate.candidate_id,
            "reference_id":evidence.reference_id,
            "reference_sha256":reference.object_sha256,
            "artifact_sha256":render_set["artifact_sha256"],
            "program_sha256":render_set["program_sha256"],
            "camera_hash":render_set["camera_hash"],
            "render_set_hash":evidence.render_set_object_sha256,
            "comparison_report_hash":evidence.comparison_report_object_sha256,
            "quality_report_hash":evidence.quality_report_object_sha256
        },
        "canonical_sha256":""
    }));
    if let Some(cross_view) = runtime
        .store
        .get_latest_cross_view_evidence(&candidate.candidate_id)?
    {
        let cross_view_bundle = read_json_object(
            runtime,
            &cross_view.bundle_object_sha256,
            "cross-view evidence",
        )?;
        super::validate_cross_view_evidence_bundle(&cross_view_bundle)?;
        if cross_view_bundle
            .get("candidate_id")
            .and_then(Value::as_str)
            != Some(candidate.candidate_id.as_str())
            || cross_view_bundle.get("project_id").and_then(Value::as_str)
                != Some(candidate.project_id.as_str())
        {
            return Err(binding_error(
                "cross-view evidence is bound to another candidate or project",
            ));
        }
        bundle["cross_view_evidence"] = cross_view_bundle;
        bundle = canonicalize(bundle);
    }
    Ok(VisualContext {
        bundle,
        reference_id: Some(evidence.reference_id),
        quality_report: Some(quality_report),
        quality_report_hash: Some(evidence.quality_report_object_sha256),
        comparison_status,
        part_error: None,
        surface_readback: empty_visual_surface_readback(),
    })
}

fn unknown_visual_context(
    candidate_id: Option<String>,
    reference_id: Option<String>,
    quality_report_hash: Option<String>,
) -> VisualContext {
    VisualContext {
        bundle: canonicalize(json!({
            "schema_version":"VisualEvidenceBundle@1",
            "projection_status":AGENTIC_PROJECTION_STATUS,
            "read_only":true,
            "available":false,
            "status":"unknown",
            "candidate_id":candidate_id,
            "reference_id":reference_id,
            "camera":{"camera_hash":Value::Null,"status":"unknown"},
            "render_set":Value::Null,
            "comparison_report":Value::Null,
            "quality_report":Value::Null,
            "hashes":{
                "render_set_hash":Value::Null,
                "comparison_report_hash":Value::Null,
                "quality_report_hash":quality_report_hash,
                "visual_review_hash":Value::Null,
                "human_receipt_hash":Value::Null
            },
            "unknowns":["render_set","comparison_report","camera","visual_gate"],
            "canonical_sha256":""
        })),
        reference_id,
        quality_report: None,
        quality_report_hash,
        comparison_status: "unknown".to_owned(),
        part_error: None,
        surface_readback: empty_visual_surface_readback(),
    }
}

fn build_quality_context(
    runtime: &Runtime,
    candidate: Option<&CandidateRecord>,
    geometry: &GeometryContext,
    visual: &VisualContext,
) -> Result<QualityContext, RuntimeError> {
    let Some(candidate) = candidate else {
        return Ok(QualityContext {
            projection: unknown_quality_projection(None, None, "unknown", "unknown", "unknown"),
            report_hash: None,
            report_id: None,
            structural_status: "unknown".to_owned(),
            visual_status: "unknown".to_owned(),
            strict_visual_gate: "unknown".to_owned(),
        });
    };

    let (report, report_hash) = if let Some(report) = visual.quality_report.clone() {
        (Some(report), visual.quality_report_hash.clone())
    } else {
        let reference_id = geometry
            .evidence
            .as_ref()
            .and_then(|evidence| evidence.reference_id.as_deref());
        match runtime.quality(&candidate.candidate_id, reference_id) {
            Ok(report) => (
                Some(report),
                geometry
                    .evidence
                    .as_ref()
                    .map(|evidence| evidence.quality_report_object_sha256.clone()),
            ),
            Err(RuntimeError::InvalidInput(message))
                if message.starts_with("QUALITY_REPORT_UNAVAILABLE") =>
            {
                (None, None)
            }
            Err(error) => return Err(error),
        }
    };
    let structural_status = report
        .as_ref()
        .and_then(|report| report.get("structural_status"))
        .and_then(Value::as_str)
        .map(str::to_owned)
        .unwrap_or_else(|| {
            if candidate.state == "failed" {
                "failed".to_owned()
            } else if candidate.quality_hard_gate_passed {
                "passed".to_owned()
            } else {
                "unknown".to_owned()
            }
        });
    let visual_status = report
        .as_ref()
        .and_then(|report| report.get("visual_status"))
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_owned();
    let strict_visual_gate =
        strict_visual_gate_status(candidate, report.as_ref(), visual, &visual_status);
    let report_id = report
        .as_ref()
        .and_then(|report| report.get("quality_report_id"))
        .and_then(Value::as_str)
        .map(str::to_owned);
    let projection = quality_projection(
        report.clone(),
        report_hash.clone(),
        &structural_status,
        &visual_status,
        &strict_visual_gate,
        candidate,
    );
    Ok(QualityContext {
        projection,
        report_hash,
        report_id,
        structural_status,
        visual_status,
        strict_visual_gate,
    })
}

fn strict_visual_gate_status(
    candidate: &CandidateRecord,
    report: Option<&Value>,
    visual: &VisualContext,
    visual_status: &str,
) -> String {
    if candidate.state == "failed" {
        return "failed".to_owned();
    }
    let Some(report) = report else {
        return "unknown".to_owned();
    };
    let report_passed = report.get("hard_gate_passed").and_then(Value::as_bool) == Some(true);
    if visual.bundle.get("available").and_then(Value::as_bool) == Some(true)
        && visual_status == "PARTIAL_VISIBLE_VIEW_PASS"
        && report_passed
        && candidate.quality_hard_gate_passed
        && visual.comparison_status == "PARTIAL_VISIBLE_VIEW_PASS"
    {
        "passed".to_owned()
    } else if visual.bundle.get("available").and_then(Value::as_bool) == Some(true)
        || matches!(
            visual_status,
            "QUALITY_TARGET_NOT_MET" | "BLOCKED_REFERENCE_COVERAGE"
        )
        || candidate.state == "failed"
    {
        "failed".to_owned()
    } else {
        "unknown".to_owned()
    }
}

fn build_reference_canvas(
    runtime: &Runtime,
    project_id: &str,
    selected_reference_id: Option<&str>,
    candidate_id: Option<&str>,
) -> Result<Value, RuntimeError> {
    let references = runtime.references(project_id)?;
    if let Some(selected_reference_id) = selected_reference_id {
        if !references
            .iter()
            .any(|reference| reference.reference_id == selected_reference_id)
        {
            return Err(binding_error("selected reference is outside the project"));
        }
    }
    let reference_values = references
        .iter()
        .map(reference_projection)
        .collect::<Vec<_>>();
    let mut projection = canonicalize(json!({
        "schema_version":"ReferenceCanvasProjection@1",
        "projection_status":AGENTIC_PROJECTION_STATUS,
        "read_only":true,
        "project_id":project_id,
        "selected_reference_id":selected_reference_id,
        "references":reference_values,
        "coverage":{"status":"unknown","observed_views":[],"missing_views":["front","back","left","right","top","three-quarter"],"reason":"ReferenceEvidence does not carry a complete view-coverage contract"},
        "unknowns":["view_coverage","reference_camera_claim","hidden_geometry","material_detail_coverage"],
        "canonical_sha256":""
    }));
    if let Some((canvas, canvas_object_sha256)) =
        super::agentic_session::durable_reference_canvas_for_binding(
            runtime,
            project_id,
            candidate_id,
        )?
    {
        projection["coverage"] = projection_coverage_from_authoring(&canvas);
        projection["unknowns"] = json!([
            "hidden_geometry",
            "unseen_surface_detail",
            "per_view_render_comparison",
            "per_view_camera_binding"
        ]);
        projection["authoring_context"] = json!({
            "status":"observed",
            "canvas_sha256":canvas_object_sha256,
            "canvas":canvas
        });
        projection = canonicalize(projection);
    }
    Ok(projection)
}

fn projection_coverage_from_authoring(canvas: &Value) -> Value {
    const PROJECTED_VIEWS: [&str; 8] = [
        "front",
        "back",
        "left",
        "right",
        "top",
        "perspective",
        "three-quarter",
        "rear-three-quarter",
    ];
    let coverage = canvas.get("coverage");
    let observed_views = coverage
        .and_then(|coverage| coverage.get("supplied_views"))
        .and_then(Value::as_array)
        .map(|views| {
            views
                .iter()
                .filter_map(Value::as_str)
                .filter(|view| PROJECTED_VIEWS.contains(view))
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let missing_views = coverage
        .and_then(|coverage| coverage.get("missing_views"))
        .and_then(Value::as_array)
        .map(|views| {
            views
                .iter()
                .filter_map(Value::as_str)
                .filter(|view| PROJECTED_VIEWS.contains(view))
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let status = match coverage
        .and_then(|coverage| coverage.get("coverage_status"))
        .and_then(Value::as_str)
    {
        Some("complete") => "complete",
        Some("partial") | Some("blocked") => "partial",
        _ => "unknown",
    };
    let reason = if status == "complete" {
        "durable ReferenceCanvas coverage is complete for its declared required views"
    } else {
        "durable ReferenceCanvas declares only a partial or blocked view set"
    };
    json!({
        "status":status,
        "observed_views":observed_views,
        "missing_views":missing_views,
        "reason":reason
    })
}

fn enrich_visual_context(
    runtime: &Runtime,
    visual: &mut VisualContext,
    reference_canvas: &Value,
    candidate_id: Option<&str>,
) -> Result<(), RuntimeError> {
    let Some(authoring_canvas) = reference_canvas
        .get("authoring_context")
        .and_then(|context| context.get("canvas"))
    else {
        visual.bundle = canonicalize(visual.bundle.clone());
        return Ok(());
    };
    let view_records = candidate_id
        .map(|candidate_id| runtime.store.list_visual_evidence_views(candidate_id))
        .transpose()?
        .unwrap_or_default();
    let view_evidence = authoring_canvas
        .get("views")
        .and_then(Value::as_array)
        .map(|views| {
            views
                .iter()
                .filter_map(|view| {
                    let view_id = view.get("view_id").and_then(Value::as_str)?;
                    let record = view_records.iter().find(|record| {
                        record.view_id == view_id
                            && record.reference_id
                                == view
                                    .get("reference_id")
                                    .and_then(Value::as_str)
                                    .unwrap_or_default()
                            && record.reference_sha256
                                == view
                                    .get("reference_sha256")
                                    .and_then(Value::as_str)
                                    .unwrap_or_default()
                    });
                    build_view_evidence(view, &visual.bundle, record)
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let scope = if view_evidence.len() > 1 {
        "multi-view-reference-inventory"
    } else {
        "single-view-reference-inventory"
    };
    visual.bundle["reference_canvas"] = reference_canvas.clone();
    visual.bundle["evidence_scope"] = Value::String(scope.to_owned());
    visual.bundle["view_evidence"] = Value::Array(view_evidence);
    visual.bundle = canonicalize(visual.bundle.clone());
    Ok(())
}

fn build_view_evidence(
    view: &Value,
    visual_bundle: &Value,
    view_record: Option<&forgecad_store::VisualEvidenceViewRecord>,
) -> Option<Value> {
    let view_id = view.get("view_id").and_then(Value::as_str)?;
    let reference_id = view.get("reference_id").and_then(Value::as_str)?;
    let reference_sha256 = view.get("reference_sha256").and_then(Value::as_str)?;
    let candidate_reference_id = visual_bundle.get("reference_id").and_then(Value::as_str);
    let reference_matches = candidate_reference_id == Some(reference_id);
    let hashes = visual_bundle.get("hashes");
    let camera_hash = visual_bundle
        .get("camera")
        .and_then(|camera| camera.get("camera_hash"))
        .cloned()
        .unwrap_or(Value::Null);
    let actual_evidence = view_record.is_some()
        || (reference_matches
            && visual_bundle.get("available").and_then(Value::as_bool) == Some(true));
    let (binding_status, view_match_status, unknowns) = if actual_evidence {
        if let Some(_record) = view_record {
            ("candidate-bound-reference", "observed", Vec::new())
        } else {
            (
                "candidate-bound-reference",
                "unknown",
                vec!["render_and_comparison_are_not_bound_to_view_id"],
            )
        }
    } else {
        (
            "not-run",
            "not-run",
            vec!["render_set", "comparison_report", "quality_report"],
        )
    };
    Some(json!({
        "view_id":view_id,
        "kind":view.get("kind").cloned().unwrap_or(Value::String("detail".to_owned())),
        "reference_id":reference_id,
        "reference_sha256":reference_sha256,
        "camera_claim":view.get("camera_claim").cloned().unwrap_or(Value::Null),
        "binding_status":binding_status,
        "view_match_status":view_match_status,
        "camera_hash":if let Some(record) = view_record { Value::String(record.camera_hash.clone()) } else if actual_evidence {camera_hash} else {Value::Null},
        "render_set_hash":if let Some(record) = view_record { Value::String(record.render_set_object_sha256.clone()) } else if actual_evidence {hashes.and_then(|hashes| hashes.get("render_set_hash")).cloned().unwrap_or(Value::Null)} else {Value::Null},
        "comparison_report_hash":if let Some(record) = view_record { record.comparison_report_object_sha256.clone().map(Value::String).unwrap_or(Value::Null) } else if actual_evidence {hashes.and_then(|hashes| hashes.get("comparison_report_hash")).cloned().unwrap_or(Value::Null)} else {Value::Null},
        "quality_report_hash":if let Some(record) = view_record { Value::String(record.quality_report_object_sha256.clone()) } else if actual_evidence {hashes.and_then(|hashes| hashes.get("quality_report_hash")).cloned().unwrap_or(Value::Null)} else {Value::Null},
        "quality_status":if let Some(record) = view_record { Value::String(record.quality_status.clone()) } else if actual_evidence {visual_bundle.get("status").cloned().unwrap_or(Value::String("unknown".to_owned()))} else {Value::String("not-run".to_owned())},
        "unknowns":unknowns
    }))
}

fn reference_projection(reference: &ReferenceEvidenceRecord) -> Value {
    json!({
        "reference_id":reference.reference_id,
        "reference_sha256":reference.object_sha256,
        "reference_canonical_sha256":reference.canonical_sha256,
        "mime":reference.mime,
        "size_bytes":reference.size_bytes,
        "width":reference.width,
        "height":reference.height,
        "frame_count":reference.frame_count,
        "import_mode":reference.import_mode,
        "authorization":{"user_authorized":reference.authorization.user_authorized,"status":"observed"},
        "view_claim":{"value":Value::Null,"status":"unknown"},
        "lineage":{"project_id":reference.project_id,"reference_id":reference.reference_id,"reference_sha256":reference.object_sha256}
    })
}

fn build_lineage(
    project: &ProjectRecord,
    snapshot: Option<&SnapshotRecord>,
    candidate: Option<&CandidateRecord>,
    geometry: &GeometryContext,
    visual: &VisualContext,
    quality: &QualityContext,
    reference_id: Option<&str>,
    runtime: &Runtime,
) -> Result<Value, RuntimeError> {
    let reference = reference_id
        .map(|reference_id| runtime.reference(reference_id))
        .transpose()?
        .flatten();
    Ok(json!({
        "project_id":project.project_id,
        "project_canonical_sha256":project.canonical_sha256,
        "snapshot_id":snapshot.map(|snapshot| snapshot.snapshot_id.clone()),
        "snapshot_revision":snapshot.map(|snapshot| snapshot.revision),
        "snapshot_manifest_hash":snapshot.map(|snapshot| snapshot.manifest_hash.clone()),
        "candidate_id":candidate.map(|candidate| candidate.candidate_id.clone()),
        "candidate_canonical_sha256":candidate.map(|candidate| candidate.canonical_sha256.clone()),
        "candidate_request_sha256":candidate.map(|candidate| candidate.request_sha256.clone()),
        "reference_id":reference.as_ref().map(|reference| reference.reference_id.clone()),
        "reference_sha256":reference.as_ref().map(|reference| reference.object_sha256.clone()),
        "reference_canonical_sha256":reference.as_ref().map(|reference| reference.canonical_sha256.clone()),
        "geometry_program_sha256":geometry.evidence.as_ref().map(|evidence| evidence.geometry_program_sha256.clone()),
        "geometry_program_object_sha256":geometry.evidence.as_ref().map(|evidence| evidence.geometry_program_object_sha256.clone()),
        "artifact_sha256":geometry.artifact.as_ref().and_then(|artifact| artifact.get("artifact_id").cloned()),
        "artifact_readback_sha256":geometry.evidence.as_ref().map(|evidence| evidence.artifact_readback_object_sha256.clone()),
        "render_set_hash":visual.bundle.get("hashes").and_then(|hashes| hashes.get("render_set_hash")).cloned().unwrap_or(Value::Null),
        "comparison_report_hash":visual.bundle.get("hashes").and_then(|hashes| hashes.get("comparison_report_hash")).cloned().unwrap_or(Value::Null),
        "quality_report_hash":quality.report_hash.clone().or_else(|| visual.quality_report_hash.clone()),
        "quality_report_id":quality.report_id.clone(),
        "camera_hash":visual.bundle.get("camera").and_then(|camera| camera.get("camera_hash")).cloned().unwrap_or(Value::Null),
        "lineage_status":"hash-bound where evidence exists; missing values remain unknown"
    }))
}

fn build_scene_graph(context: &ProjectionContext) -> Value {
    let (parts, part_observed, part_unknown) = scene_parts(context);
    let materials = context
        .geometry
        .artifact
        .as_ref()
        .and_then(|artifact| artifact.get("material_zone_ids"))
        .and_then(Value::as_array)
        .map(|zones| {
            zones
                .iter()
                .filter_map(Value::as_str)
                .map(|zone| {
                    json!({
                        "material_zone_id":zone,
                        "status":"observed",
                        "channels":{"base_color":"unknown","metallic_roughness":"unknown","normal":"unknown","ao":"unknown","emissive":"unknown"},
                        "lineage":context.lineage.clone()
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let mut observed = vec![
        "project_id",
        "project_canonical_sha256",
        "candidate_selection",
    ];
    observed.extend(part_observed);
    let inferred = vec![
        "display_name_from_stable_part_id",
        "current_stage_from_evidence_gate",
    ];
    let mut unknown = vec![
        "part_role",
        "part_parent",
        "symmetry_partner",
        "part_bbox",
        "part_dimensions",
        "editability_parameters",
        "reference_view_coverage",
    ];
    unknown.extend(part_unknown);
    observed.sort_unstable();
    observed.dedup();
    unknown.sort_unstable();
    unknown.dedup();
    canonicalize(json!({
        "schema_version":"SemanticSceneGraph@1",
        "projection_status":AGENTIC_PROJECTION_STATUS,
        "read_only":true,
        "project_id":context.project.project_id,
        "candidate_id":context.candidate.as_ref().map(|candidate| candidate.candidate_id.clone()),
        "selection":{"status":"unknown","part_ids":[],"reason":"Viewer selection is not part of the authoritative Runtime read model"},
        "parts":parts,
        "materials":materials,
        "cameras":scene_cameras(context),
        "geometry":{
            "artifact_sha256":context.geometry.artifact.as_ref().and_then(|artifact| artifact.get("artifact_id")).cloned().unwrap_or(Value::Null),
            "triangle_count":context.geometry.artifact.as_ref().and_then(|artifact| artifact.get("triangle_count")).cloned().unwrap_or(Value::Null),
            "triangle_count_status":if context.geometry.artifact.is_some() {"observed"} else {"unknown"},
            "bbox":"unknown",
            "dimensions":"unknown"
        },
        "evidence":context.lineage.clone(),
        "uncertainty":{"observed":observed,"inferred":inferred,"unknown":unknown},
        "canonical_sha256":""
    }))
}

fn scene_parts(context: &ProjectionContext) -> (Vec<Value>, Vec<&'static str>, Vec<&'static str>) {
    let mut operators = BTreeMap::new();
    if let Some(program) = context.geometry.program.as_ref() {
        if let Some(nodes) = program.get("nodes").and_then(Value::as_array) {
            for node in nodes {
                if let (Some(node_id), Some(operator_id)) = (
                    node.get("node_id").and_then(Value::as_str),
                    node.get("operator_id").and_then(Value::as_str),
                ) {
                    operators.insert(node_id.to_owned(), operator_id.to_owned());
                }
            }
        }
    }
    let bindings = context
        .geometry
        .artifact
        .as_ref()
        .and_then(|artifact| artifact.get("part_bindings"))
        .and_then(Value::as_array);
    let mut parts = Vec::new();
    let mut observed = Vec::new();
    let mut unknown = Vec::new();
    if let Some(bindings) = bindings {
        for binding in bindings {
            let Some(part_id) = binding.get("part_id").and_then(Value::as_str) else {
                continue;
            };
            let source_node_id = binding.get("source_node_id").and_then(Value::as_str);
            let operator_id = source_node_id
                .and_then(|source| operators.get(source))
                .cloned();
            parts.push(json!({
                "part_id":part_id,
                "part_id_status":"observed",
                "display_name":part_id,
                "display_name_status":"inferred",
                "role":Value::Null,
                "role_status":"unknown",
                "parent_part_id":Value::Null,
                "parent_part_id_status":"unknown",
                "children":Value::Null,
                "children_status":"unknown",
                "symmetry_partner":Value::Null,
                "symmetry_partner_status":"unknown",
                "visibility":Value::Null,
                "visibility_status":"unknown",
                "source_node_id":source_node_id,
                "source_node_id_status":if source_node_id.is_some() {"observed"} else {"unknown"},
                "source_operator_id":operator_id,
                "source_operator_id_status":if operator_id.is_some() {"observed"} else {"unknown"},
                "material_zone_id":binding.get("material_zone_id"),
                "material_zone_id_status":"observed",
                "triangle_count":binding.get("triangle_count"),
                "triangle_count_status":"observed",
                "solid":binding.get("solid"),
                "solid_status":"observed",
                "bbox":Value::Null,
                "bbox_status":"unknown",
                "dimensions":Value::Null,
                "dimensions_status":"unknown",
                "editability":Value::Null,
                "editability_status":"unknown",
                "lineage":context.lineage.clone()
            }));
            observed.extend([
                "part_id",
                "source_node_id",
                "source_operator_id",
                "material_zone_id",
                "triangle_count",
            ]);
            unknown.extend([
                "part_role",
                "part_parent",
                "symmetry_partner",
                "part_bbox",
                "part_dimensions",
            ]);
        }
    } else if let Some(part_ids) = context
        .geometry
        .artifact
        .as_ref()
        .and_then(|artifact| artifact.get("part_ids"))
        .and_then(Value::as_array)
    {
        for part_id in part_ids.iter().filter_map(Value::as_str) {
            parts.push(json!({
                "part_id":part_id,
                "part_id_status":"observed",
                "display_name":part_id,
                "display_name_status":"inferred",
                "role":Value::Null,
                "role_status":"unknown",
                "source_node_id":Value::Null,
                "source_node_id_status":"unknown",
                "material_zone_id":Value::Null,
                "material_zone_id_status":"unknown",
                "triangle_count":Value::Null,
                "triangle_count_status":"unknown",
                "lineage":context.lineage.clone()
            }));
            observed.push("part_id");
            unknown.extend(["source_node_id", "material_zone_id", "triangle_count"]);
        }
    } else if context.candidate.is_some() {
        unknown.push("parts");
    }
    (parts, observed, unknown)
}

fn scene_cameras(context: &ProjectionContext) -> Vec<Value> {
    let Some(camera_hash) = context
        .visual
        .bundle
        .get("camera")
        .and_then(|camera| camera.get("camera_hash"))
        .and_then(Value::as_str)
    else {
        return Vec::new();
    };
    vec![json!({
        "camera_hash":camera_hash,
        "status":"observed",
        "kind":"candidate-bound-render-camera",
        "parameters":Value::Null,
        "parameters_status":"unknown",
        "lineage":context.lineage.clone()
    })]
}

fn build_model_understanding_bundle(context: &ProjectionContext, scene_graph: &Value) -> Value {
    canonicalize(json!({
        "schema_version":"ModelUnderstandingBundle@1",
        "projection_status":AGENTIC_PROJECTION_STATUS,
        "read_only":true,
        "project_id":context.project.project_id,
        "candidate_id":context.candidate.as_ref().map(|candidate| candidate.candidate_id.clone()),
        "semantic_scene_graph":scene_graph,
        "reference_canvas":context.reference_canvas.clone(),
        "visual_evidence_bundle":context.visual.bundle.clone(),
        "quality":context.quality.projection.clone(),
        "lineage":context.lineage.clone(),
        "uncertainty":{
            "observed":["project","candidate","artifact_readback","reference_metadata","visual_hashes"],
            "inferred":["display_names","stage_signal"],
            "unknown":["hidden_geometry","complete_reference_coverage","semantic_roles","editable_parameters"]
        },
        "canonical_sha256":""
    }))
}

fn build_stage_plan(context: &ProjectionContext) -> Value {
    let candidate = context.candidate.as_ref();
    let current_stage = if candidate.is_none() {
        "reference-canvas"
    } else if context.quality.strict_visual_gate == "passed" {
        if candidate.is_some_and(|candidate| candidate.state == "confirmed") {
            "final-review"
        } else {
            "secondary-structure"
        }
    } else {
        "primary-form"
    };
    let current_index = STAGES
        .iter()
        .position(|stage| *stage == current_stage)
        .unwrap_or(0);
    let strict_passed = context.quality.strict_visual_gate == "passed";
    let strict_failed = context.quality.strict_visual_gate == "failed";
    let stage_advance_allowed =
        candidate.is_some() && strict_passed && current_index + 1 < STAGES.len();
    let next_stage = if stage_advance_allowed {
        Some(STAGES[current_index + 1])
    } else {
        None
    };
    let (status, allowed_actions) = if candidate.is_none() {
        (
            "not-started",
            vec!["read_reference_evidence", "prepare_candidate"],
        )
    } else if strict_failed {
        (
            "blocked",
            vec![
                "inspect_failed_gate",
                "repair_bounded_part_or_camera",
                "rerun_readback_render_compare",
            ],
        )
    } else if strict_passed {
        (
            "ready-to-advance",
            vec![
                "advance_one_stage",
                "inspect_part_lineage",
                "prepare_bounded_action",
            ],
        )
    } else {
        (
            "awaiting-evidence",
            vec![
                "inspect_artifact",
                "render_reference_comparison",
                "evaluate_quality",
            ],
        )
    };
    let pbr_unlocked = strict_passed;
    let confirm_unlocked =
        strict_passed && candidate.is_some_and(|candidate| candidate.state == "reviewable");
    let export_unlocked = strict_passed
        && candidate.is_some_and(|candidate| candidate.state == "confirmed")
        && context.snapshot.as_ref().is_some_and(|snapshot| {
            snapshot.candidate_id.as_deref()
                == candidate.map(|candidate| candidate.candidate_id.as_str())
        });
    let blocked_actions = if strict_passed {
        Vec::new()
    } else {
        vec!["pbr_prepare", "candidate_confirm", "export_confirm"]
    };
    let session_id = format!(
        "design-session-projection-{}",
        &context.projection_key[..32]
    );
    canonicalize(json!({
        "schema_version":"DesignStagePlan@1",
        "projection_status":AGENTIC_PROJECTION_STATUS,
        "read_only":true,
        "durable_checkpoint":false,
        "project_id":context.project.project_id,
        "candidate_id":candidate.map(|candidate| candidate.candidate_id.clone()),
        "design_session_id":session_id,
        "stage_order":STAGES,
        "current_stage":current_stage,
        "current_stage_status":status,
        "next_stage":next_stage,
        "stage_advance_allowed":stage_advance_allowed,
        "strict_visual_gate":{
            "status":context.quality.strict_visual_gate.clone(),
            "visual_status":context.quality.visual_status.clone(),
            "comparison_status":context.visual.comparison_status.clone(),
            "reason":if strict_passed {"candidate-bound visual metrics and quality gate passed"} else if strict_failed {"strict visual evidence failed or binding is not eligible"} else {"strict visual evidence is unavailable"}
        },
        "quality_gate":{
            "structural_status":context.quality.structural_status.clone(),
            "candidate_quality_hard_gate_passed":candidate.map(|candidate| candidate.quality_hard_gate_passed).unwrap_or(false)
        },
        "unlocks":{
            "pbr":pbr_unlocked,
            "confirm":confirm_unlocked,
            "export":export_unlocked
        },
        "allowed_actions":allowed_actions,
        "blocked_actions":blocked_actions,
        "checkpoint":{"status":"not-persisted","durable":false,"reason":"DesignSession is a recomputable projection; checkpoint writes remain outside this phase"},
        "rollback":{"status":"not-available-in-projection","reason":"restore remains an existing Runtime write flow and is not invoked by observe/plan"},
        "lineage":context.lineage.clone(),
        "canonical_sha256":""
    }))
}

fn build_design_session(
    context: &ProjectionContext,
    scene_graph: &Value,
    stage_plan: &Value,
    critic: &Value,
) -> Value {
    canonicalize(json!({
        "schema_version":"DesignSession@1",
        "projection_status":AGENTIC_PROJECTION_STATUS,
        "read_only":true,
        "durable":false,
        "project_id":context.project.project_id,
        "candidate_id":context.candidate.as_ref().map(|candidate| candidate.candidate_id.clone()),
        "session_id":format!("design-session-projection-{}", &context.projection_key[..32]),
        "current_stage":stage_plan["current_stage"],
        "stage_plan_hash":stage_plan["canonical_sha256"],
        "scene_graph_hash":scene_graph["canonical_sha256"],
        "critic_report_hash":critic["canonical_sha256"],
        "checkpoint":{"status":"not-persisted","durable":false},
        "rollback_intent":{"status":"not-persisted","durable":false},
        "lineage":context.lineage.clone(),
        "canonical_sha256":""
    }))
}

fn build_critic_report(context: &ProjectionContext, stage_plan: &Value) -> Value {
    let stage = stage_plan
        .get("current_stage")
        .and_then(Value::as_str)
        .unwrap_or("reference-canvas");
    let comparison_hash = context
        .visual
        .bundle
        .get("hashes")
        .and_then(|hashes| hashes.get("comparison_report_hash"))
        .and_then(Value::as_str);
    let visual_surface = json!({
        "status":context.visual.surface_readback.get("status").cloned().unwrap_or_else(|| json!("not-run")),
        "readback_status":context.visual.surface_readback.get("status").cloned().unwrap_or_else(|| json!("not-run")),
        "readback_canonical_sha256":context.visual.surface_readback.get("canonical_sha256").cloned().unwrap_or(Value::Null),
        "surface_signal_status":context.visual.surface_readback.pointer("/surface/status").cloned().unwrap_or_else(|| json!("not-run")),
        "surface_signal_canonical_sha256":context.visual.surface_readback.pointer("/surface/canonical_sha256").cloned().unwrap_or(Value::Null),
        "binding":visual_surface_binding(&context.visual.bundle)
    });
    let metrics = context
        .visual
        .bundle
        .get("comparison_report")
        .and_then(|report| report.get("metrics"));
    let mut issues = Vec::new();
    if let Some(metrics) = metrics {
        let checks = [
            ("silhouette_iou", "min", 0.90, metrics.get("silhouette_iou")),
            (
                "boundary_f1_4px",
                "min",
                0.90,
                metrics.get("boundary_f1_4px"),
            ),
            (
                "bbox_edge_error",
                "max",
                0.02,
                metrics.get("bbox_edge_error"),
            ),
            ("centroid_error", "max", 0.02, metrics.get("centroid_error")),
            (
                "landmark_coverage",
                "min",
                0.80,
                metrics.get("landmark_coverage"),
            ),
            ("landmark_nme", "max", 0.03, metrics.get("landmark_nme")),
            (
                "region_median_iou",
                "min",
                0.85,
                metrics.get("region_median_iou"),
            ),
            (
                "critical_region_min_iou",
                "min",
                0.85,
                metrics.get("critical_region_min_iou"),
            ),
        ];
        for (metric_name, direction, threshold, observed) in checks {
            let Some(observed) = observed.and_then(Value::as_f64) else {
                continue;
            };
            let failed = if direction == "min" {
                observed < threshold
            } else {
                observed > threshold
            };
            if failed {
                let issue_key = canonical_json_hash(
                    &json!({"candidate_id":context.candidate.as_ref().map(|candidate| candidate.candidate_id.clone()),"metric_name":metric_name,"observed":observed,"threshold":threshold,"evidence_hash":comparison_hash}),
                );
                issues.push(json!({
                    "issue_id":format!("issue-{}", &issue_key[..24]),
                    "stage":stage,
                    "part_id":Value::Null,
                    "part_id_status":"unknown",
                    "material_zone_id":Value::Null,
                    "material_zone_id_status":"unknown",
                    "metric_name":metric_name,
                    "threshold":threshold,
                    "observed":observed,
                    "evidence_hash":comparison_hash,
                    "proposed_bounded_action":{"operation":"reference_compare_prepare","scope":"candidate-wide-evidence-recheck","part_id":Value::Null,"parameters":{},"allowed":true},
                    "risk":"A global visual metric does not identify a single editable Part; do not apply an unbound geometry patch.",
                    "status":"fail",
                    "knowledge_state":"observed",
                    "lineage":context.lineage.clone()
                }));
            }
        }
    }
    if let Some(part_error) = context.visual.part_error.as_ref() {
        let part_error_hash = part_error
            .get("canonical_sha256")
            .and_then(Value::as_str)
            .map(|hash| Value::String(hash.to_owned()))
            .unwrap_or(Value::Null);
        let boundary_threshold = 4.0 / 512.0;
        if let Some(parts) = part_error.get("parts").and_then(Value::as_array) {
            for part in parts {
                let part_id = part.get("part_id").and_then(Value::as_str);
                let status = part.get("status").and_then(Value::as_str);
                let boundary_error_px = part.get("boundary_error_px").and_then(Value::as_f64);
                let (Some(part_id), Some("ready"), Some(boundary_error_px)) =
                    (part_id, status, boundary_error_px)
                else {
                    continue;
                };
                if boundary_error_px <= 4.0 {
                    continue;
                }
                let observed = (boundary_error_px / 512.0).clamp(0.0, 1.0);
                let issue_key = canonical_json_hash(&json!({
                    "candidate_id":context.candidate.as_ref().map(|candidate| candidate.candidate_id.clone()),
                    "part_id":part_id,
                    "metric_name":"part_boundary_error_normalized",
                    "observed":observed,
                    "threshold":boundary_threshold,
                    "evidence_hash":part_error_hash.clone()
                }));
                issues.push(json!({
                    "issue_id":format!("issue-{}", &issue_key[..24]),
                    "stage":stage,
                    "part_id":part_id,
                    "part_id_status":"observed",
                    "material_zone_id":Value::Null,
                    "material_zone_id_status":"unknown",
                    "metric_name":"part_boundary_error_normalized",
                    "threshold":boundary_threshold,
                    "observed":observed,
                    "evidence_hash":part_error_hash.clone(),
                    "proposed_bounded_action":{"operation":"part_contour_fit_prepare","scope":"part-boundary-error","part_id":part_id,"parameters":{},"allowed":true},
                    "risk":"The observed contour error is scoped to one Part; a new candidate and explicit user approval are still required.",
                    "status":"fail",
                    "knowledge_state":"observed",
                    "lineage":context.lineage.clone()
                }));
            }
        }
    }
    if context.candidate.is_none() {
        issues.push(json!({
            "issue_id":"candidate-missing",
            "stage":"reference-canvas",
            "part_id":Value::Null,
            "part_id_status":"unknown",
            "metric_name":"candidate_presence",
            "threshold":"candidate required",
            "observed":Value::Null,
            "evidence_hash":Value::Null,
            "proposed_bounded_action":{"operation":"candidate_prepare","scope":"project","part_id":Value::Null,"parameters":{},"allowed":false},
            "risk":"No candidate exists to inspect or repair.",
            "status":"unknown",
            "knowledge_state":"unknown",
            "material_zone_id":Value::Null,
            "material_zone_id_status":"unknown",
            "lineage":context.lineage.clone()
        }));
    } else if context
        .visual
        .bundle
        .get("available")
        .and_then(Value::as_bool)
        != Some(true)
        && context.quality.strict_visual_gate != "passed"
    {
        issues.push(json!({
            "issue_id":"visual-evidence-missing",
            "stage":stage_plan.get("current_stage"),
            "part_id":Value::Null,
            "part_id_status":"unknown",
            "metric_name":"strict_visual_gate",
            "threshold":"PARTIAL_VISIBLE_VIEW_PASS",
            "observed":Value::Null,
            "evidence_hash":context.quality.report_hash,
            "proposed_bounded_action":{"operation":"reference_compare_prepare","scope":"candidate","part_id":Value::Null,"parameters":{},"allowed":true},
            "risk":"PBR, confirm and export remain locked until candidate-bound visual evidence exists.",
            "status":"unknown",
            "knowledge_state":"unknown",
            "material_zone_id":Value::Null,
            "material_zone_id_status":"unknown",
            "lineage":context.lineage.clone()
        }));
    }
    if context
        .candidate
        .as_ref()
        .is_some_and(|candidate| candidate.state == "failed")
    {
        issues.push(json!({
            "issue_id":"candidate-quality-failed",
            "stage":stage,
            "part_id":Value::Null,
            "part_id_status":"unknown",
            "metric_name":"candidate_quality_hard_gate",
            "threshold":true,
            "observed":false,
            "evidence_hash":context.quality.report_hash,
            "proposed_bounded_action":{"operation":"rerun_readback_render_compare","scope":"candidate","part_id":Value::Null,"parameters":{},"allowed":false},
            "risk":"A failed candidate cannot be advanced or confirmed by a projection.",
            "status":"fail",
            "knowledge_state":"observed",
            "material_zone_id":Value::Null,
            "material_zone_id_status":"unknown",
            "lineage":context.lineage.clone()
        }));
    }
    let critic_status = if context.quality.strict_visual_gate == "passed" && issues.is_empty() {
        "passed"
    } else if issues
        .iter()
        .any(|issue| issue.get("status").and_then(Value::as_str) == Some("fail"))
    {
        "action-required"
    } else {
        "unknown"
    };
    let repair_intents = issues
        .iter()
        // A global comparison metric is useful critic evidence, but it does
        // not identify an editable Part or MaterialZone.  Only the optional
        // hash-bound PartError projection below can provide the scope needed
        // for a proposed RepairIntent.  The intent remains projection-only;
        // the typed prepare/approval flow still owns every write.
        .filter(|issue| issue_has_editable_scope(issue))
        .filter_map(|issue| {
            let issue_id = issue.get("issue_id").and_then(Value::as_str)?;
            let repair_key = canonical_json_hash(&json!({"issue_id":issue_id,"lineage":context.lineage.clone()}));
            let part_id = issue.get("part_id").cloned().unwrap_or(Value::Null);
            let part_id_status = issue
                .get("part_id_status")
                .cloned()
                .unwrap_or_else(|| json!("unknown"));
            let material_zone_id = issue
                .get("material_zone_id")
                .cloned()
                .unwrap_or(Value::Null);
            let material_zone_id_status = issue
                .get("material_zone_id_status")
                .cloned()
                .unwrap_or_else(|| json!("unknown"));
            Some(json!({
                "repair_intent_id":format!("repair-{}", &repair_key[..24]),
                "stage":stage,
                "issue_ids":[issue_id],
                "scope":{"part_id":part_id,"part_id_status":part_id_status,"material_zone_id":material_zone_id,"material_zone_id_status":material_zone_id_status},
                "bounded_action":{"operation":issue.get("proposed_bounded_action").and_then(|action| action.get("operation")),"parameters":{},"requires_new_candidate":true,"arbitrary_script":false},
                "status":"proposed",
                "projection_only":true,
                "execution_allowed":false,
                "reason":"RepairIntent is evidence-bound planning output; the existing typed prepare/approval flow must execute any change.",
                "lineage":context.lineage.clone()
            }))
        })
        .collect::<Vec<_>>();
    canonicalize(json!({
        "schema_version":"DesignCriticReport@1",
        "projection_status":AGENTIC_PROJECTION_STATUS,
        "read_only":true,
        "project_id":context.project.project_id,
        "candidate_id":context.candidate.as_ref().map(|candidate| candidate.candidate_id.clone()),
        "stage":stage,
        "status":critic_status,
        "issues":issues,
        "repair_intents":repair_intents,
        "part_error":context.visual.part_error.clone().unwrap_or(Value::Null),
        "visual_surface":visual_surface,
        "strict_visual_gate":context.quality.strict_visual_gate.clone(),
        "lineage":context.lineage.clone(),
        "canonical_sha256":""
    }))
}

fn issue_has_editable_scope(issue: &Value) -> bool {
    issue
        .get("part_id")
        .and_then(Value::as_str)
        .is_some_and(|part_id| !part_id.is_empty())
        || issue
            .get("material_zone_id")
            .and_then(Value::as_str)
            .is_some_and(|material_zone_id| !material_zone_id.is_empty())
}

fn quality_projection(
    report: Option<Value>,
    report_hash: Option<String>,
    structural_status: &str,
    visual_status: &str,
    strict_visual_gate: &str,
    candidate: &CandidateRecord,
) -> Value {
    canonicalize(json!({
        "schema_version":"AgenticQualityProjection@1",
        "projection_status":AGENTIC_PROJECTION_STATUS,
        "read_only":true,
        "candidate_id":candidate.candidate_id,
        "quality_report":report,
        "quality_report_hash":report_hash,
        "quality_report_id":candidate.quality_report_id,
        "structural_status":structural_status,
        "visual_status":visual_status,
        "candidate_quality_hard_gate_passed":candidate.quality_hard_gate_passed,
        "strict_visual_gate":strict_visual_gate,
        "pbr_unlocked":strict_visual_gate == "passed",
        "confirm_unlocked":strict_visual_gate == "passed" && candidate.state == "reviewable",
        "export_unlocked":false,
        "canonical_sha256":""
    }))
}

fn unknown_quality_projection(
    report: Option<Value>,
    report_hash: Option<String>,
    structural_status: &str,
    visual_status: &str,
    strict_visual_gate: &str,
) -> Value {
    canonicalize(json!({
        "schema_version":"AgenticQualityProjection@1",
        "projection_status":AGENTIC_PROJECTION_STATUS,
        "read_only":true,
        "candidate_id":Value::Null,
        "quality_report":report,
        "quality_report_hash":report_hash,
        "quality_report_id":Value::Null,
        "structural_status":structural_status,
        "visual_status":visual_status,
        "candidate_quality_hard_gate_passed":false,
        "strict_visual_gate":strict_visual_gate,
        "pbr_unlocked":false,
        "confirm_unlocked":false,
        "export_unlocked":false,
        "canonical_sha256":""
    }))
}

fn read_json_object(runtime: &Runtime, hash: &str, label: &str) -> Result<Value, RuntimeError> {
    if !is_sha256(hash) {
        return Err(binding_error(&format!("{label} hash is invalid")));
    }
    let value: Value = serde_json::from_slice(&runtime.cas_read(hash)?).map_err(|error| {
        RuntimeError::InvalidInput(format!("AGENTIC_PROJECTION_INVALID: {label}: {error}"))
    })?;
    if !value.is_object() {
        return Err(binding_error(&format!("{label} is not an object")));
    }
    Ok(value)
}

fn canonicalize(mut value: Value) -> Value {
    if value.is_object() {
        value["canonical_sha256"] = Value::String(String::new());
        let hash = canonical_json_hash(&value);
        let object = value
            .as_object_mut()
            .expect("object was checked before canonical hashing");
        object.insert("canonical_sha256".to_owned(), Value::String(hash));
    }
    value
}

fn binding_error(message: &str) -> RuntimeError {
    RuntimeError::InvalidInput(format!("AGENTIC_BINDING_FAIL_CLOSED: {message}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use forgecad_contracts::{
        ReferenceAuthorization, ReferenceImportRequest, ReferenceImportSource,
    };

    const REFERENCE_PNG: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=";

    fn project(runtime: &Runtime) -> ProjectRecord {
        runtime
            .create_project("agentic projection test", json!({"profile":"mvp"}))
            .expect("project")
    }

    fn import_reference(runtime: &Runtime, project_id: &str) -> ReferenceEvidenceRecord {
        runtime
            .import_reference(&ReferenceImportRequest {
                project_id: project_id.to_owned(),
                source: ReferenceImportSource::InlineContent {
                    mime: "image/png".to_owned(),
                    content_base64: REFERENCE_PNG.to_owned(),
                },
                authorization: ReferenceAuthorization {
                    user_authorized: true,
                    declaration: "authorized test reference".to_owned(),
                },
                expected_sha256: None,
            })
            .expect("reference")
            .reference
    }

    #[test]
    fn scene_observe_without_candidate_is_read_only_and_unknown() {
        let runtime = Runtime::ephemeral().expect("runtime");
        let project = project(&runtime);
        let result = runtime
            .agentic_scene_observe(&project.project_id, None)
            .expect("projection");
        assert_eq!(result["projection_status"], AGENTIC_PROJECTION_STATUS);
        assert_eq!(result["candidate_id"], Value::Null);
        assert_eq!(result["design_session"]["durable"], false);
        assert_eq!(
            result["design_stage_plan"]["current_stage"],
            "reference-canvas"
        );
        assert_eq!(
            result["design_stage_plan"]["strict_visual_gate"]["status"],
            "unknown"
        );
        assert_eq!(result["design_stage_plan"]["unlocks"]["pbr"], false);
        assert_eq!(result["visual_evidence_bundle"]["status"], "unknown");
        assert!(result["canonical_sha256"].as_str().is_some_and(is_sha256));
    }

    #[test]
    fn structural_quality_pass_without_visual_evidence_stays_unknown_and_locked() {
        let runtime = Runtime::ephemeral().expect("runtime");
        let project = project(&runtime);
        let prepared = runtime
            .prepare_diagnostic_candidate(
                &project.project_id,
                None,
                json!({"typed":"diagnostic","label":"agentic-test"}),
            )
            .expect("candidate");
        let result = runtime
            .agentic_stage_plan(&project.project_id, Some(&prepared.candidate.candidate_id))
            .expect("stage plan");
        assert_eq!(result["quality_gate"]["structural_status"], "passed");
        assert_eq!(result["strict_visual_gate"]["status"], "unknown");
        assert_eq!(result["unlocks"]["pbr"], false);
        assert_eq!(result["unlocks"]["confirm"], false);
        assert_eq!(result["unlocks"]["export"], false);
    }

    #[test]
    fn failed_visual_quality_does_not_advance_stage_or_unlock_downstream_actions() {
        let runtime = Runtime::ephemeral().expect("runtime");
        let project = project(&runtime);
        let reference = import_reference(&runtime, &project.project_id);
        let mut program = json!({
            "schema_version":"GeometryProgram@2",
            "project_id":project.project_id,
            "representation_plan_sha256":"a".repeat(64),
            "operator_catalog_sha256":super::super::operator_catalog_sha256(),
            "units":{"length":"meter","angle":"radian","coordinate_system":"right-handed-y-up"},
            "budgets":{"max_nodes":1,"max_triangles":1000,"max_glb_bytes":1048576,"max_worker_memory_bytes":536870912,"max_runtime_ms":10000},
            "nodes":[{"node_id":"body","operator_id":"forgecad.geometry.primitive@2","inputs":[],"parameters":{"shape":"box","size_m":[1.0,1.0,0.6],"position_m":[0.0,0.5,0.0],"rotation_rad":[0.0,0.0,0.0]}}],
            "part_outputs":[{"part_id":"body","input_node_ids":["body"],"material_zone_id":"zone-white-shell","solid":true}]
        });
        program["canonical_sha256"] = Value::String(canonical_json_hash(&program));
        let prepared = runtime
            .prepare_geometry_candidate(
                &project.project_id,
                None,
                json!({"typed":"geometry","geometry_program":program}),
            )
            .expect("candidate");
        let candidate_id = prepared["candidate"]["candidate_id"]
            .as_str()
            .expect("candidate id");
        let mut view_spec = json!({
            "schema_version":"ReferenceViewSpec@1",
            "reference_id":reference.reference_id,
            "reference_sha256":reference.object_sha256,
            "view_id":"agentic-test-view",
            "source_view":"three-quarter",
            "image":{"width":1,"height":1,"rotation_degrees":0.0,"crop":{"x":0.0,"y":0.0,"width":1.0,"height":1.0}},
            "landmarks":[],"regions":[],"canonical_sha256":""
        });
        view_spec["canonical_sha256"] = Value::String(canonical_json_hash(&view_spec));
        runtime
            .prepare_reference_comparison(
                &project.project_id,
                json!({"candidate_id":candidate_id,"reference_id":reference.reference_id,"view_spec":view_spec}),
            )
            .expect("comparison");
        let result = runtime
            .agentic_stage_plan(&project.project_id, Some(candidate_id))
            .expect("stage plan");
        assert_eq!(result["current_stage"], "primary-form");
        assert_eq!(result["current_stage_status"], "blocked");
        assert_eq!(result["strict_visual_gate"]["status"], "failed");
        assert_eq!(result["unlocks"]["pbr"], false);
        assert_eq!(result["unlocks"]["confirm"], false);
        assert_eq!(result["unlocks"]["export"], false);
    }

    #[test]
    fn cross_candidate_visual_evidence_binding_fails_closed() {
        let runtime = Runtime::ephemeral().expect("runtime");
        let current_project = project(&runtime);
        let second = runtime
            .prepare_diagnostic_candidate(
                &current_project.project_id,
                None,
                json!({"typed":"diagnostic","label":"second"}),
            )
            .expect("second candidate");
        let foreign_project = project(&runtime);
        let foreign_reference = import_reference(&runtime, &foreign_project.project_id);
        let render_object = runtime
            .put_object(
                br#"{"kind":"render"}"#,
                None,
                "application/json",
                "agentic-test-render",
            )
            .expect("render object");
        let quality_object = runtime
            .put_object(
                br#"{"kind":"quality"}"#,
                None,
                "application/json",
                "agentic-test-quality",
            )
            .expect("quality object");
        runtime
            .store
            .upsert_visual_evidence(&super::super::VisualEvidenceRecord {
                candidate_id: second.candidate.candidate_id.clone(),
                project_id: foreign_project.project_id.clone(),
                reference_id: foreign_reference.reference_id,
                render_set_object_sha256: render_object.record.sha256,
                comparison_report_object_sha256: None,
                visual_review_object_sha256: None,
                quality_report_object_sha256: quality_object.record.sha256,
                human_receipt_object_sha256: None,
                created_at: "test".to_owned(),
                updated_at: "test".to_owned(),
            })
            .expect("test evidence");
        let error = runtime
            .agentic_scene_observe(
                &current_project.project_id,
                Some(&second.candidate.candidate_id),
            )
            .expect_err("cross-candidate evidence must fail closed");
        assert!(error.to_string().contains("AGENTIC_BINDING_FAIL_CLOSED"));
    }

    #[test]
    fn reference_projection_never_returns_original_bytes_or_path() {
        let runtime = Runtime::ephemeral().expect("runtime");
        let project = project(&runtime);
        let reference = import_reference(&runtime, &project.project_id);
        let result = runtime
            .agentic_scene_observe(&project.project_id, None)
            .expect("projection");
        let references = result["reference_canvas"]["references"]
            .as_array()
            .expect("references");
        assert_eq!(references.len(), 1);
        assert_eq!(references[0]["reference_sha256"], reference.object_sha256);
        assert!(references[0].get("content_base64").is_none());
        assert!(references[0].get("path").is_none());
    }

    #[test]
    fn global_critic_issue_cannot_become_a_repair_intent_without_scope() {
        assert!(!issue_has_editable_scope(&json!({
            "part_id": null,
            "part_id_status": "unknown",
            "material_zone_id": null,
            "material_zone_id_status": "unknown"
        })));
        assert!(issue_has_editable_scope(&json!({
            "part_id": "chest-shell",
            "part_id_status": "observed",
            "material_zone_id": null,
            "material_zone_id_status": "unknown"
        })));
        assert!(issue_has_editable_scope(&json!({
            "part_id": null,
            "part_id_status": "unknown",
            "material_zone_id": "zone-white-shell",
            "material_zone_id_status": "observed"
        })));
    }

    fn visual_surface_request(project_id: &str, candidate_id: &str) -> Value {
        let mut request = json!({
            "schema_version":"VisualSurfaceRequest@1",
            "project_id":project_id,
            "candidate_id":candidate_id,
            "requested_signals":["silhouette","boundary"],
            "expected_binding":{
                "reference_id":null,
                "reference_sha256":null,
                "artifact_sha256":null,
                "render_set_hash":null,
                "camera_hash":null,
                "comparison_report_hash":null,
                "quality_report_hash":null
            },
            "target_sha256":null,
            "max_part_errors":8,
            "canonical_sha256":""
        });
        request["canonical_sha256"] = Value::String(canonical_json_hash(&request));
        request
    }

    #[test]
    fn visual_surface_projection_is_blocked_without_candidate_bound_evidence() {
        let runtime = Runtime::ephemeral().expect("runtime");
        let project = project(&runtime);
        let candidate = runtime
            .prepare_diagnostic_candidate(
                &project.project_id,
                None,
                json!({"typed":"diagnostic","label":"surface-boundary-test"}),
            )
            .expect("candidate");
        let result = runtime
            .visual_surface_get(visual_surface_request(
                &project.project_id,
                &candidate.candidate.candidate_id,
            ))
            .expect("surface projection");
        assert_eq!(result["schema_version"], "VisualSurfaceResult@1");
        assert_eq!(result["status"], "blocked");
        assert_eq!(result["surface_program_status"], "not-run");
        assert_eq!(result["binding"]["camera_hash"], Value::Null);
        assert!(result["unknowns"]
            .as_array()
            .expect("unknowns")
            .iter()
            .any(|value| value == "surface-program-not-run"));
    }

    #[test]
    fn visual_surface_readback_decodes_candidate_bound_render_set() {
        let runtime = Runtime::ephemeral().expect("runtime");
        let project = project(&runtime);
        let reference = import_reference(&runtime, &project.project_id);
        let mut program = json!({
            "schema_version":"GeometryProgram@2",
            "project_id":project.project_id,
            "representation_plan_sha256":"b".repeat(64),
            "operator_catalog_sha256":super::super::operator_catalog_sha256(),
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
            .expect("geometry candidate");
        let candidate_id = prepared["candidate"]["candidate_id"]
            .as_str()
            .expect("candidate ID")
            .to_owned();
        let mut view_spec = json!({
            "schema_version":"ReferenceViewSpec@1",
            "reference_id":reference.reference_id,
            "reference_sha256":reference.object_sha256,
            "view_id":"visual-surface-readback-view",
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
        let result = runtime
            .visual_surface_get(visual_surface_request(&project.project_id, &candidate_id))
            .expect("visual surface readback");
        assert_eq!(result["status"], "ready");
        assert_eq!(result["readback"]["status"], "ready");
        assert_eq!(result["readback"]["resolution"], json!([512, 512]));
        assert_eq!(result["readback"]["aov"]["status"], "ready");
        assert_eq!(
            result["readback"]["aov"]["passes"]
                .as_array()
                .expect("AOV rows")
                .len(),
            9
        );
        assert_eq!(result["readback"]["aov"]["missing_passes"], json!([]));
        assert_eq!(result["readback"]["reference_mask"]["decoded"], true);
        assert_eq!(result["readback"]["candidate_mask"]["decoded"], true);
        assert_eq!(result["readback"]["edge"]["status"], "ready");
        assert!(result["readback"]["roi"]["part_id_sha256"]
            .as_str()
            .is_some_and(is_sha256));
        assert!(result["readback"]["roi"]["parts"]
            .as_array()
            .is_some_and(|parts| !parts.is_empty()));
        let mut surface_request = visual_surface_request(&project.project_id, &candidate_id);
        surface_request["requested_signals"] = json!(["curvature", "feature-line"]);
        surface_request["canonical_sha256"] = Value::String(String::new());
        surface_request["canonical_sha256"] = Value::String(canonical_json_hash(&surface_request));
        let surface_result = runtime
            .visual_surface_get(surface_request)
            .expect("mesh-derived surface signals");
        assert_eq!(
            surface_result["backend"],
            "candidate-bound-surface-analysis@1"
        );
        assert_eq!(surface_result["surface_program_status"], "ready");
        assert_eq!(
            surface_result["available_signals"],
            json!(["curvature", "feature-line"])
        );
        assert_eq!(surface_result["unsupported_signals"], json!([]));
        assert_eq!(surface_result["readback"]["surface"]["status"], "ready");
        assert_eq!(
            surface_result["readback"]["surface"]["curvature"]["status"],
            "ready"
        );
        assert_eq!(
            surface_result["readback"]["surface"]["feature_line"]["status"],
            "ready"
        );
        let mut readback_for_hash = result["readback"].clone();
        readback_for_hash["canonical_sha256"] = Value::String(String::new());
        assert_eq!(
            result["readback"]["canonical_sha256"],
            Value::String(canonical_json_hash(&readback_for_hash))
        );
        assert_eq!(
            result["binding"]["render_set_hash"],
            visual["render_set_object_sha256"]
        );
        let critic = runtime
            .agentic_critic_projection(&project.project_id, Some(&candidate_id), None)
            .expect("critic projection");
        assert_eq!(critic["visual_surface"]["status"], "ready");
        assert_eq!(critic["visual_surface"]["readback_status"], "ready");
        assert_eq!(critic["visual_surface"]["surface_signal_status"], "ready");
        assert_eq!(
            critic["visual_surface"]["readback_canonical_sha256"],
            result["readback"]["canonical_sha256"]
        );
        assert_eq!(
            critic["visual_surface"]["surface_signal_canonical_sha256"],
            result["readback"]["surface"]["canonical_sha256"]
        );
        assert_eq!(
            critic["visual_surface"]["binding"]["render_set_hash"],
            result["binding"]["render_set_hash"]
        );
    }

    #[test]
    fn visual_surface_request_rejects_duplicate_signal_and_binding_mismatch() {
        let runtime = Runtime::ephemeral().expect("runtime");
        let project = project(&runtime);
        let candidate = runtime
            .prepare_diagnostic_candidate(
                &project.project_id,
                None,
                json!({"typed":"diagnostic","label":"surface-negative-test"}),
            )
            .expect("candidate");
        let mut duplicate =
            visual_surface_request(&project.project_id, &candidate.candidate.candidate_id);
        duplicate["requested_signals"] = json!(["silhouette", "silhouette"]);
        duplicate["canonical_sha256"] = Value::String(canonical_json_hash(&duplicate));
        let duplicate_error = runtime
            .visual_surface_get(duplicate)
            .expect_err("duplicate signals must fail closed");
        assert!(duplicate_error
            .to_string()
            .contains("VISUAL_SURFACE_REQUEST_INVALID"));

        let mut mismatch =
            visual_surface_request(&project.project_id, &candidate.candidate.candidate_id);
        mismatch["expected_binding"]["artifact_sha256"] = Value::String("f".repeat(64));
        mismatch["canonical_sha256"] = Value::String(String::new());
        mismatch["canonical_sha256"] = Value::String(canonical_json_hash(&mismatch));
        let mismatch_error = runtime
            .visual_surface_get(mismatch)
            .expect_err("binding mismatch must fail closed");
        assert!(mismatch_error
            .to_string()
            .contains("AGENTIC_BINDING_FAIL_CLOSED"));
    }
}
