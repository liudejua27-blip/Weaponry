//! Runtime-owned execution for one bounded DesignActionRun.
//!
//! This is deliberately a small, evidence-bound P0 loop.  The action payload
//! is recorded as intent, while the execution revalidates the current
//! candidate's existing typed program, GLB, visual evidence and quality
//! report.  A geometry action may ask Runtime to materialize one bounded
//! RuntimeParameterPatch from its typed `parameter_changes`; the resulting
//! proposal is still isolated, review-only and never changes the source
//! candidate.  It does not create a version, confirm, or export anything.  A
//! failed stage is still persisted as an immutable, fail-closed run so Codex
//! can inspect the exact stopping point.

use super::production_weapon_assembly_parameter_mutator::production_weapon_stock_profile_reconstruction_mutate;
use super::{
    canonical_json_bytes, canonical_json_hash, hash_geometry_program_with_runtime_worker,
    now_string, sha256_hex, strict_glb_inspection, validate_quality_report_v2_output,
    validate_reference_comparison_report, validate_render_set_v2_output, validate_worker_metadata,
    Runtime, RuntimeError,
};
use forgecad_contracts::{
    is_opaque_id, is_sha256, CandidateConfirmRequest, CandidateRecord,
    GeometryCandidateEvidenceRecord, JobEventRecord, JobRecord,
};
use forgecad_store::AgenticSessionRecord;
use serde_json::{json, Map, Value};
use std::collections::{HashMap, HashSet};

const DESIGN_STAGES: [&str; 6] = [
    "reference-canvas",
    "primary-form",
    "secondary-structure",
    "tertiary-detail",
    "uv-pbr",
    "final-review",
];

const PIPELINE_STAGES: [&str; 5] = ["prepare", "compile", "readback", "render", "evaluate"];
const REAL_D1_REPAIR_SIX_VIEW_KINDS: [&str; 6] = [
    "front",
    "back",
    "left",
    "right",
    "top",
    "rear-three-quarter",
];

const ACTION_KINDS: [&str; 16] = [
    "reference-import",
    "coverage-annotation",
    "mark-unknown",
    "primary-blockout",
    "primary-form-adjustment",
    "secondary-structure",
    "tertiary-detail",
    "material-zone",
    "final-review",
    "request-reference",
    "bounded-repair",
    "checkpoint",
    "rollback",
    "human-review",
    "next-stage",
    "uv-pbr",
];

const OPERATOR_IDS: [&str; 27] = [
    "forgecad.geometry.primitive@2",
    "forgecad.geometry.profile-extrude@1",
    "forgecad.geometry.profile-loft@1",
    "forgecad.geometry.profile-loft@2",
    "forgecad.geometry.multi-loop-profile-loft@1",
    "forgecad.geometry.longitudinal-section-loft@1",
    "forgecad.geometry.subd-cage@1",
    "forgecad.geometry.subd-cage@2",
    "forgecad.geometry.authoring-mesh@1",
    "forgecad.geometry.surface-patch@1",
    "forgecad.geometry.surface-shell@1",
    "forgecad.geometry.revolve@1",
    "forgecad.geometry.tube-sweep@1",
    "forgecad.geometry.transform@2",
    "forgecad.geometry.mirror@1",
    "forgecad.geometry.array@1",
    "forgecad.geometry.bevel@1",
    "forgecad.geometry.normal-policy@1",
    "forgecad.geometry.panel@1",
    "forgecad.geometry.panel@2",
    "forgecad.geometry.vent-array@1",
    "forgecad.geometry.vent-array@2",
    "forgecad.geometry.recessed-channel@1",
    "forgecad.geometry.energy-core@1",
    "forgecad.geometry.joint-stack@1",
    "forgecad.geometry.boolean@1",
    "forgecad.geometry.part-output@1",
];

#[derive(Debug, Clone)]
pub(crate) struct GeometryBindings {
    pub(crate) evidence: GeometryCandidateEvidenceRecord,
    pub(crate) program: Value,
    pub(crate) artifact_sha256: String,
}

#[derive(Debug, Clone)]
struct VisualBindings {
    render_set_sha256: String,
    quality_sha256: String,
    quality_status: String,
    metrics: Value,
    camera: Value,
    target_sha256: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct ViewEvaluation {
    pub(crate) view_id: String,
    pub(crate) kind: String,
    pub(crate) visibility: String,
    pub(crate) confidence: f64,
    pub(crate) reference_id: String,
    pub(crate) reference_sha256: String,
    pub(crate) target_sha256: Option<String>,
    pub(crate) view_spec: Value,
    pub(crate) camera: Value,
}

impl Runtime {
    /// Read one strict DesignActionRun@1 payload inside the caller's bound
    /// project/session/candidate scope.  The response is the contract itself,
    /// not a Viewer-generated projection.
    pub fn design_action_run_get(&self, request: Value) -> Result<Value, RuntimeError> {
        let object = request_object(&request, "design_action_run_get")?;
        reject_unknown_keys(
            object,
            &["project_id", "session_id", "candidate_id", "run_id"],
        )?;
        let project_id = required_id(object, "project_id")?;
        let session_id = required_id(object, "session_id")?;
        let candidate_id = required_id(object, "candidate_id")?;
        let run_id = required_id(object, "run_id")?;
        let run = self.store.get_design_action_run(run_id)?.ok_or_else(|| {
            RuntimeError::InvalidInput("NOT_FOUND: DesignActionRun not found".to_owned())
        })?;
        validate_run_scope(&run, run_id, session_id, project_id, candidate_id)?;
        Ok(run)
    }

