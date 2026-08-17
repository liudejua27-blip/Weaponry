//! Runtime-owned bounded orchestration for a same-stage DesignAction batch.
//!
//! A batch is deliberately not a hidden mesh transaction.  Each item is
//! delegated to the existing immutable DesignActionRun path and the batch is
//! indexed by the existing RuntimeJob/event state machine.  This gives the
//! harness deterministic resume semantics while keeping proposal candidates
//! independent until a later, explicit promotion flow is approved.

use super::{
    canonical_json_bytes, canonical_json_hash, hash_geometry_program_with_runtime_worker,
    is_opaque_id, now_string, Runtime, RuntimeError,
};
use forgecad_contracts::{is_sha256, JobEventRecord, JobRecord};
use serde_json::{json, Map, Value};
use std::collections::HashSet;

const MAX_BATCH_ACTIONS: usize = 6;
const JOB_KIND: &str = "agentic_design_stage_batch";
const BATCH_ACTION_KINDS: [&str; 6] = [
    "checkpoint",
    "primary-blockout",
    "primary-form-adjustment",
    "secondary-structure",
    "tertiary-detail",
    "bounded-repair",
];
const COMPOSITION_ACTION_KINDS: [&str; 5] = [
    "primary-blockout",
    "primary-form-adjustment",
    "secondary-structure",
    "tertiary-detail",
    "bounded-repair",
];
const COMPOSITION_JOB_KIND: &str = "agentic_design_composition";
const MAX_COMPOSITION_ACTIONS: usize = 6;

#[derive(Debug, Clone)]
struct CompositionMergePlan {
    step_records: Vec<Value>,
    final_step_index: usize,
    final_action: Value,
    final_proposal: Value,
    final_program_sha256: String,
}

