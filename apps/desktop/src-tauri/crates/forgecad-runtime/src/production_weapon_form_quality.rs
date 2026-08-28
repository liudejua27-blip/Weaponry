//! Runtime-owned FPS weapon form-quality composition.
//!
//! This gate consumes an already-produced, immutable CrossViewEvidenceBundle.
//! It deliberately does not render, fit a camera, compile geometry, or infer
//! artistic evidence.  The only durable objects created here are the bounded
//! report and receipt after every source binding has been independently read
//! back.

use super::{
    canonical_json_bytes, canonical_json_hash, is_opaque_id, is_sha256, sha256_hex, CasObject,
    Runtime, RuntimeError,
};
use forgecad_contracts::{
    ProductionCameraLockRecord, ProductionStageHeadV3Record, ProductionStageTransitionV3Record,
    ProductionWeaponFormEvidenceRecord, ProductionWeaponFormQualityEvidenceBinding,
    ProductionWeaponFormQualityFormGate, ProductionWeaponFormQualityGetRequest,
    ProductionWeaponFormQualityHardGate, ProductionWeaponFormQualityLineFlowEvidence,
    ProductionWeaponFormQualityNegativeSpaceEvidence, ProductionWeaponFormQualityNoRegression,
    ProductionWeaponFormQualityPartIdEvidence, ProductionWeaponFormQualityPrepareRequest,
    ProductionWeaponFormQualityRecord, ProductionWeaponFormQualityViewRecord,
    PRODUCTION_CAMERA_LOCK_CAMERA_VIEW_KINDS, PRODUCTION_CAMERA_LOCK_REFERENCE_VIEW_KINDS,
    PRODUCTION_CAMERA_LOCK_SCHEMA_VERSION, PRODUCTION_WEAPON_FORM_EVIDENCE_QUALITY_STATUS,
    PRODUCTION_WEAPON_FORM_EVIDENCE_SCHEMA_VERSION,
    PRODUCTION_WEAPON_FORM_QUALITY_FIXED_CAMERA_VIEW_KINDS,
    PRODUCTION_WEAPON_FORM_QUALITY_FORM_STAGES,
    PRODUCTION_WEAPON_FORM_QUALITY_GET_REQUEST_SCHEMA_VERSION,
    PRODUCTION_WEAPON_FORM_QUALITY_GET_RESULT_SCHEMA_VERSION,
    PRODUCTION_WEAPON_FORM_QUALITY_POLICY,
    PRODUCTION_WEAPON_FORM_QUALITY_PREPARE_REQUEST_SCHEMA_VERSION,
    PRODUCTION_WEAPON_FORM_QUALITY_PREPARE_RESULT_SCHEMA_VERSION,
    PRODUCTION_WEAPON_FORM_QUALITY_REVIEWED_REFERENCE_VIEW_KINDS,
    PRODUCTION_WEAPON_FORM_QUALITY_SCHEMA_VERSION, PRODUCTION_WEAPON_FORM_QUALITY_SOURCE_STAGES,
    PRODUCTION_WEAPON_FORM_QUALITY_TARGET_STAGES, PRODUCTION_WEAPON_FORM_QUALITY_THRESHOLD_POLICY,
};
use forgecad_store::CasReservation;
use serde_json::{Map, Value};
use std::collections::BTreeSet;

const RECEIPT_KIND: &str = "production-weapon-form-quality-receipt";
const JSON_MIME: &str = "application/json";
const MAX_JSON_BYTES: usize = 1024 * 1024;

const PREPARE_FIELDS: &[&str] = &[
    "schema_version",
    "form_quality_id",
    "session_id",
    "project_id",
    "form_stage",
    "source_stage",
    "target_stage",
    "camera_calibrated_head_transition_id",
    "camera_calibrated_head_transition_sha256",
    "camera_calibrated_head_canonical_sha256",
    "camera_calibrated_head_candidate_id",
    "camera_calibrated_head_candidate_state_sha256",
    "camera_calibrated_head_artifact_id",
    "camera_calibrated_head_artifact_sha256",
    "camera_calibrated_head_stage",
    "candidate_id",
    "candidate_state_sha256",
    "artifact_id",
    "artifact_sha256",
    "reference_id",
    "reference_sha256",
    "reference_canvas_object_sha256",
    "reference_canvas_canonical_sha256",
    "design_spec_object_sha256",
    "design_spec_canonical_sha256",
    "camera_lock_id",
    "camera_lock_canonical_sha256",
    "camera_rig_object_sha256",
    "camera_rig_canonical_sha256",
    "camera_lock_receipt_object_sha256",
    "camera_lock_source_transition_id",
    "camera_lock_source_transition_sha256",
    "camera_lock_source_head_canonical_sha256",
    "reviewed_reference_view_kinds",
    "fixed_camera_view_kinds",
    "cross_view_evidence_object_sha256",
    "cross_view_evidence_canonical_sha256",
    "cross_view_evidence_view_kinds",
    "form_evidence_object_sha256",
    "form_evidence_canonical_sha256",
    "form_view_evaluations",
    "previous_form_quality_id",
    "previous_form_quality_report_object_sha256",
    "previous_form_quality_canonical_sha256",
    "form_quality_policy",
    "form_quality_policy_sha256",
    "threshold_policy",
    "threshold_policy_sha256",
    "input_sha256",
    "idempotency_key",
];
const GET_FIELDS: &[&str] = &[
    "schema_version",
    "form_quality_id",
    "session_id",
    "project_id",
    "candidate_id",
    "form_stage",
];

fn invalid(message: impl Into<String>) -> RuntimeError {
    RuntimeError::InvalidInput(message.into())
}

fn exact_object<'a>(
    value: &'a Value,
    fields: &[&str],
    context: &str,
) -> Result<&'a Map<String, Value>, RuntimeError> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid(format!("{context} must be an object")))?;
    if let Some(key) = object.keys().find(|key| !fields.contains(&key.as_str())) {
        return Err(invalid(format!(
            "{context} contains unsupported field {key}"
        )));
    }
    for field in fields {
        if !object.contains_key(*field) {
            return Err(invalid(format!("{context} missing {field}")));
        }
    }
    Ok(object)
}

fn required_text<'a>(object: &'a Map<String, Value>, field: &str) -> Result<&'a str, RuntimeError> {
    object
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| invalid(format!("{field} must be non-empty text")))
}

fn required_id<'a>(object: &'a Map<String, Value>, field: &str) -> Result<&'a str, RuntimeError> {
    let value = required_text(object, field)?;
    if !is_opaque_id(value) {
        return Err(invalid(format!("{field} is not an opaque id")));
    }
    Ok(value)
}

fn required_sha<'a>(object: &'a Map<String, Value>, field: &str) -> Result<&'a str, RuntimeError> {
    let value = required_text(object, field)?;
    if !is_sha256(value) {
        return Err(invalid(format!("{field} is not a SHA-256")));
    }
    Ok(value)
}

fn json_object(runtime: &Runtime, hash: &str, label: &str) -> Result<Value, RuntimeError> {
    let bytes = runtime.cas_read(hash)?;
    if bytes.len() > MAX_JSON_BYTES {
        return Err(invalid(format!("{label} exceeds 1 MiB")));
    }
    serde_json::from_slice(&bytes)
        .map_err(|error| invalid(format!("{label} JSON is invalid: {error}")))
}

fn canonical_document(value: &Value, schema: &str, label: &str) -> Result<String, RuntimeError> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid(format!("{label} is not an object")))?;
    if object.get("schema_version").and_then(Value::as_str) != Some(schema) {
        return Err(invalid(format!("{label} schema differs")));
    }
    let mut normalized = value.clone();
    normalized["canonical_sha256"] = Value::String(String::new());
    let hash = canonical_json_hash(&normalized);
    if object.get("canonical_sha256").and_then(Value::as_str) != Some(hash.as_str()) {
        return Err(invalid(format!("{label} canonical hash differs")));
    }
    Ok(hash)
}