    /// Prepare an explicit, replayable Repair application boundary.  The
    /// proposal candidate is already compiled and independently evaluated by
    /// `design_action_run_prepare`; this method revalidates the source head,
    /// immutable RepairIntent, candidate lineage and visual gate, then stores
    /// only a CAS-backed apply intent.  It never changes the active snapshot,
    /// candidate head, version history or export state.  The final
    /// `candidate_confirm` / `cross_view_promotion_confirm` transaction must
    /// repeat these checks and receive a fresh user approval.
    pub fn repair_apply_prepare(&self, request: Value) -> Result<Value, RuntimeError> {
        let object = request_object(&request, "repair_apply_prepare")?;
        reject_unknown_keys(
            object,
            &[
                "project_id",
                "session_id",
                "candidate_id",
                "proposal_candidate_id",
                "run_id",
                "source_candidate_state_sha256",
                "intent_sha256",
                "intent_object_sha256",
                "proposal_candidate_state_sha256",
                "prepared_object_id",
                "prepared_object_sha256",
                "quality_report_id",
                "cross_view_evidence_sha256",
                "base_version_id",
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
        let proposal_candidate_id = required_id(object, "proposal_candidate_id")?;
        let run_id = required_id(object, "run_id")?;
        let source_candidate_state_sha256 = required_sha(object, "source_candidate_state_sha256")?;
        let intent_sha256 = required_sha(object, "intent_sha256")?;
        let intent_object_sha256 = required_sha(object, "intent_object_sha256")?;
        let proposal_candidate_state_sha256 =
            required_sha(object, "proposal_candidate_state_sha256")?;
        let prepared_object_id = required_id(object, "prepared_object_id")?;
        let prepared_object_sha256 = required_sha(object, "prepared_object_sha256")?;
        let quality_report_id = required_id(object, "quality_report_id")?;
        let input_sha256 = required_sha(object, "input_sha256")?;
        let idempotency_key = required_id(object, "idempotency_key")?;
        let cross_view_evidence_sha256 = object
            .get("cross_view_evidence_sha256")
            .cloned()
            .unwrap_or(Value::Null);
        if !cross_view_evidence_sha256.is_null()
            && !cross_view_evidence_sha256.as_str().is_some_and(is_sha256)
        {
            return Err(RuntimeError::InvalidInput(
                "REPAIR_APPLY_EVIDENCE_INVALID: cross_view_evidence_sha256 must be null or SHA-256"
                    .to_owned(),
            ));
        }
        let base_version_id = object
            .get("base_version_id")
            .cloned()
            .ok_or_else(|| RuntimeError::InvalidInput("base_version_id is required".to_owned()))?;
        if !base_version_id.is_null() && !base_version_id.as_str().is_some_and(is_opaque_id) {
            return Err(RuntimeError::InvalidInput(
                "REPAIR_APPLY_SCOPE_INVALID: base_version_id must be null or an opaque id"
                    .to_owned(),
            ));
        }
        if object.get("approval_session_id").and_then(Value::as_str) != Some(session_id) {
            return Err(RuntimeError::InvalidInput(
                "REPAIR_APPLY_SCOPE_MISMATCH: approval_session_id must match session_id".to_owned(),
            ));
        }

        let input_binding = json!({
            "project_id":project_id,
            "session_id":session_id,
            "candidate_id":source_candidate_id,
            "proposal_candidate_id":proposal_candidate_id,
            "run_id":run_id,
            "source_candidate_state_sha256":source_candidate_state_sha256,
            "intent_sha256":intent_sha256,
            "intent_object_sha256":intent_object_sha256,
            "proposal_candidate_state_sha256":proposal_candidate_state_sha256,
            "prepared_object_id":prepared_object_id,
            "prepared_object_sha256":prepared_object_sha256,
            "quality_report_id":quality_report_id,
            "cross_view_evidence_sha256":cross_view_evidence_sha256,
            "base_version_id":base_version_id,
            "idempotency_key":idempotency_key
        });
        if input_sha256 != canonical_json_hash(&input_binding) {
            return Err(RuntimeError::InvalidInput(
                "REPAIR_APPLY_INPUT_HASH_MISMATCH: input_sha256 must bind the complete apply request"
                    .to_owned(),
            ));
        }

        let key_hash = sha256_hex(idempotency_key.as_bytes());
        let job_id = format!("repair-apply-key-{}", &key_hash[..32]);
        if let Some(existing) = self.store.get_job_record(&job_id)? {
            if existing.project_id != project_id
                || existing.kind != "agentic_repair_apply_prepare"
                || existing.request_sha256 != input_sha256
            {
                return Err(RuntimeError::InvalidInput(
                    "REPAIR_APPLY_IDEMPOTENCY_CONFLICT: idempotency key is bound to another request"
                        .to_owned(),
                ));
            }
            let checkpoint_sha256 = existing.checkpoint_sha256.as_deref().ok_or_else(|| {
                RuntimeError::InvalidInput(
                    "REPAIR_APPLY_READBACK_MISSING: prepared apply result is unavailable"
                        .to_owned(),
                )
            })?;
            let result = read_json_object(self, checkpoint_sha256)?;
            validate_repair_apply_result(
                &result,
                project_id,
                session_id,
                source_candidate_id,
                proposal_candidate_id,
                run_id,
                input_sha256,
            )?;
            return Ok(result);
        }

        let session = self
            .store
            .get_agentic_session(session_id)?
            .ok_or_else(|| RuntimeError::InvalidInput("AGENTIC_SESSION_NOT_FOUND".to_owned()))?;
        validate_session_scope(&session, session_id, project_id, source_candidate_id)?;
        self.session_get(json!({
            "session_id":session_id,
            "project_id":project_id,
            "candidate_id":source_candidate_id
        }))?;
        // A conservative/default session may be read for intake, but it is
        // never sufficient to prepare a Repair application intent.  Re-read
        // the explicit ReferenceCanvas lineage before any candidate/job work.
        super::agentic_session::require_bound_authoring_context(self, &session)?;

        let source_candidate = self.candidate(source_candidate_id)?.ok_or_else(|| {
            RuntimeError::InvalidInput("NOT_FOUND: source candidate not found".to_owned())
        })?;
        if source_candidate.project_id != project_id
            || source_candidate.canonical_sha256 != source_candidate_state_sha256
            || source_candidate.base_version_id != value_to_optional_id(&base_version_id)?
        {
            return Err(RuntimeError::InvalidInput(
                "REPAIR_APPLY_SOURCE_STALE: source candidate state or base version changed"
                    .to_owned(),
            ));
        }
        if source_candidate_id == proposal_candidate_id {
            return Err(RuntimeError::InvalidInput(
                "REPAIR_APPLY_SCOPE_MISMATCH: source and proposal candidates must differ"
                    .to_owned(),
            ));
        }
        let proposal_candidate = self.candidate(proposal_candidate_id)?.ok_or_else(|| {
            RuntimeError::InvalidInput("NOT_FOUND: proposal candidate not found".to_owned())
        })?;
        if proposal_candidate.project_id != project_id
            || proposal_candidate.canonical_sha256 != proposal_candidate_state_sha256
            || proposal_candidate.base_version_id != source_candidate.base_version_id
            || proposal_candidate.prepared_object_id.as_deref() != Some(prepared_object_id)
            || proposal_candidate.prepared_object_sha256.as_deref() != Some(prepared_object_sha256)
            || proposal_candidate.quality_report_id.as_deref() != Some(quality_report_id)
            || proposal_candidate.state != "reviewable"
            || !proposal_candidate.quality_hard_gate_passed
        {
            return Err(RuntimeError::InvalidInput(
                "REPAIR_APPLY_PROPOSAL_BINDING_MISMATCH: proposal candidate is not the approved reviewable result"
                    .to_owned(),
            ));
        }

        let run = self.store.get_design_action_run(run_id)?.ok_or_else(|| {
            RuntimeError::InvalidInput("NOT_FOUND: DesignActionRun not found".to_owned())
        })?;
        validate_run_scope(&run, run_id, session_id, project_id, source_candidate_id)?;
        if run.get("status").and_then(Value::as_str) != Some("completed") {
            return Err(RuntimeError::InvalidInput(
                "REPAIR_APPLY_RUN_NOT_REVIEWABLE: DesignActionRun did not complete".to_owned(),
            ));
        }
        let proposal = run
            .get("proposal")
            .and_then(Value::as_object)
            .ok_or_else(|| {
                RuntimeError::InvalidInput(
                    "REPAIR_APPLY_RUN_NOT_REVIEWABLE: run has no proposal".to_owned(),
                )
            })?;
        if proposal.get("candidate_id").and_then(Value::as_str) != Some(proposal_candidate_id)
            || proposal
                .get("candidate_state_sha256")
                .and_then(Value::as_str)
                != Some(proposal_candidate_state_sha256)
            || proposal.get("intent_sha256").and_then(Value::as_str) != Some(intent_sha256)
            || proposal.get("intent_object_sha256").and_then(Value::as_str)
                != Some(intent_object_sha256)
            || proposal.get("artifact_sha256").and_then(Value::as_str)
                != Some(prepared_object_sha256)
            || proposal.get("visual_status").and_then(Value::as_str)
                != Some("PARTIAL_VISIBLE_VIEW_PASS")
            || proposal.get("strict_improvement") != Some(&Value::Bool(true))
            || proposal.get("non_regressing") != Some(&Value::Bool(true))
            || proposal.get("promotion").and_then(Value::as_str) != Some("reviewable")
        {
            return Err(RuntimeError::InvalidInput(
                "REPAIR_APPLY_QUALITY_GATE_FAILED: run proposal is not a strict reviewable improvement"
                    .to_owned(),
            ));
        }

        let intent_metadata = self
            .store
            .get_object(intent_object_sha256)?
            .ok_or_else(|| {
                RuntimeError::InvalidInput(
                    "REPAIR_APPLY_INTENT_UNAVAILABLE: RepairIntent CAS metadata is missing"
                        .to_owned(),
                )
            })?;
        if intent_metadata.mime != "application/json"
            || intent_metadata.kind != "agentic-repair-intent"
        {
            return Err(RuntimeError::InvalidInput(
                "REPAIR_APPLY_INTENT_BINDING_MISMATCH: CAS object is not a RepairIntent".to_owned(),
            ));
        }
        let intent = read_json_object(self, intent_object_sha256)?;
        let mut intent_without_hash = intent.clone();
        intent_without_hash["canonical_sha256"] = Value::String(String::new());
        if intent.get("canonical_sha256").and_then(Value::as_str) != Some(intent_sha256)
            || canonical_json_hash(&intent_without_hash) != intent_sha256
            || intent.get("candidate_id").and_then(Value::as_str) != Some(source_candidate_id)
            || intent.get("candidate_state_sha256").and_then(Value::as_str)
                != Some(source_candidate_state_sha256)
            || intent.get("project_id").and_then(Value::as_str) != Some(project_id)
            || intent.get("session_id").and_then(Value::as_str) != Some(session_id)
        {
            return Err(RuntimeError::InvalidInput(
                "REPAIR_APPLY_INTENT_BINDING_MISMATCH: RepairIntent is stale or cross-scoped"
                    .to_owned(),
            ));
        }

        self.revalidate_candidate_for_confirmation(&proposal_candidate, prepared_object_sha256)?;
        let next_transaction = if cross_view_evidence_sha256.is_null() {
            self.revalidate_visual_evidence_for_confirmation(&proposal_candidate)?;
            "candidate_confirm"
        } else {
            let bundle_sha256 = cross_view_evidence_sha256.as_str().unwrap_or_default();
            if proposal
                .get("cross_view_evidence_sha256")
                .and_then(Value::as_str)
                != Some(bundle_sha256)
            {
                return Err(RuntimeError::InvalidInput(
                    "REPAIR_APPLY_EVIDENCE_BINDING_MISMATCH: cross-view bundle differs from the run"
                        .to_owned(),
                ));
            }
            let bundle_record = self
                .store
                .get_cross_view_evidence(bundle_sha256)?
                .ok_or_else(|| {
                    RuntimeError::InvalidInput(
                        "REPAIR_APPLY_EVIDENCE_UNAVAILABLE: cross-view bundle is not indexed"
                            .to_owned(),
                    )
                })?;
            if bundle_record.project_id != project_id
                || bundle_record.session_id != session_id
                || bundle_record.candidate_id != proposal_candidate_id
                || !bundle_record.hard_gate_passed
            {
                return Err(RuntimeError::InvalidInput(
                    "REPAIR_APPLY_EVIDENCE_BINDING_MISMATCH: cross-view bundle is not a passing proposal"
                        .to_owned(),
                ));
            }
            let bundle = read_json_object(self, bundle_sha256)?;
            super::validate_cross_view_evidence_bundle(&bundle)?;
            if bundle.get("candidate_state_sha256").and_then(Value::as_str)
                != Some(proposal_candidate_state_sha256)
                || bundle.get("artifact_sha256").and_then(Value::as_str)
                    != Some(prepared_object_sha256)
                || bundle.get("aggregate_status").and_then(Value::as_str)
                    != Some("PARTIAL_VISIBLE_VIEW_PASS")
                || bundle.get("hard_gate_passed") != Some(&Value::Bool(true))
                || bundle.get("strict_improvement") != Some(&Value::Bool(true))
                || bundle.get("non_regressing") != Some(&Value::Bool(true))
            {
                return Err(RuntimeError::InvalidInput(
                    "REPAIR_APPLY_EVIDENCE_QUALITY_GATE_FAILED: cross-view bundle is not promotable"
                        .to_owned(),
                ));
            }
            "cross_view_promotion_confirm"
        };

        let mut apply_intent = json!({
            "schema_version":"RepairApplyIntent@1",
            "project_id":project_id,
            "session_id":session_id,
            "source_candidate_id":source_candidate_id,
            "source_candidate_state_sha256":source_candidate_state_sha256,
            "proposal_candidate_id":proposal_candidate_id,
            "proposal_candidate_state_sha256":proposal_candidate_state_sha256,
            "run_id":run_id,
            "intent_sha256":intent_sha256,
            "intent_object_sha256":intent_object_sha256,
            "prepared_object_id":prepared_object_id,
            "prepared_object_sha256":prepared_object_sha256,
            "quality_report_id":quality_report_id,
            "cross_view_evidence_sha256":cross_view_evidence_sha256,
            "base_version_id":base_version_id,
            "input_sha256":input_sha256,
            "next_transaction":next_transaction,
            "approval_required":true,
            "confirm_allowed":false,
            "source_candidate_unchanged":true,
            "active_design_state_mutated":false,
            "persistent_user_data_touched":false,
            "canonical_sha256":""
        });
        apply_intent["canonical_sha256"] = Value::String(canonical_json_hash(&apply_intent));
        let apply_intent_object = self.put_object(
            &canonical_json_bytes(&apply_intent)
                .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?,
            None,
            "application/json",
            "agentic-repair-apply-intent",
        )?;

        let mut result = json!({
            "schema_version":"RepairApplyPrepareResult@1",
            "job_id":job_id,
            "project_id":project_id,
            "session_id":session_id,
            "candidate_id":source_candidate_id,
            "source_candidate_id":source_candidate_id,
            "proposal_candidate_id":proposal_candidate_id,
            "run_id":run_id,
            "input_sha256":input_sha256,
            "intent_sha256":intent_sha256,
            "intent_object_sha256":intent_object_sha256,
            "apply_intent_object_sha256":apply_intent_object.record.sha256,
            "apply_intent_canonical_sha256":apply_intent["canonical_sha256"].clone(),
            "base_version_id":base_version_id,
            "prepared_object_id":prepared_object_id,
            "prepared_object_sha256":prepared_object_sha256,
            "quality_report_id":quality_report_id,
            "cross_view_evidence_sha256":cross_view_evidence_sha256,
            "status":"ready",
            "next_transaction":next_transaction,
            "confirm_allowed":false,
            "source_candidate_unchanged":true,
            "active_design_state_mutated":false,
            "persistent_user_data_touched":false,
            "canonical_sha256":""
        });
        result["canonical_sha256"] = Value::String(canonical_json_hash(&result));
        let result_object = self.put_object(
            &canonical_json_bytes(&result)
                .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?,
            None,
            "application/json",
            "agentic-repair-apply-result",
        )?;
        let now = now_string();
        let job = JobRecord {
            schema_version: "RuntimeJob@1".to_owned(),
            job_id: job_id.clone(),
            project_id: project_id.to_owned(),
            kind: "agentic_repair_apply_prepare".to_owned(),
            status: "succeeded".to_owned(),
            progress: 100,
            request_sha256: input_sha256.to_owned(),
            checkpoint_sha256: Some(result_object.record.sha256.clone()),
            error_code: None,
            created_at: now.clone(),
            updated_at: now.clone(),
        };
        let event = JobEventRecord {
            schema_version: "RuntimeJobEvent@1".to_owned(),
            job_id: job_id.clone(),
            sequence: 1,
            kind: "repair_apply_prepared".to_owned(),
            payload: json!({
                "session_id":session_id,
                "source_candidate_id":source_candidate_id,
                "proposal_candidate_id":proposal_candidate_id,
                "apply_intent_object_sha256":apply_intent_object.record.sha256,
                "next_transaction":next_transaction
            }),
            created_at: now,
        };
        self.store.insert_job_with_event_if_absent(
            &job,
            &event,
            &[
                apply_intent_object.record.sha256,
                result_object.record.sha256,
            ],
        )?;
        Ok(result)
    }

    /// Consume a single-view RepairApplyIntent after a fresh approval.  This
    /// is the only confirmation path for a Repair proposal without a
    /// CrossViewEvidenceBundle.  The method deliberately re-reads every
    /// mutable boundary before asking Store to create the immutable version:
    /// source candidate head, RepairIntent, DesignActionRun, proposal
    /// candidate, artifact lineage and candidate-bound visual evidence.
    /// Multi-view intents fail closed and must use
    /// `cross_view_promotion_confirm`.
    pub fn repair_apply_confirm(&self, request: Value) -> Result<Value, RuntimeError> {
        let object = request_object(&request, "repair_apply_confirm")?;
        reject_unknown_keys(
            object,
            &[
                "project_id",
                "session_id",
                "candidate_id",
                "proposal_candidate_id",
                "run_id",
                "apply_intent_object_sha256",
                "apply_intent_canonical_sha256",
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
        let proposal_candidate_id = required_id(object, "proposal_candidate_id")?;
        let run_id = required_id(object, "run_id")?;
        let apply_intent_object_sha256 = required_sha(object, "apply_intent_object_sha256")?;
        let apply_intent_canonical_sha256 = required_sha(object, "apply_intent_canonical_sha256")?;
        let approval_receipt_id = required_id(object, "approval_receipt_id")?;
        let approval_summary = object
            .get("approval_summary")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                RuntimeError::InvalidInput(
                    "REPAIR_APPLY_CONFIRM_APPROVAL_REQUIRED: approval_summary is required"
                        .to_owned(),
                )
            })?;
        let approval_expires_at = object
            .get("approval_expires_at")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                RuntimeError::InvalidInput(
                    "REPAIR_APPLY_CONFIRM_APPROVAL_REQUIRED: approval_expires_at is required"
                        .to_owned(),
                )
            })?;
        let approval_session_id = required_id(object, "approval_session_id")?;
        let idempotency_key = required_id(object, "idempotency_key")?;
        if approval_session_id != session_id {
            return Err(RuntimeError::InvalidInput(
                "REPAIR_APPLY_CONFIRM_SCOPE_MISMATCH: approval_session_id must match session_id"
                    .to_owned(),
            ));
        }

        let input_sha256 = canonical_json_hash(&request);
        let apply_intent_metadata = self
            .store
            .get_object(apply_intent_object_sha256)?
            .ok_or_else(|| {
                RuntimeError::InvalidInput(
                    "REPAIR_APPLY_CONFIRM_INTENT_UNAVAILABLE: apply intent CAS object is missing"
                        .to_owned(),
                )
            })?;
        if apply_intent_metadata.mime != "application/json"
            || apply_intent_metadata.kind != "agentic-repair-apply-intent"
        {
            return Err(RuntimeError::InvalidInput(
                "REPAIR_APPLY_CONFIRM_INTENT_BINDING_MISMATCH: CAS object is not a RepairApplyIntent"
                    .to_owned(),
            ));
        }
        let apply_intent = read_json_object(self, apply_intent_object_sha256)?;
        if apply_intent.get("schema_version").and_then(Value::as_str) != Some("RepairApplyIntent@1")
            || apply_intent.get("canonical_sha256").and_then(Value::as_str)
                != Some(apply_intent_canonical_sha256)
        {
            return Err(RuntimeError::InvalidInput(
                "REPAIR_APPLY_CONFIRM_INTENT_BINDING_MISMATCH: apply intent schema or canonical hash drifted"
                    .to_owned(),
            ));
        }
        let mut apply_intent_without_hash = apply_intent.clone();
        apply_intent_without_hash["canonical_sha256"] = Value::String(String::new());
        if canonical_json_hash(&apply_intent_without_hash) != apply_intent_canonical_sha256 {
            return Err(RuntimeError::InvalidInput(
                "REPAIR_APPLY_CONFIRM_INTENT_BINDING_MISMATCH: apply intent canonical hash is invalid"
                    .to_owned(),
            ));
        }
        for (key, expected) in [
            ("project_id", project_id),
            ("session_id", session_id),
            ("source_candidate_id", source_candidate_id),
            ("proposal_candidate_id", proposal_candidate_id),
            ("run_id", run_id),
        ] {
            if apply_intent.get(key).and_then(Value::as_str) != Some(expected) {
                return Err(RuntimeError::InvalidInput(format!(
                    "REPAIR_APPLY_CONFIRM_INTENT_SCOPE_MISMATCH: {key} differs from apply intent"
                )));
            }
        }
        if apply_intent.get("next_transaction").and_then(Value::as_str) != Some("candidate_confirm")
            || apply_intent.get("cross_view_evidence_sha256") != Some(&Value::Null)
            || apply_intent.get("confirm_allowed") != Some(&Value::Bool(false))
            || apply_intent.get("approval_required") != Some(&Value::Bool(true))
            || apply_intent.get("source_candidate_unchanged") != Some(&Value::Bool(true))
            || apply_intent.get("active_design_state_mutated") != Some(&Value::Bool(false))
        {
            return Err(RuntimeError::InvalidInput(
                "REPAIR_APPLY_CONFIRM_CROSS_VIEW_REQUIRED: only a single-view apply intent can use repair_apply_confirm"
                    .to_owned(),
            ));
        }

        let session = self
            .store
            .get_agentic_session(session_id)?
            .ok_or_else(|| RuntimeError::InvalidInput("AGENTIC_SESSION_NOT_FOUND".to_owned()))?;
        validate_session_scope(&session, session_id, project_id, source_candidate_id)?;
        self.session_get(json!({
            "session_id": session_id,
            "project_id": project_id,
            "candidate_id": source_candidate_id
        }))?;
        super::agentic_session::require_bound_authoring_context(self, &session)?;

        let source_candidate = self.candidate(source_candidate_id)?.ok_or_else(|| {
            RuntimeError::InvalidInput("NOT_FOUND: source candidate not found".to_owned())
        })?;
        if source_candidate.project_id != project_id
            || source_candidate.canonical_sha256
                != apply_intent
                    .get("source_candidate_state_sha256")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
        {
            return Err(RuntimeError::InvalidInput(
                "REPAIR_APPLY_CONFIRM_SOURCE_STALE: source candidate state changed".to_owned(),
            ));
        }
        let base_version_id =
            value_to_optional_id(apply_intent.get("base_version_id").ok_or_else(|| {
                RuntimeError::InvalidInput(
                    "REPAIR_APPLY_CONFIRM_INTENT_INVALID: base_version_id is missing".to_owned(),
                )
            })?)?;
        if source_candidate.base_version_id != base_version_id {
            return Err(RuntimeError::InvalidInput(
                "REPAIR_APPLY_CONFIRM_SOURCE_STALE: source base version changed".to_owned(),
            ));
        }

        let proposal_candidate = self.candidate(proposal_candidate_id)?.ok_or_else(|| {
            RuntimeError::InvalidInput("NOT_FOUND: proposal candidate not found".to_owned())
        })?;
        let expected_proposal_state = apply_intent
            .get("proposal_candidate_state_sha256")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let expected_prepared_object_id = apply_intent
            .get("prepared_object_id")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let expected_prepared_object_sha256 = apply_intent
            .get("prepared_object_sha256")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let expected_quality_report_id = apply_intent
            .get("quality_report_id")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if proposal_candidate.project_id != project_id
            || proposal_candidate.canonical_sha256 != expected_proposal_state
            || proposal_candidate.base_version_id != base_version_id
            || proposal_candidate.prepared_object_id.as_deref() != Some(expected_prepared_object_id)
            || proposal_candidate.prepared_object_sha256.as_deref()
                != Some(expected_prepared_object_sha256)
            || proposal_candidate.quality_report_id.as_deref() != Some(expected_quality_report_id)
            || !proposal_candidate.quality_hard_gate_passed
        {
            return Err(RuntimeError::InvalidInput(
                "REPAIR_APPLY_CONFIRM_PROPOSAL_BINDING_MISMATCH: proposal candidate changed"
                    .to_owned(),
            ));
        }

        let run = self.store.get_design_action_run(run_id)?.ok_or_else(|| {
            RuntimeError::InvalidInput("NOT_FOUND: DesignActionRun not found".to_owned())
        })?;
        validate_run_scope(&run, run_id, session_id, project_id, source_candidate_id)?;
        if run.get("status").and_then(Value::as_str) != Some("completed") {
            return Err(RuntimeError::InvalidInput(
                "REPAIR_APPLY_CONFIRM_RUN_NOT_REVIEWABLE: DesignActionRun did not complete"
                    .to_owned(),
            ));
        }
        let proposal = run
            .get("proposal")
            .and_then(Value::as_object)
            .ok_or_else(|| {
                RuntimeError::InvalidInput(
                    "REPAIR_APPLY_CONFIRM_RUN_NOT_REVIEWABLE: run has no proposal".to_owned(),
                )
            })?;
        if proposal.get("candidate_id").and_then(Value::as_str) != Some(proposal_candidate_id)
            || proposal
                .get("candidate_state_sha256")
                .and_then(Value::as_str)
                != Some(expected_proposal_state)
            || proposal.get("intent_sha256").and_then(Value::as_str)
                != apply_intent.get("intent_sha256").and_then(Value::as_str)
            || proposal.get("intent_object_sha256").and_then(Value::as_str)
                != apply_intent
                    .get("intent_object_sha256")
                    .and_then(Value::as_str)
            || proposal.get("artifact_sha256").and_then(Value::as_str)
                != Some(expected_prepared_object_sha256)
            || proposal.get("visual_status").and_then(Value::as_str)
                != Some("PARTIAL_VISIBLE_VIEW_PASS")
            || proposal.get("strict_improvement") != Some(&Value::Bool(true))
            || proposal.get("non_regressing") != Some(&Value::Bool(true))
            || proposal.get("promotion").and_then(Value::as_str) != Some("reviewable")
        {
            return Err(RuntimeError::InvalidInput(
                "REPAIR_APPLY_CONFIRM_QUALITY_GATE_FAILED: run proposal is not a strict reviewable improvement"
                    .to_owned(),
            ));
        }

        let intent_sha256 = apply_intent
            .get("intent_sha256")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                RuntimeError::InvalidInput(
                    "REPAIR_APPLY_CONFIRM_INTENT_INVALID: RepairIntent hash is missing".to_owned(),
                )
            })?;
        let intent_object_sha256 = apply_intent
            .get("intent_object_sha256")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                RuntimeError::InvalidInput(
                    "REPAIR_APPLY_CONFIRM_INTENT_INVALID: RepairIntent object is missing"
                        .to_owned(),
                )
            })?;
        let repair_intent_metadata =
            self.store
                .get_object(intent_object_sha256)?
                .ok_or_else(|| {
                    RuntimeError::InvalidInput(
                "REPAIR_APPLY_CONFIRM_INTENT_UNAVAILABLE: RepairIntent CAS object is missing"
                    .to_owned(),
            )
                })?;
        if repair_intent_metadata.mime != "application/json"
            || repair_intent_metadata.kind != "agentic-repair-intent"
        {
            return Err(RuntimeError::InvalidInput(
                "REPAIR_APPLY_CONFIRM_INTENT_BINDING_MISMATCH: RepairIntent metadata drifted"
                    .to_owned(),
            ));
        }
        let repair_intent = read_json_object(self, intent_object_sha256)?;
        let mut repair_intent_without_hash = repair_intent.clone();
        repair_intent_without_hash["canonical_sha256"] = Value::String(String::new());
        if repair_intent
            .get("canonical_sha256")
            .and_then(Value::as_str)
            != Some(intent_sha256)
            || canonical_json_hash(&repair_intent_without_hash) != intent_sha256
            || repair_intent.get("project_id").and_then(Value::as_str) != Some(project_id)
            || repair_intent.get("session_id").and_then(Value::as_str) != Some(session_id)
            || repair_intent.get("candidate_id").and_then(Value::as_str)
                != Some(source_candidate_id)
        {
            return Err(RuntimeError::InvalidInput(
                "REPAIR_APPLY_CONFIRM_INTENT_BINDING_MISMATCH: RepairIntent is stale or cross-scoped"
                    .to_owned(),
            ));
        }

        self.revalidate_candidate_for_confirmation(
            &proposal_candidate,
            expected_prepared_object_sha256,
        )?;
        self.revalidate_visual_evidence_for_confirmation(&proposal_candidate)?;

        let candidate_request = CandidateConfirmRequest {
            project_id: project_id.to_owned(),
            candidate_id: proposal_candidate_id.to_owned(),
            base_version_id,
            prepared_object_id: expected_prepared_object_id.to_owned(),
            prepared_object_sha256: expected_prepared_object_sha256.to_owned(),
            quality_report_id: expected_quality_report_id.to_owned(),
            approval_receipt_id: approval_receipt_id.to_owned(),
            approval_summary: approval_summary.to_owned(),
            approval_session_id: approval_session_id.to_owned(),
            approval_expires_at: approval_expires_at.to_owned(),
            idempotency_key: idempotency_key.to_owned(),
        };
        let confirmed = self.store.confirm_repair_apply_candidate(
            &candidate_request,
            &now_string(),
            &input_sha256,
        )?;
        let mut output = json!({
            "schema_version":"RepairApplyConfirmResult@1",
            "project_id":project_id,
            "session_id":session_id,
            "candidate_id":source_candidate_id,
            "source_candidate_id":source_candidate_id,
            "proposal_candidate_id":proposal_candidate_id,
            "run_id":run_id,
            "apply_intent_object_sha256":apply_intent_object_sha256,
            "apply_intent_canonical_sha256":apply_intent_canonical_sha256,
            "version_id":confirmed.version_id,
            "snapshot_id":confirmed.snapshot_id,
            "approval_receipt_id":confirmed.approval_receipt_id,
            "request_sha256":input_sha256,
            "source_candidate_unchanged":true,
            "proposal_candidate_confirmed":true,
            "active_design_state_mutated":true,
            "replayed":confirmed.replayed,
            "canonical_sha256":""
        });
        output["canonical_sha256"] = Value::String(canonical_json_hash(&output));
        Ok(output)
    }

    /// Execute one durable RepairIntent through the Runtime-owned bounded
    /// prepare/compile/readback/render/evaluate loop. The CAS intent is the
    /// source of truth for the proposed repair; callers provide only the
    /// candidate-bound GeometryProgram/ViewSpec/camera payload. This keeps
    /// critic output, execution and the later RepairApply transaction on one
    /// exact intent lineage without allowing implicit confirmation.
    pub fn repair_intent_run_prepare(&self, request: Value) -> Result<Value, RuntimeError> {
        let object = request_object(&request, "repair_intent_run_prepare")?;
        reject_unknown_keys(
            object,
            &[
                "project_id",
                "session_id",
                "candidate_id",
                "run_id",
                "intent_sha256",
                "intent_object_sha256",
                "observation_sha256",
                "source_evidence_sha256",
                "reference_sha256",
                "action",
                "proposal",
                "requested_stage",
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
        let candidate_id = required_id(object, "candidate_id")?;
        let run_id = required_id(object, "run_id")?;
        let intent_sha256 = required_sha(object, "intent_sha256")?;
        let intent_object_sha256 = required_sha(object, "intent_object_sha256")?;
        let observation_sha256 = required_sha(object, "observation_sha256")?;
        let source_evidence_sha256 = required_sha(object, "source_evidence_sha256")?;
        let reference_sha256 = required_sha(object, "reference_sha256")?;
        let requested_stage = required_stage(object, "requested_stage")?;
        let input_sha256 = required_sha(object, "input_sha256")?;
        let action = object
            .get("action")
            .ok_or_else(|| RuntimeError::InvalidInput("action is required".to_owned()))?;
        validate_action(action)?;
        if matches!(
            runtime_parameter_patch_strategy(action),
            Ok("rear-stock-profile-reconstruction-v1")
        ) {
            return Err(RuntimeError::InvalidInput(
                "ACTION_STOCK_PROFILE_EXTERNAL_REPAIR_INTENT_FORBIDDEN".to_owned(),
            ));
        }
        let proposal = object
            .get("proposal")
            .and_then(Value::as_object)
            .ok_or_else(|| {
                RuntimeError::InvalidInput("REPAIR_INTENT_RUN_PROPOSAL_REQUIRED".to_owned())
            })?;
        if proposal.keys().any(|key| {
            !matches!(
                key.as_str(),
                "geometry_program" | "view_spec" | "camera" | "view_evaluations"
            )
        }) || !proposal.contains_key("geometry_program")
            || !proposal.contains_key("view_spec")
            || !proposal.contains_key("camera")
        {
            return Err(RuntimeError::InvalidInput(
                "REPAIR_INTENT_RUN_PROPOSAL_INVALID".to_owned(),
            ));
        }

        let input_binding = json!({
            "project_id":project_id,
            "session_id":session_id,
            "candidate_id":candidate_id,
            "run_id":run_id,
            "intent_sha256":intent_sha256,
            "intent_object_sha256":intent_object_sha256,
            "observation_sha256":observation_sha256,
            "source_evidence_sha256":source_evidence_sha256,
            "reference_sha256":reference_sha256,
            "action":action,
            "proposal":proposal,
            "requested_stage":requested_stage
        });
        let expected_input_sha256 = canonical_json_hash(&input_binding);
        let numeric_compatibility_input_sha256 =
            canonical_json_hash(&normalize_action_input_numbers(&input_binding));
        if input_sha256 != expected_input_sha256
            && input_sha256 != numeric_compatibility_input_sha256
        {
            return Err(RuntimeError::InvalidInput(format!(
                "REPAIR_INTENT_RUN_INPUT_HASH_MISMATCH: expected={expected_input_sha256} numeric_compatibility={numeric_compatibility_input_sha256} actual={input_sha256}"
            )));
        }

        let session = self
            .store
            .get_agentic_session(session_id)?
            .ok_or_else(|| RuntimeError::InvalidInput("AGENTIC_SESSION_NOT_FOUND".to_owned()))?;
        validate_session_scope(&session, session_id, project_id, candidate_id)?;
        if session.observation_sha256 != observation_sha256
            || session.evidence_sha256 != source_evidence_sha256
            || session.reference_sha256 != reference_sha256
            || session.current_stage != requested_stage
        {
            return Err(RuntimeError::InvalidInput(
                "REPAIR_INTENT_RUN_SESSION_BINDING_MISMATCH".to_owned(),
            ));
        }
        self.session_get(json!({
            "session_id":session_id,
            "project_id":project_id,
            "candidate_id":candidate_id
        }))?;
        let bound_observation =
            self.bound_agentic_observation(project_id, Some(candidate_id), observation_sha256)?;
        if bound_observation
            .get("canonical_sha256")
            .and_then(Value::as_str)
            != Some(observation_sha256)
        {
            return Err(RuntimeError::InvalidInput(
                "REPAIR_INTENT_RUN_OBSERVATION_STALE".to_owned(),
            ));
        }

        let intent_metadata = self
            .store
            .get_object(intent_object_sha256)?
            .ok_or_else(|| {
                RuntimeError::InvalidInput("REPAIR_INTENT_RUN_INTENT_OBJECT_UNAVAILABLE".to_owned())
            })?;
        if intent_metadata.mime != "application/json"
            || intent_metadata.kind != "agentic-repair-intent"
        {
            return Err(RuntimeError::InvalidInput(
                "REPAIR_INTENT_RUN_INTENT_METADATA_MISMATCH".to_owned(),
            ));
        }
        let intent = read_json_object(self, intent_object_sha256)?;
        let mut intent_without_hash = intent.clone();
        intent_without_hash["canonical_sha256"] = Value::String(String::new());
        if intent.get("schema_version").and_then(Value::as_str) != Some("RepairIntent@1")
            || intent.get("canonical_sha256").and_then(Value::as_str) != Some(intent_sha256)
            || canonical_json_hash(&intent_without_hash) != intent_sha256
            || !matches!(
                intent.get("status").and_then(Value::as_str),
                Some("proposed" | "approved")
            )
            || intent.get("approval_required") != Some(&Value::Bool(true))
            || intent.get("runtime_write") != Some(&Value::Bool(false))
        {
            return Err(RuntimeError::InvalidInput(
                "REPAIR_INTENT_RUN_INTENT_INVALID".to_owned(),
            ));
        }
        for (key, expected) in [
            ("project_id", project_id),
            ("session_id", session_id),
            ("candidate_id", candidate_id),
            ("reference_id", session.reference_id.as_str()),
            ("reference_sha256", reference_sha256),
            ("camera_hash", session.camera_hash.as_str()),
            ("observation_sha256", observation_sha256),
            ("source_evidence_sha256", source_evidence_sha256),
            ("stage", requested_stage),
        ] {
            if intent.get(key).and_then(Value::as_str) != Some(expected) {
                return Err(RuntimeError::InvalidInput(
                    "REPAIR_INTENT_RUN_INTENT_BINDING_MISMATCH".to_owned(),
                ));
            }
        }
        let candidate = self.candidate(candidate_id)?.ok_or_else(|| {
            RuntimeError::InvalidInput("NOT_FOUND: candidate not found".to_owned())
        })?;
        if candidate.project_id != project_id
            || intent.get("candidate_state_sha256").and_then(Value::as_str)
                != Some(candidate.canonical_sha256.as_str())
        {
            return Err(RuntimeError::InvalidInput(
                "REPAIR_INTENT_RUN_CANDIDATE_STALE".to_owned(),
            ));
        }
        let intent_scope = intent
            .get("scope")
            .and_then(Value::as_object)
            .ok_or_else(|| {
                RuntimeError::InvalidInput("REPAIR_INTENT_RUN_SCOPE_INVALID".to_owned())
            })?;
        let action_object = action.as_object().ok_or_else(|| {
            RuntimeError::InvalidInput("REPAIR_INTENT_RUN_ACTION_INVALID".to_owned())
        })?;
        if action_object.get("action_kind").and_then(Value::as_str) != Some("bounded-repair")
            || action_object.get("scope_kind").and_then(Value::as_str) != Some("part")
            || action_object.get("bounded") != Some(&Value::Bool(true))
            || intent_scope.get("kind").and_then(Value::as_str) != Some("part")
            || intent_scope.get("part_id") != action_object.get("target_id")
        {
            return Err(RuntimeError::InvalidInput(
                "REPAIR_INTENT_RUN_SCOPE_MISMATCH".to_owned(),
            ));
        }
        let intent_action = intent
            .get("action")
            .and_then(Value::as_object)
            .ok_or_else(|| {
                RuntimeError::InvalidInput("REPAIR_INTENT_RUN_ACTION_INVALID".to_owned())
            })?;
        if intent_action.get("action_kind").and_then(Value::as_str) != Some("bounded-repair")
            || intent_action.get("bounded") != Some(&Value::Bool(true))
            || intent_action.get("parameter_changes") != action_object.get("parameter_changes")
        {
            return Err(RuntimeError::InvalidInput(
                "REPAIR_INTENT_RUN_ACTION_BINDING_MISMATCH".to_owned(),
            ));
        }
        let precondition = intent
            .get("precondition")
            .and_then(Value::as_object)
            .ok_or_else(|| {
                RuntimeError::InvalidInput("REPAIR_INTENT_RUN_PRECONDITION_INVALID".to_owned())
            })?;
        if precondition
            .get("current_candidate_state_sha256")
            .and_then(Value::as_str)
            != Some(candidate.canonical_sha256.as_str())
            || precondition.get("evidence_sha256").and_then(Value::as_str)
                != Some(source_evidence_sha256)
        {
            return Err(RuntimeError::InvalidInput(
                "REPAIR_INTENT_RUN_PRECONDITION_MISMATCH".to_owned(),
            ));
        }
        if intent.get("recompute")
            != Some(&json!({
                "steps":["compile","readback","render","compare"],
                "must_rebind_reference":true,
                "must_rebind_camera":true,
                "confirm_allowed":false
            }))
        {
            return Err(RuntimeError::InvalidInput(
                "REPAIR_INTENT_RUN_RECOMPUTE_INVALID".to_owned(),
            ));
        }

        let mut bound_proposal = Value::Object(proposal.clone());
        if bound_proposal.get("repair_intent").is_some() {
            return Err(RuntimeError::InvalidInput(
                "REPAIR_INTENT_RUN_PROPOSAL_MUST_NOT_OVERRIDE_INTENT".to_owned(),
            ));
        }
        bound_proposal["repair_intent"] = intent;
        let action_input = json!({
            "project_id":project_id,
            "session_id":session_id,
            "candidate_id":candidate_id,
            "run_id":run_id,
            "action":action,
            "requested_stage":requested_stage,
            "observation_sha256":observation_sha256,
            "proposal":bound_proposal
        });
        let action_input_sha256 = canonical_json_hash(&action_input);
        let action_run = self.design_action_run_prepare(json!({
            "project_id":project_id,
            "session_id":session_id,
            "candidate_id":candidate_id,
            "run_id":run_id,
            "action":action,
            "input_sha256":action_input_sha256,
            "requested_stage":requested_stage,
            "approved":true,
            "approval_receipt_id":object.get("approval_receipt_id").cloned().unwrap_or(Value::Null),
            "approval_summary":object.get("approval_summary").cloned().unwrap_or(Value::Null),
            "approval_expires_at":object.get("approval_expires_at").cloned().unwrap_or(Value::Null),
            "approval_session_id":object.get("approval_session_id").cloned().unwrap_or(Value::String(session_id.to_owned())),
            "idempotency_key":format!("repair-intent-run-{}", &action_input_sha256[..24]),
            "observation_sha256":observation_sha256,
            "proposal":bound_proposal
        }))?;
        let run_proposal = action_run.get("proposal").and_then(Value::as_object);
        let strict_improvement = run_proposal
            .and_then(|value| value.get("strict_improvement"))
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let non_regressing = run_proposal
            .and_then(|value| value.get("non_regressing"))
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let quality_status = action_run
            .get("quality_status")
            .and_then(Value::as_str)
            .unwrap_or("QUALITY_TARGET_NOT_MET");
        let reviewable = strict_improvement
            && non_regressing
            && quality_status == "PARTIAL_VISIBLE_VIEW_PASS"
            && run_proposal
                .and_then(|value| value.get("promotion"))
                .and_then(Value::as_str)
                == Some("reviewable");
        let mut result = json!({
            "schema_version":"RepairIntentRunResult@1",
            "project_id":project_id,
            "session_id":session_id,
            "candidate_id":candidate_id,
            "run_id":run_id,
            "intent_sha256":intent_sha256,
            "intent_object_sha256":intent_object_sha256,
            "input_sha256":input_sha256,
            "observation_sha256":observation_sha256,
            "source_evidence_sha256":source_evidence_sha256,
            "reference_sha256":reference_sha256,
            "status":if reviewable {"reviewable"} else {"blocked"},
            "run_status":action_run.get("status").cloned().unwrap_or(Value::String("blocked".to_owned())),
            "quality_status":quality_status,
            "action_run_sha256":action_run.get("canonical_sha256").cloned().unwrap_or(Value::Null),
            "action_run":action_run,
            "proposal_candidate_id":run_proposal.and_then(|value| value.get("candidate_id")).cloned().unwrap_or(Value::Null),
            "proposal_candidate_state_sha256":run_proposal.and_then(|value| value.get("candidate_state_sha256")).cloned().unwrap_or(Value::Null),
            "prepared_object_sha256":run_proposal.and_then(|value| value.get("artifact_sha256")).cloned().unwrap_or(Value::Null),
            "quality_report_id":run_proposal.and_then(|value| value.get("quality_report_sha256")).cloned().unwrap_or(Value::Null),
            "apply_status":if reviewable {"ready_for_repair_apply_prepare"} else {"blocked"},
            "next_transaction":if reviewable {"repair_apply_prepare"} else {"inspect_or_retry"},
            "confirm_allowed":false,
            "source_candidate_unchanged":true,
            "active_design_state_mutated":false,
            "runtime_write":false,
            "persistent_user_data_touched":false,
            "canonical_sha256":""
        });
        result["canonical_sha256"] = Value::String(canonical_json_hash(&result));
        Ok(result)
    }

    /// Execute the bounded P0 action loop over the current immutable
    /// candidate.  The action is an approved, typed intent; this first slice
    /// uses it to authorize a deterministic revalidation pass and does not
    /// pretend to apply a general mesh-delta repair.
    pub fn design_action_run_prepare(&self, request: Value) -> Result<Value, RuntimeError> {
        let object = request_object(&request, "design_action_run_prepare")?;
        reject_unknown_keys(
            object,
            &[
                "project_id",
                "session_id",
                "candidate_id",
                "run_id",
                "action",
                "input_sha256",
                "requested_stage",
                "approved",
                "approval_receipt_id",
                "approval_summary",
                "approval_expires_at",
                "approval_session_id",
                "idempotency_key",
                "proposal",
                "optimization_intent",
                "view_spec",
                "observation_sha256",
            ],
        )?;
        validate_approval(object)?;

        let project_id = required_id(object, "project_id")?;
        let session_id = required_id(object, "session_id")?;
        let candidate_id = required_id(object, "candidate_id")?;
        let run_id = required_id(object, "run_id")?;
        let requested_stage = required_stage(object, "requested_stage")?;
        let input_sha256 = required_sha(object, "input_sha256")?;
        let observation_sha256 = required_sha(object, "observation_sha256")?;
        let action = object
            .get("action")
            .ok_or_else(|| RuntimeError::InvalidInput("action is required".to_owned()))?;
        validate_action(action)?;
        validate_execution_payload(
            action,
            object.get("proposal").filter(|value| !value.is_null()),
            object.get("optimization_intent"),
            object.get("view_spec").filter(|value| !value.is_null()),
        )?;

        let mut input_binding = json!({
            "project_id": project_id,
            "session_id": session_id,
            "candidate_id": candidate_id,
            "run_id": run_id,
            "action": action,
            "requested_stage": requested_stage,
            "observation_sha256": observation_sha256,
        });
        if let Some(proposal) = object.get("proposal") {
            input_binding["proposal"] = proposal.clone();
        }
        if let Some(optimization_intent) = object.get("optimization_intent") {
            input_binding["optimization_intent"] = optimization_intent.clone();
        }
        if let Some(view_spec) = object.get("view_spec") {
            input_binding["view_spec"] = view_spec.clone();
        }
        let expected_input_sha256 = canonical_json_hash(&input_binding);
        let numeric_compatibility_input_sha256 =
            canonical_json_hash(&normalize_action_input_numbers(&input_binding));
        if input_sha256 != expected_input_sha256
            && input_sha256 != numeric_compatibility_input_sha256
        {
            return Err(RuntimeError::InvalidInput(format!(
                "DESIGN_ACTION_INPUT_HASH_MISMATCH: expected={expected_input_sha256} actual={input_sha256}"
            )));
        }

        let session = self
            .store
            .get_agentic_session(session_id)?
            .ok_or_else(|| RuntimeError::InvalidInput("AGENTIC_SESSION_NOT_FOUND".to_owned()))?;
        validate_session_scope(&session, session_id, project_id, candidate_id)?;
        // Re-read the durable authoring objects before any action work.  A
        // DesignActionRun must fail closed if its ReferenceCanvas or
        // DesignSpec is missing, tampered with, or no longer bound to the
        // session; this call is read-only and does not create a new session.
        self.session_get(json!({
            "session_id": session_id,
            "project_id": project_id,
            "candidate_id": candidate_id
        }))?;
        if !is_sha256(&session.observation_sha256)
            || session.observation_sha256 != observation_sha256
        {
            return Err(RuntimeError::InvalidInput(
                "AGENTIC_OBSERVATION_STALE: supplied observation is not the durable session observation"
                    .to_owned(),
            ));
        }
        let bound_observation =
            self.bound_agentic_observation(project_id, Some(candidate_id), observation_sha256)?;
        if bound_observation
            .get("canonical_sha256")
            .and_then(Value::as_str)
            != Some(observation_sha256)
        {
            return Err(RuntimeError::InvalidInput(
                "AGENTIC_OBSERVATION_STALE: supplied observation does not match the current Runtime projection"
                    .to_owned(),
            ));
        }
        let action_kind = action
            .get("action_kind")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if !matches!(action_kind, "request-reference" | "checkpoint") {
            super::agentic_session::require_bound_authoring_context(self, &session)?;
        }
        let candidate = self.candidate(candidate_id)?.ok_or_else(|| {
            RuntimeError::InvalidInput("NOT_FOUND: candidate not found".to_owned())
        })?;
        if candidate.project_id != project_id {
            return Err(RuntimeError::InvalidInput(
                "PROJECT_SCOPE_DENIED: candidate is outside the requested project".to_owned(),
            ));
        }

        if let Some(existing) = self.store.get_design_action_run(run_id)? {
            if existing.get("input_sha256").and_then(Value::as_str) != Some(input_sha256)
                || existing.get("requested_stage").and_then(Value::as_str) != Some(requested_stage)
                || existing.get("action") != Some(action)
            {
                return Err(RuntimeError::InvalidInput(
                    "DESIGN_ACTION_RUN_IMMUTABLE_CONFLICT: run_id is already bound to another action"
                        .to_owned(),
                ));
            }
            // input_sha256 already binds the complete original proposal when
            // one is present.  The persisted `proposal` field is deliberately
            // a Runtime-generated result summary, so comparing it with the
            // request payload would reject a valid replay after execution.
            validate_run_scope(&existing, run_id, session_id, project_id, candidate_id)?;
            return Ok(existing);
        }

        let mut run = initial_run(
            run_id,
            session_id,
            project_id,
            candidate_id,
            &session,
            input_sha256,
            action,
            requested_stage,
            observation_sha256,
        );

        // A reference request is an orchestration boundary, not a geometry
        // operation.  Persist the missing-coverage decision without loading
        // the candidate program, compiling, rendering, or creating a
        // checkpoint.  The target is bound to the session's authorized
        // reference so a caller cannot turn this action into an arbitrary
        // reference import or a cross-project lookup.
        if action.get("action_kind").and_then(Value::as_str) == Some("request-reference") {
            if action.get("target_id").and_then(Value::as_str)
                != Some(session.reference_id.as_str())
            {
                return Err(RuntimeError::InvalidInput(
                    "DESIGN_ACTION_REFERENCE_TARGET_MISMATCH".to_owned(),
                ));
            }
            return persist_reference_request_run(self, run);
        }

        let geometry = match load_geometry_bindings(self, &candidate, project_id, &session) {
            Ok(bindings) => bindings,
            Err(_) => {
                return persist_blocked_run(
                    self,
                    run,
                    "prepare",
                    None,
                    "geometry-evidence",
                    "prepare-evidence-unavailable",
                )
            }
        };
        set_stage_completed(&mut run, "prepare", input_sha256);

        if requested_stage != session.current_stage {
            return persist_blocked_run(
                self,
                run,
                "compile",
                Some("prepare"),
                "stage-binding",
                "requested-stage-is-not-current",
            );
        }

        let inspection = match recompile_candidate(self, &geometry) {
            Ok(inspection) => inspection,
            Err(_) => {
                return persist_blocked_run(
                    self,
                    run,
                    "compile",
                    Some("prepare"),
                    "compile-readback",
                    "compile-revalidation-failed",
                )
            }
        };
        set_stage_completed(&mut run, "compile", &geometry.artifact_sha256);

        let readback_sha256 =
            match verify_artifact_readback(self, &candidate, &geometry, &inspection) {
                Ok(hash) => hash,
                Err(_) => {
                    return persist_blocked_run(
                        self,
                        run,
                        "readback",
                        Some("compile"),
                        "artifact-readback",
                        "artifact-readback-failed",
                    )
                }
            };
        set_stage_completed(&mut run, "readback", &readback_sha256);

        let visual = match verify_visual_bindings(self, &candidate, &geometry, &session, project_id)
        {
            Ok(bindings) => bindings,
            Err(error) => {
                return persist_blocked_run(
                    self,
                    run,
                    "render",
                    Some("readback"),
                    "reference-comparison",
                    &stable_proposal_failure_code(&error),
                )
            }
        };
        set_stage_completed(&mut run, "render", &visual.render_set_sha256);
        set_stage_completed(&mut run, "evaluate", &visual.quality_sha256);

        // The mainline Primary Form action is intentionally a separate,
        // Runtime-owned path from the generic proposal executor.  It accepts
        // only the already validated bounded parameter changes, derives a
        // SilhouetteRig, and delegates the search/compile/readback/compare
        // loop to `primary_form_repair_prepare`.  No caller-supplied
        // GeometryProgram or view payload is admitted on this path.
        if matches!(
            action.get("action_kind").and_then(Value::as_str),
            Some("primary-form-adjustment" | "bounded-repair")
        ) && object.get("proposal").is_none_or(Value::is_null)
            && object.get("optimization_intent").is_none()
            && object.get("view_spec").is_none_or(Value::is_null)
        {
            return execute_direct_primary_form_action(
                self, run, action, &session, &candidate, &visual,
            );
        }

        if object.get("optimization_intent").is_some()
            && object.get("proposal").is_some_and(|value| !value.is_null())
        {
            return persist_blocked_run(
                self,
                run,
                "evaluate",
                Some("render"),
                "stage-precondition",
                "optimization-intent-cannot-be-combined-with-repair-proposal",
            );
        }

        if let Some(optimization_intent) = object.get("optimization_intent") {
            let optimization = match prepare_action_optimization(
                self,
                object,
                optimization_intent,
                run_id,
                requested_stage,
                action,
                &candidate,
            ) {
                Ok(value) => value,
                Err(error) => {
                    let reason = stable_optimization_failure_code(&error);
                    return persist_blocked_run(
                        self,
                        run,
                        "evaluate",
                        Some("render"),
                        "evaluate",
                        &reason,
                    );
                }
            };
            run["optimization_job_id"] = optimization["job_id"].clone();
            run["optimization_intent_sha256"] = optimization["intent_sha256"].clone();
        }

        let automatic_parameter_patch = if object.get("proposal").is_none_or(Value::is_null)
            && object.get("optimization_intent").is_none()
        {
            let view_spec = object.get("view_spec").filter(|value| !value.is_null());
            if let Some(view_spec) = view_spec {
                let strategy = match runtime_parameter_patch_strategy(action) {
                    Ok(strategy) => strategy,
                    Err(error) => {
                        let reason = stable_proposal_failure_code(&error);
                        return persist_blocked_run(
                            self,
                            run,
                            "evaluate",
                            Some("render"),
                            "repair-proposal",
                            &reason,
                        );
                    }
                };
                Some(json!({
                    "parameter_patch":{
                        "schema_version":"RuntimeParameterPatch@1",
                        "strategy":strategy
                    },
                    "view_spec":view_spec,
                    "camera":visual.camera.clone()
                }))
            } else {
                None
            }
        } else {
            None
        };
        let requested_proposal = object
            .get("proposal")
            .filter(|value| !value.is_null())
            .or(automatic_parameter_patch.as_ref());

        if let Some(requested_proposal) = requested_proposal {
            if matches!(
                runtime_parameter_patch_strategy(action),
                Ok("rear-stock-profile-reconstruction-v1")
            ) && !is_runtime_parameter_patch_proposal(requested_proposal)
            {
                return persist_blocked_run(
                    self,
                    run,
                    "evaluate",
                    Some("render"),
                    "repair-proposal",
                    "ACTION_STOCK_PROFILE_CALLER_PROGRAM_FORBIDDEN",
                );
            }
            let proposal = if is_runtime_parameter_patch_proposal(requested_proposal) {
                match materialize_runtime_parameter_patch_proposal(
                    requested_proposal,
                    action,
                    &session,
                    &candidate,
                    &geometry,
                    &visual,
                    requested_stage,
                ) {
                    Ok(value) => value,
                    Err(error) => {
                        let reason = stable_proposal_failure_code(&error);
                        return persist_blocked_run(
                            self,
                            run,
                            "evaluate",
                            Some("render"),
                            "repair-proposal",
                            &reason,
                        );
                    }
                }
            } else {
                requested_proposal.clone()
            };
            return match execute_bounded_repair_proposal(
                self,
                run.clone(),
                object,
                action,
                &proposal,
                &session,
                &candidate,
                &geometry,
                &visual,
                requested_stage,
            ) {
                Ok(result) => Ok(result),
                Err(error) => {
                    let reason = stable_proposal_failure_code(&error);
                    persist_blocked_run(
                        self,
                        run,
                        "evaluate",
                        Some("render"),
                        "repair-proposal",
                        &reason,
                    )
                }
            };
        }

        let visual_passed = visual.quality_status == "PARTIAL_VISIBLE_VIEW_PASS";
        let checkpoint = match self.prepare_action_checkpoint(
            object,
            &session,
            &candidate,
            requested_stage,
            &geometry,
            visual_passed,
        ) {
            Ok(checkpoint) => checkpoint,
            Err(_) => {
                return persist_blocked_run(
                    self,
                    run,
                    "evaluate",
                    Some("render"),
                    "checkpoint",
                    "checkpoint-prepare-failed",
                )
            }
        };

        run["status"] = Value::String("completed".to_owned());
        run["completed_stage"] = Value::String("evaluate".to_owned());
        run["quality_status"] = Value::String(visual.quality_status);
        run["failed_gates"] = if visual_passed {
            json!([])
        } else {
            json!(["visible-view"])
        };
        run["allowed_actions"] = if visual_passed {
            json!(["checkpoint", "inspect", "retry"])
        } else {
            json!(["inspect", "retry"])
        };
        run["locked_actions"] = if visual_passed {
            json!(["confirm", "export"])
        } else {
            json!(["confirm", "export", "next-stage"])
        };
        run["checkpoint_id"] = checkpoint
            .get("checkpoint")
            .and_then(|value| value.get("checkpoint_id"))
            .cloned()
            .unwrap_or(Value::Null);
        run["checkpoint_hash"] = checkpoint
            .get("checkpoint")
            .and_then(|value| value.get("canonical_sha256"))
            .cloned()
            .unwrap_or(Value::Null);
        finalize_run(&mut run);
        persist_run(self, &run)
    }

    fn prepare_action_checkpoint(
        &self,
        request: &Map<String, Value>,
        session: &AgenticSessionRecord,
        candidate: &CandidateRecord,
        stage: &str,
        geometry: &GeometryBindings,
        visual_passed: bool,
    ) -> Result<Value, RuntimeError> {
        let reference = self
            .reference(&session.reference_id)?
            .ok_or_else(|| RuntimeError::InvalidInput("AGENTIC_REFERENCE_NOT_FOUND".to_owned()))?;
        if reference.project_id != session.project_id
            || reference.object_sha256 != session.reference_sha256
        {
            return Err(RuntimeError::InvalidInput(
                "AGENTIC_REFERENCE_BINDING_MISMATCH".to_owned(),
            ));
        }
        let checkpoint_key = canonical_json_hash(&json!({
            "run_id": request.get("run_id"),
            "candidate_id": candidate.candidate_id,
            "input_sha256": request.get("input_sha256"),
        }));
        let checkpoint_request = json!({
            "session_id": session.session_id,
            "project_id": session.project_id,
            "candidate_id": candidate.candidate_id,
            "visual_state": if visual_passed { "pass" } else { "fail" },
            "evidence_sha256": session.evidence_sha256,
            "stage": stage,
            "checkpoint_type": if visual_passed { "stage-pass" } else { "stage-fail" },
            "candidate_state_sha256": candidate.canonical_sha256,
            "artifact_sha256": geometry.artifact_sha256,
            "reference_id": reference.reference_id,
            "reference_sha256": reference.object_sha256,
            "camera_hash": session.camera_hash,
            "idempotency_key": format!("action-checkpoint-{}", &checkpoint_key[..32]),
            "approved": request.get("approved").cloned().unwrap_or(Value::Bool(false)),
            "approval_receipt_id": request.get("approval_receipt_id").cloned().unwrap_or(Value::Null),
            "approval_summary": request.get("approval_summary").cloned().unwrap_or(Value::Null),
            "approval_expires_at": request.get("approval_expires_at").cloned().unwrap_or(Value::Null),
        });
        self.checkpoint_prepare(checkpoint_request)
    }
}

fn initial_run(
    run_id: &str,
    session_id: &str,
    project_id: &str,
    candidate_id: &str,
    session: &AgenticSessionRecord,
    input_sha256: &str,
    action: &Value,
    requested_stage: &str,
    observation_sha256: &str,
) -> Value {
    json!({
        "schema_version":"DesignActionRun@1",
        "run_id":run_id,
        "session_id":session_id,
        "project_id":project_id,
        "candidate_id":candidate_id,
        "reference_id":session.reference_id,
        "reference_sha256":session.reference_sha256,
        "camera_hash":session.camera_hash,
        "input_sha256":input_sha256,
        "observation_sha256":observation_sha256,
        "action":action,
        "requested_stage":requested_stage,
        "status":"running",
        "completed_stage":Value::Null,
        "stage_results":initial_stage_results(),
        "quality_status":"not-run",
        "failed_gates":[],
        "allowed_actions":["inspect","retry"],
        "locked_actions":["confirm","export","next-stage"],
        "checkpoint_id":Value::Null,
        "checkpoint_hash":Value::Null,
        "optimization_job_id":Value::Null,
        "optimization_intent_sha256":Value::Null,
        "runtime_write":false,
        "persistent_user_data_touched":false,
        "canonical_sha256":""
    })
}

fn initial_stage_results() -> Value {
    let mut stages = Map::new();
    for stage in PIPELINE_STAGES {
        stages.insert(
            stage.to_owned(),
            stage_result("not-run", None, Some("not-reached")),
        );
    }
    Value::Object(stages)
}

fn stage_result(status: &str, hash: Option<&str>, reason: Option<&str>) -> Value {
    json!({
        "status":status,
        "hash":hash.map(|value| Value::String(value.to_owned())).unwrap_or(Value::Null),
        "reason":reason.map(|value| Value::String(value.to_owned())).unwrap_or(Value::Null)
    })
}

fn set_stage_completed(run: &mut Value, stage: &str, hash: &str) {
    run["stage_results"][stage] = stage_result("completed", Some(hash), None);
    run["completed_stage"] = Value::String(stage.to_owned());
}

fn persist_blocked_run(
    runtime: &Runtime,
    mut run: Value,
    stage: &str,
    completed_stage: Option<&str>,
    gate: &str,
    reason: &str,
) -> Result<Value, RuntimeError> {
    run["status"] = Value::String("blocked".to_owned());
    run["completed_stage"] = completed_stage
        .map(|value| Value::String(value.to_owned()))
        .unwrap_or(Value::Null);
    run["stage_results"][stage] = stage_result("blocked", None, Some(reason));
    run["quality_status"] = Value::String("unknown".to_owned());
    run["failed_gates"] = json!([gate]);
    run["allowed_actions"] = json!(["inspect", "retry"]);
    run["locked_actions"] = json!(["confirm", "export", "next-stage"]);
    run["checkpoint_id"] = Value::Null;
    run["checkpoint_hash"] = Value::Null;
    finalize_run(&mut run);
    persist_run(runtime, &run)
}

fn persist_reference_request_run(runtime: &Runtime, mut run: Value) -> Result<Value, RuntimeError> {
    run["status"] = Value::String("blocked".to_owned());
    run["completed_stage"] = Value::Null;
    run["stage_results"]["prepare"] =
        stage_result("blocked", None, Some("reference-coverage-requested"));
    run["quality_status"] = Value::String("BLOCKED_REFERENCE_COVERAGE".to_owned());
    run["failed_gates"] = json!(["reference-coverage"]);
    run["allowed_actions"] = json!(["request-reference", "inspect", "retry"]);
    run["locked_actions"] = json!(["confirm", "export", "next-stage"]);
    run["checkpoint_id"] = Value::Null;
    run["checkpoint_hash"] = Value::Null;
    run["optimization_job_id"] = Value::Null;
    run["optimization_intent_sha256"] = Value::Null;
    run["runtime_write"] = Value::Bool(false);
    run["persistent_user_data_touched"] = Value::Bool(false);
    finalize_run(&mut run);
    persist_run(runtime, &run)
}

/// Start the CADFit-style optimizer as an explicit child of one ActionRun.
/// The child is intentionally asynchronous: the immutable ActionRun records
/// the exact intent and Job id, while `optimization_job_get` remains the live
/// source for progress/result.  This prevents a queued search from being
/// mistaken for a completed quality gate.
fn prepare_action_optimization(
    runtime: &Runtime,
    request: &Map<String, Value>,
    intent: &Value,
    run_id: &str,
    requested_stage: &str,
    action: &Value,
    candidate: &CandidateRecord,
) -> Result<Value, RuntimeError> {
    let intent_object = intent.as_object().ok_or_else(|| {
        RuntimeError::InvalidInput(
            "ACTION_OPTIMIZATION_INTENT_INVALID: intent must be an object".to_owned(),
        )
    })?;
    let project_id = required_id(request, "project_id")?;
    let candidate_id = required_id(request, "candidate_id")?;
    let session_id = required_id(request, "session_id")?;
    if candidate.candidate_id != candidate_id || candidate.project_id != project_id {
        return Err(RuntimeError::InvalidInput(
            "ACTION_OPTIMIZATION_CANDIDATE_SCOPE_DENIED".to_owned(),
        ));
    }
    if intent_object.get("action_run_id").and_then(Value::as_str) != Some(run_id) {
        return Err(RuntimeError::InvalidInput(
            "ACTION_OPTIMIZATION_ACTION_RUN_BINDING_MISMATCH".to_owned(),
        ));
    }
    if intent_object.get("project_id").and_then(Value::as_str) != Some(project_id)
        || intent_object.get("candidate_id").and_then(Value::as_str) != Some(candidate_id)
        || intent_object.get("stage").and_then(Value::as_str) != Some(requested_stage)
    {
        return Err(RuntimeError::InvalidInput(
            "ACTION_OPTIMIZATION_SCOPE_OR_STAGE_MISMATCH".to_owned(),
        ));
    }
    let action_object = action.as_object().ok_or_else(|| {
        RuntimeError::InvalidInput("ACTION_OPTIMIZATION_ACTION_INVALID".to_owned())
    })?;
    if action_object.get("scope_kind").and_then(Value::as_str) != Some("part")
        || !matches!(
            action_object.get("action_kind").and_then(Value::as_str),
            Some(
                "primary-blockout"
                    | "primary-form-adjustment"
                    | "secondary-structure"
                    | "tertiary-detail"
                    | "bounded-repair"
            )
        )
    {
        return Err(RuntimeError::InvalidInput(
            "ACTION_OPTIMIZATION_ACTION_KIND_UNSUPPORTED".to_owned(),
        ));
    }
    let target_part_id = action_object
        .get("target_id")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            RuntimeError::InvalidInput("ACTION_OPTIMIZATION_PART_TARGET_REQUIRED".to_owned())
        })?;
    if intent_object.get("part_id").and_then(Value::as_str) != Some(target_part_id) {
        return Err(RuntimeError::InvalidInput(
            "ACTION_OPTIMIZATION_PART_BINDING_MISMATCH".to_owned(),
        ));
    }

    // MCP has an equivalent wire rebind for standalone optimization calls.
    // ActionRun invokes the Runtime directly, so repeat that normalization at
    // this boundary: verify the supplied outer hash, then make the nested
    // camera and resulting Intent canonical under Runtime's serializer.
    let rebound_intent = canonicalize_action_optimization_intent(intent)?;

    let child_request = json!({
        "project_id":project_id,
        "candidate_id":candidate_id,
        "intent":rebound_intent,
        "approved":true,
        "approval_receipt_id":request.get("approval_receipt_id").cloned().unwrap_or(Value::Null),
        "approval_summary":request.get("approval_summary").cloned().unwrap_or(Value::Null),
        "approval_expires_at":request.get("approval_expires_at").cloned().unwrap_or(Value::Null),
        "approval_session_id":session_id,
        "idempotency_key":format!(
            "action-optimization-{}",
            &canonical_json_hash(&json!({"run_id":run_id,"intent":rebound_intent}))[..32]
        )
    });
    let child = runtime.optimization_job_prepare(child_request)?;
    let job_id = child
        .get("job")
        .and_then(|job| job.get("job_id"))
        .and_then(Value::as_str)
        .filter(|value| is_opaque_id(value))
        .ok_or_else(|| {
            RuntimeError::InvalidInput(
                "ACTION_OPTIMIZATION_JOB_READBACK_MISSING: job_id".to_owned(),
            )
        })?;
    let intent_sha256 = child
        .get("intent_sha256")
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .ok_or_else(|| {
            RuntimeError::InvalidInput(
                "ACTION_OPTIMIZATION_JOB_READBACK_MISSING: intent_sha256".to_owned(),
            )
        })?;
    Ok(json!({"job_id":job_id,"intent_sha256":intent_sha256}))
}

