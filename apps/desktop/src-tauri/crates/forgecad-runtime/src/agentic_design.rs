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
    ) -> Result<Value, RuntimeError> {
        let context = build_context(self, project_id, candidate_id)?;
        let stage_plan = build_stage_plan(&context);
        Ok(build_critic_report(&context, &stage_plan))
    }
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
    let reference_canvas = build_reference_canvas(runtime, project_id, reference_id.as_deref())?;
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

    Ok(ProjectionContext {
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
    })
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

    let candidates = runtime.candidates(project_id)?;
    match candidates.as_slice() {
        [] => Ok((None, "latest_candidate_when_no_active_snapshot")),
        [candidate] => Ok((
            Some(candidate.clone()),
            "latest_candidate_when_no_active_snapshot",
        )),
        _ => Err(binding_error(
            "multiple candidates require an explicit candidate_id or active snapshot",
        )),
    }
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
    if let Some(target_sha256) = evidence.target_sha256.as_deref() {
        let target = runtime.read_silhouette_target(target_sha256)?;
        if target.get("reference_id").and_then(Value::as_str)
            != Some(evidence.reference_id.as_str())
            || target.get("reference_sha256").and_then(Value::as_str)
                != Some(reference.object_sha256.as_str())
        {
            return Err(binding_error(
                "silhouette target is not bound to the visual reference",
            ));
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
    let bundle = canonicalize(json!({
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
            "target_sha256":evidence.target_sha256,
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
            "target_sha256":evidence.target_sha256,
            "render_set_hash":evidence.render_set_object_sha256,
            "comparison_report_hash":evidence.comparison_report_object_sha256,
            "quality_report_hash":evidence.quality_report_object_sha256
        },
        "canonical_sha256":""
    }));
    Ok(VisualContext {
        bundle,
        reference_id: Some(evidence.reference_id),
        quality_report: Some(quality_report),
        quality_report_hash: Some(evidence.quality_report_object_sha256),
        comparison_status,
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
                "target_sha256":Value::Null,
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
    Ok(canonicalize(json!({
        "schema_version":"ReferenceCanvasProjection@1",
        "projection_status":AGENTIC_PROJECTION_STATUS,
        "read_only":true,
        "project_id":project_id,
        "selected_reference_id":selected_reference_id,
        "references":reference_values,
        "coverage":{"status":"unknown","observed_views":[],"missing_views":["front","back","left","right","top","three-quarter"],"reason":"ReferenceEvidence does not carry a complete view-coverage contract"},
        "unknowns":["view_coverage","reference_camera_claim","hidden_geometry","material_detail_coverage"],
        "canonical_sha256":""
    })))
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
    let metrics = context
        .visual
        .bundle
        .get("comparison_report")
        .and_then(|report| report.get("metrics"));
    let mut issues = Vec::new();
    let mut failed_metrics = Vec::new();
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
                failed_metrics.push(json!({
                    "metric_name":metric_name,
                    "direction":direction,
                    "observed":observed,
                    "threshold":threshold,
                    "evidence_hash":comparison_hash
                }));
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
                    "proposed_bounded_action":{"operation":if context.visual.bundle.get("hashes").and_then(|hashes| hashes.get("target_sha256")).and_then(Value::as_str).is_some() {"silhouette_part_error_get"} else {"reference_compare_prepare"},"scope":if context.visual.bundle.get("hashes").and_then(|hashes| hashes.get("target_sha256")).and_then(Value::as_str).is_some() {"candidate-bound-target-diagnostic"} else {"candidate-wide-evidence-recheck"},"part_id":Value::Null,"parameters":context.visual.bundle.get("hashes").and_then(|hashes| hashes.get("target_sha256")).and_then(Value::as_str).map(|target| json!({"target_sha256":target})).unwrap_or_else(|| json!({})),"allowed":true},
                    "risk":"A global visual metric does not identify a single editable Part; do not apply an unbound geometry patch.",
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
    let target_sha256 = context
        .visual
        .bundle
        .get("hashes")
        .and_then(|hashes| hashes.get("target_sha256"))
        .and_then(Value::as_str)
        .map(str::to_owned);
    let target_sha_value = target_sha256
        .clone()
        .map(Value::String)
        .unwrap_or(Value::Null);
    let primary_form_directive = json!({
        "status":if failed_metrics.is_empty() {
            if context.quality.strict_visual_gate == "passed" {"passed"} else {"unknown"}
        } else if target_sha256.is_some() {"ready"} else {"requires-target"},
        "owner":"runtime",
        "objective":"primary-form-convergence",
        "metric_priority":["boundary_f1_4px","silhouette_iou","bbox_edge_error","centroid_error","landmark_coverage","landmark_nme","region_median_iou","critical_region_min_iou"],
        "failed_metrics":failed_metrics,
        "target_sha256":target_sha_value,
        "diagnostic_operation":if target_sha256.is_some() {"silhouette_part_error_get"} else if metrics.is_some() {"reference_compare_prepare"} else {"none"},
        // `silhouette_fit_prepare` remains a Runtime read-only primitive, but
        // it is not the Codex-facing next action. Exposing it here would split
        // the observation back into a caller-steered search. The Runtime-owned
        // Primary Form action consumes the same bounded intent and closes
        // fit -> compile -> readback -> render -> compare itself.
        "repair_operation":if target_sha256.is_some() {"primary_form_repair_prepare"} else {"reference_compare_prepare"},
        "continuous_search_owner":"runtime",
        "execution_allowed":false
    });
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
    let repair_intents = if !failed_metrics.is_empty() {
        let issue_ids = issues
            .iter()
            .filter(|issue| issue.get("status").and_then(Value::as_str) == Some("fail"))
            .filter_map(|issue| issue.get("issue_id").and_then(Value::as_str))
            .take(8)
            .collect::<Vec<_>>();
        let repair_key =
            canonical_json_hash(&json!({"issue_ids":issue_ids,"lineage":context.lineage.clone()}));
        vec![json!({
            "repair_intent_id":format!("repair-{}", &repair_key[..24]),
            "stage":stage,
            "issue_ids":issue_ids,
            "scope":{"part_id":Value::Null,"part_id_status":"unknown","material_zone_id":Value::Null,"material_zone_id_status":"unknown"},
            "bounded_action":{"operation":primary_form_directive["repair_operation"],"parameters":target_sha256.as_ref().map(|target| json!({"target_sha256":target})).unwrap_or_else(|| json!({})),"requires_new_candidate":true,"arbitrary_script":false},
            "status":"proposed",
            "projection_only":true,
            "execution_allowed":false,
            "reason":"One Runtime-owned bounded Primary Form action covers the priority-ordered failures; Codex selects/approves the next action but does not search continuous parameters.",
            "lineage":context.lineage.clone()
        })]
    } else {
        issues
            .iter()
            .filter_map(|issue| {
                let issue_id = issue.get("issue_id").and_then(Value::as_str)?;
                let repair_key = canonical_json_hash(&json!({"issue_id":issue_id,"lineage":context.lineage.clone()}));
                Some(json!({
                    "repair_intent_id":format!("repair-{}", &repair_key[..24]),
                    "stage":stage,
                    "issue_ids":[issue_id],
                    "scope":{"part_id":Value::Null,"part_id_status":"unknown","material_zone_id":Value::Null,"material_zone_id_status":"unknown"},
                    "bounded_action":{"operation":issue.get("proposed_bounded_action").and_then(|action| action.get("operation")),"parameters":{},"requires_new_candidate":true,"arbitrary_script":false},
                    "status":"proposed",
                    "projection_only":true,
                    "execution_allowed":false,
                    "reason":"RepairIntent is evidence-bound planning output; the existing typed prepare/approval flow must execute any change.",
                    "lineage":context.lineage.clone()
                }))
            })
            .collect::<Vec<_>>()
    };
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
        "primary_form_directive":primary_form_directive,
        "strict_visual_gate":context.quality.strict_visual_gate.clone(),
        "lineage":context.lineage.clone(),
        "canonical_sha256":""
    }))
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
    fn scene_observe_without_binding_rejects_ambiguous_candidates() {
        let runtime = Runtime::ephemeral().expect("runtime");
        let project = project(&runtime);
        runtime
            .prepare_diagnostic_candidate(
                &project.project_id,
                None,
                json!({"typed":"diagnostic","label":"first"}),
            )
            .expect("first candidate");
        runtime
            .prepare_diagnostic_candidate(
                &project.project_id,
                None,
                json!({"typed":"diagnostic","label":"second"}),
            )
            .expect("second candidate");

        let error = runtime
            .agentic_scene_observe(&project.project_id, None)
            .expect_err("ambiguous observation must fail closed");
        assert!(error
            .to_string()
            .contains("multiple candidates require an explicit candidate_id or active snapshot"));
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
                target_sha256: None,
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
}
