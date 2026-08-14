//! Runtime-owned bounded Agentic action execution.
//!
//! The MCP action definitions existed before the Runtime producer.  This
//! module is the narrow landing slice: a bound `primary-form` action is
//! admitted once, converted to a typed SilhouetteRig, and delegated to the
//! existing Runtime-owned Primary Form pipeline.  Codex supplies intent and
//! approval; it never receives a parameter-search loop.

use super::{
    canonical_json_bytes, canonical_json_hash, default_camera_calibration,
    Runtime, RuntimeError,
};
use super::agentic_session::validate_observation_claims;
use forgecad_contracts::{is_opaque_id, is_sha256, CandidateRecord};
use forgecad_store::AgenticActionRunRecord;
use serde_json::{json, Map, Value};

const ACTION_STAGES: [&str; 6] = [
    "reference-canvas",
    "primary-form",
    "secondary-structure",
    "tertiary-detail",
    "uv-pbr",
    "final-review",
];

const ALLOWED_ACTION_KINDS: [&str; 2] = ["bounded-repair", "primary-form-adjustment"];

impl Runtime {
    /// Execute one approved, bounded action and persist only its immutable
    /// Runtime receipt. The Primary Form implementation creates a staged
    /// candidate when its own strict comparison selects a geometry program;
    /// this method never confirms a version or exports an asset.
    pub fn design_action_run_prepare(&self, request: Value) -> Result<Value, RuntimeError> {
        let object = action_request_object(&request, "design_action_run_prepare")?;
        validate_action_request(object)?;
        let project_id = required_id(object, "project_id")?;
        let session_id = required_id(object, "session_id")?;
        let candidate_id = required_id(object, "candidate_id")?;
        let run_id = required_id(object, "run_id")?;
        let action = object
            .get("action")
            .ok_or_else(|| invalid_action("action is required"))?;
        let requested_stage = required_id(object, "requested_stage")?;
        let input_sha256 = required_sha(object, "input_sha256")?;
        let expected_input_sha256 = action_input_sha256(
            project_id,
            session_id,
            candidate_id,
            run_id,
            action,
            requested_stage,
        );
        if input_sha256 != expected_input_sha256 {
            return Err(invalid_action(
                "input_sha256 does not bind the action and design-session scope",
            ));
        }

        if let Some(existing) = self.store.get_agentic_action_run(run_id)? {
            if existing.input_sha256 != input_sha256
                || existing.session_id != session_id
                || existing.project_id != project_id
                || existing.candidate_id != candidate_id
            {
                return Err(invalid_action(
                    "run_id is already bound to another action or design scope",
                ));
            }
            return Ok(action_run_value(&existing));
        }

        let session = self
            .store
            .get_agentic_session(session_id)?
            .ok_or_else(|| invalid_action("AGENTIC_SESSION_NOT_FOUND"))?;
        if session.project_id != project_id
            || session.candidate_id != candidate_id
            || session.current_stage != requested_stage
        {
            return Err(invalid_action(
                "AGENTIC_ACTION_SCOPE_MISMATCH: session, candidate or stage differs",
            ));
        }
        let candidate = bound_candidate(self, project_id, candidate_id)?;
        let reference = bound_reference(self, project_id, &session.reference_id)?;
        let observation = self.agentic_scene_observe(project_id, Some(candidate_id))?;
        validate_observation_claims(
            &observation,
            &candidate,
            &reference,
            &session.camera_hash,
            &session.evidence_sha256,
        )?;
        let evidence = self.visual_evidence(candidate_id).map_err(|error| {
            RuntimeError::InvalidInput(format!(
                "AGENTIC_ACTION_PRECONDITION_FAILED: candidate-bound visual evidence is unavailable: {error}"
            ))
        })?;
        let target_sha256 = evidence
            .get("target_sha256")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                invalid_action(
                    "AGENTIC_ACTION_PRECONDITION_FAILED: Primary Form action requires a hash-bound silhouette target",
                )
            })?;
        let reference_id = required_id_from_value(evidence.get("reference_id"), "reference_id")?;
        if reference_id != session.reference_id || reference_id != reference.reference_id {
            return Err(invalid_action(
                "AGENTIC_REFERENCE_BINDING_MISMATCH: visual evidence reference differs",
            ));
        }
        let part_id = required_id_from_value(
            action.get("target_id"),
            "action.target_id",
        )?;
        let rig = rig_from_action(candidate_id, input_sha256, part_id, action)?;
        let base_version_id = session.current_version_id.as_deref();
        let mut primary_form_request = json!({
            "project_id": project_id,
            "candidate_id": candidate_id,
            "target_sha256": target_sha256,
            "rig": rig,
            "base_camera": default_camera_calibration(),
            "optimizer": {
                "algorithm": "coordinate_descent",
                "max_iterations": 1,
                "max_evaluations": 64,
                "step_fraction": 0.1
            },
            "base_version_id": base_version_id,
            "canonical_sha256": ""
        });
        primary_form_request["canonical_sha256"] = Value::String(canonical_json_hash(
            &primary_form_request,
        ));
        let primary_form_result = self.primary_form_repair_prepare(
            project_id,
            base_version_id,
            primary_form_request,
        )?;
        let result_object = self.put_object(
            &canonical_json_bytes(&primary_form_result)
                .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?,
            None,
            "application/json",
            "agentic-action-primary-form-result",
        )?;
        let prepared = primary_form_result.get("status").and_then(Value::as_str) == Some("prepared");
        let quality_status = if prepared {
            primary_form_result
                .get("quality_status")
                .and_then(Value::as_str)
                .unwrap_or("unknown")
                .to_owned()
        } else {
            evidence
                .pointer("/quality_report/visual_status")
                .and_then(Value::as_str)
                .unwrap_or("QUALITY_TARGET_NOT_MET")
                .to_owned()
        };
        let failed_gates = action_failed_gates(&primary_form_result, &quality_status, prepared);
        let stage_results = action_stage_results(
            &primary_form_result,
            &result_object.record.sha256,
            prepared,
        );
        let completed_stage = if prepared {
            Some("evaluate".to_owned())
        } else {
            Some("prepare".to_owned())
        };
        let status = if prepared { "completed" } else { "blocked" };
        let camera_hash = primary_form_result
            .pointer("/visual_evidence/camera_hash")
            .or_else(|| evidence.pointer("/render_set/camera_hash"))
            .and_then(Value::as_str)
            .unwrap_or(&session.camera_hash)
            .to_owned();
        let run = AgenticActionRunRecord {
            schema_version: "AgenticActionRun@1".to_owned(),
            run_id: run_id.to_owned(),
            session_id: session_id.to_owned(),
            project_id: project_id.to_owned(),
            candidate_id: candidate_id.to_owned(),
            reference_id: reference_id.to_owned(),
            reference_sha256: reference.object_sha256.clone(),
            camera_hash,
            input_sha256: input_sha256.to_owned(),
            action: action.clone(),
            requested_stage: requested_stage.to_owned(),
            status: status.to_owned(),
            completed_stage,
            stage_results,
            quality_status,
            failed_gates,
            allowed_actions: vec![
                "inspect".to_owned(),
                "retry".to_owned(),
                "bounded-repair".to_owned(),
                "checkpoint".to_owned(),
            ],
            locked_actions: vec![
                "confirm".to_owned(),
                "export".to_owned(),
                "next-stage".to_owned(),
            ],
            checkpoint_id: None,
            checkpoint_sha256: None,
            immutable: true,
            runtime_write: false,
            persistent_user_data_touched: false,
            object_sha256: Some("0".repeat(64)),
            canonical_sha256: "0".repeat(64),
            created_at: action_timestamp(),
        };
        let mut run = run;
        run.canonical_sha256 = forgecad_store::agentic_action_run_canonical_sha256(&run)?;
        let payload = forgecad_store::agentic_action_run_payload_bytes(&run)?;
        let object = self.put_object(
            &payload,
            None,
            "application/json",
            "agentic-action-run",
        )?;
        run.object_sha256 = Some(object.record.sha256.clone());
        let stored = self.store.agentic_action_run_create_or_resume(&run, &object.record)?;
        Ok(action_run_value(&stored))
    }

    pub fn design_action_run_get(&self, request: Value) -> Result<Value, RuntimeError> {
        let object = action_request_object(&request, "design_action_run_get")?;
        reject_action_keys(object, &["project_id", "session_id", "candidate_id", "run_id"])?;
        let project_id = required_id(object, "project_id")?;
        let session_id = required_id(object, "session_id")?;
        let candidate_id = required_id(object, "candidate_id")?;
        let run_id = required_id(object, "run_id")?;
        let run = self
            .store
            .get_agentic_action_run(run_id)?
            .ok_or_else(|| invalid_action("NOT_FOUND: DesignActionRun not found"))?;
        if run.project_id != project_id
            || run.session_id != session_id
            || run.candidate_id != candidate_id
        {
            return Err(invalid_action(
                "AGENTIC_ACTION_SCOPE_MISMATCH: action run is outside the requested session",
            ));
        }
        Ok(action_run_value(&run))
    }
}

