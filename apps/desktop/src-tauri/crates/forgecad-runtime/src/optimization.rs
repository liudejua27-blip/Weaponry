//! Runtime-owned CADFit-style multi-fidelity optimization.
//!
//! The optimizer is deliberately a bounded seed-then-adaptive trust-region
//! search over a typed SilhouetteRig for one candidate-bound Part.  Codex
//! chooses the intent and the user approves it; Runtime owns every
//! compile/render/metric evaluation, the durable RuntimeJob, best-so-far
//! checkpoints and the final proposal.  A Boolean residual is a proposal
//! lane: candidate zero remains the unmodified baseline, while subsequent
//! candidates carry a bounded family derived from the approved residual.
//! No candidate, version, confirmation or export is changed here.

use super::{
    canonical_json_bytes, canonical_json_hash, decode_binary_mask,
    decode_binary_mask_at_resolution, downsample_mask, finalize_v2_geometry_program,
    geometry_worker, hash_geometry_program_with_runtime_worker, materialize_rig_geometry_program,
    now_string, render_glb_with_runtime_worker, sha256_hex, stable_visual_metric,
    strict_glb_inspection, transient_loss_metrics_at_resolution, validate_camera_calibration,
    validate_silhouette_rig, validate_worker_metadata, Runtime, RuntimeError,
};
use forgecad_contracts::{is_opaque_id, is_sha256, JobEventRecord, JobRecord, JobSummary};
use image::GenericImageView;
use serde_json::{json, Map, Value};
use std::cmp::Ordering;
use std::collections::HashSet;
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

const OPTIMIZATION_KIND: &str = "design_optimization";
const OPTIMIZATION_PROPOSAL_KIND: &str = "design_optimization_proposal";
const OPTIMIZATION_PROPOSAL_RESULT_KIND: &str = "design-optimization-proposal-result";
const OPTIMIZATION_SEARCH_STRATEGY: &str =
    "seed-then-adaptive-trust-region-v5-surface-control-groups-final-top-k-plus-baseline";
const MAX_INTENT_BYTES: usize = 256 * 1024;
const MAX_RESULT_BYTES: usize = 256 * 1024;
const STRICT_IMPROVEMENT_EPSILON: f64 = 1.0e-9;
const MAX_RESIDUAL_LANE_CANDIDATES: usize = 9;

static ACTIVE_JOBS: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();

fn active_jobs() -> &'static Mutex<HashSet<String>> {
    ACTIVE_JOBS.get_or_init(|| Mutex::new(HashSet::new()))
}

#[derive(Debug, Clone)]
struct OptimizationContext {
    intent: Value,
    intent_sha256: String,
    target: Value,
    target_mask: Vec<bool>,
    evaluation_objective: Option<Value>,
    evaluation_objective_sha256: Option<String>,
    part_target_mask: Option<Vec<bool>>,
    program: Value,
    camera: Value,
    rig: Value,
    part_id: String,
    residual_variants: Vec<Value>,
}

#[derive(Debug, Clone)]
struct CompiledCandidate {
    glb: Vec<u8>,
    program_sha256: String,
    program_object_sha256: String,
    artifact_object_sha256: String,
    triangle_count: u64,
}

#[derive(Debug, Clone)]
struct EvaluationRecord {
    value: Value,
    object_sha256: String,
    candidate_index: usize,
    loss: f64,
    fidelity: String,
    final_object_hashes: Vec<String>,
}

#[derive(Debug, Clone)]
struct OptimizationCheckpoint {
    next_stage: String,
    evaluations: Vec<EvaluationRecord>,
    candidate_program_object_sha256s: Vec<String>,
    candidate_artifact_object_sha256s: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MultiObjectiveComparison {
    non_regressing: bool,
    strict_improvement: bool,
}

impl Runtime {
    /// Materialize one completed CADFit proposal into a separate, reviewable
    /// candidate.  The optimizer itself remains proposal-only: this
    /// continuation revalidates the child result, recompiles the exact
    /// hash-bound GeometryProgram through the normal Runtime path, then runs
    /// an explicit caller-supplied ReferenceViewSpec comparison.  It never
    /// mutates the source candidate, parent ActionRun, version history or
    /// confirmation state.
    pub fn design_action_optimization_proposal_prepare(
        &self,
        request: Value,
    ) -> Result<Value, RuntimeError> {
        let object = request_object(&request, "design_action_optimization_proposal_prepare")?;
        reject_unknown_keys(
            object,
            &[
                "project_id",
                "session_id",
                "candidate_id",
                "run_id",
                "job_id",
                "view_spec",
                "input_sha256",
                "approved",
                "approval_receipt_id",
                "approval_summary",
                "approval_expires_at",
                "approval_session_id",
                "idempotency_key",
            ],
        )?;
        validate_approval(object)?;

        let project_id = required_id(object, "project_id")?;
        let session_id = required_id(object, "session_id")?;
        let source_candidate_id = required_id(object, "candidate_id")?;
        let run_id = required_id(object, "run_id")?;
        let optimization_job_id = required_id(object, "job_id")?;
        let input_sha256 = required_sha(object, "input_sha256")?;
        let idempotency_key = required_id(object, "idempotency_key")?;
        if object.get("approval_session_id").and_then(Value::as_str) != Some(session_id) {
            return Err(RuntimeError::InvalidInput(
                "OPTIMIZATION_PROPOSAL_SCOPE_MISMATCH: approval_session_id must match session_id"
                    .to_owned(),
            ));
        }
        let view_spec = object
            .get("view_spec")
            .ok_or_else(|| RuntimeError::InvalidInput("REFERENCE_VIEW_SPEC_REQUIRED".to_owned()))?
            .clone();
        if !view_spec.is_object() {
            return Err(RuntimeError::InvalidInput(
                "REFERENCE_VIEW_SPEC_REQUIRED: view_spec must be an object".to_owned(),
            ));
        }
        let input_binding = json!({
            "project_id":project_id,
            "session_id":session_id,
            "candidate_id":source_candidate_id,
            "run_id":run_id,
            "job_id":optimization_job_id,
            "view_spec":view_spec,
            "idempotency_key":idempotency_key
        });
        if canonical_json_hash(&input_binding) != input_sha256 {
            return Err(RuntimeError::InvalidInput(
                "OPTIMIZATION_PROPOSAL_INPUT_HASH_MISMATCH: input_sha256 must bind the complete continuation request"
                    .to_owned(),
            ));
        }

        let source_candidate = self.candidate(source_candidate_id)?.ok_or_else(|| {
            RuntimeError::InvalidInput("NOT_FOUND: candidate not found".to_owned())
        })?;
        if source_candidate.project_id != project_id {
            return Err(RuntimeError::InvalidInput(
                "OPTIMIZATION_PROPOSAL_SCOPE_DENIED: source candidate is outside the project"
                    .to_owned(),
            ));
        }
        let source_artifact_sha256 = source_candidate
            .manifest_hash
            .clone()
            .or_else(|| source_candidate.prepared_object_sha256.clone());

        let parent = self.store.get_design_action_run(run_id)?.ok_or_else(|| {
            RuntimeError::InvalidInput("NOT_FOUND: DesignActionRun not found".to_owned())
        })?;
        validate_action_optimization_parent(
            &parent,
            run_id,
            project_id,
            session_id,
            source_candidate_id,
            optimization_job_id,
        )?;

        let optimization_job =
            self.store
                .get_job_record(optimization_job_id)?
                .ok_or_else(|| {
                    RuntimeError::InvalidInput("NOT_FOUND: optimization job not found".to_owned())
                })?;
        if optimization_job.kind != OPTIMIZATION_KIND
            || optimization_job.project_id != project_id
            || parent
                .get("optimization_intent_sha256")
                .and_then(Value::as_str)
                != Some(optimization_job.request_sha256.as_str())
        {
            return Err(RuntimeError::InvalidInput(
                "OPTIMIZATION_PROPOSAL_CHILD_BINDING_MISMATCH".to_owned(),
            ));
        }

        let proposal_job_id = format!("design-opt-proposal-{}", &input_sha256[..32]);
        let optimizer_result_sha256 = optimization_job.checkpoint_sha256.clone();
        let child_result = optimizer_result_sha256
            .as_deref()
            .map(|hash| read_json_object(self, hash, "optimization-result"))
            .transpose()?;

        if !matches!(
            optimization_job.status.as_str(),
            "succeeded" | "failed" | "cancelled"
        ) {
            return Ok(finalize_optimization_proposal_result(json!({
                "schema_version":"OptimizationProposalPrepareResult@1",
                "project_id":project_id,
                "session_id":session_id,
                "candidate_id":source_candidate_id,
                "run_id":run_id,
                "job_id":optimization_job_id,
                "proposal_job_id":proposal_job_id,
                "view_spec_sha256":canonical_json_hash(&view_spec),
                "status":"not-ready",
                "reason_code":"optimization-job-not-terminal",
                "optimizer_result_sha256":optimizer_result_sha256,
                "optimizer_proposal_status":child_result.as_ref().and_then(|value| value.get("proposal_status")).cloned().unwrap_or(Value::Null),
                "baseline_loss":child_result.as_ref().and_then(|value| value.get("baseline_loss")).cloned().unwrap_or(Value::Null),
                "best_loss":child_result.as_ref().and_then(|value| value.get("best_loss")).cloned().unwrap_or(Value::Null),
                "strict_improvement":child_result.as_ref().and_then(|value| value.get("strict_improvement")).and_then(Value::as_bool).unwrap_or(false),
                "non_regressing":child_result.as_ref().and_then(|value| value.get("non_regressing")).and_then(Value::as_bool).unwrap_or(false),
                "source_candidate_state_sha256":source_candidate.canonical_sha256,
                "source_artifact_sha256":source_artifact_sha256,
                "proposal_candidate_id":Value::Null,
                "proposal_candidate_state_sha256":Value::Null,
                "proposal_program_sha256":Value::Null,
                "proposal_program_object_sha256":Value::Null,
                "proposal_artifact_sha256":Value::Null,
                "render_set_object_sha256":Value::Null,
                "comparison_report_object_sha256":Value::Null,
                "quality_report_id":Value::Null,
                "quality_report_object_sha256":Value::Null,
                "visual_status":"NOT_RUN",
                "visual_gate_passed":false,
                "confirm_allowed":false,
                "repair_apply_status":"not-ready",
                "source_candidate_unchanged":true,
                "persistent_user_data_touched":false,
                "version_created":false,
                "canonical_sha256":""
            })));
        }

        let Some(child_result) = child_result else {
            return Ok(finalize_optimization_proposal_result(json!({
                "schema_version":"OptimizationProposalPrepareResult@1",
                "project_id":project_id,
                "session_id":session_id,
                "candidate_id":source_candidate_id,
                "run_id":run_id,
                "job_id":optimization_job_id,
                "proposal_job_id":proposal_job_id,
                "view_spec_sha256":canonical_json_hash(&view_spec),
                "status":"blocked",
                "reason_code":"optimization-result-unavailable",
                "optimizer_result_sha256":Value::Null,
                "optimizer_proposal_status":Value::Null,
                "baseline_loss":Value::Null,
                "best_loss":Value::Null,
                "strict_improvement":false,
                "non_regressing":false,
                "source_candidate_state_sha256":source_candidate.canonical_sha256,
                "source_artifact_sha256":source_artifact_sha256,
                "proposal_candidate_id":Value::Null,
                "proposal_candidate_state_sha256":Value::Null,
                "proposal_program_sha256":Value::Null,
                "proposal_program_object_sha256":Value::Null,
                "proposal_artifact_sha256":Value::Null,
                "render_set_object_sha256":Value::Null,
                "comparison_report_object_sha256":Value::Null,
                "quality_report_id":Value::Null,
                "quality_report_object_sha256":Value::Null,
                "visual_status":"NOT_RUN",
                "visual_gate_passed":false,
                "confirm_allowed":false,
                "repair_apply_status":"blocked-no-improvement",
                "source_candidate_unchanged":true,
                "persistent_user_data_touched":false,
                "version_created":false,
                "canonical_sha256":""
            })));
        };
        validate_result(
            &child_result,
            optimization_job_id,
            &optimization_job.request_sha256,
        )?;

        let optimizer_proposal_status = child_result
            .get("proposal_status")
            .and_then(Value::as_str)
            .unwrap_or("blocked-invalid");
        let strict_improvement = child_result
            .get("strict_improvement")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let non_regressing = child_result
            .get("non_regressing")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        if optimization_job.status != "succeeded"
            || optimizer_proposal_status != "proposed"
            || !strict_improvement
            || !non_regressing
        {
            let reason_code = if optimization_job.status != "succeeded" {
                "optimization-job-terminal-failure"
            } else if optimizer_proposal_status == "blocked-no-improvement" {
                "optimization-no-strict-improvement"
            } else {
                "optimization-proposal-invalid"
            };
            return Ok(finalize_optimization_proposal_result(json!({
                "schema_version":"OptimizationProposalPrepareResult@1",
                "project_id":project_id,
                "session_id":session_id,
                "candidate_id":source_candidate_id,
                "run_id":run_id,
                "job_id":optimization_job_id,
                "proposal_job_id":proposal_job_id,
                "view_spec_sha256":canonical_json_hash(&view_spec),
                "status":"blocked",
                "reason_code":reason_code,
                "optimizer_result_sha256":optimizer_result_sha256,
                "optimizer_proposal_status":optimizer_proposal_status,
                "baseline_loss":child_result.get("baseline_loss").cloned().unwrap_or(Value::Null),
                "best_loss":child_result.get("best_loss").cloned().unwrap_or(Value::Null),
                "strict_improvement":strict_improvement,
                "non_regressing":non_regressing,
                "source_candidate_state_sha256":source_candidate.canonical_sha256,
                "source_artifact_sha256":source_artifact_sha256,
                "proposal_candidate_id":Value::Null,
                "proposal_candidate_state_sha256":Value::Null,
                "proposal_program_sha256":Value::Null,
                "proposal_program_object_sha256":Value::Null,
                "proposal_artifact_sha256":Value::Null,
                "render_set_object_sha256":Value::Null,
                "comparison_report_object_sha256":Value::Null,
                "quality_report_id":Value::Null,
                "quality_report_object_sha256":Value::Null,
                "visual_status":"NOT_RUN",
                "visual_gate_passed":false,
                "confirm_allowed":false,
                "repair_apply_status":if reason_code == "optimization-no-strict-improvement" { "blocked-no-improvement" } else { "blocked-optimizer-state" },
                "source_candidate_unchanged":true,
                "persistent_user_data_touched":false,
                "version_created":false,
                "canonical_sha256":""
            })));
        }

        let existing = self.store.get_job_record(&proposal_job_id)?;
        if let Some(existing) = existing {
            if existing.project_id != project_id
                || existing.kind != OPTIMIZATION_PROPOSAL_KIND
                || existing.request_sha256 != input_sha256
            {
                return Err(RuntimeError::InvalidInput(
                    "OPTIMIZATION_PROPOSAL_IDEMPOTENCY_CONFLICT".to_owned(),
                ));
            }
            if let Some(checkpoint_sha256) = existing.checkpoint_sha256.as_deref() {
                let result =
                    read_json_object(self, checkpoint_sha256, OPTIMIZATION_PROPOSAL_RESULT_KIND)?;
                validate_optimization_proposal_result(&result)?;
                return Ok(result);
            }
            if existing.status == "succeeded" {
                return Err(RuntimeError::InvalidInput(
                    "OPTIMIZATION_PROPOSAL_READBACK_MISSING: succeeded proposal job has no result"
                        .to_owned(),
                ));
            }
            if matches!(existing.status.as_str(), "failed" | "cancelled") {
                return Err(RuntimeError::InvalidInput(
                    "OPTIMIZATION_PROPOSAL_PREVIOUS_ATTEMPT_FAILED: retry with a new idempotency key"
                        .to_owned(),
                ));
            }
            return Ok(finalize_optimization_proposal_result(json!({
                "schema_version":"OptimizationProposalPrepareResult@1",
                "project_id":project_id,
                "session_id":session_id,
                "candidate_id":source_candidate_id,
                "run_id":run_id,
                "job_id":optimization_job_id,
                "proposal_job_id":proposal_job_id,
                "view_spec_sha256":canonical_json_hash(&view_spec),
                "status":"not-ready",
                "reason_code":"proposal-materialization-in-progress",
                "optimizer_result_sha256":optimizer_result_sha256,
                "optimizer_proposal_status":"proposed",
                "baseline_loss":child_result.get("baseline_loss").cloned().unwrap_or(Value::Null),
                "best_loss":child_result.get("best_loss").cloned().unwrap_or(Value::Null),
                "strict_improvement":true,
                "non_regressing":true,
                "source_candidate_state_sha256":source_candidate.canonical_sha256,
                "source_artifact_sha256":source_artifact_sha256,
                "proposal_candidate_id":Value::Null,
                "proposal_candidate_state_sha256":Value::Null,
                "proposal_program_sha256":Value::Null,
                "proposal_program_object_sha256":Value::Null,
                "proposal_artifact_sha256":Value::Null,
                "render_set_object_sha256":Value::Null,
                "comparison_report_object_sha256":Value::Null,
                "quality_report_id":Value::Null,
                "quality_report_object_sha256":Value::Null,
                "visual_status":"NOT_RUN",
                "visual_gate_passed":false,
                "confirm_allowed":false,
                "repair_apply_status":"not-ready",
                "source_candidate_unchanged":true,
                "persistent_user_data_touched":false,
                "version_created":false,
                "canonical_sha256":""
            })));
        }

        let started_at = now_string();
        let initial_job = JobRecord {
            schema_version: "RuntimeJob@1".to_owned(),
            job_id: proposal_job_id.clone(),
            project_id: project_id.to_owned(),
            kind: OPTIMIZATION_PROPOSAL_KIND.to_owned(),
            status: "running".to_owned(),
            progress: 0,
            request_sha256: input_sha256.to_owned(),
            checkpoint_sha256: None,
            error_code: None,
            created_at: started_at.clone(),
            updated_at: started_at.clone(),
        };
        let started_event = JobEventRecord {
            schema_version: "RuntimeJobEvent@1".to_owned(),
            job_id: proposal_job_id.clone(),
            sequence: 1,
            kind: "optimization_proposal_materialization_started".to_owned(),
            payload: json!({
                "project_id":project_id,
                "session_id":session_id,
                "candidate_id":source_candidate_id,
                "run_id":run_id,
                "optimization_job_id":optimization_job_id,
                "view_spec_sha256":canonical_json_hash(&view_spec)
            }),
            created_at: started_at,
        };
        let inserted =
            self.store
                .insert_job_with_event_if_absent(&initial_job, &started_event, &[])?;
        if inserted.status != "running" {
            return Err(RuntimeError::InvalidInput(
                "OPTIMIZATION_PROPOSAL_JOB_STATE_INVALID".to_owned(),
            ));
        }

        let result = self.materialize_optimization_proposal(
            project_id,
            session_id,
            source_candidate_id,
            run_id,
            optimization_job_id,
            &proposal_job_id,
            &source_candidate,
            source_artifact_sha256.as_deref(),
            &view_spec,
            &optimization_job,
            &child_result,
        );
        let result = match result {
            Ok(result) => result,
            Err(error) => {
                mark_optimization_proposal_failed(self, &proposal_job_id, &error)?;
                return Err(error);
            }
        };
        let bytes = canonical_json_bytes(&result)
            .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?;
        let result_object = self.put_object(
            &bytes,
            None,
            "application/json",
            OPTIMIZATION_PROPOSAL_RESULT_KIND,
        )?;
        let current = self
            .store
            .get_job_record(&proposal_job_id)?
            .ok_or_else(|| {
                RuntimeError::InvalidInput("OPTIMIZATION_PROPOSAL_JOB_NOT_FOUND".to_owned())
            })?;
        let mut reachable = vec![
            result_object.record.sha256.clone(),
            optimizer_result_sha256
                .clone()
                .expect("validated succeeded optimizer result"),
        ];
        for key in [
            "proposal_program_object_sha256",
            "proposal_artifact_sha256",
            "render_set_object_sha256",
            "comparison_report_object_sha256",
            "quality_report_object_sha256",
        ] {
            if let Some(hash) = result.get(key).and_then(Value::as_str) {
                reachable.push(hash.to_owned());
            }
        }
        reachable.sort();
        reachable.dedup();
        let next = JobRecord {
            schema_version: current.schema_version,
            job_id: current.job_id,
            project_id: current.project_id,
            kind: current.kind,
            status: "succeeded".to_owned(),
            progress: 100,
            request_sha256: current.request_sha256,
            checkpoint_sha256: Some(result_object.record.sha256.clone()),
            error_code: None,
            created_at: current.created_at,
            updated_at: now_string(),
        };
        self.store.update_job_with_event(
            &next,
            "optimization_proposal_materialized",
            &json!({
                "result_sha256":result_object.record.sha256,
                "optimization_job_id":optimization_job_id,
                "proposal_candidate_id":result["proposal_candidate_id"],
                "visual_status":result["visual_status"],
                "confirm_allowed":false
            }),
            &reachable,
        )?;
        Ok(result)
    }