fn canonicalize_action_optimization_intent(intent: &Value) -> Result<Value, RuntimeError> {
    let object = intent.as_object().ok_or_else(|| {
        RuntimeError::InvalidInput(
            "ACTION_OPTIMIZATION_INTENT_INVALID: intent must be an object".to_owned(),
        )
    })?;
    let supplied = object
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .ok_or_else(|| {
            RuntimeError::InvalidInput(
                "ACTION_OPTIMIZATION_INTENT_CANONICAL_HASH_MISSING".to_owned(),
            )
        })?;
    let mut wire = intent.clone();
    wire["canonical_sha256"] = Value::String(String::new());
    let normalized_wire = super::normalize_json_numbers(&wire);
    if supplied != canonical_json_hash(&wire) && supplied != canonical_json_hash(&normalized_wire) {
        return Err(RuntimeError::InvalidInput(
            "ACTION_OPTIMIZATION_INTENT_CANONICAL_HASH_MISMATCH".to_owned(),
        ));
    }
    let camera = object.get("camera").ok_or_else(|| {
        RuntimeError::InvalidInput("ACTION_OPTIMIZATION_CAMERA_MISSING".to_owned())
    })?;
    let mut rebound_camera = normalize_action_optimization_camera(camera);
    {
        let camera_object = rebound_camera.as_object_mut().ok_or_else(|| {
            RuntimeError::InvalidInput("ACTION_OPTIMIZATION_CAMERA_INVALID".to_owned())
        })?;
        camera_object.insert("camera_hash".to_owned(), Value::String(String::new()));
        camera_object.insert("canonical_sha256".to_owned(), Value::String(String::new()));
    }
    let camera_hash = canonical_json_hash(&rebound_camera);
    {
        let camera_object = rebound_camera.as_object_mut().ok_or_else(|| {
            RuntimeError::InvalidInput("ACTION_OPTIMIZATION_CAMERA_INVALID".to_owned())
        })?;
        camera_object.insert("camera_hash".to_owned(), Value::String(camera_hash.clone()));
    }
    let camera_canonical_sha256 = canonical_json_hash(&rebound_camera);
    {
        let camera_object = rebound_camera.as_object_mut().ok_or_else(|| {
            RuntimeError::InvalidInput("ACTION_OPTIMIZATION_CAMERA_INVALID".to_owned())
        })?;
        camera_object.insert(
            "canonical_sha256".to_owned(),
            Value::String(camera_canonical_sha256),
        );
    }
    let mut rebound_intent = intent.clone();
    rebound_intent["camera"] = rebound_camera;
    rebound_intent["camera_hash"] = Value::String(camera_hash);
    rebound_intent["canonical_sha256"] = Value::String(String::new());
    let rebound_hash = canonical_json_hash(&rebound_intent);
    rebound_intent["canonical_sha256"] = Value::String(rebound_hash);
    Ok(rebound_intent)
}

fn normalize_action_optimization_camera(value: &Value) -> Value {
    match value {
        Value::Number(number) => number
            .as_f64()
            .and_then(serde_json::Number::from_f64)
            .map(Value::Number)
            .unwrap_or_else(|| value.clone()),
        Value::Array(values) => Value::Array(
            values
                .iter()
                .map(normalize_action_optimization_camera)
                .collect(),
        ),
        Value::Object(object) => Value::Object(
            object
                .iter()
                .map(|(key, child)| {
                    let normalized = if key == "resolution" {
                        child.clone()
                    } else {
                        normalize_action_optimization_camera(child)
                    };
                    (key.clone(), normalized)
                })
                .collect(),
        ),
        _ => value.clone(),
    }
}

/// Persist a useful failure class without copying Runtime details, paths or
/// user-authored text into a durable DesignActionRun receipt.
fn stable_proposal_failure_code(error: &RuntimeError) -> String {
    let detail = error.to_string();
    if detail.contains("DesignActionRun proposal must be an object") {
        return "STORE_INVALID_DATA_PROPOSAL_OBJECT".to_owned();
    }
    if detail.contains("DesignActionRun proposal has missing or unknown fields") {
        return "STORE_INVALID_DATA_PROPOSAL_FIELDS".to_owned();
    }
    if detail.contains("DesignActionRun proposal ") {
        return "STORE_INVALID_DATA_PROPOSAL".to_owned();
    }
    // InvalidInput is rendered as `invalid runtime input: <stable-code>` by
    // RuntimeError. Strip that wrapper before extracting the bounded code;
    // otherwise every useful REPAIR_/ACTION_ failure collapses to the generic
    // `repair-proposal-failed` receipt reason.
    let code_detail = detail
        .strip_prefix("invalid runtime input: ")
        .unwrap_or(detail.as_str());
    if let Some(suffix) = code_detail
        .split_once("CONTRACT_OUTPUT_INVALID:")
        .map(|(_, value)| value)
    {
        let suffix = suffix
            .chars()
            .filter(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | '@' | '.')
            })
            .take(72)
            .collect::<String>();
        if !suffix.is_empty() {
            return format!("CONTRACT_OUTPUT_INVALID_{suffix}");
        }
    }
    let code = code_detail.split(':').next().unwrap_or_default().trim();
    let allowed_prefix = [
        "ACTION_",
        "AGENTIC_",
        "ASSEMBLY_",
        "CAMERA_",
        "CONTRACT_",
        "CROSS_VIEW_",
        "GEOMETRY_",
        "REPAIR_",
        "REFERENCE_",
        "STORE_",
    ];
    if !code.is_empty()
        && code.len() <= 96
        && allowed_prefix.iter().any(|prefix| code.starts_with(prefix))
        && code.bytes().all(|byte| {
            byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_' || byte == b'-'
        })
    {
        code.to_owned()
    } else {
        "repair-proposal-failed".to_owned()
    }
}

/// Preserve only a bounded machine-readable child-job failure class in the
/// parent ActionRun.  Runtime errors may contain implementation detail, so a
/// queued design receipt must never copy them into durable evidence.
fn stable_optimization_failure_code(error: &RuntimeError) -> String {
    let detail = error.to_string();
    let allowed_prefix = [
        "ACTION_OPTIMIZATION_",
        "OPTIMIZATION_",
        "CAMERA_",
        "CONTRACT_",
        "GEOMETRY_",
        "REFERENCE_",
        "SILHOUETTE_",
        "STORE_",
    ];
    if let Some(suffix) = detail.split("CONTRACT_OUTPUT_INVALID:").nth(1) {
        let suffix = suffix
            .chars()
            .filter(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | '@' | '.')
            })
            .take(72)
            .collect::<String>();
        if !suffix.is_empty() {
            return format!("CONTRACT_OUTPUT_INVALID_{suffix}");
        }
    }
    if let Some(suffix) = detail.split("CAMERA_CALIBRATION_INVALID:").nth(1) {
        let suffix = suffix
            .chars()
            .filter(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '-'))
            .take(72)
            .collect::<String>();
        if !suffix.is_empty() {
            return format!("CAMERA_CALIBRATION_INVALID_{suffix}");
        }
    }
    for code in detail.split(|character: char| {
        !character.is_ascii_alphanumeric() && character != '_' && character != '-'
    }) {
        if code.len() <= 96
            && allowed_prefix.iter().any(|prefix| code.starts_with(prefix))
            && code.bytes().all(|byte| {
                byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_' || byte == b'-'
            })
        {
            return code.to_owned();
        }
    }
    "optimization-job-prepare-failed".to_owned()
}

fn persist_run(runtime: &Runtime, run: &Value) -> Result<Value, RuntimeError> {
    let bytes =
        canonical_json_bytes(run).map_err(|error| RuntimeError::InvalidInput(error.to_string()))?;
    let object = runtime.put_object(
        &bytes,
        None,
        "application/json",
        "agentic-design-action-run",
    )?;
    Ok(runtime
        .store
        .design_action_run_create_or_resume(run, &object.record, &now_string())?)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum RuntimeParameterSemantic {
    Size(usize),
    Position(usize),
    Rotation(usize),
    Radius,
    OuterRadius,
    InnerRadius,
    Thickness,
    Bevel,
    RearStockInnerReceiverDeltaY,
    RearStockInnerCapDeltaY,
    RearStockReceiverInnerXDelta,
    RearStockCapInnerXDelta,
    RearStockDepthCenterInnerDeltaY,
    SurfaceControlPoint { index: usize, axis: usize },
}

fn is_runtime_parameter_patch_proposal(value: &Value) -> bool {
    value
        .as_object()
        .is_some_and(|object| object.contains_key("parameter_patch"))
}

fn runtime_parameter_semantic(parameter_id: &str) -> Option<RuntimeParameterSemantic> {
    if let Some(control_point) = parameter_id.strip_prefix("control-point-") {
        let mut parts = control_point.split('-');
        let index = parts.next()?.parse::<usize>().ok()?;
        let axis = match parts.next()? {
            "x" => 0,
            "y" => 1,
            "z" => 2,
            _ => return None,
        };
        if parts.next().is_none() {
            return Some(RuntimeParameterSemantic::SurfaceControlPoint { index, axis });
        }
    }
    let matches =
        |suffix: &str| parameter_id == suffix || parameter_id.ends_with(&format!("-{suffix}"));
    if parameter_id == "rear-stock-inner-receiver-delta-y" {
        Some(RuntimeParameterSemantic::RearStockInnerReceiverDeltaY)
    } else if parameter_id == "rear-stock-inner-cap-delta-y" {
        Some(RuntimeParameterSemantic::RearStockInnerCapDeltaY)
    } else if parameter_id == "rear-stock-receiver-inner-x-delta" {
        Some(RuntimeParameterSemantic::RearStockReceiverInnerXDelta)
    } else if parameter_id == "rear-stock-cap-inner-x-delta" {
        Some(RuntimeParameterSemantic::RearStockCapInnerXDelta)
    } else if parameter_id == "rear-stock-depth-center-inner-delta-y" {
        Some(RuntimeParameterSemantic::RearStockDepthCenterInnerDeltaY)
    } else if matches("width") {
        Some(RuntimeParameterSemantic::Size(0))
    } else if matches("height") {
        Some(RuntimeParameterSemantic::Size(1))
    } else if matches("depth") {
        Some(RuntimeParameterSemantic::Size(2))
    } else if matches("offset-x") {
        Some(RuntimeParameterSemantic::Position(0))
    } else if matches("offset-y") {
        Some(RuntimeParameterSemantic::Position(1))
    } else if matches("offset-z") {
        Some(RuntimeParameterSemantic::Position(2))
    } else if matches("rotation-x") {
        Some(RuntimeParameterSemantic::Rotation(0))
    } else if matches("rotation-y") {
        Some(RuntimeParameterSemantic::Rotation(1))
    } else if matches("rotation-z") {
        Some(RuntimeParameterSemantic::Rotation(2))
    } else if matches("outer-radius") {
        Some(RuntimeParameterSemantic::OuterRadius)
    } else if matches("inner-radius") {
        Some(RuntimeParameterSemantic::InnerRadius)
    } else if matches("radius") {
        Some(RuntimeParameterSemantic::Radius)
    } else if matches("thickness") {
        Some(RuntimeParameterSemantic::Thickness)
    } else if matches("bevel") {
        Some(RuntimeParameterSemantic::Bevel)
    } else {
        None
    }
}

fn runtime_parameter_strategy(semantic: RuntimeParameterSemantic) -> &'static str {
    match semantic {
        RuntimeParameterSemantic::RearStockInnerReceiverDeltaY
        | RuntimeParameterSemantic::RearStockInnerCapDeltaY
        | RuntimeParameterSemantic::RearStockReceiverInnerXDelta
        | RuntimeParameterSemantic::RearStockCapInnerXDelta
        | RuntimeParameterSemantic::RearStockDepthCenterInnerDeltaY => {
            "rear-stock-profile-reconstruction-v1"
        }
        RuntimeParameterSemantic::SurfaceControlPoint { .. } => "surface-control-points-v1",
        RuntimeParameterSemantic::Thickness | RuntimeParameterSemantic::Bevel => {
            "hard-surface-finish-v1"
        }
        _ => "primitive-dimensions-v1",
    }
}

/// Select the only Runtime-owned parameter patch family that can represent a
/// high-level DesignActionRun.  The action may contain several changes, but
/// they must all belong to the same semantic family; mixed primitive/surface
/// patches would make the one-node/one-Part boundary ambiguous.
fn runtime_parameter_patch_strategy(action: &Value) -> Result<&'static str, RuntimeError> {
    let changes = action
        .get("parameter_changes")
        .and_then(Value::as_array)
        .filter(|changes| !changes.is_empty())
        .ok_or_else(|| {
            RuntimeError::InvalidInput("REPAIR_PARAMETER_CHANGES_REQUIRED".to_owned())
        })?;
    let mut family = None;
    let mut seen = HashSet::new();
    for change in changes {
        let parameter_id = change
            .get("parameter_id")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                RuntimeError::InvalidInput(
                    "ACTION_PARAMETER_PATCH_INVALID: parameter_id".to_owned(),
                )
            })?;
        let semantic = runtime_parameter_semantic(parameter_id).ok_or_else(|| {
            RuntimeError::InvalidInput(format!(
                "ACTION_PARAMETER_PATCH_UNSUPPORTED: parameter_id {parameter_id}"
            ))
        })?;
        if !seen.insert(semantic) {
            return Err(RuntimeError::InvalidInput(
                "ACTION_PARAMETER_PATCH_INVALID: duplicate semantic parameter".to_owned(),
            ));
        }
        let semantic_family = runtime_parameter_strategy(semantic);
        if let Some(previous) = family {
            if previous != semantic_family {
                return Err(RuntimeError::InvalidInput(
                    "ACTION_PARAMETER_PATCH_STRATEGY_PARAMETER_MISMATCH".to_owned(),
                ));
            }
        } else {
            family = Some(semantic_family);
        }
    }
    family.ok_or_else(|| RuntimeError::InvalidInput("REPAIR_PARAMETER_CHANGES_REQUIRED".to_owned()))
}

fn runtime_parameter_node_supports(
    node: &Map<String, Value>,
    semantic: RuntimeParameterSemantic,
) -> bool {
    let operator_id = node.get("operator_id").and_then(Value::as_str);
    let primitive_or_panel = matches!(
        operator_id,
        Some("forgecad.geometry.primitive@2" | "forgecad.geometry.panel@1")
    );
    let surface_operator = matches!(
        operator_id,
        Some(
            "forgecad.geometry.subd-cage@1"
                | "forgecad.geometry.subd-cage@2"
                | "forgecad.geometry.surface-patch@1"
                | "forgecad.geometry.surface-shell@1"
        )
    );
    let panel_operator = operator_id == Some("forgecad.geometry.panel@1");
    let energy_core_operator = operator_id == Some("forgecad.geometry.energy-core@1");
    let Some(parameters) = node.get("parameters").and_then(Value::as_object) else {
        return false;
    };
    match semantic {
        RuntimeParameterSemantic::RearStockInnerReceiverDeltaY
        | RuntimeParameterSemantic::RearStockInnerCapDeltaY
        | RuntimeParameterSemantic::RearStockReceiverInnerXDelta
        | RuntimeParameterSemantic::RearStockCapInnerXDelta
        | RuntimeParameterSemantic::RearStockDepthCenterInnerDeltaY => false,
        RuntimeParameterSemantic::Size(2) if energy_core_operator => {
            parameters.get("depth_m").and_then(Value::as_f64).is_some()
        }
        RuntimeParameterSemantic::Size(index) if primitive_or_panel => parameters
            .get("size_m")
            .and_then(Value::as_array)
            .is_some_and(|values| values.len() == 3 && values[index].as_f64().is_some()),
        RuntimeParameterSemantic::Position(index)
            if primitive_or_panel || surface_operator || energy_core_operator =>
        {
            parameters
                .get("position_m")
                .and_then(Value::as_array)
                .is_some_and(|values| values.len() == 3 && values[index].as_f64().is_some())
        }
        RuntimeParameterSemantic::Rotation(index)
            if primitive_or_panel || surface_operator || energy_core_operator =>
        {
            parameters
                .get("rotation_rad")
                .and_then(Value::as_array)
                .is_some_and(|values| values.len() == 3 && values[index].as_f64().is_some())
        }
        RuntimeParameterSemantic::Radius if primitive_or_panel => {
            parameters.get("radius_m").and_then(Value::as_f64).is_some()
        }
        RuntimeParameterSemantic::OuterRadius if energy_core_operator => parameters
            .get("outer_radius_m")
            .and_then(Value::as_f64)
            .is_some(),
        RuntimeParameterSemantic::InnerRadius if energy_core_operator => parameters
            .get("inner_radius_m")
            .and_then(Value::as_f64)
            .is_some(),
        RuntimeParameterSemantic::Thickness
            if panel_operator || operator_id == Some("forgecad.geometry.surface-shell@1") =>
        {
            parameters
                .get("thickness_m")
                .and_then(Value::as_f64)
                .is_some()
        }
        RuntimeParameterSemantic::Bevel if panel_operator => {
            parameters.get("bevel_m").and_then(Value::as_f64).is_some()
        }
        RuntimeParameterSemantic::SurfaceControlPoint { index, axis } if surface_operator => {
            parameters
                .get("control_points")
                .and_then(Value::as_array)
                .and_then(|points| points.get(index))
                .and_then(Value::as_array)
                .is_some_and(|point| point.len() == 3 && point[axis].as_f64().is_some())
        }
        _ => false,
    }
}