fn parse_prepare(
    value: &Value,
) -> Result<(ProductionWeaponFormQualityPrepareRequest, String), RuntimeError> {
    let object = exact_object(
        value,
        PREPARE_FIELDS,
        "ProductionWeaponFormQualityPrepareRequest@1",
    )?;
    if required_text(object, "schema_version")?
        != PRODUCTION_WEAPON_FORM_QUALITY_PREPARE_REQUEST_SCHEMA_VERSION
    {
        return Err(invalid("prepare schema version differs"));
    }
    let request: ProductionWeaponFormQualityPrepareRequest = serde_json::from_value(value.clone())
        .map_err(|error| invalid(format!("prepare request is malformed: {error}")))?;
    for field in [
        "camera_calibrated_head_transition_sha256",
        "camera_calibrated_head_canonical_sha256",
        "camera_calibrated_head_candidate_state_sha256",
        "camera_calibrated_head_artifact_sha256",
        "candidate_state_sha256",
        "artifact_sha256",
        "reference_sha256",
        "reference_canvas_object_sha256",
        "reference_canvas_canonical_sha256",
        "design_spec_object_sha256",
        "design_spec_canonical_sha256",
        "camera_lock_canonical_sha256",
        "camera_rig_object_sha256",
        "camera_rig_canonical_sha256",
        "camera_lock_receipt_object_sha256",
        "camera_lock_source_transition_sha256",
        "camera_lock_source_head_canonical_sha256",
        "cross_view_evidence_object_sha256",
        "cross_view_evidence_canonical_sha256",
        "form_evidence_object_sha256",
        "form_evidence_canonical_sha256",
        "form_quality_policy_sha256",
        "threshold_policy_sha256",
        "input_sha256",
    ] {
        required_sha(object, field)?;
    }
    for field in [
        "form_quality_id",
        "session_id",
        "project_id",
        "camera_calibrated_head_transition_id",
        "camera_calibrated_head_candidate_id",
        "camera_calibrated_head_artifact_id",
        "candidate_id",
        "artifact_id",
        "reference_id",
        "camera_lock_id",
        "camera_lock_source_transition_id",
    ] {
        required_id(object, field)?;
    }
    for field in [
        "camera_calibrated_head_stage",
        "source_stage",
        "target_stage",
        "form_stage",
    ] {
        required_text(object, field)?;
    }
    if !PRODUCTION_WEAPON_FORM_QUALITY_FORM_STAGES.contains(&request.form_stage.as_str())
        || !PRODUCTION_WEAPON_FORM_QUALITY_SOURCE_STAGES.contains(&request.source_stage.as_str())
        || !PRODUCTION_WEAPON_FORM_QUALITY_TARGET_STAGES.contains(&request.target_stage.as_str())
    {
        return Err(invalid("form stage transition is not in the closed set"));
    }
    let expected_stages = match request.form_stage.as_str() {
        "blockout" => ("camera-calibrated", "blockout-reviewed"),
        "primary" => ("blockout-reviewed", "primary-form-approved"),
        "secondary" => ("primary-form-approved", "secondary-form-approved"),
        _ => unreachable!(),
    };
    if (request.source_stage.as_str(), request.target_stage.as_str()) != expected_stages {
        return Err(invalid("form stage source/target differs"));
    }
    if request.camera_calibrated_head_stage != "camera-calibrated"
        || request.form_quality_policy != PRODUCTION_WEAPON_FORM_QUALITY_POLICY
        || request.threshold_policy != PRODUCTION_WEAPON_FORM_QUALITY_THRESHOLD_POLICY
        || request.form_quality_policy_sha256
            != sha256_hex(PRODUCTION_WEAPON_FORM_QUALITY_POLICY.as_bytes())
        || request.threshold_policy_sha256
            != sha256_hex(PRODUCTION_WEAPON_FORM_QUALITY_THRESHOLD_POLICY.as_bytes())
    {
        return Err(invalid("form-quality policy or head stage differs"));
    }
    if request.reviewed_reference_view_kinds
        != PRODUCTION_WEAPON_FORM_QUALITY_REVIEWED_REFERENCE_VIEW_KINDS
        || request.fixed_camera_view_kinds != PRODUCTION_WEAPON_FORM_QUALITY_FIXED_CAMERA_VIEW_KINDS
        || request.cross_view_evidence_view_kinds
            != PRODUCTION_WEAPON_FORM_QUALITY_REVIEWED_REFERENCE_VIEW_KINDS
    {
        return Err(invalid("reference/camera view kind set differs"));
    }
    if request.form_view_evaluations.len() != 6
        || request
            .form_view_evaluations
            .iter()
            .map(|value| value.view_kind.as_str())
            .collect::<Vec<_>>()
            != PRODUCTION_WEAPON_FORM_QUALITY_REVIEWED_REFERENCE_VIEW_KINDS
    {
        return Err(invalid(
            "form view evaluations must contain the six ordered views",
        ));
    }
    if request.form_stage == "blockout" {
        if request.previous_form_quality_id.is_some()
            || request.previous_form_quality_report_object_sha256.is_some()
            || request.previous_form_quality_canonical_sha256.is_some()
        {
            return Err(invalid("blockout previous quality must be null"));
        }
    } else if request.previous_form_quality_id.is_none()
        || request.previous_form_quality_report_object_sha256.is_none()
        || request.previous_form_quality_canonical_sha256.is_none()
    {
        return Err(invalid("later form edge requires previous quality"));
    }
    let mut preimage = object.clone();
    preimage.remove("input_sha256");
    preimage.remove("idempotency_key");
    let request_sha = canonical_json_hash(&Value::Object(preimage));
    if request.input_sha256 != request_sha {
        return Err(invalid("input_sha256 differs"));
    }
    Ok((request, request_sha))
}

fn parse_get(value: &Value) -> Result<ProductionWeaponFormQualityGetRequest, RuntimeError> {
    let object = exact_object(value, GET_FIELDS, "ProductionWeaponFormQualityGetRequest@1")?;
    if required_text(object, "schema_version")?
        != PRODUCTION_WEAPON_FORM_QUALITY_GET_REQUEST_SCHEMA_VERSION
    {
        return Err(invalid("get schema version differs"));
    }
    let request: ProductionWeaponFormQualityGetRequest = serde_json::from_value(value.clone())
        .map_err(|error| invalid(format!("get request is malformed: {error}")))?;
    required_id(object, "form_quality_id")?;
    required_id(object, "session_id")?;
    required_id(object, "project_id")?;
    required_id(object, "candidate_id")?;
    if !PRODUCTION_WEAPON_FORM_QUALITY_FORM_STAGES.contains(&request.form_stage.as_str()) {
        return Err(invalid("get form stage differs"));
    }
    Ok(request)
}

/// The legacy request names the source-head fields after the original
/// `camera-calibrated` edge, but each form edge is bound to the *current*
/// ProductionStage@3 head.  Keep the old wire shape while deriving the
/// expected head stage from the closed form-stage transition.
fn expected_source_head_stage(form_stage: &str) -> Result<&'static str, RuntimeError> {
    match form_stage {
        "blockout" => Ok("camera-calibrated"),
        "primary" => Ok("blockout-reviewed"),
        "secondary" => Ok("primary-form-approved"),
        _ => Err(invalid("form stage is not a supported source-head edge")),
    }
}

fn validate_stage_and_lock(
    runtime: &Runtime,
    request: &ProductionWeaponFormQualityPrepareRequest,
) -> Result<
    (
        ProductionStageTransitionV3Record,
        ProductionStageHeadV3Record,
        ProductionCameraLockRecord,
    ),
    RuntimeError,