impl Runtime {
    /// Execute a bounded ordered list of same-stage actions.  The batch is
    /// resumable by `batch_id` and `input_sha256`; every item is still an
    /// independent DesignActionRun receipt.  The method stops at the first
    /// blocked quality gate and never promotes a proposal candidate.
    pub fn design_stage_run_prepare(&self, request: Value) -> Result<Value, RuntimeError> {
        let object = request_object(&request, "design_stage_run_prepare")?;
        reject_unknown_keys(
            object,
            &[
                "project_id",
                "session_id",
                "candidate_id",
                "batch_id",
                "requested_stage",
                "actions",
                "observation_sha256",
                "input_sha256",
                "approved",
                "approval_receipt_id",
                "approval_summary",
                "approval_expires_at",
                "approval_session_id",
                "idempotency_key",
            ],
        )?;
        require_approval(object)?;

        let project_id = required_id(object, "project_id")?;
        let session_id = required_id(object, "session_id")?;
        let candidate_id = required_id(object, "candidate_id")?;
        let batch_id = required_id(object, "batch_id")?;
        let requested_stage = required_stage(object, "requested_stage")?;
        let observation_sha256 = required_sha(object, "observation_sha256")?;
        let input_sha256 = required_sha(object, "input_sha256")?;
        let _idempotency_key = required_id(object, "idempotency_key")?;
        if object
            .get("approval_session_id")
            .and_then(Value::as_str)
            .is_some_and(|approval_session_id| approval_session_id != session_id)
        {
            return Err(RuntimeError::InvalidInput(
                "AGENTIC_APPROVAL_SESSION_MISMATCH".to_owned(),
            ));
        }
        let actions = object
            .get("actions")
            .and_then(Value::as_array)
            .ok_or_else(|| RuntimeError::InvalidInput("actions is required".to_owned()))?;
        validate_batch_actions(actions)?;

        let session = self
            .store
            .get_agentic_session(session_id)?
            .ok_or_else(|| RuntimeError::InvalidInput("AGENTIC_SESSION_NOT_FOUND".to_owned()))?;
        if session.project_id != project_id
            || session.candidate_id != candidate_id
            || session.current_stage != requested_stage
        {
            return Err(RuntimeError::InvalidInput(
                "DESIGN_STAGE_SCOPE_OR_STAGE_MISMATCH".to_owned(),
            ));
        }
        if !is_sha256(&session.observation_sha256)
            || session.observation_sha256 != observation_sha256
        {
            return Err(RuntimeError::InvalidInput(
                "AGENTIC_OBSERVATION_STALE: supplied observation is not the durable session observation"
                    .to_owned(),
            ));
        }

        let input_binding = json!({
            "project_id":project_id,
            "session_id":session_id,
            "candidate_id":candidate_id,
            "batch_id":batch_id,
            "requested_stage":requested_stage,
            "actions":actions,
            "observation_sha256":observation_sha256
        });
        let expected_input_sha256 = canonical_json_hash(&input_binding);
        let numeric_compatibility_input_sha256 =
            canonical_json_hash(&normalize_action_input_numbers(&input_binding));
        if input_sha256 != expected_input_sha256
            && input_sha256 != numeric_compatibility_input_sha256
        {
            return Err(RuntimeError::InvalidInput(format!(
                "DESIGN_STAGE_INPUT_HASH_MISMATCH: expected={expected_input_sha256} numeric_compatibility={numeric_compatibility_input_sha256} actual={input_sha256}"
            )));
        }

        // A terminal batch is a durable read, not a second execution.  The
        // result is stored in the Job checkpoint so a fresh Runtime process
        // can return the exact receipt without replaying child actions.
        if let Some(existing) = self.store.get_job_record(batch_id)? {
            if existing.project_id != project_id
                || existing.kind != JOB_KIND
                || existing.request_sha256 != input_sha256
            {
                return Err(RuntimeError::InvalidInput(
                    "DESIGN_STAGE_BATCH_IMMUTABLE_CONFLICT".to_owned(),
                ));
            }
            if is_terminal_job(&existing) {
                let checkpoint_sha256 = existing.checkpoint_sha256.as_deref().ok_or_else(|| {
                    RuntimeError::InvalidInput(
                        "DESIGN_STAGE_BATCH_CHECKPOINT_MISSING: terminal batch has no result checkpoint"
                            .to_owned(),
                    )
                })?;
                let result =
                    read_json_object(self, checkpoint_sha256, "design-action-batch-result")?;
                validate_stage_batch_result(
                    &result,
                    project_id,
                    session_id,
                    candidate_id,
                    batch_id,
                    observation_sha256,
                    input_sha256,
                )?;
                return Ok(result);
            }
        }

        self.session_get(json!({
            "session_id":session_id,
            "project_id":project_id,
            "candidate_id":candidate_id
        }))?;

        let started_at = now_string();
        let initial_job = JobRecord {
            schema_version: "RuntimeJob@1".to_owned(),
            job_id: batch_id.to_owned(),
            project_id: project_id.to_owned(),
            kind: JOB_KIND.to_owned(),
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
            job_id: batch_id.to_owned(),
            sequence: 1,
            kind: "design_stage_batch_started".to_owned(),
            payload: json!({
                "session_id":session_id,
                "candidate_id":candidate_id,
                "requested_stage":requested_stage,
                "action_count":actions.len(),
                "execution_mode":"independent-reviewable-actions"
            }),
            created_at: started_at,
        };
        let mut job =
            self.store
                .insert_job_with_event_if_absent(&initial_job, &started_event, &[])?;
        if job.project_id != project_id
            || job.kind != JOB_KIND
            || job.request_sha256 != input_sha256
        {
            return Err(RuntimeError::InvalidInput(
                "DESIGN_STAGE_BATCH_IMMUTABLE_CONFLICT".to_owned(),
            ));
        }

        let mut action_runs = Vec::new();
        let mut next_action_index = None;
        let mut terminal_quality_status = "not-run".to_owned();
        for (index, action_entry) in actions.iter().enumerate() {
            let action_object = action_entry
                .as_object()
                .expect("validate_batch_actions checked action object");
            let run_id = action_object["run_id"]
                .as_str()
                .expect("validate_batch_actions checked run_id");
            let action = action_object
                .get("action")
                .cloned()
                .expect("validate_batch_actions checked action");
            let mut action_input = json!({
                "project_id":project_id,
                "session_id":session_id,
                "candidate_id":candidate_id,
                "run_id":run_id,
                "action":action,
                "requested_stage":requested_stage,
                "observation_sha256":observation_sha256
            });
            if let Some(proposal) = action_object.get("proposal") {
                action_input["proposal"] = proposal.clone();
            }
            if let Some(optimization_intent) = action_object.get("optimization_intent") {
                action_input["optimization_intent"] = optimization_intent.clone();
            }
            // A high-level geometry action may omit a caller-authored
            // GeometryProgram and let DesignActionRun materialize its
            // RuntimeParameterPatch from typed parameter_changes.  The
            // external ReferenceViewSpec is part of that action's evidence
            // binding and must survive the batch boundary unchanged.
            if let Some(view_spec) = action_object.get("view_spec") {
                action_input["view_spec"] = view_spec.clone();
            }
            let action_input_sha256 = canonical_json_hash(&action_input);
            let mut action_request = json!({
                "project_id":project_id,
                "session_id":session_id,
                "candidate_id":candidate_id,
                "run_id":run_id,
                "action":action,
                "input_sha256":action_input_sha256,
                "requested_stage":requested_stage,
                "observation_sha256":observation_sha256,
                "approved":true,
                "approval_receipt_id":object.get("approval_receipt_id").cloned().unwrap_or(Value::Null),
                "approval_summary":object.get("approval_summary").cloned().unwrap_or(Value::Null),
                "approval_expires_at":object.get("approval_expires_at").cloned().unwrap_or(Value::Null),
                "approval_session_id":object.get("approval_session_id").cloned().unwrap_or(Value::String(session_id.to_owned())),
                "idempotency_key":format!("stage-action-{}-{index}", &action_input_sha256[..24])
            });
            if let Some(proposal) = action_object.get("proposal") {
                action_request["proposal"] = proposal.clone();
            }
            if let Some(optimization_intent) = action_object.get("optimization_intent") {
                action_request["optimization_intent"] = optimization_intent.clone();
            }
            if let Some(view_spec) = action_object.get("view_spec") {
                action_request["view_spec"] = view_spec.clone();
            }

            let action_run = match self.design_action_run_prepare(action_request) {
                Ok(run) => run,
                Err(error) => {
                    if !is_terminal_job(&job) {
                        job.status = "failed".to_owned();
                        job.progress = ((index * 100) / actions.len()) as u8;
                        job.error_code = Some("DESIGN_STAGE_ACTION_ERROR".to_owned());
                        job.updated_at = now_string();
                        let _ = self.store.update_job_with_event(
                            &job,
                            "design_stage_batch_failed",
                            &json!({"index":index,"run_id":run_id,"error":safe_error(&error.to_string())}),
                            &[],
                        )?;
                    }
                    return Err(error);
                }
            };
            terminal_quality_status = action_run
                .get("quality_status")
                .and_then(Value::as_str)
                .unwrap_or("unknown")
                .to_owned();
            let action_status = action_run
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or("blocked");
            let action_quality_passed = terminal_quality_status == "PARTIAL_VISIBLE_VIEW_PASS";
            let action_blocked = action_status != "completed" || !action_quality_passed;
            action_runs.push(action_run.clone());

            if !is_terminal_job(&job) {
                job.progress = (((index + 1) * 100) / actions.len()) as u8;
                job.updated_at = now_string();
                job = self.store.update_job_with_event(
                    &job,
                    "design_stage_action_completed",
                    &json!({
                        "index":index,
                        "run_id":run_id,
                        "action_run_sha256":action_run.get("canonical_sha256"),
                        "status":action_status,
                        "quality_status":terminal_quality_status,
                        "continued":!action_blocked
                    }),
                    &[],
                )?;
            }
            if action_blocked {
                next_action_index = Some(index);
                break;
            }
        }

        let blocked = next_action_index.is_some() || action_runs.len() != actions.len();
        let final_job_status = if blocked { "failed" } else { "succeeded" };
        let final_job_progress = if blocked {
            ((action_runs.len() * 100) / actions.len()) as u8
        } else {
            100
        };
        let mut result = json!({
            "schema_version":"DesignActionBatchResult@1",
            "batch_id":batch_id,
            "session_id":session_id,
            "project_id":project_id,
            "candidate_id":candidate_id,
            "requested_stage":requested_stage,
            "observation_sha256":observation_sha256,
            "input_sha256":input_sha256,
            "job_id":job.job_id,
            "job_status":final_job_status,
            "job_progress":final_job_progress,
            "status":if blocked {"blocked"} else {"completed"},
            "action_runs":action_runs,
            "completed_count":action_runs.len(),
            "next_action_index":next_action_index,
            "execution_mode":"independent-reviewable-actions",
            "proposal_promotion":{
                "status":"not-implemented",
                "confirm_allowed":false,
                "reason":"proposal candidates remain independent until an explicit promotion flow binds user approval, candidate confirmation, version creation and export gates"
            },
            "runtime_write":false,
            "persistent_user_data_touched":false,
            "canonical_sha256":""
        });
        result["canonical_sha256"] = Value::String(canonical_json_hash(&result));
        let result_bytes = canonical_json_bytes(&result)
            .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?;
        let result_object = self.put_object(
            &result_bytes,
            None,
            "application/json",
            "design-action-batch-result",
        )?;
        job.status = final_job_status.to_owned();
        job.progress = final_job_progress;
        job.error_code = blocked.then_some("DESIGN_STAGE_GATE_BLOCKED".to_owned());
        job.checkpoint_sha256 = Some(result_object.record.sha256.clone());
        job.updated_at = now_string();
        self.store.update_job_with_event(
            &job,
            if blocked {
                "design_stage_batch_blocked"
            } else {
                "design_stage_batch_succeeded"
            },
            &json!({
                "action_count":action_runs.len(),
                "requested_action_count":actions.len(),
                "next_action_index":next_action_index,
                "quality_status":terminal_quality_status,
                "result_sha256":result_object.record.sha256
            }),
            &[result_object.record.sha256],
        )?;
        Ok(result)
    }

