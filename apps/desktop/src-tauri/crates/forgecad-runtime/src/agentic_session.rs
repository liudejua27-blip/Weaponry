//! Durable Agentic Design Runtime session and checkpoint operations.
//!
//! This module is deliberately narrower than the observation projection.  It
//! materializes only Runtime-owned, hash-bound metadata and delegates every
//! SQLite/CAS mutation to the Store.  It never confirms a candidate, creates a
//! version, or treats a failed visual gate as a pass.

use super::{canonical_json_hash, normalize_json_numbers, sha256_hex, Runtime, RuntimeError};
use forgecad_contracts::{is_opaque_id, is_sha256, CandidateRecord, ReferenceEvidenceRecord};
use forgecad_store::{AgenticCheckpointRecord, AgenticSessionRecord};
use serde_json::{json, Map, Value};
use std::collections::HashSet;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

const STAGES: [&str; 6] = [
    "reference-canvas",
    "primary-form",
    "secondary-structure",
    "tertiary-detail",
    "uv-pbr",
    "final-review",
];

/// A complete high-quality reference set must contain the five identity
/// views. Perspective, top, material and detail views are useful supplements,
/// but none can replace a canonical front/back/left/right or rear-three-quarter
/// view for an HQ_360 claim.
const REQUIRED_HQ_REFERENCE_VIEWS: [&str; 5] =
    ["front", "back", "left", "right", "rear-three-quarter"];

impl Runtime {
    /// Create or resume one durable DesignSession. A new session is bound to
    /// a Runtime-owned visual evidence hash from the current projection;
    /// callers cannot inject an unverified evidence claim into durable state.
    pub fn session_create_or_resume(&self, request: Value) -> Result<Value, RuntimeError> {
        let object = request_object(&request, "session_create_or_resume")?;
        reject_unknown_keys(
            object,
            &[
                "session_id",
                "project_id",
                "candidate_id",
                "idempotency_key",
                "reference_id",
                "design_spec_id",
                "reference_canvas_id",
                "camera_hash",
                "evidence_sha256",
                "approved",
                "approval_receipt_id",
                "approval_summary",
                "approval_expires_at",
                "authoring_context",
            ],
        )?;
        require_approval(object)?;
        let project_id = required_id(object, "project_id")?;
        let candidate_id = required_id(object, "candidate_id")?;
        let reference_id = required_id(object, "reference_id")?;
        let design_spec_id = required_id(object, "design_spec_id")?;
        let reference_canvas_id = required_id(object, "reference_canvas_id")?;
        let camera_hash = required_sha(object, "camera_hash")?;
        let evidence_sha256 = required_sha(object, "evidence_sha256")?;

        let candidate = bound_candidate(self, project_id, candidate_id)?;
        let reference = bound_reference(self, project_id, reference_id)?;
        let observation = self.agentic_scene_observe(project_id, Some(candidate_id))?;
        let observation_sha256 = observation_hash(&observation)?;
        validate_observation_claims(
            &observation,
            &candidate,
            &reference,
            camera_hash,
            evidence_sha256,
        )?;

        let session_id = object
            .get("session_id")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
            .unwrap_or_else(|| format!("design-session-{}", Uuid::new_v4().simple()));
        if !is_opaque_id(&session_id) {
            return Err(RuntimeError::InvalidInput(
                "AGENTIC_INVALID_INPUT: session_id is malformed".to_owned(),
            ));
        }

        if let Some(existing) = self.store.get_agentic_session(&session_id)? {
            validate_session_binding(
                &existing,
                &session_id,
                project_id,
                candidate_id,
                reference_id,
                camera_hash,
                evidence_sha256,
                &observation_sha256,
            )?;
            if existing.design_spec_id != design_spec_id
                || existing.reference_canvas_id != reference_canvas_id
            {
                return Err(RuntimeError::InvalidInput(
                    "AGENTIC_SESSION_BINDING_MISMATCH: durable DesignSpec/ReferenceCanvas IDs differ"
                        .to_owned(),
                ));
            }
            if object.contains_key("authoring_context") {
                return Err(RuntimeError::InvalidInput(
                    "AGENTIC_AUTHORING_CONTEXT_IMMUTABLE: authoring_context is accepted only when creating a new session"
                        .to_owned(),
                ));
            }
            return session_result_with_authoring(self, &existing, "resumed");
        }

        let explicit_authoring = object.get("authoring_context");
        let canvas = match explicit_authoring {
            Some(authoring) => build_reference_canvas_from_authoring(
                self,
                authoring,
                project_id,
                reference_canvas_id,
                &reference,
                camera_hash,
                evidence_sha256,
            )?,
            None => canonical_value(build_reference_canvas(&reference, reference_canvas_id)),
        };
        let canvas_bytes = canonical_json_bytes(&canvas)?;
        let canvas_object = self.put_object(
            &canvas_bytes,
            None,
            "application/json",
            "agentic-reference-canvas",
        )?;

        let spec = match explicit_authoring {
            Some(authoring) => build_design_spec_from_authoring(
                authoring,
                project_id,
                design_spec_id,
                reference_canvas_id,
                &canvas_object.record.sha256,
                &reference,
            )?,
            None => canonical_value(build_design_spec(
                project_id,
                design_spec_id,
                reference_canvas_id,
                &canvas_object.record.sha256,
                &reference,
            )),
        };
        let spec_bytes = canonical_json_bytes(&spec)?;
        let spec_object =
            self.put_object(&spec_bytes, None, "application/json", "agentic-design-spec")?;

        let session = session_from_observation(
            &observation,
            &candidate,
            &reference,
            &session_id,
            design_spec_id,
            &spec_object.record.sha256,
            reference_canvas_id,
            &canvas_object.record.sha256,
            camera_hash,
            evidence_sha256,
            &observation_sha256,
        )?;
        let mut session = with_session_canonical(session)?;
        let session_bytes = forgecad_store::agentic_session_payload_bytes(&session)?;
        let session_object = self.put_object(
            &session_bytes,
            None,
            "application/json",
            "agentic-design-session",
        )?;
        session.object_sha256 = Some(session_object.record.sha256.clone());
        let stored = self
            .store
            .agentic_session_create_or_resume(&session, &session_object.record)?;
        // Creating the durable authoring documents changes the read-only
        // ReferenceCanvas projection from its pre-session conservative view
        // to the exact CAS-bound document. Rebind the session to that stable
        // post-create observation so a subsequent scene_observe_get returns
        // the same one-shot hash instead of a false stale-session error.
        let stable_observation = self.agentic_scene_observe(project_id, Some(candidate_id))?;
        let stable_observation_sha256 = observation_hash(&stable_observation)?;
        if stable_observation_sha256 == stored.observation_sha256 {
            return session_result_with_authoring(self, &stored, "created");
        }
        let mut rebound = stored;
        rebound.observation_sha256 = stable_observation_sha256;
        rebound.lineage = stable_observation
            .get("lineage")
            .cloned()
            .unwrap_or(rebound.lineage);
        rebound.updated_at = agentic_timestamp();
        rebound.object_sha256 = Some("0".repeat(64));
        rebound.canonical_sha256 = "0".repeat(64);
        rebound = with_session_canonical(rebound)?;
        let rebound_bytes = forgecad_store::agentic_session_payload_bytes(&rebound)?;
        let rebound_object = self.put_object(
            &rebound_bytes,
            None,
            "application/json",
            "agentic-design-session",
        )?;
        rebound.object_sha256 = Some(rebound_object.record.sha256.clone());
        let rebound_stored = self
            .store
            .agentic_session_create_or_resume(&rebound, &rebound_object.record)?;
        session_result_with_authoring(self, &rebound_stored, "created")
    }

    pub fn session_get(&self, request: Value) -> Result<Value, RuntimeError> {
        let object = request_object(&request, "session_get")?;
        reject_unknown_keys(object, &["session_id", "project_id", "candidate_id"])?;
        let session_id = required_id(object, "session_id")?;
        let project_id = required_id(object, "project_id")?;
        let candidate_id = required_id(object, "candidate_id")?;
        let session = self
            .store
            .get_agentic_session(session_id)?
            .ok_or_else(|| RuntimeError::InvalidInput("NOT_FOUND: session not found".to_owned()))?;
        let candidate = bound_candidate(self, project_id, candidate_id)?;
        let reference = bound_reference(self, project_id, &session.reference_id)?;
        let observation = self.agentic_scene_observe(project_id, Some(candidate_id))?;
        let observation_sha256 = observation_hash(&observation)?;
        validate_observation_claims(
            &observation,
            &candidate,
            &reference,
            &session.camera_hash,
            &session.evidence_sha256,
        )?;
        validate_session_binding(
            &session,
            session_id,
            project_id,
            candidate_id,
            &session.reference_id,
            &session.camera_hash,
            &session.evidence_sha256,
            &observation_sha256,
        )?;
        session_result_with_authoring(self, &session, "read")
    }

