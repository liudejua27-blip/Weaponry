//! Runtime-owned read-only owner-to-reviewed-void calibration projection.

use super::{canonical_json_hash, is_opaque_id, is_sha256, Runtime, RuntimeError};
use forgecad_contracts::{
    ProductionWeaponOwnerReviewedVoidCalibrationProjectionGetRequest,
    ProductionWeaponOwnerReviewedVoidCalibrationProjectionGetResult,
    PRODUCTION_WEAPON_OWNER_REVIEWED_VOID_CALIBRATION_PROJECTION_CANONICALIZATION_POLICY,
    PRODUCTION_WEAPON_OWNER_REVIEWED_VOID_CALIBRATION_PROJECTION_GET_OPERATION,
    PRODUCTION_WEAPON_OWNER_REVIEWED_VOID_CALIBRATION_PROJECTION_GET_REQUEST_SCHEMA_VERSION,
    PRODUCTION_WEAPON_OWNER_REVIEWED_VOID_CALIBRATION_PROJECTION_GET_RESULT_SCHEMA_VERSION,
    PRODUCTION_WEAPON_OWNER_REVIEWED_VOID_CALIBRATION_PROJECTION_MAX_RESPONSE_BYTES,
    PRODUCTION_WEAPON_OWNER_REVIEWED_VOID_CALIBRATION_PROJECTION_QUALITY_STATUS,
    PRODUCTION_WEAPON_OWNER_REVIEWED_VOID_CALIBRATION_PROJECTION_WRITER_POLICY,
};
use serde_json::Value;

fn invalid(message: impl Into<String>) -> RuntimeError {
    RuntimeError::InvalidInput(message.into())
}

fn parse_request(
    value: &Value,
) -> Result<ProductionWeaponOwnerReviewedVoidCalibrationProjectionGetRequest, RuntimeError> {
    let request: ProductionWeaponOwnerReviewedVoidCalibrationProjectionGetRequest =
        serde_json::from_value(value.clone()).map_err(|error| {
            invalid(format!(
                "PRODUCTION_WEAPON_OWNER_REVIEWED_VOID_CALIBRATION_REQUEST_INVALID: {error}"
            ))
        })?;
    if request.schema_version
        != PRODUCTION_WEAPON_OWNER_REVIEWED_VOID_CALIBRATION_PROJECTION_GET_REQUEST_SCHEMA_VERSION
        || request.operation
            != PRODUCTION_WEAPON_OWNER_REVIEWED_VOID_CALIBRATION_PROJECTION_GET_OPERATION
    {
        return Err(invalid(
            "PRODUCTION_WEAPON_OWNER_REVIEWED_VOID_CALIBRATION_SCHEMA_OR_OPERATION_MISMATCH",
        ));
    }
    for value in [
        request.projection_id.as_str(),
        request.session_id.as_str(),
        request.project_id.as_str(),
        request.candidate_id.as_str(),
        request.artifact_id.as_str(),
        request.form_art_evidence_id.as_str(),
        request.fresh_baseline_id.as_str(),
        request.registration_lineage_id.as_str(),
        request.registered_rig_v2_id.as_str(),
    ] {
        if !is_opaque_id(value) {
            return Err(invalid(
                "PRODUCTION_WEAPON_OWNER_REVIEWED_VOID_CALIBRATION_ID_INVALID",
            ));
        }
    }
    for value in [
        request.candidate_state_sha256.as_str(),
        request.artifact_sha256.as_str(),
        request.artifact_readback_sha256.as_str(),
        request.form_art_evidence_object_sha256.as_str(),
        request.form_art_evidence_canonical_sha256.as_str(),
        request.fresh_baseline_canonical_sha256.as_str(),
        request.fresh_baseline_receipt_object_sha256.as_str(),
        request.registration_lineage_canonical_sha256.as_str(),
        request.registration_lineage_receipt_object_sha256.as_str(),
        request.registered_rig_v2_object_sha256.as_str(),
        request.registered_rig_v2_canonical_sha256.as_str(),
        request.input_sha256.as_str(),
    ] {
        if !is_sha256(value) {
            return Err(invalid(
                "PRODUCTION_WEAPON_OWNER_REVIEWED_VOID_CALIBRATION_HASH_INVALID",
            ));
        }
    }
    if request.max_response_bytes
        != PRODUCTION_WEAPON_OWNER_REVIEWED_VOID_CALIBRATION_PROJECTION_MAX_RESPONSE_BYTES
        || request.writer_policy
            != PRODUCTION_WEAPON_OWNER_REVIEWED_VOID_CALIBRATION_PROJECTION_WRITER_POLICY
        || request.runtime_write_performed
        || request.persistent_user_data_touched
    {
        return Err(invalid(
            "PRODUCTION_WEAPON_OWNER_REVIEWED_VOID_CALIBRATION_POLICY_MISMATCH",
        ));
    }
    let mut input_preimage = value.clone();
    input_preimage
        .as_object_mut()
        .ok_or_else(|| {
            invalid("PRODUCTION_WEAPON_OWNER_REVIEWED_VOID_CALIBRATION_REQUEST_OBJECT_REQUIRED")
        })?
        .remove("input_sha256");
    if canonical_json_hash(&input_preimage) != request.input_sha256 {
        return Err(invalid(
            "PRODUCTION_WEAPON_OWNER_REVIEWED_VOID_CALIBRATION_INPUT_HASH_MISMATCH",
        ));
    }
    Ok(request)
}

