//! Read-only authority preflight for the CameraLock registration child.
//!
//! The 180-degree signal used by the D1 diagnostics is kept explicitly on the
//! inferred side of the authority boundary.  This module never materializes a
//! child, writes a receipt, advances a stage or starts a Worker.  A durable
//! promotable child is the only readback that can make the user-approved side
//! true.

use super::{
    canonical_json_bytes, canonical_json_hash, is_opaque_id, is_sha256, Runtime, RuntimeError,
};
use crate::agentic_session::{
    materialize_production_camera_lock_registered_rig,
    read_production_camera_lock_geometry_program, validate_production_camera_lock_record,
    validate_production_camera_lock_registration_lineage_runtime,
};
use forgecad_contracts::{
    ProductionCameraLockRegistrationLineagePreflightGetRequest,
    ProductionCameraLockRegistrationLineagePreflightGetResult,
    ProductionCameraLockRegistrationLineagePreflightProjectionGetRequest,
    ProductionCameraLockRegistrationLineagePreflightProjectionGetResult,
    ProductionCameraLockRegistrationLineagePreflightProjectionProof,
    PRODUCTION_CAMERA_LOCK_REGISTRATION_LINEAGE_PREFLIGHT_GET_OPERATION,
    PRODUCTION_CAMERA_LOCK_REGISTRATION_LINEAGE_PREFLIGHT_GET_REQUEST_SCHEMA_VERSION,
    PRODUCTION_CAMERA_LOCK_REGISTRATION_LINEAGE_PREFLIGHT_GET_RESULT_SCHEMA_VERSION,
    PRODUCTION_CAMERA_LOCK_REGISTRATION_LINEAGE_PREFLIGHT_POLICY,
    PRODUCTION_CAMERA_LOCK_REGISTRATION_LINEAGE_PREFLIGHT_PROJECTION_GET_OPERATION,
    PRODUCTION_CAMERA_LOCK_REGISTRATION_LINEAGE_PREFLIGHT_PROJECTION_GET_REQUEST_SCHEMA_VERSION,
    PRODUCTION_CAMERA_LOCK_REGISTRATION_LINEAGE_PREFLIGHT_PROJECTION_GET_RESULT_SCHEMA_VERSION,
    PRODUCTION_CAMERA_LOCK_REGISTRATION_LINEAGE_PREFLIGHT_PROJECTION_POLICY,
    PRODUCTION_CAMERA_LOCK_REGISTRATION_LINEAGE_WRITER_POLICY,
};
use serde_json::{Map, Value};

const MAX_RESPONSE_BYTES: u64 = 1_048_576;
const REQUEST_FIELDS: &[&str] = &[
    "schema_version",
    "operation",
    "preflight_id",
    "registration_lineage_id",
    "session_id",
    "project_id",
    "candidate_id",
    "candidate_state_sha256",
    "camera_lock_id",
    "camera_lock_canonical_sha256",
    "diagnostic_inferred_rotation_degrees",
    "max_response_bytes",
    "writer_policy",
    "runtime_write_performed",
    "persistent_user_data_touched",
    "input_sha256",
];

const PROJECTION_REQUEST_FIELDS: &[&str] = &[
    "schema_version",
    "operation",
    "preflight_id",
    "registration_lineage_id",
    "session_id",
    "project_id",
    "candidate_id",
    "candidate_state_sha256",
    "camera_lock_id",
    "camera_lock_canonical_sha256",
    "proposed_board_rotation_degrees",
    "proposed_subject_screen_order",
    "max_response_bytes",
    "writer_policy",
    "runtime_write_performed",
    "persistent_user_data_touched",
    "input_sha256",
];

const BLOCKER_CAMERA_LOCK_MISSING: &str = "CAMERA_LOCK_MISSING";
const BLOCKER_CAMERA_LOCK_READ_UNAVAILABLE: &str = "CAMERA_LOCK_READ_UNAVAILABLE";
const BLOCKER_CAMERA_LOCK_INVALID: &str = "CAMERA_LOCK_INVALID";
const BLOCKER_LINEAGE_READ_UNAVAILABLE: &str = "LINEAGE_READBACK_UNAVAILABLE";
const BLOCKER_USER_APPROVAL_REQUIRED: &str = "ORIENTATION_SPECIFIC_USER_RECEIPT_REQUIRED";
const BLOCKER_DIAGNOSTIC_NOT_PROMOTABLE: &str = "DIAGNOSTIC_TRANSFORM_NOT_PROMOTABLE";
const BLOCKER_LINEAGE_NOT_CREATED: &str = "REAL_D1_LINEAGE_NOT_CREATED";
const BLOCKER_SEMANTIC_CAMERA_PREVIEW_UNAVAILABLE: &str = "SEMANTIC_CAMERA_PREVIEW_UNAVAILABLE";
const BLOCKER_SEMANTIC_CAMERA_PROOF_FAILED: &str = "SEMANTIC_CAMERA_PROOF_FAILED";