    /// Prepare and persist one immutable checkpoint.  The checkpoint is
    /// allowed to record a failed visual state, but a claimed pass is checked
    /// against the Runtime's current strict visual gate first.
    pub fn checkpoint_prepare(&self, request: Value) -> Result<Value, RuntimeError> {
        let object = request_object(&request, "checkpoint_prepare")?;
        reject_unknown_keys(
            object,
            &[
                "session_id",
                "project_id",
                "candidate_id",
                "visual_state",
                "evidence_sha256",
                "stage",
                "checkpoint_type",
                "candidate_state_sha256",
                "artifact_sha256",
                "reference_id",
                "reference_sha256",
                "camera_hash",
                "idempotency_key",
                "approved",
                "approval_receipt_id",
                "approval_summary",
                "approval_expires_at",
            ],
        )?;
        require_approval(object)?;
        let session_id = required_id(object, "session_id")?;
        let project_id = required_id(object, "project_id")?;
        let candidate_id = required_id(object, "candidate_id")?;
        let visual_state = object
            .get("visual_state")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                RuntimeError::InvalidInput(
                    "AGENTIC_VISUAL_STATE_REQUIRED: visual_state is required".to_owned(),
                )
            })?;
        if !matches!(visual_state, "pass" | "fail") {
            return Err(RuntimeError::InvalidInput(
                "AGENTIC_VISUAL_STATE_UNKNOWN: unknown visual state cannot be checkpointed"
                    .to_owned(),
            ));
        }
        let session = self
            .store
            .get_agentic_session(session_id)?
            .ok_or_else(|| RuntimeError::InvalidInput("AGENTIC_SESSION_NOT_FOUND".to_owned()))?;
        validate_session_binding(
            &session,
            session_id,
            project_id,
            candidate_id,
            &session.reference_id,
            &session.camera_hash,
            &session.evidence_sha256,
            &session.observation_sha256,
        )?;
        let candidate = bound_candidate(self, project_id, candidate_id)?;
        let reference = bound_reference(self, project_id, &session.reference_id)?;
        let observation = self.agentic_scene_observe(project_id, Some(candidate_id))?;
        let observation_sha256 = observation_hash(&observation)?;
        if observation_sha256 != session.observation_sha256 {
            return Err(RuntimeError::InvalidInput(
                "AGENTIC_SESSION_STALE: current observation differs from the session".to_owned(),
            ));
        }
        let requested_evidence = required_sha(object, "evidence_sha256")?;
        validate_observation_claims(
            &observation,
            &candidate,
            &reference,
            required_sha(object, "camera_hash")?,
            requested_evidence,
        )?;
        if requested_evidence != session.evidence_sha256 {
            return Err(RuntimeError::InvalidInput(
                "AGENTIC_SESSION_STALE: checkpoint evidence differs from the session".to_owned(),
            ));
        }
        let stage = required_stage(object, "stage")?;
        let current_stage = observation
            .pointer("/design_stage_plan/current_stage")
            .and_then(Value::as_str)
            .unwrap_or("reference-canvas");
        if stage != current_stage {
            return Err(RuntimeError::InvalidInput(
                "AGENTIC_STAGE_MISMATCH: checkpoint stage is not the current Runtime stage"
                    .to_owned(),
            ));
        }
        let checkpoint_type = object
            .get("checkpoint_type")
            .and_then(Value::as_str)
            .ok_or_else(|| RuntimeError::InvalidInput("checkpoint_type is required".to_owned()))?;
        if !matches!(
            checkpoint_type,
            "stage-entry"
                | "stage-pass"
                | "stage-fail"
                | "manual-save"
                | "rollback-source"
                | "rollback-result"
        ) {
            return Err(RuntimeError::InvalidInput(
                "AGENTIC_INVALID_INPUT: checkpoint_type is unsupported".to_owned(),
            ));
        }
        let candidate_state_sha256 = required_sha(object, "candidate_state_sha256")?;
        if candidate_state_sha256 != candidate.canonical_sha256 {
            return Err(RuntimeError::InvalidInput(
                "AGENTIC_CANDIDATE_STATE_MISMATCH: candidate state hash differs".to_owned(),
            ));
        }
        let artifact_sha256 = required_sha(object, "artifact_sha256")?;
        let candidate_artifact = candidate
            .prepared_object_sha256
            .as_deref()
            .or(candidate.manifest_hash.as_deref());
        if candidate_artifact != Some(artifact_sha256)
            || self.store.get_object(artifact_sha256)?.is_none()
        {
            return Err(RuntimeError::InvalidInput(
                "AGENTIC_ARTIFACT_BINDING_MISMATCH: artifact is not the candidate artifact"
                    .to_owned(),
            ));
        }
        let reference_sha256 = required_sha(object, "reference_sha256")?;
        if reference_sha256 != reference.object_sha256 {
            return Err(RuntimeError::InvalidInput(
                "AGENTIC_REFERENCE_BINDING_MISMATCH: reference hash differs".to_owned(),
            ));
        }

        let checkpoint_digest = canonical_json_hash(&json!({
            "session_id":session_id,
            "idempotency_key":required_id(object, "idempotency_key")?,
            "candidate_state_sha256":candidate_state_sha256,
            "evidence_sha256":requested_evidence
        }));
        let checkpoint_id = format!("checkpoint-{}", &checkpoint_digest[..32]);
        if let Some(existing) = self.store.get_agentic_checkpoint(&checkpoint_id)? {
            if existing.session_id != session_id
                || existing.project_id != project_id
                || existing.candidate_id != candidate_id
            {
                return Err(RuntimeError::InvalidInput(
                    "AGENTIC_CHECKPOINT_BINDING_MISMATCH: idempotency key is already bound"
                        .to_owned(),
                ));
            }
            return Ok(checkpoint_result(&existing, &session, "replayed"));
        }

        let stage_gate = checkpoint_stage_gate(&observation, visual_state, requested_evidence);
        if visual_state == "pass"
            && observation
                .pointer("/design_stage_plan/strict_visual_gate/status")
                .and_then(Value::as_str)
                != Some("passed")
        {
            return Err(RuntimeError::InvalidInput(
                "QUALITY_HARD_GATE_FAILED: visual_state=pass is not supported by the strict Runtime gate"
                    .to_owned(),
            ));
        }
        let checkpoint = AgenticCheckpointRecord {
            schema_version: "DesignCheckpoint@1".to_owned(),
            checkpoint_id,
            session_id: session_id.to_owned(),
            project_id: project_id.to_owned(),
            candidate_id: candidate_id.to_owned(),
            revision: session.revision,
            stage: stage.to_owned(),
            checkpoint_type: checkpoint_type.to_owned(),
            candidate_state_sha256: candidate_state_sha256.to_owned(),
            artifact_sha256: artifact_sha256.to_owned(),
            reference_id: session.reference_id.clone(),
            reference_sha256: reference_sha256.to_owned(),
            camera_hash: required_sha(object, "camera_hash")?.to_owned(),
            input_sha256: canonical_json_hash(&json!({
                "stage":stage,
                "checkpoint_type":checkpoint_type,
                "visual_state":visual_state,
                "candidate_state_sha256":candidate_state_sha256,
                "artifact_sha256":artifact_sha256,
                "reference_id":session.reference_id,
                "reference_sha256":reference_sha256,
                "camera_hash":required_sha(object, "camera_hash")?,
                "evidence_sha256":requested_evidence,
                "observation_sha256":observation_sha256,
            })),
            evidence_sha256: requested_evidence.to_owned(),
            observation_sha256: observation_sha256.clone(),
            version_id: session.current_version_id.clone(),
            version_sha256: session.current_version_sha256.clone(),
            parent_checkpoint_id: session.current_checkpoint_id.clone(),
            parent_checkpoint_sha256: session.current_checkpoint_sha256.clone(),
            stage_gate: stage_gate.clone(),
            rollback: no_checkpoint_rollback(),
            observed: observation_strings(&observation, "observed"),
            inferred: observation_strings(&observation, "inferred"),
            unknown: observation_strings(&observation, "unknown"),
            failed_gates: stage_gate
                .get("failed_checks")
                .and_then(Value::as_array)
                .map(|values| {
                    values
                        .iter()
                        .filter_map(Value::as_str)
                        .map(str::to_owned)
                        .collect()
                })
                .unwrap_or_default(),
            allowed_actions: checkpoint_actions(&stage_gate, true),
            locked_actions: checkpoint_actions(&stage_gate, false),
            immutable: true,
            runtime_write: false,
            object_sha256: Some("0".repeat(64)),
            canonical_sha256: "0".repeat(64),
            created_at: agentic_timestamp(),
        };
        let mut checkpoint = with_checkpoint_canonical(checkpoint)?;
        let mut next_session = session.clone();
        next_session.current_checkpoint_id = Some(checkpoint.checkpoint_id.clone());
        next_session.current_checkpoint_sha256 = Some(checkpoint.canonical_sha256.clone());
        next_session
            .checkpoint_ids
            .push(checkpoint.checkpoint_id.clone());
        next_session.quality_status = if visual_state == "pass" {
            "PARTIAL_VISIBLE_VIEW_PASS".to_owned()
        } else {
            "QUALITY_TARGET_NOT_MET".to_owned()
        };
        next_session.status = "awaiting-review".to_owned();
        next_session.stage_gate = stage_gate;
        next_session.next_actions = next_actions(
            stage,
            &next_session.quality_status,
            &next_session.session_id,
            &next_session.reference_id,
        );
        next_session.updated_at = agentic_timestamp();
        next_session.object_sha256 = Some("0".repeat(64));
        next_session.canonical_sha256 = "0".repeat(64);
        next_session = with_session_canonical(next_session)?;

        let checkpoint_bytes = forgecad_store::agentic_checkpoint_payload_bytes(&checkpoint)?;
        let checkpoint_object = self.put_object(
            &checkpoint_bytes,
            None,
            "application/json",
            "agentic-design-checkpoint",
        )?;
        checkpoint.object_sha256 = Some(checkpoint_object.record.sha256.clone());
        let session_bytes = forgecad_store::agentic_session_payload_bytes(&next_session)?;
        let session_object = self.put_object(
            &session_bytes,
            None,
            "application/json",
            "agentic-design-session",
        )?;
        next_session.object_sha256 = Some(session_object.record.sha256.clone());
        self.store.agentic_checkpoint_prepare(
            &checkpoint,
            &next_session,
            &checkpoint_object.record,
            &session_object.record,
        )?;
        let stored_session = self
            .store
            .get_agentic_session(session_id)?
            .ok_or_else(|| RuntimeError::InvalidInput("AGENTIC_SESSION_NOT_FOUND".to_owned()))?;
        let stored_checkpoint = self
            .store
            .get_agentic_checkpoint(&checkpoint.checkpoint_id)?
            .ok_or_else(|| RuntimeError::InvalidInput("AGENTIC_CHECKPOINT_NOT_FOUND".to_owned()))?;
        Ok(checkpoint_result(
            &stored_checkpoint,
            &stored_session,
            "prepared",
        ))
    }

    pub fn checkpoint_get(&self, request: Value) -> Result<Value, RuntimeError> {
        let object = request_object(&request, "checkpoint_get")?;
        reject_unknown_keys(
            object,
            &["checkpoint_id", "session_id", "project_id", "candidate_id"],
        )?;
        let checkpoint_id = required_id(object, "checkpoint_id")?;
        let session_id = required_id(object, "session_id")?;
        let project_id = required_id(object, "project_id")?;
        let candidate_id = required_id(object, "candidate_id")?;
        let checkpoint = self
            .store
            .get_agentic_checkpoint(checkpoint_id)?
            .ok_or_else(|| {
                RuntimeError::InvalidInput("NOT_FOUND: checkpoint not found".to_owned())
            })?;
        if checkpoint.session_id != session_id
            || checkpoint.project_id != project_id
            || checkpoint.candidate_id != candidate_id
        {
            return Err(RuntimeError::InvalidInput(
                "AGENTIC_CHECKPOINT_BINDING_MISMATCH: checkpoint scope differs".to_owned(),
            ));
        }
        let session = self
            .store
            .get_agentic_session(session_id)?
            .ok_or_else(|| RuntimeError::InvalidInput("AGENTIC_SESSION_NOT_FOUND".to_owned()))?;
        validate_session_binding(
            &session,
            session_id,
            project_id,
            candidate_id,
            &session.reference_id,
            &session.camera_hash,
            &session.evidence_sha256,
            &session.observation_sha256,
        )?;
        Ok(checkpoint_result(&checkpoint, &session, "read"))
    }

    /// Create a CAS-persisted, approval-gated RepairIntent.  It never mutates
    /// a candidate, version, snapshot, or checkpoint history.
    pub fn checkpoint_restore_prepare(&self, request: Value) -> Result<Value, RuntimeError> {
        let object = request_object(&request, "checkpoint_restore_prepare")?;
        reject_unknown_keys(
            object,
            &[
                "checkpoint_id",
                "checkpoint_sha256",
                "session_id",
                "project_id",
                "candidate_id",
                "visual_state",
                "idempotency_key",
                "approved",
                "approval_receipt_id",
                "approval_summary",
                "approval_expires_at",
            ],
        )?;
        require_approval(object)?;
        let session_id = required_id(object, "session_id")?;
        let project_id = required_id(object, "project_id")?;
        let candidate_id = required_id(object, "candidate_id")?;
        let checkpoint_id = required_id(object, "checkpoint_id")?;
        let checkpoint_sha256 = required_sha(object, "checkpoint_sha256")?;
        let visual_state = object
            .get("visual_state")
            .and_then(Value::as_str)
            .ok_or_else(|| RuntimeError::InvalidInput("visual_state is required".to_owned()))?;
        if !matches!(visual_state, "pass" | "fail") {
            return Err(RuntimeError::InvalidInput(
                "AGENTIC_VISUAL_STATE_UNKNOWN: restore requires known visual state".to_owned(),
            ));
        }
        let session = self
            .store
            .get_agentic_session(session_id)?
            .ok_or_else(|| RuntimeError::InvalidInput("AGENTIC_SESSION_NOT_FOUND".to_owned()))?;
        validate_session_binding(
            &session,
            session_id,
            project_id,
            candidate_id,
            &session.reference_id,
            &session.camera_hash,
            &session.evidence_sha256,
            &session.observation_sha256,
        )?;
        let checkpoint = self
            .store
            .get_agentic_checkpoint(checkpoint_id)?
            .ok_or_else(|| RuntimeError::InvalidInput("AGENTIC_CHECKPOINT_NOT_FOUND".to_owned()))?;
        if checkpoint.canonical_sha256 != checkpoint_sha256
            || checkpoint.session_id != session_id
            || checkpoint.project_id != project_id
            || checkpoint.candidate_id != candidate_id
            || checkpoint.observation_sha256 != session.observation_sha256
        {
            return Err(RuntimeError::InvalidInput(
                "AGENTIC_CHECKPOINT_BINDING_MISMATCH: restore source differs".to_owned(),
            ));
        }
        let intent_id = format!(
            "repair-intent-{}",
            &canonical_json_hash(&json!({
                "session_id":session_id,
                "checkpoint_sha256":checkpoint_sha256,
                "idempotency_key":required_id(object, "idempotency_key")?
            }))[..32]
        );
        let mut intent = json!({
            "schema_version":"RepairIntent@1",
            "intent_id":intent_id,
            "session_id":session_id,
            "project_id":project_id,
            "candidate_id":candidate_id,
            "candidate_state_sha256":checkpoint.candidate_state_sha256,
            "reference_id":checkpoint.reference_id,
            "reference_sha256":checkpoint.reference_sha256,
            "camera_hash":checkpoint.camera_hash,
            "observation_sha256":checkpoint.observation_sha256,
            "source_evidence_sha256":checkpoint.evidence_sha256,
            "source_critic_report_id":format!("checkpoint-critic-{}",checkpoint.checkpoint_id),
            "source_critic_report_sha256":checkpoint.canonical_sha256,
            "stage":checkpoint.stage,
            "scope":{"kind":"part","part_id":"scene"},
            "action":{
                "action_kind":"bounded-repair",
                "kit_id":"forgecad.kit.housing@1",
                "operator_id":"forgecad.geometry.primitive@2",
                "operation":"rebuild-part",
                "parameter_changes":[{"parameter_id":"restore-source","before":0.0,"after":1.0,"minimum":0.0,"maximum":1.0,"unit":"count"}],
                "bounded":true,
                "description":"Prepare a new candidate from the selected checkpoint; no history is rewritten"
            },
            "precondition":{
                "failed_gate_id":"visible-view",
                "quality_status":if visual_state == "pass" {"PARTIAL_VISIBLE_VIEW_PASS"} else {"QUALITY_TARGET_NOT_MET"},
                "current_candidate_state_sha256":checkpoint.candidate_state_sha256,
                "evidence_sha256":checkpoint.evidence_sha256,
                "status":if visual_state == "pass" {"unknown"} else {"failed"}
            },
            "recompute":{"steps":["compile","readback","render","compare"],"must_rebind_reference":true,"must_rebind_camera":true,"confirm_allowed":false},
            "rollback":{"relation":"restore-checkpoint","target_checkpoint_id":checkpoint.checkpoint_id,"target_checkpoint_sha256":checkpoint.canonical_sha256,"target_version_id":Value::Null,"target_version_sha256":Value::Null,"on_failure":"request-user","reason":"Restore is a new candidate intent and never rewrites historical state"},
            "status":"proposed",
            "approval_required":true,
            "runtime_write":false,
            "canonical_sha256":""
        });
        intent["canonical_sha256"] = Value::String(canonical_json_hash(&intent));
        let intent_bytes = canonical_json_bytes(&intent)?;
        let intent_object = self.put_object(
            &intent_bytes,
            None,
            "application/json",
            "agentic-repair-intent",
        )?;
        Ok(json!({
            "schema_version":"CheckpointRestorePrepareResult@1",
            "status":"prepared",
            "durable":false,
            "read_only":false,
            "runtime_confirm_allowed":false,
            "session_id":session_id,
            "project_id":project_id,
            "candidate_id":candidate_id,
            "checkpoint":checkpoint_contract_value(&checkpoint),
            "intent":intent,
            "intent_object_sha256":intent_object.record.sha256,
            "reason":"RepairIntent is CAS-persisted; applying it requires a separate bounded candidate prepare and user approval"
        }))
    }

    /// Viewer-only lookup by project/candidate.  It returns the durable session
    /// when one exists and never creates a session as a side effect.
    pub fn agentic_session_lookup(
        &self,
        project_id: &str,
        candidate_id: &str,
    ) -> Result<Value, RuntimeError> {
        let session = self
            .store
            .get_agentic_session_for_binding(project_id, candidate_id)?;
        Ok(match session {
            Some(session) => session_result_with_authoring(self, &session, "lookup")?,
            None => json!({
                "schema_version":"AgenticSessionLookupResult@1",
                "status":"unavailable",
                "durable":false,
                "read_only":true,
                "project_id":project_id,
                "candidate_id":candidate_id,
                "reason":"no durable DesignSession exists for this candidate"
            }),
        })
    }
}

fn request_object<'a>(
    request: &'a Value,
    operation: &str,
) -> Result<&'a Map<String, Value>, RuntimeError> {
    request.as_object().ok_or_else(|| {
        RuntimeError::InvalidInput(format!(
            "AGENTIC_INVALID_INPUT: {operation} requires an object"
        ))
    })
}

/// Read the durable authoring context that a DesignSession was created from.
/// The session stores object hashes, while this method verifies the CAS bytes,
/// object metadata, canonical hashes and cross-object bindings before exposing
/// the DesignSpec/ReferenceCanvas to MCP or the Viewer.  This is a readback
/// operation; it never rebuilds or rewrites either object.
fn session_result_with_authoring(
    runtime: &Runtime,
    session: &AgenticSessionRecord,
    status: &str,
) -> Result<Value, RuntimeError> {
    let authoring_context = read_authoring_context(runtime, session)?;
    let documents = json!({
        "reference_canvas": {
            "object_sha256": session.reference_canvas_sha256,
            "document": authoring_context["reference_canvas"].clone()
        },
        "design_spec": {
            "object_sha256": session.design_spec_sha256,
            "document": authoring_context["design_spec"].clone()
        }
    });
    let mut result = session_result(session, status);
    result["documents"] = documents;
    result["authoring_context"] = authoring_context;
    Ok(result)
}

fn read_authoring_context(
    runtime: &Runtime,
    session: &AgenticSessionRecord,
) -> Result<Value, RuntimeError> {
    let canvas = read_authoring_object(
        runtime,
        &session.reference_canvas_sha256,
        "agentic-reference-canvas",
        "ReferenceCanvas@1",
    )?;
    let spec = read_authoring_object(
        runtime,
        &session.design_spec_sha256,
        "agentic-design-spec",
        "DesignSpec@1",
    )?;

    if canvas.get("canvas_id").and_then(Value::as_str) != Some(session.reference_canvas_id.as_str())
        || canvas.get("project_id").and_then(Value::as_str) != Some(session.project_id.as_str())
        || !canvas
            .get("reference_set_sha256")
            .and_then(Value::as_str)
            .is_some_and(is_sha256)
    {
        return Err(RuntimeError::InvalidInput(
            "AGENTIC_REFERENCE_CANVAS_BINDING_MISMATCH".to_owned(),
        ));
    }
    if spec.get("spec_id").and_then(Value::as_str) != Some(session.design_spec_id.as_str())
        || spec.get("project_id").and_then(Value::as_str) != Some(session.project_id.as_str())
        || spec.get("reference_canvas_id").and_then(Value::as_str)
            != Some(session.reference_canvas_id.as_str())
        || spec.get("reference_canvas_sha256").and_then(Value::as_str)
            != Some(session.reference_canvas_sha256.as_str())
    {
        return Err(RuntimeError::InvalidInput(
            "AGENTIC_DESIGN_SPEC_BINDING_MISMATCH".to_owned(),
        ));
    }

    let view_matches = canvas
        .get("views")
        .and_then(Value::as_array)
        .is_some_and(|views| {
            views.iter().any(|view| {
                view.get("reference_id").and_then(Value::as_str)
                    == Some(session.reference_id.as_str())
                    && view.get("reference_sha256").and_then(Value::as_str)
                        == Some(session.reference_sha256.as_str())
            })
        });
    if !view_matches {
        return Err(RuntimeError::InvalidInput(
            "AGENTIC_REFERENCE_CANVAS_VIEW_BINDING_MISMATCH".to_owned(),
        ));
    }

    let mut context = json!({
        "schema_version":"AgenticAuthoringContext@1",
        "durable":true,
        "read_only":true,
        "session_id":session.session_id,
        "project_id":session.project_id,
        "candidate_id":session.candidate_id,
        "reference_id":session.reference_id,
        "reference_sha256":session.reference_sha256,
        "reference_canvas_object_sha256":session.reference_canvas_sha256,
        "design_spec_object_sha256":session.design_spec_sha256,
        "reference_canvas":canvas,
        "design_spec":spec,
        "canonical_sha256":""
    });
    context["canonical_sha256"] = Value::String(canonical_json_hash(&context));
    Ok(context)
}