    fn materialize_optimization_proposal(
        &self,
        project_id: &str,
        session_id: &str,
        source_candidate_id: &str,
        run_id: &str,
        optimization_job_id: &str,
        proposal_job_id: &str,
        source_candidate: &forgecad_contracts::CandidateRecord,
        source_artifact_sha256: Option<&str>,
        view_spec: &Value,
        optimization_job: &JobRecord,
        child_result: &Value,
    ) -> Result<Value, RuntimeError> {
        let intent = read_json_object(
            self,
            &optimization_job.request_sha256,
            "optimization-intent",
        )?;
        let _context = validate_intent(
            self,
            &intent,
            &optimization_job.request_sha256,
            project_id,
            source_candidate_id,
        )?;
        if intent.get("action_run_id").and_then(Value::as_str) != Some(run_id) {
            return Err(RuntimeError::InvalidInput(
                "OPTIMIZATION_PROPOSAL_ACTION_RUN_BINDING_MISMATCH".to_owned(),
            ));
        }
        let view_reference_id = view_spec.get("reference_id").and_then(Value::as_str);
        let view_reference_sha256 = view_spec.get("reference_sha256").and_then(Value::as_str);
        if view_reference_id != intent.get("reference_id").and_then(Value::as_str)
            || view_reference_sha256 != intent.get("reference_sha256").and_then(Value::as_str)
        {
            return Err(RuntimeError::InvalidInput(
                "OPTIMIZATION_PROPOSAL_VIEW_REFERENCE_MISMATCH".to_owned(),
            ));
        }

        let result_sha256 = optimization_job
            .checkpoint_sha256
            .as_deref()
            .ok_or_else(|| {
                RuntimeError::InvalidInput("OPTIMIZATION_PROPOSAL_RESULT_UNAVAILABLE".to_owned())
            })?;
        let proposal_program_object_sha256 = child_result
            .get("proposal_program_object_sha256")
            .and_then(Value::as_str)
            .filter(|hash| is_sha256(hash))
            .ok_or_else(|| {
                RuntimeError::InvalidInput("OPTIMIZATION_PROPOSAL_PROGRAM_MISSING".to_owned())
            })?;
        let proposal_artifact_sha256 = child_result
            .get("proposal_artifact_sha256")
            .and_then(Value::as_str)
            .filter(|hash| is_sha256(hash))
            .ok_or_else(|| {
                RuntimeError::InvalidInput("OPTIMIZATION_PROPOSAL_ARTIFACT_MISSING".to_owned())
            })?;
        let best_program_sha256 = child_result
            .get("best_program_sha256")
            .and_then(Value::as_str)
            .filter(|hash| is_sha256(hash))
            .ok_or_else(|| {
                RuntimeError::InvalidInput(
                    "OPTIMIZATION_PROPOSAL_PROGRAM_LINEAGE_MISSING".to_owned(),
                )
            })?;
        let best_program_object_sha256 = child_result
            .get("best_program_object_sha256")
            .and_then(Value::as_str)
            .filter(|hash| is_sha256(hash))
            .ok_or_else(|| {
                RuntimeError::InvalidInput(
                    "OPTIMIZATION_PROPOSAL_BEST_PROGRAM_OBJECT_MISSING".to_owned(),
                )
            })?;
        if proposal_program_object_sha256 != best_program_object_sha256 {
            return Err(RuntimeError::InvalidInput(
                "OPTIMIZATION_PROPOSAL_BEST_PROGRAM_OBJECT_LINEAGE_MISMATCH".to_owned(),
            ));
        }
        let best_artifact_sha256 = child_result
            .get("best_artifact_sha256")
            .and_then(Value::as_str)
            .filter(|hash| is_sha256(hash))
            .ok_or_else(|| {
                RuntimeError::InvalidInput("OPTIMIZATION_PROPOSAL_BEST_ARTIFACT_MISSING".to_owned())
            })?;
        if proposal_artifact_sha256 != best_artifact_sha256 {
            return Err(RuntimeError::InvalidInput(
                "OPTIMIZATION_PROPOSAL_BEST_LINEAGE_MISMATCH".to_owned(),
            ));
        }
        let proposal_program =
            read_json_object(self, proposal_program_object_sha256, "geometry-program-v2")?;
        if proposal_program.get("project_id").and_then(Value::as_str) != Some(project_id) {
            return Err(RuntimeError::InvalidInput(
                "OPTIMIZATION_PROPOSAL_PROGRAM_SCOPE_DENIED".to_owned(),
            ));
        }
        let mut proposal_program_draft = proposal_program.clone();
        proposal_program_draft
            .as_object_mut()
            .expect("proposal GeometryProgram was checked as an object")
            .remove("canonical_sha256");
        let program_hash = hash_geometry_program_with_runtime_worker(&proposal_program_draft)
            .map_err(|error| {
                RuntimeError::InvalidInput(format!(
                    "OPTIMIZATION_PROPOSAL_PROGRAM_INVALID: {error}"
                ))
            })?;
        if program_hash.get("canonical_sha256").and_then(Value::as_str) != Some(best_program_sha256)
        {
            return Err(RuntimeError::InvalidInput(
                "OPTIMIZATION_PROPOSAL_PROGRAM_HASH_MISMATCH".to_owned(),
            ));
        }
        if let Some(declared_hash) = proposal_program
            .get("canonical_sha256")
            .and_then(Value::as_str)
        {
            if declared_hash != best_program_sha256 {
                return Err(RuntimeError::InvalidInput(
                    "OPTIMIZATION_PROPOSAL_PROGRAM_HASH_MISMATCH".to_owned(),
                ));
            }
        }
        let artifact_object = self
            .store
            .get_object(proposal_artifact_sha256)?
            .ok_or_else(|| {
                RuntimeError::InvalidInput("OPTIMIZATION_PROPOSAL_ARTIFACT_UNAVAILABLE".to_owned())
            })?;
        if artifact_object.mime != "model/gltf-binary" {
            return Err(RuntimeError::InvalidInput(
                "OPTIMIZATION_PROPOSAL_ARTIFACT_METADATA_MISMATCH".to_owned(),
            ));
        }
        let artifact = self.cas_read(proposal_artifact_sha256)?;
        if sha256_hex(&artifact) != proposal_artifact_sha256 {
            return Err(RuntimeError::InvalidInput(
                "OPTIMIZATION_PROPOSAL_ARTIFACT_HASH_MISMATCH".to_owned(),
            ));
        }
        let inspection = strict_glb_inspection(&artifact)?;
        if !inspection.hard_gate_passed || inspection.program_sha256 != best_program_sha256 {
            return Err(RuntimeError::InvalidInput(
                "OPTIMIZATION_PROPOSAL_ARTIFACT_READBACK_FAILED".to_owned(),
            ));
        }
        let prepared = self.prepare_geometry_candidate(
            project_id,
            source_candidate.base_version_id.as_deref(),
            json!({
                "typed":"geometry",
                "reference_id":intent["reference_id"],
                "geometry_program":proposal_program
            }),
        )?;
        let proposal_candidate = prepared.get("candidate").ok_or_else(|| {
            RuntimeError::InvalidInput("OPTIMIZATION_PROPOSAL_CANDIDATE_MISSING".to_owned())
        })?;
        let proposal_candidate_id = proposal_candidate
            .get("candidate_id")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                RuntimeError::InvalidInput("OPTIMIZATION_PROPOSAL_CANDIDATE_ID_MISSING".to_owned())
            })?;
        let proposal_candidate_state_sha256 = proposal_candidate
            .get("canonical_sha256")
            .and_then(Value::as_str)
            .filter(|hash| is_sha256(hash))
            .ok_or_else(|| {
                RuntimeError::InvalidInput(
                    "OPTIMIZATION_PROPOSAL_CANDIDATE_STATE_MISSING".to_owned(),
                )
            })?;
        let proposal_artifact_bound = proposal_candidate
            .get("prepared_object_sha256")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                RuntimeError::InvalidInput(
                    "OPTIMIZATION_PROPOSAL_CANDIDATE_ARTIFACT_MISSING".to_owned(),
                )
            })?;
        if proposal_artifact_bound != proposal_artifact_sha256 {
            return Err(RuntimeError::InvalidInput(
                "OPTIMIZATION_PROPOSAL_CANDIDATE_ARTIFACT_MISMATCH".to_owned(),
            ));
        }
        let evidence = self
            .store
            .get_geometry_candidate_evidence(proposal_candidate_id)?
            .ok_or_else(|| {
                RuntimeError::InvalidInput(
                    "OPTIMIZATION_PROPOSAL_GEOMETRY_EVIDENCE_MISSING".to_owned(),
                )
            })?;
        // The optimization candidate stores the canonical GeometryProgram
        // object (which includes canonical_sha256).  The normal candidate
        // prepare path deliberately stores its hash-omitted draft under the
        // semantic program hash, so the evidence object is bound to
        // best_program_sha256 rather than the full-object CAS hash.
        if evidence.geometry_program_sha256 != best_program_sha256
            || evidence.geometry_program_object_sha256 != best_program_sha256
            || evidence.artifact_object_sha256 != proposal_artifact_sha256
        {
            return Err(RuntimeError::InvalidInput(
                "OPTIMIZATION_PROPOSAL_GEOMETRY_LINEAGE_MISMATCH".to_owned(),
            ));
        }

        let visual = self.prepare_reference_comparison(
            project_id,
            json!({
                "project_id":project_id,
                "candidate_id":proposal_candidate_id,
                "session_id":session_id,
                "authoring_candidate_id":source_candidate_id,
                "reference_id":intent["reference_id"],
                "view_spec":view_spec,
                "camera":intent["camera"],
                "target_sha256":intent["target_sha256"]
            }),
        )?;
        let quality = visual.get("quality_report").ok_or_else(|| {
            RuntimeError::InvalidInput("OPTIMIZATION_PROPOSAL_QUALITY_MISSING".to_owned())
        })?;
        let visual_status = quality
            .get("visual_status")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                RuntimeError::InvalidInput("OPTIMIZATION_PROPOSAL_VISUAL_STATUS_MISSING".to_owned())
            })?;
        let visual_gate_passed = visual_status == "PARTIAL_VISIBLE_VIEW_PASS";
        let source_after = self.candidate(source_candidate_id)?.ok_or_else(|| {
            RuntimeError::InvalidInput("OPTIMIZATION_PROPOSAL_SOURCE_DISAPPEARED".to_owned())
        })?;
        let source_candidate_unchanged = source_after.canonical_sha256
            == source_candidate.canonical_sha256
            && source_after.prepared_object_sha256 == source_candidate.prepared_object_sha256
            && source_after.manifest_hash == source_candidate.manifest_hash;
        if !source_candidate_unchanged {
            return Err(RuntimeError::InvalidInput(
                "OPTIMIZATION_PROPOSAL_SOURCE_MUTATED".to_owned(),
            ));
        }

        let mut response = json!({
            "schema_version":"OptimizationProposalPrepareResult@1",
            "project_id":project_id,
            "session_id":session_id,
            "candidate_id":source_candidate_id,
            "run_id":run_id,
            "job_id":optimization_job_id,
            "proposal_job_id":proposal_job_id,
            "view_spec_sha256":canonical_json_hash(view_spec),
            "status":"proposed",
            "reason_code":"proposal-materialized-for-review",
            "optimizer_result_sha256":result_sha256,
            "optimizer_proposal_status":"proposed",
            "baseline_loss":child_result["baseline_loss"],
            "best_loss":child_result["best_loss"],
            "strict_improvement":true,
            "non_regressing":true,
            "source_candidate_state_sha256":source_candidate.canonical_sha256,
            "source_artifact_sha256":source_artifact_sha256,
            "proposal_candidate_id":proposal_candidate_id,
            "proposal_candidate_state_sha256":proposal_candidate_state_sha256,
            "proposal_program_sha256":best_program_sha256,
            "proposal_program_object_sha256":proposal_program_object_sha256,
            "proposal_artifact_sha256":proposal_artifact_sha256,
            "render_set_object_sha256":visual["render_set_object_sha256"],
            "comparison_report_object_sha256":visual["comparison_report_object_sha256"],
            "quality_report_id":quality["quality_report_id"],
            "quality_report_object_sha256":visual["quality_report_object_sha256"],
            "visual_status":visual_status,
            "visual_gate_passed":visual_gate_passed,
            "confirm_allowed":false,
            "repair_apply_status":if visual_gate_passed { "blocked-action-run-proposal-boundary" } else { "blocked-quality-gate" },
            "source_candidate_unchanged":source_candidate_unchanged,
            "persistent_user_data_touched":false,
            "version_created":false,
            "canonical_sha256":""
        });
        response["canonical_sha256"] = Value::String(canonical_json_hash(&response));
        validate_optimization_proposal_result(&response)?;
        Ok(response)
    }

    pub fn optimization_job_prepare(&self, request: Value) -> Result<Value, RuntimeError> {
        let object = request_object(&request, "optimization_job_prepare")?;
        reject_unknown_keys(
            object,
            &[
                "project_id",
                "candidate_id",
                "intent",
                "approved",
                "approval_receipt_id",
                "approval_summary",
                "approval_expires_at",
                "approval_session_id",
                "idempotency_key",
            ],
        )?;
        validate_approval(object)?;
        let project_id = required_id(object, "project_id")?;
        let candidate_id = required_id(object, "candidate_id")?;
        let intent = object
            .get("intent")
            .ok_or_else(|| RuntimeError::InvalidInput("intent is required".to_owned()))?
            .clone();
        let job_id = intent
            .get("job_id")
            .and_then(Value::as_str)
            .ok_or_else(|| RuntimeError::InvalidInput("intent.job_id is required".to_owned()))?;
        let candidate = self.candidate(candidate_id)?.ok_or_else(|| {
            RuntimeError::InvalidInput("NOT_FOUND: candidate not found".to_owned())
        })?;
        if candidate.project_id != project_id {
            return Err(RuntimeError::InvalidInput(
                "PROJECT_SCOPE_DENIED: candidate is outside the requested project".to_owned(),
            ));
        }
        let intent_bytes = canonical_json_bytes(&intent)
            .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?;
        if intent_bytes.len() > MAX_INTENT_BYTES {
            return Err(RuntimeError::InvalidInput(
                "OPTIMIZATION_INTENT_TOO_LARGE".to_owned(),
            ));
        }
        let intent_object = self.put_object(
            &intent_bytes,
            None,
            "application/json",
            "optimization-intent",
        )?;
        let intent_sha256 = intent_object.record.sha256.clone();
        let _context = validate_intent(self, &intent, &intent_sha256, project_id, candidate_id)?;

        let existing = self.store.get_job_record(job_id)?;
        let job = if let Some(existing) = existing {
            if existing.kind != OPTIMIZATION_KIND
                || existing.project_id != project_id
                || existing.request_sha256 != intent_sha256
            {
                return Err(RuntimeError::InvalidInput(
                    "OPTIMIZATION_JOB_IDEMPOTENCY_CONFLICT".to_owned(),
                ));
            }
            existing
        } else {
            let now = now_string();
            let job = JobRecord {
                schema_version: "RuntimeJob@1".to_owned(),
                job_id: job_id.to_owned(),
                project_id: project_id.to_owned(),
                kind: OPTIMIZATION_KIND.to_owned(),
                status: "queued".to_owned(),
                progress: 0,
                request_sha256: intent_sha256.clone(),
                checkpoint_sha256: None,
                error_code: None,
                created_at: now.clone(),
                updated_at: now.clone(),
            };
            let event = JobEventRecord {
                schema_version: "RuntimeJobEvent@1".to_owned(),
                job_id: job_id.to_owned(),
                sequence: 1,
                kind: "optimization_queued".to_owned(),
                payload: json!({
                    "intent_sha256":intent_sha256,
                    "candidate_id":candidate_id,
                    "project_id":project_id
                }),
                created_at: now,
            };
            self.store.insert_job_with_event_if_absent(
                &job,
                &event,
                &[intent_object.record.sha256.clone()],
            )?
        };

        // Build the initial queued readback before spawning the Worker.  The
        // Worker owns the same SQLite/CAS store and may immediately hold a
        // write transaction while compiling its first evaluation; reading
        // after `spawn_optimization_job` made this supposedly asynchronous
        // prepare call block until the whole CADFit search finished.
        let initial = self.optimization_job_get(json!({
            "project_id":project_id,
            "candidate_id":candidate_id,
            "job_id":job_id
        }))?;
        if job.status == "queued" {
            spawn_optimization_job(self, job_id);
        }
        Ok(initial)
    }

    pub fn optimization_job_resume(&self, request: Value) -> Result<Value, RuntimeError> {
        let object = request_object(&request, "optimization_job_resume")?;
        reject_unknown_keys(
            object,
            &[
                "project_id",
                "candidate_id",
                "job_id",
                "approved",
                "approval_receipt_id",
                "approval_summary",
                "approval_expires_at",
                "approval_session_id",
                "idempotency_key",
            ],
        )?;
        validate_approval(object)?;
        let project_id = required_id(object, "project_id")?;
        let candidate_id = required_id(object, "candidate_id")?;
        let job_id = required_id(object, "job_id")?;
        let job = self.store.get_job_record(job_id)?.ok_or_else(|| {
            RuntimeError::InvalidInput("NOT_FOUND: optimization job not found".to_owned())
        })?;
        if job.kind != OPTIMIZATION_KIND || job.project_id != project_id {
            return Err(RuntimeError::InvalidInput(
                "OPTIMIZATION_JOB_SCOPE_DENIED".to_owned(),
            ));
        }
        let candidate = self.candidate(candidate_id)?.ok_or_else(|| {
            RuntimeError::InvalidInput("NOT_FOUND: candidate not found".to_owned())
        })?;
        if candidate.project_id != project_id {
            return Err(RuntimeError::InvalidInput(
                "PROJECT_SCOPE_DENIED: candidate is outside the requested project".to_owned(),
            ));
        }
        let intent = read_json_object(self, &job.request_sha256, "optimization-intent")?;
        let context =
            validate_intent(self, &intent, &job.request_sha256, project_id, candidate_id)?;
        let checkpoint_sha256 = job.checkpoint_sha256.as_deref().ok_or_else(|| {
            RuntimeError::InvalidInput("OPTIMIZATION_CHECKPOINT_REQUIRED_FOR_RESUME".to_owned())
        })?;
        let fidelity = intent.get("fidelity").ok_or_else(|| {
            RuntimeError::InvalidInput("OPTIMIZATION_FIDELITY_INVALID".to_owned())
        })?;
        let coarse_count = fidelity["coarse_evaluations"].as_u64().unwrap_or(0) as usize;
        let mid_top_k = fidelity["mid_top_k"].as_u64().unwrap_or(0) as usize;
        let final_top_k = fidelity["final_top_k"].as_u64().unwrap_or(0) as usize;
        let checkpoint = load_optimization_checkpoint(
            self,
            checkpoint_sha256,
            job_id,
            &job.request_sha256,
            coarse_count,
            mid_top_k,
            final_top_k,
        )?;
        validate_evaluation_objective_checkpoint(
            &checkpoint.evaluations,
            context.evaluation_objective_sha256.as_deref(),
        )?;
        if checkpoint.next_stage == "done" {
            return Err(RuntimeError::InvalidInput(
                "OPTIMIZATION_CHECKPOINT_ALREADY_COMPLETE".to_owned(),
            ));
        }
        let requeued = self.store.requeue_job(
            job_id,
            &now_string(),
            &json!({"intent_sha256":job.request_sha256,"candidate_id":candidate_id}),
        )?;
        // As in prepare, obtain the queued snapshot before starting the
        // Worker so resume remains a genuinely non-blocking control-plane
        // operation even when the previous checkpoint is large.
        let initial = self.optimization_job_get(json!({
            "project_id":project_id,
            "candidate_id":candidate_id,
            "job_id":job_id
        }))?;
        if requeued.status == "queued" {
            spawn_optimization_job(self, job_id);
        }
        Ok(initial)
    }

    pub fn optimization_job_get(&self, request: Value) -> Result<Value, RuntimeError> {
        let object = request_object(&request, "optimization_job_get")?;
        reject_unknown_keys(object, &["project_id", "candidate_id", "job_id"])?;
        let project_id = required_id(object, "project_id")?;
        let candidate_id = required_id(object, "candidate_id")?;
        let job_id = required_id(object, "job_id")?;
        let job = self.store.get_job_record(job_id)?.ok_or_else(|| {
            RuntimeError::InvalidInput("NOT_FOUND: optimization job not found".to_owned())
        })?;
        if job.kind != OPTIMIZATION_KIND || job.project_id != project_id {
            return Err(RuntimeError::InvalidInput(
                "OPTIMIZATION_JOB_SCOPE_DENIED".to_owned(),
            ));
        }
        let candidate = self.candidate(candidate_id)?.ok_or_else(|| {
            RuntimeError::InvalidInput("NOT_FOUND: candidate not found".to_owned())
        })?;
        if candidate.project_id != project_id {
            return Err(RuntimeError::InvalidInput(
                "PROJECT_SCOPE_DENIED: candidate is outside the requested project".to_owned(),
            ));
        }
        let intent = read_json_object(self, &job.request_sha256, "optimization-intent")?;
        validate_intent(self, &intent, &job.request_sha256, project_id, candidate_id)?;
        let result = job
            .checkpoint_sha256
            .as_deref()
            .map(|hash| read_json_object(self, hash, "optimization-result"))
            .transpose()?;
        if let Some(result) = result.as_ref() {
            validate_result(result, job_id, &job.request_sha256)?;
            if result
                .get("evaluation_objective_sha256")
                .cloned()
                .unwrap_or(Value::Null)
                != intent
                    .get("evaluation_objective_sha256")
                    .cloned()
                    .unwrap_or(Value::Null)
            {
                return Err(RuntimeError::InvalidInput(
                    "OPTIMIZATION_RESULT_EVALUATION_OBJECTIVE_MISMATCH".to_owned(),
                ));
            }
        }
        let summary = JobSummary {
            job_id: job.job_id.clone(),
            project_id: job.project_id.clone(),
            kind: job.kind.clone(),
            status: job.status.clone(),
            progress: job.progress,
            error_code: job.error_code.clone(),
            created_at: job.created_at.clone(),
            updated_at: job.updated_at.clone(),
        };
        let mut response = json!({
            "schema_version":"OptimizationJobResult@1",
            "job":summary,
            "intent_sha256":job.request_sha256,
            "result":result,
            "canonical_sha256":""
        });
        response["canonical_sha256"] = Value::String(canonical_json_hash(&response));
        Ok(response)
    }
}

fn validate_action_optimization_parent(
    run: &Value,
    run_id: &str,
    project_id: &str,
    session_id: &str,
    candidate_id: &str,
    optimization_job_id: &str,
) -> Result<(), RuntimeError> {
    for (key, expected) in [
        ("schema_version", "DesignActionRun@1"),
        ("run_id", run_id),
        ("project_id", project_id),
        ("session_id", session_id),
        ("candidate_id", candidate_id),
    ] {
        if run.get(key).and_then(Value::as_str) != Some(expected) {
            return Err(RuntimeError::InvalidInput(format!(
                "OPTIMIZATION_PROPOSAL_PARENT_SCOPE_MISMATCH: {key}"
            )));
        }
    }
    if run.get("status").and_then(Value::as_str) != Some("completed") {
        return Err(RuntimeError::InvalidInput(
            "OPTIMIZATION_PROPOSAL_PARENT_NOT_COMPLETED".to_owned(),
        ));
    }
    if run.get("optimization_job_id").and_then(Value::as_str) != Some(optimization_job_id)
        || run
            .get("optimization_intent_sha256")
            .and_then(Value::as_str)
            .is_none()
    {
        return Err(RuntimeError::InvalidInput(
            "OPTIMIZATION_PROPOSAL_PARENT_CHILD_MISMATCH".to_owned(),
        ));
    }
    Ok(())
}

fn finalize_optimization_proposal_result(mut result: Value) -> Value {
    result["canonical_sha256"] = Value::String(String::new());
    result["canonical_sha256"] = Value::String(canonical_json_hash(&result));
    result
}

fn validate_optimization_proposal_result(result: &Value) -> Result<(), RuntimeError> {
    let object = result.as_object().ok_or_else(|| {
        RuntimeError::InvalidInput("OPTIMIZATION_PROPOSAL_RESULT_INVALID".to_owned())
    })?;
    if object.get("schema_version").and_then(Value::as_str)
        != Some("OptimizationProposalPrepareResult@1")
    {
        return Err(RuntimeError::InvalidInput(
            "OPTIMIZATION_PROPOSAL_RESULT_SCHEMA_INVALID".to_owned(),
        ));
    }
    for key in [
        "project_id",
        "session_id",
        "candidate_id",
        "run_id",
        "job_id",
        "proposal_job_id",
        "reason_code",
        "visual_status",
        "repair_apply_status",
    ] {
        if object
            .get(key)
            .and_then(Value::as_str)
            .is_none_or(|value| !is_opaque_id(value))
        {
            return Err(RuntimeError::InvalidInput(format!(
                "OPTIMIZATION_PROPOSAL_RESULT_{key}_INVALID"
            )));
        }
    }
    let view_spec_sha256 = object
        .get("view_spec_sha256")
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .ok_or_else(|| {
            RuntimeError::InvalidInput("OPTIMIZATION_PROPOSAL_RESULT_VIEW_SPEC_INVALID".to_owned())
        })?;
    let _ = view_spec_sha256;
    for key in ["optimizer_result_sha256", "source_artifact_sha256"] {
        if let Some(value) = object.get(key) {
            if !value.is_null() && !value.as_str().is_some_and(is_sha256) {
                return Err(RuntimeError::InvalidInput(format!(
                    "OPTIMIZATION_PROPOSAL_RESULT_{key}_INVALID"
                )));
            }
        }
    }
    for key in [
        "proposal_candidate_state_sha256",
        "proposal_program_sha256",
        "proposal_program_object_sha256",
        "proposal_artifact_sha256",
        "render_set_object_sha256",
        "comparison_report_object_sha256",
        "quality_report_object_sha256",
    ] {
        if !object
            .get(key)
            .is_some_and(|value| value.is_null() || value.as_str().is_some_and(is_sha256))
        {
            return Err(RuntimeError::InvalidInput(format!(
                "OPTIMIZATION_PROPOSAL_RESULT_{key}_INVALID"
            )));
        }
    }
    if let Some(value) = object.get("proposal_candidate_id") {
        if !value.is_null() && !value.as_str().is_some_and(is_opaque_id) {
            return Err(RuntimeError::InvalidInput(
                "OPTIMIZATION_PROPOSAL_RESULT_CANDIDATE_INVALID".to_owned(),
            ));
        }
    }
    for key in ["quality_report_id"] {
        if let Some(value) = object.get(key) {
            if !value.is_null() && !value.as_str().is_some_and(is_opaque_id) {
                return Err(RuntimeError::InvalidInput(format!(
                    "OPTIMIZATION_PROPOSAL_RESULT_{key}_INVALID"
                )));
            }
        }
    }
    for key in [
        "strict_improvement",
        "non_regressing",
        "visual_gate_passed",
        "confirm_allowed",
        "source_candidate_unchanged",
        "persistent_user_data_touched",
        "version_created",
    ] {
        if object.get(key).and_then(Value::as_bool).is_none() {
            return Err(RuntimeError::InvalidInput(format!(
                "OPTIMIZATION_PROPOSAL_RESULT_{key}_INVALID"
            )));
        }
    }
    if object.get("confirm_allowed") != Some(&Value::Bool(false))
        || object.get("source_candidate_unchanged") != Some(&Value::Bool(true))
        || object.get("persistent_user_data_touched") != Some(&Value::Bool(false))
        || object.get("version_created") != Some(&Value::Bool(false))
    {
        return Err(RuntimeError::InvalidInput(
            "OPTIMIZATION_PROPOSAL_RESULT_FAIL_CLOSED_REQUIRED".to_owned(),
        ));
    }
    if !matches!(
        object.get("status").and_then(Value::as_str),
        Some("not-ready" | "blocked" | "proposed")
    ) || !matches!(
        object.get("visual_status").and_then(Value::as_str),
        Some("NOT_RUN" | "QUALITY_TARGET_NOT_MET" | "PARTIAL_VISIBLE_VIEW_PASS")
    ) {
        return Err(RuntimeError::InvalidInput(
            "OPTIMIZATION_PROPOSAL_RESULT_STATUS_INVALID".to_owned(),
        ));
    }
    if object.get("status").and_then(Value::as_str) == Some("proposed") {
        if object.get("strict_improvement") != Some(&Value::Bool(true))
            || object.get("non_regressing") != Some(&Value::Bool(true))
            || object
                .get("proposal_candidate_id")
                .is_none_or(Value::is_null)
            || object.get("visual_status").and_then(Value::as_str) == Some("NOT_RUN")
        {
            return Err(RuntimeError::InvalidInput(
                "OPTIMIZATION_PROPOSAL_RESULT_PROPOSED_FIELDS_INVALID".to_owned(),
            ));
        }
    } else if object
        .get("proposal_candidate_id")
        .is_some_and(|value| !value.is_null())
    {
        return Err(RuntimeError::InvalidInput(
            "OPTIMIZATION_PROPOSAL_RESULT_BLOCKED_HAS_CANDIDATE".to_owned(),
        ));
    }
    let canonical = object
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .ok_or_else(|| {
            RuntimeError::InvalidInput("OPTIMIZATION_PROPOSAL_RESULT_CANONICAL_INVALID".to_owned())
        })?;
    let mut canonical_input = result.clone();
    canonical_input["canonical_sha256"] = Value::String(String::new());
    if canonical_json_hash(&canonical_input) != canonical {
        return Err(RuntimeError::InvalidInput(
            "OPTIMIZATION_PROPOSAL_RESULT_CANONICAL_HASH_MISMATCH".to_owned(),
        ));
    }
    Ok(())
}