> {
    let stage_result = runtime.production_stage_transition_v3_get(serde_json::json!({
        "schema_version":"ProductionStageTransitionGetRequest@3",
        "transition_id":request.camera_calibrated_head_transition_id,
        "session_id":request.session_id,
        "project_id":request.project_id,
        "root_candidate_id":request.camera_calibrated_head_candidate_id,
        "head_candidate_id":request.camera_calibrated_head_candidate_id,
    }))?;
    let transition: ProductionStageTransitionV3Record = serde_json::from_value(
        stage_result
            .get("transition")
            .cloned()
            .ok_or_else(|| invalid("camera transition missing"))?,
    )
    .map_err(|error| invalid(format!("camera transition malformed: {error}")))?;
    let head: ProductionStageHeadV3Record = serde_json::from_value(
        stage_result
            .get("production_stage_head")
            .cloned()
            .ok_or_else(|| invalid("camera head missing"))?,
    )
    .map_err(|error| invalid(format!("camera head malformed: {error}")))?;
    let expected_head_stage = expected_source_head_stage(request.form_stage.as_str())?;
    if transition.canonical_sha256 != request.camera_calibrated_head_transition_sha256
        || head.canonical_sha256 != request.camera_calibrated_head_canonical_sha256
        || transition.to_stage != expected_head_stage
        || head.head_stage != expected_head_stage
        || head.head_candidate_id != request.camera_calibrated_head_candidate_id
        || head.head_candidate_state_sha256 != request.camera_calibrated_head_candidate_state_sha256
        || head.output_artifact_id != request.camera_calibrated_head_artifact_id
        || head.head_artifact_sha256 != request.camera_calibrated_head_artifact_sha256
        || head.session_id != request.session_id
        || head.project_id != request.project_id
    {
        return Err(invalid("current form source head binding differs"));
    }
    if request.candidate_id != head.head_candidate_id
        || request.candidate_state_sha256 != head.head_candidate_state_sha256
        || request.artifact_id != head.output_artifact_id
        || request.artifact_sha256 != head.head_artifact_sha256
    {
        return Err(invalid(
            "form candidate is not the current form source head",
        ));
    }
    let candidate = runtime
        .candidate(&request.candidate_id)?
        .ok_or_else(|| invalid("form candidate is unavailable"))?;
    if candidate.project_id != request.project_id
        || candidate.canonical_sha256 != request.candidate_state_sha256
        || candidate.prepared_object_id.as_deref() != Some(request.artifact_id.as_str())
        || candidate.prepared_object_sha256.as_deref() != Some(request.artifact_sha256.as_str())
        || candidate
            .manifest_hash
            .as_deref()
            .is_some_and(|hash| hash != request.artifact_sha256)
    {
        return Err(invalid("form candidate artifact readback differs"));
    }
    let lock_result = runtime.production_camera_lock_get(serde_json::json!({
        "schema_version":"ProductionCameraLockGetRequest@1", "camera_lock_id":request.camera_lock_id,
        "session_id":request.session_id, "project_id":request.project_id, "candidate_id":request.candidate_id
    }))?;
    let lock: ProductionCameraLockRecord = serde_json::from_value(
        lock_result
            .get("camera_lock")
            .cloned()
            .ok_or_else(|| invalid("camera lock missing"))?,
    )
    .map_err(|error| invalid(format!("camera lock malformed: {error}")))?;
    if lock.schema_version != PRODUCTION_CAMERA_LOCK_SCHEMA_VERSION
        || lock.camera_lock_id != request.camera_lock_id
        || lock.canonical_sha256 != request.camera_lock_canonical_sha256
        || lock.camera_rig_object_sha256 != request.camera_rig_object_sha256
        || lock.camera_rig_canonical_sha256 != request.camera_rig_canonical_sha256
        || lock.receipt_object_sha256 != request.camera_lock_receipt_object_sha256
        || lock.source_transition_id != request.camera_lock_source_transition_id
        || lock.source_transition_sha256 != request.camera_lock_source_transition_sha256
        || lock.source_head_canonical_sha256 != request.camera_lock_source_head_canonical_sha256
        || lock.session_id != request.session_id
        || lock.project_id != request.project_id
        || lock.candidate_id != request.candidate_id
        || lock.candidate_state_sha256 != request.candidate_state_sha256
        || lock.artifact_id != request.artifact_id
        || lock.artifact_sha256 != request.artifact_sha256
        || lock.reference_canvas_object_sha256 != request.reference_canvas_object_sha256
        || lock.reference_canvas_canonical_sha256 != request.reference_canvas_canonical_sha256
        || lock.design_spec_object_sha256 != request.design_spec_object_sha256
        || lock.design_spec_canonical_sha256 != request.design_spec_canonical_sha256
        || lock.calibration_policy != "fps-weapon-reviewed-six-reference-seven-camera-lock@1"
        || lock.primary_view_kind != "left"
        || lock.review_status != "user-approved-reference-coverage"
        || lock.calibration_status != "passed"
        || lock.structural_status != "PASS_SOURCE_STRUCTURAL"
        || lock.visual_status != "QUALITY_TARGET_NOT_MET"
        || lock.human_status != "NOT_RUN"
        || lock.engine_status != "NOT_RUN"
        || lock.distribution_status != "NOT_RUN"
        || lock.required_reference_view_kinds != PRODUCTION_CAMERA_LOCK_REFERENCE_VIEW_KINDS
        || lock.required_camera_view_kinds != PRODUCTION_CAMERA_LOCK_CAMERA_VIEW_KINDS
    {
        return Err(invalid("ProductionCameraLock binding differs"));
    }
    Ok((transition, head, lock))
}

fn validate_authoring_documents(
    runtime: &Runtime,
    request: &ProductionWeaponFormQualityPrepareRequest,
) -> Result<Value, RuntimeError> {
    let canvas = json_object(
        runtime,
        &request.reference_canvas_object_sha256,
        "ReferenceCanvas",
    )?;
    let canvas_hash = canonical_document(&canvas, "ReferenceCanvas@1", "ReferenceCanvas")?;
    if canvas_hash != request.reference_canvas_canonical_sha256
        || canvas.get("project_id").and_then(Value::as_str) != Some(request.project_id.as_str())
        || canvas
            .get("reference_set_sha256")
            .and_then(Value::as_str)
            .is_none_or(|value| !is_sha256(value))
    {
        return Err(invalid("ReferenceCanvas binding differs"));
    }
    let canvas_id = canvas
        .get("canvas_id")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("ReferenceCanvas canvas_id missing"))?;
    let spec = json_object(runtime, &request.design_spec_object_sha256, "DesignSpec")?;
    let spec_hash = canonical_document(&spec, "DesignSpec@1", "DesignSpec")?;
    if spec_hash != request.design_spec_canonical_sha256
        || spec.get("project_id").and_then(Value::as_str) != Some(request.project_id.as_str())
        || spec.get("reference_canvas_id").and_then(Value::as_str) != Some(canvas_id)
        || spec.get("reference_canvas_sha256").and_then(Value::as_str)
            != Some(request.reference_canvas_object_sha256.as_str())
    {
        return Err(invalid("DesignSpec binding differs"));
    }
    Ok(canvas)
}

fn index_reference_canvas_views<'a>(
    canvas: &'a Value,
) -> Result<std::collections::BTreeMap<&'a str, &'a Value>, RuntimeError> {
    let canvas_views = canvas
        .get("views")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("ReferenceCanvas views missing"))?;
    let mut by_id = std::collections::BTreeMap::new();
    for canvas_view in canvas_views {
        let canvas_view_object = canvas_view
            .as_object()
            .ok_or_else(|| invalid("ReferenceCanvas view is malformed"))?;
        let canvas_view_id = canvas_view_object
            .get("view_id")
            .and_then(Value::as_str)
            .filter(|value| is_opaque_id(value))
            .ok_or_else(|| invalid("ReferenceCanvas view_id is invalid"))?;
        if by_id.insert(canvas_view_id, canvas_view).is_some() {
            return Err(invalid("ReferenceCanvas contains duplicate view_id"));
        }
        let canvas_kind = canvas_view_object
            .get("kind")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| invalid("ReferenceCanvas view kind is invalid"))?;
        let canvas_reference_id = canvas_view_object
            .get("reference_id")
            .and_then(Value::as_str)
            .filter(|value| is_opaque_id(value))
            .ok_or_else(|| invalid("ReferenceCanvas view reference_id is invalid"))?;
        let canvas_reference_sha256 = canvas_view_object
            .get("reference_sha256")
            .and_then(Value::as_str)
            .filter(|value| is_sha256(value))
            .ok_or_else(|| invalid("ReferenceCanvas view reference_sha256 is invalid"))?;
        let _ = (canvas_kind, canvas_reference_id, canvas_reference_sha256);
    }
    Ok(by_id)
}