    /// Prepare an explicit ordered composition proposal from multiple typed
    /// geometry actions. Each action first executes through the existing
    /// independent DesignActionRun path. When the optional `merge` envelope
    /// is present, it additionally proves a hash-linked cumulative
    /// GeometryProgram chain and compiles the final cumulative program into a
    /// distinct review candidate. It still never confirms, versions, exports,
    /// or mutates the source candidate.
    pub fn design_composition_prepare(&self, request: Value) -> Result<Value, RuntimeError> {
        let object = request_object(&request, "design_composition_prepare")?;
        reject_unknown_keys(
            object,
            &[
                "project_id",
                "session_id",
                "candidate_id",
                "composition_id",
                "requested_stage",
                "actions",
                "observation_sha256",
                "input_sha256",
                "approved",
                "approval_receipt_id",
                "approval_summary",
                "approval_expires_at",
                "approval_session_id",
                "idempotency_key",
                "merge",
            ],
        )?;
        require_approval(object)?;

        let project_id = required_id(object, "project_id")?;
        let session_id = required_id(object, "session_id")?;
        let candidate_id = required_id(object, "candidate_id")?;
        let composition_id = required_id(object, "composition_id")?;
        let requested_stage = required_stage(object, "requested_stage")?;
        let observation_sha256 = required_sha(object, "observation_sha256")?;
        let input_sha256 = required_sha(object, "input_sha256")?;
        let _idempotency_key = required_id(object, "idempotency_key")?;
        if object
            .get("approval_session_id")
            .and_then(Value::as_str)
            .is_some_and(|approval_session_id| approval_session_id != session_id)
        {
            return Err(RuntimeError::InvalidInput(
                "AGENTIC_APPROVAL_SESSION_MISMATCH".to_owned(),
            ));
        }
        let actions = object
            .get("actions")
            .and_then(Value::as_array)
            .ok_or_else(|| RuntimeError::InvalidInput("actions is required".to_owned()))?;
        validate_composition_actions(actions)?;

        let session = self
            .store
            .get_agentic_session(session_id)?
            .ok_or_else(|| RuntimeError::InvalidInput("AGENTIC_SESSION_NOT_FOUND".to_owned()))?;
        if session.project_id != project_id
            || session.candidate_id != candidate_id
            || session.current_stage != requested_stage
        {
            return Err(RuntimeError::InvalidInput(
                "DESIGN_COMPOSITION_SCOPE_OR_STAGE_MISMATCH".to_owned(),
            ));
        }
        if !is_sha256(&session.observation_sha256)
            || session.observation_sha256 != observation_sha256
        {
            return Err(RuntimeError::InvalidInput(
                "AGENTIC_OBSERVATION_STALE: supplied observation is not the durable session observation"
                    .to_owned(),
            ));
        }

        let mut input_binding = json!({
            "project_id":project_id,
            "session_id":session_id,
            "candidate_id":candidate_id,
            "composition_id":composition_id,
            "requested_stage":requested_stage,
            "actions":actions,
            "observation_sha256":observation_sha256
        });
        if let Some(merge) = object.get("merge") {
            input_binding["merge"] = merge.clone();
        }
        let expected_input_sha256 = canonical_json_hash(&input_binding);
        let numeric_compatibility_input_sha256 =
            canonical_json_hash(&normalize_action_input_numbers(&input_binding));
        if input_sha256 != expected_input_sha256
            && input_sha256 != numeric_compatibility_input_sha256
        {
            return Err(RuntimeError::InvalidInput(format!(
                "DESIGN_COMPOSITION_INPUT_HASH_MISMATCH: expected={expected_input_sha256} numeric_compatibility={numeric_compatibility_input_sha256} actual={input_sha256}"
            )));
        }

        // A terminal composition is served from its immutable result
        // checkpoint.  This makes the ordered orchestrator restart-safe and
        // prevents a second MCP call from recompiling child proposals.
        if let Some(existing) = self.store.get_job_record(composition_id)? {
            if existing.project_id != project_id
                || existing.kind != COMPOSITION_JOB_KIND
                || existing.request_sha256 != input_sha256
            {
                return Err(RuntimeError::InvalidInput(
                    "DESIGN_COMPOSITION_IMMUTABLE_CONFLICT".to_owned(),
                ));
            }
            if is_terminal_job(&existing) {
                let checkpoint_sha256 = existing.checkpoint_sha256.as_deref().ok_or_else(|| {
                    RuntimeError::InvalidInput(
                        "DESIGN_COMPOSITION_CHECKPOINT_MISSING: terminal composition has no result checkpoint"
                            .to_owned(),
                    )
                })?;
                let result =
                    read_json_object(self, checkpoint_sha256, "design-composition-result")?;
                validate_composition_result(
                    &result,
                    project_id,
                    session_id,
                    candidate_id,
                    composition_id,
                    observation_sha256,
                    input_sha256,
                )?;
                return Ok(result);
            }
        }

        self.session_get(json!({
            "session_id":session_id,
            "project_id":project_id,
            "candidate_id":candidate_id
        }))?;
        let merge_plan = object
            .get("merge")
            .map(|merge| validate_composition_merge(self, project_id, candidate_id, actions, merge))
            .transpose()?;
        let execution_mode = if merge_plan.is_some() {
            "ordered-independent-proposal-with-cumulative-merge"
        } else {
            "ordered-independent-proposal"
        };

        let started_at = now_string();
        let initial_job = JobRecord {
            schema_version: "RuntimeJob@1".to_owned(),
            job_id: composition_id.to_owned(),
            project_id: project_id.to_owned(),
            kind: COMPOSITION_JOB_KIND.to_owned(),
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
            job_id: composition_id.to_owned(),
            sequence: 1,
            kind: "design_composition_started".to_owned(),
            payload: json!({
                "session_id":session_id,
                "candidate_id":candidate_id,
                "requested_stage":requested_stage,
                "action_count":actions.len(),
                "execution_mode":execution_mode
            }),
            created_at: started_at,
        };
        let mut job =
            self.store
                .insert_job_with_event_if_absent(&initial_job, &started_event, &[])?;
        if job.project_id != project_id
            || job.kind != COMPOSITION_JOB_KIND
            || job.request_sha256 != input_sha256
        {
            return Err(RuntimeError::InvalidInput(
                "DESIGN_COMPOSITION_IMMUTABLE_CONFLICT".to_owned(),
            ));
        }

        // Reuse the proven stage-batch executor for the actual bounded action
        // receipts.  Dependencies stay in the composition hash and result;
        // the executor receives only the strict action payload it supports.
        let batch_id = format!("composition-batch-{}", &input_sha256[..32]);
        let batch_actions: Vec<Value> = actions
            .iter()
            .map(|entry| {
                let entry_object = entry
                    .as_object()
                    .expect("validate_composition_actions checked object");
                let mut action = Map::new();
                action.insert("run_id".to_owned(), entry_object["run_id"].clone());
                action.insert("action".to_owned(), entry_object["action"].clone());
                action.insert("proposal".to_owned(), entry_object["proposal"].clone());
                Value::Object(action)
            })
            .collect();
        let batch_input_sha256 = canonical_json_hash(&json!({
            "project_id":project_id,
            "session_id":session_id,
            "candidate_id":candidate_id,
            "batch_id":batch_id,
            "requested_stage":requested_stage,
            "actions":batch_actions,
            "observation_sha256":observation_sha256
        }));
        let batch_request = json!({
            "project_id":project_id,
            "session_id":session_id,
            "candidate_id":candidate_id,
            "batch_id":batch_id,
            "requested_stage":requested_stage,
            "actions":batch_actions,
            "observation_sha256":observation_sha256,
            "input_sha256":batch_input_sha256,
            "approved":true,
            "approval_receipt_id":object.get("approval_receipt_id").cloned().unwrap_or(Value::Null),
            "approval_summary":object.get("approval_summary").cloned().unwrap_or(Value::Null),
            "approval_expires_at":object.get("approval_expires_at").cloned().unwrap_or(Value::Null),
            "approval_session_id":object.get("approval_session_id").cloned().unwrap_or(Value::String(session_id.to_owned())),
            "idempotency_key":format!("composition-batch-{}", &input_sha256[..24])
        });
        let batch = self.design_stage_run_prepare(batch_request)?;
        let action_runs = batch
            .get("action_runs")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let mut steps = Vec::with_capacity(actions.len());
        let mut proposal_candidate_ids = Vec::new();
        let mut all_reviewable = true;
        let mut all_strict_improvement = true;
        let mut all_non_regressing = true;
        for (index, entry) in actions.iter().enumerate() {
            let entry_object = entry
                .as_object()
                .expect("validate_composition_actions checked object");
            let run_id = entry_object
                .get("run_id")
                .and_then(Value::as_str)
                .expect("run_id validated");
            let run = action_runs
                .iter()
                .find(|run| run.get("run_id").and_then(Value::as_str) == Some(run_id));
            let proposal = run
                .and_then(|run| run.get("proposal"))
                .filter(|value| value.is_object());
            let proposal_candidate_id = proposal
                .and_then(|value| value.get("candidate_id"))
                .and_then(Value::as_str);
            if let Some(proposal_candidate_id) = proposal_candidate_id {
                proposal_candidate_ids.push(proposal_candidate_id.to_owned());
            } else {
                all_reviewable = false;
            }
            all_strict_improvement &= proposal
                .and_then(|value| value.get("strict_improvement"))
                .and_then(Value::as_bool)
                .unwrap_or(false);
            all_non_regressing &= proposal
                .and_then(|value| value.get("non_regressing"))
                .and_then(Value::as_bool)
                .unwrap_or(false);
            steps.push(json!({
                "step_index":index,
                "run_id":run_id,
                "depends_on":entry_object["depends_on"].clone(),
                "status":run.and_then(|value| value.get("status")).cloned().unwrap_or_else(|| Value::String("not-run".to_owned())),
                "quality_status":run.and_then(|value| value.get("quality_status")).cloned().unwrap_or_else(|| Value::String("not-run".to_owned())),
                "action_run_sha256":run.and_then(|value| value.get("canonical_sha256")).cloned().unwrap_or(Value::Null),
                "proposal_candidate_id":proposal_candidate_id.map(|value| Value::String(value.to_owned())).unwrap_or(Value::Null),
                "proposal_candidate_state_sha256":proposal.and_then(|value| value.get("candidate_state_sha256")).cloned().unwrap_or(Value::Null)
            }));
        }
        let batch_blocked = batch.get("status").and_then(Value::as_str) != Some("completed")
            || action_runs.len() != actions.len()
            || !all_reviewable;
        let mut merge_result = Value::Null;
        let mut merge_blocked = false;
        if let Some(plan) = merge_plan.as_ref() {
            if batch_blocked {
                merge_blocked = true;
                merge_result = json!({
                    "status":"blocked",
                    "mode":"cumulative-program",
                    "steps":plan.step_records.clone(),
                    "final_step_index":plan.final_step_index,
                    "final_program_sha256":plan.final_program_sha256,
                    "merge_run_id":Value::Null,
                    "merge_run_sha256":Value::Null,
                    "merged_candidate_id":Value::Null,
                    "merged_candidate_state_sha256":Value::Null,
                    "quality_status":"QUALITY_TARGET_NOT_MET",
                    "strict_improvement":false,
                    "non_regressing":false,
                    "confirm_allowed":false,
                    "source_candidate_unchanged":true,
                    "reason":"ordered action batch stopped before the cumulative merge could be compiled"
                });
            } else {
                match self.design_composition_merge_execute(
                    object,
                    project_id,
                    session_id,
                    candidate_id,
                    composition_id,
                    requested_stage,
                    observation_sha256,
                    input_sha256,
                    plan,
                ) {
                    Ok(result) => {
                        merge_blocked =
                            result.get("status").and_then(Value::as_str) != Some("prepared");
                        merge_result = result;
                    }
                    Err(error) => {
                        merge_blocked = true;
                        merge_result = json!({
                            "status":"blocked",
                            "mode":"cumulative-program",
                            "steps":plan.step_records.clone(),
                            "final_step_index":plan.final_step_index,
                            "final_program_sha256":plan.final_program_sha256,
                            "merge_run_id":Value::Null,
                            "merge_run_sha256":Value::Null,
                            "merged_candidate_id":Value::Null,
                            "merged_candidate_state_sha256":Value::Null,
                            "quality_status":"QUALITY_TARGET_NOT_MET",
                            "strict_improvement":false,
                            "non_regressing":false,
                            "confirm_allowed":false,
                            "source_candidate_unchanged":true,
                            "reason":format!("cumulative merge execution failed closed: {}", safe_error(&error.to_string()))
                        });
                    }
                }
            }
        }
        let blocked = batch_blocked || merge_blocked;
        let composition_status = if blocked { "blocked" } else { "reviewable" };
        let merge_quality_status = merge_result.get("quality_status").and_then(Value::as_str);
        let quality_status = if let Some(merge_quality_status) = merge_quality_status {
            merge_quality_status
        } else if !blocked && all_strict_improvement {
            "PARTIAL_VISIBLE_VIEW_PASS"
        } else {
            "QUALITY_TARGET_NOT_MET"
        };
        let strict_improvement = merge_result
            .get("strict_improvement")
            .and_then(Value::as_bool)
            .map(|value| value && !blocked)
            .unwrap_or(all_strict_improvement && !blocked);
        let non_regressing = merge_result
            .get("non_regressing")
            .and_then(Value::as_bool)
            .map(|value| value && !blocked)
            .unwrap_or(all_non_regressing && !blocked);
        let final_job_status = if blocked { "failed" } else { "succeeded" };
        let final_job_progress = if blocked {
            batch
                .get("job_progress")
                .and_then(Value::as_u64)
                .unwrap_or(0) as u8
        } else {
            100
        };
        let final_error_code = if batch_blocked {
            Some("DESIGN_COMPOSITION_PROPOSAL_BLOCKED".to_owned())
        } else if merge_blocked {
            Some("DESIGN_COMPOSITION_MERGE_BLOCKED".to_owned())
        } else {
            None
        };
        let mut result = json!({
            "schema_version":"DesignCompositionResult@1",
            "composition_id":composition_id,
            "session_id":session_id,
            "project_id":project_id,
            "candidate_id":candidate_id,
            "requested_stage":requested_stage,
            "observation_sha256":observation_sha256,
            "input_sha256":input_sha256,
            "job_id":job.job_id,
            "job_status":final_job_status,
            "job_progress":final_job_progress,
            "status":composition_status,
            "execution_mode":execution_mode,
            "steps":steps,
            "action_runs":action_runs,
            "completed_count":action_runs.len(),
            "next_action_index":batch.get("next_action_index").cloned().unwrap_or(Value::Null),
            "aggregate":{
                "quality_status":quality_status,
                "strict_improvement":strict_improvement,
                "non_regressing":non_regressing,
                "proposal_candidate_ids":proposal_candidate_ids.clone()
            },
            "composition_proposal":{
                "status":if blocked {"blocked"} else {"reviewable"},
                "source_candidate_id":candidate_id,
                "proposal_candidate_ids":proposal_candidate_ids.clone(),
                "merged_candidate_id":merge_result.get("merged_candidate_id").cloned().unwrap_or(Value::Null),
                "confirm_allowed":false,
                "merge_status":merge_result.get("status").and_then(Value::as_str).map(|status| status.to_owned()).unwrap_or_else(|| "not-requested".to_owned()),
                "reason":if merge_plan.is_some() { "cumulative merge is a separate review-candidate prepare; Repair application and candidate promotion still require an explicit transaction" } else { "action proposals remain independently reviewable; an explicit cumulative merge envelope is required before a merged candidate is compiled" }
            },
            "merge":merge_result,
            "failure_recovery":{
                "stopped_at_index":batch.get("next_action_index").cloned().unwrap_or(Value::Null),
                "retry_input_sha256":input_sha256,
                "source_candidate_unchanged":true,
                "replayable":true
            },
            "runtime_write":false,
            "persistent_user_data_touched":false,
            "canonical_sha256":""
        });
        result["canonical_sha256"] = Value::String(canonical_json_hash(&result));
        let result_bytes = canonical_json_bytes(&result)
            .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?;
        let result_object = self.put_object(
            &result_bytes,
            None,
            "application/json",
            "design-composition-result",
        )?;
        job.status = final_job_status.to_owned();
        job.progress = final_job_progress;
        job.error_code = final_error_code;
        job.checkpoint_sha256 = Some(result_object.record.sha256.clone());
        job.updated_at = now_string();
        self.store.update_job_with_event(
            &job,
            if blocked {
                "design_composition_blocked"
            } else {
                "design_composition_reviewable"
            },
            &json!({
                "action_count":action_runs.len(),
                "requested_action_count":actions.len(),
                "proposal_candidate_ids":proposal_candidate_ids.clone(),
                "quality_status":quality_status,
                "merge_status":merge_result.get("status").cloned().unwrap_or(Value::Null),
                "merged_candidate_id":merge_result.get("merged_candidate_id").cloned().unwrap_or(Value::Null),
                "result_sha256":result_object.record.sha256
            }),
            &[result_object.record.sha256],
        )?;
        Ok(result)
    }