fn action_request_object<'a>(request: &'a Value, operation: &str) -> Result<&'a Map<String, Value>, RuntimeError> {
    request.as_object().ok_or_else(|| {
        invalid_action(&format!("{operation} requires an object"))
    })
}

fn reject_action_keys(object: &Map<String, Value>, allowed: &[&str]) -> Result<(), RuntimeError> {
    if let Some(key) = object.keys().find(|key| !allowed.contains(&key.as_str())) {
        return Err(invalid_action(&format!("unsupported field {key}")));
    }
    Ok(())
}

fn validate_action_request(object: &Map<String, Value>) -> Result<(), RuntimeError> {
    reject_action_keys(
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
        ],
    )?;
    for key in [
        "project_id",
        "session_id",
        "candidate_id",
        "run_id",
        "input_sha256",
        "requested_stage",
        "approval_receipt_id",
        "approval_summary",
        "idempotency_key",
    ] {
        if object.get(key).is_none() {
            return Err(invalid_action(&format!("{key} is required")));
        }
    }
    if object.get("approved") != Some(&Value::Bool(true)) {
        return Err(invalid_action("AGENTIC_APPROVAL_REQUIRED: approved=true is required"));
    }
    if object
        .get("approval_session_id")
        .and_then(Value::as_str)
        .is_some_and(|session| session != object.get("session_id").and_then(Value::as_str).unwrap_or_default())
    {
        return Err(invalid_action(
            "AGENTIC_SCOPE_MISMATCH: approval_session_id differs",
        ));
    }
    let requested_stage = required_id(object, "requested_stage")?;
    if !ACTION_STAGES.contains(&requested_stage) || requested_stage != "primary-form" {
        return Err(invalid_action(
            "AGENTIC_ACTION_STAGE_UNSUPPORTED: only primary-form is Runtime-executable in this slice",
        ));
    }
    let action = object
        .get("action")
        .and_then(Value::as_object)
        .ok_or_else(|| invalid_action("action must be an object"))?;
    reject_action_keys(
        action,
        &[
            "action_id",
            "action_kind",
            "scope_kind",
            "target_id",
            "operator_id",
            "parameter_changes",
            "bounded",
            "description",
        ],
    )?;
    let action_kind = required_id(action, "action_kind")?;
    if !ALLOWED_ACTION_KINDS.contains(&action_kind) {
        return Err(invalid_action(
            "AGENTIC_ACTION_NOT_BOUNDED: action kind is not executable",
        ));
    }
    if action.get("scope_kind").and_then(Value::as_str) != Some("part")
        || action.get("bounded") != Some(&Value::Bool(true))
    {
        return Err(invalid_action(
            "AGENTIC_ACTION_SCOPE_MISMATCH: Primary Form action must be bounded to one Part",
        ));
    }
    required_id_from_value(action.get("target_id"), "action.target_id")?;
    let changes = action
        .get("parameter_changes")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid_action("action.parameter_changes must be an array"))?;
    if changes.is_empty() || changes.len() > 8 {
        return Err(invalid_action(
            "AGENTIC_ACTION_NOT_BOUNDED: one Part action requires 1..8 parameter changes",
        ));
    }
    if action
        .get("description")
        .and_then(Value::as_str)
        .is_none_or(|description| description.is_empty() || description.len() > 512)
    {
        return Err(invalid_action("action.description is required and bounded"));
    }
    Ok(())
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
        .ok_or_else(|| invalid_action("action.parameter_changes is required"))?;
    let mut parameters = Vec::with_capacity(changes.len());
    for change in changes {
        let object = change
            .as_object()
            .ok_or_else(|| invalid_action("parameter change must be an object"))?;
        let parameter_id = required_id(object, "parameter_id")?;
        let semantic = parameter_semantic(parameter_id).ok_or_else(|| {
            invalid_action("AGENTIC_ACTION_PARAMETER_UNSUPPORTED: parameter semantic is not typed")
        })?;
        let unit = object
            .get("unit")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid_action("parameter change unit is required"))?;
        if !matches!(unit, "meter" | "ratio") {
            return Err(invalid_action(
                "AGENTIC_ACTION_PARAMETER_UNSUPPORTED: Primary Form accepts meter/ratio only",
            ));
        }
        let before = finite_number(object, "before")?;
        let after = finite_number(object, "after")?;
        let minimum = finite_number(object, "minimum")?;
        let maximum = finite_number(object, "maximum")?;
        if minimum >= maximum || before < minimum || before > maximum || after < minimum || after > maximum {
            return Err(invalid_action(
                "AGENTIC_ACTION_NOT_BOUNDED: parameter change exceeds its declared bounds",
            ));
        }
        let span = (maximum - minimum).abs();
        let delta = (after - before).abs();
        let step = delta.max((span / 20.0).max(0.0001));
        parameters.push(json!({
            "parameter_id": parameter_id,
            "part_id": part_id,
            "semantic": semantic,
            "value": after,
            "min": minimum,
            "max": maximum,
            "step": step,
            "unit": unit
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

fn parameter_semantic(parameter_id: &str) -> Option<&'static str> {
    [
        ("offset_x", "offset_x"),
        ("offset_y", "offset_y"),
        ("offset_z", "offset_z"),
        ("width", "width"),
        ("height", "height"),
        ("depth", "depth"),
        ("scale", "scale"),
    ]
    .into_iter()
    .find_map(|(suffix, semantic)| parameter_id.ends_with(suffix).then_some(semantic))
}

fn finite_number(object: &Map<String, Value>, key: &str) -> Result<f64, RuntimeError> {
    let value = object
        .get(key)
        .and_then(Value::as_f64)
        .ok_or_else(|| invalid_action(&format!("parameter change {key} must be a number")))?;
    if !value.is_finite() || !(-1000.0..=1000.0).contains(&value) {
        return Err(invalid_action(&format!("parameter change {key} is out of bounds")));
    }
    Ok(value)
}

fn action_stage_results(result: &Value, result_sha256: &str, prepared: bool) -> Value {
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
    let comparison_hash = result
        .pointer("/visual_evidence/comparison_report_hash")
        .and_then(Value::as_str);
    let mut stages = Map::new();
    stages.insert(
        "prepare".to_owned(),
        json!({"status":"completed","output_sha256":result_sha256}),
    );
    if prepared {
        let mut compile = Map::new();
        compile.insert("status".to_owned(), Value::String("completed".to_owned()));
        if let Some(hash) = artifact_sha256 {
            compile.insert("output_sha256".to_owned(), Value::String(hash.to_owned()));
        }
        stages.insert("compile".to_owned(), Value::Object(compile.clone()));
        stages.insert("readback".to_owned(), Value::Object(compile));

        let mut render = Map::new();
        render.insert("status".to_owned(), Value::String("completed".to_owned()));
        if let Some(hash) = render_set_hash {
            render.insert("output_sha256".to_owned(), Value::String(hash.to_owned()));
        }
        stages.insert("render".to_owned(), Value::Object(render));

        let mut evaluate = Map::new();
        evaluate.insert("status".to_owned(), Value::String("completed".to_owned()));
        if let Some(hash) = quality_hash {
            evaluate.insert("output_sha256".to_owned(), Value::String(hash.to_owned()));
        }
        if let Some(hash) = comparison_hash {
            evaluate.insert("summary_sha256".to_owned(), Value::String(hash.to_owned()));
        }
        stages.insert("evaluate".to_owned(), Value::Object(evaluate));
    } else {
        stages.insert(
            "compile".to_owned(),
            json!({"status":"blocked","error_code":"QUALITY_TARGET_NOT_MET"}),
        );
        stages.insert("readback".to_owned(), json!({"status":"skipped"}));
        stages.insert("render".to_owned(), json!({"status":"skipped"}));
        stages.insert("evaluate".to_owned(), json!({"status":"skipped"}));
    }
    Value::Object(stages)
}

fn action_failed_gates(_result: &Value, quality_status: &str, prepared: bool) -> Vec<String> {
    if !prepared {
        return vec!["prepare".to_owned(), "primary-silhouette".to_owned()];
    }
    if quality_status == "PARTIAL_VISIBLE_VIEW_PASS" {
        Vec::new()
    } else {
        vec!["visible-view".to_owned()]
    }
}

fn action_input_sha256(
    project_id: &str,
    session_id: &str,
    candidate_id: &str,
    run_id: &str,
    action: &Value,
    requested_stage: &str,
) -> String {
    canonical_json_hash(&json!({
        "project_id":project_id,
        "session_id":session_id,
        "candidate_id":candidate_id,
        "run_id":run_id,
        "action":action,
        "requested_stage":requested_stage
    }))
}

fn action_run_value(run: &AgenticActionRunRecord) -> Value {
    json!({
        "schema_version":"DesignActionRun@1",
        "run_id":run.run_id,
        "session_id":run.session_id,
        "project_id":run.project_id,
        "candidate_id":run.candidate_id,
        "reference_id":run.reference_id,
        "reference_sha256":run.reference_sha256,
        "camera_hash":run.camera_hash,
        "input_sha256":run.input_sha256,
        "action":run.action,
        "requested_stage":run.requested_stage,
        "status":run.status,
        "completed_stage":run.completed_stage,
        "stage_results":run.stage_results,
        "quality_status":run.quality_status,
        "failed_gates":run.failed_gates,
        "allowed_actions":run.allowed_actions,
        "locked_actions":run.locked_actions,
        "checkpoint_id":run.checkpoint_id,
        "checkpoint_hash":run.checkpoint_sha256,
        "runtime_write":run.runtime_write,
        "persistent_user_data_touched":run.persistent_user_data_touched,
        "canonical_sha256":run.canonical_sha256
    })
}

fn required_id<'a>(object: &'a Map<String, Value>, key: &str) -> Result<&'a str, RuntimeError> {
    let value = object
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| invalid_action(&format!("{key} is required")))?;
    if !is_opaque_id(value) {
        return Err(invalid_action(&format!("{key} is malformed")));
    }
    Ok(value)
}