fn validate_reference_canvas_view_binding(
    canvas_views: &std::collections::BTreeMap<&str, &Value>,
    view_id: &str,
    kind: &str,
    reference_id: &str,
    reference_sha256: &str,
) -> Result<(), RuntimeError> {
    let canvas_view = canvas_views
        .get(view_id)
        .ok_or_else(|| invalid("CrossView view is missing from ReferenceCanvas"))?;
    if canvas_view.get("kind").and_then(Value::as_str) != Some(kind)
        || canvas_view.get("reference_id").and_then(Value::as_str) != Some(reference_id)
        || canvas_view.get("reference_sha256").and_then(Value::as_str) != Some(reference_sha256)
    {
        return Err(invalid(
            "CrossView view_id/kind/reference binding differs from ReferenceCanvas",
        ));
    }
    Ok(())
}

fn validate_comparison_render_binding(
    label: &str,
    comparison: &Value,
    render: &Value,
    render_hash: &str,
    expected_reference_id: &str,
    expected_reference_sha256: &str,
    expected_camera_hash: &str,
    expected_view_id: &str,
    proposal_candidate_id: Option<&str>,
    proposal_artifact_sha256: Option<&str>,
) -> Result<(), RuntimeError> {
    if comparison.get("reference_id").and_then(Value::as_str) != Some(expected_reference_id)
        || comparison.get("reference_sha256").and_then(Value::as_str)
            != Some(expected_reference_sha256)
        || comparison.get("camera_hash").and_then(Value::as_str) != Some(expected_camera_hash)
        || comparison.get("render_set_hash").and_then(Value::as_str) != Some(render_hash)
        || comparison.get("view_id").and_then(Value::as_str) != Some(expected_view_id)
    {
        return Err(invalid(format!("{label} ComparisonReport binding differs")));
    }
    if comparison.get("candidate_id").and_then(Value::as_str)
        != render.get("candidate_id").and_then(Value::as_str)
        || comparison.get("artifact_sha256").and_then(Value::as_str)
            != render.get("artifact_sha256").and_then(Value::as_str)
    {
        return Err(invalid(format!(
            "{label} ComparisonReport candidate/artifact differs from RenderSet"
        )));
    }
    if let (Some(candidate_id), Some(artifact_sha256)) =
        (proposal_candidate_id, proposal_artifact_sha256)
    {
        if comparison.get("candidate_id").and_then(Value::as_str) != Some(candidate_id)
            || comparison.get("artifact_sha256").and_then(Value::as_str) != Some(artifact_sha256)
        {
            return Err(invalid(
                "proposal ComparisonReport candidate/artifact differs",
            ));
        }
    }
    Ok(())
}

fn validate_form_evidence_binding(
    runtime: &Runtime,
    request: &ProductionWeaponFormQualityPrepareRequest,
) -> Result<ProductionWeaponFormEvidenceRecord, RuntimeError> {
    let bytes = runtime.cas_read(&request.form_evidence_object_sha256)?;
    if bytes.len() > MAX_JSON_BYTES {
        return Err(invalid("form evidence parent receipt exceeds 1 MiB"));
    }
    let value: Value = serde_json::from_slice(&bytes)
        .map_err(|error| invalid(format!("form evidence parent receipt is invalid: {error}")))?;
    if canonical_document(
        &value,
        PRODUCTION_WEAPON_FORM_EVIDENCE_SCHEMA_VERSION,
        "ProductionWeaponFormEvidence",
    )? != request.form_evidence_canonical_sha256
        || value.get("receipt_object_sha256").and_then(Value::as_str) != Some("")
    {
        return Err(invalid("form evidence parent receipt binding differs"));
    }
    let mut evidence: ProductionWeaponFormEvidenceRecord =
        serde_json::from_value(value).map_err(|error| {
            invalid(format!(
                "form evidence parent receipt is malformed: {error}"
            ))
        })?;
    evidence.receipt_object_sha256 = request.form_evidence_object_sha256.clone();
    let stored = runtime
        .store
        .get_production_weapon_form_evidence(&evidence.form_evidence_id)?
        .ok_or_else(|| invalid("form evidence durable link is unavailable"))?;
    if stored != evidence
        || evidence.session_id != request.session_id
        || evidence.project_id != request.project_id
        || evidence.candidate_id != request.candidate_id
        || evidence.candidate_state_sha256 != request.candidate_state_sha256
        || evidence.artifact_id != request.artifact_id
        || evidence.artifact_sha256 != request.artifact_sha256
        || evidence.reference_canvas_object_sha256 != request.reference_canvas_object_sha256
        || evidence.reference_canvas_canonical_sha256 != request.reference_canvas_canonical_sha256
        || evidence.design_spec_object_sha256 != request.design_spec_object_sha256
        || evidence.design_spec_canonical_sha256 != request.design_spec_canonical_sha256
        || evidence.camera_lock_id != request.camera_lock_id
        || evidence.camera_lock_canonical_sha256 != request.camera_lock_canonical_sha256
        || evidence.camera_rig_object_sha256 != request.camera_rig_object_sha256
        || evidence.camera_rig_canonical_sha256 != request.camera_rig_canonical_sha256
        || evidence.camera_lock_receipt_object_sha256 != request.camera_lock_receipt_object_sha256
        || evidence.camera_lock_source_transition_id != request.camera_lock_source_transition_id
        || evidence.camera_lock_source_transition_sha256
            != request.camera_lock_source_transition_sha256
        || evidence.camera_lock_source_head_canonical_sha256
            != request.camera_lock_source_head_canonical_sha256
        || evidence.view_kinds != request.cross_view_evidence_view_kinds
        || evidence.quality_status != PRODUCTION_WEAPON_FORM_EVIDENCE_QUALITY_STATUS
        || evidence.production_stage_advanced
        || evidence.candidate_confirmed
        || evidence.version_created
        || evidence.export_performed
    {
        return Err(invalid("form evidence durable binding differs"));
    }
    Ok(evidence)
}

fn validate_cross_view_and_build_views(
    runtime: &Runtime,
    request: &ProductionWeaponFormQualityPrepareRequest,
    canvas: &Value,
) -> Result<
    (
        Value,
        Vec<ProductionWeaponFormQualityViewRecord>,
        bool,
        bool,
        bool,
        bool,
    ),
    RuntimeError,
