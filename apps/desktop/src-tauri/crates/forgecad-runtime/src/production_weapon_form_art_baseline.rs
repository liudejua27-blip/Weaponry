//! Read-only preflight and Runtime-owned producer for a fresh, same-cohort
//! FormArt baseline.
//!
//! This is intentionally separate from the historical FormEvidence/FormArt
//! readers.  Those contracts are bound to the legacy CameraLock/V1 camera
//! rig and may not be used as the source of a new baseline.  This preflight
//! proves that an approved CameraLock registration lineage and its
//! Runtime-owned RigV2 contain an unambiguous six-view camera plan.  The
//! prepare path then renders all six views through the fixed Render Worker and
//! commits their complete evidence graph without advancing production stage.

use super::production_weapon_form_art_baseline_single_flight::{
    BeginError, Guard as BaselineSingleFlightGuard, Outcome as BaselineSingleFlightOutcome,
    WaitError,
};
use super::{
    build_cohort_sha256, canonical_json_bytes, canonical_json_hash, is_opaque_id, is_sha256,
    Runtime, RuntimeError,
};
use crate::agentic_session::validate_production_camera_lock_registration_lineage_runtime;
use forgecad_contracts::{
    ProductionCameraLockRegistrationLineageRecord, ProductionWeaponFormArtBaselineGetRequest,
    ProductionWeaponFormArtBaselineGetResult, ProductionWeaponFormArtBaselinePrepareRequest,
    ProductionWeaponFormArtBaselinePrepareResult, ProductionWeaponFormArtBaselineRecord,
    ProductionWeaponFormArtBaselineView, PRODUCTION_WEAPON_FORM_ART_BASELINE_AOV_KINDS,
    PRODUCTION_WEAPON_FORM_ART_BASELINE_CANONICALIZATION_POLICY,
    PRODUCTION_WEAPON_FORM_ART_BASELINE_GET_OPERATION,
    PRODUCTION_WEAPON_FORM_ART_BASELINE_GET_REQUEST_SCHEMA_VERSION,
    PRODUCTION_WEAPON_FORM_ART_BASELINE_GET_RESULT_SCHEMA_VERSION,
    PRODUCTION_WEAPON_FORM_ART_BASELINE_IDEMPOTENCY_POLICY,
    PRODUCTION_WEAPON_FORM_ART_BASELINE_MATERIALIZATION_STATUS,
    PRODUCTION_WEAPON_FORM_ART_BASELINE_MAX_RESPONSE_BYTES,
    PRODUCTION_WEAPON_FORM_ART_BASELINE_POLICY,
    PRODUCTION_WEAPON_FORM_ART_BASELINE_PREPARE_OPERATION,
    PRODUCTION_WEAPON_FORM_ART_BASELINE_PREPARE_REQUEST_SCHEMA_VERSION,
    PRODUCTION_WEAPON_FORM_ART_BASELINE_PREPARE_RESULT_SCHEMA_VERSION,
    PRODUCTION_WEAPON_FORM_ART_BASELINE_QUALITY_STATUS,
    PRODUCTION_WEAPON_FORM_ART_BASELINE_SCHEMA_VERSION,
    PRODUCTION_WEAPON_FORM_ART_BASELINE_VIEW_KINDS,
    PRODUCTION_WEAPON_FORM_ART_BASELINE_VIEW_SCHEMA_VERSION,
    PRODUCTION_WEAPON_FORM_ART_BASELINE_WRITER_POLICY,
};
use forgecad_store::{
    CasObject, ProductionWeaponFormArtBaselineCommitBundle,
    PRODUCTION_WEAPON_FORM_ART_BASELINE_PARENT_OBJECT_KIND,
    PRODUCTION_WEAPON_FORM_ART_BASELINE_VIEW_OBJECT_KIND,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::HashSet;

pub const PRODUCTION_WEAPON_FORM_ART_BASELINE_PREFLIGHT_REQUEST_SCHEMA_VERSION: &str =
    "ProductionWeaponFormArtBaselinePreflightRequest@1";
pub const PRODUCTION_WEAPON_FORM_ART_BASELINE_PREFLIGHT_RESULT_SCHEMA_VERSION: &str =
    "ProductionWeaponFormArtBaselinePreflightResult@1";
pub const PRODUCTION_WEAPON_FORM_ART_BASELINE_PREFLIGHT_OPERATION: &str =
    "forgecad.production.weapon.form-art-baseline-preflight-get@1";
pub const PRODUCTION_WEAPON_FORM_ART_BASELINE_PREFLIGHT_POLICY: &str =
    "fresh-same-cohort-form-art-baseline-lineage-rig-v2-preflight@1";
pub const PRODUCTION_WEAPON_FORM_ART_BASELINE_PREFLIGHT_WRITER_POLICY: &str =
    "forgecad-runtime-only-state-writer@1";

const MAX_RESPONSE_BYTES: u64 = 1_048_576;
const RIG_V2_SCHEMA_VERSION: &str = "RegisteredCameraRigCalibration@2";
const REGISTRATION_LINEAGE_MISSING: &str = "REGISTRATION_LINEAGE_MISSING";
const REGISTRATION_LINEAGE_READ_UNAVAILABLE: &str = "REGISTRATION_LINEAGE_READ_UNAVAILABLE";
const REGISTRATION_LINEAGE_INVALID: &str = "REGISTRATION_LINEAGE_INVALID";
const REGISTRATION_LINEAGE_NOT_PROMOTABLE: &str = "REGISTRATION_LINEAGE_NOT_PROMOTABLE";
const RIG_V2_READ_UNAVAILABLE: &str = "REGISTERED_RIG_V2_READ_UNAVAILABLE";
const RIG_V2_INVALID: &str = "REGISTERED_RIG_V2_INVALID";
const RIG_V2_SCOPE_MISMATCH: &str = "REGISTERED_RIG_V2_SCOPE_MISMATCH";
const VIEW_COVERAGE_INVALID: &str = "REGISTERED_RIG_V2_SIX_VIEW_COVERAGE_INVALID";

const SIX_VIEW_KINDS: [&str; 6] = [
    "front",
    "back",
    "left",
    "right",
    "top",
    "rear-three-quarter",
];
const ALL_RIG_VIEW_KINDS: [&str; 7] = [
    "front",
    "back",
    "left",
    "right",
    "top",
    "bottom",
    "rear-three-quarter",
];

const REQUEST_FIELDS: &[&str] = &[
    "schema_version",
    "operation",
    "preflight_id",
    "registration_lineage_id",
    "registration_lineage_canonical_sha256",
    "session_id",
    "project_id",
    "candidate_id",
    "candidate_state_sha256",
    "artifact_id",
    "artifact_sha256",
    "max_response_bytes",
    "writer_policy",
    "runtime_write_performed",
    "persistent_user_data_touched",
    "input_sha256",
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProductionWeaponFormArtBaselinePreflightRequest {
    pub schema_version: String,
    pub operation: String,
    pub preflight_id: String,
    pub registration_lineage_id: String,
    pub registration_lineage_canonical_sha256: String,
    pub session_id: String,
    pub project_id: String,
    pub candidate_id: String,
    pub candidate_state_sha256: String,
    pub artifact_id: String,
    pub artifact_sha256: String,
    pub max_response_bytes: u64,
    pub writer_policy: String,
    pub runtime_write_performed: bool,
    pub persistent_user_data_touched: bool,
    pub input_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProductionWeaponFormArtBaselinePreflightView {
    pub view_kind: String,
    pub camera_source: String,
    pub registered_rig_v2_id: Option<String>,
    pub registered_camera_hash: Option<String>,
    pub registered_camera_canonical_sha256: Option<String>,
    pub lineage_view_present: bool,
    pub legacy_camera_lock_rig_used: bool,
    pub historical_render_set_reused: bool,
    pub fresh_render_required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProductionWeaponFormArtBaselinePreflightResult {
    pub schema_version: String,
    pub operation: String,
    pub preflight_id: String,
    pub registration_lineage_id: String,
    pub registration_lineage_canonical_sha256: String,
    pub session_id: String,
    pub project_id: String,
    pub candidate_id: String,
    pub candidate_state_sha256: String,
    pub artifact_id: String,
    pub artifact_sha256: String,
    pub lineage_status: String,
    pub lineage_promotable: bool,
    pub lineage_receipt_object_sha256: Option<String>,
    pub registered_rig_v2_id: Option<String>,
    pub registered_rig_v2_object_sha256: Option<String>,
    pub registered_rig_v2_canonical_sha256: Option<String>,
    pub rig_v2_status: String,
    pub artifact_binding_status: String,
    pub expected_view_kinds: Vec<String>,
    pub views: Vec<ProductionWeaponFormArtBaselinePreflightView>,
    pub rear_three_quarter_camera_source: String,
    pub runtime_build_cohort_sha256: Option<String>,
    pub worker_cohort_required: bool,
    pub historical_form_art_reuse: bool,
    pub fresh_render_worker_started: bool,
    pub fresh_baseline_materialized: bool,
    pub ready_for_fresh_baseline: bool,
    pub blocking_reasons: Vec<String>,
    pub policy: String,
    pub writer_policy: String,
    pub runtime_write: bool,
    pub persistent_user_data_touched: bool,
    pub worker_started: bool,
    pub production_stage_advanced: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
    pub restart_hash_verified: bool,
    pub readiness_sha256: String,
}

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
) -> Result<ProductionWeaponFormArtBaselinePreflightRequest, RuntimeError> {
    let object = value.as_object().ok_or_else(|| {
        invalid("PRODUCTION_WEAPON_FORM_ART_BASELINE_PREFLIGHT_REQUEST_OBJECT_REQUIRED")
    })?;
    if let Some(field) = object
        .keys()
        .find(|field| !REQUEST_FIELDS.contains(&field.as_str()))
    {
        return Err(invalid(format!(
            "PRODUCTION_WEAPON_FORM_ART_BASELINE_PREFLIGHT_UNSUPPORTED_FIELD: {field}"
        )));
    }
    if required_text(object, "schema_version")?
        != PRODUCTION_WEAPON_FORM_ART_BASELINE_PREFLIGHT_REQUEST_SCHEMA_VERSION
        || required_text(object, "operation")?
            != PRODUCTION_WEAPON_FORM_ART_BASELINE_PREFLIGHT_OPERATION
    {
        return Err(invalid(
            "PRODUCTION_WEAPON_FORM_ART_BASELINE_PREFLIGHT_SCHEMA_OR_OPERATION_MISMATCH",
        ));
    }
    let request: ProductionWeaponFormArtBaselinePreflightRequest =
        serde_json::from_value(value.clone()).map_err(|error| {
            invalid(format!(
                "PRODUCTION_WEAPON_FORM_ART_BASELINE_PREFLIGHT_REQUEST_INVALID: {error}"
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
        ("artifact_id", request.artifact_id.as_str()),
    ] {
        if !is_opaque_id(value) {
            return Err(invalid(format!(
                "PRODUCTION_WEAPON_FORM_ART_BASELINE_PREFLIGHT_{field}_INVALID"
            )));
        }
    }
    for (field, value) in [
        (
            "registration_lineage_canonical_sha256",
            request.registration_lineage_canonical_sha256.as_str(),
        ),
        (
            "candidate_state_sha256",
            request.candidate_state_sha256.as_str(),
        ),
        ("artifact_sha256", request.artifact_sha256.as_str()),
        ("input_sha256", request.input_sha256.as_str()),
    ] {
        if !is_sha256(value) {
            return Err(invalid(format!(
                "PRODUCTION_WEAPON_FORM_ART_BASELINE_PREFLIGHT_{field}_INVALID"
            )));
        }
    }
    if request.max_response_bytes != MAX_RESPONSE_BYTES
        || request.writer_policy != PRODUCTION_WEAPON_FORM_ART_BASELINE_PREFLIGHT_WRITER_POLICY
        || request.runtime_write_performed
        || request.persistent_user_data_touched
    {
        return Err(invalid(
            "PRODUCTION_WEAPON_FORM_ART_BASELINE_PREFLIGHT_POLICY_MISMATCH",
        ));
    }
    let mut input_preimage = object.clone();
    input_preimage.remove("input_sha256");
    if canonical_json_hash(&Value::Object(input_preimage)) != request.input_sha256 {
        return Err(invalid(
            "PRODUCTION_WEAPON_FORM_ART_BASELINE_PREFLIGHT_INPUT_HASH_MISMATCH",
        ));
    }
    Ok(request)
}

fn read_canonical_json(
    runtime: &Runtime,
    object_sha256: &str,
    canonical_sha256: &str,
    label: &str,
) -> Result<Value, RuntimeError> {
    if !is_sha256(object_sha256) || !is_sha256(canonical_sha256) {
        return Err(invalid(format!("{label}_HASH_INVALID")));
    }
    let bytes = runtime.cas_read_bounded(object_sha256, MAX_RESPONSE_BYTES)?;
    let value: Value = serde_json::from_slice(&bytes)
        .map_err(|error| invalid(format!("{label}_INVALID_JSON: {error}")))?;
    if canonical_json_bytes(&value)
        .map_err(|error| invalid(format!("{label}_CANONICALIZE_FAILED: {error}")))?
        != bytes
        || value.get("canonical_sha256").and_then(Value::as_str) != Some(canonical_sha256)
    {
        return Err(invalid(format!("{label}_CANONICAL_BYTES_MISMATCH")));
    }
    let mut canonical_preimage = value.clone();
    canonical_preimage["canonical_sha256"] = Value::String(String::new());
    if canonical_json_hash(&canonical_preimage) != canonical_sha256 {
        return Err(invalid(format!("{label}_CANONICAL_HASH_MISMATCH")));
    }
    Ok(value)
}

fn lineage_scope_matches(
    request: &ProductionWeaponFormArtBaselinePreflightRequest,
    lineage: &ProductionCameraLockRegistrationLineageRecord,
) -> bool {
    lineage.registration_lineage_id == request.registration_lineage_id
        && lineage.canonical_sha256 == request.registration_lineage_canonical_sha256
        && lineage.session_id == request.session_id
        && lineage.project_id == request.project_id
        && lineage.candidate_id == request.candidate_id
        && lineage.candidate_state_sha256 == request.candidate_state_sha256
        && lineage.artifact_id == request.artifact_id
        && lineage.artifact_sha256 == request.artifact_sha256
}

fn result_value(
    result: &ProductionWeaponFormArtBaselinePreflightResult,
) -> Result<Value, RuntimeError> {
    serde_json::to_value(result).map_err(|error| {
        invalid(format!(
            "PRODUCTION_WEAPON_FORM_ART_BASELINE_PREFLIGHT_RESULT_SERIALIZE_FAILED: {error}"
        ))
    })
}

fn rig_v2_views(
    rig_v2: &Value,
    lineage: &ProductionCameraLockRegistrationLineageRecord,
) -> Result<Vec<ProductionWeaponFormArtBaselinePreflightView>, RuntimeError> {
    if rig_v2.get("schema_version").and_then(Value::as_str) != Some(RIG_V2_SCHEMA_VERSION)
        || rig_v2.get("project_id").and_then(Value::as_str) != Some(lineage.project_id.as_str())
        || rig_v2.get("candidate_id").and_then(Value::as_str) != Some(lineage.candidate_id.as_str())
        || rig_v2.get("candidate_state_sha256").and_then(Value::as_str)
            != Some(lineage.candidate_state_sha256.as_str())
        || rig_v2.get("artifact_id").and_then(Value::as_str) != Some(lineage.artifact_id.as_str())
        || rig_v2.get("artifact_sha256").and_then(Value::as_str)
            != Some(lineage.artifact_sha256.as_str())
        || rig_v2.get("camera_lock_id").and_then(Value::as_str)
            != Some(lineage.camera_lock_id.as_str())
        || rig_v2
            .get("camera_lock_canonical_sha256")
            .and_then(Value::as_str)
            != Some(lineage.camera_lock_canonical_sha256.as_str())
    {
        return Err(invalid(RIG_V2_SCOPE_MISMATCH));
    }
    let rig_v2_id = rig_v2
        .get("registered_rig_v2_id")
        .and_then(Value::as_str)
        .filter(|value| is_opaque_id(value))
        .ok_or_else(|| invalid("REGISTERED_RIG_V2_ID_INVALID"))?;
    let renderer_views = rig_v2
        .get("renderer_views")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid(VIEW_COVERAGE_INVALID))?;
    let actual_kinds = renderer_views
        .iter()
        .map(|view| view.get("kind").and_then(Value::as_str))
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| invalid(VIEW_COVERAGE_INVALID))?;
    if actual_kinds != ALL_RIG_VIEW_KINDS {
        return Err(invalid(VIEW_COVERAGE_INVALID));
    }
    let mut result = Vec::with_capacity(SIX_VIEW_KINDS.len());
    for kind in SIX_VIEW_KINDS {
        let view = renderer_views
            .iter()
            .find(|view| view.get("kind").and_then(Value::as_str) == Some(kind))
            .ok_or_else(|| invalid(VIEW_COVERAGE_INVALID))?;
        let view_id = view
            .get("view_id")
            .and_then(Value::as_str)
            .filter(|value| is_opaque_id(value))
            .ok_or_else(|| invalid(format!("{VIEW_COVERAGE_INVALID}: {kind} view_id")))?;
        let camera = view
            .get("registered_camera")
            .ok_or_else(|| invalid(format!("{VIEW_COVERAGE_INVALID}: {kind} camera")))?;
        crate::multiview::camera_rig::validate_camera_calibration_v2(camera)
            .map_err(|error| invalid(format!("{RIG_V2_INVALID}: {error}")))?;
        let camera_hash = view
            .get("registered_camera_hash")
            .and_then(Value::as_str)
            .filter(|value| is_sha256(value))
            .ok_or_else(|| invalid(format!("{VIEW_COVERAGE_INVALID}: {kind} camera hash")))?;
        if camera.get("camera_hash").and_then(Value::as_str) != Some(camera_hash) {
            return Err(invalid(format!(
                "{VIEW_COVERAGE_INVALID}: {kind} camera identity"
            )));
        }
        let camera_canonical_sha256 = camera
            .get("canonical_sha256")
            .and_then(Value::as_str)
            .filter(|value| is_sha256(value))
            .ok_or_else(|| invalid(format!("{VIEW_COVERAGE_INVALID}: {kind} camera canonical")))?;
        if view_id.is_empty() {
            return Err(invalid(VIEW_COVERAGE_INVALID));
        }
        if kind == "rear-three-quarter"
            && (view.get("orientation_authority").and_then(Value::as_str)
                != Some("authored-orientation-receipt")
                || view
                    .get("authored_orientation_canonical_sha256")
                    .and_then(Value::as_str)
                    != Some(lineage.authored_orientation_canonical_sha256.as_str())
                || view
                    .get("registered_camera_orbit_degrees")
                    .and_then(Value::as_i64)
                    .is_none())
        {
            return Err(invalid(format!(
                "{VIEW_COVERAGE_INVALID}: rear-three-quarter lineage authority"
            )));
        }
        result.push(ProductionWeaponFormArtBaselinePreflightView {
            view_kind: kind.to_owned(),
            camera_source: "registered-rig-v2.renderer_views".to_owned(),
            registered_rig_v2_id: Some(rig_v2_id.to_owned()),
            registered_camera_hash: Some(camera_hash.to_owned()),
            registered_camera_canonical_sha256: Some(camera_canonical_sha256.to_owned()),
            lineage_view_present: true,
            legacy_camera_lock_rig_used: false,
            historical_render_set_reused: false,
            fresh_render_required: true,
        });
    }
    Ok(result)
}

impl Runtime {
    /// Read-only preflight for the fresh FormArt baseline producer.
    ///
    /// A valid result proves only lineage/RigV2/six-view camera provenance.
    /// It intentionally never starts a Worker, reads historical FormArt as a
    /// substitute, or writes Store/CAS. `ready_for_fresh_baseline` is true
    /// only when the approved lineage, RigV2, six views and Runtime cohort are
    /// all available to the separate prepare operation.
    pub fn production_weapon_form_art_baseline_preflight_get(
        &self,
        request_value: Value,
    ) -> Result<Value, RuntimeError> {
        let request = parse_request(&request_value)?;
        let mut blocking_reasons = Vec::new();
        let mut lineage_status = "MISSING".to_owned();
        let mut lineage_promotable = false;
        let mut lineage_receipt_object_sha256 = None;
        let mut registered_rig_v2_id = None;
        let mut registered_rig_v2_object_sha256 = None;
        let mut registered_rig_v2_canonical_sha256 = None;
        let mut rig_v2_status = "NOT_READ".to_owned();
        let mut artifact_binding_status = "UNAVAILABLE".to_owned();
        let mut views = Vec::new();

        let lineage = match self
            .store
            .get_production_camera_lock_registration_lineage(&request.registration_lineage_id)
        {
            Ok(Some(lineage)) => {
                if !lineage_scope_matches(&request, &lineage) {
                    return Err(invalid(
                        "PRODUCTION_WEAPON_FORM_ART_BASELINE_PREFLIGHT_LINEAGE_SCOPE_MISMATCH",
                    ));
                }
                lineage_status = if lineage.promotable {
                    "PRESENT_PROMOTABLE".to_owned()
                } else {
                    "PRESENT_BLOCKED".to_owned()
                };
                lineage_promotable = lineage.promotable;
                lineage_receipt_object_sha256 = Some(lineage.receipt_object_sha256.clone());
                if !lineage.promotable {
                    blocking_reasons.push(REGISTRATION_LINEAGE_NOT_PROMOTABLE.to_owned());
                }
                Some(lineage)
            }
            Ok(None) => {
                blocking_reasons.push(REGISTRATION_LINEAGE_MISSING.to_owned());
                None
            }
            Err(_) => {
                lineage_status = "UNAVAILABLE".to_owned();
                blocking_reasons.push(REGISTRATION_LINEAGE_READ_UNAVAILABLE.to_owned());
                None
            }
        };

        if let Some(lineage) = lineage.as_ref() {
            if validate_production_camera_lock_registration_lineage_runtime(self, lineage).is_err()
            {
                lineage_status = "INVALID".to_owned();
                lineage_promotable = false;
                artifact_binding_status = "LINEAGE_INVALID".to_owned();
                blocking_reasons.push(REGISTRATION_LINEAGE_INVALID.to_owned());
            } else {
                artifact_binding_status = "LINEAGE_BOUND".to_owned();
                registered_rig_v2_object_sha256 =
                    Some(lineage.registered_rig_v2_object_sha256.clone());
                registered_rig_v2_canonical_sha256 =
                    Some(lineage.registered_rig_v2_canonical_sha256.clone());
                match read_canonical_json(
                    self,
                    &lineage.registered_rig_v2_object_sha256,
                    &lineage.registered_rig_v2_canonical_sha256,
                    "REGISTERED_RIG_V2",
                )
                .and_then(|rig_v2| {
                    registered_rig_v2_id = rig_v2
                        .get("registered_rig_v2_id")
                        .and_then(Value::as_str)
                        .filter(|value| is_opaque_id(value))
                        .map(ToOwned::to_owned);
                    rig_v2_views(&rig_v2, lineage)
                }) {
                    Ok(rig_views) => {
                        rig_v2_status = "VALID".to_owned();
                        views = rig_views;
                    }
                    Err(error) => {
                        if matches!(&error, RuntimeError::Store(_)) {
                            rig_v2_status = "UNAVAILABLE".to_owned();
                            blocking_reasons.push(RIG_V2_READ_UNAVAILABLE.to_owned());
                        } else {
                            rig_v2_status = "INVALID".to_owned();
                            blocking_reasons.push(RIG_V2_INVALID.to_owned());
                        }
                    }
                }
            }
        }

        if views.len() != SIX_VIEW_KINDS.len() {
            blocking_reasons.push(VIEW_COVERAGE_INVALID.to_owned());
        }
        let runtime_build_cohort_sha256 = build_cohort_sha256();
        if runtime_build_cohort_sha256.is_none() {
            blocking_reasons.push("RUNTIME_BUILD_COHORT_UNAVAILABLE".to_owned());
        }
        blocking_reasons.sort();
        blocking_reasons.dedup();
        let ready_for_fresh_baseline = blocking_reasons.is_empty()
            && lineage_promotable
            && lineage_status == "PRESENT_PROMOTABLE"
            && rig_v2_status == "VALID"
            && artifact_binding_status == "LINEAGE_BOUND"
            && views.len() == SIX_VIEW_KINDS.len();
        let mut result = ProductionWeaponFormArtBaselinePreflightResult {
            schema_version: PRODUCTION_WEAPON_FORM_ART_BASELINE_PREFLIGHT_RESULT_SCHEMA_VERSION
                .to_owned(),
            operation: PRODUCTION_WEAPON_FORM_ART_BASELINE_PREFLIGHT_OPERATION.to_owned(),
            preflight_id: request.preflight_id,
            registration_lineage_id: request.registration_lineage_id,
            registration_lineage_canonical_sha256: request.registration_lineage_canonical_sha256,
            session_id: request.session_id,
            project_id: request.project_id,
            candidate_id: request.candidate_id,
            candidate_state_sha256: request.candidate_state_sha256,
            artifact_id: request.artifact_id,
            artifact_sha256: request.artifact_sha256,
            lineage_status,
            lineage_promotable,
            lineage_receipt_object_sha256,
            registered_rig_v2_id,
            registered_rig_v2_object_sha256,
            registered_rig_v2_canonical_sha256,
            rig_v2_status,
            artifact_binding_status,
            expected_view_kinds: SIX_VIEW_KINDS
                .iter()
                .map(|kind| (*kind).to_owned())
                .collect(),
            rear_three_quarter_camera_source: if views
                .iter()
                .any(|view| view.view_kind == "rear-three-quarter")
            {
                "registered-rig-v2.renderer_views".to_owned()
            } else {
                "unavailable".to_owned()
            },
            views,
            runtime_build_cohort_sha256,
            worker_cohort_required: true,
            historical_form_art_reuse: false,
            fresh_render_worker_started: false,
            fresh_baseline_materialized: false,
            ready_for_fresh_baseline,
            blocking_reasons,
            policy: PRODUCTION_WEAPON_FORM_ART_BASELINE_PREFLIGHT_POLICY.to_owned(),
            writer_policy: PRODUCTION_WEAPON_FORM_ART_BASELINE_PREFLIGHT_WRITER_POLICY.to_owned(),
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

fn baseline_request_hash(value: &Value, input_sha256: &str) -> Result<String, RuntimeError> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid("PRODUCTION_WEAPON_FORM_ART_BASELINE_REQUEST_OBJECT_REQUIRED"))?;
    let mut input_preimage = object.clone();
    input_preimage.remove("input_sha256");
    if canonical_json_hash(&Value::Object(input_preimage)) != input_sha256 {
        return Err(invalid(
            "PRODUCTION_WEAPON_FORM_ART_BASELINE_INPUT_HASH_MISMATCH",
        ));
    }
    Ok(canonical_json_hash(value))
}

fn validate_baseline_request_common(
    baseline_id: &str,
    registration_lineage_id: &str,
    registration_lineage_canonical_sha256: &str,
    session_id: &str,
    project_id: &str,
    candidate_id: &str,
    candidate_state_sha256: &str,
    artifact_id: &str,
    artifact_sha256: &str,
    base_version_id: Option<&str>,
    idempotency_key: &str,
    max_response_bytes: u64,
    writer_policy: &str,
    canonicalization_policy: &str,
    runtime_write_performed: bool,
    persistent_user_data_touched: bool,
    input_sha256: &str,
) -> Result<(), RuntimeError> {
    for value in [
        baseline_id,
        registration_lineage_id,
        session_id,
        project_id,
        candidate_id,
        artifact_id,
        idempotency_key,
    ] {
        if !is_opaque_id(value) {
            return Err(invalid(
                "PRODUCTION_WEAPON_FORM_ART_BASELINE_IDENTITY_INVALID",
            ));
        }
    }
    if base_version_id.is_some_and(|value| !is_opaque_id(value))
        || [
            registration_lineage_canonical_sha256,
            candidate_state_sha256,
            artifact_sha256,
            input_sha256,
        ]
        .iter()
        .any(|value| !is_sha256(value))
        || max_response_bytes != PRODUCTION_WEAPON_FORM_ART_BASELINE_MAX_RESPONSE_BYTES
        || writer_policy != PRODUCTION_WEAPON_FORM_ART_BASELINE_WRITER_POLICY
        || canonicalization_policy != PRODUCTION_WEAPON_FORM_ART_BASELINE_CANONICALIZATION_POLICY
        || runtime_write_performed
        || persistent_user_data_touched
    {
        return Err(invalid(
            "PRODUCTION_WEAPON_FORM_ART_BASELINE_REQUEST_POLICY_OR_HASH_INVALID",
        ));
    }
    Ok(())
}

/// Candidate/artifact/base-version values are claims about Runtime-owned
/// state, not caller metadata. Resolve all of them before a replay or fresh
/// render. `None` is valid only when the candidate itself has no base version;
/// the operation must neither invent nor silently omit an existing binding.
fn validate_authoritative_base_version_binding(
    runtime: &Runtime,
    project_id: &str,
    candidate_id: &str,
    candidate_state_sha256: &str,
    artifact_id: &str,
    artifact_sha256: &str,
    base_version_id: Option<&str>,
) -> Result<(), RuntimeError> {
    let candidate = runtime.candidate(candidate_id)?.ok_or_else(|| {
        invalid("PRODUCTION_WEAPON_FORM_ART_BASELINE_BASE_VERSION_BINDING_UNAVAILABLE")
    })?;
    if candidate.project_id != project_id
        || candidate.canonical_sha256 != candidate_state_sha256
        || candidate.prepared_object_id.as_deref() != Some(artifact_id)
        || candidate.prepared_object_sha256.as_deref() != Some(artifact_sha256)
        || candidate
            .manifest_hash
            .as_deref()
            .is_some_and(|manifest| manifest != artifact_sha256)
    {
        return Err(invalid(
            "PRODUCTION_WEAPON_FORM_ART_BASELINE_BASE_VERSION_CANDIDATE_BINDING_MISMATCH",
        ));
    }
    if candidate.base_version_id.as_deref() != base_version_id {
        return Err(invalid(
            "PRODUCTION_WEAPON_FORM_ART_BASELINE_BASE_VERSION_NOT_AUTHORIZED",
        ));
    }
    let Some(base_version_id) = base_version_id else {
        return Ok(());
    };
    let version = runtime
        .version(base_version_id)?
        .ok_or_else(|| invalid("PRODUCTION_WEAPON_FORM_ART_BASELINE_BASE_VERSION_UNAVAILABLE"))?;
    if version.version_id != base_version_id || version.project_id != project_id {
        return Err(invalid(
            "PRODUCTION_WEAPON_FORM_ART_BASELINE_BASE_VERSION_PROJECT_MISMATCH",
        ));
    }
    Ok(())
}

fn baseline_scope_matches(
    baseline: &ProductionWeaponFormArtBaselineRecord,
    baseline_id: &str,
    registration_lineage_id: &str,
    registration_lineage_canonical_sha256: &str,
    session_id: &str,
    project_id: &str,
    candidate_id: &str,
    candidate_state_sha256: &str,
    artifact_id: &str,
    artifact_sha256: &str,
    base_version_id: Option<&str>,
    idempotency_key: &str,
) -> bool {
    baseline.baseline_id == baseline_id
        && baseline.registration_lineage_id == registration_lineage_id
        && baseline.registration_lineage_canonical_sha256 == registration_lineage_canonical_sha256
        && baseline.session_id == session_id
        && baseline.project_id == project_id
        && baseline.candidate_id == candidate_id
        && baseline.candidate_state_sha256 == candidate_state_sha256
        && baseline.artifact_id == artifact_id
        && baseline.artifact_sha256 == artifact_sha256
        && baseline.base_version_id.as_deref() == base_version_id
        && baseline.idempotency_key == idempotency_key
}

fn prepare_result_value(
    baseline: ProductionWeaponFormArtBaselineRecord,
    request_sha256: String,
    request_input_sha256: String,
    replayed: bool,
) -> Result<Value, RuntimeError> {
    let mut result = ProductionWeaponFormArtBaselinePrepareResult {
        schema_version: PRODUCTION_WEAPON_FORM_ART_BASELINE_PREPARE_RESULT_SCHEMA_VERSION
            .to_owned(),
        operation: PRODUCTION_WEAPON_FORM_ART_BASELINE_PREPARE_OPERATION.to_owned(),
        baseline_id: baseline.baseline_id.clone(),
        registration_lineage_id: baseline.registration_lineage_id.clone(),
        registration_lineage_canonical_sha256: baseline
            .registration_lineage_canonical_sha256
            .clone(),
        session_id: baseline.session_id.clone(),
        project_id: baseline.project_id.clone(),
        candidate_id: baseline.candidate_id.clone(),
        candidate_state_sha256: baseline.candidate_state_sha256.clone(),
        artifact_id: baseline.artifact_id.clone(),
        artifact_sha256: baseline.artifact_sha256.clone(),
        runtime_build_cohort_sha256: baseline.runtime_build_cohort_sha256.clone(),
        request_sha256,
        request_input_sha256,
        idempotency_key: baseline.idempotency_key.clone(),
        replayed,
        // `get` is only returned after Store has re-read every persisted CAS
        // root and Runtime has revalidated the current source lineage below.
        restart_hash_verified: true,
        writer_policy: PRODUCTION_WEAPON_FORM_ART_BASELINE_WRITER_POLICY.to_owned(),
        canonicalization_policy: PRODUCTION_WEAPON_FORM_ART_BASELINE_CANONICALIZATION_POLICY
            .to_owned(),
        runtime_write_performed: !replayed,
        persistent_user_data_touched: !replayed,
        promotion_eligible: false,
        quality_status: PRODUCTION_WEAPON_FORM_ART_BASELINE_QUALITY_STATUS.to_owned(),
        production_stage_advanced: false,
        candidate_confirmed: false,
        version_created: false,
        export_performed: false,
        canonical_sha256: String::new(),
        baseline,
    };
    let mut value = serde_json::to_value(&result)
        .map_err(|error| invalid(format!("baseline prepare result serialize failed: {error}")))?;
    result.canonical_sha256 = canonical_json_hash(&value);
    value = serde_json::to_value(result)
        .map_err(|error| invalid(format!("baseline prepare result serialize failed: {error}")))?;
    if canonical_json_bytes(&value)
        .map_err(|error| invalid(error.to_string()))?
        .len() as u64
        > PRODUCTION_WEAPON_FORM_ART_BASELINE_MAX_RESPONSE_BYTES
    {
        return Err(invalid(
            "PRODUCTION_WEAPON_FORM_ART_BASELINE_RESPONSE_TOO_LARGE",
        ));
    }
    Ok(value)
}

fn get_result_value(
    baseline: ProductionWeaponFormArtBaselineRecord,
    request_sha256: String,
    request_input_sha256: String,
) -> Result<Value, RuntimeError> {
    let mut result = ProductionWeaponFormArtBaselineGetResult {
        schema_version: PRODUCTION_WEAPON_FORM_ART_BASELINE_GET_RESULT_SCHEMA_VERSION.to_owned(),
        operation: PRODUCTION_WEAPON_FORM_ART_BASELINE_GET_OPERATION.to_owned(),
        baseline_id: baseline.baseline_id.clone(),
        registration_lineage_id: baseline.registration_lineage_id.clone(),
        registration_lineage_canonical_sha256: baseline
            .registration_lineage_canonical_sha256
            .clone(),
        session_id: baseline.session_id.clone(),
        project_id: baseline.project_id.clone(),
        candidate_id: baseline.candidate_id.clone(),
        candidate_state_sha256: baseline.candidate_state_sha256.clone(),
        artifact_id: baseline.artifact_id.clone(),
        artifact_sha256: baseline.artifact_sha256.clone(),
        runtime_build_cohort_sha256: baseline.runtime_build_cohort_sha256.clone(),
        request_sha256,
        request_input_sha256,
        idempotency_key: baseline.idempotency_key.clone(),
        replayed: true,
        // Store has re-read every persisted baseline root and the caller has
        // already passed current cohort and source-lineage validation before
        // this result is constructed.
        restart_hash_verified: true,
        writer_policy: PRODUCTION_WEAPON_FORM_ART_BASELINE_WRITER_POLICY.to_owned(),
        canonicalization_policy: PRODUCTION_WEAPON_FORM_ART_BASELINE_CANONICALIZATION_POLICY
            .to_owned(),
        runtime_write_performed: false,
        persistent_user_data_touched: false,
        promotion_eligible: false,
        quality_status: PRODUCTION_WEAPON_FORM_ART_BASELINE_QUALITY_STATUS.to_owned(),
        production_stage_advanced: false,
        candidate_confirmed: false,
        version_created: false,
        export_performed: false,
        canonical_sha256: String::new(),
        baseline,
    };
    let value = serde_json::to_value(&result)
        .map_err(|error| invalid(format!("baseline get result serialize failed: {error}")))?;
    result.canonical_sha256 = canonical_json_hash(&value);
    serde_json::to_value(result)
        .map_err(|error| invalid(format!("baseline get result serialize failed: {error}")))
}

fn validate_persisted_baseline_source_binding(
    runtime: &Runtime,
    baseline: &ProductionWeaponFormArtBaselineRecord,
) -> Result<(), RuntimeError> {
    let runtime_cohort = build_cohort_sha256()
        .ok_or_else(|| invalid("PRODUCTION_WEAPON_FORM_ART_BASELINE_RUNTIME_COHORT_UNAVAILABLE"))?;
    if runtime_cohort != baseline.runtime_build_cohort_sha256 {
        return Err(invalid(
            "PRODUCTION_WEAPON_FORM_ART_BASELINE_RUNTIME_COHORT_MISMATCH",
        ));
    }

    let lineage = runtime
        .store
        .get_production_camera_lock_registration_lineage(&baseline.registration_lineage_id)?
        .ok_or_else(|| invalid("PRODUCTION_WEAPON_FORM_ART_BASELINE_LINEAGE_NOT_FOUND"))?;
    validate_production_camera_lock_registration_lineage_runtime(runtime, &lineage)?;
    if !lineage.promotable {
        return Err(invalid(
            "PRODUCTION_WEAPON_FORM_ART_BASELINE_LINEAGE_NOT_PROMOTABLE",
        ));
    }
    if lineage.canonical_sha256 != baseline.registration_lineage_canonical_sha256
        || lineage.receipt_object_sha256 != baseline.registration_lineage_receipt_object_sha256
        || lineage.session_id != baseline.session_id
        || lineage.project_id != baseline.project_id
        || lineage.candidate_id != baseline.candidate_id
        || lineage.candidate_state_sha256 != baseline.candidate_state_sha256
        || lineage.artifact_id != baseline.artifact_id
        || lineage.artifact_sha256 != baseline.artifact_sha256
        || lineage.registered_rig_v2_object_sha256 != baseline.registered_rig_v2_object_sha256
        || lineage.registered_rig_v2_canonical_sha256 != baseline.registered_rig_v2_canonical_sha256
    {
        return Err(invalid(
            "PRODUCTION_WEAPON_FORM_ART_BASELINE_SOURCE_BINDING_MISMATCH",
        ));
    }

    let rig_v2 = read_canonical_json(
        runtime,
        &lineage.registered_rig_v2_object_sha256,
        &lineage.registered_rig_v2_canonical_sha256,
        "REGISTERED_RIG_V2",
    )?;
    if rig_v2.get("registered_rig_v2_id").and_then(Value::as_str)
        != Some(baseline.registered_rig_v2_id.as_str())
        || rig_v2_views(&rig_v2, &lineage)?.len()
            != PRODUCTION_WEAPON_FORM_ART_BASELINE_VIEW_KINDS.len()
    {
        return Err(invalid(
            "PRODUCTION_WEAPON_FORM_ART_BASELINE_RIG_V2_BINDING_MISMATCH",
        ));
    }

    let (canvas, canvas_object_sha256) =
        crate::agentic_session::durable_reference_canvas_for_session_binding(
            runtime,
            &baseline.project_id,
            &baseline.session_id,
            &baseline.candidate_id,
        )?;
    if canvas_object_sha256 != lineage.reference_canvas_object_sha256
        || canvas.get("canonical_sha256").and_then(Value::as_str)
            != Some(lineage.reference_canvas_canonical_sha256.as_str())
    {
        return Err(invalid(
            "PRODUCTION_WEAPON_FORM_ART_BASELINE_REFERENCE_CANVAS_BINDING_MISMATCH",
        ));
    }
    Ok(())
}

/// Revalidate the complete Runtime-owned source immediately before a fresh
/// baseline is committed or a replay is returned.  The persisted-source
/// validator covers the current build cohort, promotable registration lineage
/// (including its receipt), RigV2 and the exact durable ReferenceCanvas; the
/// authoritative candidate validator covers the live candidate state/artifact
/// and optional base-version binding.  Keeping these together prevents an
/// expensive six-view render from turning into a Store write after any source
/// drift, and prevents replay paths from bypassing Runtime semantics.
fn validate_exact_baseline_source_binding(
    runtime: &Runtime,
    baseline: &ProductionWeaponFormArtBaselineRecord,
) -> Result<(), RuntimeError> {
    validate_authoritative_base_version_binding(
        runtime,
        &baseline.project_id,
        &baseline.candidate_id,
        &baseline.candidate_state_sha256,
        &baseline.artifact_id,
        &baseline.artifact_sha256,
        baseline.base_version_id.as_deref(),
    )?;
    validate_persisted_baseline_source_binding(runtime, baseline)
}

fn release_baseline_reservation(
    runtime: &Runtime,
    reservation: &forgecad_store::CasReservation,
    objects: &[CasObject],
    cleanup: bool,
) -> Result<(), RuntimeError> {
    let mut released = HashSet::new();
    let mut first_error = None;
    for object in objects.iter().rev() {
        if released.insert(object.record.sha256.clone()) {
            if let Err(error) =
                runtime
                    .store
                    .release_cas_reservation_object(reservation, object, cleanup)
            {
                if first_error.is_none() {
                    first_error = Some(RuntimeError::Store(error));
                }
            }
        }
    }
    match first_error {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

fn begin_baseline_single_flight(
    runtime: &Runtime,
    project_id: &str,
    idempotency_key: &str,
    request_sha256: &str,
) -> Result<BaselineSingleFlightGuard, RuntimeError> {
    runtime
        .form_art_baseline_flights
        .begin(project_id, idempotency_key, request_sha256)
        .map_err(|error| {
            invalid(match error {
                BeginError::Conflict => "PRODUCTION_WEAPON_FORM_ART_BASELINE_IDEMPOTENCY_CONFLICT",
                BeginError::Capacity => {
                    "PRODUCTION_WEAPON_FORM_ART_BASELINE_SINGLE_FLIGHT_CAPACITY"
                }
                BeginError::LockPoisoned => {
                    "PRODUCTION_WEAPON_FORM_ART_BASELINE_SINGLE_FLIGHT_LOCK_POISONED"
                }
            })
        })
}

fn validate_single_flight_prepare_result(
    runtime: &Runtime,
    value: &Value,
    request: &ProductionWeaponFormArtBaselinePrepareRequest,
    request_sha256: &str,
) -> Result<(), RuntimeError> {
    let result: ProductionWeaponFormArtBaselinePrepareResult =
        serde_json::from_value(value.clone()).map_err(|error| {
            invalid(format!(
                "PRODUCTION_WEAPON_FORM_ART_BASELINE_SINGLE_FLIGHT_RESULT_INVALID: {error}"
            ))
        })?;
    if result.schema_version != PRODUCTION_WEAPON_FORM_ART_BASELINE_PREPARE_RESULT_SCHEMA_VERSION
        || result.operation != PRODUCTION_WEAPON_FORM_ART_BASELINE_PREPARE_OPERATION
        || result.request_sha256 != request_sha256
        || result.request_input_sha256 != request.input_sha256
        || !baseline_scope_matches(
            &result.baseline,
            &request.baseline_id,
            &request.registration_lineage_id,
            &request.registration_lineage_canonical_sha256,
            &request.session_id,
            &request.project_id,
            &request.candidate_id,
            &request.candidate_state_sha256,
            &request.artifact_id,
            &request.artifact_sha256,
            request.base_version_id.as_deref(),
            &request.idempotency_key,
        )
        || result.baseline.request_sha256 != request_sha256
        || result.baseline.input_sha256 != request.input_sha256
    {
        return Err(invalid(
            "PRODUCTION_WEAPON_FORM_ART_BASELINE_SINGLE_FLIGHT_RESULT_BINDING_MISMATCH",
        ));
    }
    let mut canonical_preimage = value.clone();
    canonical_preimage["canonical_sha256"] = Value::String(String::new());
    if canonical_json_hash(&canonical_preimage) != result.canonical_sha256 {
        return Err(invalid(
            "PRODUCTION_WEAPON_FORM_ART_BASELINE_SINGLE_FLIGHT_RESULT_HASH_MISMATCH",
        ));
    }
    validate_exact_baseline_source_binding(runtime, &result.baseline)?;
    Ok(())
}

fn wait_baseline_single_flight(
    runtime: &Runtime,
    guard: &BaselineSingleFlightGuard,
    request: &ProductionWeaponFormArtBaselinePrepareRequest,
    request_sha256: &str,
) -> Result<Value, RuntimeError> {
    match runtime.form_art_baseline_flights.wait(guard) {
        Ok(BaselineSingleFlightOutcome::Completed(response_json)) => {
            let value: Value = serde_json::from_str(&response_json).map_err(|error| {
                invalid(format!(
                    "PRODUCTION_WEAPON_FORM_ART_BASELINE_SINGLE_FLIGHT_RESULT_INVALID: {error}"
                ))
            })?;
            validate_single_flight_prepare_result(runtime, &value, request, request_sha256)?;
            Ok(value)
        }
        Ok(BaselineSingleFlightOutcome::Failed(message)) => Err(invalid(message)),
        Err(WaitError::Timeout) => Err(invalid(
            "PRODUCTION_WEAPON_FORM_ART_BASELINE_SINGLE_FLIGHT_TIMEOUT",
        )),
        Err(WaitError::LockPoisoned) => Err(invalid(
            "PRODUCTION_WEAPON_FORM_ART_BASELINE_SINGLE_FLIGHT_LOCK_POISONED",
        )),
        Err(WaitError::MissingOutcome) => Err(invalid(
            "PRODUCTION_WEAPON_FORM_ART_BASELINE_SINGLE_FLIGHT_OUTCOME_MISSING",
        )),
    }
}

impl Runtime {
    pub fn production_weapon_form_art_baseline_prepare(
        &self,
        request_value: Value,
    ) -> Result<Value, RuntimeError> {
        let request: ProductionWeaponFormArtBaselinePrepareRequest =
            serde_json::from_value(request_value.clone()).map_err(|error| {
                invalid(format!(
                    "PRODUCTION_WEAPON_FORM_ART_BASELINE_PREPARE_REQUEST_INVALID: {error}"
                ))
            })?;
        if request.schema_version
            != PRODUCTION_WEAPON_FORM_ART_BASELINE_PREPARE_REQUEST_SCHEMA_VERSION
            || request.operation != PRODUCTION_WEAPON_FORM_ART_BASELINE_PREPARE_OPERATION
        {
            return Err(invalid(
                "PRODUCTION_WEAPON_FORM_ART_BASELINE_PREPARE_SCHEMA_OR_OPERATION_MISMATCH",
            ));
        }
        validate_baseline_request_common(
            &request.baseline_id,
            &request.registration_lineage_id,
            &request.registration_lineage_canonical_sha256,
            &request.session_id,
            &request.project_id,
            &request.candidate_id,
            &request.candidate_state_sha256,
            &request.artifact_id,
            &request.artifact_sha256,
            request.base_version_id.as_deref(),
            &request.idempotency_key,
            request.max_response_bytes,
            &request.writer_policy,
            &request.canonicalization_policy,
            request.runtime_write_performed,
            request.persistent_user_data_touched,
            &request.input_sha256,
        )?;
        validate_authoritative_base_version_binding(
            self,
            &request.project_id,
            &request.candidate_id,
            &request.candidate_state_sha256,
            &request.artifact_id,
            &request.artifact_sha256,
            request.base_version_id.as_deref(),
        )?;
        let request_sha256 = baseline_request_hash(&request_value, &request.input_sha256)?;

        if let Some(existing) = self.store.get_production_weapon_form_art_baseline(
            &request.project_id,
            &request.idempotency_key,
        )? {
            if !baseline_scope_matches(
                &existing,
                &request.baseline_id,
                &request.registration_lineage_id,
                &request.registration_lineage_canonical_sha256,
                &request.session_id,
                &request.project_id,
                &request.candidate_id,
                &request.candidate_state_sha256,
                &request.artifact_id,
                &request.artifact_sha256,
                request.base_version_id.as_deref(),
                &request.idempotency_key,
            ) || existing.input_sha256 != request.input_sha256
                || existing.request_sha256 != request_sha256
            {
                return Err(invalid(
                    "PRODUCTION_WEAPON_FORM_ART_BASELINE_IDEMPOTENCY_CONFLICT",
                ));
            }
            validate_exact_baseline_source_binding(self, &existing)?;
            return prepare_result_value(existing, request_sha256, request.input_sha256, true);
        }

        let single_flight = begin_baseline_single_flight(
            self,
            &request.project_id,
            &request.idempotency_key,
            &request_sha256,
        )?;
        if !single_flight.is_owner() {
            return wait_baseline_single_flight(self, &single_flight, &request, &request_sha256);
        }

        let owner_result = (|| -> Result<Value, RuntimeError> {
            let runtime_cohort = build_cohort_sha256().ok_or_else(|| {
                invalid("PRODUCTION_WEAPON_FORM_ART_BASELINE_RUNTIME_COHORT_UNAVAILABLE")
            })?;
            let lineage = self
                .store
                .get_production_camera_lock_registration_lineage(&request.registration_lineage_id)?
                .ok_or_else(|| invalid("PRODUCTION_WEAPON_FORM_ART_BASELINE_LINEAGE_NOT_FOUND"))?;
            let preflight_request = ProductionWeaponFormArtBaselinePreflightRequest {
                schema_version:
                    PRODUCTION_WEAPON_FORM_ART_BASELINE_PREFLIGHT_REQUEST_SCHEMA_VERSION.to_owned(),
                operation: PRODUCTION_WEAPON_FORM_ART_BASELINE_PREFLIGHT_OPERATION.to_owned(),
                preflight_id: request.baseline_id.clone(),
                registration_lineage_id: request.registration_lineage_id.clone(),
                registration_lineage_canonical_sha256: request
                    .registration_lineage_canonical_sha256
                    .clone(),
                session_id: request.session_id.clone(),
                project_id: request.project_id.clone(),
                candidate_id: request.candidate_id.clone(),
                candidate_state_sha256: request.candidate_state_sha256.clone(),
                artifact_id: request.artifact_id.clone(),
                artifact_sha256: request.artifact_sha256.clone(),
                max_response_bytes: MAX_RESPONSE_BYTES,
                writer_policy: PRODUCTION_WEAPON_FORM_ART_BASELINE_PREFLIGHT_WRITER_POLICY
                    .to_owned(),
                runtime_write_performed: false,
                persistent_user_data_touched: false,
                input_sha256: String::new(),
            };
            let mut preflight_value = serde_json::to_value(preflight_request)
                .map_err(|error| invalid(error.to_string()))?;
            let preflight_input = {
                let mut value = preflight_value.clone();
                value
                    .as_object_mut()
                    .expect("typed preflight is an object")
                    .remove("input_sha256");
                canonical_json_hash(&value)
            };
            preflight_value["input_sha256"] = Value::String(preflight_input);
            let preflight =
                self.production_weapon_form_art_baseline_preflight_get(preflight_value)?;
            let preflight_blockers = preflight
                .get("blocking_reasons")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .filter_map(|value| value.as_str().map(ToOwned::to_owned))
                .collect::<Vec<_>>();
            let preflight_ready = preflight
                .get("ready_for_fresh_baseline")
                .and_then(Value::as_bool)
                == Some(true);
            if !lineage.promotable || !preflight_ready || !preflight_blockers.is_empty() {
                let reasons = if preflight_blockers.is_empty() {
                    vec!["FRESH_BASELINE_PREFLIGHT_NOT_READY".to_owned()]
                } else {
                    preflight_blockers
                };
                return Err(invalid(format!(
                    "PRODUCTION_WEAPON_FORM_ART_BASELINE_PREFLIGHT_BLOCKED: {}",
                    reasons.join(",")
                )));
            }
            validate_production_camera_lock_registration_lineage_runtime(self, &lineage)?;
            let rig_v2 = read_canonical_json(
                self,
                &lineage.registered_rig_v2_object_sha256,
                &lineage.registered_rig_v2_canonical_sha256,
                "REGISTERED_RIG_V2",
            )?;
            let registered_rig_v2_id = rig_v2
                .get("registered_rig_v2_id")
                .and_then(Value::as_str)
                .filter(|value| is_opaque_id(value))
                .ok_or_else(|| invalid("REGISTERED_RIG_V2_ID_INVALID"))?
                .to_owned();
            let rig_views = rig_v2
                .get("renderer_views")
                .and_then(Value::as_array)
                .ok_or_else(|| invalid(VIEW_COVERAGE_INVALID))?;
            let (canvas, canvas_object_sha256) =
                crate::agentic_session::durable_reference_canvas_for_session_binding(
                    self,
                    &request.project_id,
                    &request.session_id,
                    &request.candidate_id,
                )?;
            if canvas_object_sha256 != lineage.reference_canvas_object_sha256
                || canvas.get("canonical_sha256").and_then(Value::as_str)
                    != Some(lineage.reference_canvas_canonical_sha256.as_str())
            {
                return Err(invalid(
                    "PRODUCTION_WEAPON_FORM_ART_BASELINE_REFERENCE_CANVAS_BINDING_MISMATCH",
                ));
            }
            let canvas_views = canvas
                .get("views")
                .and_then(Value::as_array)
                .ok_or_else(|| {
                    invalid("PRODUCTION_WEAPON_FORM_ART_BASELINE_CANVAS_VIEWS_MISSING")
                })?;
            let batch = self
                .store
                .begin_production_weapon_form_art_baseline_cas_batch_for_prepare(
                    &request.baseline_id,
                    &request.registration_lineage_id,
                    &request.session_id,
                    &request.project_id,
                    &request.candidate_id,
                    &request.candidate_state_sha256,
                    &request.artifact_id,
                    &request.artifact_sha256,
                    &request_sha256,
                    &request.input_sha256,
                    &runtime_cohort,
                )?;
            let mut reserved_objects = Vec::<CasObject>::new();
            // Before Store commit, every failure must remove temporary objects.
            // A new commit makes the roots durable; an idempotent concurrent
            // replay instead cleans this invocation's unlinked derived objects.
            let mut cleanup_reserved_objects = true;
            let operation = (|| -> Result<Value, RuntimeError> {
                let mut views =
                    Vec::with_capacity(PRODUCTION_WEAPON_FORM_ART_BASELINE_VIEW_KINDS.len());
                let mut view_receipt_objects = Vec::with_capacity(views.capacity());
                for kind in PRODUCTION_WEAPON_FORM_ART_BASELINE_VIEW_KINDS {
                    let rig_view = rig_views
                        .iter()
                        .find(|view| view.get("kind").and_then(Value::as_str) == Some(kind))
                        .ok_or_else(|| invalid(format!("{VIEW_COVERAGE_INVALID}: {kind}")))?;
                    let canvas_view = canvas_views
                        .iter()
                        .find(|view| view.get("kind").and_then(Value::as_str) == Some(kind))
                        .ok_or_else(|| {
                            invalid(format!(
                                "PRODUCTION_WEAPON_FORM_ART_BASELINE_CANVAS_VIEW_MISSING: {kind}"
                            ))
                        })?;
                    let view_id = rig_view
                        .get("view_id")
                        .and_then(Value::as_str)
                        .filter(|value| is_opaque_id(value))
                        .ok_or_else(|| {
                            invalid(format!("{VIEW_COVERAGE_INVALID}: {kind} view_id"))
                        })?;
                    let camera = rig_view.get("registered_camera").cloned().ok_or_else(|| {
                        invalid(format!("{VIEW_COVERAGE_INVALID}: {kind} camera"))
                    })?;
                    let reference_id = canvas_view
                        .get("reference_id")
                        .and_then(Value::as_str)
                        .filter(|value| is_opaque_id(value))
                        .ok_or_else(|| invalid("baseline canvas reference_id missing"))?;
                    let reference_sha256 = canvas_view
                        .get("reference_sha256")
                        .and_then(Value::as_str)
                        .filter(|value| is_sha256(value))
                        .ok_or_else(|| invalid("baseline canvas reference_sha256 missing"))?;
                    let view_spec = canvas_view
                        .get("view_spec")
                        .cloned()
                        .ok_or_else(|| invalid("baseline canvas view_spec missing"))?;
                    let mut render_request = json!({
                        "candidate_id":request.candidate_id,
                        "reference_id":reference_id,
                        "view_spec":view_spec,
                        "camera":camera,
                        "view_id":view_id,
                    });
                    if let Some(target_sha256) = canvas_view
                        .get("target_sha256")
                        .and_then(Value::as_str)
                        .filter(|value| is_sha256(value))
                    {
                        render_request["target_sha256"] = Value::String(target_sha256.to_owned());
                    }
                    let rendered = self.prepare_reference_comparison_detached_form_art_batch(
                        &request.project_id,
                        render_request,
                        &batch,
                        &mut reserved_objects,
                    )?;
                    let render_set = rendered
                        .get("render_set")
                        .ok_or_else(|| invalid("baseline RenderSet missing"))?;
                    if render_set
                        .get("render_worker_build_cohort_sha256")
                        .and_then(Value::as_str)
                        != Some(runtime_cohort.as_str())
                        || render_set.get("width").and_then(Value::as_u64) != Some(512)
                        || render_set.get("height").and_then(Value::as_u64) != Some(512)
                    {
                        return Err(invalid(
                            "PRODUCTION_WEAPON_FORM_ART_BASELINE_WORKER_COHORT_OR_SIZE_MISMATCH",
                        ));
                    }
                    let pass_artifacts = PRODUCTION_WEAPON_FORM_ART_BASELINE_AOV_KINDS
                        .iter()
                        .map(|pass| {
                            render_set
                                .pointer(&format!("/pass_artifacts/{pass}/sha256"))
                                .and_then(Value::as_str)
                                .filter(|value| is_sha256(value))
                                .map(ToOwned::to_owned)
                                .ok_or_else(|| invalid(format!("baseline AOV {pass} missing")))
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    let camera_value = rendered
                        .get("camera")
                        .ok_or_else(|| invalid("baseline camera missing"))?;
                    let comparison = rendered
                        .get("comparison_report")
                        .ok_or_else(|| invalid("baseline comparison missing"))?;
                    let quality = rendered
                        .get("quality_report")
                        .ok_or_else(|| invalid("baseline quality report missing"))?;
                    let render_set_object_sha256 = rendered["render_set_object_sha256"]
                        .as_str()
                        .filter(|value| is_sha256(value))
                        .ok_or_else(|| invalid("baseline RenderSet object hash missing"))?;
                    let comparison_object_sha256 = rendered["comparison_report_object_sha256"]
                        .as_str()
                        .filter(|value| is_sha256(value))
                        .ok_or_else(|| invalid("baseline comparison object hash missing"))?;
                    if render_set.get("candidate_id").and_then(Value::as_str)
                        != Some(request.candidate_id.as_str())
                        || render_set.get("artifact_sha256").and_then(Value::as_str)
                            != Some(request.artifact_sha256.as_str())
                        || comparison.get("candidate_id").and_then(Value::as_str)
                            != Some(request.candidate_id.as_str())
                        || comparison.get("artifact_sha256").and_then(Value::as_str)
                            != Some(request.artifact_sha256.as_str())
                        || comparison.get("render_set_hash").and_then(Value::as_str)
                            != Some(render_set_object_sha256)
                        || quality.get("candidate_id").and_then(Value::as_str)
                            != Some(request.candidate_id.as_str())
                        || quality.get("artifact_sha256").and_then(Value::as_str)
                            != Some(request.artifact_sha256.as_str())
                        || quality.get("render_set_hash").and_then(Value::as_str)
                            != Some(render_set_object_sha256)
                        || quality
                            .get("comparison_report_hash")
                            .and_then(Value::as_str)
                            != Some(comparison_object_sha256)
                    {
                        return Err(invalid(
                        "PRODUCTION_WEAPON_FORM_ART_BASELINE_DERIVED_CANDIDATE_ARTIFACT_BINDING_MISMATCH",
                    ));
                    }
                    let mut view = ProductionWeaponFormArtBaselineView {
                        schema_version: PRODUCTION_WEAPON_FORM_ART_BASELINE_VIEW_SCHEMA_VERSION
                            .to_owned(),
                        view_kind: kind.to_owned(),
                        view_id: view_id.to_owned(),
                        reference_id: reference_id.to_owned(),
                        reference_sha256: reference_sha256.to_owned(),
                        camera_hash: camera_value["camera_hash"]
                            .as_str()
                            .ok_or_else(|| invalid("baseline camera hash missing"))?
                            .to_owned(),
                        camera_canonical_sha256: camera_value["canonical_sha256"]
                            .as_str()
                            .ok_or_else(|| invalid("baseline camera canonical hash missing"))?
                            .to_owned(),
                        camera_object_sha256: rendered["camera_object_sha256"]
                            .as_str()
                            .ok_or_else(|| invalid("baseline camera object hash missing"))?
                            .to_owned(),
                        render_set_id: render_set["render_set_id"]
                            .as_str()
                            .ok_or_else(|| invalid("baseline RenderSet id missing"))?
                            .to_owned(),
                        render_set_object_sha256: render_set_object_sha256.to_owned(),
                        render_set_canonical_sha256: render_set["canonical_sha256"]
                            .as_str()
                            .ok_or_else(|| invalid("baseline RenderSet canonical hash missing"))?
                            .to_owned(),
                        render_set_view_id: render_set["view_id"]
                            .as_str()
                            .ok_or_else(|| invalid("baseline RenderSet view id missing"))?
                            .to_owned(),
                        pass_artifact_object_sha256: pass_artifacts,
                        reference_mask_object_sha256: comparison
                            .pointer("/mask/sha256")
                            .and_then(Value::as_str)
                            .ok_or_else(|| invalid("baseline reference mask hash missing"))?
                            .to_owned(),
                        comparison_report_object_sha256: comparison_object_sha256.to_owned(),
                        quality_report_object_sha256: rendered["quality_report_object_sha256"]
                            .as_str()
                            .ok_or_else(|| invalid("baseline quality object hash missing"))?
                            .to_owned(),
                        render_worker_build_cohort_sha256: runtime_cohort.clone(),
                        quality_status: PRODUCTION_WEAPON_FORM_ART_BASELINE_QUALITY_STATUS
                            .to_owned(),
                        receipt_object_sha256: String::new(),
                        canonical_sha256: String::new(),
                        created_at: super::now_string(),
                    };
                    let mut canonical_view =
                        serde_json::to_value(&view).map_err(|error| invalid(error.to_string()))?;
                    canonical_view["receipt_object_sha256"] = Value::String(String::new());
                    canonical_view["canonical_sha256"] = Value::String(String::new());
                    view.canonical_sha256 = canonical_json_hash(&canonical_view);
                    let mut receipt_view =
                        serde_json::to_value(&view).map_err(|error| invalid(error.to_string()))?;
                    receipt_view["receipt_object_sha256"] = Value::String(String::new());
                    let receipt_bytes = canonical_json_bytes(&receipt_view)
                        .map_err(|error| invalid(error.to_string()))?;
                    let receipt_object = self
                        .store
                        .put_production_weapon_form_art_baseline_cas_object(
                            &batch,
                            &receipt_bytes,
                            None,
                            "application/json",
                            PRODUCTION_WEAPON_FORM_ART_BASELINE_VIEW_OBJECT_KIND,
                            &super::now_string(),
                        )?;
                    view.receipt_object_sha256 = receipt_object.record.sha256.clone();
                    reserved_objects.push(receipt_object.clone());
                    view_receipt_objects.push(receipt_object.record.clone());
                    views.push(view);
                }

                let lineage_receipt_object = self
                    .store
                    .get_object(&lineage.receipt_object_sha256)?
                    .ok_or_else(|| invalid("baseline lineage receipt CAS missing"))?;
                let rig_v2_object = self
                    .store
                    .get_object(&lineage.registered_rig_v2_object_sha256)?
                    .ok_or_else(|| invalid("baseline RigV2 CAS missing"))?;
                // Six detached renders may take minutes. Re-pin the live
                // candidate immediately before the durable Store transaction so
                // no mutable candidate projection can silently drift underneath
                // the approved lineage while the views are being produced.
                validate_authoritative_base_version_binding(
                    self,
                    &request.project_id,
                    &request.candidate_id,
                    &request.candidate_state_sha256,
                    &request.artifact_id,
                    &request.artifact_sha256,
                    request.base_version_id.as_deref(),
                )?;
                let mut baseline = ProductionWeaponFormArtBaselineRecord {
                    schema_version: PRODUCTION_WEAPON_FORM_ART_BASELINE_SCHEMA_VERSION.to_owned(),
                    baseline_id: request.baseline_id.clone(),
                    registration_lineage_id: lineage.registration_lineage_id.clone(),
                    registration_lineage_canonical_sha256: lineage.canonical_sha256.clone(),
                    registration_lineage_receipt_object_sha256: lineage
                        .receipt_object_sha256
                        .clone(),
                    registered_rig_v2_id,
                    registered_rig_v2_object_sha256: lineage
                        .registered_rig_v2_object_sha256
                        .clone(),
                    registered_rig_v2_canonical_sha256: lineage
                        .registered_rig_v2_canonical_sha256
                        .clone(),
                    session_id: request.session_id.clone(),
                    project_id: request.project_id.clone(),
                    candidate_id: request.candidate_id.clone(),
                    candidate_state_sha256: request.candidate_state_sha256.clone(),
                    artifact_id: request.artifact_id.clone(),
                    artifact_sha256: request.artifact_sha256.clone(),
                    base_version_id: request.base_version_id.clone(),
                    view_kinds: PRODUCTION_WEAPON_FORM_ART_BASELINE_VIEW_KINDS
                        .iter()
                        .map(|value| (*value).to_owned())
                        .collect(),
                    views,
                    runtime_build_cohort_sha256: runtime_cohort.clone(),
                    baseline_policy: PRODUCTION_WEAPON_FORM_ART_BASELINE_POLICY.to_owned(),
                    materialization_status:
                        PRODUCTION_WEAPON_FORM_ART_BASELINE_MATERIALIZATION_STATUS.to_owned(),
                    historical_form_art_reused: false,
                    worker_started: true,
                    worker_cohort_verified: true,
                    quality_status: PRODUCTION_WEAPON_FORM_ART_BASELINE_QUALITY_STATUS.to_owned(),
                    visual_status: "NOT_PROVEN".to_owned(),
                    human_status: "NOT_RUN".to_owned(),
                    engine_status: "NOT_RUN".to_owned(),
                    distribution_status: "NOT_RUN".to_owned(),
                    promotion_eligible: false,
                    runtime_write_performed: true,
                    persistent_user_data_touched: true,
                    production_stage_advanced: false,
                    candidate_confirmed: false,
                    version_created: false,
                    export_performed: false,
                    request_sha256: request_sha256.clone(),
                    input_sha256: request.input_sha256.clone(),
                    idempotency_key: request.idempotency_key.clone(),
                    idempotency_policy: PRODUCTION_WEAPON_FORM_ART_BASELINE_IDEMPOTENCY_POLICY
                        .to_owned(),
                    writer_policy: PRODUCTION_WEAPON_FORM_ART_BASELINE_WRITER_POLICY.to_owned(),
                    receipt_object_sha256: String::new(),
                    canonicalization_policy:
                        PRODUCTION_WEAPON_FORM_ART_BASELINE_CANONICALIZATION_POLICY.to_owned(),
                    canonical_sha256: String::new(),
                    created_at: super::now_string(),
                };
                let mut canonical_baseline =
                    serde_json::to_value(&baseline).map_err(|error| invalid(error.to_string()))?;
                canonical_baseline["receipt_object_sha256"] = Value::String(String::new());
                canonical_baseline["canonical_sha256"] = Value::String(String::new());
                for view in canonical_baseline["views"]
                    .as_array_mut()
                    .ok_or_else(|| invalid("baseline views serialization failed"))?
                {
                    view["receipt_object_sha256"] = Value::String(String::new());
                    view["canonical_sha256"] = Value::String(String::new());
                }
                baseline.canonical_sha256 = canonical_json_hash(&canonical_baseline);
                let mut parent_payload =
                    serde_json::to_value(&baseline).map_err(|error| invalid(error.to_string()))?;
                parent_payload["receipt_object_sha256"] = Value::String(String::new());
                let parent_object = self
                    .store
                    .put_production_weapon_form_art_baseline_cas_object(
                        &batch,
                        &canonical_json_bytes(&parent_payload)
                            .map_err(|error| invalid(error.to_string()))?,
                        None,
                        "application/json",
                        PRODUCTION_WEAPON_FORM_ART_BASELINE_PARENT_OBJECT_KIND,
                        &super::now_string(),
                    )?;
                baseline.receipt_object_sha256 = parent_object.record.sha256.clone();
                reserved_objects.push(parent_object.clone());
                let bundle = ProductionWeaponFormArtBaselineCommitBundle::new(
                    baseline.clone(),
                    parent_object.record.clone(),
                    view_receipt_objects,
                    lineage_receipt_object,
                    rig_v2_object,
                );
                // The six detached renders above may take minutes.  Revalidate
                // every Runtime-owned source binding at the last point before
                // Store commit so a changed cohort, lineage/receipt, RigV2,
                // ReferenceCanvas or candidate/artifact cannot be persisted.
                validate_exact_baseline_source_binding(self, &baseline)?;
                let (stored, replayed) = self
                    .store
                    .commit_production_weapon_form_art_baseline_with_replay(&bundle)?;
                if replayed {
                    // A concurrent idempotent writer may win the Store race
                    // after this invocation rendered its own graph.  The
                    // replayed record is authoritative and must pass the same
                    // Runtime semantic source validation before it is exposed.
                    validate_exact_baseline_source_binding(self, &stored)?;
                }
                if !replayed {
                    self.store
                        .complete_production_weapon_form_art_baseline_cas_batch(&batch, &stored)?;
                }
                cleanup_reserved_objects = replayed;
                let readback = self
                    .store
                    .get_production_weapon_form_art_baseline_by_baseline_id(&stored.baseline_id)?
                    .ok_or_else(|| invalid("baseline Store readback missing"))?;
                if readback != stored {
                    return Err(invalid(
                        "PRODUCTION_WEAPON_FORM_ART_BASELINE_STORE_READBACK_MISMATCH",
                    ));
                }
                prepare_result_value(
                    stored,
                    request_sha256.clone(),
                    request.input_sha256.clone(),
                    replayed,
                )
            })();
            match operation {
                Ok(value) => {
                    release_baseline_reservation(
                        self,
                        batch.reservation(),
                        &reserved_objects,
                        cleanup_reserved_objects,
                    )?;
                    Ok(value)
                }
                Err(error) => {
                    match release_baseline_reservation(
                        self,
                        batch.reservation(),
                        &reserved_objects,
                        cleanup_reserved_objects,
                    ) {
                        Ok(()) => Err(error),
                        Err(cleanup_error) => Err(invalid(format!(
                            "baseline prepare failed ({error}); CAS rollback failed ({cleanup_error})"
                        ))),
                    }
                }
            }
        })();
        match owner_result {
            Ok(value) => match serde_json::to_string(&value) {
                Ok(response_json) => {
                    self.form_art_baseline_flights.complete(
                        &single_flight,
                        BaselineSingleFlightOutcome::Completed(response_json),
                    );
                    Ok(value)
                }
                Err(error) => {
                    let error = invalid(format!(
                        "PRODUCTION_WEAPON_FORM_ART_BASELINE_SINGLE_FLIGHT_RESULT_SERIALIZE_FAILED: {error}"
                    ));
                    self.form_art_baseline_flights.complete(
                        &single_flight,
                        BaselineSingleFlightOutcome::Failed(error.to_string()),
                    );
                    Err(error)
                }
            },
            Err(error) => {
                self.form_art_baseline_flights.complete(
                    &single_flight,
                    BaselineSingleFlightOutcome::Failed(error.to_string()),
                );
                Err(error)
            }
        }
    }

    pub fn production_weapon_form_art_baseline_get(
        &self,
        request_value: Value,
    ) -> Result<Value, RuntimeError> {
        let request: ProductionWeaponFormArtBaselineGetRequest =
            serde_json::from_value(request_value.clone()).map_err(|error| {
                invalid(format!(
                    "PRODUCTION_WEAPON_FORM_ART_BASELINE_GET_REQUEST_INVALID: {error}"
                ))
            })?;
        if request.schema_version != PRODUCTION_WEAPON_FORM_ART_BASELINE_GET_REQUEST_SCHEMA_VERSION
            || request.operation != PRODUCTION_WEAPON_FORM_ART_BASELINE_GET_OPERATION
        {
            return Err(invalid(
                "PRODUCTION_WEAPON_FORM_ART_BASELINE_GET_SCHEMA_OR_OPERATION_MISMATCH",
            ));
        }
        validate_baseline_request_common(
            &request.baseline_id,
            &request.registration_lineage_id,
            &request.registration_lineage_canonical_sha256,
            &request.session_id,
            &request.project_id,
            &request.candidate_id,
            &request.candidate_state_sha256,
            &request.artifact_id,
            &request.artifact_sha256,
            request.base_version_id.as_deref(),
            &request.idempotency_key,
            request.max_response_bytes,
            &request.writer_policy,
            &request.canonicalization_policy,
            request.runtime_write_performed,
            request.persistent_user_data_touched,
            &request.input_sha256,
        )?;
        validate_authoritative_base_version_binding(
            self,
            &request.project_id,
            &request.candidate_id,
            &request.candidate_state_sha256,
            &request.artifact_id,
            &request.artifact_sha256,
            request.base_version_id.as_deref(),
        )?;
        let request_sha256 = baseline_request_hash(&request_value, &request.input_sha256)?;
        let baseline = self
            .store
            .get_production_weapon_form_art_baseline_by_baseline_id(&request.baseline_id)?
            .ok_or_else(|| invalid("PRODUCTION_WEAPON_FORM_ART_BASELINE_NOT_FOUND"))?;
        if !baseline_scope_matches(
            &baseline,
            &request.baseline_id,
            &request.registration_lineage_id,
            &request.registration_lineage_canonical_sha256,
            &request.session_id,
            &request.project_id,
            &request.candidate_id,
            &request.candidate_state_sha256,
            &request.artifact_id,
            &request.artifact_sha256,
            request.base_version_id.as_deref(),
            &request.idempotency_key,
        ) {
            return Err(invalid(
                "PRODUCTION_WEAPON_FORM_ART_BASELINE_GET_SCOPE_MISMATCH",
            ));
        }
        validate_persisted_baseline_source_binding(self, &baseline)?;
        get_result_value(baseline, request_sha256, request.input_sha256)
    }
}