fn invalid(message: impl Into<String>) -> RuntimeError {
    RuntimeError::InvalidInput(message.into())
}

fn required_text<'a>(object: &'a Map<String, Value>, field: &str) -> Result<&'a str, RuntimeError> {
    object
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| invalid(format!("{field} is required")))
}

fn parse_request(
    value: &Value,
) -> Result<ProductionCameraLockRegistrationLineagePreflightGetRequest, RuntimeError> {
    let object = value.as_object().ok_or_else(|| {
        invalid("PRODUCTION_CAMERA_LOCK_REGISTRATION_LINEAGE_PREFLIGHT_REQUEST_OBJECT_REQUIRED")
    })?;
    if let Some(field) = object
        .keys()
        .find(|field| !REQUEST_FIELDS.contains(&field.as_str()))
    {
        return Err(invalid(format!(
            "PRODUCTION_CAMERA_LOCK_REGISTRATION_LINEAGE_PREFLIGHT_UNSUPPORTED_FIELD: {field}"
        )));
    }
    if required_text(object, "schema_version")?
        != PRODUCTION_CAMERA_LOCK_REGISTRATION_LINEAGE_PREFLIGHT_GET_REQUEST_SCHEMA_VERSION
        || required_text(object, "operation")?
            != PRODUCTION_CAMERA_LOCK_REGISTRATION_LINEAGE_PREFLIGHT_GET_OPERATION
    {
        return Err(invalid(
            "PRODUCTION_CAMERA_LOCK_REGISTRATION_LINEAGE_PREFLIGHT_SCHEMA_OR_OPERATION_MISMATCH",
        ));
    }
    let request: ProductionCameraLockRegistrationLineagePreflightGetRequest =
        serde_json::from_value(value.clone()).map_err(|error| {
            invalid(format!(
                "PRODUCTION_CAMERA_LOCK_REGISTRATION_LINEAGE_PREFLIGHT_REQUEST_INVALID: {error}"
            ))
        })?;
    for (field, value) in [
        ("preflight_id", request.preflight_id.as_str()),
        (
            "registration_lineage_id",
            request.registration_lineage_id.as_str(),
        ),
        ("session_id", request.session_id.as_str()),
        ("project_id", request.project_id.as_str()),
        ("candidate_id", request.candidate_id.as_str()),
        ("camera_lock_id", request.camera_lock_id.as_str()),
    ] {
        if !is_opaque_id(value) {
            return Err(invalid(format!(
                "PRODUCTION_CAMERA_LOCK_REGISTRATION_LINEAGE_PREFLIGHT_{field}_INVALID"
            )));
        }
    }
    for (field, value) in [
        (
            "candidate_state_sha256",
            request.candidate_state_sha256.as_str(),
        ),
        (
            "camera_lock_canonical_sha256",
            request.camera_lock_canonical_sha256.as_str(),
        ),
        ("input_sha256", request.input_sha256.as_str()),
    ] {
        if !is_sha256(value) {
            return Err(invalid(format!(
                "PRODUCTION_CAMERA_LOCK_REGISTRATION_LINEAGE_PREFLIGHT_{field}_INVALID"
            )));
        }
    }
    if request.max_response_bytes != MAX_RESPONSE_BYTES
        || request.writer_policy != PRODUCTION_CAMERA_LOCK_REGISTRATION_LINEAGE_WRITER_POLICY
        || request.runtime_write_performed
        || request.persistent_user_data_touched
        || ![-180, -90, 0, 90, 180].contains(&request.diagnostic_inferred_rotation_degrees)
    {
        return Err(invalid(
            "PRODUCTION_CAMERA_LOCK_REGISTRATION_LINEAGE_PREFLIGHT_POLICY_MISMATCH",
        ));
    }
    let mut input_preimage = object.clone();
    input_preimage.remove("input_sha256");
    if canonical_json_hash(&Value::Object(input_preimage)) != request.input_sha256 {
        return Err(invalid(
            "PRODUCTION_CAMERA_LOCK_REGISTRATION_LINEAGE_PREFLIGHT_INPUT_HASH_MISMATCH",
        ));
    }
    Ok(request)
}

