//! Durable Agentic Design Runtime session and checkpoint operations.
//!
//! This module is deliberately narrower than the observation projection.  It
//! materializes only Runtime-owned, hash-bound metadata and delegates every
//! SQLite/CAS mutation to the Store.  It never confirms a candidate, creates a
//! version, or treats a failed visual gate as a pass.

use super::{canonical_json_hash, Runtime, RuntimeError};
use forgecad_contracts::{is_opaque_id, is_sha256, CandidateRecord, ReferenceEvidenceRecord};
use forgecad_store::{
    AgenticCheckpointRecord, AgenticSessionRecord,
};
use serde_json::{json, Map, Value};
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

impl Runtime {
    /// Create or resume one durable DesignSession.  A new session is bound to
    /// the current Runtime projection hash; callers cannot inject an
    /// unverified evidence claim into the durable state.
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
            )?;
            if existing.design_spec_id != design_spec_id
                || existing.reference_canvas_id != reference_canvas_id
            {
                return Err(RuntimeError::InvalidInput(
                    "AGENTIC_SESSION_BINDING_MISMATCH: durable DesignSpec/ReferenceCanvas IDs differ"
                        .to_owned(),
                ));
            }
            return Ok(session_result(&existing, "resumed"));
        }

        let canvas = build_reference_canvas(&reference, reference_canvas_id);
        let canvas = canonical_value(canvas);
        let canvas_bytes = canonical_json_bytes(&canvas)?;
        let canvas_object = self.put_object(
            &canvas_bytes,
            None,
            "application/json",
            "agentic-reference-canvas",
        )?;

        let spec = build_design_spec(
            project_id,
            design_spec_id,
            reference_canvas_id,
            &canvas_object.record.sha256,
            &reference,
        );
        let spec = canonical_value(spec);
        let spec_bytes = canonical_json_bytes(&spec)?;
        let spec_object = self.put_object(
            &spec_bytes,
            None,
            "application/json",
            "agentic-design-spec",
        )?;

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
        Ok(session_result(&stored, "created"))
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
        validate_session_binding(
            &session,
            session_id,
            project_id,
            candidate_id,
            &session.reference_id,
            &session.camera_hash,
            &session.evidence_sha256,
        )?;
        Ok(session_result(&session, "read"))
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
        )?;
        let candidate = bound_candidate(self, project_id, candidate_id)?;
        let reference = bound_reference(self, project_id, &session.reference_id)?;
        let observation = self.agentic_scene_observe(project_id, Some(candidate_id))?;
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
                "AGENTIC_SESSION_STALE: checkpoint evidence differs from the session"
                    .to_owned(),
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
                "AGENTIC_CANDIDATE_STATE_MISMATCH: candidate state hash differs"
                    .to_owned(),
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
            })),
            evidence_sha256: requested_evidence.to_owned(),
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
                .map(|values| values.iter().filter_map(Value::as_str).map(str::to_owned).collect())
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
        next_session.checkpoint_ids.push(checkpoint.checkpoint_id.clone());
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
            &next_session.evidence_sha256,
            &next_session.session_id,
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
        Ok(checkpoint_result(&stored_checkpoint, &stored_session, "prepared"))
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
            .ok_or_else(|| RuntimeError::InvalidInput("NOT_FOUND: checkpoint not found".to_owned()))?;
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
        )?;
        let checkpoint = self
            .store
            .get_agentic_checkpoint(checkpoint_id)?
            .ok_or_else(|| RuntimeError::InvalidInput("AGENTIC_CHECKPOINT_NOT_FOUND".to_owned()))?;
        if checkpoint.canonical_sha256 != checkpoint_sha256
            || checkpoint.session_id != session_id
            || checkpoint.project_id != project_id
            || checkpoint.candidate_id != candidate_id
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
            Some(session) => session_result(&session, "lookup"),
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