fn runtime_parameter_value_in_bounds(semantic: RuntimeParameterSemantic, value: f64) -> bool {
    if !value.is_finite() {
        return false;
    }
    match semantic {
        RuntimeParameterSemantic::Size(_) => value > 0.0 && value <= 10.0,
        RuntimeParameterSemantic::Position(_) => (-10.0..=10.0).contains(&value),
        RuntimeParameterSemantic::Rotation(_) => {
            (-2.0 * std::f64::consts::PI..=2.0 * std::f64::consts::PI).contains(&value)
        }
        RuntimeParameterSemantic::Radius => value > 0.0 && value <= 5.0,
        RuntimeParameterSemantic::OuterRadius => value > 0.0 && value <= 5.0,
        RuntimeParameterSemantic::InnerRadius => (0.0..=5.0).contains(&value),
        RuntimeParameterSemantic::Thickness => value > 0.0 && value <= 10.0,
        RuntimeParameterSemantic::Bevel => value >= 0.0 && value <= 5.0,
        RuntimeParameterSemantic::RearStockInnerReceiverDeltaY
        | RuntimeParameterSemantic::RearStockInnerCapDeltaY => (0.0..=0.07).contains(&value),
        RuntimeParameterSemantic::RearStockReceiverInnerXDelta
        | RuntimeParameterSemantic::RearStockCapInnerXDelta => (-0.01..=0.01).contains(&value),
        RuntimeParameterSemantic::RearStockDepthCenterInnerDeltaY => (0.0..=0.01).contains(&value),
        RuntimeParameterSemantic::SurfaceControlPoint { .. } => (-10.0..=10.0).contains(&value),
    }
}

fn runtime_parameter_unit_allowed(semantic: RuntimeParameterSemantic, unit: &str) -> bool {
    match semantic {
        RuntimeParameterSemantic::Rotation(_) => matches!(unit, "radian" | "ratio"),
        RuntimeParameterSemantic::Size(_)
        | RuntimeParameterSemantic::Position(_)
        | RuntimeParameterSemantic::Radius
        | RuntimeParameterSemantic::OuterRadius
        | RuntimeParameterSemantic::InnerRadius
        | RuntimeParameterSemantic::Thickness
        | RuntimeParameterSemantic::Bevel
        | RuntimeParameterSemantic::RearStockInnerReceiverDeltaY
        | RuntimeParameterSemantic::RearStockInnerCapDeltaY
        | RuntimeParameterSemantic::RearStockReceiverInnerXDelta
        | RuntimeParameterSemantic::RearStockCapInnerXDelta
        | RuntimeParameterSemantic::RearStockDepthCenterInnerDeltaY
        | RuntimeParameterSemantic::SurfaceControlPoint { .. } => {
            matches!(unit, "meter" | "ratio")
        }
    }
}

fn runtime_parameter_relationship_valid(
    operator_id: &str,
    parameters: &Map<String, Value>,
    semantic: RuntimeParameterSemantic,
    proposed: f64,
) -> bool {
    if operator_id == "forgecad.geometry.energy-core@1" {
        let outer_radius_m = parameters
            .get("outer_radius_m")
            .and_then(Value::as_f64)
            .filter(|value| value.is_finite());
        let inner_radius_m = parameters
            .get("inner_radius_m")
            .and_then(Value::as_f64)
            .filter(|value| value.is_finite());
        let component = parameters.get("component").and_then(Value::as_str);
        let (Some(mut outer_radius_m), Some(mut inner_radius_m), Some(component)) =
            (outer_radius_m, inner_radius_m, component)
        else {
            return false;
        };
        match semantic {
            RuntimeParameterSemantic::OuterRadius => outer_radius_m = proposed,
            RuntimeParameterSemantic::InnerRadius => inner_radius_m = proposed,
            _ => return true,
        }
        if !(outer_radius_m > 0.0
            && inner_radius_m >= 0.0
            && inner_radius_m < outer_radius_m - 1.0e-5)
        {
            return false;
        }
        return match component {
            "guard-ring" | "mechanical-ring" => inner_radius_m > 1.0e-5,
            "emitter-core" | "mechanical-backplate" => inner_radius_m == 0.0,
            _ => false,
        };
    }
    if operator_id != "forgecad.geometry.panel@1" {
        return true;
    }
    let size = parameters
        .get("size_m")
        .and_then(Value::as_array)
        .and_then(|values| {
            (values.len() == 3)
                .then(|| [values[0].as_f64(), values[1].as_f64(), values[2].as_f64()])
        });
    let Some([Some(width), Some(height), Some(depth)]) = size else {
        return false;
    };
    match semantic {
        RuntimeParameterSemantic::Thickness => proposed <= depth,
        RuntimeParameterSemantic::Bevel => proposed * 2.0 < width.min(height),
        _ => true,
    }
}

fn runtime_parameter_number(value: f64, label: &str) -> Result<Value, RuntimeError> {
    serde_json::Number::from_f64(value)
        .map(Value::Number)
        .ok_or_else(|| {
            RuntimeError::InvalidInput(format!(
                "ACTION_PARAMETER_PATCH_INVALID: {label} is not finite"
            ))
        })
}

/// Bind a proposal's camera to the Runtime-owned source visual evidence.
///
/// CameraCalibration@1 contains floating-point payloads, so a JSON client can
/// change the textual representation while preserving the same camera. The
/// source RenderSet carries the exact CAS-backed calibration selected by the
/// comparison stage; proposals may therefore submit either that full value
/// or its compact CameraCalibrationRef, but Runtime always executes the
/// evidence-bound calibration and never trusts a caller's re-serialized
/// camera bytes as a new camera.
fn bind_repair_camera(
    proposal: &Value,
    session: &AgenticSessionRecord,
    source_visual: &VisualBindings,
) -> Result<Value, RuntimeError> {
    let object = proposal.as_object().ok_or_else(|| {
        RuntimeError::InvalidInput("REPAIR_PROPOSAL_INVALID: proposal must be an object".to_owned())
    })?;
    let input_camera = object
        .get("camera")
        .ok_or_else(|| RuntimeError::InvalidInput("REPAIR_CAMERA_REQUIRED".to_owned()))?;
    let source_camera_hash = source_visual
        .camera
        .get("camera_hash")
        .and_then(Value::as_str)
        .filter(|hash| is_sha256(hash))
        .ok_or_else(|| RuntimeError::InvalidInput("CAMERA_EVIDENCE_UNAVAILABLE".to_owned()))?;
    if source_camera_hash != session.camera_hash.as_str() {
        return Err(RuntimeError::InvalidInput(
            "CAMERA_EVIDENCE_BINDING_MISMATCH_SOURCE".to_owned(),
        ));
    }
    let input_camera_hash = input_camera
        .get("camera_hash")
        .and_then(Value::as_str)
        .filter(|hash| is_sha256(hash))
        .ok_or_else(|| {
            RuntimeError::InvalidInput("REPAIR_CAMERA_BINDING_MISMATCH_MISSING_HASH".to_owned())
        })?;
    if input_camera_hash != session.camera_hash.as_str() {
        return Err(RuntimeError::InvalidInput(
            "REPAIR_CAMERA_BINDING_MISMATCH_INPUT".to_owned(),
        ));
    }
    let mut bound = proposal.clone();
    bound["camera"] = source_visual.camera.clone();
    Ok(bound)
}

fn materialize_rear_stock_profile_reconstruction_proposal(
    action_object: &Map<String, Value>,
    changes: &[Value],
    session: &AgenticSessionRecord,
    source_candidate: &CandidateRecord,
    source_geometry: &GeometryBindings,
    source_visual: &VisualBindings,
    requested_stage: &str,
    view_spec: &Value,
    camera: &Value,
) -> Result<Value, RuntimeError> {
    if action_object.get("target_id").and_then(Value::as_str) != Some("rear-stock")
        || action_object.get("operator_id").and_then(Value::as_str)
            != Some("forgecad.geometry.profile-loft@2")
    {
        return Err(RuntimeError::InvalidInput(
            "ACTION_STOCK_PROFILE_RECONSTRUCTION_SCOPE_MISMATCH".to_owned(),
        ));
    }
    let required = [
        ("rear-stock-inner-receiver-delta-y", 0.0_f64),
        ("rear-stock-inner-cap-delta-y", 0.0_f64),
        ("rear-stock-receiver-inner-x-delta", 0.0_f64),
        ("rear-stock-cap-inner-x-delta", 0.0_f64),
        ("rear-stock-depth-center-inner-delta-y", 0.0_f64),
    ];
    if changes.len() != required.len() {
        return Err(RuntimeError::InvalidInput(
            "ACTION_STOCK_PROFILE_RECONSTRUCTION_COMPLETE_CONTROL_SET_REQUIRED".to_owned(),
        ));
    }
    let mut after_values = HashMap::new();
    for change in changes {
        let object = change.as_object().ok_or_else(|| {
            RuntimeError::InvalidInput(
                "ACTION_STOCK_PROFILE_RECONSTRUCTION_INVALID_CHANGE".to_owned(),
            )
        })?;
        let parameter_id = object
            .get("parameter_id")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                RuntimeError::InvalidInput(
                    "ACTION_STOCK_PROFILE_RECONSTRUCTION_PARAMETER_REQUIRED".to_owned(),
                )
            })?;
        let Some((_, expected_before)) = required
            .iter()
            .find(|(required_id, _)| *required_id == parameter_id)
        else {
            return Err(RuntimeError::InvalidInput(format!(
                "ACTION_STOCK_PROFILE_RECONSTRUCTION_UNSUPPORTED_PARAMETER: {parameter_id}"
            )));
        };
        if object.get("unit").and_then(Value::as_str) != Some("meter") {
            return Err(RuntimeError::InvalidInput(
                "ACTION_STOCK_PROFILE_RECONSTRUCTION_METER_UNIT_REQUIRED".to_owned(),
            ));
        }
        let before = object
            .get("before")
            .and_then(Value::as_f64)
            .ok_or_else(|| {
                RuntimeError::InvalidInput(
                    "ACTION_STOCK_PROFILE_RECONSTRUCTION_BEFORE_REQUIRED".to_owned(),
                )
            })?;
        let tolerance = 1.0e-9 * expected_before.abs().max(1.0);
        if (before - expected_before).abs() > tolerance {
            return Err(RuntimeError::InvalidInput(
                "ACTION_STOCK_PROFILE_RECONSTRUCTION_STALE_BASELINE".to_owned(),
            ));
        }
        let after = object.get("after").and_then(Value::as_f64).ok_or_else(|| {
            RuntimeError::InvalidInput(
                "ACTION_STOCK_PROFILE_RECONSTRUCTION_AFTER_REQUIRED".to_owned(),
            )
        })?;
        let semantic = runtime_parameter_semantic(parameter_id).ok_or_else(|| {
            RuntimeError::InvalidInput(
                "ACTION_STOCK_PROFILE_RECONSTRUCTION_SEMANTIC_UNAVAILABLE".to_owned(),
            )
        })?;
        if runtime_parameter_strategy(semantic) != "rear-stock-profile-reconstruction-v1"
            || !runtime_parameter_value_in_bounds(semantic, after)
            || after_values.insert(parameter_id, after).is_some()
        {
            return Err(RuntimeError::InvalidInput(
                "ACTION_STOCK_PROFILE_RECONSTRUCTION_CONTROL_INVALID".to_owned(),
            ));
        }
    }
    let value = |parameter_id: &str| {
        after_values.get(parameter_id).copied().ok_or_else(|| {
            RuntimeError::InvalidInput(
                "ACTION_STOCK_PROFILE_RECONSTRUCTION_COMPLETE_CONTROL_SET_REQUIRED".to_owned(),
            )
        })
    };
    let mut program = production_weapon_stock_profile_reconstruction_mutate(
        &source_geometry.program,
        value("rear-stock-inner-receiver-delta-y")?,
        value("rear-stock-inner-cap-delta-y")?,
        value("rear-stock-receiver-inner-x-delta")?,
        value("rear-stock-cap-inner-x-delta")?,
        value("rear-stock-depth-center-inner-delta-y")?,
    )?;
    program
        .as_object_mut()
        .ok_or_else(|| RuntimeError::InvalidInput("REPAIR_PROGRAM_INVALID".to_owned()))?
        .remove("canonical_sha256");
    let hash_result = hash_geometry_program_with_runtime_worker(&program).map_err(|error| {
        RuntimeError::InvalidInput(format!("ACTION_PARAMETER_PATCH_HASH_FAILED: {error}"))
    })?;
    let proposed_program_sha256 = hash_result
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .ok_or_else(|| {
            RuntimeError::InvalidInput("ACTION_PARAMETER_PATCH_HASH_INVALID".to_owned())
        })?
        .to_owned();
    program["canonical_sha256"] = Value::String(proposed_program_sha256.clone());

    let seed = canonical_json_hash(&json!({
        "candidate_id": source_candidate.candidate_id,
        "action": Value::Object(action_object.clone()),
        "geometry_program_sha256": proposed_program_sha256,
    }));
    let critic_prefix = source_visual
        .quality_sha256
        .chars()
        .take(24)
        .collect::<String>();
    let mut intent = json!({
        "schema_version":"RepairIntent@1",
        "intent_id":format!("runtime-stock-profile-reconstruction-{}", &seed[..32]),
        "session_id":session.session_id,
        "project_id":session.project_id,
        "candidate_id":source_candidate.candidate_id,
        "candidate_state_sha256":source_candidate.canonical_sha256,
        "reference_id":session.reference_id,
        "reference_sha256":session.reference_sha256,
        "camera_hash":session.camera_hash,
        "observation_sha256":session.observation_sha256,
        "source_evidence_sha256":session.evidence_sha256,
        "source_critic_report_id":format!("critic-report-{critic_prefix}"),
        "source_critic_report_sha256":source_visual.quality_sha256,
        "stage":requested_stage,
        "scope":{"kind":"part","part_id":"rear-stock"},
        "action":{
            "action_kind":"bounded-repair",
            "kit_id":"forgecad.kit.frame@1",
            "operator_id":"forgecad.geometry.profile-loft@2",
            "operation":"rebuild-part",
            "parameter_changes":action_object["parameter_changes"].clone(),
            "bounded":true,
            "description":action_object["description"].clone()
        },
        "precondition":{
            "failed_gate_id":"visible-view",
            "quality_status":source_visual.quality_status,
            "current_candidate_state_sha256":source_candidate.canonical_sha256,
            "evidence_sha256":session.evidence_sha256,
            "status":"failed"
        },
        "recompute":{
            "steps":["compile","readback","render","compare"],
            "must_rebind_reference":true,
            "must_rebind_camera":true,
            "confirm_allowed":false
        },
        "rollback":{
            "relation":"none",
            "target_checkpoint_id":null,
            "target_checkpoint_sha256":null,
            "target_version_id":null,
            "target_version_sha256":null,
            "on_failure":"keep-current",
            "reason":null
        },
        "status":"approved",
        "approval_required":true,
        "runtime_write":false,
        "canonical_sha256":""
    });
    intent["canonical_sha256"] = Value::String(canonical_json_hash(&intent));
    Ok(json!({
        "repair_intent":intent,
        "geometry_program":program,
        "view_spec":view_spec,
        "camera":camera
    }))
}

fn materialize_runtime_parameter_patch_proposal(
    proposal: &Value,
    action: &Value,
    session: &AgenticSessionRecord,
    source_candidate: &CandidateRecord,
    source_geometry: &GeometryBindings,
    source_visual: &VisualBindings,
    requested_stage: &str,
) -> Result<Value, RuntimeError> {
    let proposal_object = proposal.as_object().ok_or_else(|| {
        RuntimeError::InvalidInput(
            "ACTION_PARAMETER_PATCH_INVALID: proposal must be an object".to_owned(),
        )
    })?;
    reject_unknown_keys(
        proposal_object,
        &["parameter_patch", "view_spec", "camera", "view_evaluations"],
    )?;
    let patch = proposal_object
        .get("parameter_patch")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            RuntimeError::InvalidInput(
                "ACTION_PARAMETER_PATCH_INVALID: parameter_patch must be an object".to_owned(),
            )
        })?;
    reject_unknown_keys(patch, &["schema_version", "strategy"])?;
    if patch.get("schema_version").and_then(Value::as_str) != Some("RuntimeParameterPatch@1") {
        return Err(RuntimeError::InvalidInput(
            "ACTION_PARAMETER_PATCH_UNSUPPORTED: schema_version is unavailable".to_owned(),
        ));
    }
    let strategy = patch
        .get("strategy")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            RuntimeError::InvalidInput(
                "ACTION_PARAMETER_PATCH_INVALID: strategy is required".to_owned(),
            )
        })?;
    if !matches!(
        strategy,
        "primitive-dimensions-v1"
            | "surface-control-points-v1"
            | "hard-surface-finish-v1"
            | "rear-stock-profile-reconstruction-v1"
    ) {
        return Err(RuntimeError::InvalidInput(
            "ACTION_PARAMETER_PATCH_UNSUPPORTED: strategy is unavailable".to_owned(),
        ));
    }
    let view_spec = proposal_object
        .get("view_spec")
        .ok_or_else(|| RuntimeError::InvalidInput("REPAIR_VIEW_SPEC_REQUIRED".to_owned()))?;
    let bound_proposal = bind_repair_camera(proposal, session, source_visual)?;
    let camera = bound_proposal
        .get("camera")
        .cloned()
        .ok_or_else(|| RuntimeError::InvalidInput("REPAIR_CAMERA_REQUIRED".to_owned()))?;
    // Keep the optional, candidate-bound six-view request attached to the
    // Runtime-generated proposal.  It is validated only after the exact
    // source-node program is materialized; callers never get to replace the
    // GeometryProgram or camera with copies of their own.
    let view_evaluations = proposal_object.get("view_evaluations").cloned();
    let action_object = action.as_object().ok_or_else(|| {
        RuntimeError::InvalidInput("ACTION_PARAMETER_PATCH_INVALID: action".to_owned())
    })?;
    let target_part_id = action_object
        .get("target_id")
        .and_then(Value::as_str)
        .filter(|value| is_opaque_id(value))
        .ok_or_else(|| RuntimeError::InvalidInput("REPAIR_PART_TARGET_REQUIRED".to_owned()))?;
    let changes = action_object
        .get("parameter_changes")
        .and_then(Value::as_array)
        .filter(|changes| !changes.is_empty())
        .ok_or_else(|| {
            RuntimeError::InvalidInput("REPAIR_PARAMETER_CHANGES_REQUIRED".to_owned())
        })?;

    if strategy == "rear-stock-profile-reconstruction-v1" {
        let mut materialized = materialize_rear_stock_profile_reconstruction_proposal(
            action_object,
            changes,
            session,
            source_candidate,
            source_geometry,
            source_visual,
            requested_stage,
            view_spec,
            &camera,
        )?;
        if let Some(view_evaluations) = view_evaluations {
            materialized["view_evaluations"] = view_evaluations;
        }
        return Ok(materialized);
    }

    let mut program = source_geometry.program.clone();
    let program_object = program.as_object_mut().ok_or_else(|| {
        RuntimeError::InvalidInput("ACTION_PARAMETER_PATCH_INVALID: source program".to_owned())
    })?;
    let nodes = program_object
        .get("nodes")
        .and_then(Value::as_array)
        .cloned()
        .ok_or_else(|| RuntimeError::InvalidInput("REPAIR_PROGRAM_NODES_INVALID".to_owned()))?;
    let nodes_by_id = node_map(program_object)?;
    let part_outputs = part_output_map(program_object)?;
    let roots = part_roots(part_outputs.get(target_part_id));
    if roots.is_empty() {
        return Err(RuntimeError::InvalidInput(
            "REPAIR_PART_TARGET_NOT_IN_PROGRAM".to_owned(),
        ));
    }
    let closure = node_closure(&nodes_by_id, &roots);
    let mut seen = HashSet::new();
    let mut selected_node_id: Option<String> = None;
    let mut selected_operator: Option<String> = None;

    for change in changes {
        let change_object = change.as_object().ok_or_else(|| {
            RuntimeError::InvalidInput("ACTION_PARAMETER_PATCH_INVALID: change".to_owned())
        })?;
        let parameter_id = change_object
            .get("parameter_id")
            .and_then(Value::as_str)
            .filter(|value| is_opaque_id(value))
            .ok_or_else(|| {
                RuntimeError::InvalidInput(
                    "ACTION_PARAMETER_PATCH_INVALID: parameter_id".to_owned(),
                )
            })?;
        let semantic = runtime_parameter_semantic(parameter_id).ok_or_else(|| {
            RuntimeError::InvalidInput(format!(
                "ACTION_PARAMETER_PATCH_UNSUPPORTED: parameter_id {parameter_id}"
            ))
        })?;
        if runtime_parameter_strategy(semantic) != strategy {
            return Err(RuntimeError::InvalidInput(
                "ACTION_PARAMETER_PATCH_STRATEGY_PARAMETER_MISMATCH".to_owned(),
            ));
        }
        if !seen.insert(semantic) {
            return Err(RuntimeError::InvalidInput(
                "ACTION_PARAMETER_PATCH_INVALID: duplicate semantic parameter".to_owned(),
            ));
        }
        let unit = change_object
            .get("unit")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                RuntimeError::InvalidInput("ACTION_PARAMETER_PATCH_INVALID: unit".to_owned())
            })?;
        if !runtime_parameter_unit_allowed(semantic, unit) {
            return Err(RuntimeError::InvalidInput(format!(
                "ACTION_PARAMETER_PATCH_UNSUPPORTED: unit {unit} is incompatible with {parameter_id}"
            )));
        }
        let before = change_object
            .get("before")
            .and_then(Value::as_f64)
            .filter(|value| value.is_finite())
            .ok_or_else(|| {
                RuntimeError::InvalidInput("ACTION_PARAMETER_PATCH_INVALID: before".to_owned())
            })?;
        let after = change_object
            .get("after")
            .and_then(Value::as_f64)
            .filter(|value| value.is_finite())
            .ok_or_else(|| {
                RuntimeError::InvalidInput("ACTION_PARAMETER_PATCH_INVALID: after".to_owned())
            })?;
        let minimum = change_object
            .get("minimum")
            .and_then(Value::as_f64)
            .filter(|value| value.is_finite())
            .ok_or_else(|| {
                RuntimeError::InvalidInput("ACTION_PARAMETER_PATCH_INVALID: minimum".to_owned())
            })?;
        let maximum = change_object
            .get("maximum")
            .and_then(Value::as_f64)
            .filter(|value| value.is_finite())
            .ok_or_else(|| {
                RuntimeError::InvalidInput("ACTION_PARAMETER_PATCH_INVALID: maximum".to_owned())
            })?;
        if minimum > maximum
            || before < minimum
            || before > maximum
            || after < minimum
            || after > maximum
        {
            return Err(RuntimeError::InvalidInput(
                "ACTION_PARAMETER_PATCH_OUT_OF_BOUNDS".to_owned(),
            ));
        }
        let candidates = nodes
            .iter()
            .enumerate()
            .filter_map(|(index, node)| {
                let node_object = node.as_object()?;
                let node_id = node_object.get("node_id")?.as_str()?;
                (closure.contains(node_id)
                    && runtime_parameter_node_supports(node_object, semantic))
                .then_some((index, node_id.to_owned()))
            })
            .collect::<Vec<_>>();
        let root_candidates = candidates
            .iter()
            .filter(|(_, node_id)| roots.iter().any(|root| root == node_id))
            .cloned()
            .collect::<Vec<_>>();
        let selected = if root_candidates.len() == 1 {
            root_candidates[0].clone()
        } else if root_candidates.len() > 1 || candidates.len() != 1 {
            return Err(RuntimeError::InvalidInput(
                "ACTION_PARAMETER_PATCH_AMBIGUOUS_NODE: one deterministic source node is required"
                    .to_owned(),
            ));
        } else {
            candidates[0].clone()
        };
        let node_object = nodes[selected.0]
            .as_object()
            .ok_or_else(|| RuntimeError::InvalidInput("REPAIR_PROGRAM_NODE_INVALID".to_owned()))?;
        let operator_id = node_object
            .get("operator_id")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                RuntimeError::InvalidInput("REPAIR_PROGRAM_OPERATOR_MISSING".to_owned())
            })?;
        if let Some(requested_operator) = action_object.get("operator_id").and_then(Value::as_str) {
            if requested_operator != operator_id {
                return Err(RuntimeError::InvalidInput(
                    "ACTION_PARAMETER_PATCH_OPERATOR_MISMATCH".to_owned(),
                ));
            }
        }
        if let Some(previous_node_id) = selected_node_id.as_deref() {
            if previous_node_id != selected.1 {
                return Err(RuntimeError::InvalidInput(
                    "ACTION_PARAMETER_PATCH_SINGLE_NODE_REQUIRED".to_owned(),
                ));
            }
        } else {
            selected_node_id = Some(selected.1.clone());
            selected_operator = Some(operator_id.to_owned());
        }

        let parameters = program_object
            .get_mut("nodes")
            .and_then(Value::as_array_mut)
            .and_then(|nodes| nodes.get_mut(selected.0))
            .and_then(Value::as_object_mut)
            .and_then(|node| node.get_mut("parameters"))
            .and_then(Value::as_object_mut)
            .ok_or_else(|| {
                RuntimeError::InvalidInput("REPAIR_PROGRAM_PARAMETERS_INVALID".to_owned())
            })?;
        let current = match semantic {
            RuntimeParameterSemantic::Size(2)
                if operator_id == "forgecad.geometry.energy-core@1" =>
            {
                parameters.get("depth_m").and_then(Value::as_f64)
            }
            RuntimeParameterSemantic::Size(index) => parameters
                .get("size_m")
                .and_then(Value::as_array)
                .and_then(|values| values.get(index))
                .and_then(Value::as_f64),
            RuntimeParameterSemantic::Position(index) => parameters
                .get("position_m")
                .and_then(Value::as_array)
                .and_then(|values| values.get(index))
                .and_then(Value::as_f64),
            RuntimeParameterSemantic::Rotation(index) => parameters
                .get("rotation_rad")
                .and_then(Value::as_array)
                .and_then(|values| values.get(index))
                .and_then(Value::as_f64),
            RuntimeParameterSemantic::Radius => parameters.get("radius_m").and_then(Value::as_f64),
            RuntimeParameterSemantic::OuterRadius => {
                parameters.get("outer_radius_m").and_then(Value::as_f64)
            }
            RuntimeParameterSemantic::InnerRadius => {
                parameters.get("inner_radius_m").and_then(Value::as_f64)
            }
            RuntimeParameterSemantic::Thickness => {
                parameters.get("thickness_m").and_then(Value::as_f64)
            }
            RuntimeParameterSemantic::Bevel => parameters.get("bevel_m").and_then(Value::as_f64),
            RuntimeParameterSemantic::RearStockInnerReceiverDeltaY
            | RuntimeParameterSemantic::RearStockInnerCapDeltaY
            | RuntimeParameterSemantic::RearStockReceiverInnerXDelta
            | RuntimeParameterSemantic::RearStockCapInnerXDelta
            | RuntimeParameterSemantic::RearStockDepthCenterInnerDeltaY => None,
            RuntimeParameterSemantic::SurfaceControlPoint { index, axis } => parameters
                .get("control_points")
                .and_then(Value::as_array)
                .and_then(|points| points.get(index))
                .and_then(Value::as_array)
                .and_then(|point| point.get(axis))
                .and_then(Value::as_f64),
        }
        .filter(|value| value.is_finite())
        .ok_or_else(|| {
            RuntimeError::InvalidInput("ACTION_PARAMETER_PATCH_CURRENT_VALUE_MISSING".to_owned())
        })?;
        // Absolute units describe the source value and therefore get a
        // numeric stale-before check. A ratio describes a normalized edit
        // factor (for example 1.00 -> 1.04), not the node's meter/radian
        // value; the exact source candidate/evidence hashes already provide
        // the stale binding for that form.
        let tolerance = 1e-9 * current.abs().max(before.abs()).max(1.0);
        if unit != "ratio" && (current - before).abs() > tolerance {
            return Err(RuntimeError::InvalidInput(
                "ACTION_PARAMETER_PATCH_STALE_BEFORE".to_owned(),
            ));
        }
        let proposed = if unit == "ratio" {
            if before.abs() <= f64::EPSILON {
                return Err(RuntimeError::InvalidInput(
                    "ACTION_PARAMETER_PATCH_INVALID: ratio before must be non-zero".to_owned(),
                ));
            }
            current * after / before
        } else {
            after
        };
        if !runtime_parameter_value_in_bounds(semantic, proposed) {
            return Err(RuntimeError::InvalidInput(
                "ACTION_PARAMETER_PATCH_OUT_OF_BOUNDS".to_owned(),
            ));
        }
        if !runtime_parameter_relationship_valid(operator_id, parameters, semantic, proposed) {
            return Err(RuntimeError::InvalidInput(
                "ACTION_PARAMETER_PATCH_GEOMETRY_RELATIONSHIP_INVALID".to_owned(),
            ));
        }
        let proposed_value = runtime_parameter_number(proposed, parameter_id)?;
        match semantic {
            RuntimeParameterSemantic::Size(2)
                if operator_id == "forgecad.geometry.energy-core@1" =>
            {
                parameters.insert("depth_m".to_owned(), proposed_value);
            }
            RuntimeParameterSemantic::Size(index) => {
                let values = parameters
                    .get_mut("size_m")
                    .and_then(Value::as_array_mut)
                    .ok_or_else(|| {
                        RuntimeError::InvalidInput("REPAIR_PROGRAM_PARAMETERS_INVALID".to_owned())
                    })?;
                values[index] = proposed_value;
            }
            RuntimeParameterSemantic::Position(index) => {
                let values = parameters
                    .get_mut("position_m")
                    .and_then(Value::as_array_mut)
                    .ok_or_else(|| {
                        RuntimeError::InvalidInput("REPAIR_PROGRAM_PARAMETERS_INVALID".to_owned())
                    })?;
                values[index] = proposed_value;
            }
            RuntimeParameterSemantic::Rotation(index) => {
                let values = parameters
                    .get_mut("rotation_rad")
                    .and_then(Value::as_array_mut)
                    .ok_or_else(|| {
                        RuntimeError::InvalidInput("REPAIR_PROGRAM_PARAMETERS_INVALID".to_owned())
                    })?;
                values[index] = proposed_value;
            }
            RuntimeParameterSemantic::Radius => {
                parameters.insert("radius_m".to_owned(), proposed_value);
            }
            RuntimeParameterSemantic::OuterRadius => {
                parameters.insert("outer_radius_m".to_owned(), proposed_value);
            }
            RuntimeParameterSemantic::InnerRadius => {
                parameters.insert("inner_radius_m".to_owned(), proposed_value);
            }
            RuntimeParameterSemantic::Thickness => {
                parameters.insert("thickness_m".to_owned(), proposed_value);
            }
            RuntimeParameterSemantic::Bevel => {
                parameters.insert("bevel_m".to_owned(), proposed_value);
            }
            RuntimeParameterSemantic::RearStockInnerReceiverDeltaY
            | RuntimeParameterSemantic::RearStockInnerCapDeltaY
            | RuntimeParameterSemantic::RearStockReceiverInnerXDelta
            | RuntimeParameterSemantic::RearStockCapInnerXDelta
            | RuntimeParameterSemantic::RearStockDepthCenterInnerDeltaY => {
                return Err(RuntimeError::InvalidInput(
                    "ACTION_STOCK_PROFILE_RECONSTRUCTION_ROUTING_MISMATCH".to_owned(),
                ));
            }
            RuntimeParameterSemantic::SurfaceControlPoint { index, axis } => {
                let points = parameters
                    .get_mut("control_points")
                    .and_then(Value::as_array_mut)
                    .ok_or_else(|| {
                        RuntimeError::InvalidInput("REPAIR_PROGRAM_PARAMETERS_INVALID".to_owned())
                    })?;
                let point = points
                    .get_mut(index)
                    .and_then(Value::as_array_mut)
                    .ok_or_else(|| {
                        RuntimeError::InvalidInput("REPAIR_PROGRAM_PARAMETERS_INVALID".to_owned())
                    })?;
                point[axis] = proposed_value;
            }
        }
    }

    let selected_operator = selected_operator.ok_or_else(|| {
        RuntimeError::InvalidInput("ACTION_PARAMETER_PATCH_NODE_NOT_FOUND".to_owned())
    })?;
    let mut proposed_draft = program.clone();
    proposed_draft
        .as_object_mut()
        .expect("GeometryProgram object was checked")
        .remove("canonical_sha256");
    let hash_result =
        hash_geometry_program_with_runtime_worker(&proposed_draft).map_err(|error| {
            RuntimeError::InvalidInput(format!("ACTION_PARAMETER_PATCH_HASH_FAILED: {error}"))
        })?;
    let proposed_program_sha256 = hash_result
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .ok_or_else(|| {
            RuntimeError::InvalidInput("ACTION_PARAMETER_PATCH_HASH_INVALID".to_owned())
        })?
        .to_owned();
    program["canonical_sha256"] = Value::String(proposed_program_sha256.clone());

    let seed = canonical_json_hash(&json!({
        "candidate_id": source_candidate.candidate_id,
        "action": action,
        "geometry_program_sha256": proposed_program_sha256,
    }));
    let critic_prefix = source_visual
        .quality_sha256
        .chars()
        .take(24)
        .collect::<String>();
    let mut intent = json!({
        "schema_version":"RepairIntent@1",
        "intent_id":format!("runtime-parameter-patch-{}", &seed[..32]),
        "session_id":session.session_id,
        "project_id":session.project_id,
        "candidate_id":source_candidate.candidate_id,
        "candidate_state_sha256":source_candidate.canonical_sha256,
        "reference_id":session.reference_id,
        "reference_sha256":session.reference_sha256,
        "camera_hash":session.camera_hash,
        "observation_sha256":session.observation_sha256,
        "source_evidence_sha256":session.evidence_sha256,
        "source_critic_report_id":format!("critic-report-{critic_prefix}"),
        "source_critic_report_sha256":source_visual.quality_sha256,
        "stage":requested_stage,
        "scope":{"kind":"part","part_id":target_part_id},
        "action":{
            "action_kind":"bounded-repair",
            "kit_id":"forgecad.kit.runtime-parameter-patch@1",
            "operator_id":selected_operator,
            "operation":"adjust-parameter",
            "parameter_changes":action_object["parameter_changes"].clone(),
            "bounded":true,
            "description":action_object["description"].clone()
        },
        "precondition":{
            "failed_gate_id":"visible-view",
            "quality_status":source_visual.quality_status,
            "current_candidate_state_sha256":source_candidate.canonical_sha256,
            "evidence_sha256":session.evidence_sha256,
            "status":"failed"
        },
        "recompute":{
            "steps":["compile","readback","render","compare"],
            "must_rebind_reference":true,
            "must_rebind_camera":true,
            "confirm_allowed":false
        },
        "rollback":{
            "relation":"none",
            "target_checkpoint_id":null,
            "target_checkpoint_sha256":null,
            "target_version_id":null,
            "target_version_sha256":null,
            "on_failure":"keep-current",
            "reason":null
        },
        "status":"approved",
        "approval_required":true,
        "runtime_write":false,
        "canonical_sha256":""
    });
    intent["canonical_sha256"] = Value::String(canonical_json_hash(&intent));
    let mut materialized = json!({
        "repair_intent":intent,
        "geometry_program":program,
        "view_spec":view_spec,
        "camera":camera
    });
    if let Some(view_evaluations) = view_evaluations {
        materialized["view_evaluations"] = view_evaluations;
    }
    Ok(materialized)
}