fn mark_optimization_proposal_failed(
    runtime: &Runtime,
    job_id: &str,
    error: &RuntimeError,
) -> Result<(), RuntimeError> {
    let Some(current) = runtime.store.get_job_record(job_id)? else {
        return Ok(());
    };
    if matches!(
        current.status.as_str(),
        "succeeded" | "failed" | "cancelled"
    ) {
        return Ok(());
    }
    let next = JobRecord {
        schema_version: current.schema_version,
        job_id: current.job_id,
        project_id: current.project_id,
        kind: current.kind,
        status: "failed".to_owned(),
        progress: current.progress,
        request_sha256: current.request_sha256,
        checkpoint_sha256: current.checkpoint_sha256,
        error_code: Some("OPTIMIZATION_PROPOSAL_MATERIALIZATION_FAILED".to_owned()),
        created_at: current.created_at,
        updated_at: now_string(),
    };
    runtime.store.update_job_with_event(
        &next,
        "optimization_proposal_materialization_failed",
        &json!({
            "code":"OPTIMIZATION_PROPOSAL_MATERIALIZATION_FAILED",
            "detail":error.to_string().chars().take(256).collect::<String>()
        }),
        &[],
    )?;
    Ok(())
}

fn spawn_optimization_job(runtime: &Runtime, job_id: &str) {
    let should_spawn = active_jobs()
        .lock()
        .map(|mut jobs| jobs.insert(job_id.to_owned()))
        .unwrap_or(false);
    if !should_spawn {
        return;
    }
    let job_id = job_id.to_owned();
    let store = runtime.store.clone();
    let roots = runtime.reference_attachment_roots.clone();
    let _ = thread::Builder::new()
        .name(format!("forgecad-optimization-{job_id}"))
        .spawn(move || {
            match Runtime::from_store_with_attachment_roots(store, roots) {
                Ok(runtime) => {
                    if let Err(error) = run_optimization_job(&runtime, &job_id) {
                        if !error.to_string().contains("OPTIMIZATION_CANCELLED") {
                            let _ = mark_optimization_failed(&runtime, &job_id);
                            eprintln!("ForgeCAD optimization job failed: {error}");
                        }
                    }
                }
                Err(error) => {
                    eprintln!("ForgeCAD optimization Runtime failed: {error}");
                }
            }
            if let Ok(mut jobs) = active_jobs().lock() {
                jobs.remove(&job_id);
            }
        });
}

fn mark_optimization_failed(runtime: &Runtime, job_id: &str) -> Result<(), RuntimeError> {
    let Some(current) = runtime.store.get_job_record(job_id)? else {
        return Ok(());
    };
    if matches!(
        current.status.as_str(),
        "succeeded" | "failed" | "cancelled"
    ) {
        return Ok(());
    }
    let next = JobRecord {
        schema_version: current.schema_version,
        job_id: current.job_id,
        project_id: current.project_id,
        kind: current.kind,
        status: "failed".to_owned(),
        progress: current.progress,
        request_sha256: current.request_sha256,
        checkpoint_sha256: current.checkpoint_sha256,
        error_code: Some("OPTIMIZATION_RUNTIME_FAILED".to_owned()),
        created_at: current.created_at,
        updated_at: now_string(),
    };
    runtime.store.update_job_with_event(
        &next,
        "optimization_failed",
        &json!({"code":"OPTIMIZATION_RUNTIME_FAILED"}),
        &[],
    )?;
    Ok(())
}

fn run_optimization_job(runtime: &Runtime, job_id: &str) -> Result<(), RuntimeError> {
    let job = runtime.store.get_job_record(job_id)?.ok_or_else(|| {
        RuntimeError::InvalidInput("NOT_FOUND: optimization job not found".to_owned())
    })?;
    if job.kind != OPTIMIZATION_KIND || job.status != "queued" {
        return Ok(());
    }
    let intent = read_json_object(runtime, &job.request_sha256, "optimization-intent")?;
    let project_id = required_id_from(&intent, "project_id")?;
    let candidate_id = required_id_from(&intent, "candidate_id")?;
    let context = validate_intent(
        runtime,
        &intent,
        &job.request_sha256,
        project_id,
        candidate_id,
    )?;
    let started_at = Instant::now();
    let Some(_claimed) = runtime.store.claim_job_running(
        job_id,
        &now_string(),
        &json!({"intent_sha256":job.request_sha256,"candidate_id":candidate_id}),
    )?
    else {
        return Ok(());
    };
    let fidelity = intent.get("fidelity").expect("validated fidelity");
    let coarse_count = fidelity["coarse_evaluations"].as_u64().unwrap_or(32) as usize;
    let mid_top_k = fidelity["mid_top_k"].as_u64().unwrap_or(4) as usize;
    let final_top_k = fidelity["final_top_k"].as_u64().unwrap_or(2) as usize;
    let max_runtime_ms = intent["budget"]["max_runtime_ms"]
        .as_u64()
        .unwrap_or(120_000);
    let checkpoint = job
        .checkpoint_sha256
        .as_deref()
        .map(|hash| {
            load_optimization_checkpoint(
                runtime,
                hash,
                job_id,
                &job.request_sha256,
                coarse_count,
                mid_top_k,
                final_top_k,
            )
        })
        .transpose()?;
    if let Some(checkpoint) = checkpoint.as_ref() {
        validate_evaluation_objective_checkpoint(
            &checkpoint.evaluations,
            context.evaluation_objective_sha256.as_deref(),
        )?;
    }
    let baseline = baseline_parameters(&context.rig);
    let parameter_sets = deterministic_parameter_sets(&context.rig, &baseline, coarse_count);
    if parameter_sets.len() != coarse_count {
        return Err(RuntimeError::InvalidInput(
            "OPTIMIZATION_PARAMETER_SET_COUNT_MISMATCH".to_owned(),
        ));
    }
    let exploration_count = exploration_candidate_count(&context.rig, coarse_count);
    let mut candidates = if let Some(checkpoint) = checkpoint.as_ref() {
        load_checkpoint_candidates(runtime, checkpoint, &context)?
    } else {
        let mut candidates = Vec::with_capacity(coarse_count);
        for parameters in &parameter_sets {
            check_run_budget(runtime, job_id, started_at, max_runtime_ms)?;
            candidates.push(compile_candidate(
                runtime,
                &context,
                parameters.clone(),
                candidate_residual(&context, candidates.len()),
            )?);
        }
        candidates
    };
    if candidates.len() != coarse_count {
        return Err(RuntimeError::InvalidInput(
            "OPTIMIZATION_CHECKPOINT_CANDIDATE_COUNT_MISMATCH".to_owned(),
        ));
    }
    let mut evaluations = checkpoint
        .as_ref()
        .map(|value| value.evaluations.clone())
        .unwrap_or_default();
    let mut next_stage = checkpoint
        .as_ref()
        .map(|value| value.next_stage.as_str())
        .unwrap_or("coarse");
    let mut candidate_reachable = candidate_reachable_hashes(&candidates);
    if checkpoint.is_none() {
        persist_optimization_state(
            runtime,
            job_id,
            &context,
            &evaluations,
            &candidates,
            "running",
            1,
            None,
            candidate_reachable.clone(),
            "optimization_candidates_checkpoint",
            "coarse",
        )?;
    }

    if next_stage == "coarse" {
        // First evaluate a deterministic space-filling seed set.  Only after
        // those low-fidelity measurements exist do we spend the remaining
        // coarse budget on a local trust-region around the best seed.  This
        // makes the job an optimizer rather than a fixed list of coordinate
        // perturbations while keeping every candidate slot checkpointable.
        for candidate_index in 0..exploration_count {
            if evaluation_for_candidate(&evaluations, candidate_index, "coarse").is_some() {
                continue;
            }
            check_run_budget(runtime, job_id, started_at, max_runtime_ms)?;
            let evaluation = evaluate_candidate(
                runtime,
                &context,
                &candidates[candidate_index],
                candidate_index,
                evaluations.len() + 1,
                "coarse",
                128,
                started_at,
            )?;
            evaluations.push(evaluation);
            persist_optimization_state(
                runtime,
                job_id,
                &context,
                &evaluations,
                &candidates,
                "running",
                stage_progress("coarse", evaluations.len(), coarse_count, mid_top_k),
                None,
                candidate_reachable.clone(),
                "optimization_coarse_evaluation",
                "coarse",
            )?;
        }
        let exploration_complete = (0..exploration_count).all(|candidate_index| {
            evaluation_for_candidate(&evaluations, candidate_index, "coarse").is_some()
        });
        let refinement_started = evaluations.iter().any(|evaluation| {
            evaluation.fidelity == "coarse" && evaluation.candidate_index >= exploration_count
        });
        if exploration_complete && !refinement_started && exploration_count < coarse_count {
            let mut ranked_seeds = evaluations
                .iter()
                .filter(|evaluation| {
                    evaluation.fidelity == "coarse"
                        && evaluation.candidate_index < exploration_count
                })
                .collect::<Vec<_>>();
            ranked_seeds.sort_by(|left, right| {
                compare_evaluation_quality_for_context(&context, left, right)
                    .then_with(|| left.candidate_index.cmp(&right.candidate_index))
            });
            let seed_index = ranked_seeds
                .first()
                .map(|evaluation| evaluation.candidate_index)
                .ok_or_else(|| {
                    RuntimeError::InvalidInput("OPTIMIZATION_NO_VALID_SEED_EVALUATION".to_owned())
                })?;
            let local_sets = adaptive_parameter_sets(
                &context.rig,
                &parameter_sets[seed_index],
                coarse_count - exploration_count,
            );
            for (offset, parameters) in local_sets.into_iter().enumerate() {
                let candidate_index = exploration_count + offset;
                check_run_budget(runtime, job_id, started_at, max_runtime_ms)?;
                candidates[candidate_index] = compile_candidate(
                    runtime,
                    &context,
                    parameters,
                    candidate_residual(&context, candidate_index),
                )?;
            }
            candidate_reachable = candidate_reachable_hashes(&candidates);
        }
        for candidate_index in exploration_count..coarse_count {
            if evaluation_for_candidate(&evaluations, candidate_index, "coarse").is_some() {
                continue;
            }
            check_run_budget(runtime, job_id, started_at, max_runtime_ms)?;
            let evaluation = evaluate_candidate(
                runtime,
                &context,
                &candidates[candidate_index],
                candidate_index,
                evaluations.len() + 1,
                "coarse",
                128,
                started_at,
            )?;
            evaluations.push(evaluation);
            persist_optimization_state(
                runtime,
                job_id,
                &context,
                &evaluations,
                &candidates,
                "running",
                stage_progress("coarse", evaluations.len(), coarse_count, mid_top_k),
                None,
                candidate_reachable.clone(),
                "optimization_coarse_refinement_evaluation",
                "coarse",
            )?;
        }
        persist_optimization_state(
            runtime,
            job_id,
            &context,
            &evaluations,
            &candidates,
            "running",
            55,
            None,
            candidate_reachable.clone(),
            "optimization_coarse_checkpoint",
            "mid",
        )?;
        next_stage = "mid";
    }

    if next_stage == "mid" {
        let mut ranked = evaluations
            .iter()
            .filter(|evaluation| evaluation.fidelity == "coarse")
            .collect::<Vec<_>>();
        ranked.sort_by(|left, right| {
            compare_evaluation_quality_for_context(&context, left, right)
                .then_with(|| left.candidate_index.cmp(&right.candidate_index))
        });
        let mid_indices = ranked
            .iter()
            .take(mid_top_k)
            .map(|evaluation| evaluation.candidate_index)
            .collect::<Vec<_>>();
        if mid_indices.is_empty() {
            return Err(RuntimeError::InvalidInput(
                "OPTIMIZATION_NO_VALID_COARSE_EVALUATION".to_owned(),
            ));
        }
        for candidate_index in mid_indices.clone() {
            if evaluation_for_candidate(&evaluations, candidate_index, "mid").is_some() {
                continue;
            }
            check_run_budget(runtime, job_id, started_at, max_runtime_ms)?;
            let evaluation = evaluate_candidate(
                runtime,
                &context,
                &candidates[candidate_index],
                candidate_index,
                evaluations.len() + 1,
                "mid",
                256,
                started_at,
            )?;
            evaluations.push(evaluation);
            let completed_mid = evaluations
                .iter()
                .filter(|evaluation| evaluation.fidelity == "mid")
                .count();
            persist_optimization_state(
                runtime,
                job_id,
                &context,
                &evaluations,
                &candidates,
                "running",
                stage_progress("mid", completed_mid, mid_top_k, mid_top_k),
                None,
                candidate_reachable.clone(),
                "optimization_mid_evaluation",
                "mid",
            )?;
        }
        persist_optimization_state(
            runtime,
            job_id,
            &context,
            &evaluations,
            &candidates,
            "running",
            75,
            None,
            candidate_reachable.clone(),
            "optimization_mid_checkpoint",
            "final",
        )?;
        next_stage = "final";
    }

    if next_stage != "final" {
        return Err(RuntimeError::InvalidInput(
            "OPTIMIZATION_CHECKPOINT_STAGE_INVALID".to_owned(),
        ));
    }
    let mut ranked = evaluations
        .iter()
        .filter(|evaluation| evaluation.fidelity == "mid")
        .collect::<Vec<_>>();
    ranked.sort_by(|left, right| {
        compare_evaluation_quality_for_context(&context, left, right)
            .then_with(|| left.candidate_index.cmp(&right.candidate_index))
    });
    let baseline_index = 0usize;
    let mut final_indices = select_final_proposal_indices(
        &ranked,
        baseline_index,
        final_top_k,
        context.evaluation_objective.is_some(),
    );
    if final_indices.is_empty() {
        return Err(RuntimeError::InvalidInput(
            "OPTIMIZATION_NO_VALID_MID_PROPOSAL_EVALUATION".to_owned(),
        ));
    }
    // The unmodified candidate is always the final control, even when it was
    // present in the mid-ranked set.  `final_top_k` therefore means the number
    // of proposal finalists, not the total number of final renders.
    final_indices.push(baseline_index);
    let final_evaluation_target = final_indices.len();
    for candidate_index in final_indices {
        if evaluation_for_candidate(&evaluations, candidate_index, "final").is_some() {
            continue;
        }
        check_run_budget(runtime, job_id, started_at, max_runtime_ms)?;
        let evaluation = evaluate_candidate(
            runtime,
            &context,
            &candidates[candidate_index],
            candidate_index,
            evaluations.len() + 1,
            "final",
            512,
            started_at,
        )?;
        evaluations.push(evaluation);
        let completed_final = evaluations
            .iter()
            .filter(|evaluation| evaluation.fidelity == "final")
            .count();
        persist_optimization_state(
            runtime,
            job_id,
            &context,
            &evaluations,
            &candidates,
            "running",
            stage_progress(
                "final",
                completed_final,
                final_evaluation_target,
                final_top_k,
            ),
            None,
            candidate_reachable.clone(),
            "optimization_final_evaluation",
            "final",
        )?;
    }
    let baseline_final = evaluation_for_candidate(&evaluations, baseline_index, "final")
        .ok_or_else(|| {
            RuntimeError::InvalidInput("OPTIMIZATION_BASELINE_FINAL_MISSING".to_owned())
        })?;
    let best_final = evaluations
        .iter()
        .filter(|evaluation| evaluation.fidelity == "final")
        .filter(|evaluation| {
            evaluation.candidate_index == baseline_index
                || compare_final_evaluations_for_context(baseline_final, evaluation, &context)
                    .non_regressing
        })
        .min_by(|left, right| compare_evaluation_quality_for_context(&context, left, right))
        .ok_or_else(|| {
            RuntimeError::InvalidInput("OPTIMIZATION_FINAL_EVALUATION_MISSING".to_owned())
        })?;
    let final_comparison =
        compare_final_evaluations_for_context(baseline_final, best_final, &context);
    let strict_improvement = final_comparison.strict_improvement;
    let mut final_objects = Vec::new();
    let mut proposal = Vec::new();
    if strict_improvement {
        let best_candidate = &candidates[best_final.candidate_index];
        // Reuse the exact objects that were compiled, rendered and evaluated.
        // Re-materializing a draft here can change the canonical/readback lineage
        // even when the visible geometry is otherwise identical.
        final_objects.push(best_candidate.program_object_sha256.clone());
        final_objects.push(best_candidate.artifact_object_sha256.clone());
        proposal.push(best_candidate.program_object_sha256.clone());
        proposal.push(best_candidate.artifact_object_sha256.clone());
    }
    let reasons = if strict_improvement {
        Vec::new()
    } else if context.evaluation_objective.is_some() {
        vec!["blocked_global_or_part_objective".to_owned()]
    } else if !final_comparison.non_regressing {
        vec!["final-search-regressed-a-locked-metric".to_owned()]
    } else {
        vec!["final-search-did-not-strictly-improve-multi-objective".to_owned()]
    };
    persist_optimization_state(
        runtime,
        job_id,
        &context,
        &evaluations,
        &candidates,
        "succeeded",
        100,
        Some((
            strict_improvement,
            final_comparison.non_regressing,
            proposal,
            reasons,
        )),
        candidate_reachable
            .into_iter()
            .chain(final_objects)
            .collect(),
        "optimization_completed",
        "done",
    )?;
    Ok(())
}

fn request_object<'a>(
    request: &'a Value,
    method: &str,
) -> Result<&'a Map<String, Value>, RuntimeError> {
    request
        .as_object()
        .ok_or_else(|| RuntimeError::InvalidInput(format!("{method}: request must be an object")))
}

fn reject_unknown_keys(object: &Map<String, Value>, allowed: &[&str]) -> Result<(), RuntimeError> {
    if object.keys().any(|key| !allowed.contains(&key.as_str())) {
        return Err(RuntimeError::InvalidInput(
            "OPTIMIZATION_REQUEST_UNKNOWN_FIELD".to_owned(),
        ));
    }
    Ok(())
}

fn required_id<'a>(object: &'a Map<String, Value>, key: &str) -> Result<&'a str, RuntimeError> {
    required_id_from_object(object, key)
}

fn required_id_from<'a>(value: &'a Value, key: &str) -> Result<&'a str, RuntimeError> {
    let object = value.as_object().ok_or_else(|| {
        RuntimeError::InvalidInput("OPTIMIZATION_CONTRACT_INVALID: object required".to_owned())
    })?;
    required_id_from_object(object, key)
}

fn required_id_from_object<'a>(
    object: &'a Map<String, Value>,
    key: &str,
) -> Result<&'a str, RuntimeError> {
    let value = object.get(key).and_then(Value::as_str).ok_or_else(|| {
        RuntimeError::InvalidInput(format!("{key} is required and must be an opaque id"))
    })?;
    if !is_opaque_id(value) {
        return Err(RuntimeError::InvalidInput(format!(
            "{key} is required and must be an opaque id"
        )));
    }
    Ok(value)
}

fn required_sha<'a>(object: &'a Map<String, Value>, key: &str) -> Result<&'a str, RuntimeError> {
    let value = object.get(key).and_then(Value::as_str).ok_or_else(|| {
        RuntimeError::InvalidInput(format!("{key} is required and must be a SHA-256"))
    })?;
    if !is_sha256(value) {
        return Err(RuntimeError::InvalidInput(format!(
            "{key} is required and must be a SHA-256"
        )));
    }
    Ok(value)
}

fn validate_approval(object: &Map<String, Value>) -> Result<(), RuntimeError> {
    if object.get("approved") != Some(&Value::Bool(true)) {
        return Err(RuntimeError::InvalidInput(
            "OPTIMIZATION_APPROVAL_REQUIRED".to_owned(),
        ));
    }
    let receipt = object
        .get("approval_receipt_id")
        .and_then(Value::as_str)
        .filter(|value| is_opaque_id(value))
        .ok_or_else(|| RuntimeError::InvalidInput("approval_receipt_id is required".to_owned()))?;
    let _ = receipt;
    let summary = object
        .get("approval_summary")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty() && value.len() <= 512)
        .ok_or_else(|| RuntimeError::InvalidInput("approval_summary is required".to_owned()))?;
    let _ = summary;
    let expires = object
        .get("approval_expires_at")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty() && value.len() <= 64)
        .ok_or_else(|| RuntimeError::InvalidInput("approval_expires_at is required".to_owned()))?;
    let _ = expires;
    let session = object
        .get("approval_session_id")
        .and_then(Value::as_str)
        .filter(|value| is_opaque_id(value))
        .ok_or_else(|| RuntimeError::InvalidInput("approval_session_id is required".to_owned()))?;
    let _ = session;
    let idempotency = object
        .get("idempotency_key")
        .and_then(Value::as_str)
        .filter(|value| is_opaque_id(value))
        .ok_or_else(|| RuntimeError::InvalidInput("idempotency_key is required".to_owned()))?;
    let _ = idempotency;
    Ok(())
}

fn read_json_object(runtime: &Runtime, sha256: &str, kind: &str) -> Result<Value, RuntimeError> {
    if !is_sha256(sha256) {
        return Err(RuntimeError::InvalidInput(format!(
            "{kind}: invalid CAS hash"
        )));
    }
    let object = runtime
        .store
        .get_object(sha256)?
        .ok_or_else(|| RuntimeError::InvalidInput(format!("{kind}: CAS object unavailable")))?;
    if object.mime != "application/json"
        || (!object.kind.starts_with(kind) && kind != "optimization-result")
    {
        return Err(RuntimeError::InvalidInput(format!(
            "{kind}: CAS object metadata mismatch"
        )));
    }
    serde_json::from_slice(&runtime.cas_read(sha256)?).map_err(|error| {
        RuntimeError::InvalidInput(format!("{kind}: invalid JSON payload: {error}"))
    })
}