/// Return the durable ReferenceCanvas for an existing project/candidate
/// session without rebuilding a session or invoking the observation
/// projection.  Agentic scene observation uses this narrow helper so the
/// multi-view authoring facts remain the same CAS-bound object that
/// `session_get` exposes.  No session means the caller must retain its
/// conservative single-reference projection.
pub(crate) fn durable_reference_canvas_for_binding(
    runtime: &Runtime,
    project_id: &str,
    candidate_id: Option<&str>,
) -> Result<Option<(Value, String)>, RuntimeError> {
    let Some(candidate_id) = candidate_id else {
        return Ok(None);
    };
    let Some(session) = runtime
        .store
        .get_agentic_session_for_binding(project_id, candidate_id)?
    else {
        return Ok(None);
    };
    let authoring_context = read_authoring_context(runtime, &session)?;
    let canvas = authoring_context
        .get("reference_canvas")
        .cloned()
        .ok_or_else(|| {
            RuntimeError::InvalidInput(
                "AGENTIC_REFERENCE_CANVAS_READBACK_MISSING: durable canvas is absent".to_owned(),
            )
        })?;
    Ok(Some((canvas, session.reference_canvas_sha256)))
}

/// Read the authoring canvas from one exact durable session.  Candidate-only
/// lookup is sufficient for a single active session, but cross-view evaluation
/// must use the session whose approval and view set are being evaluated when
/// several sessions share the same candidate.
pub(crate) fn durable_reference_canvas_for_session_binding(
    runtime: &Runtime,
    project_id: &str,
    session_id: &str,
    candidate_id: &str,
) -> Result<(Value, String), RuntimeError> {
    let session = runtime
        .store
        .get_agentic_session(session_id)?
        .ok_or_else(|| RuntimeError::InvalidInput("AGENTIC_SESSION_NOT_FOUND".to_owned()))?;
    if session.project_id != project_id || session.candidate_id != candidate_id {
        return Err(RuntimeError::InvalidInput(
            "AGENTIC_SESSION_SCOPE_MISMATCH".to_owned(),
        ));
    }
    let authoring_context = read_authoring_context(runtime, &session)?;
    let canvas = authoring_context
        .get("reference_canvas")
        .cloned()
        .ok_or_else(|| {
            RuntimeError::InvalidInput(
                "AGENTIC_REFERENCE_CANVAS_READBACK_MISSING: durable canvas is absent".to_owned(),
            )
        })?;
    Ok((canvas, session.reference_canvas_sha256))
}

/// Require an explicit, hash-bound authoring context before a design action
/// can compile or compare geometry.  `session_create_or_resume` deliberately
/// retains a conservative default canvas for observation-only intake, but
/// that fallback must never become an implicit visual-quality input.  Every
/// supplied view therefore needs its own ViewSpec, target/mask pair and
/// non-unknown camera claim; the top-level binding must also point at the
/// session's evidence.  This function is read-only and never changes CAS or
/// candidate state.
pub(crate) fn require_bound_authoring_context(
    runtime: &Runtime,
    session: &AgenticSessionRecord,
) -> Result<Value, RuntimeError> {
    let context = read_authoring_context(runtime, session)?;
    let canvas = context
        .get("reference_canvas")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            RuntimeError::InvalidInput(
                "AGENTIC_EXPLICIT_AUTHORING_REQUIRED: durable ReferenceCanvas is missing"
                    .to_owned(),
            )
        })?;
    let bindings = canvas
        .get("bindings")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            RuntimeError::InvalidInput(
                "AGENTIC_EXPLICIT_AUTHORING_REQUIRED: ReferenceCanvas bindings are missing"
                    .to_owned(),
            )
        })?;
    if bindings.get("status").and_then(Value::as_str) != Some("bound")
        || bindings.get("camera_hash").and_then(Value::as_str) != Some(session.camera_hash.as_str())
        || bindings.get("evidence_sha256").and_then(Value::as_str)
            != Some(session.evidence_sha256.as_str())
    {
        return Err(RuntimeError::InvalidInput(
            "AGENTIC_EXPLICIT_AUTHORING_REQUIRED: default or stale ReferenceCanvas bindings cannot drive a design action"
                .to_owned(),
        ));
    }

    let coverage = canvas
        .get("coverage")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            RuntimeError::InvalidInput(
                "AGENTIC_EXPLICIT_AUTHORING_REQUIRED: coverage is missing".to_owned(),
            )
        })?;
    let supplied = coverage
        .get("supplied_views")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            RuntimeError::InvalidInput(
                "AGENTIC_EXPLICIT_AUTHORING_REQUIRED: supplied views are missing".to_owned(),
            )
        })?;
    let views = canvas
        .get("views")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            RuntimeError::InvalidInput(
                "AGENTIC_EXPLICIT_AUTHORING_REQUIRED: views are missing".to_owned(),
            )
        })?;
    for supplied_kind in supplied {
        let kind = supplied_kind.as_str().ok_or_else(|| {
            RuntimeError::InvalidInput(
                "AGENTIC_EXPLICIT_AUTHORING_REQUIRED: supplied view kind is invalid".to_owned(),
            )
        })?;
        let view = views
            .iter()
            .find(|view| view.get("kind").and_then(Value::as_str) == Some(kind))
            .and_then(Value::as_object)
            .ok_or_else(|| {
                RuntimeError::InvalidInput(format!(
                    "AGENTIC_EXPLICIT_AUTHORING_REQUIRED: supplied view {kind} is not authored"
                ))
            })?;
        let target = view.get("target_sha256").and_then(Value::as_str);
        let mask = view.get("mask_sha256").and_then(Value::as_str);
        if !target.is_some_and(is_sha256) || !mask.is_some_and(is_sha256) {
            return Err(RuntimeError::InvalidInput(format!(
                "AGENTIC_EXPLICIT_AUTHORING_REQUIRED: supplied view {kind} needs a hash-bound target/mask"
            )));
        }
        let view_spec = view
            .get("view_spec")
            .and_then(Value::as_object)
            .ok_or_else(|| {
                RuntimeError::InvalidInput(format!(
                    "AGENTIC_EXPLICIT_AUTHORING_REQUIRED: supplied view {kind} needs ReferenceViewSpec"
                ))
            })?;
        if !view_spec
            .get("canonical_sha256")
            .and_then(Value::as_str)
            .is_some_and(is_sha256)
        {
            return Err(RuntimeError::InvalidInput(format!(
                "AGENTIC_EXPLICIT_AUTHORING_REQUIRED: supplied view {kind} ViewSpec is not canonical"
            )));
        }
        let camera_claim = view
            .get("camera_claim")
            .and_then(Value::as_object)
            .ok_or_else(|| {
                RuntimeError::InvalidInput(format!(
                    "AGENTIC_EXPLICIT_AUTHORING_REQUIRED: supplied view {kind} camera claim is missing"
                ))
            })?;
        if !matches!(
            camera_claim.get("visibility").and_then(Value::as_str),
            Some("observed" | "inferred")
        ) || !camera_claim
            .get("camera_hash")
            .and_then(Value::as_str)
            .is_some_and(is_sha256)
        {
            return Err(RuntimeError::InvalidInput(format!(
                "AGENTIC_EXPLICIT_AUTHORING_REQUIRED: supplied view {kind} camera is unknown"
            )));
        }
    }
    Ok(context)
}

fn read_authoring_object(
    runtime: &Runtime,
    object_sha256: &str,
    expected_kind: &str,
    expected_schema: &str,
) -> Result<Value, RuntimeError> {
    if !is_sha256(object_sha256) {
        return Err(RuntimeError::InvalidInput(
            "AGENTIC_AUTHORING_OBJECT_HASH_INVALID".to_owned(),
        ));
    }
    let record = runtime.store.get_object(object_sha256)?.ok_or_else(|| {
        RuntimeError::InvalidInput("AGENTIC_AUTHORING_OBJECT_UNAVAILABLE".to_owned())
    })?;
    if record.mime != "application/json" || record.kind != expected_kind {
        return Err(RuntimeError::InvalidInput(
            "AGENTIC_AUTHORING_OBJECT_METADATA_MISMATCH".to_owned(),
        ));
    }
    let bytes = runtime.cas_read(object_sha256)?;
    if sha256_hex(&bytes) != object_sha256 {
        return Err(RuntimeError::InvalidInput(
            "AGENTIC_AUTHORING_OBJECT_HASH_MISMATCH".to_owned(),
        ));
    }
    let value: Value = serde_json::from_slice(&bytes).map_err(|error| {
        RuntimeError::InvalidInput(format!("AGENTIC_AUTHORING_OBJECT_INVALID: {error}"))
    })?;
    if value.get("schema_version").and_then(Value::as_str) != Some(expected_schema) {
        return Err(RuntimeError::InvalidInput(
            "AGENTIC_AUTHORING_OBJECT_SCHEMA_MISMATCH".to_owned(),
        ));
    }
    let canonical = value
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .ok_or_else(|| {
            RuntimeError::InvalidInput("AGENTIC_AUTHORING_OBJECT_CANONICAL_INVALID".to_owned())
        })?;
    let mut without_hash = value.clone();
    without_hash["canonical_sha256"] = Value::String(String::new());
    if canonical_json_hash(&without_hash) != canonical {
        return Err(RuntimeError::InvalidInput(
            "AGENTIC_AUTHORING_OBJECT_CANONICAL_MISMATCH".to_owned(),
        ));
    }
    Ok(value)
}

fn reject_unknown_keys(object: &Map<String, Value>, allowed: &[&str]) -> Result<(), RuntimeError> {
    if let Some(key) = object.keys().find(|key| !allowed.contains(&key.as_str())) {
        return Err(RuntimeError::InvalidInput(format!(
            "AGENTIC_INVALID_INPUT: unsupported field {key}"
        )));
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
    let value = object
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty());
    let value = value.ok_or_else(|| RuntimeError::InvalidInput(format!("{key} is required")))?;
    if !is_sha256(value) {
        return Err(RuntimeError::InvalidInput(format!(
            "{key} is not a SHA-256"
        )));
    }
    Ok(value)
}

fn required_stage<'a>(object: &'a Map<String, Value>, key: &str) -> Result<&'a str, RuntimeError> {
    let value = required_id(object, key)?;
    if !STAGES.contains(&value) {
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
    for key in ["approval_receipt_id", "approval_summary", "idempotency_key"] {
        let value = object
            .get(key)
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty());
        if value.is_none() {
            return Err(RuntimeError::InvalidInput(format!(
                "AGENTIC_APPROVAL_REQUIRED: {key} is required"
            )));
        }
    }
    let summary = object
        .get("approval_summary")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if summary.len() > 512
        || summary.contains('/')
        || summary.contains('\\')
        || summary.contains("http")
    {
        return Err(RuntimeError::InvalidInput(
            "AGENTIC_APPROVAL_REQUIRED: approval_summary is unsafe".to_owned(),
        ));
    }
    Ok(())
}

fn bound_candidate<'a>(
    runtime: &'a Runtime,
    project_id: &str,
    candidate_id: &str,
) -> Result<CandidateRecord, RuntimeError> {
    let candidate = runtime
        .candidate(candidate_id)?
        .ok_or_else(|| RuntimeError::InvalidInput("NOT_FOUND: candidate not found".to_owned()))?;
    if candidate.project_id != project_id {
        return Err(RuntimeError::InvalidInput(
            "PROJECT_SCOPE_DENIED: candidate is outside the project".to_owned(),
        ));
    }
    if !is_sha256(&candidate.canonical_sha256) {
        return Err(RuntimeError::InvalidInput(
            "AGENTIC_CANDIDATE_STATE_MISMATCH: candidate canonical hash is invalid".to_owned(),
        ));
    }
    Ok(candidate)
}

fn bound_reference<'a>(
    runtime: &'a Runtime,
    project_id: &str,
    reference_id: &str,
) -> Result<ReferenceEvidenceRecord, RuntimeError> {
    let reference = runtime
        .reference(reference_id)?
        .ok_or_else(|| RuntimeError::InvalidInput("NOT_FOUND: reference not found".to_owned()))?;
    if reference.project_id != project_id {
        return Err(RuntimeError::InvalidInput(
            "PROJECT_SCOPE_DENIED: reference is outside the project".to_owned(),
        ));
    }
    Ok(reference)
}

fn validate_observation_claims(
    observation: &Value,
    candidate: &CandidateRecord,
    reference: &ReferenceEvidenceRecord,
    camera_hash: &str,
    evidence_sha256: &str,
) -> Result<(), RuntimeError> {
    if observation.get("project_id").and_then(Value::as_str) != Some(candidate.project_id.as_str())
        || observation.get("candidate_id").and_then(Value::as_str)
            != Some(candidate.candidate_id.as_str())
    {
        return Err(RuntimeError::InvalidInput(
            "AGENTIC_OBSERVATION_BINDING_MISMATCH: observation scope differs".to_owned(),
        ));
    }
    let known = observation_hashes(observation, candidate, reference);
    if !known.iter().any(|hash| hash == evidence_sha256) {
        return Err(RuntimeError::InvalidInput(
            "AGENTIC_EVIDENCE_BINDING_MISMATCH: evidence hash is not Runtime-owned".to_owned(),
        ));
    }
    if let Some(observed_camera) = observation
        .pointer("/lineage/camera_hash")
        .and_then(Value::as_str)
    {
        if observed_camera != camera_hash {
            return Err(RuntimeError::InvalidInput(
                "AGENTIC_CAMERA_BINDING_MISMATCH: camera hash differs from visual evidence"
                    .to_owned(),
            ));
        }
    }
    Ok(())
}

fn observation_hashes(
    observation: &Value,
    candidate: &CandidateRecord,
    reference: &ReferenceEvidenceRecord,
) -> Vec<String> {
    let mut hashes = vec![
        candidate.canonical_sha256.clone(),
        reference.object_sha256.clone(),
    ];
    for pointer in [
        "/canonical_sha256",
        "/lineage/render_set_hash",
        "/lineage/comparison_report_hash",
        "/lineage/quality_report_hash",
        "/lineage/artifact_sha256",
    ] {
        if let Some(hash) = observation.pointer(pointer).and_then(Value::as_str) {
            if is_sha256(hash) {
                hashes.push(hash.to_owned());
            }
        }
    }
    hashes.sort();
    hashes.dedup();
    hashes
}