/// Execute the first real RepairIntent slice.  The source candidate remains
/// immutable; the typed proposal is compiled into a separate prepared
/// candidate and evaluated against the same reference, camera and view spec.
/// This deliberately stops at a reviewable proposal and never confirms a
/// version or exports an artifact.
fn execute_bounded_repair_proposal(
    runtime: &Runtime,
    mut run: Value,
    request: &Map<String, Value>,
    action: &Value,
    proposal: &Value,
    session: &AgenticSessionRecord,
    source_candidate: &CandidateRecord,
    source_geometry: &GeometryBindings,
    source_visual: &VisualBindings,
    requested_stage: &str,
) -> Result<Value, RuntimeError> {
    let bound_proposal = bind_repair_camera(proposal, session, source_visual)?;
    let proposal = &bound_proposal;
    let proposal_object = proposal.as_object().ok_or_else(|| {
        RuntimeError::InvalidInput("REPAIR_PROPOSAL_INVALID: proposal must be an object".to_owned())
    })?;
    if !(proposal_object.len() == 4 || proposal_object.len() == 5)
        || proposal_object.keys().any(|key| {
            !matches!(
                key.as_str(),
                "repair_intent" | "geometry_program" | "view_spec" | "camera" | "view_evaluations"
            )
        })
    {
        return Err(RuntimeError::InvalidInput(
            "REPAIR_PROPOSAL_INVALID: proposal fields are incomplete or unsupported".to_owned(),
        ));
    }
    let intent = proposal_object
        .get("repair_intent")
        .ok_or_else(|| RuntimeError::InvalidInput("REPAIR_INTENT_REQUIRED".to_owned()))?;
    let geometry_program = proposal_object
        .get("geometry_program")
        .ok_or_else(|| RuntimeError::InvalidInput("REPAIR_GEOMETRY_PROGRAM_REQUIRED".to_owned()))?;
    let view_spec = proposal_object
        .get("view_spec")
        .ok_or_else(|| RuntimeError::InvalidInput("REPAIR_VIEW_SPEC_REQUIRED".to_owned()))?;
    let camera = proposal_object
        .get("camera")
        .ok_or_else(|| RuntimeError::InvalidInput("REPAIR_CAMERA_REQUIRED".to_owned()))?;

    validate_repair_proposal(
        runtime,
        action,
        intent,
        geometry_program,
        view_spec,
        camera,
        session,
        source_candidate,
        source_geometry,
        requested_stage,
    )?;
    let view_evaluations = validate_view_evaluations(runtime, proposal_object, session)?;
    let (intent_sha256, intent_object_sha256) = persist_repair_intent(runtime, intent)?;

    let prepared = runtime.prepare_geometry_candidate(
        &session.project_id,
        source_candidate.base_version_id.as_deref(),
        json!({
            "typed":"geometry",
            "reference_id":session.reference_id,
            "geometry_program":geometry_program
        }),
    )?;
    let proposal_candidate: CandidateRecord = serde_json::from_value(
        prepared
            .get("candidate")
            .cloned()
            .ok_or_else(|| RuntimeError::InvalidInput("REPAIR_CANDIDATE_MISSING".to_owned()))?,
    )
    .map_err(|error| RuntimeError::InvalidInput(format!("REPAIR_CANDIDATE_INVALID: {error}")))?;
    let proposal_candidate_id = proposal_candidate.candidate_id.clone();
    let proposal_evidence = runtime
        .store
        .get_geometry_candidate_evidence(&proposal_candidate_id)?
        .ok_or_else(|| RuntimeError::InvalidInput("REPAIR_GEOMETRY_EVIDENCE_MISSING".to_owned()))?;

    let comparison = runtime.prepare_reference_comparison(
        &session.project_id,
        json!({
            "project_id":session.project_id,
            "candidate_id":proposal_candidate_id,
            "reference_id":session.reference_id,
            "view_spec":view_spec,
            "camera":camera
        }),
    )?;
    let comparison_report = comparison
        .get("comparison_report")
        .ok_or_else(|| RuntimeError::InvalidInput("REPAIR_COMPARISON_MISSING".to_owned()))?;
    let quality_report = comparison
        .get("quality_report")
        .ok_or_else(|| RuntimeError::InvalidInput("REPAIR_QUALITY_REPORT_MISSING".to_owned()))?;
    let mut visual_status = quality_report
        .get("visual_status")
        .and_then(Value::as_str)
        .ok_or_else(|| RuntimeError::InvalidInput("REPAIR_QUALITY_STATUS_MISSING".to_owned()))?
        .to_owned();
    let proposal_metrics = comparison_report
        .get("metrics")
        .ok_or_else(|| RuntimeError::InvalidInput("REPAIR_METRICS_MISSING".to_owned()))?;
    let (mut non_regressing, mut strict_improvement, mut baseline_score, mut proposal_score) =
        compare_visual_metrics(&source_visual.metrics, proposal_metrics)?;
    let mut cross_view_bundle_sha256 = None;
    let mut cross_view_hard_gate_passed = false;
    if let Some(view_evaluations) = view_evaluations.as_ref() {
        let cross_view = if matches!(
            runtime_parameter_patch_strategy(action),
            Ok("rear-stock-profile-reconstruction-v1")
        ) {
            evaluate_rear_stock_profile_six_view_gate(
                runtime,
                session,
                source_candidate,
                &proposal_candidate,
                view_evaluations,
            )?
        } else {
            evaluate_cross_view_proposal(
                runtime,
                session,
                source_candidate,
                &proposal_candidate,
                view_evaluations,
            )?
        };
        cross_view_bundle_sha256 = Some(cross_view.bundle_sha256.clone());
        cross_view_hard_gate_passed = cross_view.hard_gate_passed;
        visual_status = cross_view.aggregate_status.clone();
        non_regressing = cross_view.non_regressing;
        strict_improvement = cross_view.strict_improvement;
        baseline_score = cross_view.baseline_score;
        proposal_score = cross_view.proposal_score;
    }
    let promotion = if strict_improvement {
        "reviewable"
    } else if non_regressing {
        "not-improved"
    } else {
        "rejected-regression"
    };

    let quality_report_id = quality_report
        .get("quality_report_id")
        .and_then(Value::as_str)
        .ok_or_else(|| RuntimeError::InvalidInput("REPAIR_QUALITY_REPORT_ID_MISSING".to_owned()))?;
    let quality_passed = if view_evaluations.is_some() {
        cross_view_hard_gate_passed
    } else {
        visual_status == "PARTIAL_VISIBLE_VIEW_PASS"
    };
    // ActionRun proposals are review evidence, not active candidate commits.
    // Mutating the proposal's quality bit here would change its canonical
    // state after the CrossViewEvidenceBundle had already bound that state,
    // leaving an internally stale evidence chain.  Keep the reviewable
    // candidate immutable; confirmation/promotion owns any later state move.
    let _quality_report_id = quality_report_id;

    let visual_passed = quality_passed;
    let checkpoint = runtime.prepare_action_checkpoint(
        request,
        session,
        source_candidate,
        requested_stage,
        source_geometry,
        source_visual.quality_status == "PARTIAL_VISIBLE_VIEW_PASS",
    )?;
    let proposal_summary = json!({
        "intent_sha256":intent_sha256,
        "intent_object_sha256":intent_object_sha256,
        "candidate_id":proposal_candidate.candidate_id,
        "candidate_state_sha256":proposal_candidate.canonical_sha256,
        "artifact_sha256":proposal_evidence.artifact_object_sha256,
        "geometry_program_sha256":proposal_evidence.geometry_program_sha256,
        "render_set_sha256":comparison["render_set_object_sha256"],
        "comparison_report_sha256":comparison["comparison_report_object_sha256"],
        "quality_report_sha256":comparison["quality_report_object_sha256"],
        "visual_status":visual_status,
        "baseline_score":baseline_score,
        "proposal_score":proposal_score,
        "strict_improvement":strict_improvement,
        "non_regressing":non_regressing,
        "promotion":promotion,
        "confirm_allowed":false,
        "cross_view_evidence_sha256":cross_view_bundle_sha256
            .clone()
            .map(Value::String)
            .unwrap_or(Value::Null)
    });
    run["proposal"] = proposal_summary;
    run["status"] = Value::String("completed".to_owned());
    run["completed_stage"] = Value::String("evaluate".to_owned());
    run["quality_status"] = Value::String(visual_status.to_owned());
    run["failed_gates"] = if visual_passed {
        json!([])
    } else {
        json!(["visible-view"])
    };
    run["allowed_actions"] = if strict_improvement {
        json!(["inspect", "retry"])
    } else {
        json!(["inspect", "retry"])
    };
    run["locked_actions"] = json!(["confirm", "export", "next-stage"]);
    run["checkpoint_id"] = checkpoint
        .get("checkpoint")
        .and_then(|value| value.get("checkpoint_id"))
        .cloned()
        .unwrap_or(Value::Null);
    run["checkpoint_hash"] = checkpoint
        .get("checkpoint")
        .and_then(|value| value.get("canonical_sha256"))
        .cloned()
        .unwrap_or(Value::Null);
    finalize_run(&mut run);
    persist_run(runtime, &run)
}

/// Execute the canonical direct Primary Form ActionRun shape used by the
/// Agentic Runtime contract.  The action is converted into a typed
/// `SilhouetteRig@1`; the existing Runtime pipeline remains the only writer
/// of geometry and the result is persisted as an immutable receipt only.
fn execute_direct_primary_form_action(
    runtime: &Runtime,
    mut run: Value,
    action: &Value,
    session: &AgenticSessionRecord,
    candidate: &CandidateRecord,
    visual: &VisualBindings,
) -> Result<Value, RuntimeError> {
    let input_sha256 = run
        .get("input_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| RuntimeError::InvalidInput("DESIGN_ACTION_INPUT_HASH_MISSING".to_owned()))?;
    let part_id = action
        .get("target_id")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            RuntimeError::InvalidInput("PRIMARY_FORM_ACTION_PART_REQUIRED".to_owned())
        })?;
    let target_sha256 = visual.target_sha256.as_deref().ok_or_else(|| {
        RuntimeError::InvalidInput(
            "PRIMARY_FORM_ACTION_TARGET_REQUIRED: visual evidence has no bound silhouette target"
                .to_owned(),
        )
    })?;
    let rig = rig_from_action(&candidate.candidate_id, input_sha256, part_id, action)?;
    let base_version_id = session.current_version_id.as_deref();
    let mut request = json!({
        "project_id":session.project_id,
        "candidate_id":candidate.candidate_id,
        "target_sha256":target_sha256,
        "part_id":part_id,
        "rig":rig,
        "base_camera":visual.camera,
        "optimizer":{
            "algorithm":"coordinate_descent",
            "max_iterations":1,
            "max_evaluations":64,
            "step_fraction":0.1
        },
        "base_version_id":base_version_id,
        "canonical_sha256":""
    });
    request["canonical_sha256"] = Value::String(canonical_json_hash(&request));
    let result =
        runtime.primary_form_repair_prepare(&session.project_id, base_version_id, request)?;
    let result_object = runtime.put_object(
        &canonical_json_bytes(&result)
            .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?,
        None,
        "application/json",
        "agentic-action-primary-form-result",
    )?;
    let prepared = result.get("status").and_then(Value::as_str) == Some("prepared");
    let quality_status = if prepared {
        result
            .get("quality_status")
            .and_then(Value::as_str)
            .unwrap_or("QUALITY_TARGET_NOT_MET")
            .to_owned()
    } else {
        visual.quality_status.clone()
    };
    run["status"] = Value::String(if prepared { "completed" } else { "blocked" }.to_owned());
    run["completed_stage"] =
        Value::String(if prepared { "evaluate" } else { "prepare" }.to_owned());
    run["stage_results"] =
        direct_primary_form_stage_results(&result, &result_object.record.sha256, prepared);
    run["quality_status"] = Value::String(quality_status.clone());
    run["failed_gates"] = json!(if prepared {
        if quality_status == "PARTIAL_VISIBLE_VIEW_PASS" {
            Vec::<&str>::new()
        } else {
            vec!["visible-view"]
        }
    } else {
        vec!["prepare", "primary-silhouette"]
    });
    run["allowed_actions"] = json!(["inspect", "retry", "bounded-repair"]);
    run["locked_actions"] = json!(["confirm", "export", "next-stage"]);
    run["checkpoint_id"] = Value::Null;
    run["checkpoint_hash"] = Value::Null;
    finalize_run(&mut run);
    persist_run(runtime, &run)
}

fn direct_primary_form_stage_results(result: &Value, result_sha256: &str, prepared: bool) -> Value {
    let mut stages = initial_stage_results();
    stages["prepare"] = stage_result("completed", Some(result_sha256), None);
    if prepared {
        let prepared_candidate = result.pointer("/prepared_candidate/candidate");
        let artifact_sha256 = prepared_candidate
            .and_then(|candidate| candidate.get("prepared_object_sha256"))
            .and_then(Value::as_str);
        let render_set_hash = result
            .pointer("/visual_evidence/render_set_hash")
            .and_then(Value::as_str);
        let quality_hash = result
            .pointer("/visual_evidence/quality_report_hash")
            .and_then(Value::as_str);
        stages["compile"] = stage_result("completed", artifact_sha256, None);
        stages["readback"] = stage_result("completed", artifact_sha256, None);
        stages["render"] = stage_result("completed", render_set_hash, None);
        stages["evaluate"] = stage_result("completed", quality_hash, None);
    } else {
        stages["compile"] = stage_result("blocked", None, Some("primary-form-no-improvement"));
        stages["readback"] = stage_result("blocked", None, Some("primary-form-not-prepared"));
        stages["render"] = stage_result("blocked", None, Some("primary-form-not-prepared"));
        stages["evaluate"] = stage_result("blocked", None, Some("primary-form-not-prepared"));
    }
    stages
}

pub(crate) fn validate_view_evaluations(
    runtime: &Runtime,
    proposal: &Map<String, Value>,
    session: &AgenticSessionRecord,
) -> Result<Option<Vec<ViewEvaluation>>, RuntimeError> {
    let Some(value) = proposal.get("view_evaluations") else {
        return Ok(None);
    };
    let entries = value.as_array().ok_or_else(|| {
        RuntimeError::InvalidInput("REPAIR_VIEW_EVALUATIONS_INVALID: expected an array".to_owned())
    })?;
    if !(2..=8).contains(&entries.len()) {
        return Err(RuntimeError::InvalidInput(
            "REPAIR_VIEW_EVALUATIONS_INVALID: expected 2 to 8 views".to_owned(),
        ));
    }
    let (canvas, _canvas_sha256) =
        super::agentic_session::durable_reference_canvas_for_session_binding(
            runtime,
            &session.project_id,
            &session.session_id,
            &session.candidate_id,
        )?;
    let authored_views = canvas
        .get("views")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            RuntimeError::InvalidInput(
                "REPAIR_VIEW_EVALUATIONS_BLOCKED: ReferenceCanvas views are missing".to_owned(),
            )
        })?;
    let mut seen = HashSet::new();
    let mut result = Vec::with_capacity(entries.len());
    for entry in entries {
        let object = entry.as_object().ok_or_else(|| {
            RuntimeError::InvalidInput(
                "REPAIR_VIEW_EVALUATION_INVALID: entry must be an object".to_owned(),
            )
        })?;
        if object.len() != 5
            || object.keys().any(|key| {
                !matches!(
                    key.as_str(),
                    "view_id" | "reference_id" | "reference_sha256" | "view_spec" | "camera"
                )
            })
        {
            return Err(RuntimeError::InvalidInput(
                "REPAIR_VIEW_EVALUATION_INVALID: unsupported or missing fields".to_owned(),
            ));
        }
        let view_id = super::required_value_id(object.get("view_id"), "view_id")?.to_owned();
        if !seen.insert(view_id.clone()) {
            return Err(RuntimeError::InvalidInput(
                "REPAIR_VIEW_EVALUATIONS_INVALID: duplicate view_id".to_owned(),
            ));
        }
        let reference_id =
            super::required_value_id(object.get("reference_id"), "reference_id")?.to_owned();
        let reference_sha256 = object
            .get("reference_sha256")
            .and_then(Value::as_str)
            .filter(|value| is_sha256(value))
            .ok_or_else(|| {
                RuntimeError::InvalidInput(
                    "REPAIR_VIEW_EVALUATION_INVALID: reference_sha256 is invalid".to_owned(),
                )
            })?
            .to_owned();
        let reference = runtime.reference(&reference_id)?.ok_or_else(|| {
            RuntimeError::InvalidInput("REPAIR_VIEW_REFERENCE_NOT_FOUND".to_owned())
        })?;
        if reference.project_id != session.project_id || reference.object_sha256 != reference_sha256
        {
            return Err(RuntimeError::InvalidInput(
                "REPAIR_VIEW_REFERENCE_BINDING_MISMATCH".to_owned(),
            ));
        }
        let authored_view = authored_views
            .iter()
            .find(|view| view.get("view_id").and_then(Value::as_str) == Some(view_id.as_str()))
            .ok_or_else(|| {
                RuntimeError::InvalidInput(
                    "REPAIR_VIEW_BINDING_MISMATCH: view is not in ReferenceCanvas".to_owned(),
                )
            })?;
        if authored_view.get("reference_id").and_then(Value::as_str) != Some(reference_id.as_str())
            || authored_view
                .get("reference_sha256")
                .and_then(Value::as_str)
                != Some(reference_sha256.as_str())
        {
            return Err(RuntimeError::InvalidInput(
                "REPAIR_VIEW_BINDING_MISMATCH: reference differs from ReferenceCanvas".to_owned(),
            ));
        }
        let view_spec = object
            .get("view_spec")
            .ok_or_else(|| RuntimeError::InvalidInput("REPAIR_VIEW_SPEC_REQUIRED".to_owned()))?;
        if view_spec.get("view_id").and_then(Value::as_str) != Some(view_id.as_str()) {
            return Err(RuntimeError::InvalidInput(
                "REPAIR_VIEW_BINDING_MISMATCH: view spec id differs".to_owned(),
            ));
        }
        super::validate_reference_view_spec(view_spec, &reference)?;
        if let Some(authored_view_spec) = authored_view.get("view_spec") {
            if authored_view_spec.get("canonical_sha256") != view_spec.get("canonical_sha256") {
                return Err(RuntimeError::InvalidInput(
                    "REPAIR_VIEW_BINDING_MISMATCH: view spec differs from ReferenceCanvas"
                        .to_owned(),
                ));
            }
        }
        let target_sha256 = authored_view
            .get("target_sha256")
            .and_then(Value::as_str)
            .map(str::to_owned);
        let mask_sha256 = authored_view
            .get("mask_sha256")
            .and_then(Value::as_str)
            .map(str::to_owned);
        if target_sha256.is_some() != mask_sha256.is_some() {
            return Err(RuntimeError::InvalidInput(
                "REPAIR_VIEW_BINDING_MISMATCH: target and mask must be paired".to_owned(),
            ));
        }
        if let Some(target_sha256) = target_sha256.as_deref() {
            let target = runtime.read_silhouette_target(target_sha256)?;
            if target.get("reference_id").and_then(Value::as_str) != Some(reference_id.as_str())
                || target.get("reference_sha256").and_then(Value::as_str)
                    != Some(reference_sha256.as_str())
                || target.get("mask_sha256").and_then(Value::as_str) != mask_sha256.as_deref()
            {
                return Err(RuntimeError::InvalidInput(
                    "REPAIR_VIEW_BINDING_MISMATCH: target lineage differs".to_owned(),
                ));
            }
        }
        let camera = object
            .get("camera")
            .ok_or_else(|| RuntimeError::InvalidInput("REPAIR_VIEW_CAMERA_REQUIRED".to_owned()))?;
        super::validate_camera_calibration(camera)?;
        super::validate_reference_canvas_view_camera(
            runtime,
            &session.project_id,
            &session.candidate_id,
            Some(&session.session_id),
            Some(&session.candidate_id),
            &view_id,
            &reference_id,
            camera,
        )?;
        let camera_claim = authored_view
            .get("camera_claim")
            .and_then(Value::as_object)
            .ok_or_else(|| RuntimeError::InvalidInput("REPAIR_VIEW_CAMERA_UNKNOWN".to_owned()))?;
        let visibility = camera_claim
            .get("visibility")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_owned();
        let confidence = if visibility == "observed" { 1.0 } else { 0.5 };
        result.push(ViewEvaluation {
            view_id,
            kind: authored_view
                .get("kind")
                .and_then(Value::as_str)
                .unwrap_or("detail")
                .to_owned(),
            visibility,
            confidence,
            reference_id,
            reference_sha256,
            target_sha256,
            view_spec: view_spec.clone(),
            camera: camera.clone(),
        });
    }
    validate_cross_view_evaluation_coverage(&canvas, &result)?;
    Ok(Some(result))
}

/// A cross-view proposal is only meaningful when it evaluates every supplied
/// view in the durable ReferenceCanvas exactly once.  Checking only the
/// number of entries is insufficient: a caller could repeat two easy views
/// while a complete canvas silently omits the rear or side reference.
fn validate_cross_view_evaluation_coverage(
    canvas: &Value,
    evaluations: &[ViewEvaluation],
) -> Result<(), RuntimeError> {
    let coverage = canvas
        .get("coverage")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            RuntimeError::InvalidInput(
                "REPAIR_VIEW_EVALUATIONS_COVERAGE_INVALID: coverage is missing".to_owned(),
            )
        })?;
    let supplied = coverage
        .get("supplied_views")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            RuntimeError::InvalidInput(
                "REPAIR_VIEW_EVALUATIONS_COVERAGE_INVALID: supplied views are missing".to_owned(),
            )
        })?;
    let expected = supplied
        .iter()
        .map(|value| {
            value.as_str().ok_or_else(|| {
                RuntimeError::InvalidInput(
                    "REPAIR_VIEW_EVALUATIONS_COVERAGE_INVALID: supplied view kind is invalid"
                        .to_owned(),
                )
            })
        })
        .collect::<Result<HashSet<_>, _>>()?;
    let actual = evaluations
        .iter()
        .map(|evaluation| evaluation.kind.as_str())
        .collect::<HashSet<_>>();
    if expected.len() != evaluations.len() || expected.len() != actual.len() || expected != actual {
        return Err(RuntimeError::InvalidInput(
            "REPAIR_VIEW_EVALUATIONS_COVERAGE_MISMATCH: every supplied view must be evaluated exactly once"
                .to_owned(),
        ));
    }
    Ok(())
}

#[derive(Debug, Clone)]
pub(crate) struct CrossViewEvaluationResult {
    pub(crate) bundle_sha256: String,
    pub(crate) aggregate_status: String,
    pub(crate) hard_gate_passed: bool,
    pub(crate) strict_improvement: bool,
    pub(crate) non_regressing: bool,
    pub(crate) baseline_score: f64,
    pub(crate) proposal_score: f64,
}