fn read_evaluation_objective(
    runtime: &Runtime,
    objective_sha256: &str,
) -> Result<Value, RuntimeError> {
    if !is_sha256(objective_sha256) {
        return Err(RuntimeError::InvalidInput(
            "OPTIMIZATION_EVALUATION_OBJECTIVE_HASH_INVALID".to_owned(),
        ));
    }
    let object = runtime.store.get_object(objective_sha256)?.ok_or_else(|| {
        RuntimeError::InvalidInput("OPTIMIZATION_EVALUATION_OBJECTIVE_UNAVAILABLE".to_owned())
    })?;
    if object.mime != "application/json"
        || !object.kind.starts_with("silhouette-evaluation-objective")
    {
        return Err(RuntimeError::InvalidInput(
            "OPTIMIZATION_EVALUATION_OBJECTIVE_METADATA_MISMATCH".to_owned(),
        ));
    }
    let bytes = runtime.cas_read(objective_sha256)?;
    if sha256_hex(&bytes) != objective_sha256 {
        return Err(RuntimeError::InvalidInput(
            "OPTIMIZATION_EVALUATION_OBJECTIVE_HASH_MISMATCH".to_owned(),
        ));
    }
    let value: Value = serde_json::from_slice(&bytes).map_err(|error| {
        RuntimeError::InvalidInput(format!(
            "OPTIMIZATION_EVALUATION_OBJECTIVE_INVALID: {error}"
        ))
    })?;
    super::validate_silhouette_evaluation_objective(&value)?;
    Ok(value)
}

fn validate_evaluation_objective_binding(
    _runtime: &Runtime,
    objective: &Value,
    _objective_sha256: &str,
    project_id: &str,
    candidate_id: &str,
    reference_id: &str,
    reference_sha256: &str,
    target_sha256: &str,
    camera_hash: &str,
    camera: &Value,
    part_id: &str,
) -> Result<(), RuntimeError> {
    let bindings = [
        ("project_id", project_id),
        ("baseline_candidate_id", candidate_id),
        ("reference_id", reference_id),
        ("reference_sha256", reference_sha256),
        ("global_target_sha256", target_sha256),
        ("part_id", part_id),
        ("camera_hash", camera_hash),
    ];
    for (key, expected) in bindings {
        if objective.get(key).and_then(Value::as_str) != Some(expected) {
            return Err(RuntimeError::InvalidInput(format!(
                "OPTIMIZATION_EVALUATION_OBJECTIVE_{key}_BINDING_MISMATCH"
            )));
        }
    }
    if objective
        .get("camera_canonical_sha256")
        .and_then(Value::as_str)
        != camera.get("canonical_sha256").and_then(Value::as_str)
    {
        return Err(RuntimeError::InvalidInput(
            "OPTIMIZATION_EVALUATION_OBJECTIVE_CAMERA_CANONICAL_MISMATCH".to_owned(),
        ));
    }
    Ok(())
}

fn validate_intent(
    runtime: &Runtime,
    intent: &Value,
    intent_sha256: &str,
    project_id: &str,
    candidate_id: &str,
) -> Result<OptimizationContext, RuntimeError> {
    let object = intent.as_object().ok_or_else(|| {
        RuntimeError::InvalidInput("OPTIMIZATION_INTENT_INVALID: object required".to_owned())
    })?;
    let allowed = [
        "schema_version",
        "intent_id",
        "action_run_id",
        "job_id",
        "project_id",
        "candidate_id",
        "reference_id",
        "reference_sha256",
        "program_sha256",
        "target_sha256",
        "evaluation_objective_sha256",
        "camera",
        "camera_hash",
        "part_id",
        "stage",
        "rig",
        "fidelity",
        "budget",
        "objective",
        "residual",
        "canonical_sha256",
    ];
    reject_unknown_keys(object, &allowed)?;
    if object.get("schema_version").and_then(Value::as_str) != Some("OptimizationIntent@1") {
        return Err(RuntimeError::InvalidInput(
            "OPTIMIZATION_INTENT_INVALID: schema_version".to_owned(),
        ));
    }
    for key in [
        "intent_id",
        "job_id",
        "project_id",
        "candidate_id",
        "reference_id",
        "part_id",
    ] {
        let _ = required_id(object, key)?;
    }
    if let Some(action_run_id) = object.get("action_run_id") {
        if !action_run_id.is_null() && !action_run_id.as_str().is_some_and(is_opaque_id) {
            return Err(RuntimeError::InvalidInput(
                "OPTIMIZATION_ACTION_RUN_ID_INVALID".to_owned(),
            ));
        }
    }
    if object.get("project_id").and_then(Value::as_str) != Some(project_id)
        || object.get("candidate_id").and_then(Value::as_str) != Some(candidate_id)
    {
        return Err(RuntimeError::InvalidInput(
            "OPTIMIZATION_INTENT_SCOPE_DENIED".to_owned(),
        ));
    }
    let reference_sha256 = required_sha(object, "reference_sha256")?;
    let program_sha256 = required_sha(object, "program_sha256")?;
    let target_sha256 = required_sha(object, "target_sha256")?;
    let evaluation_objective_sha256 = object
        .get("evaluation_objective_sha256")
        .map(|_| required_sha(object, "evaluation_objective_sha256"))
        .transpose()?
        .map(str::to_owned);
    let camera_hash = required_sha(object, "camera_hash")?;
    let canonical_sha256 = required_sha(object, "canonical_sha256")?;
    let mut canonical = intent.clone();
    canonical["canonical_sha256"] = Value::String(String::new());
    if canonical_json_hash(&canonical) != canonical_sha256 {
        return Err(RuntimeError::InvalidInput(
            "OPTIMIZATION_INTENT_CANONICAL_HASH_MISMATCH".to_owned(),
        ));
    }
    if !is_sha256(intent_sha256) {
        return Err(RuntimeError::InvalidInput(
            "OPTIMIZATION_INTENT_OBJECT_HASH_INVALID".to_owned(),
        ));
    }
    let camera_input = object.get("camera").ok_or_else(|| {
        RuntimeError::InvalidInput("OPTIMIZATION_INTENT_CAMERA_REQUIRED".to_owned())
    })?;
    // A JSON client can preserve the Runtime-owned camera hashes while
    // changing only the shortest decimal spelling of f64 leaves. Resolve that
    // identity from the candidate/target cache instead of promoting the
    // round-tripped float payload to a new camera hash. Complete calibrations
    // that already validate remain authoritative; the fallback requires both
    // Runtime-owned hashes and an exact candidate/target cache match.
    let camera = if validate_camera_calibration(camera_input).is_ok() {
        camera_input.clone()
    } else {
        let camera_object = camera_input.as_object().ok_or_else(|| {
            RuntimeError::InvalidInput("OPTIMIZATION_CAMERA_INVALID".to_owned())
        })?;
        let camera_ref = json!({
            "schema_version": "CameraCalibrationRef@1",
            "camera_hash": camera_object.get("camera_hash").cloned().unwrap_or(Value::Null),
            "canonical_sha256": camera_object
                .get("canonical_sha256")
                .cloned()
                .unwrap_or(Value::Null),
        });
        match runtime.resolve_silhouette_fit_camera(
            project_id,
            candidate_id,
            target_sha256,
            &camera_ref,
        ) {
            Ok(camera) => camera,
            Err(global_error) => {
                // A unified objective may deliberately fit the camera against
                // its refined Part target while the intent remains globally
                // bound.  Keep the fallback Runtime-owned: read the immutable
                // objective, take only its typed Part-target hash, and resolve
                // the same camera identity from that candidate/target cache.
                let Some(objective_sha256) = evaluation_objective_sha256.as_deref() else {
                    return Err(global_error);
                };
                let objective = read_evaluation_objective(runtime, objective_sha256)?;
                let part_target_sha256 = objective
                    .get("part_target_sha256")
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        RuntimeError::InvalidInput(
                            "OPTIMIZATION_EVALUATION_OBJECTIVE_PART_TARGET_MISSING".to_owned(),
                        )
                    })?;
                runtime
                    .resolve_silhouette_fit_camera(
                        project_id,
                        candidate_id,
                        part_target_sha256,
                        &camera_ref,
                    )
                    .map_err(|_| global_error)?
            }
        }
    };
    if camera.get("camera_hash").and_then(Value::as_str) != Some(camera_hash) {
        return Err(RuntimeError::InvalidInput(
            "OPTIMIZATION_CAMERA_BINDING_MISMATCH".to_owned(),
        ));
    }
    let rig = object
        .get("rig")
        .ok_or_else(|| RuntimeError::InvalidInput("OPTIMIZATION_INTENT_RIG_REQUIRED".to_owned()))?;
    validate_silhouette_rig(rig, candidate_id)?;
    let parameters = rig
        .get("parameters")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            RuntimeError::InvalidInput("OPTIMIZATION_RIG_PARAMETERS_REQUIRED".to_owned())
        })?;
    if !(4..=12).contains(&parameters.len()) {
        return Err(RuntimeError::InvalidInput(
            "OPTIMIZATION_RIG_PARAMETER_BUDGET".to_owned(),
        ));
    }
    let part_id = object
        .get("part_id")
        .and_then(Value::as_str)
        .ok_or_else(|| RuntimeError::InvalidInput("part_id is required".to_owned()))?;
    if parameters
        .iter()
        .any(|parameter| parameter.get("part_id").and_then(Value::as_str) != Some(part_id))
    {
        return Err(RuntimeError::InvalidInput(
            "OPTIMIZATION_RIG_MUST_TARGET_ONE_PART".to_owned(),
        ));
    }
    if object.get("stage").and_then(Value::as_str) != Some("primary-form")
        && object.get("stage").and_then(Value::as_str) != Some("secondary-structure")
        && object.get("stage").and_then(Value::as_str) != Some("tertiary-detail")
        && object.get("stage").and_then(Value::as_str) != Some("uv-pbr")
        && object.get("stage").and_then(Value::as_str) != Some("final-review")
    {
        return Err(RuntimeError::InvalidInput(
            "OPTIMIZATION_STAGE_INVALID".to_owned(),
        ));
    }
    let fidelity = object
        .get("fidelity")
        .and_then(Value::as_object)
        .ok_or_else(|| RuntimeError::InvalidInput("OPTIMIZATION_FIDELITY_INVALID".to_owned()))?;
    if fidelity.get("coarse_resolution").and_then(Value::as_u64) != Some(128)
        || fidelity.get("mid_resolution").and_then(Value::as_u64) != Some(256)
        || fidelity.get("final_resolution").and_then(Value::as_u64) != Some(512)
    {
        return Err(RuntimeError::InvalidInput(
            "OPTIMIZATION_FIDELITY_RESOLUTION_INVALID".to_owned(),
        ));
    }
    let coarse_count = fidelity
        .get("coarse_evaluations")
        .and_then(Value::as_u64)
        .filter(|value| (32..=48).contains(value))
        .ok_or_else(|| {
            RuntimeError::InvalidInput("OPTIMIZATION_COARSE_BUDGET_INVALID".to_owned())
        })?;
    let mid_top_k = fidelity
        .get("mid_top_k")
        .and_then(Value::as_u64)
        .filter(|value| (4..=8).contains(value))
        .ok_or_else(|| RuntimeError::InvalidInput("OPTIMIZATION_MID_BUDGET_INVALID".to_owned()))?;
    if fidelity.get("final_top_k").and_then(Value::as_u64) != Some(2) {
        return Err(RuntimeError::InvalidInput(
            "OPTIMIZATION_FINAL_BUDGET_INVALID".to_owned(),
        ));
    }
    let budget = object
        .get("budget")
        .and_then(Value::as_object)
        .ok_or_else(|| RuntimeError::InvalidInput("OPTIMIZATION_BUDGET_INVALID".to_owned()))?;
    let max_evaluations = budget
        .get("max_evaluations")
        .and_then(Value::as_u64)
        .filter(|value| (42..=64).contains(value))
        .ok_or_else(|| {
            RuntimeError::InvalidInput("OPTIMIZATION_EVALUATION_BUDGET_INVALID".to_owned())
        })?;
    // `final_top_k` counts proposal finalists; the unmodified baseline is an
    // additional final control and is never allowed to disappear from the
    // final comparison.
    if coarse_count + mid_top_k + 3 > max_evaluations {
        return Err(RuntimeError::InvalidInput(
            "OPTIMIZATION_EVALUATION_BUDGET_TOO_SMALL".to_owned(),
        ));
    }
    let max_runtime_ms = budget
        .get("max_runtime_ms")
        .and_then(Value::as_u64)
        .filter(|value| (1_000..=120_000).contains(value))
        .ok_or_else(|| {
            RuntimeError::InvalidInput("OPTIMIZATION_RUNTIME_BUDGET_INVALID".to_owned())
        })?;
    let max_triangles = budget
        .get("max_output_triangles")
        .and_then(Value::as_u64)
        .filter(|value| (1..=2_000_000).contains(value))
        .ok_or_else(|| {
            RuntimeError::InvalidInput("OPTIMIZATION_TRIANGLE_BUDGET_INVALID".to_owned())
        })?;
    let _memory = budget
        .get("max_worker_memory_bytes")
        .and_then(Value::as_u64)
        .filter(|value| (1_048_576..=536_870_912).contains(value))
        .ok_or_else(|| {
            RuntimeError::InvalidInput("OPTIMIZATION_MEMORY_BUDGET_INVALID".to_owned())
        })?;
    let objective = object
        .get("objective")
        .and_then(Value::as_object)
        .ok_or_else(|| RuntimeError::InvalidInput("OPTIMIZATION_OBJECTIVE_INVALID".to_owned()))?;
    let weights = [
        "silhouette_iou",
        "boundary_f1_4px",
        "landmark_coverage",
        "landmark_nme",
        "part_region",
        "program_complexity",
    ];
    let weight_sum = weights
        .iter()
        .map(|key| {
            objective
                .get(*key)
                .and_then(Value::as_f64)
                .filter(|value| value.is_finite() && (0.0..=1.0).contains(value))
                .ok_or_else(|| {
                    RuntimeError::InvalidInput("OPTIMIZATION_OBJECTIVE_WEIGHT_INVALID".to_owned())
                })
        })
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .sum::<f64>();
    if (weight_sum - 1.0).abs() > 1.0e-6 {
        return Err(RuntimeError::InvalidInput(
            "OPTIMIZATION_OBJECTIVE_WEIGHTS_MUST_SUM_TO_ONE".to_owned(),
        ));
    }
    let candidate = runtime
        .candidate(candidate_id)?
        .ok_or_else(|| RuntimeError::InvalidInput("NOT_FOUND: candidate not found".to_owned()))?;
    if candidate.project_id != project_id {
        return Err(RuntimeError::InvalidInput(
            "OPTIMIZATION_CANDIDATE_SCOPE_DENIED".to_owned(),
        ));
    }
    let evidence = runtime
        .store
        .get_geometry_candidate_evidence(candidate_id)?
        .ok_or_else(|| {
            RuntimeError::InvalidInput("OPTIMIZATION_GEOMETRY_EVIDENCE_UNAVAILABLE".to_owned())
        })?;
    if evidence.project_id != project_id
        || evidence.reference_id.as_deref() != object.get("reference_id").and_then(Value::as_str)
        || evidence.reference_sha256.as_deref() != Some(reference_sha256)
        || evidence.geometry_program_sha256 != program_sha256
    {
        return Err(RuntimeError::InvalidInput(
            "OPTIMIZATION_LINEAGE_BINDING_MISMATCH".to_owned(),
        ));
    }
    let reference = runtime
        .reference(
            object["reference_id"]
                .as_str()
                .expect("validated reference_id"),
        )?
        .ok_or_else(|| {
            RuntimeError::InvalidInput("OPTIMIZATION_REFERENCE_UNAVAILABLE".to_owned())
        })?;
    if reference.project_id != project_id || reference.object_sha256 != reference_sha256 {
        return Err(RuntimeError::InvalidInput(
            "OPTIMIZATION_REFERENCE_SCOPE_DENIED".to_owned(),
        ));
    }
    let target = runtime.read_silhouette_target(target_sha256)?;
    if target.get("reference_id").and_then(Value::as_str) != Some(reference.reference_id.as_str())
        || target.get("reference_sha256").and_then(Value::as_str) != Some(reference_sha256)
    {
        return Err(RuntimeError::InvalidInput(
            "OPTIMIZATION_TARGET_BINDING_MISMATCH".to_owned(),
        ));
    }
    let (evaluation_objective, part_target_mask) = if let Some(objective_sha256) =
        evaluation_objective_sha256.as_deref()
    {
        let objective = read_evaluation_objective(runtime, objective_sha256)?;
        validate_evaluation_objective_binding(
            runtime,
            &objective,
            objective_sha256,
            project_id,
            candidate_id,
            object
                .get("reference_id")
                .and_then(Value::as_str)
                .expect("validated reference_id"),
            reference_sha256,
            target_sha256,
            camera_hash,
            &camera,
            part_id,
        )?;
        let part_target_sha256 = objective
            .get("part_target_sha256")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                RuntimeError::InvalidInput(
                    "OPTIMIZATION_EVALUATION_OBJECTIVE_PART_TARGET_MISSING".to_owned(),
                )
            })?;
        let part_target = runtime.read_silhouette_target(part_target_sha256)?;
        if part_target.get("reference_id").and_then(Value::as_str)
            != Some(
                object
                    .get("reference_id")
                    .and_then(Value::as_str)
                    .expect("validated reference_id"),
            )
            || part_target.get("reference_sha256").and_then(Value::as_str) != Some(reference_sha256)
            || part_target
                .get("parent_target_sha256")
                .and_then(Value::as_str)
                != Some(target_sha256)
        {
            return Err(RuntimeError::InvalidInput(
                "OPTIMIZATION_EVALUATION_OBJECTIVE_PART_TARGET_LINEAGE_MISMATCH".to_owned(),
            ));
        }
        let part_target_mask = runtime.target_mask(part_target_sha256, &part_target)?.mask;
        (Some(objective), Some(part_target_mask))
    } else {
        (None, None)
    };
    let residual = object.get("residual").cloned();
    let residual_target_sha256 = evaluation_objective
        .as_ref()
        .and_then(|objective| objective.get("part_target_sha256"))
        .and_then(Value::as_str)
        .unwrap_or(target_sha256);
    if let Some(residual) = residual.as_ref() {
        validate_optimization_residual(
            runtime,
            project_id,
            candidate_id,
            residual_target_sha256,
            part_id,
            residual,
        )?;
    }
    let residual_variants = residual
        .as_ref()
        .map(residual_variant_family)
        .transpose()?
        .unwrap_or_default();
    let target_mask = runtime.target_mask(target_sha256, &target)?.mask;
    let program_object = runtime
        .store
        .get_object(&evidence.geometry_program_object_sha256)?
        .ok_or_else(|| {
            RuntimeError::InvalidInput("OPTIMIZATION_PROGRAM_OBJECT_UNAVAILABLE".to_owned())
        })?;
    let program_bytes = runtime.cas_read(&program_object.sha256)?;
    if sha256_hex(&program_bytes) != evidence.geometry_program_object_sha256 {
        return Err(RuntimeError::InvalidInput(
            "OPTIMIZATION_PROGRAM_OBJECT_HASH_MISMATCH".to_owned(),
        ));
    }
    let mut program: Value = serde_json::from_slice(&program_bytes).map_err(|_| {
        RuntimeError::InvalidInput("OPTIMIZATION_PROGRAM_OBJECT_INVALID".to_owned())
    })?;
    let hash_result = super::hash_geometry_program_with_runtime_worker(&program)
        .map_err(|_| RuntimeError::InvalidInput("OPTIMIZATION_PROGRAM_HASH_FAILED".to_owned()))?;
    if hash_result.get("canonical_sha256").and_then(Value::as_str) != Some(program_sha256) {
        return Err(RuntimeError::InvalidInput(
            "OPTIMIZATION_PROGRAM_LINEAGE_MISMATCH".to_owned(),
        ));
    }
    program["canonical_sha256"] = Value::String(program_sha256.to_owned());
    let artifact_sha256 = evidence.artifact_object_sha256.clone();
    if candidate.prepared_object_sha256.as_deref() != Some(artifact_sha256.as_str())
        && candidate.manifest_hash.as_deref() != Some(artifact_sha256.as_str())
    {
        return Err(RuntimeError::InvalidInput(
            "OPTIMIZATION_ARTIFACT_BINDING_MISMATCH".to_owned(),
        ));
    }
    let _ = max_runtime_ms;
    let _ = max_triangles;
    Ok(OptimizationContext {
        intent: intent.clone(),
        intent_sha256: intent_sha256.to_owned(),
        target,
        target_mask,
        evaluation_objective,
        evaluation_objective_sha256,
        part_target_mask,
        program,
        camera: camera.clone(),
        rig: rig.clone(),
        part_id: part_id.to_owned(),
        residual_variants,
    })
}

fn validate_optimization_residual(
    runtime: &Runtime,
    project_id: &str,
    candidate_id: &str,
    target_sha256: &str,
    part_id: &str,
    residual: &Value,
) -> Result<(), RuntimeError> {
    let object = residual.as_object().ok_or_else(|| {
        RuntimeError::InvalidInput("OPTIMIZATION_RESIDUAL_INVALID: object required".to_owned())
    })?;
    reject_unknown_keys(
        object,
        &[
            "schema_version",
            "part_id",
            "node_id",
            "operation",
            "parameters",
            "source_critic_report_sha256",
            "source_part_error_sha256",
            "source_visual_surface_sha256",
            "canonical_sha256",
        ],
    )?;
    if object.get("schema_version").and_then(Value::as_str) != Some("OptimizationResidual@1") {
        return Err(RuntimeError::InvalidInput(
            "OPTIMIZATION_RESIDUAL_INVALID: schema_version".to_owned(),
        ));
    }
    if object.get("part_id").and_then(Value::as_str) != Some(part_id) {
        return Err(RuntimeError::InvalidInput(
            "OPTIMIZATION_RESIDUAL_PART_BINDING_MISMATCH".to_owned(),
        ));
    }
    let node_id = object
        .get("node_id")
        .and_then(Value::as_str)
        .filter(|value| is_opaque_id(value) && value.starts_with("residual-"))
        .ok_or_else(|| {
            RuntimeError::InvalidInput(
                "OPTIMIZATION_RESIDUAL_NODE_ID_INVALID: residual node id must be opaque and prefixed"
                    .to_owned(),
            )
        })?;
    if node_id.len() > 96 {
        return Err(RuntimeError::InvalidInput(
            "OPTIMIZATION_RESIDUAL_NODE_ID_INVALID".to_owned(),
        ));
    }
    let operation = object
        .get("operation")
        .and_then(Value::as_str)
        .filter(|value| matches!(*value, "union" | "difference" | "intersection"))
        .ok_or_else(|| {
            RuntimeError::InvalidInput("OPTIMIZATION_RESIDUAL_OPERATION_INVALID".to_owned())
        })?;
    let parameters = object.get("parameters").ok_or_else(|| {
        RuntimeError::InvalidInput("OPTIMIZATION_RESIDUAL_PARAMETERS_REQUIRED".to_owned())
    })?;
    validate_residual_primitive_parameters(parameters)?;
    let critic_sha256 = required_sha(object, "source_critic_report_sha256")?;
    let part_error_sha256 = required_sha(object, "source_part_error_sha256")?;
    let visual_surface_sha256 = object
        .get("source_visual_surface_sha256")
        .map(|_| required_sha(object, "source_visual_surface_sha256"))
        .transpose()?;
    let canonical_sha256 = required_sha(object, "canonical_sha256")?;
    let mut canonical = residual.clone();
    canonical["canonical_sha256"] = Value::String(String::new());
    if canonical_json_hash(&canonical) != canonical_sha256 {
        return Err(RuntimeError::InvalidInput(
            "OPTIMIZATION_RESIDUAL_CANONICAL_HASH_MISMATCH".to_owned(),
        ));
    }

    // Critic and PartError are projections rather than caller-authored claims.
    // Recompute both from the same candidate/target binding before accepting a
    // Boolean residual.  This prevents stale or cross-candidate repair hints
    // from entering the optimization DAG.
    let critic = runtime
        .agentic_critic_projection(project_id, Some(candidate_id), Some(target_sha256))
        .map_err(|error| {
            RuntimeError::InvalidInput(format!("OPTIMIZATION_RESIDUAL_CRITIC_UNAVAILABLE: {error}"))
        })?;
    if critic.get("canonical_sha256").and_then(Value::as_str) != Some(critic_sha256) {
        return Err(RuntimeError::InvalidInput(
            "OPTIMIZATION_RESIDUAL_CRITIC_BINDING_MISMATCH".to_owned(),
        ));
    }
    let visual_surface = critic
        .get("visual_surface")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            RuntimeError::InvalidInput(
                "OPTIMIZATION_RESIDUAL_VISUAL_SURFACE_UNAVAILABLE".to_owned(),
            )
        })?;
    if visual_surface.get("status").and_then(Value::as_str) != Some("ready")
        || visual_surface
            .get("readback_status")
            .and_then(Value::as_str)
            != Some("ready")
        || !visual_surface
            .get("readback_canonical_sha256")
            .and_then(Value::as_str)
            .is_some_and(is_sha256)
    {
        return Err(RuntimeError::InvalidInput(
            "OPTIMIZATION_RESIDUAL_VISUAL_SURFACE_UNAVAILABLE".to_owned(),
        ));
    }
    if visual_surface
        .get("surface_signal_status")
        .and_then(Value::as_str)
        != Some("ready")
    {
        return Err(RuntimeError::InvalidInput(
            "OPTIMIZATION_RESIDUAL_VISUAL_SURFACE_SIGNAL_UNAVAILABLE".to_owned(),
        ));
    }
    if let Some(expected_surface_sha256) = visual_surface_sha256 {
        if visual_surface
            .get("surface_signal_canonical_sha256")
            .and_then(Value::as_str)
            != Some(expected_surface_sha256)
        {
            return Err(RuntimeError::InvalidInput(
                "OPTIMIZATION_RESIDUAL_VISUAL_SURFACE_SIGNAL_BINDING_MISMATCH".to_owned(),
            ));
        }
    }
    let surface_binding = visual_surface
        .get("binding")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            RuntimeError::InvalidInput(
                "OPTIMIZATION_RESIDUAL_VISUAL_SURFACE_BINDING_MISSING".to_owned(),
            )
        })?;
    let critic_lineage = critic
        .get("lineage")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            RuntimeError::InvalidInput("OPTIMIZATION_RESIDUAL_CRITIC_LINEAGE_MISSING".to_owned())
        })?;
    for key in [
        "reference_id",
        "reference_sha256",
        "artifact_sha256",
        "render_set_hash",
        "camera_hash",
        "comparison_report_hash",
        "quality_report_hash",
    ] {
        if surface_binding.get(key) != critic_lineage.get(key) {
            return Err(RuntimeError::InvalidInput(
                "OPTIMIZATION_RESIDUAL_VISUAL_SURFACE_BINDING_MISMATCH".to_owned(),
            ));
        }
    }
    let part_error = runtime
        .silhouette_part_error(
            project_id,
            json!({
                "project_id":project_id,
                "candidate_id":candidate_id,
                "target_sha256":target_sha256
            }),
        )
        .map_err(|error| {
            RuntimeError::InvalidInput(format!(
                "OPTIMIZATION_RESIDUAL_PART_ERROR_UNAVAILABLE: {error}"
            ))
        })?;
    if part_error.get("canonical_sha256").and_then(Value::as_str) != Some(part_error_sha256)
        || part_error.get("candidate_id").and_then(Value::as_str) != Some(candidate_id)
        || part_error.get("target_sha256").and_then(Value::as_str) != Some(target_sha256)
    {
        return Err(RuntimeError::InvalidInput(
            "OPTIMIZATION_RESIDUAL_PART_ERROR_BINDING_MISMATCH".to_owned(),
        ));
    }
    let part_is_observed = part_error
        .get("parts")
        .and_then(Value::as_array)
        .is_some_and(|parts| {
            parts
                .iter()
                .any(|part| part.get("part_id").and_then(Value::as_str) == Some(part_id))
        });
    if !part_is_observed {
        return Err(RuntimeError::InvalidInput(
            "OPTIMIZATION_RESIDUAL_PART_ERROR_PART_NOT_FOUND".to_owned(),
        ));
    }
    let _ = operation;
    Ok(())
}