fn session_from_observation(
    observation: &Value,
    candidate: &CandidateRecord,
    reference: &ReferenceEvidenceRecord,
    session_id: &str,
    design_spec_id: &str,
    design_spec_sha256: &str,
    reference_canvas_id: &str,
    reference_canvas_sha256: &str,
    camera_hash: &str,
    evidence_sha256: &str,
    observation_sha256: &str,
) -> Result<AgenticSessionRecord, RuntimeError> {
    let stage_plan = observation.get("design_stage_plan").ok_or_else(|| {
        RuntimeError::InvalidInput("AGENTIC_PROJECTION_INVALID: stage plan missing".to_owned())
    })?;
    let stage = stage_plan
        .get("current_stage")
        .and_then(Value::as_str)
        .filter(|stage| STAGES.contains(stage))
        .unwrap_or("reference-canvas");
    let quality_status = observation
        .pointer("/quality/visual_status")
        .and_then(Value::as_str)
        .filter(|value| {
            matches!(
                *value,
                "PARTIAL_VISIBLE_VIEW_PASS"
                    | "QUALITY_TARGET_NOT_MET"
                    | "BLOCKED_REFERENCE_COVERAGE"
            )
        })
        .unwrap_or("not-run");
    let gate = session_stage_gate(observation, stage, evidence_sha256);
    Ok(AgenticSessionRecord {
        schema_version: "DesignSession@1".to_owned(),
        session_id: session_id.to_owned(),
        project_id: candidate.project_id.clone(),
        candidate_id: candidate.candidate_id.clone(),
        revision: observation
            .pointer("/snapshot/revision")
            .and_then(Value::as_i64)
            .unwrap_or(0),
        candidate_state_sha256: candidate.canonical_sha256.clone(),
        design_spec_id: design_spec_id.to_owned(),
        design_spec_sha256: design_spec_sha256.to_owned(),
        reference_canvas_id: reference_canvas_id.to_owned(),
        reference_canvas_sha256: reference_canvas_sha256.to_owned(),
        reference_id: reference.reference_id.clone(),
        reference_sha256: reference.object_sha256.clone(),
        camera_hash: camera_hash.to_owned(),
        evidence_sha256: evidence_sha256.to_owned(),
        observation_sha256: observation_sha256.to_owned(),
        current_version_id: None,
        current_version_sha256: None,
        current_stage: stage.to_owned(),
        quality_status: quality_status.to_owned(),
        status: "active".to_owned(),
        stage_gate: gate,
        next_actions: next_actions(stage, quality_status, session_id, &reference.reference_id),
        rollback: no_session_rollback(),
        current_checkpoint_id: None,
        current_checkpoint_sha256: None,
        checkpoint_ids: Vec::new(),
        lineage: observation.get("lineage").cloned().unwrap_or_else(|| {
            json!({
                "project_id":candidate.project_id,
                "candidate_id":candidate.candidate_id,
                "reference_id":reference.reference_id,
                "reference_sha256":reference.object_sha256
            })
        }),
        object_sha256: Some("0".repeat(64)),
        canonical_sha256: "0".repeat(64),
        created_at: agentic_timestamp(),
        updated_at: agentic_timestamp(),
    })
}

fn session_stage_gate(observation: &Value, stage: &str, evidence_sha256: &str) -> Value {
    let strict = observation
        .pointer("/design_stage_plan/strict_visual_gate/status")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let status = if strict == "passed" {
        "pass"
    } else {
        "unknown"
    };
    let required_checks = match stage {
        "reference-canvas" => vec!["reference-authorized", "reference-coverage"],
        "primary-form" => vec!["primary-silhouette", "primary-proportion", "visible-view"],
        "secondary-structure" => vec!["secondary-structure", "visible-view"],
        "tertiary-detail" => vec!["tertiary-detail", "visible-view"],
        "uv-pbr" => vec!["uv-tangent-pbr", "visible-view"],
        _ => vec![
            "multi-view-compare",
            "codex-typed-review",
            "human-review",
            "export-restart-hash",
        ],
    };
    let failed_checks = if status == "pass" {
        Vec::new()
    } else {
        vec!["visible-view"]
    };
    let locks = if status == "pass" {
        Vec::new()
    } else {
        vec![
            "tertiary-detail",
            "uv-pbr",
            "confirm",
            "export",
            "next-stage",
        ]
    };
    let unlocks = if status == "pass" {
        vec!["checkpoint", "next-stage"]
    } else {
        vec!["checkpoint", "bounded-repair"]
    };
    json!({
        "stage":stage,
        "status":status,
        "required_checks":required_checks,
        "failed_checks":failed_checks,
        "evidence_hashes":[evidence_sha256],
        "unlocks":unlocks,
        "locks":locks
    })
}

fn checkpoint_stage_gate(observation: &Value, visual_state: &str, evidence_sha256: &str) -> Value {
    let stage = observation
        .pointer("/design_stage_plan/current_stage")
        .and_then(Value::as_str)
        .unwrap_or("reference-canvas");
    let mut gate = session_stage_gate(observation, stage, evidence_sha256);
    if visual_state == "fail" {
        gate["status"] = Value::String("fail".to_owned());
        gate["failed_checks"] = json!(["visible-view"]);
        gate["locks"] = json!([
            "tertiary-detail",
            "uv-pbr",
            "confirm",
            "export",
            "next-stage"
        ]);
    }
    gate
}

fn next_actions(
    stage: &str,
    quality_status: &str,
    session_id: &str,
    reference_id: &str,
) -> Vec<Value> {
    let action_kind = if quality_status == "QUALITY_TARGET_NOT_MET" {
        "bounded-repair"
    } else if quality_status == "not-run" {
        "request-reference"
    } else {
        "checkpoint"
    };
    let is_reference_request = action_kind == "request-reference";
    let scope_kind = if is_reference_request {
        "reference"
    } else {
        "session"
    };
    let target_id = if is_reference_request {
        Value::String(reference_id.to_owned())
    } else {
        Value::Null
    };
    vec![json!({
        "action_id":format!("{}-{}", action_kind, &canonical_json_hash(&json!({"stage":stage,"session_id":session_id,"reference_id":reference_id}))[..16]),
        "action_kind":action_kind,
        "scope_kind":scope_kind,
        "target_id":target_id,
        "operator_id":null,
        "parameter_changes":[],
        "bounded":true,
        "description":if action_kind == "bounded-repair" {"Prepare one bounded repair and rerun compile/readback/render/compare"} else if action_kind == "request-reference" {"Request or annotate missing reference coverage before advancing"} else {"Persist a checkpoint before the next bounded action"}
    })]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_reference_next_action_binds_the_session_reference() {
        let actions = next_actions(
            "reference-canvas",
            "not-run",
            "session-test",
            "reference-test",
        );
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0]["action_kind"], "request-reference");
        assert_eq!(actions[0]["scope_kind"], "reference");
        assert_eq!(actions[0]["target_id"], "reference-test");
        assert_eq!(actions[0]["operator_id"], Value::Null);
        assert_eq!(actions[0]["parameter_changes"], json!([]));
        assert_eq!(actions[0]["bounded"], true);
        assert!(actions[0].get("stage").is_none());
        assert!(actions[0].get("evidence_sha256").is_none());
    }

    #[test]
    fn complete_coverage_requires_authored_view_entities() {
        let coverage = json!({
            "required_views":["front","back","left","right","perspective"],
            "supplied_views":["front","back","left","right","perspective"],
            "missing_views":[],
            "coverage_status":"complete",
            "hq_360_status":"eligible",
            "evidence_refs":[{"kind":"reference","sha256":"a".repeat(64)}]
        });
        let error = validate_coverage_view_bindings(
            &coverage,
            &[json!({"kind":"front"}), json!({"kind":"perspective"})],
        )
        .expect_err("complete coverage without all authored views must fail closed");
        assert_eq!(
            error.to_string(),
            "invalid runtime input: AGENTIC_AUTHORING_COVERAGE_VIEW_BINDING_MISMATCH"
        );
    }

    #[test]
    fn complete_coverage_requires_core_hq_reference_views() {
        let coverage = json!({
            "required_views":["front","back","left","right","perspective"],
            "supplied_views":["front","back","left","right","perspective"],
            "missing_views":[],
            "coverage_status":"complete",
            "hq_360_status":"eligible",
            "evidence_refs":[{"kind":"reference","sha256":"a".repeat(64)}]
        });
        let error = validate_coverage(&coverage)
            .expect_err("perspective cannot replace rear-three-quarter in HQ coverage");
        assert_eq!(
            error.to_string(),
            "invalid runtime input: AGENTIC_AUTHORING_COVERAGE_CORE_VIEWS_REQUIRED"
        );
    }

    #[test]
    fn coverage_rejects_duplicate_or_extra_authored_view_kinds() {
        let coverage = json!({
            "required_views":["front","perspective"],
            "supplied_views":["front","perspective"],
            "missing_views":[],
            "coverage_status":"complete",
            "hq_360_status":"eligible",
            "evidence_refs":[{"kind":"reference","sha256":"a".repeat(64)}]
        });
        let duplicate = validate_coverage_view_bindings(
            &coverage,
            &[
                json!({"kind":"front"}),
                json!({"kind":"front"}),
                json!({"kind":"perspective"}),
            ],
        )
        .expect_err("duplicate view kinds must fail closed");
        assert_eq!(
            duplicate.to_string(),
            "invalid runtime input: AGENTIC_AUTHORING_COVERAGE_VIEW_KIND_DUPLICATE"
        );

        let extra = validate_coverage_view_bindings(
            &coverage,
            &[
                json!({"kind":"front"}),
                json!({"kind":"perspective"}),
                json!({"kind":"detail"}),
            ],
        )
        .expect_err("an authored view outside supplied coverage must fail closed");
        assert_eq!(
            extra.to_string(),
            "invalid runtime input: AGENTIC_AUTHORING_COVERAGE_VIEW_KIND_NOT_SUPPLIED"
        );
    }

    #[test]
    fn authoring_views_cover_the_full_reference_view_set() {
        for (kind, source_view) in [
            ("front", "front"),
            ("back", "back"),
            ("left", "left"),
            ("right", "right"),
            ("top", "top"),
            ("bottom", "bottom"),
            ("perspective", "three-quarter"),
            ("front-three-quarter", "front-three-quarter"),
            ("rear-three-quarter", "rear-three-quarter"),
            ("material", "detail"),
            ("detail", "detail"),
        ] {
            let value = json!({"kind": kind});
            validate_authoring_view_kind(value.as_object().expect("view object"), "kind")
                .unwrap_or_else(|error| panic!("{kind} must be accepted: {error}"));
            assert_eq!(expected_reference_source_view(kind), Some(source_view));
        }
    }
}