fn required_sha<'a>(object: &'a Map<String, Value>, key: &str) -> Result<&'a str, RuntimeError> {
    let value = object
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| invalid_action(&format!("{key} is required")))?;
    if !is_sha256(value) {
        return Err(invalid_action(&format!("{key} must be SHA-256")));
    }
    Ok(value)
}

fn required_id_from_value<'a>(value: Option<&'a Value>, key: &str) -> Result<&'a str, RuntimeError> {
    let value = value
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| invalid_action(&format!("{key} is required")))?;
    if !is_opaque_id(value) {
        return Err(invalid_action(&format!("{key} is malformed")));
    }
    Ok(value)
}

fn bound_candidate(
    runtime: &Runtime,
    project_id: &str,
    candidate_id: &str,
) -> Result<CandidateRecord, RuntimeError> {
    let candidate = runtime
        .candidate(candidate_id)?
        .ok_or_else(|| invalid_action("NOT_FOUND: candidate not found"))?;
    if candidate.project_id != project_id {
        return Err(invalid_action("PROJECT_SCOPE_DENIED: candidate is outside project"));
    }
    Ok(candidate)
}

fn bound_reference(
    runtime: &Runtime,
    project_id: &str,
    reference_id: &str,
) -> Result<forgecad_contracts::ReferenceEvidenceRecord, RuntimeError> {
    let reference = runtime
        .reference(reference_id)?
        .ok_or_else(|| invalid_action("NOT_FOUND: reference not found"))?;
    if reference.project_id != project_id {
        return Err(invalid_action("PROJECT_SCOPE_DENIED: reference is outside project"));
    }
    Ok(reference)
}

fn invalid_action(message: &str) -> RuntimeError {
    RuntimeError::InvalidInput(format!("AGENTIC_ACTION_INVALID: {message}"))
}

fn action_timestamp() -> String {
    super::now_string()
}