fn validate_residual_primitive_parameters(parameters: &Value) -> Result<(), RuntimeError> {
    let object = parameters.as_object().ok_or_else(|| {
        RuntimeError::InvalidInput("OPTIMIZATION_RESIDUAL_PARAMETERS_INVALID".to_owned())
    })?;
    let shape = object.get("shape").and_then(Value::as_str).ok_or_else(|| {
        RuntimeError::InvalidInput("OPTIMIZATION_RESIDUAL_PARAMETERS_SHAPE_REQUIRED".to_owned())
    })?;
    match shape {
        "box" => {
            require_exact_parameter_keys(
                object,
                &["shape", "size_m", "position_m", "rotation_rad"],
            )?;
            validate_residual_vector(object, "size_m", 0.0, 10.0, true)?;
            validate_residual_vector(object, "position_m", -10.0, 10.0, false)?;
            validate_residual_vector(
                object,
                "rotation_rad",
                -std::f64::consts::TAU,
                std::f64::consts::TAU,
                false,
            )?;
        }
        "cylinder" => {
            require_exact_parameter_keys(
                object,
                &[
                    "shape",
                    "radius_m",
                    "height_m",
                    "radial_segments",
                    "position_m",
                    "rotation_rad",
                ],
            )?;
            validate_residual_scalar(object, "radius_m", 0.0, 5.0, true)?;
            validate_residual_scalar(object, "height_m", 0.0, 10.0, true)?;
            validate_residual_integer(object, "radial_segments", 8, 64)?;
            validate_residual_vector(object, "position_m", -10.0, 10.0, false)?;
            validate_residual_vector(
                object,
                "rotation_rad",
                -std::f64::consts::TAU,
                std::f64::consts::TAU,
                false,
            )?;
        }
        "ellipsoid" => {
            require_exact_parameter_keys(
                object,
                &[
                    "shape",
                    "radii_m",
                    "longitude_segments",
                    "latitude_segments",
                    "position_m",
                    "rotation_rad",
                ],
            )?;
            validate_residual_vector(object, "radii_m", 0.0, 5.0, true)?;
            validate_residual_integer(object, "longitude_segments", 8, 64)?;
            validate_residual_integer(object, "latitude_segments", 4, 64)?;
            validate_residual_vector(object, "position_m", -10.0, 10.0, false)?;
            validate_residual_vector(
                object,
                "rotation_rad",
                -std::f64::consts::TAU,
                std::f64::consts::TAU,
                false,
            )?;
        }
        "sphere" => {
            require_exact_parameter_keys(
                object,
                &[
                    "shape",
                    "radius_m",
                    "longitude_segments",
                    "latitude_segments",
                    "position_m",
                    "rotation_rad",
                ],
            )?;
            validate_residual_scalar(object, "radius_m", 0.0, 5.0, true)?;
            validate_residual_integer(object, "longitude_segments", 8, 64)?;
            validate_residual_integer(object, "latitude_segments", 4, 64)?;
            validate_residual_vector(object, "position_m", -10.0, 10.0, false)?;
            validate_residual_vector(
                object,
                "rotation_rad",
                -std::f64::consts::TAU,
                std::f64::consts::TAU,
                false,
            )?;
        }
        _ => {
            return Err(RuntimeError::InvalidInput(
                "OPTIMIZATION_RESIDUAL_PARAMETERS_SHAPE_INVALID".to_owned(),
            ))
        }
    }
    Ok(())
}

fn require_exact_parameter_keys(
    object: &Map<String, Value>,
    keys: &[&str],
) -> Result<(), RuntimeError> {
    if object.len() != keys.len() || object.keys().any(|key| !keys.contains(&key.as_str())) {
        return Err(RuntimeError::InvalidInput(
            "OPTIMIZATION_RESIDUAL_PARAMETERS_UNKNOWN_FIELD".to_owned(),
        ));
    }
    Ok(())
}

fn validate_residual_scalar(
    object: &Map<String, Value>,
    key: &str,
    minimum: f64,
    maximum: f64,
    exclusive_minimum: bool,
) -> Result<(), RuntimeError> {
    let value = object.get(key).and_then(Value::as_f64).ok_or_else(|| {
        RuntimeError::InvalidInput(format!("OPTIMIZATION_RESIDUAL_PARAMETERS_{key}_INVALID"))
    })?;
    let lower_ok = if exclusive_minimum {
        value > minimum
    } else {
        value >= minimum
    };
    if !value.is_finite() || !lower_ok || value > maximum {
        return Err(RuntimeError::InvalidInput(format!(
            "OPTIMIZATION_RESIDUAL_PARAMETERS_{key}_OUT_OF_BOUNDS"
        )));
    }
    Ok(())
}

fn validate_residual_integer(
    object: &Map<String, Value>,
    key: &str,
    minimum: u64,
    maximum: u64,
) -> Result<(), RuntimeError> {
    let value = object.get(key).and_then(Value::as_u64).ok_or_else(|| {
        RuntimeError::InvalidInput(format!("OPTIMIZATION_RESIDUAL_PARAMETERS_{key}_INVALID"))
    })?;
    if !(minimum..=maximum).contains(&value) {
        return Err(RuntimeError::InvalidInput(format!(
            "OPTIMIZATION_RESIDUAL_PARAMETERS_{key}_OUT_OF_BOUNDS"
        )));
    }
    Ok(())
}

fn validate_residual_vector(
    object: &Map<String, Value>,
    key: &str,
    minimum: f64,
    maximum: f64,
    exclusive_minimum: bool,
) -> Result<(), RuntimeError> {
    let values = object.get(key).and_then(Value::as_array).ok_or_else(|| {
        RuntimeError::InvalidInput(format!("OPTIMIZATION_RESIDUAL_PARAMETERS_{key}_INVALID"))
    })?;
    if values.len() != 3
        || values.iter().any(|value| {
            let Some(value) = value.as_f64() else {
                return true;
            };
            let lower_ok = if exclusive_minimum {
                value > minimum
            } else {
                value >= minimum
            };
            !value.is_finite() || !lower_ok || value > maximum
        })
    {
        return Err(RuntimeError::InvalidInput(format!(
            "OPTIMIZATION_RESIDUAL_PARAMETERS_{key}_OUT_OF_BOUNDS"
        )));
    }
    Ok(())
}

fn validate_result(result: &Value, job_id: &str, intent_sha256: &str) -> Result<(), RuntimeError> {
    let object = result
        .as_object()
        .ok_or_else(|| RuntimeError::InvalidInput("OPTIMIZATION_RESULT_INVALID".to_owned()))?;
    if object.get("schema_version").and_then(Value::as_str) != Some("OptimizationResult@1")
        || object.get("job_id").and_then(Value::as_str) != Some(job_id)
        || object.get("intent_sha256").and_then(Value::as_str) != Some(intent_sha256)
    {
        return Err(RuntimeError::InvalidInput(
            "OPTIMIZATION_RESULT_BINDING_MISMATCH".to_owned(),
        ));
    }
    if object
        .get("non_regressing")
        .and_then(Value::as_bool)
        .is_none()
    {
        return Err(RuntimeError::InvalidInput(
            "OPTIMIZATION_RESULT_NON_REGRESSING_INVALID".to_owned(),
        ));
    }
    if object.get("search_strategy").and_then(Value::as_str) != Some(OPTIMIZATION_SEARCH_STRATEGY) {
        return Err(RuntimeError::InvalidInput(
            "OPTIMIZATION_RESULT_SEARCH_STRATEGY_MISMATCH".to_owned(),
        ));
    }
    let promotion_policy = object
        .get("promotion_policy")
        .and_then(Value::as_str)
        .unwrap_or("legacy-multi-objective-v1");
    if !matches!(
        promotion_policy,
        "legacy-multi-objective-v1" | "silhouette-evaluation-objective-v1"
    ) {
        return Err(RuntimeError::InvalidInput(
            "OPTIMIZATION_RESULT_PROMOTION_POLICY_INVALID".to_owned(),
        ));
    }
    let evaluation_objective_sha256 = object
        .get("evaluation_objective_sha256")
        .and_then(Value::as_str);
    if promotion_policy == "silhouette-evaluation-objective-v1"
        && !evaluation_objective_sha256.is_some_and(is_sha256)
    {
        return Err(RuntimeError::InvalidInput(
            "OPTIMIZATION_RESULT_EVALUATION_OBJECTIVE_BINDING_MISSING".to_owned(),
        ));
    }
    if let Some(promotion_status) = object.get("promotion_status") {
        if !matches!(
            promotion_status.as_str(),
            Some("not-ready" | "ready" | "blocked" | "blocked_global_or_part_objective")
        ) {
            return Err(RuntimeError::InvalidInput(
                "OPTIMIZATION_RESULT_PROMOTION_STATUS_INVALID".to_owned(),
            ));
        }
    }
    let canonical = object
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .ok_or_else(|| {
            RuntimeError::InvalidInput("OPTIMIZATION_RESULT_CANONICAL_INVALID".to_owned())
        })?;
    let mut input = result.clone();
    input["canonical_sha256"] = Value::String(String::new());
    if canonical_json_hash(&input) != canonical {
        return Err(RuntimeError::InvalidInput(
            "OPTIMIZATION_RESULT_CANONICAL_HASH_MISMATCH".to_owned(),
        ));
    }
    if !matches!(
        object.get("next_stage").and_then(Value::as_str),
        Some("coarse" | "mid" | "final" | "done")
    ) {
        return Err(RuntimeError::InvalidInput(
            "OPTIMIZATION_RESULT_NEXT_STAGE_INVALID".to_owned(),
        ));
    }
    if let Some(fidelity) = object.get("best_evaluation_fidelity") {
        if !fidelity.is_null() && !matches!(fidelity.as_str(), Some("coarse" | "mid" | "final")) {
            return Err(RuntimeError::InvalidInput(
                "OPTIMIZATION_RESULT_BEST_FIDELITY_INVALID".to_owned(),
            ));
        }
    }
    for key in [
        "candidate_program_object_sha256s",
        "candidate_artifact_object_sha256s",
    ] {
        let values = object.get(key).and_then(Value::as_array).ok_or_else(|| {
            RuntimeError::InvalidInput(format!("OPTIMIZATION_RESULT_{key}_INVALID"))
        })?;
        if values.is_empty()
            || values.len() > 48
            || values
                .iter()
                .any(|value| value.as_str().is_none_or(|hash| !is_sha256(hash)))
        {
            return Err(RuntimeError::InvalidInput(format!(
                "OPTIMIZATION_RESULT_{key}_INVALID"
            )));
        }
    }
    Ok(())
}

fn load_optimization_checkpoint(
    runtime: &Runtime,
    checkpoint_sha256: &str,
    job_id: &str,
    intent_sha256: &str,
    coarse_count: usize,
    mid_top_k: usize,
    final_top_k: usize,
) -> Result<OptimizationCheckpoint, RuntimeError> {
    let result = read_json_object(runtime, checkpoint_sha256, "optimization-result")?;
    validate_result(&result, job_id, intent_sha256)?;
    let object = result
        .as_object()
        .ok_or_else(|| RuntimeError::InvalidInput("OPTIMIZATION_RESULT_INVALID".to_owned()))?;
    if object.get("status").and_then(Value::as_str) != Some("running") {
        return Err(RuntimeError::InvalidInput(
            "OPTIMIZATION_CHECKPOINT_NOT_RUNNING".to_owned(),
        ));
    }
    let next_stage = object
        .get("next_stage")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            RuntimeError::InvalidInput("OPTIMIZATION_RESULT_NEXT_STAGE_INVALID".to_owned())
        })?
        .to_owned();
    let program_objects = object
        .get("candidate_program_object_sha256s")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            RuntimeError::InvalidInput("OPTIMIZATION_RESULT_CANDIDATE_OBJECTS_INVALID".to_owned())
        })?
        .iter()
        .map(|value| {
            value.as_str().map(str::to_owned).ok_or_else(|| {
                RuntimeError::InvalidInput(
                    "OPTIMIZATION_RESULT_CANDIDATE_OBJECTS_INVALID".to_owned(),
                )
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let artifact_objects = object
        .get("candidate_artifact_object_sha256s")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            RuntimeError::InvalidInput("OPTIMIZATION_RESULT_CANDIDATE_OBJECTS_INVALID".to_owned())
        })?
        .iter()
        .map(|value| {
            value.as_str().map(str::to_owned).ok_or_else(|| {
                RuntimeError::InvalidInput(
                    "OPTIMIZATION_RESULT_CANDIDATE_OBJECTS_INVALID".to_owned(),
                )
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    if program_objects.len() != coarse_count
        || artifact_objects.len() != coarse_count
        || program_objects.iter().any(|hash| !is_sha256(hash))
        || artifact_objects.iter().any(|hash| !is_sha256(hash))
    {
        return Err(RuntimeError::InvalidInput(
            "OPTIMIZATION_CHECKPOINT_CANDIDATE_OBJECT_COUNT_MISMATCH".to_owned(),
        ));
    }
    let evaluation_hashes = object
        .get("evaluation_object_sha256s")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            RuntimeError::InvalidInput("OPTIMIZATION_RESULT_EVALUATIONS_INVALID".to_owned())
        })?;
    let evaluations = evaluation_hashes
        .iter()
        .enumerate()
        .map(|(index, value)| {
            let hash = value.as_str().ok_or_else(|| {
                RuntimeError::InvalidInput("OPTIMIZATION_RESULT_EVALUATIONS_INVALID".to_owned())
            })?;
            load_evaluation_record(runtime, hash, job_id, coarse_count, index + 1)
        })
        .collect::<Result<Vec<_>, _>>()?;
    if object.get("evaluations_count").and_then(Value::as_u64) != Some(evaluations.len() as u64)
        || object.get("checkpoint_sequence").and_then(Value::as_u64)
            != Some(evaluations.len() as u64)
    {
        return Err(RuntimeError::InvalidInput(
            "OPTIMIZATION_CHECKPOINT_SEQUENCE_MISMATCH".to_owned(),
        ));
    }
    let counts = object
        .get("fidelity_counts")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            RuntimeError::InvalidInput("OPTIMIZATION_RESULT_FIDELITY_COUNTS_INVALID".to_owned())
        })?;
    let actual_counts = ["coarse", "mid", "final"]
        .into_iter()
        .map(|fidelity| {
            (
                fidelity,
                evaluations
                    .iter()
                    .filter(|evaluation| evaluation.fidelity == fidelity)
                    .count(),
            )
        })
        .collect::<Vec<_>>();
    if actual_counts.iter().any(|(fidelity, count)| {
        counts.get(*fidelity).and_then(Value::as_u64) != Some(*count as u64)
    }) {
        return Err(RuntimeError::InvalidInput(
            "OPTIMIZATION_CHECKPOINT_FIDELITY_COUNTS_MISMATCH".to_owned(),
        ));
    }
    let coarse_done = actual_counts[0].1;
    let mid_done = actual_counts[1].1;
    let final_done = actual_counts[2].1;
    let stage_valid = match next_stage.as_str() {
        "coarse" => coarse_done <= coarse_count && mid_done == 0 && final_done == 0,
        "mid" => coarse_done == coarse_count && mid_done <= mid_top_k && final_done == 0,
        "final" => {
            coarse_done == coarse_count && mid_done == mid_top_k && final_done <= final_top_k + 1
        }
        "done" => {
            coarse_done == coarse_count && mid_done == mid_top_k && final_done == final_top_k + 1
        }
        _ => false,
    };
    if !stage_valid {
        return Err(RuntimeError::InvalidInput(
            "OPTIMIZATION_CHECKPOINT_STAGE_PROGRESS_INVALID".to_owned(),
        ));
    }
    Ok(OptimizationCheckpoint {
        next_stage,
        evaluations,
        candidate_program_object_sha256s: program_objects,
        candidate_artifact_object_sha256s: artifact_objects,
    })
}

fn load_evaluation_record(
    runtime: &Runtime,
    object_sha256: &str,
    job_id: &str,
    coarse_count: usize,
    expected_sequence: usize,
) -> Result<EvaluationRecord, RuntimeError> {
    if !is_sha256(object_sha256) {
        return Err(RuntimeError::InvalidInput(
            "OPTIMIZATION_EVALUATION_OBJECT_HASH_INVALID".to_owned(),
        ));
    }
    let value = read_json_object(runtime, object_sha256, "optimization-evaluation")?;
    let object = value
        .as_object()
        .ok_or_else(|| RuntimeError::InvalidInput("OPTIMIZATION_EVALUATION_INVALID".to_owned()))?;
    if object.get("schema_version").and_then(Value::as_str) != Some("OptimizationEvaluation@1")
        || object.get("job_id").and_then(Value::as_str) != Some(job_id)
        || object.get("sequence").and_then(Value::as_u64) != Some(expected_sequence as u64)
    {
        return Err(RuntimeError::InvalidInput(
            "OPTIMIZATION_EVALUATION_BINDING_MISMATCH".to_owned(),
        ));
    }
    let fidelity = object
        .get("fidelity")
        .and_then(Value::as_str)
        .filter(|value| matches!(*value, "coarse" | "mid" | "final"))
        .ok_or_else(|| {
            RuntimeError::InvalidInput("OPTIMIZATION_EVALUATION_FIDELITY_INVALID".to_owned())
        })?
        .to_owned();
    let expected_resolution = match fidelity.as_str() {
        "coarse" => 128,
        "mid" => 256,
        "final" => 512,
        _ => unreachable!(),
    };
    if object.get("resolution").and_then(Value::as_u64) != Some(expected_resolution) {
        return Err(RuntimeError::InvalidInput(
            "OPTIMIZATION_EVALUATION_RESOLUTION_INVALID".to_owned(),
        ));
    }
    let evaluation_id = object
        .get("evaluation_id")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            RuntimeError::InvalidInput("OPTIMIZATION_EVALUATION_ID_INVALID".to_owned())
        })?;
    let prefix = format!("optimization-evaluation-{fidelity}-");
    let candidate_index = evaluation_id
        .strip_prefix(&prefix)
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|index| *index < coarse_count)
        .ok_or_else(|| {
            RuntimeError::InvalidInput("OPTIMIZATION_EVALUATION_ID_INVALID".to_owned())
        })?;
    let _program_sha256 = object
        .get("program_sha256")
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .ok_or_else(|| {
            RuntimeError::InvalidInput("OPTIMIZATION_EVALUATION_PROGRAM_INVALID".to_owned())
        })?
        .to_owned();
    let _render_sha256 = object
        .get("render_sha256")
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .ok_or_else(|| {
            RuntimeError::InvalidInput("OPTIMIZATION_EVALUATION_RENDER_INVALID".to_owned())
        })?;
    let metrics = object.get("metrics").ok_or_else(|| {
        RuntimeError::InvalidInput("OPTIMIZATION_EVALUATION_METRICS_INVALID".to_owned())
    })?;
    let metrics_sha256 = object
        .get("metrics_sha256")
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .ok_or_else(|| {
            RuntimeError::InvalidInput("OPTIMIZATION_EVALUATION_METRICS_INVALID".to_owned())
        })?;
    if canonical_json_hash(metrics) != metrics_sha256 {
        return Err(RuntimeError::InvalidInput(
            "OPTIMIZATION_EVALUATION_METRICS_HASH_MISMATCH".to_owned(),
        ));
    }
    let loss = object
        .get("loss")
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite() && *value >= 0.0)
        .ok_or_else(|| {
            RuntimeError::InvalidInput("OPTIMIZATION_EVALUATION_LOSS_INVALID".to_owned())
        })?;
    let final_object_hashes = if fidelity == "final" {
        object
            .get("pass_hashes")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                RuntimeError::InvalidInput("OPTIMIZATION_EVALUATION_PASSES_INVALID".to_owned())
            })?
            .iter()
            .map(|pass| {
                pass.get("sha256")
                    .and_then(Value::as_str)
                    .filter(|value| is_sha256(value))
                    .map(str::to_owned)
                    .ok_or_else(|| {
                        RuntimeError::InvalidInput(
                            "OPTIMIZATION_EVALUATION_PASSES_INVALID".to_owned(),
                        )
                    })
            })
            .collect::<Result<Vec<_>, _>>()?
    } else {
        Vec::new()
    };
    let canonical = object
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .ok_or_else(|| {
            RuntimeError::InvalidInput("OPTIMIZATION_EVALUATION_CANONICAL_INVALID".to_owned())
        })?;
    let mut canonical_input = value.clone();
    canonical_input["canonical_sha256"] = Value::String(String::new());
    if canonical_json_hash(&canonical_input) != canonical {
        return Err(RuntimeError::InvalidInput(
            "OPTIMIZATION_EVALUATION_CANONICAL_HASH_MISMATCH".to_owned(),
        ));
    }
    Ok(EvaluationRecord {
        value: value.clone(),
        object_sha256: object_sha256.to_owned(),
        candidate_index,
        loss,
        fidelity,
        final_object_hashes,
    })
}