/// Real D1 rear-stock repairs must be evaluated against the complete six-view
/// identity set.  This wrapper deliberately keeps the existing immutable
/// CrossViewEvidenceBundle producer as the implementation seam while making
/// the coverage contract explicit for the one-node source repair.
pub(crate) fn evaluate_rear_stock_profile_six_view_gate(
    runtime: &Runtime,
    session: &AgenticSessionRecord,
    source_candidate: &CandidateRecord,
    proposal_candidate: &CandidateRecord,
    view_evaluations: &[ViewEvaluation],
) -> Result<CrossViewEvaluationResult, RuntimeError> {
    let expected = REAL_D1_REPAIR_SIX_VIEW_KINDS
        .iter()
        .copied()
        .collect::<HashSet<_>>();
    let actual = view_evaluations
        .iter()
        .map(|evaluation| evaluation.kind.as_str())
        .collect::<HashSet<_>>();
    let view_ids = view_evaluations
        .iter()
        .map(|evaluation| evaluation.view_id.as_str())
        .collect::<HashSet<_>>();
    if view_evaluations.len() != REAL_D1_REPAIR_SIX_VIEW_KINDS.len()
        || actual.len() != REAL_D1_REPAIR_SIX_VIEW_KINDS.len()
        || view_ids.len() != REAL_D1_REPAIR_SIX_VIEW_KINDS.len()
        || actual != expected
    {
        return Err(RuntimeError::InvalidInput(
            "REPAIR_REAL_D1_SIX_VIEW_GATE_COVERAGE_REQUIRED".to_owned(),
        ));
    }
    let (canvas, canvas_sha256) =
        super::agentic_session::durable_reference_canvas_for_session_binding(
            runtime,
            &session.project_id,
            &session.session_id,
            &session.candidate_id,
        )?;
    validate_cross_view_evaluation_coverage(&canvas, view_evaluations)?;
    // A one-node FormArt proposal is idempotent across Runtime restarts. The
    // general same-candidate path already replays by immutable identity, but
    // this source-versus-proposal wrapper previously skipped that lookup and
    // attempted to insert a second bundle for the same identity. Re-read and
    // validate the existing bundle before any Render Worker or Store write.
    if let Some(existing) = runtime.store.get_cross_view_evidence_by_identity(
        &session.project_id,
        &session.session_id,
        &proposal_candidate.candidate_id,
        &canvas_sha256,
    )? {
        let bundle = read_json_object(runtime, &existing.bundle_object_sha256)?;
        super::validate_cross_view_evidence_bundle(&bundle)?;
        if bundle.get("candidate_id").and_then(Value::as_str)
            != Some(proposal_candidate.candidate_id.as_str())
            || bundle.get("candidate_state_sha256").and_then(Value::as_str)
                != Some(proposal_candidate.canonical_sha256.as_str())
            || bundle.get("project_id").and_then(Value::as_str) != Some(session.project_id.as_str())
            || bundle.get("session_id").and_then(Value::as_str) != Some(session.session_id.as_str())
            || bundle
                .get("reference_canvas_sha256")
                .and_then(Value::as_str)
                != Some(canvas_sha256.as_str())
        {
            return Err(RuntimeError::InvalidInput(
                "REPAIR_REAL_D1_SIX_VIEW_REPLAY_BINDING_MISMATCH".to_owned(),
            ));
        }
        let existing_views = bundle
            .get("view_evaluations")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                RuntimeError::InvalidInput(
                    "REPAIR_REAL_D1_SIX_VIEW_REPLAY_VIEWS_INVALID".to_owned(),
                )
            })?;
        let views_match = existing_views.len() == view_evaluations.len()
            && view_evaluations.iter().all(|expected| {
                existing_views.iter().any(|actual| {
                    actual.get("view_id").and_then(Value::as_str) == Some(expected.view_id.as_str())
                        && actual.get("kind").and_then(Value::as_str)
                            == Some(expected.kind.as_str())
                        && actual.get("reference_id").and_then(Value::as_str)
                            == Some(expected.reference_id.as_str())
                        && actual.get("reference_sha256").and_then(Value::as_str)
                            == Some(expected.reference_sha256.as_str())
                        && actual.get("camera_hash").and_then(Value::as_str)
                            == expected.camera.get("camera_hash").and_then(Value::as_str)
                })
            });
        if !views_match {
            return Err(RuntimeError::InvalidInput(
                "REPAIR_REAL_D1_SIX_VIEW_REPLAY_COVERAGE_MISMATCH".to_owned(),
            ));
        }
        return Ok(CrossViewEvaluationResult {
            bundle_sha256: existing.bundle_object_sha256,
            aggregate_status: existing.aggregate_status,
            hard_gate_passed: existing.hard_gate_passed,
            strict_improvement: bundle["strict_improvement"].as_bool().unwrap_or(false),
            non_regressing: bundle["non_regressing"].as_bool().unwrap_or(false),
            baseline_score: bundle["baseline_score"].as_f64().unwrap_or(0.0),
            proposal_score: bundle["proposal_score"].as_f64().unwrap_or(0.0),
        });
    }
    evaluate_cross_view_proposal(
        runtime,
        session,
        source_candidate,
        proposal_candidate,
        view_evaluations,
    )
}

/// Produces or replays candidate-bound six-view evidence without claiming a
/// repair or promotion.  All durable identity/head/camera/canvas checks run
/// before the first comparison write.  Identical replay reads the immutable
/// bundle and starts no Render Worker.
pub(crate) fn evaluate_same_candidate_cross_view(
    runtime: &Runtime,
    session: &AgenticSessionRecord,
    candidate: &CandidateRecord,
    view_evaluations: &[ViewEvaluation],
) -> Result<CrossViewEvaluationResult, RuntimeError> {
    if session.project_id != candidate.project_id
        || session.candidate_id != candidate.candidate_id
        || session.candidate_state_sha256 != candidate.canonical_sha256
    {
        return Err(RuntimeError::InvalidInput(
            "CROSS_VIEW_SAME_CANDIDATE_SESSION_BINDING_MISMATCH".to_owned(),
        ));
    }
    let artifact_sha256 = candidate
        .prepared_object_sha256
        .as_deref()
        .filter(|hash| is_sha256(hash))
        .ok_or_else(|| {
            RuntimeError::InvalidInput("CROSS_VIEW_SAME_CANDIDATE_ARTIFACT_UNAVAILABLE".to_owned())
        })?;
    let head = runtime
        .store
        .get_production_stage_head_v3(
            &session.session_id,
            &session.project_id,
            &session.candidate_id,
        )?
        .ok_or_else(|| {
            RuntimeError::InvalidInput("CROSS_VIEW_CAMERA_HEAD_UNAVAILABLE".to_owned())
        })?;
    if head.head_stage != "camera-calibrated"
        || head.head_candidate_id != candidate.candidate_id
        || head.head_candidate_state_sha256 != candidate.canonical_sha256
        || head.head_artifact_sha256 != artifact_sha256
    {
        return Err(RuntimeError::InvalidInput(
            "CROSS_VIEW_CAMERA_HEAD_BINDING_MISMATCH".to_owned(),
        ));
    }
    let camera_lock_id = head.camera_lock_id.as_deref().ok_or_else(|| {
        RuntimeError::InvalidInput("CROSS_VIEW_CAMERA_LOCK_UNAVAILABLE".to_owned())
    })?;
    let camera_lock = runtime
        .store
        .get_production_camera_lock(camera_lock_id)?
        .ok_or_else(|| {
            RuntimeError::InvalidInput("CROSS_VIEW_CAMERA_LOCK_UNAVAILABLE".to_owned())
        })?;
    if head.camera_lock_canonical_sha256.as_deref() != Some(camera_lock.canonical_sha256.as_str())
        || head.camera_rig_object_sha256.as_deref()
            != Some(camera_lock.camera_rig_object_sha256.as_str())
        || head.camera_rig_canonical_sha256.as_deref()
            != Some(camera_lock.camera_rig_canonical_sha256.as_str())
        || head.camera_lock_receipt_object_sha256.as_deref()
            != Some(camera_lock.receipt_object_sha256.as_str())
    {
        return Err(RuntimeError::InvalidInput(
            "CROSS_VIEW_CAMERA_LOCK_BINDING_MISMATCH".to_owned(),
        ));
    }
    let (canvas, canvas_sha256) =
        super::agentic_session::durable_reference_canvas_for_session_binding(
            runtime,
            &session.project_id,
            &session.session_id,
            &session.candidate_id,
        )?;
    validate_cross_view_evaluation_coverage(&canvas, view_evaluations)?;

    if let Some(existing) = runtime.store.get_cross_view_evidence_by_identity(
        &session.project_id,
        &session.session_id,
        &candidate.candidate_id,
        &canvas_sha256,
    )? {
        let bundle = read_json_object(runtime, &existing.bundle_object_sha256)?;
        super::validate_cross_view_evidence_bundle(&bundle)?;
        if bundle.get("candidate_state_sha256").and_then(Value::as_str)
            != Some(candidate.canonical_sha256.as_str())
            || bundle.get("artifact_sha256").and_then(Value::as_str) != Some(artifact_sha256)
        {
            return Err(RuntimeError::InvalidInput(
                "CROSS_VIEW_EVIDENCE_CONFLICT".to_owned(),
            ));
        }
        let existing_views = bundle["view_evaluations"]
            .as_array()
            .ok_or_else(|| RuntimeError::InvalidInput("CROSS_VIEW_EVIDENCE_INVALID".to_owned()))?;
        let views_match = existing_views.len() == view_evaluations.len()
            && view_evaluations.iter().all(|expected| {
                existing_views.iter().any(|actual| {
                    actual.get("view_id").and_then(Value::as_str) == Some(expected.view_id.as_str())
                        && actual.get("kind").and_then(Value::as_str)
                            == Some(expected.kind.as_str())
                        && actual.get("reference_id").and_then(Value::as_str)
                            == Some(expected.reference_id.as_str())
                        && actual.get("reference_sha256").and_then(Value::as_str)
                            == Some(expected.reference_sha256.as_str())
                        && actual.get("camera_hash").and_then(Value::as_str)
                            == expected.camera.get("camera_hash").and_then(Value::as_str)
                })
            });
        if !views_match {
            return Err(RuntimeError::InvalidInput(
                "CROSS_VIEW_EVIDENCE_CONFLICT".to_owned(),
            ));
        }
        return Ok(CrossViewEvaluationResult {
            bundle_sha256: existing.bundle_object_sha256,
            aggregate_status: existing.aggregate_status,
            hard_gate_passed: existing.hard_gate_passed,
            strict_improvement: bundle["strict_improvement"].as_bool().unwrap_or(false),
            non_regressing: bundle["non_regressing"].as_bool().unwrap_or(false),
            baseline_score: bundle["baseline_score"].as_f64().unwrap_or(0.0),
            proposal_score: bundle["proposal_score"].as_f64().unwrap_or(0.0),
        });
    }
    evaluate_cross_view_proposal(runtime, session, candidate, candidate, view_evaluations)
}

fn evaluate_cross_view_proposal(
    runtime: &Runtime,
    session: &AgenticSessionRecord,
    source_candidate: &CandidateRecord,
    proposal_candidate: &CandidateRecord,
    view_evaluations: &[ViewEvaluation],
) -> Result<CrossViewEvaluationResult, RuntimeError> {
    let reservation = runtime.store.begin_cas_reservation();
    let mut reserved_objects = Vec::new();
    let result = evaluate_cross_view_proposal_reserved(
        runtime,
        session,
        source_candidate,
        proposal_candidate,
        view_evaluations,
        &reservation,
        &mut reserved_objects,
    );
    let cleanup = result.is_err();
    let mut rollback_error = None;
    for object in reserved_objects.iter().rev() {
        if let Err(error) = runtime.store.release_cas_reservation_object(
            &reservation,
            object,
            cleanup && object.created_new,
        ) {
            rollback_error.get_or_insert(error);
        }
    }
    match (result, rollback_error) {
        (Ok(value), None) => Ok(value),
        (Err(error), None) => Err(error),
        (Ok(_), Some(error)) => Err(RuntimeError::Store(error)),
        (Err(error), Some(rollback)) => Err(RuntimeError::InvalidInput(format!(
            "{error}; CROSS_VIEW_ROLLBACK_FAILED: {rollback}"
        ))),
    }
}

fn evaluate_cross_view_proposal_reserved(
    runtime: &Runtime,
    session: &AgenticSessionRecord,
    source_candidate: &CandidateRecord,
    proposal_candidate: &CandidateRecord,
    view_evaluations: &[ViewEvaluation],
    reservation: &forgecad_store::CasReservation,
    reserved_objects: &mut Vec<forgecad_store::CasObject>,
) -> Result<CrossViewEvaluationResult, RuntimeError> {
    let (canvas, canvas_sha256) =
        super::agentic_session::durable_reference_canvas_for_session_binding(
            runtime,
            &session.project_id,
            &session.session_id,
            &session.candidate_id,
        )?;
    let mut per_view = Vec::with_capacity(view_evaluations.len());
    let mut all_pass = true;
    let mut all_non_regressing = true;
    let mut all_strict_improvement = true;
    let mut baseline_total = 0.0;
    let mut proposal_total = 0.0;
    for view in view_evaluations {
        let mut baseline_request = json!({
                "project_id":session.project_id,
                "candidate_id":source_candidate.candidate_id,
                "reference_id":view.reference_id,
                "view_id":view.view_id,
                "view_spec":view.view_spec,
                "camera":view.camera
        });
        if let Some(target_sha256) = view.target_sha256.as_deref() {
            baseline_request["target_sha256"] = Value::String(target_sha256.to_owned());
        }
        let baseline = runtime.prepare_reference_comparison_detached(
            &session.project_id,
            baseline_request,
            reservation,
            reserved_objects,
        )?;
        let mut proposal_request = json!({
                "project_id":session.project_id,
                "candidate_id":proposal_candidate.candidate_id,
                "reference_id":view.reference_id,
                "view_id":view.view_id,
                "view_spec":view.view_spec,
                "camera":view.camera
        });
        if let Some(target_sha256) = view.target_sha256.as_deref() {
            proposal_request["target_sha256"] = Value::String(target_sha256.to_owned());
        }
        let proposal = runtime.prepare_reference_comparison_detached(
            &session.project_id,
            proposal_request,
            reservation,
            reserved_objects,
        )?;
        let baseline_report = baseline
            .get("comparison_report")
            .and_then(Value::as_object)
            .ok_or_else(|| {
                RuntimeError::InvalidInput("CROSS_VIEW_BASELINE_COMPARISON_MISSING".to_owned())
            })?;
        let proposal_report = proposal
            .get("comparison_report")
            .and_then(Value::as_object)
            .ok_or_else(|| {
                RuntimeError::InvalidInput("CROSS_VIEW_PROPOSAL_COMPARISON_MISSING".to_owned())
            })?;
        let baseline_metrics = Value::Object(
            baseline_report
                .get("metrics")
                .and_then(Value::as_object)
                .cloned()
                .ok_or_else(|| {
                    RuntimeError::InvalidInput("CROSS_VIEW_BASELINE_METRICS_MISSING".to_owned())
                })?,
        );
        let proposal_metrics = Value::Object(
            proposal_report
                .get("metrics")
                .and_then(Value::as_object)
                .cloned()
                .ok_or_else(|| {
                    RuntimeError::InvalidInput("CROSS_VIEW_PROPOSAL_METRICS_MISSING".to_owned())
                })?,
        );
        let (non_regressing, strict_improvement, baseline_score, proposal_score) =
            compare_visual_metrics(&baseline_metrics, &proposal_metrics)?;
        let baseline_status = baseline_report
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("QUALITY_TARGET_NOT_MET");
        let proposal_status = proposal_report
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("QUALITY_TARGET_NOT_MET");
        all_pass &= proposal_status == "PARTIAL_VISIBLE_VIEW_PASS";
        all_non_regressing &= non_regressing;
        all_strict_improvement &= strict_improvement;
        baseline_total += baseline_score;
        proposal_total += proposal_score;
        per_view.push(json!({
            "view_id":view.view_id,
            "kind":view.kind,
            "visibility":view.visibility,
            "confidence":view.confidence,
            "reference_id":view.reference_id,
            "reference_sha256":view.reference_sha256,
            "camera_hash":view.camera["camera_hash"],
            "baseline_status":baseline_status,
            "proposal_status":proposal_status,
            "baseline_render_set_sha256":baseline["render_set_object_sha256"],
            "baseline_comparison_report_sha256":baseline["comparison_report_object_sha256"],
            "baseline_quality_report_sha256":baseline["quality_report_object_sha256"],
            "proposal_render_set_sha256":proposal["render_set_object_sha256"],
            "proposal_comparison_report_sha256":proposal["comparison_report_object_sha256"],
            "proposal_quality_report_sha256":proposal["quality_report_object_sha256"],
            "baseline_metrics":baseline_metrics,
            "proposal_metrics":proposal_metrics,
            "non_regressing":non_regressing,
            "strict_improvement":strict_improvement
        }));
    }
    let coverage = canvas.get("coverage").cloned().unwrap_or_else(|| json!({}));
    let coverage_complete = coverage.get("coverage_status").and_then(Value::as_str)
        == Some("complete")
        && coverage
            .get("missing_views")
            .and_then(Value::as_array)
            .is_some_and(Vec::is_empty);
    let aggregate_status = if !coverage_complete {
        "BLOCKED_REFERENCE_COVERAGE"
    } else if all_pass {
        "PARTIAL_VISIBLE_VIEW_PASS"
    } else {
        "QUALITY_TARGET_NOT_MET"
    };
    let hard_gate_passed = coverage_complete && all_pass;
    let strict_improvement = all_strict_improvement && all_pass;
    let non_regressing = all_non_regressing;
    let promotion = if strict_improvement {
        "reviewable"
    } else if non_regressing {
        "not-improved"
    } else {
        "rejected-regression"
    };
    let count = view_evaluations.len() as f64;
    let first_render_set_hash = per_view
        .first()
        .and_then(|view| view.get("proposal_render_set_sha256"))
        .and_then(Value::as_str)
        .ok_or_else(|| RuntimeError::InvalidInput("CROSS_VIEW_RENDER_SET_MISSING".to_owned()))?;
    let first_render_set = read_json_object(runtime, first_render_set_hash)?;
    let artifact_sha256 = first_render_set
        .get("artifact_sha256")
        .and_then(Value::as_str)
        .filter(|hash| is_sha256(hash))
        .ok_or_else(|| RuntimeError::InvalidInput("CROSS_VIEW_ARTIFACT_MISSING".to_owned()))?;
    let program_sha256 = first_render_set
        .get("program_sha256")
        .and_then(Value::as_str)
        .filter(|hash| is_sha256(hash))
        .ok_or_else(|| RuntimeError::InvalidInput("CROSS_VIEW_PROGRAM_MISSING".to_owned()))?;
    let mut bundle = json!({
        "schema_version":"CrossViewEvidenceBundle@1",
        "bundle_id":format!("cross-view-{}", &proposal_candidate.candidate_id[..proposal_candidate.candidate_id.len().min(32)]),
        "project_id":session.project_id,
        "session_id":session.session_id,
        "candidate_id":proposal_candidate.candidate_id,
        "candidate_state_sha256":proposal_candidate.canonical_sha256,
        "artifact_sha256":artifact_sha256,
        "program_sha256":program_sha256,
        "reference_canvas_sha256":canvas_sha256,
        "coverage":coverage,
        "view_evaluations":per_view,
        "aggregate_status":aggregate_status,
        "hard_gate_passed":hard_gate_passed,
        "baseline_score":baseline_total / count,
        "proposal_score":proposal_total / count,
        "strict_improvement":strict_improvement,
        "non_regressing":non_regressing,
        "promotion":{"status":promotion,"confirm_allowed":false},
        "limitations":["human_visual_review_not_run","export_restart_hash_not_run"],
        "canonical_sha256":""
    });
    // Stabilize serde_json's in-memory floating-number representation at the
    // exact canonical wire boundary before hashing. Candidate-derived camera
    // metrics can otherwise hash one Number spelling in memory and be parsed
    // back with an equivalent but different spelling by Store, which must
    // correctly reject the apparent canonical mismatch.
    bundle = serde_json::from_slice(
        &super::canonical_json_bytes(&bundle)
            .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?,
    )
    .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?;
    bundle["canonical_sha256"] = Value::String(super::canonical_json_hash(&bundle));
    super::validate_cross_view_evidence_bundle(&bundle)?;
    let object = runtime.store.put_object_reserved(
        reservation,
        &super::canonical_json_bytes(&bundle)
            .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?,
        None,
        "application/json",
        "cross-view-evidence-bundle",
        &super::now_string(),
    )?;
    reserved_objects.push(object.clone());
    let now = super::now_string();
    runtime
        .store
        .insert_cross_view_evidence(&super::CrossViewEvidenceRecord {
            bundle_object_sha256: object.record.sha256.clone(),
            candidate_id: proposal_candidate.candidate_id.clone(),
            project_id: session.project_id.clone(),
            session_id: session.session_id.clone(),
            reference_canvas_sha256: bundle["reference_canvas_sha256"]
                .as_str()
                .unwrap_or_default()
                .to_owned(),
            aggregate_status: aggregate_status.to_owned(),
            hard_gate_passed,
            created_at: now.clone(),
            updated_at: now,
        })?;
    Ok(CrossViewEvaluationResult {
        bundle_sha256: object.record.sha256,
        aggregate_status: aggregate_status.to_owned(),
        hard_gate_passed,
        strict_improvement,
        non_regressing,
        baseline_score: bundle["baseline_score"]
            .as_f64()
            .unwrap_or(baseline_total / count),
        proposal_score: bundle["proposal_score"]
            .as_f64()
            .unwrap_or(proposal_total / count),
    })
}

fn validate_repair_proposal(
    runtime: &Runtime,
    action: &Value,
    intent: &Value,
    geometry_program: &Value,
    view_spec: &Value,
    camera: &Value,
    session: &AgenticSessionRecord,
    source_candidate: &CandidateRecord,
    source_geometry: &GeometryBindings,
    requested_stage: &str,
) -> Result<(), RuntimeError> {
    let action_object = action
        .as_object()
        .ok_or_else(|| RuntimeError::InvalidInput("REPAIR_ACTION_INVALID".to_owned()))?;
    let action_kind = action_object
        .get("action_kind")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if !matches!(
        action_kind,
        "bounded-repair"
            | "primary-blockout"
            | "primary-form-adjustment"
            | "secondary-structure"
            | "tertiary-detail"
    ) || action_object.get("scope_kind").and_then(Value::as_str) != Some("part")
    {
        return Err(RuntimeError::InvalidInput(
            "REPAIR_ACTION_SCOPE_REQUIRED: geometry design action must target one Part".to_owned(),
        ));
    }
    let expected_stage = match action_kind {
        "primary-blockout" | "primary-form-adjustment" => Some("primary-form"),
        "secondary-structure" => Some("secondary-structure"),
        "tertiary-detail" => Some("tertiary-detail"),
        "bounded-repair" => None,
        _ => unreachable!("action kind was checked above"),
    };
    if let Some(expected_stage) = expected_stage {
        if requested_stage != expected_stage {
            return Err(RuntimeError::InvalidInput(
                "REPAIR_ACTION_STAGE_MISMATCH: geometry design action is outside its stage"
                    .to_owned(),
            ));
        }
    }
    let target_part_id = action_object
        .get("target_id")
        .and_then(Value::as_str)
        .filter(|value| is_opaque_id(value))
        .ok_or_else(|| RuntimeError::InvalidInput("REPAIR_PART_TARGET_REQUIRED".to_owned()))?;
    let changes = action_object
        .get("parameter_changes")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            RuntimeError::InvalidInput("REPAIR_PARAMETER_CHANGES_REQUIRED".to_owned())
        })?;
    if changes.is_empty() {
        return Err(RuntimeError::InvalidInput(
            "REPAIR_PARAMETER_CHANGES_REQUIRED: at least one bounded change is required".to_owned(),
        ));
    }

    let intent_object = intent
        .as_object()
        .ok_or_else(|| RuntimeError::InvalidInput("REPAIR_INTENT_INVALID".to_owned()))?;
    if intent_object.get("schema_version").and_then(Value::as_str) != Some("RepairIntent@1")
        || !matches!(
            intent_object.get("status").and_then(Value::as_str),
            Some("proposed" | "approved")
        )
        || intent_object.get("approval_required") != Some(&Value::Bool(true))
        || intent_object.get("runtime_write") != Some(&Value::Bool(false))
    {
        return Err(RuntimeError::InvalidInput(
            "REPAIR_INTENT_NOT_APPLICABLE: intent must be proposed/approved and approval-gated"
                .to_owned(),
        ));
    }
    for (key, expected) in [
        ("session_id", session.session_id.as_str()),
        ("project_id", session.project_id.as_str()),
        ("candidate_id", source_candidate.candidate_id.as_str()),
        ("reference_id", session.reference_id.as_str()),
        ("reference_sha256", session.reference_sha256.as_str()),
        ("camera_hash", session.camera_hash.as_str()),
        ("observation_sha256", session.observation_sha256.as_str()),
    ] {
        if intent_object.get(key).and_then(Value::as_str) != Some(expected) {
            return Err(RuntimeError::InvalidInput(
                "REPAIR_INTENT_BINDING_MISMATCH".to_owned(),
            ));
        }
    }
    for key in [
        "candidate_state_sha256",
        "source_evidence_sha256",
        "source_critic_report_sha256",
        "canonical_sha256",
    ] {
        if !intent_object
            .get(key)
            .and_then(Value::as_str)
            .is_some_and(is_sha256)
        {
            return Err(RuntimeError::InvalidInput(format!(
                "REPAIR_INTENT_INVALID: {key} must be SHA-256"
            )));
        }
    }
    if intent_object
        .get("candidate_state_sha256")
        .and_then(Value::as_str)
        != Some(source_candidate.canonical_sha256.as_str())
        || intent_object
            .get("source_evidence_sha256")
            .and_then(Value::as_str)
            != Some(session.evidence_sha256.as_str())
        || intent_object.get("stage").and_then(Value::as_str) != Some(requested_stage)
    {
        return Err(RuntimeError::InvalidInput(
            "REPAIR_INTENT_EVIDENCE_MISMATCH".to_owned(),
        ));
    }
    let scope = intent_object
        .get("scope")
        .and_then(Value::as_object)
        .ok_or_else(|| RuntimeError::InvalidInput("REPAIR_INTENT_SCOPE_INVALID".to_owned()))?;
    if scope.get("kind").and_then(Value::as_str) != Some("part")
        || scope.get("part_id").and_then(Value::as_str) != Some(target_part_id)
    {
        return Err(RuntimeError::InvalidInput(
            "REPAIR_INTENT_SCOPE_MISMATCH".to_owned(),
        ));
    }
    let intent_action = intent_object
        .get("action")
        .and_then(Value::as_object)
        .ok_or_else(|| RuntimeError::InvalidInput("REPAIR_INTENT_ACTION_INVALID".to_owned()))?;
    if intent_action.get("action_kind").and_then(Value::as_str) != Some("bounded-repair")
        || intent_action.get("bounded") != Some(&Value::Bool(true))
        || intent_action.get("parameter_changes") != Some(&Value::Array(changes.clone()))
    {
        return Err(RuntimeError::InvalidInput(
            "REPAIR_INTENT_ACTION_MISMATCH".to_owned(),
        ));
    }
    let precondition = intent_object
        .get("precondition")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            RuntimeError::InvalidInput("REPAIR_INTENT_PRECONDITION_INVALID".to_owned())
        })?;
    if precondition
        .get("current_candidate_state_sha256")
        .and_then(Value::as_str)
        != Some(source_candidate.canonical_sha256.as_str())
        || precondition.get("evidence_sha256").and_then(Value::as_str)
            != Some(session.evidence_sha256.as_str())
    {
        return Err(RuntimeError::InvalidInput(
            "REPAIR_INTENT_PRECONDITION_MISMATCH".to_owned(),
        ));
    }
    let recompute = intent_object
        .get("recompute")
        .and_then(Value::as_object)
        .ok_or_else(|| RuntimeError::InvalidInput("REPAIR_INTENT_RECOMPUTE_INVALID".to_owned()))?;
    if recompute.get("steps") != Some(&json!(["compile", "readback", "render", "compare"]))
        || recompute.get("must_rebind_reference") != Some(&Value::Bool(true))
        || recompute.get("must_rebind_camera") != Some(&Value::Bool(true))
        || recompute.get("confirm_allowed") != Some(&Value::Bool(false))
    {
        return Err(RuntimeError::InvalidInput(
            "REPAIR_INTENT_RECOMPUTE_INVALID".to_owned(),
        ));
    }
    let mut intent_without_hash = intent.clone();
    intent_without_hash["canonical_sha256"] = Value::String(String::new());
    if canonical_json_hash(&intent_without_hash)
        != intent_object
            .get("canonical_sha256")
            .and_then(Value::as_str)
            .unwrap_or_default()
    {
        return Err(RuntimeError::InvalidInput(
            "REPAIR_INTENT_CANONICAL_HASH_MISMATCH".to_owned(),
        ));
    }

    let program_object = geometry_program
        .as_object()
        .ok_or_else(|| RuntimeError::InvalidInput("REPAIR_GEOMETRY_PROGRAM_INVALID".to_owned()))?;
    if program_object.get("schema_version").and_then(Value::as_str) != Some("GeometryProgram@2")
        || program_object.get("project_id").and_then(Value::as_str)
            != Some(session.project_id.as_str())
        || program_object
            .get("operator_catalog_sha256")
            .and_then(Value::as_str)
            != Some(source_geometry.evidence.operator_catalog_sha256.as_str())
    {
        return Err(RuntimeError::InvalidInput(
            "REPAIR_GEOMETRY_PROGRAM_BINDING_MISMATCH".to_owned(),
        ));
    }
    let mut proposed_draft = geometry_program.clone();
    proposed_draft
        .as_object_mut()
        .expect("GeometryProgram object was checked")
        .remove("canonical_sha256");
    let proposed_hash =
        hash_geometry_program_with_runtime_worker(&proposed_draft).map_err(|error| {
            RuntimeError::InvalidInput(format!("REPAIR_GEOMETRY_PROGRAM_HASH_FAILED: {error}"))
        })?;
    if proposed_hash
        .get("canonical_sha256")
        .and_then(Value::as_str)
        != program_object
            .get("canonical_sha256")
            .and_then(Value::as_str)
    {
        return Err(RuntimeError::InvalidInput(
            "REPAIR_GEOMETRY_PROGRAM_CANONICAL_MISMATCH".to_owned(),
        ));
    }
    validate_single_part_geometry_change(
        source_geometry.program.as_object().ok_or_else(|| {
            RuntimeError::InvalidInput("REPAIR_BASELINE_PROGRAM_INVALID".to_owned())
        })?,
        program_object,
        target_part_id,
    )?;
    if matches!(
        runtime_parameter_patch_strategy(action),
        Ok("rear-stock-profile-reconstruction-v1")
    ) {
        validate_exact_rear_stock_source_node_change(
            source_geometry.program.as_object().ok_or_else(|| {
                RuntimeError::InvalidInput("REPAIR_BASELINE_PROGRAM_INVALID".to_owned())
            })?,
            program_object,
        )?;
    }

    let reference = runtime
        .reference(&session.reference_id)?
        .ok_or_else(|| RuntimeError::InvalidInput("REPAIR_REFERENCE_NOT_FOUND".to_owned()))?;
    super::validate_reference_view_spec(view_spec, &reference)?;
    super::validate_camera_calibration(camera)?;
    if camera.get("camera_hash").and_then(Value::as_str) != Some(session.camera_hash.as_str()) {
        return Err(RuntimeError::InvalidInput(
            "REPAIR_CAMERA_BINDING_MISMATCH_VALIDATE".to_owned(),
        ));
    }
    Ok(())
}