fn parse_projection_request(
    value: &Value,
) -> Result<ProductionCameraLockRegistrationLineagePreflightProjectionGetRequest, RuntimeError> {
    let object = value.as_object().ok_or_else(|| {
        invalid(
            "PRODUCTION_CAMERA_LOCK_REGISTRATION_LINEAGE_PREFLIGHT_PROJECTION_REQUEST_OBJECT_REQUIRED",
        )
    })?;
    if let Some(field) = object
        .keys()
        .find(|field| !PROJECTION_REQUEST_FIELDS.contains(&field.as_str()))
    {
        return Err(invalid(format!(
            "PRODUCTION_CAMERA_LOCK_REGISTRATION_LINEAGE_PREFLIGHT_PROJECTION_UNSUPPORTED_FIELD: {field}"
        )));
    }
    if required_text(object, "schema_version")?
        != PRODUCTION_CAMERA_LOCK_REGISTRATION_LINEAGE_PREFLIGHT_PROJECTION_GET_REQUEST_SCHEMA_VERSION
        || required_text(object, "operation")?
            != PRODUCTION_CAMERA_LOCK_REGISTRATION_LINEAGE_PREFLIGHT_PROJECTION_GET_OPERATION
    {
        return Err(invalid(
            "PRODUCTION_CAMERA_LOCK_REGISTRATION_LINEAGE_PREFLIGHT_PROJECTION_SCHEMA_OR_OPERATION_MISMATCH",
        ));
    }
    let request: ProductionCameraLockRegistrationLineagePreflightProjectionGetRequest =
        serde_json::from_value(value.clone()).map_err(|error| {
            invalid(format!(
                "PRODUCTION_CAMERA_LOCK_REGISTRATION_LINEAGE_PREFLIGHT_PROJECTION_REQUEST_INVALID: {error}"
            ))
        })?;
    for (field, value) in [
        ("preflight_id", request.preflight_id.as_str()),
        (
            "registration_lineage_id",
            request.registration_lineage_id.as_str(),
        ),
        ("session_id", request.session_id.as_str()),
        ("project_id", request.project_id.as_str()),
        ("candidate_id", request.candidate_id.as_str()),
        ("camera_lock_id", request.camera_lock_id.as_str()),
    ] {
        if !is_opaque_id(value) {
            return Err(invalid(format!(
                "PRODUCTION_CAMERA_LOCK_REGISTRATION_LINEAGE_PREFLIGHT_PROJECTION_{field}_INVALID"
            )));
        }
    }
    for (field, value) in [
        (
            "candidate_state_sha256",
            request.candidate_state_sha256.as_str(),
        ),
        (
            "camera_lock_canonical_sha256",
            request.camera_lock_canonical_sha256.as_str(),
        ),
        ("input_sha256", request.input_sha256.as_str()),
    ] {
        if !is_sha256(value) {
            return Err(invalid(format!(
                "PRODUCTION_CAMERA_LOCK_REGISTRATION_LINEAGE_PREFLIGHT_PROJECTION_{field}_INVALID"
            )));
        }
    }
    if request.max_response_bytes != MAX_RESPONSE_BYTES
        || request.writer_policy != PRODUCTION_CAMERA_LOCK_REGISTRATION_LINEAGE_WRITER_POLICY
        || request.runtime_write_performed
        || request.persistent_user_data_touched
        || ![-180, -90, 0, 90, 180].contains(&request.proposed_board_rotation_degrees)
        || !["stock-left-muzzle-right", "muzzle-left-stock-right"]
            .contains(&request.proposed_subject_screen_order.as_str())
    {
        return Err(invalid(
            "PRODUCTION_CAMERA_LOCK_REGISTRATION_LINEAGE_PREFLIGHT_PROJECTION_POLICY_MISMATCH",
        ));
    }
    let mut input_preimage = object.clone();
    input_preimage.remove("input_sha256");
    if canonical_json_hash(&Value::Object(input_preimage)) != request.input_sha256 {
        return Err(invalid(
            "PRODUCTION_CAMERA_LOCK_REGISTRATION_LINEAGE_PREFLIGHT_PROJECTION_INPUT_HASH_MISMATCH",
        ));
    }
    Ok(request)
}

fn result_value(
    result: &ProductionCameraLockRegistrationLineagePreflightGetResult,
) -> Result<Value, RuntimeError> {
    serde_json::to_value(result)
        .map_err(|error| invalid(format!("PRODUCTION_CAMERA_LOCK_REGISTRATION_LINEAGE_PREFLIGHT_RESULT_SERIALIZE_FAILED: {error}")))
}

fn projection_result_value(
    result: &ProductionCameraLockRegistrationLineagePreflightProjectionGetResult,
) -> Result<Value, RuntimeError> {
    serde_json::to_value(result).map_err(|error| {
        invalid(format!(
            "PRODUCTION_CAMERA_LOCK_REGISTRATION_LINEAGE_PREFLIGHT_PROJECTION_RESULT_SERIALIZE_FAILED: {error}"
        ))
    })
}