fn request_object<'a>(request: &'a Value, operation: &str) -> Result<&'a Map<String, Value>, RuntimeError> {
    request.as_object().ok_or_else(|| {
        RuntimeError::InvalidInput(format!("AGENTIC_INVALID_INPUT: {operation} requires an object"))
    })
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
    let value = object.get(key).and_then(Value::as_str).filter(|value| !value.is_empty());
    let value = value.ok_or_else(|| RuntimeError::InvalidInput(format!("{key} is required")))?;
    if !is_opaque_id(value) {
        return Err(RuntimeError::InvalidInput(format!("{key} is malformed")));
    }
    Ok(value)
}

fn required_sha<'a>(object: &'a Map<String, Value>, key: &str) -> Result<&'a str, RuntimeError> {
    let value = object.get(key).and_then(Value::as_str).filter(|value| !value.is_empty());
    let value = value.ok_or_else(|| RuntimeError::InvalidInput(format!("{key} is required")))?;
    if !is_sha256(value) {
        return Err(RuntimeError::InvalidInput(format!("{key} is not a SHA-256")));
    }
    Ok(value)
}

fn required_stage<'a>(object: &'a Map<String, Value>, key: &str) -> Result<&'a str, RuntimeError> {
    let value = required_id(object, key)?;
    if !STAGES.contains(&value) {
        return Err(RuntimeError::InvalidInput(format!("{key} is not a valid DesignStage")));
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
        let value = object.get(key).and_then(Value::as_str).filter(|value| !value.is_empty());
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
    if summary.len() > 512 || summary.contains('/') || summary.contains('\\') || summary.contains("http") {
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
        || observation.get("candidate_id").and_then(Value::as_str) != Some(candidate.candidate_id.as_str())
    {
        return Err(RuntimeError::InvalidInput(
            "AGENTIC_OBSERVATION_BINDING_MISMATCH: observation scope differs".to_owned(),
        ));
    }
    let known = observation_hashes(observation, candidate, reference);
    if !known.iter().any(|hash| hash == evidence_sha256) {
        return Err(RuntimeError::InvalidInput(
            "AGENTIC_EVIDENCE_BINDING_MISMATCH: evidence hash is not Runtime-owned"
                .to_owned(),
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
) -> Result<AgenticSessionRecord, RuntimeError> {
    let stage_plan = observation
        .get("design_stage_plan")
        .ok_or_else(|| RuntimeError::InvalidInput("AGENTIC_PROJECTION_INVALID: stage plan missing".to_owned()))?;
    let stage = stage_plan
        .get("current_stage")
        .and_then(Value::as_str)
        .filter(|stage| STAGES.contains(stage))
        .unwrap_or("reference-canvas");
    let quality_status = observation
        .pointer("/quality/visual_status")
        .and_then(Value::as_str)
        .filter(|value| matches!(*value, "PARTIAL_VISIBLE_VIEW_PASS" | "QUALITY_TARGET_NOT_MET" | "BLOCKED_REFERENCE_COVERAGE"))
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
        current_version_id: None,
        current_version_sha256: None,
        current_stage: stage.to_owned(),
        quality_status: quality_status.to_owned(),
        status: "active".to_owned(),
        stage_gate: gate,
        next_actions: next_actions(stage, quality_status, evidence_sha256, session_id),
        rollback: no_session_rollback(),
        current_checkpoint_id: None,
        current_checkpoint_sha256: None,
        checkpoint_ids: Vec::new(),
        lineage: observation.get("lineage").cloned().unwrap_or_else(|| json!({
            "project_id":candidate.project_id,
            "candidate_id":candidate.candidate_id,
            "reference_id":reference.reference_id,
            "reference_sha256":reference.object_sha256
        })),
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
    let status = if strict == "passed" { "pass" } else { "unknown" };
    let required_checks = match stage {
        "reference-canvas" => vec!["reference-authorized", "reference-coverage"],
        "primary-form" => vec!["primary-silhouette", "primary-proportion", "visible-view"],
        "secondary-structure" => vec!["secondary-structure", "visible-view"],
        "tertiary-detail" => vec!["tertiary-detail", "visible-view"],
        "uv-pbr" => vec!["uv-tangent-pbr", "visible-view"],
        _ => vec!["multi-view-compare", "codex-typed-review", "human-review", "export-restart-hash"],
    };
    let failed_checks = if status == "pass" { Vec::new() } else { vec!["visible-view"] };
    let locks = if status == "pass" {
        Vec::new()
    } else {
        vec!["tertiary-detail", "uv-pbr", "confirm", "export", "next-stage"]
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
        gate["locks"] = json!(["tertiary-detail", "uv-pbr", "confirm", "export", "next-stage"]);
    }
    gate
}

fn next_actions(stage: &str, quality_status: &str, evidence_sha256: &str, session_id: &str) -> Vec<Value> {
    let action_kind = if quality_status == "QUALITY_TARGET_NOT_MET" {
        "bounded-repair"
    } else if quality_status == "not-run" {
        "request-reference"
    } else {
        "checkpoint"
    };
    vec![json!({
        "action_id":format!("{}-{}", action_kind, &canonical_json_hash(&json!({"stage":stage,"session_id":session_id}))[..16]),
        "stage":stage,
        "action_kind":action_kind,
        "scope_kind":"session",
        "target_id":session_id,
        "evidence_sha256":evidence_sha256,
        "bounded":true,
        "description":if action_kind == "bounded-repair" {"Prepare one bounded repair and rerun compile/readback/render/compare"} else if action_kind == "request-reference" {"Request or annotate missing reference coverage before advancing"} else {"Persist a checkpoint before the next bounded action"}
    })]
}

fn checkpoint_actions(gate: &Value, allowed: bool) -> Vec<String> {
    let key = if allowed { "unlocks" } else { "locks" };
    gate.get(key)
        .and_then(Value::as_array)
        .map(|values| values.iter().filter_map(Value::as_str).map(str::to_owned).collect())
        .unwrap_or_default()
}

fn observation_strings(observation: &Value, key: &str) -> Vec<String> {
    observation
        .pointer(&format!("/model_understanding_bundle/uncertainty/{key}"))
        .and_then(Value::as_array)
        .map(|values| values.iter().filter_map(Value::as_str).map(str::to_owned).collect())
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
) -> Result<(), RuntimeError> {
    if session.session_id != session_id
        || session.project_id != project_id
        || session.candidate_id != candidate_id
        || session.reference_id != reference_id
        || session.camera_hash != camera_hash
        || session.evidence_sha256 != evidence_sha256
    {
        return Err(RuntimeError::InvalidInput(
            "AGENTIC_SESSION_BINDING_MISMATCH: session scope or evidence differs".to_owned(),
        ));
    }
    Ok(())
}

fn with_session_canonical(mut session: AgenticSessionRecord) -> Result<AgenticSessionRecord, RuntimeError> {
    let mut value = serde_json::to_value(&session)
        .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?;
    value["object_sha256"] = Value::Null;
    value["canonical_sha256"] = Value::String(String::new());
    session.canonical_sha256 = canonical_json_hash(&value);
    Ok(session)
}

fn with_checkpoint_canonical(mut checkpoint: AgenticCheckpointRecord) -> Result<AgenticCheckpointRecord, RuntimeError> {
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
    super::canonical_json_bytes(value).map_err(|error| RuntimeError::InvalidInput(error.to_string()))
}

fn build_reference_canvas(reference: &ReferenceEvidenceRecord, canvas_id: &str) -> Value {
    let evidence = json!({"kind":"reference","sha256":reference.object_sha256});
    json!({
        "schema_version":"ReferenceCanvas@1",
        "canvas_id":canvas_id,
        "project_id":reference.project_id,
        "reference_set_sha256":reference.object_sha256,
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
    let unknown_state = json!({"visibility":"unknown","confidence":0,"evidence_refs":[evidence.clone()]});
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