fn validate_evaluation_objective_checkpoint(
    evaluations: &[EvaluationRecord],
    expected_objective_sha256: Option<&str>,
) -> Result<(), RuntimeError> {
    for evaluation in evaluations {
        let actual = evaluation
            .value
            .get("evaluation_objective_sha256")
            .and_then(Value::as_str);
        if actual != expected_objective_sha256 {
            return Err(RuntimeError::InvalidInput(
                "OPTIMIZATION_CHECKPOINT_EVALUATION_OBJECTIVE_MISMATCH".to_owned(),
            ));
        }
        if expected_objective_sha256.is_some() && evaluation.value.get("part_metrics").is_none() {
            return Err(RuntimeError::InvalidInput(
                "OPTIMIZATION_CHECKPOINT_PART_METRICS_MISSING".to_owned(),
            ));
        }
    }
    Ok(())
}

fn load_checkpoint_candidates(
    runtime: &Runtime,
    checkpoint: &OptimizationCheckpoint,
    context: &OptimizationContext,
) -> Result<Vec<CompiledCandidate>, RuntimeError> {
    checkpoint
        .candidate_program_object_sha256s
        .iter()
        .zip(checkpoint.candidate_artifact_object_sha256s.iter())
        .map(|(program_object_sha256, artifact_object_sha256)| {
            let program_object = runtime
                .store
                .get_object(program_object_sha256)?
                .ok_or_else(|| {
                    RuntimeError::InvalidInput(
                        "OPTIMIZATION_CHECKPOINT_PROGRAM_UNAVAILABLE".to_owned(),
                    )
                })?;
            if program_object.mime != "application/json"
                || (!program_object.kind.starts_with("optimization-program")
                    && !program_object.kind.starts_with("geometry-program-v2"))
            {
                return Err(RuntimeError::InvalidInput(
                    "OPTIMIZATION_CHECKPOINT_PROGRAM_METADATA_MISMATCH".to_owned(),
                ));
            }
            let program_bytes = runtime.cas_read(program_object_sha256)?;
            if sha256_hex(&program_bytes) != *program_object_sha256 {
                return Err(RuntimeError::InvalidInput(
                    "OPTIMIZATION_CHECKPOINT_PROGRAM_HASH_MISMATCH".to_owned(),
                ));
            }
            let program: Value = serde_json::from_slice(&program_bytes).map_err(|_| {
                RuntimeError::InvalidInput("OPTIMIZATION_CHECKPOINT_PROGRAM_INVALID".to_owned())
            })?;
            let hash_result =
                super::hash_geometry_program_with_runtime_worker(&program).map_err(|_| {
                    RuntimeError::InvalidInput(
                        "OPTIMIZATION_CHECKPOINT_PROGRAM_HASH_FAILED".to_owned(),
                    )
                })?;
            let program_sha256 = hash_result
                .get("canonical_sha256")
                .and_then(Value::as_str)
                .filter(|hash| is_sha256(hash))
                .ok_or_else(|| {
                    RuntimeError::InvalidInput(
                        "OPTIMIZATION_CHECKPOINT_PROGRAM_HASH_MISSING".to_owned(),
                    )
                })?
                .to_owned();
            if program.get("canonical_sha256").and_then(Value::as_str)
                != Some(program_sha256.as_str())
            {
                return Err(RuntimeError::InvalidInput(
                    "OPTIMIZATION_CHECKPOINT_PROGRAM_LINEAGE_MISMATCH".to_owned(),
                ));
            }
            let artifact_object = runtime
                .store
                .get_object(artifact_object_sha256)?
                .ok_or_else(|| {
                    RuntimeError::InvalidInput(
                        "OPTIMIZATION_CHECKPOINT_ARTIFACT_UNAVAILABLE".to_owned(),
                    )
                })?;
            if artifact_object.mime != "model/gltf-binary"
                || (!artifact_object.kind.starts_with("optimization-artifact")
                    && !artifact_object.kind.starts_with("geometry-glb"))
            {
                return Err(RuntimeError::InvalidInput(
                    "OPTIMIZATION_CHECKPOINT_ARTIFACT_METADATA_MISMATCH".to_owned(),
                ));
            }
            let glb = runtime.cas_read(artifact_object_sha256)?;
            if sha256_hex(&glb) != *artifact_object_sha256 {
                return Err(RuntimeError::InvalidInput(
                    "OPTIMIZATION_CHECKPOINT_ARTIFACT_HASH_MISMATCH".to_owned(),
                ));
            }
            let inspection = strict_glb_inspection(&glb)?;
            if !inspection.hard_gate_passed || inspection.program_sha256 != program_sha256 {
                return Err(RuntimeError::InvalidInput(
                    "OPTIMIZATION_CHECKPOINT_ARTIFACT_READBACK_FAILED".to_owned(),
                ));
            }
            let max_triangles = context.intent["budget"]["max_output_triangles"]
                .as_u64()
                .unwrap_or(2_000_000);
            if inspection.triangle_count == 0 || inspection.triangle_count > max_triangles {
                return Err(RuntimeError::InvalidInput(
                    "OPTIMIZATION_CHECKPOINT_TRIANGLE_BUDGET_EXCEEDED".to_owned(),
                ));
            }
            Ok(CompiledCandidate {
                glb,
                program_sha256,
                program_object_sha256: program_object_sha256.clone(),
                artifact_object_sha256: artifact_object_sha256.clone(),
                triangle_count: inspection.triangle_count,
            })
        })
        .collect()
}

fn candidate_reachable_hashes(candidates: &[CompiledCandidate]) -> Vec<String> {
    candidates
        .iter()
        .flat_map(|candidate| {
            [
                candidate.program_object_sha256.clone(),
                candidate.artifact_object_sha256.clone(),
            ]
        })
        .collect()
}

fn stage_progress(stage: &str, completed: usize, total: usize, _mid_top_k: usize) -> u8 {
    let ratio = completed as f64 / total.max(1) as f64;
    match stage {
        "coarse" => (1.0 + 54.0 * ratio).round().clamp(1.0, 55.0) as u8,
        "mid" => (55.0 + 20.0 * ratio).round().clamp(55.0, 75.0) as u8,
        "final" => (75.0 + 25.0 * ratio).round().clamp(75.0, 100.0) as u8,
        _ => 0,
    }
}

fn baseline_parameters(rig: &Value) -> Vec<Value> {
    rig.get("parameters")
        .and_then(Value::as_array)
        .map(|parameters| {
            parameters
                .iter()
                .map(|parameter| {
                    json!({
                        "parameter_id":parameter["parameter_id"],
                        "part_id":parameter["part_id"],
                        "value":parameter["value"]
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn deterministic_parameter_sets(rig: &Value, baseline: &[Value], count: usize) -> Vec<Vec<Value>> {
    let definitions = rig
        .get("parameters")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut sets = Vec::with_capacity(count);
    if count == 0 || definitions.is_empty() {
        return sets;
    }
    let surface_groups = surface_control_point_groups(&definitions);
    if !surface_groups.is_empty() {
        sets.push(baseline.to_vec());
        let group_count = surface_groups.len();
        for sequence in 1..count {
            let mut parameters = baseline.to_vec();
            let group_index = (sequence - 1) % group_count;
            let cycle = (sequence - 1) / group_count;
            let level = cycle / 2 + 1;
            let direction = if cycle % 2 == 0 { 1.0 } else { -1.0 };
            for coordinate in &surface_groups[group_index] {
                let Some(definition) = definitions.get(*coordinate) else {
                    continue;
                };
                let Some(selected) = parameters.get_mut(*coordinate) else {
                    continue;
                };
                let value = definition["value"].as_f64().unwrap_or(0.0);
                let minimum = definition["min"].as_f64().unwrap_or(value);
                let maximum = definition["max"].as_f64().unwrap_or(value);
                let step = definition["step"].as_f64().unwrap_or(0.01).abs();
                let delta = surface_control_point_delta(
                    definition,
                    value,
                    step * level as f64,
                    direction,
                    1,
                );
                selected["value"] = Value::from(stable_visual_metric(
                    (value + delta).clamp(minimum, maximum),
                ));
            }
            sets.push(parameters);
        }
        return sets;
    }
    sets.push(baseline.to_vec());
    for sequence in 1..count {
        let mut parameters = baseline.to_vec();
        let coordinate = (sequence - 1) % definitions.len();
        let level = ((sequence - 1) / definitions.len()) / 2 + 1;
        let direction = if ((sequence - 1) / definitions.len()) % 2 == 0 {
            1.0
        } else {
            -1.0
        };
        if let (Some(definition), Some(selected)) =
            (definitions.get(coordinate), parameters.get_mut(coordinate))
        {
            let value = definition["value"].as_f64().unwrap_or(0.0);
            let min = definition["min"].as_f64().unwrap_or(value);
            let max = definition["max"].as_f64().unwrap_or(value);
            let step = definition["step"].as_f64().unwrap_or(0.01).abs();
            let next = (value + direction * step * level as f64).clamp(min, max);
            selected["value"] = Value::from(stable_visual_metric(next));
        }
        sets.push(parameters);
    }
    sets
}

/// Reserve a small deterministic exploration prefix before using the rest of
/// the coarse budget for adaptive local refinement.  The prefix scales with
/// the number of authored parameters but remains bounded so a large coarse
/// budget is not spent blindly before the first useful measurement.
fn exploration_candidate_count(rig: &Value, coarse_count: usize) -> usize {
    let parameter_count = rig
        .get("parameters")
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or(0);
    coarse_count.min(parameter_count.saturating_mul(2).clamp(8, 16))
}

/// Generate the local trust-region candidates around the best low-fidelity
/// seed.  Each parameter receives deterministic positive/negative probes; on
/// later rounds the step shrinks, so the search becomes local rather than
/// repeatedly sweeping the full authored range.  The returned vectors keep
/// the exact Rig parameter order required by `materialize_rig_geometry_program`.
fn adaptive_parameter_sets(rig: &Value, center: &[Value], count: usize) -> Vec<Vec<Value>> {
    let definitions = rig
        .get("parameters")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if definitions.is_empty() || center.len() != definitions.len() {
        return Vec::new();
    }
    let surface_groups = surface_control_point_groups(&definitions);
    if !surface_groups.is_empty() {
        let mut sets = Vec::with_capacity(count);
        let group_count = surface_groups.len();
        for sequence in 0..count {
            let pair_index = sequence / 2;
            let group_index = pair_index % group_count;
            let round = pair_index / group_count;
            let direction = if sequence % 2 == 0 { 1.0 } else { -1.0 };
            let mut parameters = center.to_vec();
            for coordinate in &surface_groups[group_index] {
                let Some(definition) = definitions.get(*coordinate) else {
                    continue;
                };
                let value = center[*coordinate]
                    .get("value")
                    .and_then(Value::as_f64)
                    .or_else(|| definition.get("value").and_then(Value::as_f64))
                    .unwrap_or(0.0);
                let minimum = definition
                    .get("min")
                    .and_then(Value::as_f64)
                    .unwrap_or(value);
                let maximum = definition
                    .get("max")
                    .and_then(Value::as_f64)
                    .unwrap_or(value);
                let step = definition
                    .get("step")
                    .and_then(Value::as_f64)
                    .unwrap_or(0.01)
                    .abs();
                let span = (maximum - minimum).abs();
                let shrink = 0.5_f64.powi(round as i32);
                let magnitude = (step * shrink).max(span * 0.01).min(span * 0.5);
                let delta = surface_control_point_delta(
                    definition,
                    value,
                    magnitude,
                    direction,
                    1,
                );
                if let Some(selected) = parameters.get_mut(*coordinate) {
                    selected["value"] = Value::from(stable_visual_metric(
                        (value + delta).clamp(minimum, maximum),
                    ));
                }
            }
            sets.push(parameters);
        }
        return sets;
    }
    let mut sets = Vec::with_capacity(count);
    for sequence in 0..count {
        let pair_index = sequence / 2;
        let coordinate = pair_index % definitions.len();
        let round = pair_index / definitions.len();
        let direction = if sequence % 2 == 0 { 1.0 } else { -1.0 };
        let definition = &definitions[coordinate];
        let mut parameters = center.to_vec();
        let value = center[coordinate]
            .get("value")
            .and_then(Value::as_f64)
            .or_else(|| definition.get("value").and_then(Value::as_f64))
            .unwrap_or(0.0);
        let minimum = definition
            .get("min")
            .and_then(Value::as_f64)
            .unwrap_or(value);
        let maximum = definition
            .get("max")
            .and_then(Value::as_f64)
            .unwrap_or(value);
        let step = definition
            .get("step")
            .and_then(Value::as_f64)
            .unwrap_or(0.01)
            .abs();
        let span = (maximum - minimum).abs();
        let shrink = 0.5_f64.powi(round as i32);
        let delta = (step * shrink).max(span * 0.01).min(span * 0.5);
        if let Some(selected) = parameters.get_mut(coordinate) {
            selected["value"] = Value::from(stable_visual_metric(
                (value + direction * delta).clamp(minimum, maximum),
            ));
        }
        sets.push(parameters);
    }
    sets
}

/// Group surface parameters into deterministic multi-control-point moves.
/// The authored order is part of the Rig hash, so pairing consecutive surface
/// controls gives a stable bounded lane without inventing topology or reading
/// arbitrary expressions. Non-surface parameters remain singleton groups so a
/// mixed Rig does not silently lose its other typed controls.
fn surface_control_point_groups(definitions: &[Value]) -> Vec<Vec<usize>> {
    let surface_indices = definitions
        .iter()
        .enumerate()
        .filter_map(|(index, definition)| {
            (definition.get("semantic").and_then(Value::as_str)
                == Some("surface_control_point"))
                .then_some(index)
        })
        .collect::<Vec<_>>();
    if surface_indices.is_empty() {
        return Vec::new();
    }
    let mut groups = surface_indices
        .chunks(2)
        .map(|chunk| chunk.to_vec())
        .collect::<Vec<_>>();
    for (index, definition) in definitions.iter().enumerate() {
        if definition.get("semantic").and_then(Value::as_str) != Some("surface_control_point") {
            groups.push(vec![index]);
        }
    }
    groups
}

fn surface_control_point_delta(
    definition: &Value,
    value: f64,
    magnitude: f64,
    direction: f64,
    _level: usize,
) -> f64 {
    let axis = definition.get("axis").and_then(Value::as_str);
    let orientation = if axis == Some("x") && value.abs() > f64::EPSILON {
        value.signum()
    } else {
        1.0
    };
    direction * magnitude * orientation
}

fn apply_boolean_residual(
    draft: &mut Value,
    residual: &Value,
    part_id: &str,
) -> Result<(), RuntimeError> {
    let residual_object = residual
        .as_object()
        .ok_or_else(|| RuntimeError::InvalidInput("OPTIMIZATION_RESIDUAL_INVALID".to_owned()))?;
    validate_residual_primitive_parameters(residual_object.get("parameters").ok_or_else(
        || RuntimeError::InvalidInput("OPTIMIZATION_RESIDUAL_PARAMETERS_REQUIRED".to_owned()),
    )?)?;
    let operation = residual_object
        .get("operation")
        .and_then(Value::as_str)
        .filter(|value| matches!(*value, "union" | "difference" | "intersection"))
        .ok_or_else(|| {
            RuntimeError::InvalidInput("OPTIMIZATION_RESIDUAL_OPERATION_INVALID".to_owned())
        })?;
    let residual_node_id = residual_object
        .get("node_id")
        .and_then(Value::as_str)
        .filter(|value| is_opaque_id(value))
        .ok_or_else(|| {
            RuntimeError::InvalidInput("OPTIMIZATION_RESIDUAL_NODE_ID_INVALID".to_owned())
        })?
        .to_owned();
    let boolean_node_id = format!("{}-boolean", residual_node_id);
    let base_node_id = format!("{}-base", residual_node_id);

    let program_object = draft.as_object_mut().ok_or_else(|| {
        RuntimeError::InvalidInput("OPTIMIZATION_RESIDUAL_PROGRAM_INVALID".to_owned())
    })?;
    program_object.remove("canonical_sha256");
    let existing_nodes = program_object
        .get("nodes")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            RuntimeError::InvalidInput("OPTIMIZATION_RESIDUAL_NODES_MISSING".to_owned())
        })?;
    let mut node_ids = existing_nodes
        .iter()
        .filter_map(|node| {
            node.get("node_id")
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
        .collect::<HashSet<_>>();
    if node_ids.contains(&residual_node_id)
        || node_ids.contains(&boolean_node_id)
        || node_ids.contains(&base_node_id)
    {
        return Err(RuntimeError::InvalidInput(
            "OPTIMIZATION_RESIDUAL_NODE_ID_COLLISION".to_owned(),
        ));
    }
    let outputs = program_object
        .get("part_outputs")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            RuntimeError::InvalidInput("OPTIMIZATION_RESIDUAL_PART_OUTPUTS_MISSING".to_owned())
        })?;
    let matching_indices = outputs
        .iter()
        .enumerate()
        .filter(|(_, output)| output.get("part_id").and_then(Value::as_str) == Some(part_id))
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    if matching_indices.len() != 1 {
        return Err(RuntimeError::InvalidInput(
            "OPTIMIZATION_RESIDUAL_MUST_TARGET_ONE_EXACT_PART".to_owned(),
        ));
    }
    let output_index = matching_indices[0];
    let input_node_ids = outputs[output_index]
        .get("input_node_ids")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            RuntimeError::InvalidInput("OPTIMIZATION_RESIDUAL_PART_INPUTS_MISSING".to_owned())
        })?
        .iter()
        .map(|value| {
            value.as_str().map(str::to_owned).ok_or_else(|| {
                RuntimeError::InvalidInput("OPTIMIZATION_RESIDUAL_PART_INPUT_INVALID".to_owned())
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    if input_node_ids.is_empty() || input_node_ids.iter().any(|id| !node_ids.contains(id)) {
        return Err(RuntimeError::InvalidInput(
            "OPTIMIZATION_RESIDUAL_PART_INPUT_UNKNOWN".to_owned(),
        ));
    }
    let nodes = program_object
        .get_mut("nodes")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| {
            RuntimeError::InvalidInput("OPTIMIZATION_RESIDUAL_NODES_MISSING".to_owned())
        })?;
    let base_input = if input_node_ids.len() == 1 {
        input_node_ids[0].clone()
    } else {
        nodes.push(json!({
            "node_id":base_node_id,
            "operator_id":"forgecad.geometry.part-output@1",
            "inputs":input_node_ids,
            "parameters":{"shape":"part-output"}
        }));
        node_ids.insert(base_node_id.clone());
        base_node_id.clone()
    };
    nodes.push(json!({
        "node_id":residual_node_id,
        "operator_id":"forgecad.geometry.primitive@2",
        "inputs":[],
        "parameters":residual_object["parameters"]
    }));
    node_ids.insert(residual_node_id.clone());
    nodes.push(json!({
        "node_id":boolean_node_id,
        "operator_id":"forgecad.geometry.boolean@1",
        "inputs":[base_input, residual_node_id],
        "parameters":{"shape":operation}
    }));
    let outputs = program_object
        .get_mut("part_outputs")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| {
            RuntimeError::InvalidInput("OPTIMIZATION_RESIDUAL_PART_OUTPUTS_MISSING".to_owned())
        })?;
    outputs[output_index]["input_node_ids"] = json!([boolean_node_id]);
    Ok(())
}

/// Expand one approved residual into a small, deterministic local family.
/// These are still the same typed primitive and same Boolean operation; only
/// the bounded geometric magnitude/position is searched.  The original
/// residual is always family member zero so a caller can reproduce the exact
/// requested edit, while the remaining members give CADFit a chance to avoid
/// a one-size-fits-all sphere or box.
fn residual_variant_family(residual: &Value) -> Result<Vec<Value>, RuntimeError> {
    let parameters = residual
        .get("parameters")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            RuntimeError::InvalidInput("OPTIMIZATION_RESIDUAL_PARAMETERS_REQUIRED".to_owned())
        })?;
    let shape = parameters
        .get("shape")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            RuntimeError::InvalidInput("OPTIMIZATION_RESIDUAL_PARAMETERS_SHAPE_REQUIRED".to_owned())
        })?;
    let mut variants = vec![residual.clone()];

    let mut push_variant = |mut parameters: Map<String, Value>| -> Result<(), RuntimeError> {
        let mut variant = residual.clone();
        variant["parameters"] = Value::Object(std::mem::take(&mut parameters));
        variant["canonical_sha256"] = Value::String(String::new());
        variant["canonical_sha256"] = Value::String(canonical_json_hash(&variant));
        validate_residual_primitive_parameters(&variant["parameters"])?;
        variants.push(variant);
        Ok(())
    };

    // Preserve the authored primitive tessellation and rotate through only
    // eight local perturbations.  Position steps are intentionally small in
    // model space so a residual cannot become an unbounded second body.
    let position = parameters
        .get("position_m")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            RuntimeError::InvalidInput("OPTIMIZATION_RESIDUAL_POSITION_REQUIRED".to_owned())
        })?;
    let position = position
        .iter()
        .map(|value| {
            value.as_f64().ok_or_else(|| {
                RuntimeError::InvalidInput("OPTIMIZATION_RESIDUAL_POSITION_INVALID".to_owned())
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    if position.len() != 3 {
        return Err(RuntimeError::InvalidInput(
            "OPTIMIZATION_RESIDUAL_POSITION_INVALID".to_owned(),
        ));
    }
    let position_deltas = [
        [-0.06, 0.0, 0.0],
        [0.06, 0.0, 0.0],
        [0.0, -0.06, 0.0],
        [0.0, 0.06, 0.0],
        [0.0, 0.0, -0.06],
        [0.0, 0.0, 0.06],
    ];
    for delta in position_deltas {
        let mut next = parameters.clone();
        next["position_m"] = json!([
            stable_visual_metric((position[0] + delta[0]).clamp(-10.0, 10.0)),
            stable_visual_metric((position[1] + delta[1]).clamp(-10.0, 10.0)),
            stable_visual_metric((position[2] + delta[2]).clamp(-10.0, 10.0))
        ]);
        push_variant(next)?;
    }

    let scales = [0.75, 1.25];
    for scale in scales {
        let mut next = parameters.clone();
        match shape {
            "box" => {
                let values = parameters["size_m"].as_array().ok_or_else(|| {
                    RuntimeError::InvalidInput("OPTIMIZATION_RESIDUAL_SIZE_INVALID".to_owned())
                })?;
                next["size_m"] = Value::Array(
                    values
                        .iter()
                        .map(|value| {
                            let value = value.as_f64().ok_or_else(|| {
                                RuntimeError::InvalidInput(
                                    "OPTIMIZATION_RESIDUAL_SIZE_INVALID".to_owned(),
                                )
                            })?;
                            Ok(Value::from(stable_visual_metric(
                                (value * scale).clamp(1.0e-9, 10.0),
                            )))
                        })
                        .collect::<Result<Vec<_>, RuntimeError>>()?,
                );
            }
            "cylinder" => {
                let radius = parameters["radius_m"].as_f64().ok_or_else(|| {
                    RuntimeError::InvalidInput("OPTIMIZATION_RESIDUAL_RADIUS_INVALID".to_owned())
                })?;
                let height = parameters["height_m"].as_f64().ok_or_else(|| {
                    RuntimeError::InvalidInput("OPTIMIZATION_RESIDUAL_HEIGHT_INVALID".to_owned())
                })?;
                next["radius_m"] =
                    Value::from(stable_visual_metric((radius * scale).clamp(1.0e-9, 5.0)));
                next["height_m"] =
                    Value::from(stable_visual_metric((height * scale).clamp(1.0e-9, 10.0)));
            }
            "ellipsoid" => {
                let values = parameters["radii_m"].as_array().ok_or_else(|| {
                    RuntimeError::InvalidInput("OPTIMIZATION_RESIDUAL_RADII_INVALID".to_owned())
                })?;
                next["radii_m"] = Value::Array(
                    values
                        .iter()
                        .map(|value| {
                            let value = value.as_f64().ok_or_else(|| {
                                RuntimeError::InvalidInput(
                                    "OPTIMIZATION_RESIDUAL_RADII_INVALID".to_owned(),
                                )
                            })?;
                            Ok(Value::from(stable_visual_metric(
                                (value * scale).clamp(1.0e-9, 5.0),
                            )))
                        })
                        .collect::<Result<Vec<_>, RuntimeError>>()?,
                );
            }
            "sphere" => {
                let radius = parameters["radius_m"].as_f64().ok_or_else(|| {
                    RuntimeError::InvalidInput("OPTIMIZATION_RESIDUAL_RADIUS_INVALID".to_owned())
                })?;
                next["radius_m"] =
                    Value::from(stable_visual_metric((radius * scale).clamp(1.0e-9, 5.0)));
            }
            _ => {
                return Err(RuntimeError::InvalidInput(
                    "OPTIMIZATION_RESIDUAL_PARAMETERS_SHAPE_INVALID".to_owned(),
                ))
            }
        }
        push_variant(next)?;
    }
    Ok(variants)
}

/// Keep the unmodified program as the locked baseline for every optimization
/// run.  A residual is a proposed edit and therefore belongs only to
/// non-baseline candidates; otherwise the final gate would compare two
/// already-repaired programs and could not establish whether the Boolean
/// actually helped.  Candidate order selects a deterministic residual-family
/// member so checkpoint resume reproduces the same program lane.  The lane is
/// deliberately capped; remaining slots stay available for lower-cost Rig
/// exploration under the same RuntimeJob budget.
fn candidate_residual<'a>(
    context: &'a OptimizationContext,
    candidate_index: usize,
) -> Option<&'a Value> {
    if candidate_index == 0 || candidate_index > MAX_RESIDUAL_LANE_CANDIDATES {
        return None;
    }
    context
        .residual_variants
        .get((candidate_index - 1) % context.residual_variants.len().max(1))
}

fn compile_candidate(
    runtime: &Runtime,
    context: &OptimizationContext,
    parameters: Vec<Value>,
    residual: Option<&Value>,
) -> Result<CompiledCandidate, RuntimeError> {
    let (mut draft, _applied) =
        materialize_rig_geometry_program(&context.program, &context.rig, &parameters)?;
    if let Some(residual) = residual {
        apply_boolean_residual(&mut draft, residual, &context.part_id)?;
    }
    let program = finalize_v2_geometry_program(draft)?;
    let artifact =
        super::compile_geometry_with_runtime_worker(&program, None).map_err(|error| {
            RuntimeError::InvalidInput(format!("OPTIMIZATION_COMPILE_FAILED: {error}"))
        })?;
    let inspection = strict_glb_inspection(&artifact.glb)?;
    validate_worker_metadata(&artifact, &inspection)?;
    if !inspection.hard_gate_passed {
        return Err(RuntimeError::InvalidInput(
            "OPTIMIZATION_STRICT_GLB_GATE_FAILED".to_owned(),
        ));
    }
    let max_triangles = context.intent["budget"]["max_output_triangles"]
        .as_u64()
        .unwrap_or(2_000_000);
    if inspection.triangle_count == 0 || inspection.triangle_count > max_triangles {
        return Err(RuntimeError::InvalidInput(
            "OPTIMIZATION_TRIANGLE_BUDGET_EXCEEDED".to_owned(),
        ));
    }
    let program_sha256 = program["canonical_sha256"]
        .as_str()
        .ok_or_else(|| RuntimeError::InvalidInput("OPTIMIZATION_PROGRAM_HASH_MISSING".to_owned()))?
        .to_owned();
    if artifact.program_sha256 != program_sha256 {
        return Err(RuntimeError::InvalidInput(
            "OPTIMIZATION_PROGRAM_ARTIFACT_LINEAGE_MISMATCH".to_owned(),
        ));
    }
    let program_bytes = canonical_json_bytes(&program)
        .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?;
    let program_object = runtime
        .put_object(
            &program_bytes,
            None,
            "application/json",
            "geometry-program-v2",
        )
        .map_err(|error| {
            RuntimeError::InvalidInput(format!("OPTIMIZATION_PROGRAM_PUT_FAILED: {error}"))
        })?;
    let artifact_object = runtime
        .put_object(&artifact.glb, None, "model/gltf-binary", "geometry-glb")
        .map_err(|error| {
            RuntimeError::InvalidInput(format!("OPTIMIZATION_ARTIFACT_PUT_FAILED: {error}"))
        })?;
    Ok(CompiledCandidate {
        program_sha256,
        program_object_sha256: program_object.record.sha256,
        artifact_object_sha256: artifact_object.record.sha256,
        glb: artifact.glb,
        triangle_count: inspection.triangle_count,
    })
}

fn evaluate_candidate(
    runtime: &Runtime,
    context: &OptimizationContext,
    candidate: &CompiledCandidate,
    candidate_index: usize,
    sequence: usize,
    fidelity: &str,
    resolution: u32,
    _started_at: Instant,
) -> Result<EvaluationRecord, RuntimeError> {
    let started = Instant::now();
    let passes = if fidelity == "final" {
        render_glb_with_runtime_worker(&candidate.glb, &context.camera).map_err(|error| {
            RuntimeError::InvalidInput(format!("OPTIMIZATION_RENDER_FAILED: {error}"))
        })?
    } else {
        let batches = geometry_worker::render_glb_fit_batch_at_resolution(
            &candidate.glb,
            std::slice::from_ref(&context.camera),
            resolution,
        )
        .map_err(|error| {
            RuntimeError::InvalidInput(format!("OPTIMIZATION_RENDER_FAILED: {error}"))
        })?;
        batches
            .into_iter()
            .next()
            .ok_or_else(|| RuntimeError::InvalidInput("OPTIMIZATION_RENDER_EMPTY".to_owned()))?
    };
    let silhouette = passes
        .iter()
        .find(|pass| pass.pass == "silhouette")
        .ok_or_else(|| {
            RuntimeError::InvalidInput("OPTIMIZATION_SILHOUETTE_PASS_MISSING".to_owned())
        })?;
    let model_mask = decode_binary_mask_at_resolution(&silhouette.png, resolution as usize)?;
    let target_mask = downsample_mask(&context.target_mask, 512, resolution as usize);
    let base_metrics =
        super::camera_fit_metrics_at_resolution(&target_mask, &model_mask, resolution as usize);
    let part_ids = candidate_part_ids(candidate)?;
    let part_context = passes
        .iter()
        .find(|pass| pass.pass == "part-id")
        .map(|pass| (pass.png.as_slice(), part_ids.as_slice()));
    let mut metrics = transient_loss_metrics_at_resolution(
        &base_metrics,
        &model_mask,
        resolution as usize,
        context.target.get("landmarks"),
        part_context,
    );
    let full_model_mask = decode_binary_mask(&silhouette.png)?;
    let chamfer = super::sdf_chamfer_px(&context.target_mask, &full_model_mask);
    let landmark_values = context
        .target
        .get("landmarks")
        .and_then(Value::as_array)
        .filter(|values| !values.is_empty());
    let landmark_coverage = metrics
        .get("landmark_coverage")
        .and_then(Value::as_f64)
        .unwrap_or(if landmark_values.is_some() { 0.0 } else { 1.0 });
    let landmark_nme = metrics
        .get("landmark_nme")
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    let part_region_error = part_region_error(
        context,
        passes
            .iter()
            .find(|pass| pass.pass == "part-id")
            .map(|pass| pass.png.as_slice()),
        &part_ids,
        resolution as usize,
    );
    metrics["landmark_coverage"] = Value::from(stable_visual_metric(landmark_coverage));
    metrics["landmark_nme"] = Value::from(stable_visual_metric(landmark_nme));
    metrics["part_region_error"] = Value::from(stable_visual_metric(part_region_error));
    metrics["sdf_chamfer_px"] = Value::from(stable_visual_metric(chamfer));
    let part_metrics = context.evaluation_objective.as_ref().map(|_| {
        let part_target_mask = context
            .part_target_mask
            .as_deref()
            .expect("unified evaluation objective has a Part target mask");
        let part_png = passes
            .iter()
            .find(|pass| pass.pass == "part-id")
            .map(|pass| pass.png.as_slice())
            .unwrap_or(&[]);
        super::objective_part_metrics(part_target_mask, part_png, &part_ids, &context.part_id)
    });
    let complexity_penalty = (candidate.triangle_count as f64
        / context.intent["budget"]["max_output_triangles"]
            .as_f64()
            .unwrap_or(2_000_000.0)
            .max(1.0))
    .clamp(0.0, 1.0);
    let objective = &context.intent["objective"];
    let loss = objective["silhouette_iou"].as_f64().unwrap_or(0.0)
        * (1.0 - metrics["silhouette_iou"].as_f64().unwrap_or(0.0))
        + objective["boundary_f1_4px"].as_f64().unwrap_or(0.0)
            * (1.0 - metrics["boundary_f1_4px"].as_f64().unwrap_or(0.0))
        + objective["landmark_coverage"].as_f64().unwrap_or(0.0) * (1.0 - landmark_coverage)
        + objective["landmark_nme"].as_f64().unwrap_or(0.0) * landmark_nme
        + objective["part_region"].as_f64().unwrap_or(0.0) * part_region_error
        + objective["program_complexity"].as_f64().unwrap_or(0.0) * complexity_penalty;
    let final_object_hashes = if fidelity == "final" {
        passes
            .iter()
            .map(|pass| {
                runtime
                    .put_object(
                        &pass.png,
                        None,
                        "image/png",
                        &format!("render-pass-{}", pass.pass),
                    )
                    .map_err(|error| {
                        RuntimeError::InvalidInput(format!(
                            "OPTIMIZATION_RENDER_PASS_PUT_FAILED: {error}"
                        ))
                    })
                    .map(|object| object.record.sha256)
            })
            .collect::<Result<Vec<_>, RuntimeError>>()?
    } else {
        Vec::new()
    };
    let pass_hashes = passes
        .iter()
        .enumerate()
        .map(|(index, pass)| {
            json!({
                "pass":pass.pass,
                "sha256":final_object_hashes.get(index).cloned().unwrap_or_else(|| sha256_hex(&pass.png)),
                "width":pass.width,
                "height":pass.height
            })
        })
        .collect::<Vec<_>>();
    let render_sha256 = canonical_json_hash(&json!({
        "fidelity":fidelity,
        "resolution":resolution,
        "camera_hash":context.camera["camera_hash"],
        "passes":pass_hashes
    }));
    let metrics_sha256 = canonical_json_hash(&metrics);
    let mut evaluation = json!({
        "schema_version":"OptimizationEvaluation@1",
        "evaluation_id":format!("optimization-evaluation-{}-{}", fidelity, candidate_index),
        "job_id":context.intent["job_id"],
        "sequence":sequence,
        "fidelity":fidelity,
        "resolution":resolution,
        "program_sha256":candidate.program_sha256,
        "render_sha256":render_sha256,
        "metrics_sha256":metrics_sha256,
        "metrics":metrics,
        "loss":stable_visual_metric(loss),
        "complexity_penalty":stable_visual_metric(complexity_penalty),
        "triangle_count":candidate.triangle_count,
        "valid":true,
        "duration_ms":started.elapsed().as_millis().min(120_000) as u64,
        "pass_hashes":pass_hashes,
        "canonical_sha256":""
    });
    if let Some(part_metrics) = part_metrics {
        evaluation["part_metrics"] = part_metrics;
    }
    if let Some(objective_sha256) = context.evaluation_objective_sha256.as_deref() {
        evaluation["evaluation_objective_sha256"] = Value::String(objective_sha256.to_owned());
    }
    evaluation["canonical_sha256"] = Value::String(canonical_json_hash(&evaluation));
    let bytes = canonical_json_bytes(&evaluation)
        .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?;
    let object = runtime
        .put_object(&bytes, None, "application/json", "optimization-evaluation")
        .map_err(|error| {
            RuntimeError::InvalidInput(format!("OPTIMIZATION_EVALUATION_PUT_FAILED: {error}"))
        })?;
    Ok(EvaluationRecord {
        value: evaluation,
        object_sha256: object.record.sha256,
        candidate_index,
        loss,
        fidelity: fidelity.to_owned(),
        final_object_hashes,
    })
}

fn candidate_part_ids(candidate: &CompiledCandidate) -> Result<Vec<String>, RuntimeError> {
    let inspection = strict_glb_inspection(&candidate.glb)?;
    Ok(inspection.part_ids)
}

fn part_region_error(
    context: &OptimizationContext,
    part_png: Option<&[u8]>,
    part_ids: &[String],
    resolution: usize,
) -> f64 {
    let Some(part_png) = part_png else {
        return if context
            .target
            .get("parts")
            .and_then(Value::as_array)
            .is_some_and(|parts| {
                parts
                    .iter()
                    .any(|part| part["part_id"].as_str() == Some(context.part_id.as_str()))
            }) {
            1.0
        } else {
            0.0
        };
    };
    let Some(target_part) =
        super::target_part_region_mask(&context.target, &context.part_id, &context.target_mask)
            .or_else(|| super::target_part_boundary_mask(&context.target, &context.part_id))
    else {
        return 0.0;
    };
    let target = downsample_mask(&target_part, 512, resolution);
    let Ok(image) = image::load_from_memory(part_png) else {
        return 1.0;
    };
    let image = image.resize_exact(
        resolution as u32,
        resolution as u32,
        image::imageops::FilterType::Nearest,
    );
    let mut model = vec![false; resolution * resolution];
    for (index, value) in model.iter_mut().enumerate() {
        let pixel = image
            .get_pixel((index % resolution) as u32, (index / resolution) as u32)
            .0;
        if let Some(part_index) = super::part_color_index(pixel) {
            *value = part_ids
                .get(part_index)
                .is_some_and(|value| value == &context.part_id);
        }
    }
    let intersection = target
        .iter()
        .zip(model.iter())
        .filter(|(left, right)| **left && **right)
        .count();
    let union = target
        .iter()
        .zip(model.iter())
        .filter(|(left, right)| **left || **right)
        .count();
    if union == 0 {
        1.0
    } else {
        1.0 - intersection as f64 / union as f64
    }
}

fn evaluation_for_candidate<'a>(
    evaluations: &'a [EvaluationRecord],
    candidate_index: usize,
    fidelity: &str,
) -> Option<&'a EvaluationRecord> {
    evaluations.iter().find(|evaluation| {
        evaluation.candidate_index == candidate_index && evaluation.fidelity == fidelity
    })
}