fn read_canonical_json_object(
    runtime: &Runtime,
    object_sha256: &str,
    label: &str,
) -> Result<Value, RuntimeError> {
    let bytes = runtime.cas_read_bounded(object_sha256, MAX_RESPONSE_BYTES)?;
    let value: Value = serde_json::from_slice(&bytes)
        .map_err(|error| invalid(format!("{label}_INVALID_JSON: {error}")))?;
    if canonical_json_bytes(&value)
        .map_err(|error| invalid(format!("{label}_CANONICALIZE_FAILED: {error}")))?
        != bytes
    {
        return Err(invalid(format!("{label}_NOT_CANONICAL")));
    }
    Ok(value)
}

impl Runtime {
    /// Read the exact CameraLock scope and an optional already-materialized
    /// child.  The diagnostic rotation is returned as inferred-only input; it
    /// never participates in a successful lineage decision.
    pub fn production_camera_lock_registration_lineage_preflight_get(
        &self,
        request_value: Value,
    ) -> Result<Value, RuntimeError> {
        let request = parse_request(&request_value)?;
        let mut blocking_reasons = Vec::new();
        let mut parent_status = "MISSING".to_owned();
        let mut parent_receipt = None;
        let mut durable_lineage_status = "NOT_CREATED".to_owned();
        let mut existing_promotable_lineage = false;
        let mut user_approved_orientation = false;
        let mut user_approved_source = "none".to_owned();

        let lock = match self
            .store
            .get_production_camera_lock(&request.camera_lock_id)
        {
            Ok(lock) => lock,
            Err(_) => {
                parent_status = "UNAVAILABLE".to_owned();
                blocking_reasons.push(BLOCKER_CAMERA_LOCK_READ_UNAVAILABLE.to_owned());
                None
            }
        };
        if let Some(lock) = lock {
            if lock.camera_lock_id != request.camera_lock_id
                || lock.canonical_sha256 != request.camera_lock_canonical_sha256
                || lock.session_id != request.session_id
                || lock.project_id != request.project_id
                || lock.candidate_id != request.candidate_id
                || lock.candidate_state_sha256 != request.candidate_state_sha256
            {
                return Err(invalid(
                    "PRODUCTION_CAMERA_LOCK_REGISTRATION_LINEAGE_PREFLIGHT_SCOPE_MISMATCH",
                ));
            }
            parent_receipt = Some(lock.receipt_object_sha256.clone());
            parent_status = match validate_production_camera_lock_record(self, &lock) {
                Ok(()) => "VALID".to_owned(),
                Err(_) => {
                    blocking_reasons.push(BLOCKER_CAMERA_LOCK_INVALID.to_owned());
                    "INVALID".to_owned()
                }
            };
            if parent_status == "VALID" {
                match self.store.get_production_camera_lock_registration_lineage(
                    &request.registration_lineage_id,
                ) {
                    Ok(Some(lineage)) => {
                        if lineage.camera_lock_id != request.camera_lock_id
                            || lineage.camera_lock_canonical_sha256
                                != request.camera_lock_canonical_sha256
                            || lineage.session_id != request.session_id
                            || lineage.project_id != request.project_id
                            || lineage.candidate_id != request.candidate_id
                            || lineage.candidate_state_sha256 != request.candidate_state_sha256
                        {
                            return Err(invalid(
                                "PRODUCTION_CAMERA_LOCK_REGISTRATION_LINEAGE_PREFLIGHT_EXISTING_LINEAGE_SCOPE_MISMATCH",
                            ));
                        }
                        if lineage.promotable {
                            durable_lineage_status = "EXISTING_PROMOTABLE".to_owned();
                            existing_promotable_lineage = true;
                            user_approved_orientation = true;
                            user_approved_source =
                                "durable-lineage-orientation-approval".to_owned();
                        } else {
                            durable_lineage_status = "EXISTING_BLOCKED".to_owned();
                            blocking_reasons.push(BLOCKER_USER_APPROVAL_REQUIRED.to_owned());
                        }
                    }
                    Ok(None) => {
                        blocking_reasons.push(BLOCKER_USER_APPROVAL_REQUIRED.to_owned());
                        blocking_reasons.push(BLOCKER_DIAGNOSTIC_NOT_PROMOTABLE.to_owned());
                        blocking_reasons.push(BLOCKER_LINEAGE_NOT_CREATED.to_owned());
                    }
                    Err(_) => {
                        durable_lineage_status = "STORE_UNAVAILABLE".to_owned();
                        blocking_reasons.push(BLOCKER_LINEAGE_READ_UNAVAILABLE.to_owned());
                    }
                }
            }
        } else if !blocking_reasons
            .iter()
            .any(|reason| reason == BLOCKER_CAMERA_LOCK_READ_UNAVAILABLE)
        {
            blocking_reasons.push(BLOCKER_CAMERA_LOCK_MISSING.to_owned());
            blocking_reasons.push(BLOCKER_USER_APPROVAL_REQUIRED.to_owned());
            blocking_reasons.push(BLOCKER_DIAGNOSTIC_NOT_PROMOTABLE.to_owned());
            blocking_reasons.push(BLOCKER_LINEAGE_NOT_CREATED.to_owned());
        }
        blocking_reasons.sort();
        blocking_reasons.dedup();
        let ready = parent_status == "VALID" && existing_promotable_lineage;
        if ready {
            blocking_reasons.clear();
        }
        let mut result = ProductionCameraLockRegistrationLineagePreflightGetResult {
            schema_version:
                PRODUCTION_CAMERA_LOCK_REGISTRATION_LINEAGE_PREFLIGHT_GET_RESULT_SCHEMA_VERSION
                    .to_owned(),
            operation: PRODUCTION_CAMERA_LOCK_REGISTRATION_LINEAGE_PREFLIGHT_GET_OPERATION
                .to_owned(),
            preflight_id: request.preflight_id,
            registration_lineage_id: request.registration_lineage_id,
            session_id: request.session_id,
            project_id: request.project_id,
            candidate_id: request.candidate_id,
            candidate_state_sha256: request.candidate_state_sha256,
            camera_lock_id: request.camera_lock_id,
            camera_lock_canonical_sha256: request.camera_lock_canonical_sha256,
            parent_camera_lock_status: parent_status,
            parent_camera_lock_receipt_object_sha256: parent_receipt,
            durable_lineage_status,
            existing_promotable_lineage_present: existing_promotable_lineage,
            user_approved_orientation_present: user_approved_orientation,
            user_approved_orientation_source: user_approved_source,
            diagnostic_inferred_orientation_present: true,
            diagnostic_inferred_rotation_degrees: request.diagnostic_inferred_rotation_degrees,
            diagnostic_orientation_source: "diagnostic-transform-discovery".to_owned(),
            orientation_authority_status: if ready {
                "USER_APPROVED_DURABLE_LINEAGE".to_owned()
            } else {
                "BLOCKED_USER_APPROVAL_REQUIRED".to_owned()
            },
            ready_for_promotable_lineage: ready,
            blocking_reasons,
            policy: PRODUCTION_CAMERA_LOCK_REGISTRATION_LINEAGE_PREFLIGHT_POLICY.to_owned(),
            writer_policy: PRODUCTION_CAMERA_LOCK_REGISTRATION_LINEAGE_WRITER_POLICY.to_owned(),
            runtime_write: false,
            persistent_user_data_touched: false,
            worker_started: false,
            production_stage_advanced: false,
            candidate_confirmed: false,
            version_created: false,
            export_performed: false,
            restart_hash_verified: false,
            readiness_sha256: String::new(),
        };
        let mut preimage = result_value(&result)?;
        preimage["readiness_sha256"] = Value::String(String::new());
        result.readiness_sha256 = canonical_json_hash(&preimage);
        result_value(&result)
    }
}