fn checkpoint_actions(gate: &Value, allowed: bool) -> Vec<String> {
    let key = if allowed { "unlocks" } else { "locks" };
    gate.get(key)
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

fn observation_strings(observation: &Value, key: &str) -> Vec<String> {
    observation
        .pointer(&format!("/model_understanding_bundle/uncertainty/{key}"))
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

fn no_session_rollback() -> Value {
    json!({"relation":"none","target_checkpoint_id":null,"target_checkpoint_sha256":null,"target_version_id":null,"target_version_sha256":null,"reason":null,"runtime_confirm_allowed":false})
}

fn no_checkpoint_rollback() -> Value {
    json!({"relation":"none","target_checkpoint_id":null,"target_checkpoint_sha256":null,"target_version_id":null,"target_version_sha256":null,"reason":null})
}

fn session_result(session: &AgenticSessionRecord, status: &str) -> Value {
    json!({
        "schema_version":"AgenticSessionResult@1",
        "status":status,
        "durable":true,
        "read_only":matches!(status, "read" | "lookup"),
        "project_id":session.project_id,
        "candidate_id":session.candidate_id,
        "session_id":session.session_id,
        "revision":session.revision,
        "object_sha256":session.object_sha256,
        "lineage":session.lineage,
        "session":session_contract_value(session)
    })
}

fn checkpoint_result(
    checkpoint: &AgenticCheckpointRecord,
    session: &AgenticSessionRecord,
    status: &str,
) -> Value {
    json!({
        "schema_version":"AgenticCheckpointResult@1",
        "status":status,
        "durable":true,
        "read_only":status == "read",
        "project_id":session.project_id,
        "candidate_id":session.candidate_id,
        "session_id":session.session_id,
        "revision":session.revision,
        "object_sha256":checkpoint.object_sha256,
        "session":session_contract_value(session),
        "checkpoint":checkpoint_contract_value(checkpoint)
    })
}

fn session_contract_value(session: &AgenticSessionRecord) -> Value {
    let mut value = serde_json::to_value(session).expect("AgenticSessionRecord serializes");
    let object = value
        .as_object_mut()
        .expect("AgenticSessionRecord serializes as an object");
    object.remove("revision");
    object.remove("lineage");
    object.remove("object_sha256");
    value
}

fn checkpoint_contract_value(checkpoint: &AgenticCheckpointRecord) -> Value {
    let mut value = serde_json::to_value(checkpoint).expect("AgenticCheckpointRecord serializes");
    let object = value
        .as_object_mut()
        .expect("AgenticCheckpointRecord serializes as an object");
    for key in [
        "revision",
        "input_sha256",
        "observed",
        "inferred",
        "unknown",
        "failed_gates",
        "allowed_actions",
        "locked_actions",
        "object_sha256",
    ] {
        object.remove(key);
    }
    value
}

fn validate_session_binding(
    session: &AgenticSessionRecord,
    session_id: &str,
    project_id: &str,
    candidate_id: &str,
    reference_id: &str,
    camera_hash: &str,
    evidence_sha256: &str,
    observation_sha256: &str,
) -> Result<(), RuntimeError> {
    if session.session_id != session_id
        || session.project_id != project_id
        || session.candidate_id != candidate_id
        || session.reference_id != reference_id
        || session.camera_hash != camera_hash
        || session.evidence_sha256 != evidence_sha256
        || session.observation_sha256 != observation_sha256
    {
        return Err(RuntimeError::InvalidInput(
            "AGENTIC_SESSION_BINDING_MISMATCH: session scope or evidence differs".to_owned(),
        ));
    }
    Ok(())
}

fn observation_hash(observation: &Value) -> Result<String, RuntimeError> {
    observation
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .map(str::to_owned)
        .ok_or_else(|| {
            RuntimeError::InvalidInput(
                "AGENTIC_PROJECTION_INVALID: observation canonical hash is missing".to_owned(),
            )
        })
}

fn with_session_canonical(
    mut session: AgenticSessionRecord,
) -> Result<AgenticSessionRecord, RuntimeError> {
    let mut value = serde_json::to_value(&session)
        .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?;
    value["object_sha256"] = Value::Null;
    value["canonical_sha256"] = Value::String(String::new());
    session.canonical_sha256 = canonical_json_hash(&value);
    Ok(session)
}

fn with_checkpoint_canonical(
    mut checkpoint: AgenticCheckpointRecord,
) -> Result<AgenticCheckpointRecord, RuntimeError> {
    let mut value = serde_json::to_value(&checkpoint)
        .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?;
    value["object_sha256"] = Value::Null;
    value["canonical_sha256"] = Value::String(String::new());
    checkpoint.canonical_sha256 = canonical_json_hash(&value);
    Ok(checkpoint)
}

fn canonical_value(mut value: Value) -> Value {
    value["canonical_sha256"] = Value::String(String::new());
    value["canonical_sha256"] = Value::String(canonical_json_hash(&value));
    value
}

fn canonical_json_bytes(value: &Value) -> Result<Vec<u8>, RuntimeError> {
    super::canonical_json_bytes(value)
        .map_err(|error| RuntimeError::InvalidInput(error.to_string()))
}

/// Validate and canonicalize an explicit authoring payload supplied by Codex.
/// The default session path still creates a conservative single-reference
/// canvas/spec, but an authoring payload is the Runtime-owned producer path for
/// semantic views, observed/inferred/unknown facts and stage constraints.  No
/// image bytes, paths or executable instructions are accepted here.
fn build_reference_canvas_from_authoring(
    runtime: &Runtime,
    authoring: &Value,
    project_id: &str,
    canvas_id: &str,
    primary_reference: &ReferenceEvidenceRecord,
    expected_camera_hash: &str,
    expected_evidence_sha256: &str,
) -> Result<Value, RuntimeError> {
    let authoring = authoring_object(authoring)?;
    let canvas = authoring
        .get("reference_canvas")
        .ok_or_else(|| RuntimeError::InvalidInput("AGENTIC_AUTHORING_CANVAS_REQUIRED".to_owned()))?
        .clone();
    let canvas_object = canvas
        .as_object()
        .ok_or_else(|| RuntimeError::InvalidInput("AGENTIC_AUTHORING_CANVAS_INVALID".to_owned()))?;
    reject_authoring_keys(
        canvas_object,
        &[
            "schema_version",
            "canvas_id",
            "project_id",
            "reference_set_sha256",
            "bindings",
            "views",
            "coverage",
            "unknowns",
            "claims",
            "canonical_sha256",
            "created_at",
        ],
        "ReferenceCanvas",
    )?;
    validate_bounded_authoring_value(&canvas, 0)?;
    if canvas_object.get("schema_version").and_then(Value::as_str) != Some("ReferenceCanvas@1")
        || canvas_object.get("canvas_id").and_then(Value::as_str) != Some(canvas_id)
        || canvas_object.get("project_id").and_then(Value::as_str) != Some(project_id)
    {
        return Err(RuntimeError::InvalidInput(
            "AGENTIC_AUTHORING_CANVAS_BINDING_MISMATCH".to_owned(),
        ));
    }
    let reference_set_sha256 =
        required_authoring_sha(canvas_object, "reference_set_sha256", "ReferenceCanvas")?;
    let views = canvas_object
        .get("views")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            RuntimeError::InvalidInput("AGENTIC_AUTHORING_CANVAS_VIEWS_INVALID".to_owned())
        })?;
    if views.is_empty() || views.len() > 32 {
        return Err(RuntimeError::InvalidInput(
            "AGENTIC_AUTHORING_CANVAS_VIEWS_OUT_OF_BOUNDS".to_owned(),
        ));
    }
    let mut view_ids = HashSet::new();
    let mut reference_pairs = Vec::new();
    let mut has_primary_view = false;
    for view in views {
        let view_object = view.as_object().ok_or_else(|| {
            RuntimeError::InvalidInput("AGENTIC_AUTHORING_VIEW_INVALID".to_owned())
        })?;
        reject_authoring_keys(
            view_object,
            &[
                "view_id",
                "reference_id",
                "reference_sha256",
                "kind",
                "authorization",
                "image_dimensions",
                "view_spec",
                "target_sha256",
                "mask_sha256",
                "camera_claim",
                "visible_regions",
                "unknown_regions",
            ],
            "ReferenceCanvas.view",
        )?;
        let view_id = required_authoring_id(view_object, "view_id", "ReferenceCanvas.view")?;
        if !view_ids.insert(view_id.to_owned()) {
            return Err(RuntimeError::InvalidInput(
                "AGENTIC_AUTHORING_VIEW_ID_DUPLICATE".to_owned(),
            ));
        }
        let reference_id =
            required_authoring_id(view_object, "reference_id", "ReferenceCanvas.view")?;
        let reference_sha256 =
            required_authoring_sha(view_object, "reference_sha256", "ReferenceCanvas.view")?;
        let reference = runtime.reference(reference_id)?.ok_or_else(|| {
            RuntimeError::InvalidInput("AGENTIC_AUTHORING_VIEW_REFERENCE_NOT_FOUND".to_owned())
        })?;
        if reference.project_id != project_id || reference.object_sha256 != reference_sha256 {
            return Err(RuntimeError::InvalidInput(
                "AGENTIC_AUTHORING_VIEW_REFERENCE_BINDING_MISMATCH".to_owned(),
            ));
        }
        if reference_id == primary_reference.reference_id
            && reference_sha256 == primary_reference.object_sha256
        {
            has_primary_view = true;
        }
        validate_authoring_view_kind(view_object, "kind")?;
        validate_authorization_claim(
            view_object.get("authorization").ok_or_else(|| {
                RuntimeError::InvalidInput(
                    "AGENTIC_AUTHORING_VIEW_AUTHORIZATION_REQUIRED".to_owned(),
                )
            })?,
            &reference,
            reference_sha256,
        )?;
        validate_image_dimensions(
            view_object.get("image_dimensions").ok_or_else(|| {
                RuntimeError::InvalidInput("AGENTIC_AUTHORING_VIEW_DIMENSIONS_REQUIRED".to_owned())
            })?,
            reference.width,
            reference.height,
        )?;
        validate_camera_claim(view_object.get("camera_claim").ok_or_else(|| {
            RuntimeError::InvalidInput("AGENTIC_AUTHORING_VIEW_CAMERA_REQUIRED".to_owned())
        })?)?;
        validate_reference_view_annotations(runtime, view_object, &reference)?;
        validate_regions(view_object.get("visible_regions").ok_or_else(|| {
            RuntimeError::InvalidInput("AGENTIC_AUTHORING_VIEW_REGIONS_REQUIRED".to_owned())
        })?)?;
        validate_unknown_regions(view_object.get("unknown_regions").ok_or_else(|| {
            RuntimeError::InvalidInput("AGENTIC_AUTHORING_VIEW_UNKNOWN_REGIONS_REQUIRED".to_owned())
        })?)?;
        reference_pairs.push((reference_id.to_owned(), reference_sha256.to_owned()));
    }
    if !has_primary_view {
        return Err(RuntimeError::InvalidInput(
            "AGENTIC_AUTHORING_CANVAS_PRIMARY_VIEW_REQUIRED".to_owned(),
        ));
    }
    let expected_set_hash = reference_set_hash(&reference_pairs);
    if reference_set_sha256 != expected_set_hash
        && !(reference_pairs
            .iter()
            .map(|(_, sha256)| sha256.as_str())
            .collect::<HashSet<_>>()
            .len()
            == 1
            && reference_set_sha256 == primary_reference.object_sha256)
    {
        return Err(RuntimeError::InvalidInput(
            "AGENTIC_AUTHORING_REFERENCE_SET_HASH_MISMATCH".to_owned(),
        ));
    }
    validate_reference_canvas_bindings(
        runtime,
        canvas_object.get("bindings").ok_or_else(|| {
            RuntimeError::InvalidInput("AGENTIC_AUTHORING_CANVAS_BINDINGS_REQUIRED".to_owned())
        })?,
        views,
        project_id,
        primary_reference,
        expected_camera_hash,
        expected_evidence_sha256,
    )?;
    let coverage = canvas_object.get("coverage").ok_or_else(|| {
        RuntimeError::InvalidInput("AGENTIC_AUTHORING_CANVAS_COVERAGE_REQUIRED".to_owned())
    })?;
    validate_coverage(coverage)?;
    validate_coverage_view_bindings(coverage, views)?;
    validate_unknowns(canvas_object.get("unknowns").ok_or_else(|| {
        RuntimeError::InvalidInput("AGENTIC_AUTHORING_CANVAS_UNKNOWNS_REQUIRED".to_owned())
    })?)?;
    validate_claims(canvas_object.get("claims").ok_or_else(|| {
        RuntimeError::InvalidInput("AGENTIC_AUTHORING_CANVAS_CLAIMS_REQUIRED".to_owned())
    })?)?;
    required_authoring_text(canvas_object, "created_at", "ReferenceCanvas")?;
    canonicalize_authoring_value(canvas, "ReferenceCanvas@1")
}

fn validate_reference_canvas_bindings(
    runtime: &Runtime,
    value: &Value,
    views: &[Value],
    project_id: &str,
    primary_reference: &ReferenceEvidenceRecord,
    expected_camera_hash: &str,
    expected_evidence_sha256: &str,
) -> Result<(), RuntimeError> {
    let object = value.as_object().ok_or_else(|| {
        RuntimeError::InvalidInput("AGENTIC_AUTHORING_CANVAS_BINDINGS_INVALID".to_owned())
    })?;
    reject_authoring_keys(
        object,
        &[
            "status",
            "target_sha256",
            "camera_hash",
            "camera_canonical_sha256",
            "evidence_sha256",
        ],
        "ReferenceCanvas.bindings",
    )?;
    let status = object
        .get("status")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            RuntimeError::InvalidInput(
                "AGENTIC_AUTHORING_CANVAS_BINDINGS_STATUS_REQUIRED".to_owned(),
            )
        })?;
    let target = object.get("target_sha256").ok_or_else(|| {
        RuntimeError::InvalidInput("AGENTIC_AUTHORING_CANVAS_BINDINGS_TARGET_REQUIRED".to_owned())
    })?;
    let camera_hash = object.get("camera_hash").ok_or_else(|| {
        RuntimeError::InvalidInput("AGENTIC_AUTHORING_CANVAS_BINDINGS_CAMERA_REQUIRED".to_owned())
    })?;
    let camera_canonical = object.get("camera_canonical_sha256").ok_or_else(|| {
        RuntimeError::InvalidInput(
            "AGENTIC_AUTHORING_CANVAS_BINDINGS_CAMERA_CANONICAL_REQUIRED".to_owned(),
        )
    })?;
    let evidence = object.get("evidence_sha256").ok_or_else(|| {
        RuntimeError::InvalidInput("AGENTIC_AUTHORING_CANVAS_BINDINGS_EVIDENCE_REQUIRED".to_owned())
    })?;
    match status {
        "unbound" => {
            if !target.is_null()
                || !camera_hash.is_null()
                || !camera_canonical.is_null()
                || !evidence.is_null()
            {
                return Err(RuntimeError::InvalidInput(
                    "AGENTIC_AUTHORING_CANVAS_UNBOUND_LINEAGE_MUST_BE_NULL".to_owned(),
                ));
            }
        }
        "bound" => {
            let target_sha256 = target
                .as_str()
                .filter(|value| is_sha256(value))
                .ok_or_else(|| {
                    RuntimeError::InvalidInput(
                        "AGENTIC_AUTHORING_CANVAS_TARGET_HASH_INVALID".to_owned(),
                    )
                })?;
            let camera_hash = camera_hash
                .as_str()
                .filter(|value| is_sha256(value))
                .ok_or_else(|| {
                    RuntimeError::InvalidInput(
                        "AGENTIC_AUTHORING_CANVAS_CAMERA_HASH_INVALID".to_owned(),
                    )
                })?;
            let camera_canonical = camera_canonical
                .as_str()
                .filter(|value| is_sha256(value))
                .ok_or_else(|| {
                    RuntimeError::InvalidInput(
                        "AGENTIC_AUTHORING_CANVAS_CAMERA_CANONICAL_INVALID".to_owned(),
                    )
                })?;
            let evidence = evidence
                .as_str()
                .filter(|value| is_sha256(value))
                .ok_or_else(|| {
                    RuntimeError::InvalidInput(
                        "AGENTIC_AUTHORING_CANVAS_EVIDENCE_HASH_INVALID".to_owned(),
                    )
                })?;
            if camera_hash != expected_camera_hash || evidence != expected_evidence_sha256 {
                return Err(RuntimeError::InvalidInput(
                    "AGENTIC_AUTHORING_CANVAS_SESSION_LINEAGE_MISMATCH".to_owned(),
                ));
            }
            let target = runtime.read_silhouette_target(target_sha256)?;
            let target_reference_id = target
                .get("reference_id")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    RuntimeError::InvalidInput(
                        "AGENTIC_AUTHORING_CANVAS_TARGET_REFERENCE_MISSING".to_owned(),
                    )
                })?;
            let target_reference_sha256 = target
                .get("reference_sha256")
                .and_then(Value::as_str)
                .filter(|value| is_sha256(value))
                .ok_or_else(|| {
                    RuntimeError::InvalidInput(
                        "AGENTIC_AUTHORING_CANVAS_TARGET_REFERENCE_HASH_INVALID".to_owned(),
                    )
                })?;
            if target_reference_id != primary_reference.reference_id
                || target_reference_sha256 != primary_reference.object_sha256
                || !views.iter().any(|view| {
                    view.get("reference_id").and_then(Value::as_str) == Some(target_reference_id)
                        && view.get("reference_sha256").and_then(Value::as_str)
                            == Some(target_reference_sha256)
                })
            {
                return Err(RuntimeError::InvalidInput(
                    "AGENTIC_AUTHORING_CANVAS_TARGET_REFERENCE_BINDING_MISMATCH".to_owned(),
                ));
            }
            runtime.cas_read(evidence).map_err(|_| {
                RuntimeError::InvalidInput(
                    "AGENTIC_AUTHORING_CANVAS_EVIDENCE_OBJECT_NOT_FOUND".to_owned(),
                )
            })?;
            let _ = (project_id, camera_canonical);
        }
        _ => {
            return Err(RuntimeError::InvalidInput(
                "AGENTIC_AUTHORING_CANVAS_BINDINGS_STATUS_INVALID".to_owned(),
            ));
        }
    }
    Ok(())
}

fn build_design_spec_from_authoring(
    authoring: &Value,
    project_id: &str,
    spec_id: &str,
    canvas_id: &str,
    canvas_sha256: &str,
    _primary_reference: &ReferenceEvidenceRecord,
) -> Result<Value, RuntimeError> {
    let authoring = authoring_object(authoring)?;
    let spec = authoring
        .get("design_spec")
        .ok_or_else(|| RuntimeError::InvalidInput("AGENTIC_AUTHORING_SPEC_REQUIRED".to_owned()))?
        .clone();
    let spec_object = spec
        .as_object()
        .ok_or_else(|| RuntimeError::InvalidInput("AGENTIC_AUTHORING_SPEC_INVALID".to_owned()))?;
    reject_authoring_keys(
        spec_object,
        &[
            "schema_version",
            "spec_id",
            "project_id",
            "reference_canvas_id",
            "reference_canvas_sha256",
            "category",
            "style",
            "primary_forms",
            "proportions",
            "semantic_parts",
            "material_language",
            "stage_goals",
            "risks",
            "unknowns",
            "canonical_sha256",
            "created_at",
        ],
        "DesignSpec",
    )?;
    validate_bounded_authoring_value(&spec, 0)?;
    if spec_object.get("schema_version").and_then(Value::as_str) != Some("DesignSpec@1")
        || spec_object.get("spec_id").and_then(Value::as_str) != Some(spec_id)
        || spec_object.get("project_id").and_then(Value::as_str) != Some(project_id)
        || spec_object
            .get("reference_canvas_id")
            .and_then(Value::as_str)
            != Some(canvas_id)
        || spec_object
            .get("reference_canvas_sha256")
            .and_then(Value::as_str)
            != Some(canvas_sha256)
    {
        return Err(RuntimeError::InvalidInput(
            "AGENTIC_AUTHORING_SPEC_BINDING_MISMATCH".to_owned(),
        ));
    }
    required_authoring_text(spec_object, "category", "DesignSpec")?;
    required_authoring_text(spec_object, "style", "DesignSpec")?;
    validate_nonempty_array(spec_object, "primary_forms", 64, "DesignSpec")?;
    validate_nonempty_array(spec_object, "semantic_parts", 256, "DesignSpec")?;
    validate_nonempty_array(spec_object, "stage_goals", 6, "DesignSpec")?;
    validate_bounded_array(spec_object, "proportions", 128, "DesignSpec")?;
    validate_bounded_array(spec_object, "material_language", 128, "DesignSpec")?;
    validate_bounded_array(spec_object, "risks", 128, "DesignSpec")?;
    validate_bounded_array(spec_object, "unknowns", 128, "DesignSpec")?;
    validate_design_spec_states(spec_object)?;
    required_authoring_text(spec_object, "created_at", "DesignSpec")?;
    canonicalize_authoring_value(spec, "DesignSpec@1")
}