fn evaluation_metric(evaluation: &EvaluationRecord, name: &str) -> Option<f64> {
    if name == "complexity_penalty" {
        evaluation.value.get(name).and_then(Value::as_f64)
    } else {
        evaluation
            .value
            .get("metrics")?
            .get(name)
            .and_then(Value::as_f64)
    }
}

/// Select final proposal finalists from more than one locked-metric lane.
/// The first slot follows the normal shape-first ranking; the second slot
/// preserves the best landmark-NME candidate seen at mid fidelity.  This keeps
/// a shape winner from consuming the entire final budget when it trades away
/// landmark alignment, while the immutable baseline remains a separate final
/// control in the caller.
fn select_final_proposal_indices(
    ranked: &[&EvaluationRecord],
    baseline_index: usize,
    final_top_k: usize,
    unified_objective: bool,
) -> Vec<usize> {
    let mut selected = Vec::with_capacity(final_top_k);
    if final_top_k == 0 {
        return selected;
    }
    if unified_objective {
        for evaluation in ranked {
            if selected.len() >= final_top_k {
                break;
            }
            if evaluation.candidate_index != baseline_index
                && !selected.contains(&evaluation.candidate_index)
            {
                selected.push(evaluation.candidate_index);
            }
        }
        return selected;
    }
    if let Some(primary) = ranked
        .iter()
        .find(|evaluation| evaluation.candidate_index != baseline_index)
    {
        selected.push(primary.candidate_index);
    }
    if final_top_k > 1 {
        let landmark_stable = ranked
            .iter()
            .filter(|evaluation| {
                evaluation.candidate_index != baseline_index
                    && !selected.contains(&evaluation.candidate_index)
            })
            .min_by(|left, right| {
                let left_nme = evaluation_metric(left, "landmark_nme").unwrap_or(f64::INFINITY);
                let right_nme = evaluation_metric(right, "landmark_nme").unwrap_or(f64::INFINITY);
                left_nme
                    .partial_cmp(&right_nme)
                    .unwrap_or(Ordering::Equal)
                    .then_with(|| compare_evaluation_quality(*left, *right))
                    .then_with(|| left.candidate_index.cmp(&right.candidate_index))
            });
        if let Some(candidate) = landmark_stable {
            selected.push(candidate.candidate_index);
        }
    }
    for evaluation in ranked {
        if selected.len() >= final_top_k {
            break;
        }
        let candidate_index = evaluation.candidate_index;
        if candidate_index != baseline_index && !selected.contains(&candidate_index) {
            selected.push(candidate_index);
        }
    }
    selected
}

fn compare_evaluation_quality(left: &EvaluationRecord, right: &EvaluationRecord) -> Ordering {
    // The ordering intentionally puts contour shape before scalar loss.  A
    // lower loss at one fidelity is not allowed to hide a worse silhouette
    // edge or centroid at the same fidelity.
    const PRIORITY: [(&str, bool); 9] = [
        ("boundary_f1_4px", true),
        ("silhouette_iou", true),
        ("bbox_edge_error", false),
        ("centroid_error", false),
        ("landmark_coverage", true),
        ("landmark_nme", false),
        ("part_region_error", false),
        ("sdf_chamfer_px", false),
        ("complexity_penalty", false),
    ];
    for (name, higher_is_better) in PRIORITY {
        let left_value = evaluation_metric(left, name).unwrap_or(if higher_is_better {
            f64::NEG_INFINITY
        } else {
            f64::INFINITY
        });
        let right_value = evaluation_metric(right, name).unwrap_or(if higher_is_better {
            f64::NEG_INFINITY
        } else {
            f64::INFINITY
        });
        let ordering = if higher_is_better {
            right_value
                .partial_cmp(&left_value)
                .unwrap_or(Ordering::Equal)
        } else {
            left_value
                .partial_cmp(&right_value)
                .unwrap_or(Ordering::Equal)
        };
        if ordering != Ordering::Equal {
            return ordering;
        }
    }
    left.loss
        .partial_cmp(&right.loss)
        .unwrap_or(Ordering::Equal)
        .then_with(|| {
            left.value["sequence"]
                .as_u64()
                .cmp(&right.value["sequence"].as_u64())
        })
}

fn compare_unified_evaluation_quality(
    left: &EvaluationRecord,
    right: &EvaluationRecord,
) -> Ordering {
    let part_order = super::compare_part_metric_values(
        left.value.get("part_metrics").unwrap_or(&Value::Null),
        right.value.get("part_metrics").unwrap_or(&Value::Null),
    );
    if part_order != Ordering::Equal {
        return part_order;
    }
    let left_global = json!({
        "candidate_id":left.candidate_index.to_string(),
        "metrics":left.value.get("metrics").cloned().unwrap_or(Value::Null),
        "loss":0.0
    });
    let right_global = json!({
        "candidate_id":right.candidate_index.to_string(),
        "metrics":right.value.get("metrics").cloned().unwrap_or(Value::Null),
        "loss":0.0
    });
    super::compare_silhouette_candidate_rows(&left_global, &right_global)
        .then_with(|| left.candidate_index.cmp(&right.candidate_index))
}

fn compare_evaluation_quality_for_context(
    context: &OptimizationContext,
    left: &EvaluationRecord,
    right: &EvaluationRecord,
) -> Ordering {
    if context.evaluation_objective.is_some() {
        compare_unified_evaluation_quality(left, right)
    } else {
        compare_evaluation_quality(left, right)
    }
}

fn best_completed_evaluation<'a>(
    evaluations: &'a [EvaluationRecord],
) -> Option<&'a EvaluationRecord> {
    for fidelity in ["final", "mid", "coarse"] {
        let best = evaluations
            .iter()
            .filter(|evaluation| evaluation.fidelity == fidelity)
            .min_by(|left, right| compare_evaluation_quality(*left, *right));
        if best.is_some() {
            return best;
        }
    }
    None
}

fn best_completed_evaluation_for_context<'a>(
    evaluations: &'a [EvaluationRecord],
    context: &OptimizationContext,
) -> Option<&'a EvaluationRecord> {
    if context.evaluation_objective.is_none() {
        return best_completed_evaluation(evaluations);
    }
    for fidelity in ["final", "mid", "coarse"] {
        let best = evaluations
            .iter()
            .filter(|evaluation| evaluation.fidelity == fidelity)
            .min_by(|left, right| compare_evaluation_quality_for_context(context, left, right));
        if best.is_some() {
            return best;
        }
    }
    None
}

fn best_result_evaluation<'a>(
    evaluations: &'a [EvaluationRecord],
    strict_improvement: bool,
) -> Option<&'a EvaluationRecord> {
    if strict_improvement {
        if let Some(baseline_final) = evaluation_for_candidate(evaluations, 0, "final") {
            let gated = evaluations
                .iter()
                .filter(|evaluation| evaluation.fidelity == "final")
                .filter(|evaluation| {
                    evaluation.candidate_index == 0
                        || compare_final_evaluations(baseline_final, evaluation).non_regressing
                })
                .min_by(|left, right| compare_evaluation_quality(*left, *right));
            if gated.is_some() {
                return gated;
            }
        }
    }
    best_completed_evaluation(evaluations)
}

fn best_result_evaluation_for_context<'a>(
    evaluations: &'a [EvaluationRecord],
    strict_improvement: bool,
    context: &OptimizationContext,
) -> Option<&'a EvaluationRecord> {
    if context.evaluation_objective.is_none() {
        return best_result_evaluation(evaluations, strict_improvement);
    }
    if strict_improvement {
        if let Some(baseline_final) = evaluation_for_candidate(evaluations, 0, "final") {
            let gated = evaluations
                .iter()
                .filter(|evaluation| evaluation.fidelity == "final")
                .filter(|evaluation| {
                    evaluation.candidate_index != 0
                        && compare_final_evaluations_for_context(
                            baseline_final,
                            evaluation,
                            context,
                        )
                        .strict_improvement
                })
                .min_by(|left, right| compare_evaluation_quality_for_context(context, left, right));
            if gated.is_some() {
                return gated;
            }
        }
    }
    best_completed_evaluation_for_context(evaluations, context)
}

fn compare_final_evaluations(
    baseline: &EvaluationRecord,
    proposal: &EvaluationRecord,
) -> MultiObjectiveComparison {
    if baseline.fidelity != "final" || proposal.fidelity != "final" {
        return MultiObjectiveComparison {
            non_regressing: false,
            strict_improvement: false,
        };
    }
    const METRICS: [(&str, bool); 9] = [
        ("boundary_f1_4px", true),
        ("silhouette_iou", true),
        ("bbox_edge_error", false),
        ("centroid_error", false),
        ("landmark_coverage", true),
        ("landmark_nme", false),
        ("part_region_error", false),
        ("sdf_chamfer_px", false),
        ("complexity_penalty", false),
    ];
    let mut non_regressing = true;
    let mut strict_improvement = false;
    for (name, higher_is_better) in METRICS {
        let Some(baseline_value) = evaluation_metric(baseline, name) else {
            non_regressing = false;
            continue;
        };
        let Some(proposal_value) = evaluation_metric(proposal, name) else {
            non_regressing = false;
            continue;
        };
        if !baseline_value.is_finite() || !proposal_value.is_finite() {
            non_regressing = false;
            continue;
        }
        let improvement = if higher_is_better {
            proposal_value - baseline_value
        } else {
            baseline_value - proposal_value
        };
        if improvement < -STRICT_IMPROVEMENT_EPSILON {
            non_regressing = false;
        }
        if improvement > STRICT_IMPROVEMENT_EPSILON {
            strict_improvement = true;
        }
    }
    MultiObjectiveComparison {
        non_regressing,
        strict_improvement: non_regressing && strict_improvement,
    }
}

fn compare_final_evaluations_for_context(
    baseline: &EvaluationRecord,
    proposal: &EvaluationRecord,
    context: &OptimizationContext,
) -> MultiObjectiveComparison {
    if context.evaluation_objective.is_none() {
        return compare_final_evaluations(baseline, proposal);
    }
    if baseline.fidelity != "final" || proposal.fidelity != "final" {
        return MultiObjectiveComparison {
            non_regressing: false,
            strict_improvement: false,
        };
    }
    let global_non_regressing = super::objective_global_non_regressing(
        baseline.value.get("metrics").unwrap_or(&Value::Null),
        proposal.value.get("metrics").unwrap_or(&Value::Null),
    );
    let part_strict_improvement = super::objective_part_strictly_better(
        proposal.value.get("part_metrics").unwrap_or(&Value::Null),
        baseline.value.get("part_metrics").unwrap_or(&Value::Null),
    );
    MultiObjectiveComparison {
        non_regressing: global_non_regressing,
        strict_improvement: global_non_regressing && part_strict_improvement,
    }
}