> {
    let bundle = json_object(
        runtime,
        &request.cross_view_evidence_object_sha256,
        "CrossViewEvidenceBundle",
    )?;
    super::validate_cross_view_evidence_bundle(&bundle)?;
    if canonical_document(
        &bundle,
        "CrossViewEvidenceBundle@1",
        "CrossViewEvidenceBundle",
    )? != request.cross_view_evidence_canonical_sha256
        || bundle.get("project_id").and_then(Value::as_str) != Some(request.project_id.as_str())
        || bundle.get("session_id").and_then(Value::as_str) != Some(request.session_id.as_str())
        || bundle.get("candidate_id").and_then(Value::as_str) != Some(request.candidate_id.as_str())
        || bundle.get("candidate_state_sha256").and_then(Value::as_str)
            != Some(request.candidate_state_sha256.as_str())
        || bundle.get("artifact_sha256").and_then(Value::as_str)
            != Some(request.artifact_sha256.as_str())
        || bundle
            .get("reference_canvas_sha256")
            .and_then(Value::as_str)
            != Some(request.reference_canvas_object_sha256.as_str())
    {
        return Err(invalid("CrossViewEvidenceBundle binding differs"));
    }
    let evaluations = bundle
        .get("view_evaluations")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("CrossView view evaluations missing"))?;
    let form_evidence = validate_form_evidence_binding(runtime, request)?;
    let form_evidence_by_kind = form_evidence
        .views
        .iter()
        .map(|view| (view.view_kind.as_str(), view))
        .collect::<std::collections::BTreeMap<_, _>>();
    if form_evidence_by_kind.len() != 6 {
        return Err(invalid("form evidence must contain six unique view kinds"));
    }
    let canvas_by_id = index_reference_canvas_views(canvas)?;
    let mut by_kind = std::collections::BTreeMap::new();
    for evaluation in evaluations {
        let kind = evaluation
            .get("kind")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid("CrossView view kind missing"))?;
        if PRODUCTION_WEAPON_FORM_QUALITY_REVIEWED_REFERENCE_VIEW_KINDS.contains(&kind) {
            by_kind.insert(kind, evaluation);
        }
    }
    if by_kind.len() != 6 {
        return Err(invalid(
            "CrossView bundle must contain exactly six form views",
        ));
    }
    let readback = runtime.artifact_readback(&request.artifact_sha256, &request.candidate_id)?;
    let observed_part_ids = readback
        .get("part_ids")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("ArtifactReadback part_ids missing"))?
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| invalid("ArtifactReadback part id invalid"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let observed_set = observed_part_ids.iter().cloned().collect::<BTreeSet<_>>();
    let mut views = Vec::new();
    let mut all_thresholds = true;
    let mut all_no_regression = true;
    let mut all_part = true;
    for (kind, requested) in PRODUCTION_WEAPON_FORM_QUALITY_REVIEWED_REFERENCE_VIEW_KINDS
        .iter()
        .zip(request.form_view_evaluations.iter())
    {
        let evaluation = by_kind
            .get(kind)
            .ok_or_else(|| invalid("CrossView view ordering differs"))?;
        let typed_evidence = form_evidence_by_kind
            .get(kind)
            .ok_or_else(|| invalid("form evidence view kind is unavailable"))?;
        if requested.view_kind != *kind
            || requested.view_id
                != evaluation
                    .get("view_id")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
        {
            return Err(invalid("form view id/kind differs from CrossView"));
        }
        let expected_reference_id = evaluation
            .get("reference_id")
            .and_then(Value::as_str)
            .filter(|value| is_opaque_id(value))
            .ok_or_else(|| invalid("CrossView reference id missing"))?;
        let expected_reference_sha256 = evaluation
            .get("reference_sha256")
            .and_then(Value::as_str)
            .filter(|value| is_sha256(value))
            .ok_or_else(|| invalid("CrossView reference hash missing"))?;
        let evaluation_kind = evaluation
            .get("kind")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid("CrossView view kind missing"))?;
        if evaluation_kind != *kind {
            return Err(invalid(
                "CrossView view kind differs from ordered form view",
            ));
        }
        validate_reference_canvas_view_binding(
            &canvas_by_id,
            requested.view_id.as_str(),
            kind,
            expected_reference_id,
            expected_reference_sha256,
        )?;
        let reference = runtime
            .reference(expected_reference_id)?
            .ok_or_else(|| invalid("CrossView reference is unavailable"))?;
        if reference.project_id != request.project_id
            || reference.object_sha256 != expected_reference_sha256
        {
            return Err(invalid("CrossView per-view reference binding differs"));
        }
        let proposal_render_hash = evaluation
            .get("proposal_render_set_sha256")
            .and_then(Value::as_str)
            .filter(|value| is_sha256(value))
            .ok_or_else(|| invalid("proposal RenderSet hash missing"))?;
        if typed_evidence.view_id != requested.view_id
            || typed_evidence.reference_id != expected_reference_id
            || typed_evidence.reference_sha256 != expected_reference_sha256
            || typed_evidence.render_set_object_sha256 != proposal_render_hash
            || typed_evidence.render_set_view_id != requested.view_id
        {
            return Err(invalid("form evidence view binding differs from CrossView"));
        }
        let proposal_comparison_hash = evaluation
            .get("proposal_comparison_report_sha256")
            .and_then(Value::as_str)
            .filter(|value| is_sha256(value))
            .ok_or_else(|| invalid("proposal comparison hash missing"))?;
        let proposal_quality_hash = evaluation
            .get("proposal_quality_report_sha256")
            .and_then(Value::as_str)
            .filter(|value| is_sha256(value))
            .ok_or_else(|| invalid("proposal quality hash missing"))?;
        let baseline_render_hash = evaluation
            .get("baseline_render_set_sha256")
            .and_then(Value::as_str)
            .filter(|value| is_sha256(value))
            .ok_or_else(|| invalid("baseline RenderSet hash missing"))?;
        let baseline_comparison_hash = evaluation
            .get("baseline_comparison_report_sha256")
            .and_then(Value::as_str)
            .filter(|value| is_sha256(value))
            .ok_or_else(|| invalid("baseline comparison hash missing"))?;
        let baseline_quality_hash = evaluation
            .get("baseline_quality_report_sha256")
            .and_then(Value::as_str)
            .filter(|value| is_sha256(value))
            .ok_or_else(|| invalid("baseline quality hash missing"))?;
        let expected_camera_hash = evaluation
            .get("camera_hash")
            .and_then(Value::as_str)
            .filter(|value| is_sha256(value))
            .ok_or_else(|| invalid("CrossView camera hash missing"))?;
        if typed_evidence.camera_hash != expected_camera_hash {
            return Err(invalid("form evidence camera differs from CrossView"));
        }

        // CrossViewEvidenceBundle is only authoritative when both sides of
        // every comparison are still readable.  Revalidate all six baseline
        // and proposal RenderSet/Comparison/Quality triplets instead of
        // trusting the mutable visual_evidence_views projection.
        let baseline_render = json_object(runtime, baseline_render_hash, "baseline RenderSet")?;
        super::validate_render_set_v2_output(&baseline_render)?;
        let proposal_render = json_object(runtime, proposal_render_hash, "proposal RenderSet")?;
        super::validate_render_set_v2_output(&proposal_render)?;
        for (label, render, render_hash) in [
            ("baseline", &baseline_render, baseline_render_hash),
            ("proposal", &proposal_render, proposal_render_hash),
        ] {
            if render.get("reference_id").and_then(Value::as_str) != Some(expected_reference_id)
                || render.get("camera_hash").and_then(Value::as_str) != Some(expected_camera_hash)
                || render.get("view_id").and_then(Value::as_str) != Some(requested.view_id.as_str())
            {
                return Err(invalid(format!("{label} RenderSet binding differs")));
            }
            if label == "proposal"
                && (render.get("candidate_id").and_then(Value::as_str)
                    != Some(request.candidate_id.as_str())
                    || render.get("artifact_sha256").and_then(Value::as_str)
                        != Some(request.artifact_sha256.as_str()))
            {
                return Err(invalid("proposal RenderSet candidate/artifact differs"));
            }
            if render_hash.is_empty() {
                return Err(invalid("RenderSet hash is empty"));
            }
        }
        let baseline_comparison = json_object(
            runtime,
            baseline_comparison_hash,
            "baseline ComparisonReport",
        )?;
        super::validate_reference_comparison_report(&baseline_comparison)?;
        let proposal_comparison = json_object(
            runtime,
            proposal_comparison_hash,
            "proposal ComparisonReport",
        )?;
        super::validate_reference_comparison_report(&proposal_comparison)?;
        for (label, comparison, render, render_hash) in [
            (
                "baseline",
                &baseline_comparison,
                &baseline_render,
                baseline_render_hash,
            ),
            (
                "proposal",
                &proposal_comparison,
                &proposal_render,
                proposal_render_hash,
            ),
        ] {
            validate_comparison_render_binding(
                label,
                comparison,
                render,
                render_hash,
                expected_reference_id,
                expected_reference_sha256,
                expected_camera_hash,
                requested.view_id.as_str(),
                (label == "proposal").then_some(request.candidate_id.as_str()),
                (label == "proposal").then_some(request.artifact_sha256.as_str()),
            )?;
        }
        let baseline_quality =
            json_object(runtime, baseline_quality_hash, "baseline QualityReport")?;
        super::validate_quality_report_v2_output(&baseline_quality)?;
        let proposal_quality =
            json_object(runtime, proposal_quality_hash, "proposal QualityReport")?;
        super::validate_quality_report_v2_output(&proposal_quality)?;
        for (label, quality, render, render_hash, comparison_hash) in [
            (
                "baseline",
                &baseline_quality,
                &baseline_render,
                baseline_render_hash,
                baseline_comparison_hash,
            ),
            (
                "proposal",
                &proposal_quality,
                &proposal_render,
                proposal_render_hash,
                proposal_comparison_hash,
            ),
        ] {
            if quality.get("render_set_hash").and_then(Value::as_str) != Some(render_hash)
                || quality
                    .get("comparison_report_hash")
                    .and_then(Value::as_str)
                    != Some(comparison_hash)
                || quality.get("view_id").and_then(Value::as_str)
                    != Some(requested.view_id.as_str())
            {
                return Err(invalid(format!("{label} QualityReport binding differs")));
            }
            if quality.get("candidate_id").and_then(Value::as_str)
                != render.get("candidate_id").and_then(Value::as_str)
                || quality.get("artifact_sha256").and_then(Value::as_str)
                    != render.get("artifact_sha256").and_then(Value::as_str)
            {
                return Err(invalid(format!(
                    "{label} QualityReport candidate/artifact differs from RenderSet"
                )));
            }
            if label == "proposal"
                && (quality.get("candidate_id").and_then(Value::as_str)
                    != Some(request.candidate_id.as_str())
                    || quality.get("artifact_sha256").and_then(Value::as_str)
                        != Some(request.artifact_sha256.as_str()))
            {
                return Err(invalid("proposal QualityReport candidate/artifact differs"));
            }
        }
        let expected = requested.part_id_evidence.expected_part_ids.clone();
        let expected_set = expected.iter().cloned().collect::<BTreeSet<_>>();
        // CrossViewEvidenceBundle@1 does not carry a Runtime-owned Part-ID
        // evidence receipt.  Caller-provided expected/observed arrays are
        // therefore diagnostic input only and may not promote this gate.
        // Keep the projection NOT_PROVEN until a typed receipt is added.
        let _part_shape_matches = expected_set == observed_set
            && requested
                .part_id_evidence
                .observed_part_ids
                .iter()
                .cloned()
                .collect::<BTreeSet<_>>()
                == observed_set
            && requested.part_id_evidence.missing_part_ids.is_empty()
            && requested.part_id_evidence.unexpected_part_ids.is_empty();
        let part_ok = false;
        all_part &= part_ok;
        all_thresholds &= evaluation.get("proposal_status").and_then(Value::as_str)
            == Some("PARTIAL_VISIBLE_VIEW_PASS");
        all_no_regression &= evaluation.get("non_regressing").and_then(Value::as_bool)
            == Some(true)
            && requested.no_regression.metrics_not_regressed;
        let part_source = if part_ok {
            ProductionWeaponFormQualityEvidenceBinding {
                source_kind: "cross-view-evidence-bundle".into(),
                source_object_sha256: Some(request.cross_view_evidence_object_sha256.clone()),
                evidence_object_sha256: Some(proposal_render_hash.to_owned()),
                status: "PASS".into(),
            }
        } else {
            ProductionWeaponFormQualityEvidenceBinding {
                source_kind: "not-proven".into(),
                source_object_sha256: None,
                evidence_object_sha256: None,
                status: "NOT_PROVEN".into(),
            }
        };
        let negative = ProductionWeaponFormQualityNegativeSpaceEvidence {
            source: ProductionWeaponFormQualityEvidenceBinding {
                source_kind: "not-proven".into(),
                source_object_sha256: None,
                evidence_object_sha256: None,
                status: "NOT_PROVEN".into(),
            },
            expected_count: 0,
            observed_count: 0,
            missing_count: 0,
            sealed_count: 0,
            coverage_milli: 0,
        };
        let line = ProductionWeaponFormQualityLineFlowEvidence {
            source: ProductionWeaponFormQualityEvidenceBinding {
                source_kind: "not-proven".into(),
                source_object_sha256: None,
                evidence_object_sha256: None,
                status: "NOT_PROVEN".into(),
            },
            expected_count: 0,
            observed_count: 0,
            coverage_milli: 0,
            continuity_milli: 0,
            deviation_milli: 0,
        };
        views.push(ProductionWeaponFormQualityViewRecord {
            view_kind: requested.view_kind.clone(),
            view_id: requested.view_id.clone(),
            part_id_evidence: ProductionWeaponFormQualityPartIdEvidence {
                source: part_source,
                expected_part_ids: expected,
                observed_part_ids: observed_part_ids.clone(),
                missing_part_ids: Vec::new(),
                unexpected_part_ids: Vec::new(),
                coverage_milli: if part_ok { 1000 } else { 0 },
            },
            negative_space_evidence: negative,
            line_flow_evidence: line,
            no_regression: ProductionWeaponFormQualityNoRegression {
                status: "NOT_PROVEN".into(),
                metrics_not_regressed: evaluation
                    .get("non_regressing")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                part_id_not_regressed: part_ok,
                negative_space_not_regressed: false,
                line_flow_not_regressed: false,
            },
        });
    }
    Ok((
        bundle,
        views,
        all_thresholds,
        all_no_regression,
        all_part,
        false,
    ))
}