fn authoring_object(authoring: &Value) -> Result<&Map<String, Value>, RuntimeError> {
    let object = authoring.as_object().ok_or_else(|| {
        RuntimeError::InvalidInput("AGENTIC_AUTHORING_CONTEXT_INVALID".to_owned())
    })?;
    reject_authoring_keys(
        object,
        &["reference_canvas", "design_spec"],
        "authoring_context",
    )?;
    Ok(object)
}

fn reject_authoring_keys(
    object: &Map<String, Value>,
    allowed: &[&str],
    label: &str,
) -> Result<(), RuntimeError> {
    if let Some(key) = object.keys().find(|key| !allowed.contains(&key.as_str())) {
        return Err(RuntimeError::InvalidInput(format!(
            "AGENTIC_AUTHORING_UNSUPPORTED_FIELD: {label}.{key}"
        )));
    }
    Ok(())
}

fn validate_bounded_authoring_value(value: &Value, depth: usize) -> Result<(), RuntimeError> {
    if depth > 12 {
        return Err(RuntimeError::InvalidInput(
            "AGENTIC_AUTHORING_NESTING_OUT_OF_BOUNDS".to_owned(),
        ));
    }
    match value {
        Value::Null | Value::Bool(_) | Value::Number(_) => Ok(()),
        Value::String(text) => {
            if text.len() > 4096 || unsafe_authoring_text(text) {
                return Err(RuntimeError::InvalidInput(
                    "AGENTIC_AUTHORING_UNSAFE_TEXT".to_owned(),
                ));
            }
            Ok(())
        }
        Value::Array(values) => {
            if values.len() > 256 {
                return Err(RuntimeError::InvalidInput(
                    "AGENTIC_AUTHORING_ARRAY_OUT_OF_BOUNDS".to_owned(),
                ));
            }
            for value in values {
                validate_bounded_authoring_value(value, depth + 1)?;
            }
            Ok(())
        }
        Value::Object(values) => {
            if values.len() > 64 {
                return Err(RuntimeError::InvalidInput(
                    "AGENTIC_AUTHORING_OBJECT_OUT_OF_BOUNDS".to_owned(),
                ));
            }
            for (key, value) in values {
                let lowered = key.to_ascii_lowercase();
                if lowered.split(['_', '-', '.']).any(|part| {
                    matches!(
                        part,
                        "path"
                            | "url"
                            | "secret"
                            | "token"
                            | "password"
                            | "script"
                            | "command"
                            | "shell"
                            | "bytes"
                            | "base64"
                            | "prompt"
                    )
                }) || lowered.contains("api_key")
                    || lowered.contains("apikey")
                {
                    return Err(RuntimeError::InvalidInput(format!(
                        "AGENTIC_AUTHORING_FORBIDDEN_FIELD: {key}"
                    )));
                }
                validate_bounded_authoring_value(value, depth + 1)?;
            }
            Ok(())
        }
    }
}

fn unsafe_authoring_text(text: &str) -> bool {
    let lowered = text.to_ascii_lowercase();
    text.starts_with('/')
        || text.starts_with('\\')
        || lowered.contains("://")
        || lowered.contains("api_key")
        || lowered.contains("secret")
        || lowered.contains("token")
        || lowered.contains("password")
}

fn required_authoring_id<'a>(
    object: &'a Map<String, Value>,
    key: &str,
    label: &str,
) -> Result<&'a str, RuntimeError> {
    let value = object.get(key).and_then(Value::as_str).ok_or_else(|| {
        RuntimeError::InvalidInput(format!("AGENTIC_AUTHORING_REQUIRED: {label}.{key}"))
    })?;
    if !is_opaque_id(value) {
        return Err(RuntimeError::InvalidInput(format!(
            "AGENTIC_AUTHORING_INVALID_ID: {label}.{key}"
        )));
    }
    Ok(value)
}

fn required_authoring_sha<'a>(
    object: &'a Map<String, Value>,
    key: &str,
    label: &str,
) -> Result<&'a str, RuntimeError> {
    let value = object.get(key).and_then(Value::as_str).ok_or_else(|| {
        RuntimeError::InvalidInput(format!("AGENTIC_AUTHORING_REQUIRED: {label}.{key}"))
    })?;
    if !is_sha256(value) {
        return Err(RuntimeError::InvalidInput(format!(
            "AGENTIC_AUTHORING_INVALID_SHA256: {label}.{key}"
        )));
    }
    Ok(value)
}

fn required_authoring_text<'a>(
    object: &'a Map<String, Value>,
    key: &str,
    label: &str,
) -> Result<&'a str, RuntimeError> {
    let value = object.get(key).and_then(Value::as_str).ok_or_else(|| {
        RuntimeError::InvalidInput(format!("AGENTIC_AUTHORING_REQUIRED: {label}.{key}"))
    })?;
    if value.is_empty() || value.len() > 512 || unsafe_authoring_text(value) {
        return Err(RuntimeError::InvalidInput(format!(
            "AGENTIC_AUTHORING_INVALID_TEXT: {label}.{key}"
        )));
    }
    Ok(value)
}

fn canonicalize_authoring_value(
    mut value: Value,
    expected_schema: &str,
) -> Result<Value, RuntimeError> {
    let supplied = {
        let object = value.as_object().ok_or_else(|| {
            RuntimeError::InvalidInput("AGENTIC_AUTHORING_OBJECT_INVALID".to_owned())
        })?;
        if object.get("schema_version").and_then(Value::as_str) != Some(expected_schema) {
            return Err(RuntimeError::InvalidInput(
                "AGENTIC_AUTHORING_SCHEMA_MISMATCH".to_owned(),
            ));
        }
        let supplied = object
            .get("canonical_sha256")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                RuntimeError::InvalidInput("AGENTIC_AUTHORING_CANONICAL_REQUIRED".to_owned())
            })?;
        if !supplied.is_empty() && !is_sha256(supplied) {
            return Err(RuntimeError::InvalidInput(
                "AGENTIC_AUTHORING_CANONICAL_INVALID".to_owned(),
            ));
        }
        supplied.to_owned()
    };
    value["canonical_sha256"] = Value::String(String::new());
    let canonical = canonical_json_hash(&value);
    let normalized_canonical = canonical_json_hash(&normalize_json_numbers(&value));
    if !supplied.is_empty() && supplied != canonical && supplied != normalized_canonical {
        return Err(RuntimeError::InvalidInput(
            "AGENTIC_AUTHORING_CANONICAL_MISMATCH".to_owned(),
        ));
    }
    value["canonical_sha256"] = Value::String(canonical);
    Ok(value)
}

fn validate_authoring_view_kind(
    object: &Map<String, Value>,
    key: &str,
) -> Result<(), RuntimeError> {
    let kind = object.get(key).and_then(Value::as_str).ok_or_else(|| {
        RuntimeError::InvalidInput("AGENTIC_AUTHORING_VIEW_KIND_REQUIRED".to_owned())
    })?;
    if !matches!(
        kind,
        "front"
            | "back"
            | "left"
            | "right"
            | "top"
            | "bottom"
            | "perspective"
            | "front-three-quarter"
            | "rear-three-quarter"
            | "material"
            | "detail"
    ) {
        return Err(RuntimeError::InvalidInput(
            "AGENTIC_AUTHORING_VIEW_KIND_INVALID".to_owned(),
        ));
    }
    Ok(())
}

fn validate_authorization_claim(
    value: &Value,
    reference: &ReferenceEvidenceRecord,
    reference_sha256: &str,
) -> Result<(), RuntimeError> {
    let object = value.as_object().ok_or_else(|| {
        RuntimeError::InvalidInput("AGENTIC_AUTHORING_AUTHORIZATION_INVALID".to_owned())
    })?;
    reject_authoring_keys(
        object,
        &["user_authorized", "declaration", "evidence_refs"],
        "ReferenceCanvas.authorization",
    )?;
    if object.get("user_authorized") != Some(&Value::Bool(true))
        || object.get("declaration").and_then(Value::as_str)
            != Some(reference.authorization.declaration.as_str())
    {
        return Err(RuntimeError::InvalidInput(
            "AGENTIC_AUTHORING_AUTHORIZATION_MISMATCH".to_owned(),
        ));
    }
    let declaration = object
        .get("declaration")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            RuntimeError::InvalidInput(
                "AGENTIC_AUTHORING_AUTHORIZATION_DECLARATION_REQUIRED".to_owned(),
            )
        })?;
    if declaration.is_empty() || declaration.len() > 512 || unsafe_authoring_text(declaration) {
        return Err(RuntimeError::InvalidInput(
            "AGENTIC_AUTHORING_AUTHORIZATION_DECLARATION_INVALID".to_owned(),
        ));
    }
    validate_evidence_refs(
        object.get("evidence_refs").ok_or_else(|| {
            RuntimeError::InvalidInput("AGENTIC_AUTHORING_EVIDENCE_REFS_REQUIRED".to_owned())
        })?,
        reference_sha256,
    )?;
    Ok(())
}

fn validate_evidence_refs(value: &Value, required_sha256: &str) -> Result<(), RuntimeError> {
    let refs = value.as_array().ok_or_else(|| {
        RuntimeError::InvalidInput("AGENTIC_AUTHORING_EVIDENCE_REFS_INVALID".to_owned())
    })?;
    if refs.is_empty() || refs.len() > 16 {
        return Err(RuntimeError::InvalidInput(
            "AGENTIC_AUTHORING_EVIDENCE_REFS_OUT_OF_BOUNDS".to_owned(),
        ));
    }
    let mut found = false;
    for item in refs {
        let object = item.as_object().ok_or_else(|| {
            RuntimeError::InvalidInput("AGENTIC_AUTHORING_EVIDENCE_REF_INVALID".to_owned())
        })?;
        reject_authoring_keys(object, &["kind", "sha256"], "evidence_ref")?;
        let kind = object.get("kind").and_then(Value::as_str).ok_or_else(|| {
            RuntimeError::InvalidInput("AGENTIC_AUTHORING_EVIDENCE_KIND_REQUIRED".to_owned())
        })?;
        if kind.is_empty() || kind.len() > 32 {
            return Err(RuntimeError::InvalidInput(
                "AGENTIC_AUTHORING_EVIDENCE_KIND_INVALID".to_owned(),
            ));
        }
        let sha256 = object
            .get("sha256")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                RuntimeError::InvalidInput("AGENTIC_AUTHORING_EVIDENCE_HASH_REQUIRED".to_owned())
            })?;
        if !is_sha256(sha256) {
            return Err(RuntimeError::InvalidInput(
                "AGENTIC_AUTHORING_EVIDENCE_HASH_INVALID".to_owned(),
            ));
        }
        found |= sha256 == required_sha256;
    }
    if !found {
        return Err(RuntimeError::InvalidInput(
            "AGENTIC_AUTHORING_EVIDENCE_BINDING_MISMATCH".to_owned(),
        ));
    }
    Ok(())
}

fn validate_image_dimensions(
    value: &Value,
    expected_width: u32,
    expected_height: u32,
) -> Result<(), RuntimeError> {
    let object = value.as_object().ok_or_else(|| {
        RuntimeError::InvalidInput("AGENTIC_AUTHORING_IMAGE_DIMENSIONS_INVALID".to_owned())
    })?;
    reject_authoring_keys(object, &["width", "height"], "image_dimensions")?;
    let width = object.get("width").and_then(Value::as_u64).ok_or_else(|| {
        RuntimeError::InvalidInput("AGENTIC_AUTHORING_IMAGE_WIDTH_REQUIRED".to_owned())
    })?;
    let height = object
        .get("height")
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            RuntimeError::InvalidInput("AGENTIC_AUTHORING_IMAGE_HEIGHT_REQUIRED".to_owned())
        })?;
    if width == 0
        || width > 16_384
        || height == 0
        || height > 16_384
        || width != expected_width as u64
        || height != expected_height as u64
    {
        return Err(RuntimeError::InvalidInput(
            "AGENTIC_AUTHORING_IMAGE_DIMENSIONS_MISMATCH".to_owned(),
        ));
    }
    Ok(())
}

fn validate_camera_claim(value: &Value) -> Result<(), RuntimeError> {
    let object = value.as_object().ok_or_else(|| {
        RuntimeError::InvalidInput("AGENTIC_AUTHORING_CAMERA_CLAIM_INVALID".to_owned())
    })?;
    reject_authoring_keys(
        object,
        &[
            "visibility",
            "camera_hash",
            "camera_canonical_sha256",
            "claim",
            "evidence_refs",
        ],
        "camera_claim",
    )?;
    let visibility = object
        .get("visibility")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            RuntimeError::InvalidInput("AGENTIC_AUTHORING_CAMERA_VISIBILITY_REQUIRED".to_owned())
        })?;
    let camera_hash = object.get("camera_hash").ok_or_else(|| {
        RuntimeError::InvalidInput("AGENTIC_AUTHORING_CAMERA_HASH_REQUIRED".to_owned())
    })?;
    let camera_canonical = object
        .get("camera_canonical_sha256")
        .cloned()
        .unwrap_or(Value::Null);
    let camera_canonical_present = object.contains_key("camera_canonical_sha256");
    match visibility {
        "unknown" if !camera_hash.is_null() || !camera_canonical.is_null() => {
            return Err(RuntimeError::InvalidInput(
                "AGENTIC_AUTHORING_UNKNOWN_CAMERA_MUST_BE_NULL".to_owned(),
            ))
        }
        "observed" | "inferred" if !camera_hash.as_str().is_some_and(is_sha256) => {
            return Err(RuntimeError::InvalidInput(
                "AGENTIC_AUTHORING_CAMERA_HASH_INVALID".to_owned(),
            ))
        }
        "observed" | "inferred"
            if camera_canonical_present && !camera_canonical.as_str().is_some_and(is_sha256) =>
        {
            return Err(RuntimeError::InvalidInput(
                "AGENTIC_AUTHORING_CAMERA_CANONICAL_HASH_INVALID".to_owned(),
            ))
        }
        "unknown" | "observed" | "inferred" => {}
        _ => {
            return Err(RuntimeError::InvalidInput(
                "AGENTIC_AUTHORING_CAMERA_VISIBILITY_INVALID".to_owned(),
            ))
        }
    }
    let claim = object.get("claim").and_then(Value::as_str).ok_or_else(|| {
        RuntimeError::InvalidInput("AGENTIC_AUTHORING_CAMERA_CLAIM_REQUIRED".to_owned())
    })?;
    if claim.is_empty() || claim.len() > 512 || unsafe_authoring_text(claim) {
        return Err(RuntimeError::InvalidInput(
            "AGENTIC_AUTHORING_CAMERA_CLAIM_INVALID".to_owned(),
        ));
    }
    let required_sha = camera_hash.as_str().unwrap_or_else(|| "");
    if camera_hash.is_null() {
        let refs = object.get("evidence_refs").ok_or_else(|| {
            RuntimeError::InvalidInput("AGENTIC_AUTHORING_CAMERA_EVIDENCE_REQUIRED".to_owned())
        })?;
        let refs = refs.as_array().ok_or_else(|| {
            RuntimeError::InvalidInput("AGENTIC_AUTHORING_CAMERA_EVIDENCE_INVALID".to_owned())
        })?;
        if refs.is_empty() {
            return Err(RuntimeError::InvalidInput(
                "AGENTIC_AUTHORING_CAMERA_EVIDENCE_REQUIRED".to_owned(),
            ));
        }
        for reference in refs {
            let object = reference.as_object().ok_or_else(|| {
                RuntimeError::InvalidInput("AGENTIC_AUTHORING_EVIDENCE_REF_INVALID".to_owned())
            })?;
            if !object
                .get("sha256")
                .and_then(Value::as_str)
                .is_some_and(is_sha256)
            {
                return Err(RuntimeError::InvalidInput(
                    "AGENTIC_AUTHORING_EVIDENCE_HASH_INVALID".to_owned(),
                ));
            }
        }
    } else {
        validate_evidence_refs(
            object.get("evidence_refs").ok_or_else(|| {
                RuntimeError::InvalidInput("AGENTIC_AUTHORING_CAMERA_EVIDENCE_REQUIRED".to_owned())
            })?,
            required_sha,
        )?;
    }
    Ok(())
}