impl Runtime {
    pub fn production_weapon_owner_reviewed_void_calibration_get(
        &self,
        request_value: Value,
    ) -> Result<Value, RuntimeError> {
        let request = parse_request(&request_value)?;
        let request_sha256 = canonical_json_hash(&request_value);
        let mut projection = super::production_weapon_form_art_evidence::build_owner_reviewed_void_calibration_projection(
            self,
            &request,
        )?;
        projection.request_sha256 = request_sha256.clone();
        projection.canonical_sha256.clear();
        projection.canonical_sha256 = canonical_json_hash(
            &serde_json::to_value(&projection).map_err(|error| invalid(error.to_string()))?,
        );
        let mut result = ProductionWeaponOwnerReviewedVoidCalibrationProjectionGetResult {
            schema_version: PRODUCTION_WEAPON_OWNER_REVIEWED_VOID_CALIBRATION_PROJECTION_GET_RESULT_SCHEMA_VERSION.to_owned(),
            operation: PRODUCTION_WEAPON_OWNER_REVIEWED_VOID_CALIBRATION_PROJECTION_GET_OPERATION.to_owned(),
            projection_id: request.projection_id,
            projection,
            request_sha256,
            request_input_sha256: request.input_sha256,
            replayed: false,
            restart_hash_verified: false,
            writer_policy: PRODUCTION_WEAPON_OWNER_REVIEWED_VOID_CALIBRATION_PROJECTION_WRITER_POLICY.to_owned(),
            runtime_write: false,
            persistent_user_data_touched: false,
            worker_started: false,
            production_stage_advanced: false,
            candidate_confirmed: false,
            version_created: false,
            export_performed: false,
            quality_status: PRODUCTION_WEAPON_OWNER_REVIEWED_VOID_CALIBRATION_PROJECTION_QUALITY_STATUS.to_owned(),
            depth_status: String::new(),
            canonicalization_policy: PRODUCTION_WEAPON_OWNER_REVIEWED_VOID_CALIBRATION_PROJECTION_CANONICALIZATION_POLICY.to_owned(),
            canonical_sha256: String::new(),
        };
        result.depth_status = result.projection.depth_status.clone();
        result.canonical_sha256 = canonical_json_hash(
            &serde_json::to_value(&result).map_err(|error| invalid(error.to_string()))?,
        );
        serde_json::to_value(result).map_err(|error| invalid(error.to_string()))
    }
}