fn record_from_request(
    runtime: &Runtime,
    request: ProductionWeaponFormQualityPrepareRequest,
    request_sha256: &str,
) -> Result<ProductionWeaponFormQualityRecord, RuntimeError> {
    let (_transition, head, lock) = validate_stage_and_lock(runtime, &request)?;
    let canvas = validate_authoring_documents(runtime, &request)?;
    let (_bundle, views, thresholds, no_regression, part_ids, _unused) =
        validate_cross_view_and_build_views(runtime, &request, &canvas)?;
    let previous_binding = if request.form_stage == "blockout" {
        true
    } else {
        let previous_id = request
            .previous_form_quality_id
            .as_deref()
            .ok_or_else(|| invalid("previous form quality id is missing"))?;
        let previous = runtime
            .store
            .get_production_weapon_form_quality(previous_id)?
            .ok_or_else(|| invalid("previous form quality is unavailable"))?;
        let expected_previous_stage = if request.form_stage == "primary" {
            "blockout"
        } else {
            "primary"
        };
        if previous.session_id != request.session_id
            || previous.project_id != request.project_id
            || previous.candidate_id != request.candidate_id
            || previous.candidate_state_sha256 != request.candidate_state_sha256
            || previous.artifact_id != request.artifact_id
            || previous.artifact_sha256 != request.artifact_sha256
            || previous.form_stage != expected_previous_stage
            || previous.target_stage != request.source_stage
            || request
                .previous_form_quality_report_object_sha256
                .as_deref()
                != Some(previous.receipt_object_sha256.as_str())
            || request.previous_form_quality_canonical_sha256.as_deref()
                != Some(previous.canonical_sha256.as_str())
        {
            return Err(invalid("previous form quality binding differs"));
        }
        true
    };
    let hard_gate = ProductionWeaponFormQualityHardGate {
        stage_head_binding: true,
        camera_lock_binding: true,
        same_candidate_artifact: true,
        reviewed_reference_views: true,
        fixed_camera_views: true,
        cross_view_evidence_binding: true,
        form_view_evaluations: true,
        part_id_evidence: part_ids,
        negative_space_evidence: false,
        line_flow_evidence: false,
        threshold_policy_binding: true,
    };
    let hard_gate_passed = hard_gate.stage_head_binding
        && hard_gate.camera_lock_binding
        && hard_gate.same_candidate_artifact
        && hard_gate.reviewed_reference_views
        && hard_gate.fixed_camera_views
        && hard_gate.cross_view_evidence_binding
        && hard_gate.form_view_evaluations
        && hard_gate.part_id_evidence
        && hard_gate.negative_space_evidence
        && hard_gate.line_flow_evidence
        && hard_gate.threshold_policy_binding;
    let form_gate_passed = hard_gate_passed && thresholds && no_regression && previous_binding;
    let created_at = runtime
        .candidate(&request.candidate_id)?
        .ok_or_else(|| invalid("candidate not found"))?
        .updated_at;
    let mut record = ProductionWeaponFormQualityRecord {
        schema_version: PRODUCTION_WEAPON_FORM_QUALITY_SCHEMA_VERSION.into(),
        form_quality_id: request.form_quality_id.clone(),
        session_id: request.session_id.clone(),
        project_id: request.project_id.clone(),
        form_stage: request.form_stage.clone(),
        source_stage: request.source_stage.clone(),
        target_stage: request.target_stage.clone(),
        camera_calibrated_head_transition_id: request.camera_calibrated_head_transition_id.clone(),
        camera_calibrated_head_transition_sha256: request
            .camera_calibrated_head_transition_sha256
            .clone(),
        camera_calibrated_head_canonical_sha256: request
            .camera_calibrated_head_canonical_sha256
            .clone(),
        camera_calibrated_head_candidate_id: request.camera_calibrated_head_candidate_id.clone(),
        camera_calibrated_head_candidate_state_sha256: request
            .camera_calibrated_head_candidate_state_sha256
            .clone(),
        camera_calibrated_head_artifact_id: request.camera_calibrated_head_artifact_id.clone(),
        camera_calibrated_head_artifact_sha256: request
            .camera_calibrated_head_artifact_sha256
            .clone(),
        camera_calibrated_head_stage: request.camera_calibrated_head_stage.clone(),
        candidate_id: request.candidate_id.clone(),
        candidate_state_sha256: request.candidate_state_sha256.clone(),
        artifact_id: request.artifact_id.clone(),
        artifact_sha256: request.artifact_sha256.clone(),
        reference_id: request.reference_id.clone(),
        reference_sha256: request.reference_sha256.clone(),
        reference_canvas_object_sha256: request.reference_canvas_object_sha256.clone(),
        reference_canvas_canonical_sha256: request.reference_canvas_canonical_sha256.clone(),
        design_spec_object_sha256: request.design_spec_object_sha256.clone(),
        design_spec_canonical_sha256: request.design_spec_canonical_sha256.clone(),
        camera_lock_id: request.camera_lock_id.clone(),
        camera_lock_canonical_sha256: request.camera_lock_canonical_sha256.clone(),
        camera_rig_object_sha256: request.camera_rig_object_sha256.clone(),
        camera_rig_canonical_sha256: request.camera_rig_canonical_sha256.clone(),
        camera_lock_receipt_object_sha256: request.camera_lock_receipt_object_sha256.clone(),
        camera_lock_source_transition_id: request.camera_lock_source_transition_id.clone(),
        camera_lock_source_transition_sha256: request.camera_lock_source_transition_sha256.clone(),
        camera_lock_source_head_canonical_sha256: request
            .camera_lock_source_head_canonical_sha256
            .clone(),
        reviewed_reference_view_kinds: request.reviewed_reference_view_kinds.clone(),
        fixed_camera_view_kinds: request.fixed_camera_view_kinds.clone(),
        cross_view_evidence_object_sha256: request.cross_view_evidence_object_sha256.clone(),
        cross_view_evidence_canonical_sha256: request.cross_view_evidence_canonical_sha256.clone(),
        cross_view_evidence_view_kinds: request.cross_view_evidence_view_kinds.clone(),
        form_evidence_object_sha256: request.form_evidence_object_sha256.clone(),
        form_evidence_canonical_sha256: request.form_evidence_canonical_sha256.clone(),
        form_view_evaluations: views,
        previous_form_quality_id: request.previous_form_quality_id.clone(),
        previous_form_quality_report_object_sha256: request
            .previous_form_quality_report_object_sha256
            .clone(),
        previous_form_quality_canonical_sha256: request
            .previous_form_quality_canonical_sha256
            .clone(),
        form_quality_policy: request.form_quality_policy.clone(),
        form_quality_policy_sha256: request.form_quality_policy_sha256.clone(),
        threshold_policy: request.threshold_policy.clone(),
        threshold_policy_sha256: request.threshold_policy_sha256.clone(),
        layer_status: "QUALITY_TARGET_NOT_MET".into(),
        hard_gate,
        hard_gate_passed,
        form_gate: ProductionWeaponFormQualityFormGate {
            layer_status: "QUALITY_TARGET_NOT_MET".into(),
            all_view_thresholds: thresholds,
            all_view_no_regression: no_regression,
            previous_form_quality_binding: previous_binding,
        },
        form_gate_passed,
        validator_status: if hard_gate_passed { "passed" } else { "failed" }.into(),
        structural_status: "PASS_SOURCE_STRUCTURAL".into(),
        visual_status: "QUALITY_TARGET_NOT_MET".into(),
        human_status: "NOT_RUN".into(),
        engine_status: "NOT_RUN".into(),
        distribution_status: "NOT_RUN".into(),
        quality_status: "structural_only".into(),
        runtime_write_performed: true,
        production_stage_advanced: false,
        candidate_confirmed: false,
        version_created: false,
        export_performed: false,
        request_sha256: request_sha256.into(),
        input_sha256: request.input_sha256.clone(),
        receipt_object_sha256: String::new(),
        canonical_sha256: String::new(),
        created_at,
    };
    let _ = lock;
    let _ = head;
    let mut normalized =
        serde_json::to_value(&record).map_err(|error| invalid(error.to_string()))?;
    normalized["canonical_sha256"] = Value::String(String::new());
    normalized["receipt_object_sha256"] = Value::String(String::new());
    record.canonical_sha256 = canonical_json_hash(&normalized);
    Ok(record)
}