/// Validate the durable, per-view annotation lineage when an authoring
/// producer supplies it.  Older single-view canvases may omit these optional
/// fields; a multi-view canvas that carries them must bind the target mask and
/// the exact ReferenceViewSpec to the same imported reference.
fn validate_reference_view_annotations(
    runtime: &Runtime,
    view: &Map<String, Value>,
    reference: &ReferenceEvidenceRecord,
) -> Result<(), RuntimeError> {
    let target_value = view.get("target_sha256").cloned().unwrap_or(Value::Null);
    let mask_value = view.get("mask_sha256").cloned().unwrap_or(Value::Null);
    match (target_value.as_null(), mask_value.as_null()) {
        (Some(()), Some(())) => {}
        (Some(()), None) | (None, Some(())) => {
            return Err(RuntimeError::InvalidInput(
                "AGENTIC_AUTHORING_VIEW_MASK_TARGET_PAIR_REQUIRED".to_owned(),
            ));
        }
        (None, None) => {
            let target_sha256 = target_value
                .as_str()
                .filter(|hash| is_sha256(hash))
                .ok_or_else(|| {
                    RuntimeError::InvalidInput(
                        "AGENTIC_AUTHORING_VIEW_TARGET_HASH_INVALID".to_owned(),
                    )
                })?;
            let mask_sha256 = mask_value
                .as_str()
                .filter(|hash| is_sha256(hash))
                .ok_or_else(|| {
                    RuntimeError::InvalidInput(
                        "AGENTIC_AUTHORING_VIEW_MASK_HASH_INVALID".to_owned(),
                    )
                })?;
            let target = runtime.read_silhouette_target(target_sha256)?;
            if target.get("reference_id").and_then(Value::as_str)
                != Some(reference.reference_id.as_str())
                || target.get("reference_sha256").and_then(Value::as_str)
                    != Some(reference.object_sha256.as_str())
            {
                return Err(RuntimeError::InvalidInput(
                    "AGENTIC_AUTHORING_VIEW_TARGET_REFERENCE_BINDING_MISMATCH".to_owned(),
                ));
            }
            if target.get("mask_sha256").and_then(Value::as_str) != Some(mask_sha256) {
                return Err(RuntimeError::InvalidInput(
                    "AGENTIC_AUTHORING_VIEW_TARGET_MASK_BINDING_MISMATCH".to_owned(),
                ));
            }
            runtime.cas_read(mask_sha256).map_err(|_| {
                RuntimeError::InvalidInput(
                    "AGENTIC_AUTHORING_VIEW_MASK_OBJECT_NOT_FOUND".to_owned(),
                )
            })?;
        }
    }

    if let Some(view_spec) = view.get("view_spec") {
        if view_spec.get("view_id").and_then(Value::as_str)
            != view.get("view_id").and_then(Value::as_str)
        {
            return Err(RuntimeError::InvalidInput(
                "AGENTIC_AUTHORING_VIEW_SPEC_ID_MISMATCH".to_owned(),
            ));
        }
        let kind = view.get("kind").and_then(Value::as_str).ok_or_else(|| {
            RuntimeError::InvalidInput("AGENTIC_AUTHORING_VIEW_KIND_REQUIRED".to_owned())
        })?;
        if let Some(expected_source_view) = expected_reference_source_view(kind) {
            if view_spec.get("source_view").and_then(Value::as_str) != Some(expected_source_view) {
                return Err(RuntimeError::InvalidInput(
                    "AGENTIC_AUTHORING_VIEW_SPEC_SOURCE_VIEW_MISMATCH".to_owned(),
                ));
            }
        }
        if target_value.is_string() {
            let visible_regions = view
                .get("visible_regions")
                .and_then(Value::as_array)
                .ok_or_else(|| {
                    RuntimeError::InvalidInput("AGENTIC_AUTHORING_VIEW_REGIONS_REQUIRED".to_owned())
                })?;
            let spec_region_ids: HashSet<&str> = view_spec
                .get("regions")
                .and_then(Value::as_array)
                .ok_or_else(|| {
                    RuntimeError::InvalidInput(
                        "AGENTIC_AUTHORING_VIEW_SPEC_REGIONS_REQUIRED".to_owned(),
                    )
                })?
                .iter()
                .filter_map(|region| region.get("region_id").and_then(Value::as_str))
                .collect();
            for region in visible_regions {
                let region_id =
                    region
                        .get("region_id")
                        .and_then(Value::as_str)
                        .ok_or_else(|| {
                            RuntimeError::InvalidInput(
                                "AGENTIC_AUTHORING_VIEW_REGION_ID_REQUIRED".to_owned(),
                            )
                        })?;
                if !spec_region_ids.contains(region_id) {
                    return Err(RuntimeError::InvalidInput(
                        "AGENTIC_AUTHORING_VIEW_REGION_BINDING_MISMATCH".to_owned(),
                    ));
                }
            }
        }
        super::validate_reference_view_spec(view_spec, reference)?;
    }
    Ok(())
}

fn expected_reference_source_view(kind: &str) -> Option<&'static str> {
    match kind {
        "perspective" => Some("three-quarter"),
        "front" => Some("front"),
        "back" => Some("back"),
        "left" => Some("left"),
        "right" => Some("right"),
        "top" => Some("top"),
        "bottom" => Some("bottom"),
        "front-three-quarter" => Some("front-three-quarter"),
        "rear-three-quarter" => Some("rear-three-quarter"),
        "material" => Some("detail"),
        "detail" => Some("detail"),
        _ => None,
    }
}

fn validate_regions(value: &Value) -> Result<(), RuntimeError> {
    let regions = value.as_array().ok_or_else(|| {
        RuntimeError::InvalidInput("AGENTIC_AUTHORING_REGIONS_INVALID".to_owned())
    })?;
    if regions.len() > 128 {
        return Err(RuntimeError::InvalidInput(
            "AGENTIC_AUTHORING_REGIONS_OUT_OF_BOUNDS".to_owned(),
        ));
    }
    let mut ids = HashSet::new();
    for region in regions {
        let object = region.as_object().ok_or_else(|| {
            RuntimeError::InvalidInput("AGENTIC_AUTHORING_REGION_INVALID".to_owned())
        })?;
        reject_authoring_keys(object, &["region_id", "label", "state"], "region")?;
        let id = required_authoring_id(object, "region_id", "region")?;
        if !ids.insert(id.to_owned()) {
            return Err(RuntimeError::InvalidInput(
                "AGENTIC_AUTHORING_REGION_ID_DUPLICATE".to_owned(),
            ));
        }
        required_authoring_text(object, "label", "region")?;
        validate_state(
            object.get("state").ok_or_else(|| {
                RuntimeError::InvalidInput("AGENTIC_AUTHORING_REGION_STATE_REQUIRED".to_owned())
            })?,
            false,
        )?;
    }
    Ok(())
}

fn validate_unknown_regions(value: &Value) -> Result<(), RuntimeError> {
    let regions = value.as_array().ok_or_else(|| {
        RuntimeError::InvalidInput("AGENTIC_AUTHORING_UNKNOWN_REGIONS_INVALID".to_owned())
    })?;
    if regions.len() > 128 {
        return Err(RuntimeError::InvalidInput(
            "AGENTIC_AUTHORING_UNKNOWN_REGIONS_OUT_OF_BOUNDS".to_owned(),
        ));
    }
    let mut ids = HashSet::new();
    for region in regions {
        let object = region.as_object().ok_or_else(|| {
            RuntimeError::InvalidInput("AGENTIC_AUTHORING_UNKNOWN_REGION_INVALID".to_owned())
        })?;
        reject_authoring_keys(
            object,
            &["region_id", "question", "state"],
            "unknown_region",
        )?;
        let id = required_authoring_id(object, "region_id", "unknown_region")?;
        if !ids.insert(id.to_owned()) {
            return Err(RuntimeError::InvalidInput(
                "AGENTIC_AUTHORING_UNKNOWN_REGION_ID_DUPLICATE".to_owned(),
            ));
        }
        required_authoring_text(object, "question", "unknown_region")?;
        validate_state(
            object.get("state").ok_or_else(|| {
                RuntimeError::InvalidInput(
                    "AGENTIC_AUTHORING_UNKNOWN_REGION_STATE_REQUIRED".to_owned(),
                )
            })?,
            true,
        )?;
    }
    Ok(())
}

fn validate_state(value: &Value, unknown_only: bool) -> Result<(), RuntimeError> {
    let object = value
        .as_object()
        .ok_or_else(|| RuntimeError::InvalidInput("AGENTIC_AUTHORING_STATE_INVALID".to_owned()))?;
    reject_authoring_keys(
        object,
        &["visibility", "confidence", "evidence_refs"],
        "state",
    )?;
    let visibility = object
        .get("visibility")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            RuntimeError::InvalidInput("AGENTIC_AUTHORING_STATE_VISIBILITY_REQUIRED".to_owned())
        })?;
    if !matches!(visibility, "observed" | "inferred" | "unknown")
        || (unknown_only && visibility != "unknown")
    {
        return Err(RuntimeError::InvalidInput(
            "AGENTIC_AUTHORING_STATE_VISIBILITY_INVALID".to_owned(),
        ));
    }
    let confidence = object
        .get("confidence")
        .and_then(Value::as_f64)
        .ok_or_else(|| {
            RuntimeError::InvalidInput("AGENTIC_AUTHORING_STATE_CONFIDENCE_REQUIRED".to_owned())
        })?;
    if !confidence.is_finite()
        || !(0.0..=1.0).contains(&confidence)
        || (visibility == "unknown" && confidence != 0.0)
    {
        return Err(RuntimeError::InvalidInput(
            "AGENTIC_AUTHORING_STATE_CONFIDENCE_INVALID".to_owned(),
        ));
    }
    validate_evidence_refs_any(object.get("evidence_refs").ok_or_else(|| {
        RuntimeError::InvalidInput("AGENTIC_AUTHORING_STATE_EVIDENCE_REQUIRED".to_owned())
    })?)
}

fn validate_evidence_refs_any(value: &Value) -> Result<(), RuntimeError> {
    let refs = value.as_array().ok_or_else(|| {
        RuntimeError::InvalidInput("AGENTIC_AUTHORING_EVIDENCE_REFS_INVALID".to_owned())
    })?;
    if refs.is_empty() || refs.len() > 16 {
        return Err(RuntimeError::InvalidInput(
            "AGENTIC_AUTHORING_EVIDENCE_REFS_OUT_OF_BOUNDS".to_owned(),
        ));
    }
    for item in refs {
        let object = item.as_object().ok_or_else(|| {
            RuntimeError::InvalidInput("AGENTIC_AUTHORING_EVIDENCE_REF_INVALID".to_owned())
        })?;
        reject_authoring_keys(object, &["kind", "sha256"], "evidence_ref")?;
        if object.get("kind").and_then(Value::as_str).is_none()
            || !object
                .get("sha256")
                .and_then(Value::as_str)
                .is_some_and(is_sha256)
        {
            return Err(RuntimeError::InvalidInput(
                "AGENTIC_AUTHORING_EVIDENCE_REF_INVALID".to_owned(),
            ));
        }
    }
    Ok(())
}

fn validate_coverage(value: &Value) -> Result<(), RuntimeError> {
    let object = value.as_object().ok_or_else(|| {
        RuntimeError::InvalidInput("AGENTIC_AUTHORING_COVERAGE_INVALID".to_owned())
    })?;
    reject_authoring_keys(
        object,
        &[
            "required_views",
            "supplied_views",
            "missing_views",
            "coverage_status",
            "hq_360_status",
            "evidence_refs",
        ],
        "coverage",
    )?;
    let required = view_kind_array(object, "required_views", 5, 9)?;
    let supplied = view_kind_array(object, "supplied_views", 1, 9)?;
    let missing = view_kind_array(object, "missing_views", 0, 9)?;
    let required_set: HashSet<&str> = required.iter().map(String::as_str).collect();
    let supplied_set: HashSet<&str> = supplied.iter().map(String::as_str).collect();
    let missing_set: HashSet<&str> = missing.iter().map(String::as_str).collect();
    if !supplied_set.is_subset(&required_set)
        || !missing_set.is_subset(&required_set)
        || required_set
            .difference(&supplied_set)
            .copied()
            .collect::<HashSet<_>>()
            != missing_set
    {
        return Err(RuntimeError::InvalidInput(
            "AGENTIC_AUTHORING_COVERAGE_SET_MISMATCH".to_owned(),
        ));
    }
    let status = object
        .get("coverage_status")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            RuntimeError::InvalidInput("AGENTIC_AUTHORING_COVERAGE_STATUS_REQUIRED".to_owned())
        })?;
    let hq = object
        .get("hq_360_status")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            RuntimeError::InvalidInput("AGENTIC_AUTHORING_COVERAGE_HQ_STATUS_REQUIRED".to_owned())
        })?;
    if missing.is_empty() {
        if status != "complete" || hq != "eligible" {
            return Err(RuntimeError::InvalidInput(
                "AGENTIC_AUTHORING_COVERAGE_COMPLETE_STATUS_MISMATCH".to_owned(),
            ));
        }
        if REQUIRED_HQ_REFERENCE_VIEWS
            .iter()
            .any(|kind| !required_set.contains(kind))
        {
            return Err(RuntimeError::InvalidInput(
                "AGENTIC_AUTHORING_COVERAGE_CORE_VIEWS_REQUIRED".to_owned(),
            ));
        }
    } else if !matches!(status, "partial" | "blocked") || hq != "BLOCKED_REFERENCE_COVERAGE" {
        return Err(RuntimeError::InvalidInput(
            "AGENTIC_AUTHORING_COVERAGE_BLOCKED_STATUS_MISMATCH".to_owned(),
        ));
    }
    validate_evidence_refs_any(object.get("evidence_refs").ok_or_else(|| {
        RuntimeError::InvalidInput("AGENTIC_AUTHORING_COVERAGE_EVIDENCE_REQUIRED".to_owned())
    })?)
}