fn validate_exact_rear_stock_source_node_change(
    baseline: &Map<String, Value>,
    proposed: &Map<String, Value>,
) -> Result<(), RuntimeError> {
    let baseline_nodes = node_map(baseline)?;
    let proposed_nodes = node_map(proposed)?;
    if baseline_nodes.len() != proposed_nodes.len()
        || baseline_nodes.keys().collect::<HashSet<_>>()
            != proposed_nodes.keys().collect::<HashSet<_>>()
    {
        return Err(RuntimeError::InvalidInput(
            "REPAIR_STOCK_PROFILE_SOURCE_NODE_SET_CHANGED".to_owned(),
        ));
    }
    let changed = baseline_nodes
        .iter()
        .filter_map(|(node_id, before)| {
            (proposed_nodes.get(node_id) != Some(before)).then_some(node_id.as_str())
        })
        .collect::<HashSet<_>>();
    if changed != HashSet::from(["rear-stock"])
        || baseline_nodes
            .get("rear-stock")
            .and_then(|node| node.get("operator_id"))
            .and_then(Value::as_str)
            != Some("forgecad.geometry.primitive@2")
        || proposed_nodes
            .get("rear-stock")
            .and_then(|node| node.get("operator_id"))
            .and_then(Value::as_str)
            != Some("forgecad.geometry.profile-loft@2")
        || part_output_map(baseline)? != part_output_map(proposed)?
    {
        return Err(RuntimeError::InvalidInput(
            "REPAIR_STOCK_PROFILE_EXACT_SOURCE_NODE_REQUIRED".to_owned(),
        ));
    }
    Ok(())
}

fn persist_repair_intent(
    runtime: &Runtime,
    intent: &Value,
) -> Result<(String, String), RuntimeError> {
    let canonical_sha256 = intent
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| RuntimeError::InvalidInput("REPAIR_INTENT_CANONICAL_MISSING".to_owned()))?
        .to_owned();
    let bytes = canonical_json_bytes(intent)
        .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?;
    let object = runtime.put_object(&bytes, None, "application/json", "agentic-repair-intent")?;
    Ok((canonical_sha256, object.record.sha256))
}

fn validate_single_part_geometry_change(
    baseline: &Map<String, Value>,
    proposed: &Map<String, Value>,
    target_part_id: &str,
) -> Result<(), RuntimeError> {
    let baseline_nodes = node_map(baseline)?;
    let proposed_nodes = node_map(proposed)?;
    let mut changed_nodes = HashSet::new();
    let node_ids: HashSet<String> = baseline_nodes
        .keys()
        .chain(proposed_nodes.keys())
        .cloned()
        .collect();
    for node_id in node_ids {
        if baseline_nodes.get(&node_id) != proposed_nodes.get(&node_id) {
            changed_nodes.insert(node_id);
        }
    }
    let baseline_parts = part_output_map(baseline)?;
    let proposed_parts = part_output_map(proposed)?;
    let part_ids: HashSet<String> = baseline_parts
        .keys()
        .chain(proposed_parts.keys())
        .cloned()
        .collect();
    if !part_ids.contains(target_part_id) {
        return Err(RuntimeError::InvalidInput(
            "REPAIR_PART_TARGET_NOT_IN_PROGRAM".to_owned(),
        ));
    }
    let mut changed_parts = HashSet::new();
    let mut claimed_nodes = HashSet::new();
    for part_id in &part_ids {
        let before = baseline_parts.get(part_id);
        let after = proposed_parts.get(part_id);
        let output_changed = before != after;
        let before_roots = part_roots(before);
        let after_roots = part_roots(after);
        let before_closure = node_closure(&baseline_nodes, &before_roots);
        let after_closure = node_closure(&proposed_nodes, &after_roots);
        let affects_part = changed_nodes
            .iter()
            .any(|node_id| before_closure.contains(node_id) || after_closure.contains(node_id));
        if output_changed || affects_part {
            changed_parts.insert(part_id.clone());
        }
        for node_id in changed_nodes.iter() {
            if before_closure.contains(node_id) || after_closure.contains(node_id) {
                claimed_nodes.insert(node_id.clone());
            }
        }
    }
    if changed_nodes.is_empty() || changed_parts != HashSet::from([target_part_id.to_owned()]) {
        return Err(RuntimeError::InvalidInput(
            "REPAIR_SINGLE_PART_SCOPE_VIOLATION: exactly one Part must change".to_owned(),
        ));
    }
    if claimed_nodes != changed_nodes {
        return Err(RuntimeError::InvalidInput(
            "REPAIR_SINGLE_PART_SCOPE_VIOLATION: changed node is not owned by the target Part"
                .to_owned(),
        ));
    }
    Ok(())
}

fn node_map(program: &Map<String, Value>) -> Result<HashMap<String, Value>, RuntimeError> {
    let nodes = program
        .get("nodes")
        .and_then(Value::as_array)
        .ok_or_else(|| RuntimeError::InvalidInput("REPAIR_PROGRAM_NODES_INVALID".to_owned()))?;
    let mut result = HashMap::new();
    for node in nodes {
        let object = node
            .as_object()
            .ok_or_else(|| RuntimeError::InvalidInput("REPAIR_PROGRAM_NODE_INVALID".to_owned()))?;
        let node_id = object
            .get("node_id")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                RuntimeError::InvalidInput("REPAIR_PROGRAM_NODE_ID_MISSING".to_owned())
            })?;
        if result.insert(node_id.to_owned(), node.clone()).is_some() {
            return Err(RuntimeError::InvalidInput(
                "REPAIR_PROGRAM_NODE_ID_DUPLICATE".to_owned(),
            ));
        }
    }
    Ok(result)
}

fn part_output_map(program: &Map<String, Value>) -> Result<HashMap<String, Value>, RuntimeError> {
    let outputs = program
        .get("part_outputs")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            RuntimeError::InvalidInput("REPAIR_PROGRAM_PART_OUTPUTS_INVALID".to_owned())
        })?;
    let mut result = HashMap::new();
    for output in outputs {
        let object = output.as_object().ok_or_else(|| {
            RuntimeError::InvalidInput("REPAIR_PROGRAM_PART_OUTPUT_INVALID".to_owned())
        })?;
        let part_id = object
            .get("part_id")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                RuntimeError::InvalidInput("REPAIR_PROGRAM_PART_ID_MISSING".to_owned())
            })?;
        if result.insert(part_id.to_owned(), output.clone()).is_some() {
            return Err(RuntimeError::InvalidInput(
                "REPAIR_PROGRAM_PART_ID_DUPLICATE".to_owned(),
            ));
        }
    }
    Ok(result)
}

fn part_roots(part: Option<&Value>) -> Vec<String> {
    part.and_then(Value::as_object)
        .and_then(|object| object.get("input_node_ids"))
        .and_then(Value::as_array)
        .map(|ids| {
            ids.iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

fn node_closure(nodes: &HashMap<String, Value>, roots: &[String]) -> HashSet<String> {
    let mut result = HashSet::new();
    let mut pending = roots.to_vec();
    while let Some(node_id) = pending.pop() {
        if !result.insert(node_id.clone()) {
            continue;
        }
        if let Some(inputs) = nodes
            .get(&node_id)
            .and_then(Value::as_object)
            .and_then(|object| object.get("inputs"))
            .and_then(Value::as_array)
        {
            pending.extend(inputs.iter().filter_map(Value::as_str).map(str::to_owned));
        }
    }
    result
}

fn compare_visual_metrics(
    baseline: &Value,
    proposal: &Value,
) -> Result<(bool, bool, f64, f64), RuntimeError> {
    let metrics = [
        ("silhouette_iou", true),
        ("boundary_f1_4px", true),
        ("bbox_edge_error", false),
        ("centroid_error", false),
        ("landmark_coverage", true),
        ("landmark_nme", false),
        ("region_median_iou", true),
        ("critical_region_min_iou", true),
    ];
    let mut non_regressing = true;
    let mut strict = false;
    let mut baseline_score = 0.0;
    let mut proposal_score = 0.0;
    for (key, higher_is_better) in metrics {
        let before = finite_metric(baseline, key)?;
        let after = finite_metric(proposal, key)?;
        baseline_score += if higher_is_better {
            before
        } else {
            (1.0 - before).max(0.0)
        };
        proposal_score += if higher_is_better {
            after
        } else {
            (1.0 - after).max(0.0)
        };
        let delta = if higher_is_better {
            after - before
        } else {
            before - after
        };
        if delta < -1e-9 {
            non_regressing = false;
        }
        if delta > 1e-9 {
            strict = true;
        }
    }
    Ok((
        non_regressing,
        non_regressing && strict,
        baseline_score,
        proposal_score,
    ))
}

fn finite_metric(metrics: &Value, key: &str) -> Result<f64, RuntimeError> {
    let value = metrics
        .get(key)
        .and_then(Value::as_f64)
        .ok_or_else(|| RuntimeError::InvalidInput(format!("REPAIR_METRIC_MISSING: {key}")))?;
    if !value.is_finite() {
        return Err(RuntimeError::InvalidInput(format!(
            "REPAIR_METRIC_INVALID: {key}"
        )));
    }
    Ok(value)
}

fn finalize_run(run: &mut Value) {
    run["canonical_sha256"] = Value::String(String::new());
    run["canonical_sha256"] = Value::String(canonical_json_hash(run));
}

pub(crate) fn load_geometry_bindings(
    runtime: &Runtime,
    candidate: &CandidateRecord,
    project_id: &str,
    session: &AgenticSessionRecord,
) -> Result<GeometryBindings, RuntimeError> {
    let evidence = runtime
        .store
        .get_geometry_candidate_evidence(&candidate.candidate_id)?
        .ok_or_else(|| RuntimeError::InvalidInput("GEOMETRY_EVIDENCE_UNAVAILABLE".to_owned()))?;
    if evidence.project_id != project_id
        || evidence.candidate_id != candidate.candidate_id
        || evidence.reference_id.as_deref() != Some(session.reference_id.as_str())
        || evidence.reference_sha256.as_deref() != Some(session.reference_sha256.as_str())
    {
        return Err(RuntimeError::InvalidInput(
            "GEOMETRY_EVIDENCE_BINDING_MISMATCH".to_owned(),
        ));
    }
    let artifact_sha256 = evidence.artifact_object_sha256.clone();
    if candidate.prepared_object_sha256.as_deref() != Some(artifact_sha256.as_str())
        && candidate.manifest_hash.as_deref() != Some(artifact_sha256.as_str())
    {
        return Err(RuntimeError::InvalidInput(
            "GEOMETRY_ARTIFACT_BINDING_MISMATCH".to_owned(),
        ));
    }
    let program_object = runtime
        .store
        .get_object(&evidence.geometry_program_object_sha256)?
        .ok_or_else(|| {
            RuntimeError::InvalidInput("GEOMETRY_PROGRAM_OBJECT_UNAVAILABLE".to_owned())
        })?;
    let program_bytes = runtime.cas_read(&program_object.sha256)?;
    let mut program: Value = serde_json::from_slice(&program_bytes)
        .map_err(|_| RuntimeError::InvalidInput("GEOMETRY_PROGRAM_OBJECT_INVALID".to_owned()))?;
    if program.get("schema_version").and_then(Value::as_str) != Some("GeometryProgram@2")
        || sha256_hex(&program_bytes) != evidence.geometry_program_object_sha256
    {
        return Err(RuntimeError::InvalidInput(
            "GEOMETRY_PROGRAM_OBJECT_BINDING_MISMATCH".to_owned(),
        ));
    }
    let hash_result = hash_geometry_program_with_runtime_worker(&program)
        .map_err(|_| RuntimeError::InvalidInput("GEOMETRY_PROGRAM_HASH_FAILED".to_owned()))?;
    if hash_result.get("canonical_sha256").and_then(Value::as_str)
        != Some(evidence.geometry_program_sha256.as_str())
        || hash_result
            .get("operator_catalog_sha256")
            .and_then(Value::as_str)
            != Some(evidence.operator_catalog_sha256.as_str())
    {
        return Err(RuntimeError::InvalidInput(
            "GEOMETRY_PROGRAM_LINEAGE_MISMATCH".to_owned(),
        ));
    }
    // GeometryProgram@2 is stored in CAS as the canonical hash-free draft so
    // the object hash is stable and independently auditable.  The bounded
    // Worker compile contract, however, consumes the same draft with its
    // already-verified canonical_sha256 field restored.  Reconstructing that
    // field here keeps the ActionRun revalidation byte/lineage equivalent to
    // the original candidate prepare path without trusting a caller value.
    program["canonical_sha256"] = Value::String(evidence.geometry_program_sha256.clone());
    Ok(GeometryBindings {
        evidence,
        program,
        artifact_sha256,
    })
}

pub(crate) fn recompile_candidate(
    _runtime: &Runtime,
    geometry: &GeometryBindings,
) -> Result<super::integrity::GlbIntegrity, RuntimeError> {
    let artifact = super::compile_geometry_with_runtime_worker(&geometry.program, None)
        .map_err(|_| RuntimeError::InvalidInput("GEOMETRY_COMPILE_FAILED".to_owned()))?;
    let inspection = strict_glb_inspection(&artifact.glb)?;
    validate_worker_metadata(&artifact, &inspection)?;
    if sha256_hex(&artifact.glb) != geometry.artifact_sha256
        || inspection.program_sha256 != geometry.evidence.geometry_program_sha256
        || inspection.operator_catalog_sha256.as_deref()
            != Some(geometry.evidence.operator_catalog_sha256.as_str())
        || inspection.readback_config_sha256 != geometry.evidence.readback_config_sha256
        || !inspection.hard_gate_passed
    {
        return Err(RuntimeError::InvalidInput(
            "GEOMETRY_COMPILE_LINEAGE_MISMATCH".to_owned(),
        ));
    }
    Ok(inspection)
}

/// Revalidate an immutable candidate artifact without requiring its embedded
/// historical build-cohort metadata to equal the currently running Worker.
///
/// This is the source side of a cohort transition only. The stored GLB must
/// still be byte/hash bound, pass strict structural readback, and match the
/// persisted GeometryProgram/catalog/readback lineage. Any newly prepared
/// child is compiled normally by the current cohort and receives its own
/// current-cohort artifact and evidence.
pub(crate) fn inspect_persisted_candidate_for_cohort_transition(
    runtime: &Runtime,
    geometry: &GeometryBindings,
) -> Result<super::integrity::GlbIntegrity, RuntimeError> {
    let artifact_object = runtime
        .store
        .get_object(&geometry.artifact_sha256)?
        .ok_or_else(|| {
            RuntimeError::InvalidInput("GEOMETRY_ARTIFACT_OBJECT_UNAVAILABLE".to_owned())
        })?;
    let artifact_bytes = runtime.cas_read(&artifact_object.sha256)?;
    if artifact_object.sha256 != geometry.artifact_sha256
        || sha256_hex(&artifact_bytes) != geometry.artifact_sha256
    {
        return Err(RuntimeError::InvalidInput(
            "GEOMETRY_ARTIFACT_OBJECT_BINDING_MISMATCH".to_owned(),
        ));
    }
    let inspection = strict_glb_inspection(&artifact_bytes)?;
    if inspection.program_sha256 != geometry.evidence.geometry_program_sha256
        || inspection.operator_catalog_sha256.as_deref()
            != Some(geometry.evidence.operator_catalog_sha256.as_str())
        || inspection.readback_config_sha256 != geometry.evidence.readback_config_sha256
        || !inspection.hard_gate_passed
    {
        return Err(RuntimeError::InvalidInput(
            "GEOMETRY_PERSISTED_COHORT_TRANSITION_LINEAGE_MISMATCH".to_owned(),
        ));
    }
    Ok(inspection)
}

pub(crate) fn verify_artifact_readback(
    runtime: &Runtime,
    candidate: &CandidateRecord,
    geometry: &GeometryBindings,
    inspection: &super::integrity::GlbIntegrity,
) -> Result<String, RuntimeError> {
    let readback = runtime.artifact_readback(&geometry.artifact_sha256, &candidate.candidate_id)?;
    if readback.get("schema_version").and_then(Value::as_str) != Some("ArtifactReadback@2")
        || readback.get("artifact_id").and_then(Value::as_str)
            != Some(geometry.artifact_sha256.as_str())
        || readback.get("candidate_id").and_then(Value::as_str)
            != Some(candidate.candidate_id.as_str())
        || readback.get("program_sha256").and_then(Value::as_str)
            != Some(geometry.evidence.geometry_program_sha256.as_str())
    {
        return Err(RuntimeError::InvalidInput(
            "ARTIFACT_READBACK_BINDING_MISMATCH".to_owned(),
        ));
    }
    if readback.get("hard_gate_passed") != Some(&Value::Bool(inspection.hard_gate_passed)) {
        return Err(RuntimeError::InvalidInput(
            "ARTIFACT_READBACK_GATE_MISMATCH".to_owned(),
        ));
    }
    let readback_sha256 = geometry.evidence.artifact_readback_object_sha256.as_str();
    let stored_bytes = runtime.cas_read(readback_sha256)?;
    let stored: Value = serde_json::from_slice(&stored_bytes)
        .map_err(|_| RuntimeError::InvalidInput("ARTIFACT_READBACK_OBJECT_INVALID".to_owned()))?;
    super::validate_artifact_readback_v2_output(&stored)?;
    if stored != readback
        || sha256_hex(&stored_bytes) != readback_sha256
        || runtime.store.get_object(readback_sha256)?.is_none()
    {
        return Err(RuntimeError::InvalidInput(
            "ARTIFACT_READBACK_OBJECT_BINDING_MISMATCH".to_owned(),
        ));
    }
    Ok(readback_sha256.to_owned())
}

fn verify_visual_bindings(
    runtime: &Runtime,
    candidate: &CandidateRecord,
    geometry: &GeometryBindings,
    session: &AgenticSessionRecord,
    project_id: &str,
) -> Result<VisualBindings, RuntimeError> {
    let evidence = runtime
        .store
        .get_visual_evidence(&candidate.candidate_id)?
        .ok_or_else(|| RuntimeError::InvalidInput("VISUAL_EVIDENCE_UNAVAILABLE".to_owned()))?;
    if evidence.project_id != project_id
        || evidence.candidate_id != candidate.candidate_id
        || evidence.reference_id != session.reference_id
    {
        return Err(RuntimeError::InvalidInput(
            "VISUAL_EVIDENCE_BINDING_MISMATCH".to_owned(),
        ));
    }
    let render_set = read_json_object(runtime, &evidence.render_set_object_sha256)?;
    validate_render_set_v2_output(&render_set)?;
    if render_set.get("candidate_id").and_then(Value::as_str)
        != Some(candidate.candidate_id.as_str())
        || render_set.get("artifact_sha256").and_then(Value::as_str)
            != Some(geometry.artifact_sha256.as_str())
        || render_set.get("program_sha256").and_then(Value::as_str)
            != Some(geometry.evidence.geometry_program_sha256.as_str())
        || render_set.get("reference_id").and_then(Value::as_str)
            != Some(session.reference_id.as_str())
        || render_set.get("camera_hash").and_then(Value::as_str)
            != Some(session.camera_hash.as_str())
    {
        return Err(RuntimeError::InvalidInput(
            "RENDER_SET_BINDING_MISMATCH".to_owned(),
        ));
    }
    let camera_object_sha256 = render_set
        .get("camera_object_sha256")
        .and_then(Value::as_str)
        .filter(|hash| is_sha256(hash))
        .ok_or_else(|| RuntimeError::InvalidInput("CAMERA_EVIDENCE_UNAVAILABLE".to_owned()))?;
    let camera = read_json_object(runtime, camera_object_sha256)?;
    super::validate_camera_calibration(&camera)?;
    if camera.get("camera_hash").and_then(Value::as_str) != Some(session.camera_hash.as_str())
        || camera.get("camera_hash").and_then(Value::as_str)
            != render_set.get("camera_hash").and_then(Value::as_str)
    {
        return Err(RuntimeError::InvalidInput(
            "CAMERA_EVIDENCE_BINDING_MISMATCH".to_owned(),
        ));
    }

    let comparison_sha256 = evidence
        .comparison_report_object_sha256
        .clone()
        .ok_or_else(|| RuntimeError::InvalidInput("COMPARISON_REPORT_UNAVAILABLE".to_owned()))?;
    let comparison = read_json_object(runtime, &comparison_sha256)?;
    validate_reference_comparison_report(&comparison)?;
    if comparison.get("candidate_id").and_then(Value::as_str)
        != Some(candidate.candidate_id.as_str())
        || comparison.get("artifact_sha256").and_then(Value::as_str)
            != Some(geometry.artifact_sha256.as_str())
        || comparison.get("reference_id").and_then(Value::as_str)
            != Some(session.reference_id.as_str())
        || comparison.get("reference_sha256").and_then(Value::as_str)
            != Some(session.reference_sha256.as_str())
        || comparison.get("render_set_hash").and_then(Value::as_str)
            != Some(evidence.render_set_object_sha256.as_str())
        || comparison.get("camera_hash").and_then(Value::as_str)
            != Some(session.camera_hash.as_str())
    {
        return Err(RuntimeError::InvalidInput(
            "COMPARISON_REPORT_BINDING_MISMATCH".to_owned(),
        ));
    }

    let quality = read_json_object(runtime, &evidence.quality_report_object_sha256)?;
    validate_quality_report_v2_output(&quality)?;
    if quality.get("candidate_id").and_then(Value::as_str) != Some(candidate.candidate_id.as_str())
        || quality.get("artifact_sha256").and_then(Value::as_str)
            != Some(geometry.artifact_sha256.as_str())
        || quality.get("program_sha256").and_then(Value::as_str)
            != Some(geometry.evidence.geometry_program_sha256.as_str())
        || quality.get("reference_id").and_then(Value::as_str)
            != Some(session.reference_id.as_str())
        || quality.get("reference_sha256").and_then(Value::as_str)
            != Some(session.reference_sha256.as_str())
        || quality.get("render_set_hash").and_then(Value::as_str)
            != Some(evidence.render_set_object_sha256.as_str())
        || quality
            .get("comparison_report_hash")
            .and_then(Value::as_str)
            != Some(comparison_sha256.as_str())
    {
        return Err(RuntimeError::InvalidInput(
            "QUALITY_REPORT_BINDING_MISMATCH".to_owned(),
        ));
    }
    let quality_status = quality
        .get("visual_status")
        .and_then(Value::as_str)
        .ok_or_else(|| RuntimeError::InvalidInput("QUALITY_STATUS_UNAVAILABLE".to_owned()))?;
    if !matches!(
        quality_status,
        "PARTIAL_VISIBLE_VIEW_PASS" | "QUALITY_TARGET_NOT_MET" | "BLOCKED_REFERENCE_COVERAGE"
    ) {
        return Err(RuntimeError::InvalidInput(
            "QUALITY_STATUS_UNAVAILABLE".to_owned(),
        ));
    }
    if let Some(target_sha256) = evidence.target_sha256.as_deref() {
        if !is_sha256(target_sha256) {
            return Err(RuntimeError::InvalidInput(
                "SILHOUETTE_TARGET_BINDING_INVALID".to_owned(),
            ));
        }
        let target = runtime.read_silhouette_target(target_sha256)?;
        if target.get("reference_id").and_then(Value::as_str) != Some(session.reference_id.as_str())
        {
            return Err(RuntimeError::InvalidInput(
                "SILHOUETTE_TARGET_REFERENCE_MISMATCH".to_owned(),
            ));
        }
    }
    Ok(VisualBindings {
        render_set_sha256: evidence.render_set_object_sha256,
        quality_sha256: evidence.quality_report_object_sha256,
        quality_status: quality_status.to_owned(),
        metrics: comparison
            .get("metrics")
            .cloned()
            .unwrap_or_else(|| json!({})),
        camera,
        target_sha256: evidence.target_sha256,
    })
}

pub(crate) fn read_json_object(runtime: &Runtime, sha256: &str) -> Result<Value, RuntimeError> {
    if !is_sha256(sha256) {
        return Err(RuntimeError::InvalidInput(
            "CAS_OBJECT_HASH_INVALID".to_owned(),
        ));
    }
    let bytes = runtime.cas_read(sha256)?;
    let value: Value = serde_json::from_slice(&bytes)
        .map_err(|_| RuntimeError::InvalidInput("CAS_JSON_OBJECT_INVALID".to_owned()))?;
    if !value.is_object() || sha256_hex(&bytes) != sha256 {
        return Err(RuntimeError::InvalidInput(
            "CAS_JSON_OBJECT_BINDING_MISMATCH".to_owned(),
        ));
    }
    Ok(value)
}

fn validate_run_scope(
    run: &Value,
    run_id: &str,
    session_id: &str,
    project_id: &str,
    candidate_id: &str,
) -> Result<(), RuntimeError> {
    for (key, expected) in [
        ("run_id", run_id),
        ("session_id", session_id),
        ("project_id", project_id),
        ("candidate_id", candidate_id),
    ] {
        if run.get(key).and_then(Value::as_str) != Some(expected) {
            return Err(RuntimeError::InvalidInput(
                "DESIGN_ACTION_RUN_SCOPE_MISMATCH".to_owned(),
            ));
        }
    }
    Ok(())
}

fn validate_repair_apply_result(
    result: &Value,
    project_id: &str,
    session_id: &str,
    source_candidate_id: &str,
    proposal_candidate_id: &str,
    run_id: &str,
    input_sha256: &str,
) -> Result<(), RuntimeError> {
    if result.get("schema_version").and_then(Value::as_str) != Some("RepairApplyPrepareResult@1")
        || result.get("project_id").and_then(Value::as_str) != Some(project_id)
        || result.get("session_id").and_then(Value::as_str) != Some(session_id)
        || result.get("candidate_id").and_then(Value::as_str) != Some(source_candidate_id)
        || result.get("source_candidate_id").and_then(Value::as_str) != Some(source_candidate_id)
        || result.get("proposal_candidate_id").and_then(Value::as_str)
            != Some(proposal_candidate_id)
        || result.get("run_id").and_then(Value::as_str) != Some(run_id)
        || result.get("input_sha256").and_then(Value::as_str) != Some(input_sha256)
        || result.get("status").and_then(Value::as_str) != Some("ready")
        || result.get("confirm_allowed") != Some(&Value::Bool(false))
        || result.get("source_candidate_unchanged") != Some(&Value::Bool(true))
        || result.get("active_design_state_mutated") != Some(&Value::Bool(false))
        || result.get("persistent_user_data_touched") != Some(&Value::Bool(false))
    {
        return Err(RuntimeError::InvalidInput(
            "REPAIR_APPLY_READBACK_BINDING_MISMATCH: prepared result scope or gate drifted"
                .to_owned(),
        ));
    }
    let canonical = result
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .filter(|hash| is_sha256(hash))
        .ok_or_else(|| {
            RuntimeError::InvalidInput(
                "REPAIR_APPLY_READBACK_BINDING_MISMATCH: prepared result hash is invalid"
                    .to_owned(),
            )
        })?;
    let mut result_without_hash = result.clone();
    result_without_hash["canonical_sha256"] = Value::String(String::new());
    if canonical_json_hash(&result_without_hash) != canonical {
        return Err(RuntimeError::InvalidInput(
            "REPAIR_APPLY_READBACK_BINDING_MISMATCH: prepared result hash drifted".to_owned(),
        ));
    }
    Ok(())
}

fn validate_session_scope(
    session: &AgenticSessionRecord,
    session_id: &str,
    project_id: &str,
    candidate_id: &str,
) -> Result<(), RuntimeError> {
    if session.session_id != session_id
        || session.project_id != project_id
        || session.candidate_id != candidate_id
        || !is_sha256(&session.reference_sha256)
        || !is_sha256(&session.camera_hash)
        || !is_sha256(&session.evidence_sha256)
    {
        return Err(RuntimeError::InvalidInput(
            "AGENTIC_SESSION_BINDING_MISMATCH".to_owned(),
        ));
    }
    Ok(())
}

fn request_object<'a>(
    request: &'a Value,
    operation: &str,
) -> Result<&'a Map<String, Value>, RuntimeError> {
    request
        .as_object()
        .ok_or_else(|| RuntimeError::InvalidInput(format!("{operation} requires an object")))
}

fn reject_unknown_keys(object: &Map<String, Value>, allowed: &[&str]) -> Result<(), RuntimeError> {
    if object.keys().any(|key| !allowed.contains(&key.as_str())) {
        return Err(RuntimeError::InvalidInput(
            "DESIGN_ACTION_INVALID_INPUT: unsupported field".to_owned(),
        ));
    }
    Ok(())
}

fn required_id<'a>(object: &'a Map<String, Value>, key: &str) -> Result<&'a str, RuntimeError> {
    let value = object
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty());
    let value = value.ok_or_else(|| RuntimeError::InvalidInput(format!("{key} is required")))?;
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

fn value_to_optional_id(value: &Value) -> Result<Option<String>, RuntimeError> {
    if value.is_null() {
        return Ok(None);
    }
    let id = value.as_str().ok_or_else(|| {
        RuntimeError::InvalidInput(
            "REPAIR_APPLY_SCOPE_INVALID: nullable id must be null or a string".to_owned(),
        )
    })?;
    if !is_opaque_id(id) {
        return Err(RuntimeError::InvalidInput(
            "REPAIR_APPLY_SCOPE_INVALID: nullable id is malformed".to_owned(),
        ));
    }
    Ok(Some(id.to_owned()))
}

fn required_stage<'a>(object: &'a Map<String, Value>, key: &str) -> Result<&'a str, RuntimeError> {
    let value = required_id(object, key)?;
    if !DESIGN_STAGES.contains(&value) {
        return Err(RuntimeError::InvalidInput(format!("{key} is unsupported")));
    }
    Ok(value)
}