fn result_value(
    record: &ProductionWeaponFormQualityRecord,
    replayed: bool,
    schema: &str,
    runtime_write: bool,
) -> Result<Value, RuntimeError> {
    Ok(
        serde_json::json!({"schema_version":schema,"form_quality":serde_json::to_value(record).map_err(|error| invalid(error.to_string()))?,"replayed":replayed,"runtime_write":runtime_write,"production_stage_advanced":false,"candidate_confirmed":false,"version_created":false,"export_performed":false}),
    )
}

fn release(runtime: &Runtime, reservation: &CasReservation, objects: &[CasObject], cleanup: bool) {
    for object in objects {
        let _ = runtime.store.release_cas_reservation_object(
            reservation,
            object,
            cleanup && object.created_new,
        );
    }
}

impl Runtime {
    pub fn production_weapon_form_quality_prepare(
        &self,
        value: Value,
    ) -> Result<Value, RuntimeError> {
        let (request, request_sha) = parse_prepare(&value)?;
        let mut record = record_from_request(self, request, &request_sha)?;
        // Store owns one immutable receipt object for this gate.  Its payload
        // is the canonical record projection with the receipt self-reference
        // cleared; the SQLite link binds the resulting CAS hash afterwards.
        // Keeping a separate report object would create two CAS identities
        // for the same bytes and is intentionally outside the contract.
        let mut receipt_value =
            serde_json::to_value(&record).map_err(|error| invalid(error.to_string()))?;
        receipt_value["receipt_object_sha256"] = Value::String(String::new());
        let receipt_bytes =
            canonical_json_bytes(&receipt_value).map_err(|error| invalid(error.to_string()))?;
        if receipt_bytes.len() > MAX_JSON_BYTES {
            return Err(invalid("form-quality receipt exceeds 1 MiB"));
        }
        let reservation = self.store.begin_cas_reservation();
        let receipt = self.store.put_object_reserved(
            &reservation,
            &receipt_bytes,
            None,
            JSON_MIME,
            RECEIPT_KIND,
            &record.created_at,
        )?;
        record.receipt_object_sha256 = receipt.record.sha256.clone();
        match self
            .store
            .record_production_weapon_form_quality_with_replay(&record, &receipt.record)
        {
            Ok((stored, replayed)) => {
                release(self, &reservation, std::slice::from_ref(&receipt), false);
                result_value(
                    &stored,
                    replayed,
                    PRODUCTION_WEAPON_FORM_QUALITY_PREPARE_RESULT_SCHEMA_VERSION,
                    true,
                )
            }
            Err(error) => {
                release(self, &reservation, std::slice::from_ref(&receipt), true);
                Err(error.into())
            }
        }
    }