/// A coverage claim is only meaningful when every supplied view kind has an
/// authored ReferenceCanvas view.  The set-level validator above protects the
/// required/supplied/missing bookkeeping; this second gate binds that claim
/// to the actual view entities so a caller cannot mark a six-view canvas as
/// complete while submitting only one or two view objects.
fn validate_coverage_view_bindings(coverage: &Value, views: &[Value]) -> Result<(), RuntimeError> {
    let coverage_object = coverage.as_object().ok_or_else(|| {
        RuntimeError::InvalidInput("AGENTIC_AUTHORING_COVERAGE_INVALID".to_owned())
    })?;
    let supplied = coverage_object
        .get("supplied_views")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            RuntimeError::InvalidInput(
                "AGENTIC_AUTHORING_COVERAGE_SUPPLIED_VIEWS_INVALID".to_owned(),
            )
        })?;
    let supplied_kinds: HashSet<&str> = supplied
        .iter()
        .map(|kind| {
            kind.as_str().ok_or_else(|| {
                RuntimeError::InvalidInput(
                    "AGENTIC_AUTHORING_COVERAGE_SUPPLIED_VIEWS_INVALID".to_owned(),
                )
            })
        })
        .collect::<Result<HashSet<_>, _>>()?;
    let mut authored_kinds = HashSet::new();
    for view in views {
        let kind = view.get("kind").and_then(Value::as_str).ok_or_else(|| {
            RuntimeError::InvalidInput("AGENTIC_AUTHORING_COVERAGE_VIEW_KIND_INVALID".to_owned())
        })?;
        if !supplied_kinds.contains(kind) {
            return Err(RuntimeError::InvalidInput(
                "AGENTIC_AUTHORING_COVERAGE_VIEW_KIND_NOT_SUPPLIED".to_owned(),
            ));
        }
        if !authored_kinds.insert(kind) {
            return Err(RuntimeError::InvalidInput(
                "AGENTIC_AUTHORING_COVERAGE_VIEW_KIND_DUPLICATE".to_owned(),
            ));
        }
    }
    if authored_kinds != supplied_kinds {
        return Err(RuntimeError::InvalidInput(
            "AGENTIC_AUTHORING_COVERAGE_VIEW_BINDING_MISMATCH".to_owned(),
        ));
    }
    if coverage_object
        .get("coverage_status")
        .and_then(Value::as_str)
        == Some("complete")
        && coverage_object
            .get("missing_views")
            .and_then(Value::as_array)
            .is_none_or(|missing| !missing.is_empty())
    {
        return Err(RuntimeError::InvalidInput(
            "AGENTIC_AUTHORING_COVERAGE_COMPLETE_VIEW_BINDING_MISMATCH".to_owned(),
        ));
    }
    Ok(())
}

fn view_kind_array(
    object: &Map<String, Value>,
    key: &str,
    min: usize,
    max: usize,
) -> Result<Vec<String>, RuntimeError> {
    let values = object.get(key).and_then(Value::as_array).ok_or_else(|| {
        RuntimeError::InvalidInput(format!("AGENTIC_AUTHORING_COVERAGE_{key}_INVALID"))
    })?;
    if values.len() < min || values.len() > max {
        return Err(RuntimeError::InvalidInput(format!(
            "AGENTIC_AUTHORING_COVERAGE_{key}_OUT_OF_BOUNDS"
        )));
    }
    let mut result = Vec::new();
    let mut seen = HashSet::new();
    for value in values {
        let kind = value.as_str().ok_or_else(|| {
            RuntimeError::InvalidInput("AGENTIC_AUTHORING_COVERAGE_VIEW_INVALID".to_owned())
        })?;
        if !matches!(
            kind,
            "front"
                | "back"
                | "left"
                | "right"
                | "top"
                | "bottom"
                | "perspective"
                | "front-three-quarter"
                | "rear-three-quarter"
                | "material"
                | "detail"
        ) || !seen.insert(kind)
        {
            return Err(RuntimeError::InvalidInput(
                "AGENTIC_AUTHORING_COVERAGE_VIEW_INVALID".to_owned(),
            ));
        }
        result.push(kind.to_owned());
    }
    Ok(result)
}

fn validate_unknowns(value: &Value) -> Result<(), RuntimeError> {
    let values = value.as_array().ok_or_else(|| {
        RuntimeError::InvalidInput("AGENTIC_AUTHORING_UNKNOWNS_INVALID".to_owned())
    })?;
    if values.len() > 128 {
        return Err(RuntimeError::InvalidInput(
            "AGENTIC_AUTHORING_UNKNOWNS_OUT_OF_BOUNDS".to_owned(),
        ));
    }
    let mut ids = HashSet::new();
    for value in values {
        let object = value.as_object().ok_or_else(|| {
            RuntimeError::InvalidInput("AGENTIC_AUTHORING_UNKNOWN_INVALID".to_owned())
        })?;
        reject_authoring_keys(
            object,
            &["unknown_id", "scope_kind", "scope_id", "question", "state"],
            "unknown",
        )?;
        let id = required_authoring_id(object, "unknown_id", "unknown")?;
        if !ids.insert(id.to_owned()) {
            return Err(RuntimeError::InvalidInput(
                "AGENTIC_AUTHORING_UNKNOWN_ID_DUPLICATE".to_owned(),
            ));
        }
        if !matches!(
            object.get("scope_kind").and_then(Value::as_str),
            Some("scene" | "part" | "material-zone" | "camera" | "region")
        ) {
            return Err(RuntimeError::InvalidInput(
                "AGENTIC_AUTHORING_UNKNOWN_SCOPE_INVALID".to_owned(),
            ));
        }
        required_authoring_id(object, "scope_id", "unknown")?;
        required_authoring_text(object, "question", "unknown")?;
        validate_state(
            object.get("state").ok_or_else(|| {
                RuntimeError::InvalidInput("AGENTIC_AUTHORING_UNKNOWN_STATE_REQUIRED".to_owned())
            })?,
            true,
        )?;
    }
    Ok(())
}

fn validate_claims(value: &Value) -> Result<(), RuntimeError> {
    let values = value
        .as_array()
        .ok_or_else(|| RuntimeError::InvalidInput("AGENTIC_AUTHORING_CLAIMS_INVALID".to_owned()))?;
    if values.len() > 128 {
        return Err(RuntimeError::InvalidInput(
            "AGENTIC_AUTHORING_CLAIMS_OUT_OF_BOUNDS".to_owned(),
        ));
    }
    let mut ids = HashSet::new();
    for value in values {
        let object = value.as_object().ok_or_else(|| {
            RuntimeError::InvalidInput("AGENTIC_AUTHORING_CLAIM_INVALID".to_owned())
        })?;
        reject_authoring_keys(
            object,
            &[
                "claim_id",
                "subject_kind",
                "subject_id",
                "statement",
                "state",
            ],
            "claim",
        )?;
        let id = required_authoring_id(object, "claim_id", "claim")?;
        if !ids.insert(id.to_owned()) {
            return Err(RuntimeError::InvalidInput(
                "AGENTIC_AUTHORING_CLAIM_ID_DUPLICATE".to_owned(),
            ));
        }
        if !matches!(
            object.get("subject_kind").and_then(Value::as_str),
            Some("canvas" | "view" | "region" | "camera")
        ) {
            return Err(RuntimeError::InvalidInput(
                "AGENTIC_AUTHORING_CLAIM_SUBJECT_INVALID".to_owned(),
            ));
        }
        required_authoring_id(object, "subject_id", "claim")?;
        required_authoring_text(object, "statement", "claim")?;
        validate_state(
            object.get("state").ok_or_else(|| {
                RuntimeError::InvalidInput("AGENTIC_AUTHORING_CLAIM_STATE_REQUIRED".to_owned())
            })?,
            false,
        )?;
    }
    Ok(())
}

fn validate_nonempty_array(
    object: &Map<String, Value>,
    key: &str,
    max: usize,
    label: &str,
) -> Result<(), RuntimeError> {
    let values = object.get(key).and_then(Value::as_array).ok_or_else(|| {
        RuntimeError::InvalidInput(format!("AGENTIC_AUTHORING_{label}_{key}_INVALID"))
    })?;
    if values.is_empty() || values.len() > max {
        return Err(RuntimeError::InvalidInput(format!(
            "AGENTIC_AUTHORING_{label}_{key}_OUT_OF_BOUNDS"
        )));
    }
    Ok(())
}

fn validate_bounded_array(
    object: &Map<String, Value>,
    key: &str,
    max: usize,
    label: &str,
) -> Result<(), RuntimeError> {
    let values = object.get(key).and_then(Value::as_array).ok_or_else(|| {
        RuntimeError::InvalidInput(format!("AGENTIC_AUTHORING_{label}_{key}_INVALID"))
    })?;
    if values.len() > max {
        return Err(RuntimeError::InvalidInput(format!(
            "AGENTIC_AUTHORING_{label}_{key}_OUT_OF_BOUNDS"
        )));
    }
    Ok(())
}

fn validate_design_spec_states(object: &Map<String, Value>) -> Result<(), RuntimeError> {
    for (key, unknown_only) in [
        ("primary_forms", false),
        ("proportions", false),
        ("semantic_parts", false),
        ("material_language", false),
        ("risks", false),
        ("unknowns", true),
    ] {
        let values = object[key].as_array().expect("array validated above");
        for value in values {
            let child = value.as_object().ok_or_else(|| {
                RuntimeError::InvalidInput(format!(
                    "AGENTIC_AUTHORING_DESIGN_SPEC_{key}_ITEM_INVALID"
                ))
            })?;
            validate_state(
                child.get("state").ok_or_else(|| {
                    RuntimeError::InvalidInput(format!(
                        "AGENTIC_AUTHORING_DESIGN_SPEC_{key}_STATE_REQUIRED"
                    ))
                })?,
                unknown_only,
            )?;
        }
    }
    let stage_goals = object["stage_goals"]
        .as_array()
        .expect("array validated above");
    let mut stages = HashSet::new();
    for goal in stage_goals {
        let goal = goal.as_object().ok_or_else(|| {
            RuntimeError::InvalidInput("AGENTIC_AUTHORING_STAGE_GOAL_INVALID".to_owned())
        })?;
        let stage = goal.get("stage").and_then(Value::as_str).ok_or_else(|| {
            RuntimeError::InvalidInput("AGENTIC_AUTHORING_STAGE_GOAL_STAGE_REQUIRED".to_owned())
        })?;
        if !STAGES.contains(&stage) || !stages.insert(stage.to_owned()) {
            return Err(RuntimeError::InvalidInput(
                "AGENTIC_AUTHORING_STAGE_GOAL_STAGE_INVALID".to_owned(),
            ));
        }
        required_authoring_text(goal, "objective", "stage_goal")?;
        let gate = goal
            .get("exit_gate")
            .and_then(Value::as_object)
            .ok_or_else(|| {
                RuntimeError::InvalidInput("AGENTIC_AUTHORING_STAGE_GOAL_GATE_REQUIRED".to_owned())
            })?;
        if gate.get("stage").and_then(Value::as_str) != Some(stage)
            || !matches!(
                gate.get("status").and_then(Value::as_str),
                Some("pass" | "fail" | "unknown")
            )
        {
            return Err(RuntimeError::InvalidInput(
                "AGENTIC_AUTHORING_STAGE_GOAL_GATE_INVALID".to_owned(),
            ));
        }
    }
    Ok(())
}

fn reference_set_hash(pairs: &[(String, String)]) -> String {
    let mut pairs = pairs.to_vec();
    pairs.sort();
    canonical_json_hash(&Value::Array(
        pairs
            .into_iter()
            .map(|(reference_id, reference_sha256)| {
                json!({"reference_id":reference_id,"reference_sha256":reference_sha256})
            })
            .collect(),
    ))
}

fn build_reference_canvas(reference: &ReferenceEvidenceRecord, canvas_id: &str) -> Value {
    let evidence = json!({"kind":"reference","sha256":reference.object_sha256});
    json!({
        "schema_version":"ReferenceCanvas@1",
        "canvas_id":canvas_id,
        "project_id":reference.project_id,
        "reference_set_sha256":reference.object_sha256,
        "bindings":{"status":"unbound","target_sha256":null,"camera_hash":null,"camera_canonical_sha256":null,"evidence_sha256":null},
        "views":[{
            "view_id":format!("{}-perspective",canvas_id),
            "reference_id":reference.reference_id,
            "reference_sha256":reference.object_sha256,
            "kind":"perspective",
            "authorization":{"user_authorized":true,"declaration":reference.authorization.declaration,"evidence_refs":[evidence.clone()]},
            "image_dimensions":{"width":reference.width,"height":reference.height},
            "camera_claim":{"visibility":"unknown","camera_hash":null,"claim":"Camera parameters are unknown for this reference","evidence_refs":[evidence.clone()]},
            "visible_regions":[],
            "unknown_regions":[{"region_id":"unknown-view-coverage","question":"Which additional views are required for complete coverage?","state":{"visibility":"unknown","confidence":0,"evidence_refs":[evidence.clone()]}}]
        }],
        "coverage":{"required_views":["front","back","left","right","perspective","rear-three-quarter"],"supplied_views":["perspective"],"missing_views":["front","back","left","right","rear-three-quarter"],"coverage_status":"blocked","hq_360_status":"BLOCKED_REFERENCE_COVERAGE","evidence_refs":[evidence.clone()]},
        "unknowns":[{"unknown_id":"reference-coverage","scope_kind":"scene","scope_id":"scene","question":"Are front, back, side and rear-three-quarter references available?","state":{"visibility":"unknown","confidence":0,"evidence_refs":[evidence.clone()]}}],
        "claims":[],
        "canonical_sha256":"",
        "created_at":agentic_timestamp()
    })
}

fn build_design_spec(
    project_id: &str,
    spec_id: &str,
    canvas_id: &str,
    canvas_sha256: &str,
    reference: &ReferenceEvidenceRecord,
) -> Value {
    let evidence = json!({"kind":"reference","sha256":reference.object_sha256});
    let unknown_state =
        json!({"visibility":"unknown","confidence":0,"evidence_refs":[evidence.clone()]});
    let gate = json!({"stage":"reference-canvas","status":"unknown","required_checks":["reference-authorized","reference-coverage"],"failed_checks":["reference-coverage"],"evidence_hashes":[reference.object_sha256],"unlocks":["checkpoint","bounded-repair"],"locks":["tertiary-detail","uv-pbr","confirm","export","next-stage"]});
    json!({
        "schema_version":"DesignSpec@1",
        "spec_id":spec_id,
        "project_id":project_id,
        "reference_canvas_id":canvas_id,
        "reference_canvas_sha256":canvas_sha256,
        "category":"unknown category; user intent is not yet specified",
        "style":"unknown style; user intent is not yet specified",
        "primary_forms":[{"form_id":"primary-form-unknown","name":"unknown primary form","role":"other","description":"Primary form is not inferred from one reference without a bounded design action","state":unknown_state.clone()}],
        "proportions":[],
        "semantic_parts":[{"part_id":"scene","role":"root","parent_id":null,"symmetry":"unknown","material_zone_ids":[],"state":unknown_state.clone()}],
        "material_language":[],
        "stage_goals":[{"stage":"reference-canvas","objective":"Bind authorized references and record unknown coverage before primary form work","allowed_action_kinds":["reference-import","coverage-annotation","mark-unknown","checkpoint"],"forbidden_action_kinds":["tertiary-detail","uv-pbr","export"],"exit_gate":gate}],
        "risks":[{"risk_id":"risk-reference-coverage","kind":"reference-coverage","severity":"blocking","description":"Single-reference coverage is insufficient for a 360-degree quality claim","state":unknown_state.clone()}],
        "unknowns":[{"unknown_id":"unknown-design-intent","question":"What category, style, proportions and material language should the design follow?","scope_kind":"scene","scope_id":"scene","state":{"visibility":"unknown","confidence":0,"evidence_refs":[evidence]},"blocked_stages":["primary-form","secondary-structure","tertiary-detail","uv-pbr","final-review"]}],
        "canonical_sha256":"",
        "created_at":agentic_timestamp()
    })
}

fn agentic_timestamp() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let days = seconds.div_euclid(86_400);
    let day_seconds = seconds.rem_euclid(86_400);
    let hour = day_seconds / 3_600;
    let minute = (day_seconds % 3_600) / 60;
    let second = day_seconds % 60;
    let (year, month, day) = civil_from_days(days);
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

fn civil_from_days(days_since_unix_epoch: i64) -> (i64, i64, i64) {
    let z = days_since_unix_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 }.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096).div_euclid(365);
    let year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let month_part = (5 * doy + 2).div_euclid(153);
    let day = doy - (153 * month_part + 2).div_euclid(5) + 1;
    let month = month_part + if month_part < 10 { 3 } else { -9 };
    let year = year + if month <= 2 { 1 } else { 0 };
    (year, month, day)
}