fn check_run_budget(
    runtime: &Runtime,
    job_id: &str,
    started_at: Instant,
    max_runtime_ms: u64,
) -> Result<(), RuntimeError> {
    let job = runtime
        .store
        .get_job_record(job_id)?
        .ok_or_else(|| RuntimeError::InvalidInput("OPTIMIZATION_JOB_NOT_FOUND".to_owned()))?;
    if job.status == "cancelled" {
        return Err(RuntimeError::InvalidInput(
            "OPTIMIZATION_CANCELLED".to_owned(),
        ));
    }
    if job.status != "running" {
        return Err(RuntimeError::InvalidInput(
            "OPTIMIZATION_JOB_STATE_CHANGED".to_owned(),
        ));
    }
    if started_at.elapsed() > Duration::from_millis(max_runtime_ms) {
        return Err(RuntimeError::InvalidInput(
            "OPTIMIZATION_RUNTIME_BUDGET_EXCEEDED".to_owned(),
        ));
    }
    Ok(())
}

fn persist_optimization_state(
    runtime: &Runtime,
    job_id: &str,
    context: &OptimizationContext,
    evaluations: &[EvaluationRecord],
    candidates: &[CompiledCandidate],
    status: &str,
    progress: u8,
    proposal: Option<(bool, bool, Vec<String>, Vec<String>)>,
    extra_reachable: Vec<String>,
    event_kind: &str,
    next_stage: &str,
) -> Result<(), RuntimeError> {
    let strict_improvement = proposal.as_ref().map(|value| value.0).unwrap_or(false);
    let non_regressing = proposal.as_ref().map(|value| value.1).unwrap_or(false);
    let proposal_hashes = proposal
        .as_ref()
        .map(|value| value.2.clone())
        .unwrap_or_default();
    let blocked_reasons = proposal
        .as_ref()
        .map(|value| value.3.clone())
        .unwrap_or_default();
    // When a strict proposal exists, the result's best lineage must come from
    // the same non-regressing final lane that produced the proposal hashes.
    // A regressing shape winner may still rank first lexicographically, but it
    // must not be advertised as the proposal's best object.
    let best = best_result_evaluation_for_context(evaluations, strict_improvement, context);
    let fidelity_counts = json!({
        "coarse":evaluations.iter().filter(|evaluation| evaluation.fidelity == "coarse").count(),
        "mid":evaluations.iter().filter(|evaluation| evaluation.fidelity == "mid").count(),
        "final":evaluations.iter().filter(|evaluation| evaluation.fidelity == "final").count()
    });
    let proposal_status = if status == "running" {
        "not-ready"
    } else if strict_improvement {
        "proposed"
    } else if blocked_reasons
        .iter()
        .any(|reason| reason.contains("invalid"))
    {
        "blocked-invalid"
    } else {
        "blocked-no-improvement"
    };
    let promotion_policy = if context.evaluation_objective.is_some() {
        "silhouette-evaluation-objective-v1"
    } else {
        "legacy-multi-objective-v1"
    };
    let promotion_status = if status == "running" {
        "not-ready"
    } else if strict_improvement {
        "ready"
    } else if context.evaluation_objective.is_some() {
        "blocked_global_or_part_objective"
    } else {
        "blocked"
    };
    let best_program_sha256 = best
        .and_then(|evaluation| evaluation.value.get("program_sha256"))
        .cloned()
        .unwrap_or(Value::Null);
    let best_evaluation_id = best
        .and_then(|evaluation| evaluation.value.get("evaluation_id"))
        .cloned()
        .unwrap_or(Value::Null);
    let best_evaluation_fidelity = best
        .map(|evaluation| Value::String(evaluation.fidelity.clone()))
        .unwrap_or(Value::Null);
    let best_program_object_sha256 = best
        .and_then(|evaluation| candidates.get(evaluation.candidate_index))
        .map(|candidate| Value::String(candidate.program_object_sha256.clone()))
        .unwrap_or(Value::Null);
    let best_artifact_sha256 = best
        .and_then(|evaluation| candidates.get(evaluation.candidate_index))
        .map(|candidate| Value::String(candidate.artifact_object_sha256.clone()))
        .unwrap_or(Value::Null);
    let mut result = json!({
        "schema_version":"OptimizationResult@1",
        "job_id":job_id,
        "intent_sha256":context.intent_sha256,
        "evaluation_objective_sha256":context.evaluation_objective_sha256,
        "promotion_policy":promotion_policy,
        "promotion_status":promotion_status,
        "status":status,
        "baseline_loss":evaluations.iter().find(|evaluation| evaluation.fidelity == "final" && evaluation.candidate_index == 0).map(|evaluation| stable_visual_metric(evaluation.loss)).unwrap_or_else(|| best.map(|evaluation| stable_visual_metric(evaluation.loss)).unwrap_or(0.0)),
        "best_loss":best.map(|evaluation| stable_visual_metric(evaluation.loss)).unwrap_or(0.0),
        "non_regressing":non_regressing,
        "strict_improvement":strict_improvement,
        "best_evaluation_id":best_evaluation_id,
        "best_evaluation_fidelity":best_evaluation_fidelity,
        "best_program_sha256":best_program_sha256,
        "best_program_object_sha256":best_program_object_sha256,
        "best_artifact_sha256":best_artifact_sha256,
        "proposal_program_object_sha256":proposal_hashes.first().cloned().map(Value::String).unwrap_or(Value::Null),
        "proposal_artifact_sha256":proposal_hashes.get(1).cloned().map(Value::String).unwrap_or(Value::Null),
        "evaluations_count":evaluations.len(),
        "fidelity_counts":fidelity_counts,
        "evaluation_object_sha256s":evaluations.iter().map(|evaluation| Value::String(evaluation.object_sha256.clone())).collect::<Vec<_>>(),
        "checkpoint_sequence":evaluations.len(),
        "next_stage":next_stage,
        "search_strategy":OPTIMIZATION_SEARCH_STRATEGY,
        "candidate_program_object_sha256s":candidates.iter().map(|candidate| Value::String(candidate.program_object_sha256.clone())).collect::<Vec<_>>(),
        "candidate_artifact_object_sha256s":candidates.iter().map(|candidate| Value::String(candidate.artifact_object_sha256.clone())).collect::<Vec<_>>(),
        "proposal_status":proposal_status,
        "blocked_reasons":blocked_reasons,
        "canonical_sha256":""
    });
    result["canonical_sha256"] = Value::String(canonical_json_hash(&result));
    validate_result(&result, job_id, &context.intent_sha256)?;
    let bytes = canonical_json_bytes(&result)
        .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?;
    if bytes.len() > MAX_RESULT_BYTES {
        return Err(RuntimeError::InvalidInput(
            "OPTIMIZATION_RESULT_TOO_LARGE".to_owned(),
        ));
    }
    let result_object = runtime
        .put_object(&bytes, None, "application/json", "optimization-result")
        .map_err(|error| {
            RuntimeError::InvalidInput(format!("OPTIMIZATION_RESULT_PUT_FAILED: {error}"))
        })?;
    let current = runtime
        .store
        .get_job_record(job_id)?
        .ok_or_else(|| RuntimeError::InvalidInput("OPTIMIZATION_JOB_NOT_FOUND".to_owned()))?;
    let mut reachable = Vec::with_capacity(96);
    reachable.push(context.intent_sha256.clone());
    reachable.push(result_object.record.sha256.clone());
    reachable.extend(
        evaluations
            .iter()
            .map(|evaluation| evaluation.object_sha256.clone()),
    );
    reachable.extend(
        evaluations
            .iter()
            .flat_map(|evaluation| evaluation.final_object_hashes.clone()),
    );
    reachable.extend(extra_reachable);
    if let Some(previous) = current.checkpoint_sha256 {
        reachable.push(previous);
    }
    reachable.sort();
    reachable.dedup();
    let next = JobRecord {
        schema_version: current.schema_version,
        job_id: current.job_id,
        project_id: current.project_id,
        kind: current.kind,
        status: status.to_owned(),
        progress,
        request_sha256: current.request_sha256,
        checkpoint_sha256: Some(result_object.record.sha256.clone()),
        error_code: if status == "failed" {
            Some("OPTIMIZATION_RUNTIME_FAILED".to_owned())
        } else {
            None
        },
        created_at: current.created_at,
        updated_at: now_string(),
    };
    runtime.store.update_job_with_event(
        &next,
        event_kind,
        &json!({
            "result_sha256":result_object.record.sha256,
            "intent_sha256":context.intent_sha256,
            "search_strategy":OPTIMIZATION_SEARCH_STRATEGY,
            "evaluations":evaluations.len(),
            "fidelity_counts":fidelity_counts,
            "best_loss":best.map(|evaluation| stable_visual_metric(evaluation.loss)).unwrap_or(0.0),
            "best_evaluation_fidelity":best.map(|evaluation| evaluation.fidelity.clone()),
            "strict_improvement":strict_improvement,
            "proposal_status":proposal_status
        }),
        &reachable,
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rig() -> Value {
        json!({
            "parameters":[
                {"parameter_id":"width","part_id":"shell","value":1.0,"min":0.5,"max":1.5,"step":0.1},
                {"parameter_id":"height","part_id":"shell","value":2.0,"min":1.0,"max":3.0,"step":0.2},
                {"parameter_id":"depth","part_id":"shell","value":0.8,"min":0.4,"max":1.2,"step":0.1},
                {"parameter_id":"offset","part_id":"shell","value":0.0,"min":-0.5,"max":0.5,"step":0.05}
            ]
        })
    }

    #[test]
    fn exploration_budget_is_small_but_parameter_aware() {
        let rig = rig();
        assert_eq!(exploration_candidate_count(&rig, 32), 8);
        assert_eq!(exploration_candidate_count(&rig, 48), 8);
        assert_eq!(exploration_candidate_count(&rig, 6), 6);
    }

    #[test]
    fn adaptive_candidates_are_local_ordered_and_within_bounds() {
        let rig = rig();
        let baseline = baseline_parameters(&rig);
        let variants = adaptive_parameter_sets(&rig, &baseline, 10);
        assert_eq!(variants.len(), 10);
        assert_ne!(variants[0], baseline);
        assert_ne!(variants[1], baseline);
        for parameters in &variants {
            assert_eq!(parameters.len(), 4);
            for (parameter, definition) in
                parameters.iter().zip(rig["parameters"].as_array().unwrap())
            {
                let value = parameter["value"].as_f64().unwrap();
                assert!(value >= definition["min"].as_f64().unwrap());
                assert!(value <= definition["max"].as_f64().unwrap());
            }
        }
        assert_eq!(variants, adaptive_parameter_sets(&rig, &baseline, 10));
    }

    #[test]
    fn surface_candidates_move_bounded_control_point_groups_together() {
        let rig = json!({
            "parameters":[
                {"parameter_id":"control-point-4-x","part_id":"shell","semantic":"surface_control_point","control_point_index":4,"axis":"x","value":-0.8,"min":-1.0,"max":-0.6,"step":0.05,"unit":"meter"},
                {"parameter_id":"control-point-7-x","part_id":"shell","semantic":"surface_control_point","control_point_index":7,"axis":"x","value":0.8,"min":0.6,"max":1.0,"step":0.05,"unit":"meter"},
                {"parameter_id":"control-point-8-x","part_id":"shell","semantic":"surface_control_point","control_point_index":8,"axis":"x","value":-0.8,"min":-1.0,"max":-0.6,"step":0.05,"unit":"meter"},
                {"parameter_id":"control-point-11-x","part_id":"shell","semantic":"surface_control_point","control_point_index":11,"axis":"x","value":0.8,"min":0.6,"max":1.0,"step":0.05,"unit":"meter"}
            ]
        });
        let baseline = baseline_parameters(&rig);
        let coarse = deterministic_parameter_sets(&rig, &baseline, 5);
        assert_eq!(coarse.len(), 5);
        assert_eq!(coarse[0], baseline);
        assert_eq!(coarse[1][0]["value"], -0.85);
        assert_eq!(coarse[1][1]["value"], 0.85);
        assert_eq!(coarse[1][2]["value"], -0.8);
        let adaptive = adaptive_parameter_sets(&rig, &baseline, 2);
        assert_eq!(adaptive.len(), 2);
        assert_eq!(adaptive[0][0]["value"], -0.85);
        assert_eq!(adaptive[0][1]["value"], 0.85);
        assert_eq!(adaptive[1][0]["value"], -0.75);
        assert_eq!(adaptive[1][1]["value"], 0.75);
        assert_eq!(coarse, deterministic_parameter_sets(&rig, &baseline, 5));
        assert_eq!(adaptive, adaptive_parameter_sets(&rig, &baseline, 2));
    }

    #[test]
    fn residual_lane_keeps_candidate_zero_unmodified() {
        let context = OptimizationContext {
            intent: Value::Null,
            intent_sha256: "a".repeat(64),
            target: Value::Null,
            target_mask: Vec::new(),
            evaluation_objective: None,
            evaluation_objective_sha256: None,
            part_target_mask: None,
            program: Value::Null,
            camera: Value::Null,
            rig: Value::Null,
            part_id: "shell".to_owned(),
            residual_variants: vec![json!({"schema_version":"OptimizationResidual@1"})],
        };
        assert!(candidate_residual(&context, 0).is_none());
        assert!(candidate_residual(&context, 1).is_some());
        assert!(candidate_residual(&context, MAX_RESIDUAL_LANE_CANDIDATES + 1).is_none());
    }

    #[test]
    fn residual_family_is_deterministic_and_stays_within_typed_bounds() {
        let mut residual = json!({
            "schema_version":"OptimizationResidual@1",
            "part_id":"shell",
            "node_id":"residual-shell",
            "operation":"union",
            "parameters":{
                "shape":"sphere",
                "radius_m":0.13,
                "longitude_segments":16,
                "latitude_segments":8,
                "position_m":[0.0,1.98,0.08],
                "rotation_rad":[0.0,0.0,0.0]
            },
            "source_critic_report_sha256":"b".repeat(64),
            "source_part_error_sha256":"c".repeat(64),
            "canonical_sha256":""
        });
        residual["canonical_sha256"] = Value::String(canonical_json_hash(&residual));
        let variants = residual_variant_family(&residual).expect("bounded residual family");
        assert_eq!(variants.len(), 9);
        assert_eq!(variants[0], residual);
        for variant in &variants {
            validate_residual_primitive_parameters(&variant["parameters"])
                .expect("family member stays within residual contract");
        }
        assert_eq!(
            variants,
            residual_variant_family(&residual).expect("deterministic family")
        );
    }

    fn evaluation(
        fidelity: &str,
        candidate_index: usize,
        sequence: u64,
        metrics: Value,
        complexity_penalty: f64,
        loss: f64,
    ) -> EvaluationRecord {
        EvaluationRecord {
            value: json!({
                "evaluation_id":format!("optimization-evaluation-{}-{}", fidelity, candidate_index),
                "sequence":sequence,
                "fidelity":fidelity,
                "metrics":metrics,
                "complexity_penalty":complexity_penalty,
                "program_sha256":"a".repeat(64)
            }),
            object_sha256: "b".repeat(64),
            candidate_index,
            loss,
            fidelity: fidelity.to_owned(),
            final_object_hashes: Vec::new(),
        }
    }

    #[test]
    fn best_so_far_stays_inside_the_highest_completed_fidelity() {
        let coarse = evaluation(
            "coarse",
            3,
            1,
            json!({
                "boundary_f1_4px":0.99,
                "silhouette_iou":0.99,
                "bbox_edge_error":0.01,
                "centroid_error":0.01,
                "landmark_coverage":1.0,
                "landmark_nme":0.01,
                "part_region_error":0.01,
                "sdf_chamfer_px":0.01
            }),
            0.01,
            0.01,
        );
        let final_evaluation = evaluation(
            "final",
            0,
            2,
            json!({
                "boundary_f1_4px":0.70,
                "silhouette_iou":0.70,
                "bbox_edge_error":0.30,
                "centroid_error":0.30,
                "landmark_coverage":0.70,
                "landmark_nme":0.30,
                "part_region_error":0.30,
                "sdf_chamfer_px":0.30
            }),
            0.30,
            0.80,
        );
        let evaluations = [coarse, final_evaluation];
        let best = best_completed_evaluation(&evaluations)
            .expect("highest completed fidelity should have a best");
        assert_eq!(best.fidelity, "final");
        assert_eq!(best.candidate_index, 0);
    }

    #[test]
    fn final_funnel_keeps_shape_and_landmark_lanes_distinct() {
        let shape = evaluation(
            "mid",
            8,
            1,
            json!({
                "boundary_f1_4px":0.90,
                "silhouette_iou":0.90,
                "bbox_edge_error":0.10,
                "centroid_error":0.10,
                "landmark_coverage":0.8,
                "landmark_nme":0.20,
                "part_region_error":0.10,
                "sdf_chamfer_px":10.0
            }),
            0.10,
            0.10,
        );
        let landmark_stable = evaluation(
            "mid",
            16,
            2,
            json!({
                "boundary_f1_4px":0.80,
                "silhouette_iou":0.80,
                "bbox_edge_error":0.20,
                "centroid_error":0.20,
                "landmark_coverage":0.9,
                "landmark_nme":0.05,
                "part_region_error":0.20,
                "sdf_chamfer_px":20.0
            }),
            0.10,
            0.20,
        );
        let fallback = evaluation(
            "mid",
            13,
            3,
            json!({
                "boundary_f1_4px":0.70,
                "silhouette_iou":0.70,
                "bbox_edge_error":0.30,
                "centroid_error":0.30,
                "landmark_coverage":0.7,
                "landmark_nme":0.30,
                "part_region_error":0.30,
                "sdf_chamfer_px":30.0
            }),
            0.10,
            0.30,
        );
        let ranked = vec![&shape, &landmark_stable, &fallback];
        assert_eq!(
            select_final_proposal_indices(&ranked, 0, 2, false),
            vec![8, 16]
        );
        assert_eq!(
            select_final_proposal_indices(&ranked, 0, 3, false),
            vec![8, 16, 13]
        );
    }

    #[test]
    fn final_multi_objective_gate_rejects_a_priority_metric_regression() {
        let baseline = evaluation(
            "final",
            0,
            1,
            json!({
                "boundary_f1_4px":0.80,
                "silhouette_iou":0.80,
                "bbox_edge_error":0.20,
                "centroid_error":0.20,
                "landmark_coverage":1.0,
                "landmark_nme":0.10,
                "part_region_error":0.10,
                "sdf_chamfer_px":0.10
            }),
            0.10,
            0.40,
        );
        let proposal = evaluation(
            "final",
            1,
            2,
            json!({
                "boundary_f1_4px":0.79,
                "silhouette_iou":0.90,
                "bbox_edge_error":0.10,
                "centroid_error":0.10,
                "landmark_coverage":1.0,
                "landmark_nme":0.05,
                "part_region_error":0.05,
                "sdf_chamfer_px":0.05
            }),
            0.05,
            0.20,
        );
        let comparison = compare_final_evaluations(&baseline, &proposal);
        assert!(!comparison.non_regressing);
        assert!(!comparison.strict_improvement);
    }

    #[test]
    fn unified_objective_requires_global_non_regression_and_part_strict_improvement() {
        let context = OptimizationContext {
            intent: Value::Null,
            intent_sha256: "a".repeat(64),
            target: Value::Null,
            target_mask: Vec::new(),
            evaluation_objective: Some(json!({"schema_version":"SilhouetteEvaluationObjective@1"})),
            evaluation_objective_sha256: Some("d".repeat(64)),
            part_target_mask: None,
            program: Value::Null,
            camera: Value::Null,
            rig: Value::Null,
            part_id: "shell".to_owned(),
            residual_variants: Vec::new(),
        };
        let baseline = evaluation(
            "final",
            0,
            1,
            json!({
                "boundary_f1_4px":0.80,
                "silhouette_iou":0.80,
                "bbox_edge_error":0.20,
                "centroid_error":0.20,
                "sdf_chamfer_px":20.0,
                "landmark_coverage":1.0,
                "landmark_nme":0.1,
                "part_region_error":0.1
            }),
            0.1,
            0.4,
        );
        let mut proposal = evaluation(
            "final",
            1,
            2,
            json!({
                "boundary_f1_4px":0.80,
                "silhouette_iou":0.81,
                "bbox_edge_error":0.19,
                "centroid_error":0.19,
                "sdf_chamfer_px":19.0,
                "landmark_coverage":1.0,
                "landmark_nme":0.1,
                "part_region_error":0.1
            }),
            0.1,
            0.3,
        );
        proposal.value["part_metrics"] = json!({
            "status":"ready",
            "part_boundary_error_px":12.0,
            "part_boundary_f1_4px":0.30,
            "part_silhouette_iou":0.40,
            "part_sdf_chamfer_px":12.0
        });
        let mut baseline = baseline;
        baseline.value["part_metrics"] = json!({
            "status":"ready",
            "part_boundary_error_px":16.0,
            "part_boundary_f1_4px":0.20,
            "part_silhouette_iou":0.30,
            "part_sdf_chamfer_px":16.0
        });
        let comparison = compare_final_evaluations_for_context(&baseline, &proposal, &context);
        assert!(comparison.non_regressing);
        assert!(comparison.strict_improvement);

        proposal.value["metrics"]["boundary_f1_4px"] = Value::from(0.79);
        let blocked = compare_final_evaluations_for_context(&baseline, &proposal, &context);
        assert!(!blocked.non_regressing);
        assert!(!blocked.strict_improvement);
    }

    #[test]
    fn strict_result_best_stays_inside_the_non_regressing_final_lane() {
        let baseline = evaluation(
            "final",
            0,
            1,
            json!({
                "boundary_f1_4px":0.80,
                "silhouette_iou":0.80,
                "bbox_edge_error":0.20,
                "centroid_error":0.20,
                "landmark_coverage":1.0,
                "landmark_nme":0.10,
                "part_region_error":0.10,
                "sdf_chamfer_px":0.10
            }),
            0.40,
            0.40,
        );
        let regressing_shape_winner = evaluation(
            "final",
            1,
            2,
            json!({
                "boundary_f1_4px":0.90,
                "silhouette_iou":0.90,
                "bbox_edge_error":0.10,
                "centroid_error":0.10,
                "landmark_coverage":1.0,
                "landmark_nme":0.20,
                "part_region_error":0.05,
                "sdf_chamfer_px":0.05
            }),
            0.10,
            0.20,
        );
        let non_regressing_finalist = evaluation(
            "final",
            2,
            3,
            json!({
                "boundary_f1_4px":0.81,
                "silhouette_iou":0.81,
                "bbox_edge_error":0.19,
                "centroid_error":0.19,
                "landmark_coverage":1.0,
                "landmark_nme":0.10,
                "part_region_error":0.09,
                "sdf_chamfer_px":0.09
            }),
            0.39,
            0.30,
        );
        let evaluations = [baseline, regressing_shape_winner, non_regressing_finalist];
        assert_eq!(
            best_completed_evaluation(&evaluations)
                .expect("final result exists")
                .candidate_index,
            1
        );
        assert_eq!(
            best_result_evaluation(&evaluations, true)
                .expect("gated final result exists")
                .candidate_index,
            2
        );
    }

    #[test]
    fn boolean_residual_adds_one_typed_same_part_dag() {
        let mut draft = json!({
            "schema_version":"GeometryProgram@2",
            "nodes":[{
                "node_id":"shell-source",
                "operator_id":"forgecad.geometry.primitive@2",
                "inputs":[],
                "parameters":{
                    "shape":"box",
                    "size_m":[1.0,1.0,1.0],
                    "position_m":[0.0,0.0,0.0],
                    "rotation_rad":[0.0,0.0,0.0]
                }
            }],
            "part_outputs":[{
                "part_id":"shell",
                "input_node_ids":["shell-source"],
                "material_zone_id":"zone-shell",
                "solid":true
            }],
            "canonical_sha256":"c".repeat(64)
        });
        let residual = json!({
            "node_id":"residual-shell-notch",
            "operation":"intersection",
            "parameters":{
                "shape":"box",
                "size_m":[0.2,0.2,0.2],
                "position_m":[0.0,0.0,0.0],
                "rotation_rad":[0.0,0.0,0.0]
            }
        });
        apply_boolean_residual(&mut draft, &residual, "shell").expect("typed residual");
        assert_eq!(draft["nodes"].as_array().unwrap().len(), 3);
        assert_eq!(
            draft["part_outputs"][0]["input_node_ids"],
            json!(["residual-shell-notch-boolean"])
        );
        assert_eq!(
            draft["nodes"][2]["operator_id"],
            "forgecad.geometry.boolean@1"
        );
        assert_eq!(draft["nodes"][2]["parameters"]["shape"], "intersection");
    }
}