fn validate_approval(object: &Map<String, Value>) -> Result<(), RuntimeError> {
    if object.get("approved") != Some(&Value::Bool(true)) {
        return Err(RuntimeError::InvalidInput(
            "DESIGN_ACTION_APPROVAL_REQUIRED: approved=true is required".to_owned(),
        ));
    }
    for key in ["approval_receipt_id", "approval_summary", "idempotency_key"] {
        if object
            .get(key)
            .and_then(Value::as_str)
            .is_none_or(str::is_empty)
        {
            return Err(RuntimeError::InvalidInput(format!(
                "DESIGN_ACTION_APPROVAL_REQUIRED: {key} is required"
            )));
        }
    }
    let summary = object["approval_summary"].as_str().unwrap_or_default();
    if summary.len() > 512 || unsafe_text(summary) {
        return Err(RuntimeError::InvalidInput(
            "DESIGN_ACTION_APPROVAL_REQUIRED: approval summary is unsafe".to_owned(),
        ));
    }
    if let Some(value) = object.get("approval_expires_at") {
        let value = value.as_str().ok_or_else(|| {
            RuntimeError::InvalidInput(
                "DESIGN_ACTION_APPROVAL_REQUIRED: approval expiry is invalid".to_owned(),
            )
        })?;
        if value.len() > 64 || unsafe_text(value) {
            return Err(RuntimeError::InvalidInput(
                "DESIGN_ACTION_APPROVAL_REQUIRED: approval expiry is unsafe".to_owned(),
            ));
        }
    }
    if let Some(value) = object.get("approval_session_id") {
        let value = value.as_str().ok_or_else(|| {
            RuntimeError::InvalidInput(
                "DESIGN_ACTION_APPROVAL_REQUIRED: approval session is invalid".to_owned(),
            )
        })?;
        if !is_opaque_id(value)
            || value
                != object
                    .get("session_id")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
        {
            return Err(RuntimeError::InvalidInput(
                "DESIGN_ACTION_SCOPE_MISMATCH: approval session differs".to_owned(),
            ));
        }
    }
    Ok(())
}

fn validate_action(value: &Value) -> Result<(), RuntimeError> {
    let action = value.as_object().ok_or_else(|| {
        RuntimeError::InvalidInput(
            "DESIGN_ACTION_INVALID_INPUT: action must be an object".to_owned(),
        )
    })?;
    const FIELDS: [&str; 8] = [
        "action_id",
        "action_kind",
        "scope_kind",
        "target_id",
        "operator_id",
        "parameter_changes",
        "bounded",
        "description",
    ];
    if action.keys().any(|key| !FIELDS.contains(&key.as_str()))
        || FIELDS.iter().any(|key| !action.contains_key(*key))
    {
        return Err(RuntimeError::InvalidInput(
            "DESIGN_ACTION_INVALID_INPUT: action fields are incomplete".to_owned(),
        ));
    }
    let action_id = action["action_id"].as_str().unwrap_or_default();
    let action_kind = action["action_kind"].as_str().unwrap_or_default();
    let scope_kind = action["scope_kind"].as_str().unwrap_or_default();
    if !is_opaque_id(action_id)
        || !ACTION_KINDS.contains(&action_kind)
        || !matches!(
            scope_kind,
            "session" | "part" | "material-zone" | "reference"
        )
        || action["bounded"] != Value::Bool(true)
    {
        return Err(RuntimeError::InvalidInput(
            "DESIGN_ACTION_NOT_BOUNDED: action is outside the allowlist".to_owned(),
        ));
    }
    if scope_kind == "session" {
        if !action["target_id"].is_null() {
            return Err(RuntimeError::InvalidInput(
                "DESIGN_ACTION_SCOPE_MISMATCH: session action target must be null".to_owned(),
            ));
        }
    } else if action["target_id"]
        .as_str()
        .is_none_or(|value| !is_opaque_id(value))
    {
        return Err(RuntimeError::InvalidInput(
            "DESIGN_ACTION_SCOPE_MISMATCH: target id is invalid".to_owned(),
        ));
    }
    if action_kind == "request-reference" && scope_kind != "reference" {
        return Err(RuntimeError::InvalidInput(
            "DESIGN_ACTION_SCOPE_MISMATCH: request-reference must target a reference".to_owned(),
        ));
    }
    if !action["operator_id"].is_null()
        && !action["operator_id"]
            .as_str()
            .is_some_and(|value| OPERATOR_IDS.contains(&value))
    {
        return Err(RuntimeError::InvalidInput(
            "DESIGN_ACTION_NOT_BOUNDED: operator is not allowlisted".to_owned(),
        ));
    }
    let changes = action["parameter_changes"].as_array().ok_or_else(|| {
        RuntimeError::InvalidInput(
            "DESIGN_ACTION_INVALID_INPUT: parameter changes are invalid".to_owned(),
        )
    })?;
    if changes.len() > 8 {
        return Err(RuntimeError::InvalidInput(
            "DESIGN_ACTION_NOT_BOUNDED: too many parameter changes".to_owned(),
        ));
    }
    for change in changes {
        let change = change.as_object().ok_or_else(|| {
            RuntimeError::InvalidInput(
                "DESIGN_ACTION_INVALID_INPUT: parameter change is invalid".to_owned(),
            )
        })?;
        const CHANGE_FIELDS: [&str; 6] = [
            "parameter_id",
            "before",
            "after",
            "minimum",
            "maximum",
            "unit",
        ];
        if change.len() != CHANGE_FIELDS.len()
            || change
                .keys()
                .any(|key| !CHANGE_FIELDS.contains(&key.as_str()))
        {
            return Err(RuntimeError::InvalidInput(
                "DESIGN_ACTION_INVALID_INPUT: parameter change fields are invalid".to_owned(),
            ));
        }
        if change["parameter_id"]
            .as_str()
            .is_none_or(|value| !is_opaque_id(value))
            || !matches!(
                change["unit"].as_str(),
                Some("meter" | "radian" | "ratio" | "count")
            )
        {
            return Err(RuntimeError::InvalidInput(
                "DESIGN_ACTION_NOT_BOUNDED: parameter change identity is invalid".to_owned(),
            ));
        }
        let minimum = bounded_number(&change["minimum"])?;
        let maximum = bounded_number(&change["maximum"])?;
        let before = bounded_number(&change["before"])?;
        let after = bounded_number(&change["after"])?;
        if minimum > maximum
            || before < minimum
            || before > maximum
            || after < minimum
            || after > maximum
        {
            return Err(RuntimeError::InvalidInput(
                "DESIGN_ACTION_NOT_BOUNDED: parameter change exceeds bounds".to_owned(),
            ));
        }
    }
    let description = action["description"].as_str().ok_or_else(|| {
        RuntimeError::InvalidInput("DESIGN_ACTION_INVALID_INPUT: description is invalid".to_owned())
    })?;
    if description.len() > 512 || unsafe_text(description) {
        return Err(RuntimeError::InvalidInput(
            "DESIGN_ACTION_INVALID_INPUT: description is unsafe".to_owned(),
        ));
    }
    Ok(())
}

/// Keep the action runner honest while the broader Agentic Design Runtime is
/// still being built. `checkpoint` has a real implementation in this method;
/// `bounded-repair` and the geometry-only design stages below share the same
/// typed, single-Part proposal executor. Material/UV and orchestration-only
/// action kinds remain contract-level vocabulary and must not silently fall
/// through to the checkpoint/revalidation path as if they had executed.
fn validate_execution_payload(
    action: &Value,
    proposal: Option<&Value>,
    optimization_intent: Option<&Value>,
    view_spec: Option<&Value>,
) -> Result<(), RuntimeError> {
    let action_kind = action
        .get("action_kind")
        .and_then(Value::as_str)
        .unwrap_or_default();
    match (action_kind, proposal, optimization_intent) {
        ("request-reference", None, None) => Ok(()),
        ("request-reference", _, _) => Err(RuntimeError::InvalidInput(
            "DESIGN_ACTION_PROPOSAL_UNSUPPORTED: request-reference does not accept execution payloads"
                .to_owned(),
        )),
        ("checkpoint", None, None) => Ok(()),
        ("checkpoint", _, Some(_)) | ("checkpoint", Some(_), None) => {
            Err(RuntimeError::InvalidInput(
                "DESIGN_ACTION_PROPOSAL_UNSUPPORTED: checkpoint does not accept a repair proposal"
                    .to_owned(),
            ))
        }
        (
            "primary-form-adjustment" | "bounded-repair",
            None,
            None,
        ) if action
            .get("parameter_changes")
            .and_then(Value::as_array)
            .is_some_and(|changes| !changes.is_empty())
            && view_spec.is_none_or(Value::is_null) => Ok(()),
        (
            "bounded-repair"
            | "primary-blockout"
            | "primary-form-adjustment"
            | "secondary-structure"
            | "tertiary-detail",
            None,
            Some(_),
        ) => Ok(()),
        (
            "bounded-repair"
            | "primary-blockout"
            | "primary-form-adjustment"
            | "secondary-structure"
            | "tertiary-detail",
            None,
            None,
        ) if action
            .get("parameter_changes")
            .and_then(Value::as_array)
            .is_some_and(|changes| !changes.is_empty())
            && view_spec.is_some_and(Value::is_object) => Ok(()),
        (
            "bounded-repair"
            | "primary-blockout"
            | "primary-form-adjustment"
            | "secondary-structure"
            | "tertiary-detail",
            None,
            None,
        ) => Err(RuntimeError::InvalidInput(
            "DESIGN_ACTION_EXECUTION_PAYLOAD_REQUIRED: geometry design action requires a typed proposal or a bound view_spec for RuntimeParameterPatch"
                .to_owned(),
        )),
        (
            "bounded-repair"
            | "primary-blockout"
            | "primary-form-adjustment"
            | "secondary-structure"
            | "tertiary-detail",
            Some(proposal),
            None,
        ) if !proposal.is_object() => Err(RuntimeError::InvalidInput(
            "DESIGN_ACTION_EXECUTION_PAYLOAD_INVALID: proposal must be an object".to_owned(),
        )),
        (
            "bounded-repair"
            | "primary-blockout"
            | "primary-form-adjustment"
            | "secondary-structure"
            | "tertiary-detail",
            Some(_),
            None,
        ) => Ok(()),
        (
            "bounded-repair"
            | "primary-blockout"
            | "primary-form-adjustment"
            | "secondary-structure"
            | "tertiary-detail",
            Some(_),
            Some(_),
        ) => Err(RuntimeError::InvalidInput(
            "DESIGN_ACTION_EXECUTION_PAYLOAD_CONFLICT: proposal and optimization intent are mutually exclusive"
                .to_owned(),
        )),
        (kind, _, _) => Err(RuntimeError::InvalidInput(format!(
            "DESIGN_ACTION_EXECUTION_UNAVAILABLE: action kind {kind} has no Runtime executor"
        ))),
    }
}

fn rig_from_action(
    candidate_id: &str,
    input_sha256: &str,
    part_id: &str,
    action: &Value,
) -> Result<Value, RuntimeError> {
    let changes = action
        .get("parameter_changes")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            RuntimeError::InvalidInput("PRIMARY_FORM_ACTION_PARAMETER_CHANGES_REQUIRED".to_owned())
        })?;
    let mut parameters = Vec::with_capacity(changes.len());
    for change in changes {
        let object = change.as_object().ok_or_else(|| {
            RuntimeError::InvalidInput("PRIMARY_FORM_ACTION_PARAMETER_CHANGE_INVALID".to_owned())
        })?;
        let parameter_id = object
            .get("parameter_id")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                RuntimeError::InvalidInput("PRIMARY_FORM_ACTION_PARAMETER_ID_REQUIRED".to_owned())
            })?;
        if !is_opaque_id(parameter_id) {
            return Err(RuntimeError::InvalidInput(
                "PRIMARY_FORM_ACTION_PARAMETER_ID_INVALID".to_owned(),
            ));
        }
        let semantic = primary_form_parameter_semantic(parameter_id).ok_or_else(|| {
            RuntimeError::InvalidInput("PRIMARY_FORM_ACTION_PARAMETER_UNSUPPORTED".to_owned())
        })?;
        let unit = object.get("unit").and_then(Value::as_str).ok_or_else(|| {
            RuntimeError::InvalidInput("PRIMARY_FORM_ACTION_PARAMETER_UNIT_REQUIRED".to_owned())
        })?;
        let rotation_unit = matches!(semantic, "rotation_x" | "rotation_y" | "rotation_z");
        if !(if rotation_unit {
            matches!(unit, "radian" | "ratio")
        } else {
            matches!(unit, "meter" | "ratio")
        }) {
            return Err(RuntimeError::InvalidInput(
                "PRIMARY_FORM_ACTION_PARAMETER_UNIT_UNSUPPORTED".to_owned(),
            ));
        }
        let before = primary_form_finite_number(object, "before")?;
        let after = primary_form_finite_number(object, "after")?;
        let minimum = primary_form_finite_number(object, "minimum")?;
        let maximum = primary_form_finite_number(object, "maximum")?;
        if minimum >= maximum
            || before < minimum
            || before > maximum
            || after < minimum
            || after > maximum
        {
            return Err(RuntimeError::InvalidInput(
                "PRIMARY_FORM_ACTION_PARAMETER_OUT_OF_BOUNDS".to_owned(),
            ));
        }
        let span = maximum - minimum;
        let step = (after - before).abs().max((span / 20.0).max(0.0001));
        parameters.push(json!({
            "parameter_id":parameter_id,
            "part_id":part_id,
            "semantic":semantic,
            "value":after,
            "min":minimum,
            "max":maximum,
            "step":step,
            "unit":unit
        }));
    }
    let mut rig = json!({
        "schema_version":"SilhouetteRig@1",
        "rig_id":format!("action-rig-{}", &input_sha256[..24]),
        "candidate_id":candidate_id,
        "parameters":parameters,
        "canonical_sha256":""
    });
    rig["canonical_sha256"] = Value::String(canonical_json_hash(&rig));
    Ok(rig)
}

fn primary_form_parameter_semantic(parameter_id: &str) -> Option<&'static str> {
    [
        ("offset-x", "offset_x"),
        ("offset_x", "offset_x"),
        ("offset-y", "offset_y"),
        ("offset_y", "offset_y"),
        ("offset-z", "offset_z"),
        ("offset_z", "offset_z"),
        ("width", "width"),
        ("height", "height"),
        ("depth", "depth"),
        ("scale", "scale"),
        ("rotation-x", "rotation_x"),
        ("rotation_x", "rotation_x"),
        ("rotation-y", "rotation_y"),
        ("rotation_y", "rotation_y"),
        ("rotation-z", "rotation_z"),
        ("rotation_z", "rotation_z"),
    ]
    .into_iter()
    .find_map(|(suffix, semantic)| {
        (parameter_id == suffix
            || parameter_id.ends_with(&format!("-{suffix}"))
            || parameter_id.ends_with(&format!("_{suffix}")))
        .then_some(semantic)
    })
}

fn primary_form_finite_number(object: &Map<String, Value>, key: &str) -> Result<f64, RuntimeError> {
    let value = object.get(key).and_then(Value::as_f64).ok_or_else(|| {
        RuntimeError::InvalidInput(format!("PRIMARY_FORM_ACTION_PARAMETER_{key}_INVALID"))
    })?;
    if !value.is_finite() || !(-1000.0..=1000.0).contains(&value) {
        return Err(RuntimeError::InvalidInput(format!(
            "PRIMARY_FORM_ACTION_PARAMETER_{key}_OUT_OF_BOUNDS"
        )));
    }
    Ok(value)
}

/// Keep the outer ActionRun idempotency envelope stable across JSON clients
/// whose floating-point formatter chooses a different shortest decimal for
/// the same IEEE-754 value.  The typed GeometryProgram hash remains strict;
/// this compatibility digest only binds the request envelope at a bounded
/// twelve-decimal precision, which is below the Runtime's geometry/quality
/// evidence precision.
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

fn bounded_number(value: &Value) -> Result<f64, RuntimeError> {
    let value = value.as_f64().ok_or_else(|| {
        RuntimeError::InvalidInput(
            "DESIGN_ACTION_INVALID_INPUT: parameter must be numeric".to_owned(),
        )
    })?;
    if !value.is_finite() || !(-1000.0..=1000.0).contains(&value) {
        return Err(RuntimeError::InvalidInput(
            "DESIGN_ACTION_NOT_BOUNDED: parameter is outside the numeric bound".to_owned(),
        ));
    }
    Ok(value)
}

fn unsafe_text(value: &str) -> bool {
    let lowered = value.to_ascii_lowercase();
    value.starts_with('/')
        || value.starts_with('\\')
        || lowered.contains("://")
        || lowered.contains("api_key")
        || lowered.contains("secret")
        || lowered.contains("token")
        || lowered.contains("password")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn action(action_kind: &str) -> Value {
        json!({
            "action_id":"action-test",
            "action_kind":action_kind,
            "scope_kind":"session",
            "target_id":null,
            "operator_id":null,
            "parameter_changes":[],
            "bounded":true,
            "description":"bounded action test"
        })
    }

    #[test]
    fn execution_payload_is_fail_closed_for_unimplemented_action_kinds() {
        let error = validate_execution_payload(&action("material-zone"), None, None, None)
            .expect_err("unimplemented action kind must not fall through");
        assert!(error
            .to_string()
            .contains("DESIGN_ACTION_EXECUTION_UNAVAILABLE"));
    }

    #[test]
    fn request_reference_is_an_orchestration_boundary_without_payload() {
        let action = json!({
            "action_id":"request-reference-test",
            "action_kind":"request-reference",
            "scope_kind":"reference",
            "target_id":"reference-test",
            "operator_id":null,
            "parameter_changes":[],
            "bounded":true,
            "description":"Request the missing reference coverage"
        });
        assert!(validate_execution_payload(&action, None, None, None).is_ok());
        let error = validate_execution_payload(&action, Some(&json!({})), None, None)
            .expect_err("reference request must not execute a geometry payload");
        assert!(error
            .to_string()
            .contains("request-reference does not accept execution payloads"));
    }

    #[test]
    fn bounded_repair_requires_an_object_proposal() {
        let action = action("bounded-repair");
        let missing = validate_execution_payload(&action, None, None, None)
            .expect_err("bounded repair must require a proposal");
        assert!(missing
            .to_string()
            .contains("DESIGN_ACTION_EXECUTION_PAYLOAD_REQUIRED"));

        let invalid = validate_execution_payload(
            &action,
            Some(&Value::String("nope".to_owned())),
            None,
            None,
        )
        .expect_err("bounded repair proposal must be an object");
        assert!(invalid
            .to_string()
            .contains("DESIGN_ACTION_EXECUTION_PAYLOAD_INVALID"));
    }

    #[test]
    fn cross_view_evaluation_requires_every_supplied_kind_once() {
        let canvas = json!({
            "coverage": {
                "supplied_views": ["front", "back"]
            }
        });
        let evaluation = |kind: &str| ViewEvaluation {
            view_id: format!("view-{kind}"),
            kind: kind.to_owned(),
            visibility: "observed".to_owned(),
            confidence: 1.0,
            reference_id: "reference-1".to_owned(),
            reference_sha256: "a".repeat(64),
            target_sha256: None,
            view_spec: json!({}),
            camera: json!({}),
        };

        assert!(validate_cross_view_evaluation_coverage(
            &canvas,
            &[evaluation("front"), evaluation("back")]
        )
        .is_ok());
        let missing = validate_cross_view_evaluation_coverage(&canvas, &[evaluation("front")])
            .expect_err("missing supplied view unexpectedly passed");
        assert!(missing
            .to_string()
            .contains("REPAIR_VIEW_EVALUATIONS_COVERAGE_MISMATCH"));
        let duplicate_kind = validate_cross_view_evaluation_coverage(
            &canvas,
            &[evaluation("front"), evaluation("front")],
        )
        .expect_err("duplicate supplied kind unexpectedly passed");
        assert!(duplicate_kind
            .to_string()
            .contains("REPAIR_VIEW_EVALUATIONS_COVERAGE_MISMATCH"));
    }

    #[test]
    fn geometry_design_stages_require_the_same_typed_proposal() {
        for action_kind in [
            "primary-blockout",
            "primary-form-adjustment",
            "secondary-structure",
            "tertiary-detail",
        ] {
            let action = action(action_kind);
            let missing = validate_execution_payload(&action, None, None, None)
                .expect_err("geometry stage action must require a proposal");
            assert!(missing
                .to_string()
                .contains("DESIGN_ACTION_EXECUTION_PAYLOAD_REQUIRED"));
            assert!(validate_execution_payload(&action, Some(&json!({})), None, None).is_ok());
            assert!(validate_execution_payload(&action, None, Some(&json!({})), None).is_ok());
        }
    }

    #[test]
    fn geometry_design_action_can_request_runtime_parameter_patch_from_typed_changes() {
        let action = json!({
            "action_id":"runtime-parameter-patch-action",
            "action_kind":"primary-form-adjustment",
            "scope_kind":"part",
            "target_id":"shell",
            "operator_id":"forgecad.geometry.primitive@2",
            "parameter_changes":[{"parameter_id":"shell-width","before":1.0,"after":1.1,"minimum":0.8,"maximum":1.2,"unit":"meter"}],
            "bounded":true,
            "description":"Widen the shell within the approved local bound"
        });
        let view_spec = json!({"schema_version":"ReferenceViewSpec@1"});
        assert!(validate_execution_payload(&action, None, None, Some(&view_spec)).is_ok());
        assert_eq!(
            runtime_parameter_patch_strategy(&action).expect("primitive strategy"),
            "primitive-dimensions-v1"
        );

        let mut surface_action = action.clone();
        surface_action["parameter_changes"] = json!([{
            "parameter_id":"control-point-5-z",
            "before":0.18,
            "after":0.22,
            "minimum":0.0,
            "maximum":0.5,
            "unit":"meter"
        }]);
        assert_eq!(
            runtime_parameter_patch_strategy(&surface_action).expect("surface strategy"),
            "surface-control-points-v1"
        );

        let mut pose_action = action;
        pose_action["parameter_changes"] = json!([{
            "parameter_id":"head-rotation-y",
            "before":0.0,
            "after":0.18,
            "minimum":-0.6,
            "maximum":0.6,
            "unit":"radian"
        }]);
        assert!(validate_execution_payload(&pose_action, None, None, Some(&view_spec)).is_ok());
        assert_eq!(
            runtime_parameter_patch_strategy(&pose_action).expect("pose strategy"),
            "primitive-dimensions-v1"
        );

        let mut panel_action = json!({
            "action_id":"panel-finish-patch-action",
            "action_kind":"secondary-structure",
            "scope_kind":"part",
            "target_id":"chest-shell",
            "operator_id":"forgecad.geometry.panel@1",
            "parameter_changes":[
                {"parameter_id":"panel-thickness","before":0.18,"after":0.20,"minimum":0.10,"maximum":0.30,"unit":"meter"},
                {"parameter_id":"panel-bevel","before":0.12,"after":0.14,"minimum":0.0,"maximum":0.30,"unit":"meter"}
            ],
            "bounded":true,
            "description":"Adjust panel thickness and corner bevel within the source panel envelope"
        });
        assert_eq!(
            runtime_parameter_patch_strategy(&panel_action).expect("panel finish strategy"),
            "hard-surface-finish-v1"
        );
        assert!(runtime_parameter_relationship_valid(
            "forgecad.geometry.panel@1",
            &json!({"size_m":[1.66,1.12,0.68]})
                .as_object()
                .unwrap()
                .clone(),
            RuntimeParameterSemantic::Thickness,
            0.20
        ));
        assert!(runtime_parameter_relationship_valid(
            "forgecad.geometry.panel@1",
            &json!({"size_m":[1.66,1.12,0.68]})
                .as_object()
                .unwrap()
                .clone(),
            RuntimeParameterSemantic::Bevel,
            0.14
        ));
        let panel_node = json!({
            "operator_id":"forgecad.geometry.panel@1",
            "parameters":{"size_m":[1.66,1.12,0.68],"thickness_m":0.18,"bevel_m":0.12}
        });
        assert!(runtime_parameter_node_supports(
            panel_node.as_object().unwrap(),
            RuntimeParameterSemantic::Thickness
        ));
        assert!(runtime_parameter_node_supports(
            panel_node.as_object().unwrap(),
            RuntimeParameterSemantic::Bevel
        ));
        assert!(!runtime_parameter_node_supports(
            panel_node.as_object().unwrap(),
            RuntimeParameterSemantic::SurfaceControlPoint { index: 0, axis: 0 }
        ));
        assert!(!runtime_parameter_relationship_valid(
            "forgecad.geometry.panel@1",
            &json!({"size_m":[1.66,1.12,0.68]})
                .as_object()
                .unwrap()
                .clone(),
            RuntimeParameterSemantic::Bevel,
            0.60
        ));

        let energy_core_node = json!({
            "operator_id":"forgecad.geometry.energy-core@1",
            "parameters":{
                "shape":"energy-core",
                "component":"guard-ring",
                "outer_radius_m":0.48,
                "inner_radius_m":0.40,
                "depth_m":0.08,
                "radial_segments":32,
                "position_m":[0.0,0.0,0.0],
                "rotation_rad":[0.0,0.0,0.0]
            }
        });
        let energy_core_parameters = energy_core_node["parameters"]
            .as_object()
            .expect("energy-core parameters")
            .clone();
        assert_eq!(
            runtime_parameter_semantic("energy-core-outer-radius"),
            Some(RuntimeParameterSemantic::OuterRadius)
        );
        assert_eq!(
            runtime_parameter_semantic("energy-core-inner-radius"),
            Some(RuntimeParameterSemantic::InnerRadius)
        );
        assert!(runtime_parameter_node_supports(
            energy_core_node.as_object().unwrap(),
            RuntimeParameterSemantic::OuterRadius
        ));
        assert!(runtime_parameter_node_supports(
            energy_core_node.as_object().unwrap(),
            RuntimeParameterSemantic::InnerRadius
        ));
        assert!(runtime_parameter_node_supports(
            energy_core_node.as_object().unwrap(),
            RuntimeParameterSemantic::Size(2)
        ));
        assert!(runtime_parameter_relationship_valid(
            "forgecad.geometry.energy-core@1",
            &energy_core_parameters,
            RuntimeParameterSemantic::OuterRadius,
            0.50
        ));
        assert!(!runtime_parameter_relationship_valid(
            "forgecad.geometry.energy-core@1",
            &energy_core_parameters,
            RuntimeParameterSemantic::InnerRadius,
            0.0
        ));
        assert!(!runtime_parameter_relationship_valid(
            "forgecad.geometry.energy-core@1",
            &energy_core_parameters,
            RuntimeParameterSemantic::OuterRadius,
            0.39
        ));

        let solid_parameters = json!({
            "component":"emitter-core",
            "outer_radius_m":0.25,
            "inner_radius_m":0.0
        });
        assert!(!runtime_parameter_relationship_valid(
            "forgecad.geometry.energy-core@1",
            solid_parameters.as_object().unwrap(),
            RuntimeParameterSemantic::InnerRadius,
            0.000001
        ));
        panel_action["parameter_changes"] = json!([
            {"parameter_id":"panel-bevel","before":0.12,"after":0.14,"minimum":0.0,"maximum":0.30,"unit":"meter"},
            {"parameter_id":"panel-width","before":1.66,"after":1.70,"minimum":1.4,"maximum":1.9,"unit":"meter"}
        ]);
        let mixed = runtime_parameter_patch_strategy(&panel_action)
            .expect_err("finish and primitive patch families must not mix");
        assert!(mixed
            .to_string()
            .contains("ACTION_PARAMETER_PATCH_STRATEGY_PARAMETER_MISMATCH"));
    }

    #[test]
    fn checkpoint_rejects_a_repair_proposal() {
        let error = validate_execution_payload(&action("checkpoint"), Some(&json!({})), None, None)
            .expect_err("checkpoint cannot accept a repair proposal");
        assert!(error
            .to_string()
            .contains("DESIGN_ACTION_PROPOSAL_UNSUPPORTED"));
    }

    #[test]
    fn repair_apply_readback_is_source_bound_and_never_confirmable() {
        let mut result = json!({
            "schema_version":"RepairApplyPrepareResult@1",
            "job_id":"repair-job-1",
            "project_id":"project-1",
            "session_id":"session-1",
            "candidate_id":"candidate-source",
            "source_candidate_id":"candidate-source",
            "proposal_candidate_id":"candidate-proposal",
            "run_id":"run-1",
            "input_sha256":"a".repeat(64),
            "intent_sha256":"b".repeat(64),
            "intent_object_sha256":"c".repeat(64),
            "apply_intent_object_sha256":"d".repeat(64),
            "apply_intent_canonical_sha256":"e".repeat(64),
            "base_version_id":null,
            "prepared_object_id":"artifact-1",
            "prepared_object_sha256":"f".repeat(64),
            "quality_report_id":"quality-1",
            "cross_view_evidence_sha256":null,
            "status":"ready",
            "next_transaction":"candidate_confirm",
            "confirm_allowed":false,
            "source_candidate_unchanged":true,
            "active_design_state_mutated":false,
            "persistent_user_data_touched":false,
            "canonical_sha256":""
        });
        result["canonical_sha256"] = Value::String(String::new());
        result["canonical_sha256"] = Value::String(canonical_json_hash(&result));
        assert!(validate_repair_apply_result(
            &result,
            "project-1",
            "session-1",
            "candidate-source",
            "candidate-proposal",
            "run-1",
            "a".repeat(64).as_str()
        )
        .is_ok());

        let mut confirmable = result.clone();
        confirmable["confirm_allowed"] = Value::Bool(true);
        confirmable["canonical_sha256"] = Value::String(String::new());
        confirmable["canonical_sha256"] = Value::String(canonical_json_hash(&confirmable));
        let error = validate_repair_apply_result(
            &confirmable,
            "project-1",
            "session-1",
            "candidate-source",
            "candidate-proposal",
            "run-1",
            "a".repeat(64).as_str(),
        )
        .expect_err("a prepared apply intent must not authorize confirmation");
        assert!(error.to_string().contains("READBACK_BINDING_MISMATCH"));

        let mut stale = result;
        stale["input_sha256"] = Value::String("9".repeat(64));
        let error = validate_repair_apply_result(
            &stale,
            "project-1",
            "session-1",
            "candidate-source",
            "candidate-proposal",
            "run-1",
            "a".repeat(64).as_str(),
        )
        .expect_err("a replay with another input hash must fail closed");
        assert!(error.to_string().contains("READBACK_BINDING_MISMATCH"));
    }
}