    fn design_composition_merge_execute(
        &self,
        request: &Map<String, Value>,
        project_id: &str,
        session_id: &str,
        candidate_id: &str,
        composition_id: &str,
        requested_stage: &str,
        observation_sha256: &str,
        input_sha256: &str,
        plan: &CompositionMergePlan,
    ) -> Result<Value, RuntimeError> {
        let run_id = format!("composition-merge-{}", &input_sha256[..32]);
        let mut action = plan.final_action.as_object().cloned().ok_or_else(|| {
            RuntimeError::InvalidInput("COMPOSITION_MERGE_ACTION_INVALID".to_owned())
        })?;
        action.insert(
            "action_id".to_owned(),
            Value::String(format!("composition-merge-action-{}", &input_sha256[..20])),
        );
        action.insert(
            "description".to_owned(),
            Value::String(
                "compile cumulative composition GeometryProgram into one review candidate"
                    .to_owned(),
            ),
        );
        let action = Value::Object(action);
        let action_input = json!({
            "project_id":project_id,
            "session_id":session_id,
            "candidate_id":candidate_id,
            "run_id":run_id,
            "action":action,
            "requested_stage":requested_stage,
            "observation_sha256":observation_sha256,
            "proposal":plan.final_proposal.clone()
        });
        let action_input_sha256 = canonical_json_hash(&action_input);
        let merge_run = self.design_action_run_prepare(json!({
            "project_id":project_id,
            "session_id":session_id,
            "candidate_id":candidate_id,
            "run_id":run_id,
            "action":action,
            "input_sha256":action_input_sha256,
            "requested_stage":requested_stage,
            "observation_sha256":observation_sha256,
            "approved":true,
            "approval_receipt_id":request.get("approval_receipt_id").cloned().unwrap_or(Value::Null),
            "approval_summary":format!("Compile cumulative composition {composition_id} into a review candidate"),
            "approval_expires_at":request.get("approval_expires_at").cloned().unwrap_or(Value::Null),
            "approval_session_id":request.get("approval_session_id").cloned().unwrap_or(Value::String(session_id.to_owned())),
            "idempotency_key":format!("composition-merge-{}", &input_sha256[..24]),
            "proposal":plan.final_proposal.clone()
        }))?;
        let proposal = merge_run.get("proposal").filter(|value| value.is_object());
        let merged_candidate_id = proposal
            .and_then(|value| value.get("candidate_id"))
            .cloned()
            .unwrap_or(Value::Null);
        let merged_candidate_state_sha256 = proposal
            .and_then(|value| value.get("candidate_state_sha256"))
            .cloned()
            .unwrap_or(Value::Null);
        let status = if merged_candidate_id.is_string() {
            "prepared"
        } else {
            "blocked"
        };
        Ok(json!({
            "status":status,
            "mode":"cumulative-program",
            "steps":plan.step_records.clone(),
            "final_step_index":plan.final_step_index,
            "final_program_sha256":plan.final_program_sha256,
            "merge_run_id":merge_run.get("run_id").cloned().unwrap_or(Value::String(run_id)),
            "merge_run_sha256":merge_run.get("canonical_sha256").cloned().unwrap_or(Value::Null),
            "merged_candidate_id":merged_candidate_id,
            "merged_candidate_state_sha256":merged_candidate_state_sha256,
            "quality_status":merge_run.get("quality_status").cloned().unwrap_or(Value::String("QUALITY_TARGET_NOT_MET".to_owned())),
            "strict_improvement":proposal.and_then(|value| value.get("strict_improvement")).and_then(Value::as_bool).unwrap_or(false),
            "non_regressing":proposal.and_then(|value| value.get("non_regressing")).and_then(Value::as_bool).unwrap_or(false),
            "confirm_allowed":false,
            "source_candidate_unchanged":true,
            "reason":if status == "prepared" { "cumulative GeometryProgram compiled into a distinct review candidate; confirmation remains locked" } else { "cumulative merge DesignActionRun stopped before a review candidate was produced" }
        }))
    }
}