    pub fn production_weapon_form_quality_get(&self, value: Value) -> Result<Value, RuntimeError> {
        let request = parse_get(&value)?;
        let record = self
            .store
            .get_production_weapon_form_quality(&request.form_quality_id)?
            .ok_or_else(|| invalid("form quality is unavailable"))?;
        if record.session_id != request.session_id
            || record.project_id != request.project_id
            || record.candidate_id != request.candidate_id
            || record.form_stage != request.form_stage
        {
            return Err(invalid("form quality scope differs"));
        }
        let receipt_bytes = self.cas_read(&record.receipt_object_sha256)?;
        let mut expected_receipt =
            serde_json::to_value(&record).map_err(|error| invalid(error.to_string()))?;
        expected_receipt["receipt_object_sha256"] = Value::String(String::new());
        let expected_receipt_bytes =
            canonical_json_bytes(&expected_receipt).map_err(|error| invalid(error.to_string()))?;
        if receipt_bytes != expected_receipt_bytes {
            return Err(invalid("form quality receipt bytes differ"));
        }
        let request_value =
            serde_json::to_value(&record).map_err(|error| invalid(error.to_string()))?;
        let mut normalized = request_value.clone();
        normalized["canonical_sha256"] = Value::String(String::new());
        normalized["receipt_object_sha256"] = Value::String(String::new());
        if canonical_json_hash(&normalized) != record.canonical_sha256 {
            return Err(invalid("form quality canonical hash differs"));
        }
        // Rebuild the source gate from the immutable record.  This performs the
        // same stage/CameraLock/CrossView/CAS replay without starting a Worker.
        let mut prepare = request_value;
        prepare["schema_version"] =
            Value::String(PRODUCTION_WEAPON_FORM_QUALITY_PREPARE_REQUEST_SCHEMA_VERSION.into());
        prepare["input_sha256"] = Value::String(record.input_sha256.clone());
        prepare["idempotency_key"] = Value::String(record.form_quality_id.clone());
        let request: ProductionWeaponFormQualityPrepareRequest = serde_json::from_value(prepare)
            .map_err(|error| invalid(format!("stored form quality cannot replay: {error}")))?;
        let recomputed = record_from_request(self, request, &record.request_sha256)?;
        if recomputed.canonical_sha256 != record.canonical_sha256
            || recomputed.hard_gate != record.hard_gate
            || recomputed.form_view_evaluations != record.form_view_evaluations
        {
            return Err(invalid("form quality receipt is tampered"));
        }
        result_value(
            &record,
            true,
            PRODUCTION_WEAPON_FORM_QUALITY_GET_RESULT_SCHEMA_VERSION,
            false,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn policy_hashes_are_stable() {
        assert_eq!(
            sha256_hex(PRODUCTION_WEAPON_FORM_QUALITY_POLICY.as_bytes()).len(),
            64
        );
        assert_eq!(
            sha256_hex(PRODUCTION_WEAPON_FORM_QUALITY_THRESHOLD_POLICY.as_bytes()).len(),
            64
        );
    }

    #[test]
    fn form_edges_bind_to_the_current_stage_head() {
        assert_eq!(
            expected_source_head_stage("blockout").expect("blockout source head"),
            "camera-calibrated"
        );
        assert_eq!(
            expected_source_head_stage("primary").expect("primary source head"),
            "blockout-reviewed"
        );
        assert_eq!(
            expected_source_head_stage("secondary").expect("secondary source head"),
            "primary-form-approved"
        );
        assert!(expected_source_head_stage("high-poly").is_err());
    }

    #[test]
    fn closed_get_rejects_unknown_fields() {
        let value = serde_json::json!({"schema_version":PRODUCTION_WEAPON_FORM_QUALITY_GET_REQUEST_SCHEMA_VERSION,"form_quality_id":"fq-1","session_id":"s-1","project_id":"p-1","candidate_id":"c-1","form_stage":"blockout","unknown":true});
        assert!(parse_get(&value).is_err());
    }

    #[test]
    fn closed_prepare_rejects_unknown_fields_before_deserialization() {
        let mut object = Map::new();
        for field in PREPARE_FIELDS {
            object.insert((*field).to_owned(), Value::Null);
        }
        object.insert("raw_png_bytes".to_owned(), Value::String("nope".to_owned()));
        let error = parse_prepare(&Value::Object(object)).expect_err("unknown field rejected");
        assert!(error
            .to_string()
            .contains("unsupported field raw_png_bytes"));
    }

    #[test]
    fn reference_canvas_view_binding_rejects_cross_view_retarget() {
        let canvas = serde_json::json!({
            "views":[{
                "view_id":"view-front",
                "kind":"front",
                "reference_id":"reference-front",
                "reference_sha256":"a".repeat(64)
            }]
        });
        let indexed = index_reference_canvas_views(&canvas).expect("canvas view index");
        validate_reference_canvas_view_binding(
            &indexed,
            "view-front",
            "front",
            "reference-front",
            &"a".repeat(64),
        )
        .expect("exact canvas binding");
        assert!(validate_reference_canvas_view_binding(
            &indexed,
            "view-front",
            "front",
            "reference-other",
            &"a".repeat(64),
        )
        .is_err());
        assert!(validate_reference_canvas_view_binding(
            &indexed,
            "view-front",
            "back",
            "reference-front",
            &"a".repeat(64),
        )
        .is_err());
    }

    #[test]
    fn comparison_report_rejects_candidate_retarget_with_same_artifact() {
        let render = serde_json::json!({
            "candidate_id":"candidate-form",
            "artifact_sha256":"a".repeat(64)
        });
        let exact = serde_json::json!({
            "candidate_id":"candidate-form",
            "artifact_sha256":"a".repeat(64),
            "reference_id":"reference-front",
            "reference_sha256":"b".repeat(64),
            "camera_hash":"c".repeat(64),
            "render_set_hash":"d".repeat(64),
            "view_id":"view-front"
        });
        validate_comparison_render_binding(
            "proposal",
            &exact,
            &render,
            &"d".repeat(64),
            "reference-front",
            &"b".repeat(64),
            &"c".repeat(64),
            "view-front",
            Some("candidate-form"),
            Some(&"a".repeat(64)),
        )
        .expect("exact comparison binding");

        let mut retargeted = exact;
        retargeted["candidate_id"] = Value::String("candidate-other".into());
        let error = validate_comparison_render_binding(
            "proposal",
            &retargeted,
            &render,
            &"d".repeat(64),
            "reference-front",
            &"b".repeat(64),
            &"c".repeat(64),
            "view-front",
            Some("candidate-form"),
            Some(&"a".repeat(64)),
        )
        .expect_err("candidate retarget rejected");
        assert!(error
            .to_string()
            .contains("candidate/artifact differs from RenderSet"));
    }
}