impl Runtime {
    /// Project the exact rear-three-quarter semantic camera that can be shown
    /// to a user before approval. Camera orbit, camera hashes, semantic anchors
    /// and upright proof are Runtime-derived; this method is strictly zero-write.
    pub fn production_camera_lock_registration_lineage_preflight_projection_get(
        &self,
        request_value: Value,
    ) -> Result<Value, RuntimeError> {
        let request = parse_projection_request(&request_value)?;
        let mut parent_status = "MISSING".to_owned();
        let mut parent_receipt = None;
        let mut derived_camera_orbit_degrees = None;
        let mut derived_camera_hash = None;
        let mut derived_camera_canonical_sha256 = None;
        let mut upright_proof = None;
        let mut projection_status = "UNAVAILABLE".to_owned();
        let mut projection_input_sha256 = None;
        let mut projection_ready_for_user_review = false;
        let mut existing_lineage_status = "NOT_CREATED".to_owned();
        let mut existing_promotable_lineage_present = false;
        let mut existing_lineage_matches_proposal = false;
        let mut orientation_authority_status = "BLOCKED_USER_APPROVAL_REQUIRED".to_owned();
        let mut blocking_reasons = Vec::new();

        let lock = match self
            .store
            .get_production_camera_lock(&request.camera_lock_id)
        {
            Ok(lock) => lock,
            Err(_) => {
                parent_status = "UNAVAILABLE".to_owned();
                blocking_reasons.push(BLOCKER_CAMERA_LOCK_READ_UNAVAILABLE.to_owned());
                None
            }
        };
        if let Some(lock) = lock {
            if lock.camera_lock_id != request.camera_lock_id
                || lock.canonical_sha256 != request.camera_lock_canonical_sha256
                || lock.session_id != request.session_id
                || lock.project_id != request.project_id
                || lock.candidate_id != request.candidate_id
                || lock.candidate_state_sha256 != request.candidate_state_sha256
            {
                return Err(invalid(
                    "PRODUCTION_CAMERA_LOCK_REGISTRATION_LINEAGE_PREFLIGHT_PROJECTION_SCOPE_MISMATCH",
                ));
            }
            parent_receipt = Some(lock.receipt_object_sha256.clone());
            parent_status = match validate_production_camera_lock_record(self, &lock) {
                Ok(()) => "VALID".to_owned(),
                Err(_) => {
                    blocking_reasons.push(BLOCKER_CAMERA_LOCK_INVALID.to_owned());
                    "INVALID".to_owned()
                }
            };
            if parent_status == "VALID" {
                let projection =
                    (|| -> Result<(Value, Value, String, Value, Value), RuntimeError> {
                        let subject_rig = read_canonical_json_object(
                        self,
                        &lock.camera_rig_object_sha256,
                        "PRODUCTION_CAMERA_LOCK_REGISTRATION_LINEAGE_PREFLIGHT_PROJECTION_SUBJECT_RIG",
                    )?;
                        if subject_rig.get("canonical_sha256").and_then(Value::as_str)
                            != Some(lock.camera_rig_canonical_sha256.as_str())
                        {
                            return Err(invalid(
                            "PRODUCTION_CAMERA_LOCK_REGISTRATION_LINEAGE_PREFLIGHT_PROJECTION_SUBJECT_RIG_HASH_MISMATCH",
                        ));
                        }
                        let registered_rig = materialize_production_camera_lock_registered_rig(
                            self,
                            &lock.project_id,
                            &lock.candidate_id,
                            &lock.candidate_state_sha256,
                            &lock.artifact_id,
                            &lock.artifact_sha256,
                            &subject_rig,
                            &lock.camera_rig_object_sha256,
                        )?;
                        let (program, program_object_sha256) =
                            read_production_camera_lock_geometry_program(
                                self,
                                &lock.project_id,
                                &lock.candidate_id,
                                &lock.artifact_sha256,
                            )?;
                        let lock_value = serde_json::to_value(&lock).map_err(|error| {
                        invalid(format!(
                            "PRODUCTION_CAMERA_LOCK_REGISTRATION_LINEAGE_PREFLIGHT_PROJECTION_PARENT_SERIALIZE_FAILED: {error}"
                        ))
                    })?;
                        let semantic_ordering = crate::multiview::camera_rig::materialize_production_weapon_semantic_landmark_ordering(
                        &registered_rig,
                        &lock_value,
                        &program,
                        "semantic-camera-preflight-projection",
                    )
                    .map_err(invalid)?;
                        let preview = crate::multiview::camera_rig::materialize_rear_three_quarter_semantic_camera_preview(
                        &registered_rig,
                        &semantic_ordering,
                        &request.proposed_subject_screen_order,
                    )
                    .map_err(invalid)?;
                        Ok((
                            registered_rig,
                            program,
                            program_object_sha256,
                            semantic_ordering,
                            preview,
                        ))
                    })();
                match projection {
                    Ok((
                        registered_rig,
                        program,
                        program_object_sha256,
                        semantic_ordering,
                        preview,
                    )) => {
                        let preview_hash = preview["canonical_sha256"]
                            .as_str()
                            .ok_or_else(|| {
                                invalid("SEMANTIC_CAMERA_PREFLIGHT_PREVIEW_HASH_MISSING")
                            })?
                            .to_owned();
                        let derived_orbit = preview
                            .get("camera_orbit_degrees")
                            .and_then(Value::as_i64)
                            .ok_or_else(|| invalid("SEMANTIC_CAMERA_PREFLIGHT_ORBIT_MISSING"))?;
                        let camera_hash = preview
                            .get("derived_registered_camera_hash")
                            .and_then(Value::as_str)
                            .filter(|hash| is_sha256(hash))
                            .ok_or_else(|| {
                                invalid("SEMANTIC_CAMERA_PREFLIGHT_CAMERA_HASH_MISSING")
                            })?
                            .to_owned();
                        let camera_canonical = preview
                            .get("derived_registered_camera_canonical_sha256")
                            .and_then(Value::as_str)
                            .filter(|hash| is_sha256(hash))
                            .ok_or_else(|| {
                                invalid("SEMANTIC_CAMERA_PREFLIGHT_CAMERA_CANONICAL_HASH_MISSING")
                            })?
                            .to_owned();
                        let proof: ProductionCameraLockRegistrationLineagePreflightProjectionProof =
                            serde_json::from_value(preview["upright_proof"].clone()).map_err(
                                |error| {
                                    invalid(format!(
                                        "SEMANTIC_CAMERA_PREFLIGHT_PROOF_INVALID: {error}"
                                    ))
                                },
                            )?;
                        let proof_passed = proof.passed;
                        let projection_input = serde_json::json!({
                            "camera_lock_canonical_sha256":lock.canonical_sha256,
                            "candidate_state_sha256":lock.candidate_state_sha256,
                            "subject_camera_rig_object_sha256":lock.camera_rig_object_sha256,
                            "subject_camera_rig_canonical_sha256":lock.camera_rig_canonical_sha256,
                            "geometry_program_object_sha256":program_object_sha256,
                            "geometry_program_sha256":program["canonical_sha256"],
                            "registered_rig_v1_canonical_sha256":registered_rig["canonical_sha256"],
                            "semantic_ordering_canonical_sha256":semantic_ordering["canonical_sha256"],
                            "proposed_board_rotation_degrees":request.proposed_board_rotation_degrees,
                            "proposed_subject_screen_order":request.proposed_subject_screen_order,
                            "derived_camera_orbit_degrees":derived_orbit,
                            "derived_camera_hash":camera_hash,
                            "derived_camera_canonical_sha256":camera_canonical,
                            "preview_canonical_sha256":preview_hash,
                            "projection_policy":PRODUCTION_CAMERA_LOCK_REGISTRATION_LINEAGE_PREFLIGHT_PROJECTION_POLICY
                        });
                        projection_input_sha256 = Some(canonical_json_hash(&projection_input));
                        derived_camera_orbit_degrees = Some(derived_orbit);
                        derived_camera_hash = Some(camera_hash);
                        derived_camera_canonical_sha256 = Some(camera_canonical);
                        upright_proof = Some(proof);
                        projection_ready_for_user_review = proof_passed;
                        projection_status =
                            if proof_passed { "READY" } else { "BLOCKED" }.to_owned();
                        if !proof_passed {
                            blocking_reasons.push(BLOCKER_SEMANTIC_CAMERA_PROOF_FAILED.to_owned());
                        }
                    }
                    Err(_) => {
                        projection_status = "UNAVAILABLE".to_owned();
                        blocking_reasons
                            .push(BLOCKER_SEMANTIC_CAMERA_PREVIEW_UNAVAILABLE.to_owned());
                    }
                }

                match self.store.get_production_camera_lock_registration_lineage(
                    &request.registration_lineage_id,
                ) {
                    Ok(Some(lineage)) => {
                        existing_promotable_lineage_present = lineage.promotable;
                        if validate_production_camera_lock_registration_lineage_runtime(
                            self, &lineage,
                        )
                        .is_err()
                        {
                            existing_lineage_status = "EXISTING_INVALID".to_owned();
                            orientation_authority_status =
                                "EXISTING_DURABLE_LINEAGE_INVALID".to_owned();
                            blocking_reasons.push("EXISTING_DURABLE_LINEAGE_INVALID".to_owned());
                        } else {
                            let orientation = read_canonical_json_object(
                                self,
                                &lineage.authored_orientation_object_sha256,
                                "SEMANTIC_CAMERA_PREFLIGHT_EXISTING_ORIENTATION",
                            )?;
                            let rig_v2 = read_canonical_json_object(
                                self,
                                &lineage.registered_rig_v2_object_sha256,
                                "SEMANTIC_CAMERA_PREFLIGHT_EXISTING_RIG_V2",
                            )?;
                            let existing_rear_camera_hash = rig_v2
                                .get("renderer_views")
                                .and_then(Value::as_array)
                                .and_then(|views| {
                                    views.iter().find(|view| {
                                        view.get("kind").and_then(Value::as_str)
                                            == Some("rear-three-quarter")
                                    })
                                })
                                .and_then(|view| {
                                    view.get("registered_camera_hash").and_then(Value::as_str)
                                });
                            existing_lineage_matches_proposal = orientation
                                .pointer("/reference_to_subject_view/rotation_degrees")
                                .and_then(Value::as_i64)
                                == Some(request.proposed_board_rotation_degrees)
                                && orientation
                                    .get("subject_screen_order")
                                    .and_then(Value::as_str)
                                    == Some(request.proposed_subject_screen_order.as_str())
                                && orientation
                                    .pointer("/registered_camera_orbit/yaw_degrees")
                                    .and_then(Value::as_i64)
                                    == derived_camera_orbit_degrees
                                && existing_rear_camera_hash == derived_camera_hash.as_deref();
                            if existing_lineage_matches_proposal {
                                existing_lineage_status = "EXISTING_MATCHING".to_owned();
                                orientation_authority_status =
                                    "USER_APPROVED_MATCHING_DURABLE_LINEAGE".to_owned();
                            } else {
                                existing_lineage_status = "EXISTING_PROPOSAL_MISMATCH".to_owned();
                                orientation_authority_status =
                                    "EXISTING_DURABLE_LINEAGE_PROPOSAL_MISMATCH".to_owned();
                                blocking_reasons
                                    .push("EXISTING_DURABLE_LINEAGE_PROPOSAL_MISMATCH".to_owned());
                            }
                        }
                    }
                    Ok(None) => {
                        blocking_reasons.push(BLOCKER_USER_APPROVAL_REQUIRED.to_owned());
                        blocking_reasons.push(BLOCKER_LINEAGE_NOT_CREATED.to_owned());
                    }
                    Err(_) => {
                        existing_lineage_status = "STORE_UNAVAILABLE".to_owned();
                        blocking_reasons.push(BLOCKER_LINEAGE_READ_UNAVAILABLE.to_owned());
                    }
                }
            } else {
                projection_status = "INVALID".to_owned();
            }
        } else if !blocking_reasons
            .iter()
            .any(|reason| reason == BLOCKER_CAMERA_LOCK_READ_UNAVAILABLE)
        {
            blocking_reasons.push(BLOCKER_CAMERA_LOCK_MISSING.to_owned());
            blocking_reasons.push(BLOCKER_USER_APPROVAL_REQUIRED.to_owned());
            blocking_reasons.push(BLOCKER_LINEAGE_NOT_CREATED.to_owned());
        }

        let ready_for_promotable_lineage = projection_ready_for_user_review
            && existing_promotable_lineage_present
            && existing_lineage_matches_proposal;
        if ready_for_promotable_lineage {
            blocking_reasons.clear();
        }
        blocking_reasons.sort();
        blocking_reasons.dedup();
        let mut result = ProductionCameraLockRegistrationLineagePreflightProjectionGetResult {
            schema_version:
                PRODUCTION_CAMERA_LOCK_REGISTRATION_LINEAGE_PREFLIGHT_PROJECTION_GET_RESULT_SCHEMA_VERSION
                    .to_owned(),
            operation:
                PRODUCTION_CAMERA_LOCK_REGISTRATION_LINEAGE_PREFLIGHT_PROJECTION_GET_OPERATION
                    .to_owned(),
            preflight_id: request.preflight_id,
            registration_lineage_id: request.registration_lineage_id,
            session_id: request.session_id,
            project_id: request.project_id,
            candidate_id: request.candidate_id,
            candidate_state_sha256: request.candidate_state_sha256,
            camera_lock_id: request.camera_lock_id,
            camera_lock_canonical_sha256: request.camera_lock_canonical_sha256,
            parent_camera_lock_status: parent_status,
            parent_camera_lock_receipt_object_sha256: parent_receipt,
            proposed_board_rotation_degrees: request.proposed_board_rotation_degrees,
            proposed_subject_screen_order: request.proposed_subject_screen_order,
            derived_camera_orbit_degrees,
            derived_camera_hash,
            derived_camera_canonical_sha256,
            upright_proof,
            projection_status,
            projection_input_sha256,
            projection_ready_for_user_review,
            existing_lineage_status,
            existing_promotable_lineage_present,
            existing_lineage_matches_proposal,
            orientation_authority_status,
            ready_for_promotable_lineage,
            blocking_reasons,
            policy: PRODUCTION_CAMERA_LOCK_REGISTRATION_LINEAGE_PREFLIGHT_PROJECTION_POLICY
                .to_owned(),
            writer_policy: PRODUCTION_CAMERA_LOCK_REGISTRATION_LINEAGE_WRITER_POLICY.to_owned(),
            runtime_write: false,
            persistent_user_data_touched: false,
            worker_started: false,
            production_stage_advanced: false,
            candidate_confirmed: false,
            version_created: false,
            export_performed: false,
            readiness_sha256: String::new(),
        };
        let mut preimage = projection_result_value(&result)?;
        preimage["readiness_sha256"] = Value::String(String::new());
        result.readiness_sha256 = canonical_json_hash(&preimage);
        projection_result_value(&result)
    }
}