fn validate_batch_actions(actions: &[Value]) -> Result<(), RuntimeError> {
    if actions.is_empty() || actions.len() > MAX_BATCH_ACTIONS {
        return Err(RuntimeError::InvalidInput(
            "DESIGN_STAGE_ACTION_COUNT_OUT_OF_BOUNDS".to_owned(),
        ));
    }
    let mut run_ids = HashSet::new();
    for entry in actions {
        let object = entry.as_object().ok_or_else(|| {
            RuntimeError::InvalidInput("DESIGN_STAGE_ACTION_ENTRY_INVALID".to_owned())
        })?;
        reject_unknown_keys(
            object,
            &[
                "run_id",
                "action",
                "proposal",
                "optimization_intent",
                "view_spec",
            ],
        )?;
        let run_id = required_id(object, "run_id")?;
        if !run_ids.insert(run_id.to_owned()) {
            return Err(RuntimeError::InvalidInput(
                "DESIGN_STAGE_ACTION_RUN_ID_DUPLICATE".to_owned(),
            ));
        }
        let action = object
            .get("action")
            .and_then(Value::as_object)
            .ok_or_else(|| RuntimeError::InvalidInput("DESIGN_STAGE_ACTION_REQUIRED".to_owned()))?;
        let action_kind = action
            .get("action_kind")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                RuntimeError::InvalidInput("DESIGN_STAGE_ACTION_KIND_REQUIRED".to_owned())
            })?;
        if !BATCH_ACTION_KINDS.contains(&action_kind) {
            return Err(RuntimeError::InvalidInput(
                "DESIGN_STAGE_ACTION_KIND_UNSUPPORTED".to_owned(),
            ));
        }
        if object
            .get("proposal")
            .is_some_and(|proposal| !proposal.is_object())
        {
            return Err(RuntimeError::InvalidInput(
                "DESIGN_STAGE_PROPOSAL_INVALID".to_owned(),
            ));
        }
        if object
            .get("optimization_intent")
            .is_some_and(|intent| !intent.is_object())
        {
            return Err(RuntimeError::InvalidInput(
                "DESIGN_STAGE_OPTIMIZATION_INTENT_INVALID".to_owned(),
            ));
        }
        if object
            .get("view_spec")
            .is_some_and(|view_spec| !view_spec.is_object())
        {
            return Err(RuntimeError::InvalidInput(
                "DESIGN_STAGE_VIEW_SPEC_INVALID".to_owned(),
            ));
        }
        if object.get("optimization_intent").is_some() && object.get("proposal").is_some() {
            return Err(RuntimeError::InvalidInput(
                "DESIGN_STAGE_OPTIMIZATION_AND_PROPOSAL_ARE_EXCLUSIVE".to_owned(),
            ));
        }
    }
    Ok(())
}

fn validate_composition_actions(actions: &[Value]) -> Result<(), RuntimeError> {
    if !(2..=MAX_COMPOSITION_ACTIONS).contains(&actions.len()) {
        return Err(RuntimeError::InvalidInput(
            "DESIGN_COMPOSITION_ACTION_COUNT_OUT_OF_BOUNDS".to_owned(),
        ));
    }
    let mut run_ids = HashSet::new();
    for (index, entry) in actions.iter().enumerate() {
        let object = entry.as_object().ok_or_else(|| {
            RuntimeError::InvalidInput("DESIGN_COMPOSITION_ACTION_ENTRY_INVALID".to_owned())
        })?;
        reject_unknown_keys(object, &["run_id", "depends_on", "action", "proposal"])?;
        let run_id = required_id(object, "run_id")?;
        if !run_ids.insert(run_id.to_owned()) {
            return Err(RuntimeError::InvalidInput(
                "DESIGN_COMPOSITION_ACTION_RUN_ID_DUPLICATE".to_owned(),
            ));
        }
        let dependencies = object
            .get("depends_on")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                RuntimeError::InvalidInput("DESIGN_COMPOSITION_DEPENDENCY_REQUIRED".to_owned())
            })?;
        if (index == 0 && !dependencies.is_empty())
            || (index > 0
                && (dependencies.len() != 1
                    || dependencies[0].as_str()
                        != actions[index - 1].get("run_id").and_then(Value::as_str)))
        {
            return Err(RuntimeError::InvalidInput(
                "DESIGN_COMPOSITION_DEPENDENCY_MUST_BE_PREVIOUS_ACTION".to_owned(),
            ));
        }
        let action = object
            .get("action")
            .and_then(Value::as_object)
            .ok_or_else(|| {
                RuntimeError::InvalidInput("DESIGN_COMPOSITION_ACTION_REQUIRED".to_owned())
            })?;
        let action_kind = action
            .get("action_kind")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                RuntimeError::InvalidInput("DESIGN_COMPOSITION_ACTION_KIND_REQUIRED".to_owned())
            })?;
        if !COMPOSITION_ACTION_KINDS.contains(&action_kind) {
            return Err(RuntimeError::InvalidInput(
                "DESIGN_COMPOSITION_ACTION_KIND_UNSUPPORTED".to_owned(),
            ));
        }
        if !object.get("proposal").is_some_and(Value::is_object) {
            return Err(RuntimeError::InvalidInput(
                "DESIGN_COMPOSITION_PROPOSAL_REQUIRED".to_owned(),
            ));
        }
    }
    Ok(())
}

/// Validate the explicit cumulative-program merge envelope before any job or
/// candidate write. The individual action proposals are complete programs,
/// not mesh deltas; the parent hash chain is therefore the only accepted
/// composition semantics until a typed delta/patch contract exists. The last
/// program is compiled again as one distinct review candidate after the
/// ordered action batch succeeds.
fn validate_composition_merge(
    runtime: &Runtime,
    project_id: &str,
    candidate_id: &str,
    actions: &[Value],
    merge: &Value,
) -> Result<CompositionMergePlan, RuntimeError> {
    let merge_object = merge.as_object().ok_or_else(|| {
        RuntimeError::InvalidInput(
            "DESIGN_COMPOSITION_MERGE_INVALID: merge must be an object".to_owned(),
        )
    })?;
    reject_unknown_keys(merge_object, &["mode", "steps", "final_step_index"])?;
    if merge_object.get("mode").and_then(Value::as_str) != Some("cumulative-program") {
        return Err(RuntimeError::InvalidInput(
            "DESIGN_COMPOSITION_MERGE_MODE_UNSUPPORTED".to_owned(),
        ));
    }
    let merge_steps = merge_object
        .get("steps")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            RuntimeError::InvalidInput("DESIGN_COMPOSITION_MERGE_STEPS_REQUIRED".to_owned())
        })?;
    if merge_steps.len() != actions.len() {
        return Err(RuntimeError::InvalidInput(
            "DESIGN_COMPOSITION_MERGE_STEPS_MUST_MATCH_ACTIONS".to_owned(),
        ));
    }
    let final_step_index = merge_object
        .get("final_step_index")
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            RuntimeError::InvalidInput("DESIGN_COMPOSITION_MERGE_FINAL_STEP_REQUIRED".to_owned())
        })? as usize;
    if final_step_index != actions.len() - 1 {
        return Err(RuntimeError::InvalidInput(
            "DESIGN_COMPOSITION_MERGE_FINAL_STEP_MUST_BE_LAST".to_owned(),
        ));
    }
    let source_evidence = runtime
        .store
        .get_geometry_candidate_evidence(candidate_id)?
        .ok_or_else(|| {
            RuntimeError::InvalidInput(
                "DESIGN_COMPOSITION_MERGE_SOURCE_V2_EVIDENCE_REQUIRED".to_owned(),
            )
        })?;
    if source_evidence.project_id != project_id
        || !is_sha256(&source_evidence.geometry_program_sha256)
    {
        return Err(RuntimeError::InvalidInput(
            "DESIGN_COMPOSITION_MERGE_SOURCE_EVIDENCE_SCOPE_INVALID".to_owned(),
        ));
    }

    let mut expected_parent_program_sha256 = source_evidence.geometry_program_sha256.clone();
    let mut step_records = Vec::with_capacity(actions.len());
    let mut final_action = Value::Null;
    let mut final_proposal = Value::Null;
    let mut final_program_sha256 = String::new();
    for (index, (action_entry, merge_step)) in actions.iter().zip(merge_steps).enumerate() {
        let action_object = action_entry
            .as_object()
            .expect("validate_composition_actions checked action entry");
        let run_id = action_object
            .get("run_id")
            .and_then(Value::as_str)
            .expect("validate_composition_actions checked run_id");
        let step_object = merge_step.as_object().ok_or_else(|| {
            RuntimeError::InvalidInput("DESIGN_COMPOSITION_MERGE_STEP_INVALID".to_owned())
        })?;
        reject_unknown_keys(
            step_object,
            &["run_id", "parent_program_sha256", "program_sha256"],
        )?;
        if step_object.get("run_id").and_then(Value::as_str) != Some(run_id) {
            return Err(RuntimeError::InvalidInput(
                "DESIGN_COMPOSITION_MERGE_RUN_ORDER_MISMATCH".to_owned(),
            ));
        }
        let parent_program_sha256 = required_sha(step_object, "parent_program_sha256")?;
        let program_sha256 = required_sha(step_object, "program_sha256")?;
        if parent_program_sha256 != expected_parent_program_sha256 {
            return Err(RuntimeError::InvalidInput(format!(
                "DESIGN_COMPOSITION_MERGE_PARENT_HASH_MISMATCH at step {index}"
            )));
        }
        let proposal = action_object
            .get("proposal")
            .and_then(Value::as_object)
            .ok_or_else(|| {
                RuntimeError::InvalidInput("DESIGN_COMPOSITION_PROPOSAL_REQUIRED".to_owned())
            })?;
        let program = proposal.get("geometry_program").ok_or_else(|| {
            RuntimeError::InvalidInput("DESIGN_COMPOSITION_MERGE_PROGRAM_REQUIRED".to_owned())
        })?;
        if program.get("schema_version").and_then(Value::as_str) != Some("GeometryProgram@2")
            || program.get("project_id").and_then(Value::as_str) != Some(project_id)
        {
            return Err(RuntimeError::InvalidInput(
                "DESIGN_COMPOSITION_MERGE_PROGRAM_SCOPE_INVALID".to_owned(),
            ));
        }
        let program_object = program.as_object().ok_or_else(|| {
            RuntimeError::InvalidInput("DESIGN_COMPOSITION_MERGE_PROGRAM_INVALID".to_owned())
        })?;
        let declared_program_sha256 = program_object
            .get("canonical_sha256")
            .and_then(Value::as_str)
            .filter(|value| is_sha256(value))
            .ok_or_else(|| {
                RuntimeError::InvalidInput(
                    "DESIGN_COMPOSITION_MERGE_PROGRAM_HASH_REQUIRED".to_owned(),
                )
            })?;
        let mut draft = program.clone();
        draft
            .as_object_mut()
            .expect("program object checked above")
            .remove("canonical_sha256");
        let computed_program_sha256 = hash_geometry_program_with_runtime_worker(&draft)
            .map_err(|error| {
                RuntimeError::InvalidInput(format!(
                    "DESIGN_COMPOSITION_MERGE_PROGRAM_REJECTED: {error}"
                ))
            })?
            .get("canonical_sha256")
            .and_then(Value::as_str)
            .filter(|value| is_sha256(value))
            .ok_or_else(|| {
                RuntimeError::InvalidInput(
                    "DESIGN_COMPOSITION_MERGE_PROGRAM_HASH_UNAVAILABLE".to_owned(),
                )
            })?
            .to_owned();
        if declared_program_sha256 != computed_program_sha256
            || program_sha256 != computed_program_sha256
        {
            return Err(RuntimeError::InvalidInput(format!(
                "DESIGN_COMPOSITION_MERGE_PROGRAM_HASH_MISMATCH at step {index}"
            )));
        }
        if program_sha256 == parent_program_sha256 {
            return Err(RuntimeError::InvalidInput(format!(
                "DESIGN_COMPOSITION_MERGE_NOOP_STEP at step {index}"
            )));
        }
        step_records.push(json!({
            "step_index":index,
            "run_id":run_id,
            "parent_program_sha256":parent_program_sha256,
            "program_sha256":program_sha256
        }));
        expected_parent_program_sha256 = program_sha256.to_owned();
        final_program_sha256 = program_sha256.to_owned();
        if index == final_step_index {
            final_action = action_entry.get("action").cloned().unwrap_or(Value::Null);
            final_proposal = action_entry.get("proposal").cloned().unwrap_or(Value::Null);
        }
    }

    Ok(CompositionMergePlan {
        step_records,
        final_step_index,
        final_action,
        final_proposal,
        final_program_sha256,
    })
}

fn request_object<'a>(
    request: &'a Value,
    operation: &str,
) -> Result<&'a Map<String, Value>, RuntimeError> {
    request.as_object().ok_or_else(|| {
        RuntimeError::InvalidInput(format!(
            "DESIGN_STAGE_INVALID_INPUT: {operation} requires an object"
        ))
    })
}

fn reject_unknown_keys(object: &Map<String, Value>, allowed: &[&str]) -> Result<(), RuntimeError> {
    if let Some(key) = object.keys().find(|key| !allowed.contains(&key.as_str())) {
        return Err(RuntimeError::InvalidInput(format!(
            "DESIGN_STAGE_INVALID_INPUT: unsupported field {key}"
        )));
    }
    Ok(())
}

fn required_id<'a>(object: &'a Map<String, Value>, key: &str) -> Result<&'a str, RuntimeError> {
    let value = object
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| RuntimeError::InvalidInput(format!("{key} is required")))?;
    if !is_opaque_id(value) {
        return Err(RuntimeError::InvalidInput(format!("{key} is malformed")));
    }
    Ok(value)
}

fn required_sha<'a>(object: &'a Map<String, Value>, key: &str) -> Result<&'a str, RuntimeError> {
    let value = required_id_like(object, key)?;
    if !is_sha256(value) {
        return Err(RuntimeError::InvalidInput(format!(
            "{key} is not a SHA-256"
        )));
    }
    Ok(value)
}

fn required_id_like<'a>(
    object: &'a Map<String, Value>,
    key: &str,
) -> Result<&'a str, RuntimeError> {
    object
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| RuntimeError::InvalidInput(format!("{key} is required")))
}

fn required_stage<'a>(object: &'a Map<String, Value>, key: &str) -> Result<&'a str, RuntimeError> {
    let value = required_id(object, key)?;
    if ![
        "reference-canvas",
        "primary-form",
        "secondary-structure",
        "tertiary-detail",
        "uv-pbr",
        "final-review",
    ]
    .contains(&value)
    {
        return Err(RuntimeError::InvalidInput(format!(
            "{key} is not a valid DesignStage"
        )));
    }
    Ok(value)
}

fn require_approval(object: &Map<String, Value>) -> Result<(), RuntimeError> {
    if object.get("approved") != Some(&Value::Bool(true)) {
        return Err(RuntimeError::InvalidInput(
            "AGENTIC_APPROVAL_REQUIRED: approved=true is required".to_owned(),
        ));
    }
    for key in ["approval_receipt_id", "approval_summary"] {
        if object
            .get(key)
            .and_then(Value::as_str)
            .is_none_or(|value| value.is_empty())
        {
            return Err(RuntimeError::InvalidInput(format!(
                "AGENTIC_APPROVAL_REQUIRED: {key} is required"
            )));
        }
    }
    Ok(())
}

fn is_terminal_job(job: &JobRecord) -> bool {
    matches!(job.status.as_str(), "succeeded" | "failed" | "cancelled")
}

fn safe_error(error: &str) -> String {
    error
        .chars()
        .filter(|character| !character.is_control())
        .take(256)
        .collect()
}

/// Keep the action-batch envelope compatible with clients whose JSON float
/// serializer chooses a different shortest decimal representation. Typed
/// GeometryProgram hashes remain strict; only this bounded request digest is
/// normalized to the same twelve-decimal precision used by DesignActionRun.
fn normalize_action_input_numbers(value: &Value) -> Value {
    const DECIMAL_SCALE: f64 = 1_000_000_000_000.0;
    match value {
        Value::Number(number) => number
            .as_f64()
            .filter(|value| value.is_finite())
            .and_then(|value| {
                serde_json::Number::from_f64((value * DECIMAL_SCALE).round() / DECIMAL_SCALE)
            })
            .map(Value::Number)
            .unwrap_or_else(|| value.clone()),
        Value::Array(values) => {
            Value::Array(values.iter().map(normalize_action_input_numbers).collect())
        }
        Value::Object(object) => Value::Object(
            object
                .iter()
                .map(|(key, child)| (key.clone(), normalize_action_input_numbers(child)))
                .collect(),
        ),
        _ => value.clone(),
    }
}

fn read_json_object(runtime: &Runtime, sha256: &str, kind: &str) -> Result<Value, RuntimeError> {
    if !is_sha256(sha256) {
        return Err(RuntimeError::InvalidInput(format!(
            "{kind} checkpoint hash is invalid"
        )));
    }
    let metadata = runtime.store.get_object(sha256)?.ok_or_else(|| {
        RuntimeError::InvalidInput(format!("{kind} checkpoint object is unavailable"))
    })?;
    if metadata.mime != "application/json" || metadata.kind != kind {
        return Err(RuntimeError::InvalidInput(format!(
            "{kind} checkpoint metadata is invalid"
        )));
    }
    let value: Value = serde_json::from_slice(&runtime.cas_read(sha256)?).map_err(|error| {
        RuntimeError::InvalidInput(format!("{kind} checkpoint JSON is invalid: {error}"))
    })?;
    if !value.is_object() {
        return Err(RuntimeError::InvalidInput(format!(
            "{kind} checkpoint must be an object"
        )));
    }
    Ok(value)
}

fn validate_stage_batch_result(
    result: &Value,
    project_id: &str,
    session_id: &str,
    candidate_id: &str,
    batch_id: &str,
    observation_sha256: &str,
    input_sha256: &str,
) -> Result<(), RuntimeError> {
    let object = result.as_object().ok_or_else(|| {
        RuntimeError::InvalidInput(
            "DESIGN_STAGE_BATCH_CHECKPOINT_INVALID: result must be an object".to_owned(),
        )
    })?;
    if object.get("schema_version").and_then(Value::as_str) != Some("DesignActionBatchResult@1")
        || object.get("project_id").and_then(Value::as_str) != Some(project_id)
        || object.get("session_id").and_then(Value::as_str) != Some(session_id)
        || object.get("candidate_id").and_then(Value::as_str) != Some(candidate_id)
        || object.get("batch_id").and_then(Value::as_str) != Some(batch_id)
        || object.get("observation_sha256").and_then(Value::as_str) != Some(observation_sha256)
        || object.get("job_id").and_then(Value::as_str) != Some(batch_id)
        || object.get("input_sha256").and_then(Value::as_str) != Some(input_sha256)
    {
        return Err(RuntimeError::InvalidInput(
            "DESIGN_STAGE_BATCH_CHECKPOINT_SCOPE_MISMATCH".to_owned(),
        ));
    }
    let declared_hash = object
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .ok_or_else(|| {
            RuntimeError::InvalidInput(
                "DESIGN_STAGE_BATCH_CHECKPOINT_CANONICAL_HASH_MISSING".to_owned(),
            )
        })?;
    let mut draft = result.clone();
    draft["canonical_sha256"] = Value::String(String::new());
    if canonical_json_hash(&draft) != declared_hash {
        return Err(RuntimeError::InvalidInput(
            "DESIGN_STAGE_BATCH_CHECKPOINT_CANONICAL_HASH_MISMATCH".to_owned(),
        ));
    }
    if !matches!(
        object.get("job_status").and_then(Value::as_str),
        Some("succeeded") | Some("failed")
    ) {
        return Err(RuntimeError::InvalidInput(
            "DESIGN_STAGE_BATCH_CHECKPOINT_STATUS_INVALID".to_owned(),
        ));
    }
    Ok(())
}

fn validate_composition_result(
    result: &Value,
    project_id: &str,
    session_id: &str,
    candidate_id: &str,
    composition_id: &str,
    observation_sha256: &str,
    input_sha256: &str,
) -> Result<(), RuntimeError> {
    let object = result.as_object().ok_or_else(|| {
        RuntimeError::InvalidInput(
            "DESIGN_COMPOSITION_CHECKPOINT_INVALID: result must be an object".to_owned(),
        )
    })?;
    if object.get("schema_version").and_then(Value::as_str) != Some("DesignCompositionResult@1")
        || object.get("project_id").and_then(Value::as_str) != Some(project_id)
        || object.get("session_id").and_then(Value::as_str) != Some(session_id)
        || object.get("candidate_id").and_then(Value::as_str) != Some(candidate_id)
        || object.get("composition_id").and_then(Value::as_str) != Some(composition_id)
        || object.get("observation_sha256").and_then(Value::as_str) != Some(observation_sha256)
        || object.get("job_id").and_then(Value::as_str) != Some(composition_id)
        || object.get("input_sha256").and_then(Value::as_str) != Some(input_sha256)
    {
        return Err(RuntimeError::InvalidInput(
            "DESIGN_COMPOSITION_CHECKPOINT_SCOPE_MISMATCH".to_owned(),
        ));
    }
    let declared_hash = object
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .ok_or_else(|| {
            RuntimeError::InvalidInput(
                "DESIGN_COMPOSITION_CHECKPOINT_CANONICAL_HASH_MISSING".to_owned(),
            )
        })?;
    let mut draft = result.clone();
    draft["canonical_sha256"] = Value::String(String::new());
    if canonical_json_hash(&draft) != declared_hash {
        return Err(RuntimeError::InvalidInput(
            "DESIGN_COMPOSITION_CHECKPOINT_CANONICAL_HASH_MISMATCH".to_owned(),
        ));
    }
    if !matches!(
        object.get("job_status").and_then(Value::as_str),
        Some("succeeded") | Some("failed")
    ) {
        return Err(RuntimeError::InvalidInput(
            "DESIGN_COMPOSITION_CHECKPOINT_STATUS_INVALID".to_owned(),
        ));
    }
    Ok(())
}
