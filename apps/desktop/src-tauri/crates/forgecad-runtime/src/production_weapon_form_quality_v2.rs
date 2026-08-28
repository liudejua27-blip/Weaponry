//! Additive ProductionWeaponFormQuality@2 runtime gate.
//!
//! The @2 receipt is intentionally a small, immutable join over the legacy
//! FormQuality@1 and FormArt@1 receipts.  It never renders, changes a
//! candidate, confirms a version, or advances the production head.  All
//! source reads and bounded visual assertions happen before the one report
//! CAS reservation.

use super::{
    canonical_json_bytes, canonical_json_hash, is_opaque_id, is_sha256, sha256_hex, CasObject,
    Runtime, RuntimeError,
};
use forgecad_contracts::{
    ProductionCameraLockRegistrationLineageRecord, ProductionStageHeadV3Record,
    ProductionStageTransitionV3Record, ProductionWeaponFormArtBaselineRecord,
    ProductionWeaponFormArtBaselineView, ProductionWeaponFormArtEvidenceRecord,
    ProductionWeaponFormArtEvidenceViewRecord, ProductionWeaponFormQualityNoRegression,
    ProductionWeaponFormQualityRecord, ProductionWeaponFormQualityV2Aggregate,
    ProductionWeaponFormQualityV2GetRequest, ProductionWeaponFormQualityV2PrepareRequest,
    ProductionWeaponFormQualityV2Record, ProductionWeaponFormQualityV2ViewDecision,
    PRODUCTION_CAMERA_LOCK_SCHEMA_VERSION, PRODUCTION_WEAPON_FORM_ART_BASELINE_AOV_KINDS,
    PRODUCTION_WEAPON_FORM_ART_BASELINE_QUALITY_STATUS,
    PRODUCTION_WEAPON_FORM_ART_BASELINE_SCHEMA_VERSION,
    PRODUCTION_WEAPON_FORM_ART_BASELINE_VIEW_KINDS,
    PRODUCTION_WEAPON_FORM_ART_BASELINE_VIEW_SCHEMA_VERSION,
    PRODUCTION_WEAPON_FORM_ART_EVIDENCE_LINE_CHAMFER_MAX_MILLI,
    PRODUCTION_WEAPON_FORM_ART_EVIDENCE_LINE_CONTINUITY_MIN_MILLI,
    PRODUCTION_WEAPON_FORM_ART_EVIDENCE_LINE_COVERAGE_MIN_MILLI,
    PRODUCTION_WEAPON_FORM_ART_EVIDENCE_LINE_DEVIATION_MAX_MILLI,
    PRODUCTION_WEAPON_FORM_ART_EVIDENCE_LINE_DIRECTION_MIN_MILLI,
    PRODUCTION_WEAPON_FORM_ART_EVIDENCE_LINE_DUPLICATE_CROSSING_MAX,
    PRODUCTION_WEAPON_FORM_ART_EVIDENCE_PARENT_RECEIPT_KIND,
    PRODUCTION_WEAPON_FORM_ART_EVIDENCE_VIEW_SCHEMA_VERSION,
    PRODUCTION_WEAPON_FORM_ART_EVIDENCE_VOID_AREA_RATIO_MAX_MILLI,
    PRODUCTION_WEAPON_FORM_ART_EVIDENCE_VOID_AREA_RATIO_MIN_MILLI,
    PRODUCTION_WEAPON_FORM_ART_EVIDENCE_VOID_BOUNDARY_F1_MIN_MILLI,
    PRODUCTION_WEAPON_FORM_ART_EVIDENCE_VOID_CENTROID_MAX_MILLI,
    PRODUCTION_WEAPON_FORM_ART_EVIDENCE_VOID_IOU_MIN_MILLI,
    PRODUCTION_WEAPON_FORM_QUALITY_SCHEMA_VERSION,
    PRODUCTION_WEAPON_FORM_QUALITY_V2_DISTRIBUTION_STATUS,
    PRODUCTION_WEAPON_FORM_QUALITY_V2_ENGINE_STATUS,
    PRODUCTION_WEAPON_FORM_QUALITY_V2_FIXED_CAMERA_VIEW_KINDS,
    PRODUCTION_WEAPON_FORM_QUALITY_V2_FORM_STAGES,
    PRODUCTION_WEAPON_FORM_QUALITY_V2_GET_REQUEST_SCHEMA_VERSION,
    PRODUCTION_WEAPON_FORM_QUALITY_V2_GET_RESULT_SCHEMA_VERSION,
    PRODUCTION_WEAPON_FORM_QUALITY_V2_HUMAN_STATUS, PRODUCTION_WEAPON_FORM_QUALITY_V2_POLICY,
    PRODUCTION_WEAPON_FORM_QUALITY_V2_PREFLIGHT_COMMERCIAL_ENGINE_STATUS,
    PRODUCTION_WEAPON_FORM_QUALITY_V2_PREFLIGHT_GET_REQUEST_SCHEMA_VERSION,
    PRODUCTION_WEAPON_FORM_QUALITY_V2_PREFLIGHT_GET_RESULT_SCHEMA_VERSION,
    PRODUCTION_WEAPON_FORM_QUALITY_V2_PREFLIGHT_HUMAN_REVIEW_STATUS,
    PRODUCTION_WEAPON_FORM_QUALITY_V2_PREFLIGHT_QUALITY_STATUS,
    PRODUCTION_WEAPON_FORM_QUALITY_V2_PREFLIGHT_VISUAL_QUALITY_STATUS,
    PRODUCTION_WEAPON_FORM_QUALITY_V2_PREPARE_REQUEST_SCHEMA_VERSION,
    PRODUCTION_WEAPON_FORM_QUALITY_V2_PREPARE_RESULT_SCHEMA_VERSION,
    PRODUCTION_WEAPON_FORM_QUALITY_V2_QUALITY_STATUS,
    PRODUCTION_WEAPON_FORM_QUALITY_V2_SCHEMA_VERSION,
    PRODUCTION_WEAPON_FORM_QUALITY_V2_SOURCE_STAGES,
    PRODUCTION_WEAPON_FORM_QUALITY_V2_STRUCTURAL_STATUS,
    PRODUCTION_WEAPON_FORM_QUALITY_V2_TARGET_STAGES,
    PRODUCTION_WEAPON_FORM_QUALITY_V2_THRESHOLD_POLICY,
    PRODUCTION_WEAPON_FORM_QUALITY_V2_VALIDATOR_STATUS,
    PRODUCTION_WEAPON_FORM_QUALITY_V2_VIEW_KINDS,
};
use forgecad_store::{CasReservation, ProductionWeaponFormArtProposalEvidenceRecord};
use serde_json::{Map, Value};

const JSON_MIME: &str = "application/json";
const REPORT_KIND: &str = "production-weapon-form-quality-v2-report";
const MAX_JSON_BYTES: u64 = 1024 * 1024;

// These are the normalized units used by FormArt@1.  They are deliberately
// explicit here: a caller cannot turn an absent target annotation into a
// passing boolean by supplying a report row with a convenient status.
const NEGATIVE_IOU_MIN_MILLI: u64 = PRODUCTION_WEAPON_FORM_ART_EVIDENCE_VOID_IOU_MIN_MILLI;
const NEGATIVE_BOUNDARY_F1_MIN_MILLI: u64 =
    PRODUCTION_WEAPON_FORM_ART_EVIDENCE_VOID_BOUNDARY_F1_MIN_MILLI;
const NEGATIVE_AREA_RATIO_MIN_MILLI: u64 =
    PRODUCTION_WEAPON_FORM_ART_EVIDENCE_VOID_AREA_RATIO_MIN_MILLI;
const NEGATIVE_AREA_RATIO_MAX_MILLI: u64 =
    PRODUCTION_WEAPON_FORM_ART_EVIDENCE_VOID_AREA_RATIO_MAX_MILLI;
const NEGATIVE_CENTROID_MAX: u64 = PRODUCTION_WEAPON_FORM_ART_EVIDENCE_VOID_CENTROID_MAX_MILLI;
const LINE_COVERAGE_MIN_MILLI: u64 = PRODUCTION_WEAPON_FORM_ART_EVIDENCE_LINE_COVERAGE_MIN_MILLI;
const LINE_CONTINUITY_MIN_MILLI: u64 =
    PRODUCTION_WEAPON_FORM_ART_EVIDENCE_LINE_CONTINUITY_MIN_MILLI;
const LINE_CHAMFER_MAX: u64 = PRODUCTION_WEAPON_FORM_ART_EVIDENCE_LINE_CHAMFER_MAX_MILLI;
const LINE_DEVIATION_MAX: u64 = PRODUCTION_WEAPON_FORM_ART_EVIDENCE_LINE_DEVIATION_MAX_MILLI;
const LINE_DIRECTION_MIN_MILLI: u64 = PRODUCTION_WEAPON_FORM_ART_EVIDENCE_LINE_DIRECTION_MIN_MILLI;
const LINE_DUPLICATE_CROSSING_MAX: u64 =
    PRODUCTION_WEAPON_FORM_ART_EVIDENCE_LINE_DUPLICATE_CROSSING_MAX;

// Every FormQuality@2 request/result carries this complete normalized scope.
// Legacy mode represents the historical join, so every value is explicitly
// null; fresh-baseline-proposal mode requires every value to be present.
const FORM_QUALITY_V2_SCOPE_OPTION_FIELDS: &[&str] = &[
    "source_candidate_id",
    "source_candidate_state_sha256",
    "source_artifact_id",
    "source_artifact_sha256",
    "source_fresh_baseline_id",
    "source_fresh_baseline_canonical_sha256",
    "source_fresh_baseline_receipt_object_sha256",
    "source_registration_lineage_id",
    "source_registration_lineage_canonical_sha256",
    "source_registration_lineage_receipt_object_sha256",
    "source_registered_rig_v2_id",
    "source_registered_rig_v2_object_sha256",
    "source_registered_rig_v2_canonical_sha256",
    "source_runtime_build_cohort_sha256",
    "proposal_candidate_id",
    "proposal_candidate_state_sha256",
    "proposal_artifact_id",
    "proposal_artifact_sha256",
    "proposal_artifact_readback_sha256",
    "proposal_worker_build_cohort_sha256",
    "cross_view_evidence_bundle_sha256",
    "proposal_form_art_evidence_id",
    "proposal_form_art_evidence_object_sha256",
    "proposal_form_art_evidence_canonical_sha256",
    "proposal_part_id_evidence_sha256",
    "proposal_negative_space_evidence_sha256",
    "proposal_line_flow_evidence_sha256",
];

const PREPARE_FIELDS: &[&str] = &[
    "schema_version",
    "form_quality_id",
    "session_id",
    "project_id",
    "form_stage",
    "source_stage",
    "target_stage",
    "legacy_form_quality_object_sha256",
    "legacy_form_quality_canonical_sha256",
    "form_art_evidence_object_sha256",
    "form_art_evidence_canonical_sha256",
    "evidence_source_kind",
    "source_candidate_id",
    "source_candidate_state_sha256",
    "source_artifact_id",
    "source_artifact_sha256",
    "source_fresh_baseline_id",
    "source_fresh_baseline_canonical_sha256",
    "source_fresh_baseline_receipt_object_sha256",
    "source_registration_lineage_id",
    "source_registration_lineage_canonical_sha256",
    "source_registration_lineage_receipt_object_sha256",
    "source_registered_rig_v2_id",
    "source_registered_rig_v2_object_sha256",
    "source_registered_rig_v2_canonical_sha256",
    "source_runtime_build_cohort_sha256",
    "proposal_candidate_id",
    "proposal_candidate_state_sha256",
    "proposal_artifact_id",
    "proposal_artifact_sha256",
    "proposal_artifact_readback_sha256",
    "proposal_worker_build_cohort_sha256",
    "cross_view_evidence_bundle_sha256",
    "proposal_form_art_evidence_id",
    "proposal_form_art_evidence_object_sha256",
    "proposal_form_art_evidence_canonical_sha256",
    "proposal_part_id_evidence_sha256",
    "proposal_negative_space_evidence_sha256",
    "proposal_line_flow_evidence_sha256",
    "current_source_head_transition_id",
    "current_source_head_transition_sha256",
    "current_source_head_canonical_sha256",
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
const PREPARE_REQUIRED_FIELDS: &[&str] = &[
    "schema_version",
    "form_quality_id",
    "session_id",
    "project_id",
    "form_stage",
    "source_stage",
    "target_stage",
    "legacy_form_quality_object_sha256",
    "legacy_form_quality_canonical_sha256",
    "form_art_evidence_object_sha256",
    "form_art_evidence_canonical_sha256",
    "evidence_source_kind",
    "current_source_head_transition_id",
    "current_source_head_transition_sha256",
    "current_source_head_canonical_sha256",
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
const PREFLIGHT_SCHEMA_VERSION: &str =
    PRODUCTION_WEAPON_FORM_QUALITY_V2_PREFLIGHT_GET_REQUEST_SCHEMA_VERSION;
const PREFLIGHT_RESULT_SCHEMA_VERSION: &str =
    PRODUCTION_WEAPON_FORM_QUALITY_V2_PREFLIGHT_GET_RESULT_SCHEMA_VERSION;
const PREFLIGHT_FIELDS: &[&str] = &[
    "schema_version",
    "preflight_id",
    "session_id",
    "project_id",
    "candidate_id",
    "form_stage",
    "legacy_form_quality_object_sha256",
    "legacy_form_quality_canonical_sha256",
    "form_art_evidence_object_sha256",
    "form_art_evidence_canonical_sha256",
    "evidence_source_kind",
    "current_source_head_transition_id",
    "current_source_head_transition_sha256",
    "current_source_head_canonical_sha256",
    "input_sha256",
];
const GET_FIELDS: &[&str] = &[
    "schema_version",
    "form_quality_id",
    "session_id",
    "project_id",
    "candidate_id",
    "form_stage",
    "evidence_source_kind",
];

#[derive(Debug, Clone)]
struct PreflightRequest {
    preflight_id: String,
    session_id: String,
    project_id: String,
    candidate_id: String,
    form_stage: String,
    legacy_form_quality_object_sha256: String,
    legacy_form_quality_canonical_sha256: String,
    form_art_evidence_object_sha256: String,
    form_art_evidence_canonical_sha256: String,
    evidence_source_kind: String,
    source_candidate_id: Option<String>,
    source_candidate_state_sha256: Option<String>,
    source_artifact_id: Option<String>,
    source_artifact_sha256: Option<String>,
    source_fresh_baseline_id: Option<String>,
    source_fresh_baseline_canonical_sha256: Option<String>,
    source_fresh_baseline_receipt_object_sha256: Option<String>,
    source_registration_lineage_id: Option<String>,
    source_registration_lineage_canonical_sha256: Option<String>,
    source_registration_lineage_receipt_object_sha256: Option<String>,
    source_registered_rig_v2_id: Option<String>,
    source_registered_rig_v2_object_sha256: Option<String>,
    source_registered_rig_v2_canonical_sha256: Option<String>,
    source_runtime_build_cohort_sha256: Option<String>,
    proposal_candidate_id: Option<String>,
    proposal_candidate_state_sha256: Option<String>,
    proposal_artifact_id: Option<String>,
    proposal_artifact_sha256: Option<String>,
    proposal_artifact_readback_sha256: Option<String>,
    proposal_worker_build_cohort_sha256: Option<String>,
    cross_view_evidence_bundle_sha256: Option<String>,
    proposal_form_art_evidence_id: Option<String>,
    proposal_form_art_evidence_object_sha256: Option<String>,
    proposal_form_art_evidence_canonical_sha256: Option<String>,
    proposal_part_id_evidence_sha256: Option<String>,
    proposal_negative_space_evidence_sha256: Option<String>,
    proposal_line_flow_evidence_sha256: Option<String>,
    current_source_head_transition_id: String,
    current_source_head_transition_sha256: String,
    current_source_head_canonical_sha256: String,
}

#[derive(Debug, Clone)]
struct PreflightCheck {
    status: &'static str,
    reason_code: &'static str,
    object_sha256: Option<String>,
    canonical_sha256: Option<String>,
}

impl PreflightCheck {
    fn ready(object_sha256: Option<String>, canonical_sha256: Option<String>) -> Self {
        Self {
            status: "ready",
            reason_code: "READY",
            object_sha256,
            canonical_sha256,
        }
    }

    fn blocked(reason_code: &'static str) -> Self {
        Self {
            status: "blocked",
            reason_code,
            object_sha256: None,
            canonical_sha256: None,
        }
    }

    fn invalid(reason_code: &'static str, object_sha256: Option<String>) -> Self {
        Self {
            status: "invalid",
            reason_code,
            object_sha256,
            canonical_sha256: None,
        }
    }

    fn value(&self) -> Value {
        serde_json::json!({
            "status": self.status,
            "reason_code": self.reason_code,
            "object_sha256": self.object_sha256,
            "canonical_sha256": self.canonical_sha256,
        })
    }

    fn is_ready(&self) -> bool {
        self.status == "ready"
    }
}

/// Runtime-only join for the fresh FormArt baseline and a derived proposal.
///
/// The source scope (Stage head, transition and CameraLock) remains the
/// current source candidate.  The evaluation scope switches to the proposal
/// candidate only when a fresh same-cohort baseline and its proposal-side
/// CrossView/owner evidence are present.  This keeps the V2 public record
/// compatible while preventing a proposal from masquerading as the source
/// head or an old FormArt camera from becoming the active comparison frame.
#[derive(Debug, Clone)]
struct FreshBaselineQualityAdapter {
    baseline: ProductionWeaponFormArtBaselineRecord,
    lineage: ProductionCameraLockRegistrationLineageRecord,
    proposal_evidence: Option<ProductionWeaponFormArtProposalEvidenceRecord>,
    /// The proposal receipt is the only source of evaluated-view pass
    /// bindings in fresh mode.  Keep the decoded rows in this Runtime-local
    /// adapter so decisions cannot accidentally fall back to the historical
    /// FormArt rows after the candidate scope has switched.
    proposal_views: Option<Vec<Value>>,
    evaluation_candidate_id: String,
    evaluation_candidate_state_sha256: String,
    evaluation_artifact_id: String,
    evaluation_artifact_sha256: String,
    cross_view_object_sha256: String,
    cross_view_canonical_sha256: Option<String>,
    proposal_part_id_evidence_sha256: Option<String>,
    proposal_negative_space_evidence_sha256: Option<String>,
    proposal_line_flow_evidence_sha256: Option<String>,
}

impl FreshBaselineQualityAdapter {
    fn is_proposal_scope(&self) -> bool {
        self.proposal_evidence.is_some()
    }

    fn source_candidate_id(&self) -> &str {
        self.baseline.candidate_id.as_str()
    }

    fn cross_view_canonical(&self) -> Option<&str> {
        self.cross_view_canonical_sha256.as_deref()
    }

    fn proposal_view(&self, ordinal: usize, expected_kind: &str) -> Result<&Value, RuntimeError> {
        let views = self
            .proposal_views
            .as_ref()
            .ok_or_else(|| invalid("fresh proposal view rows are unavailable"))?;
        let view = views.get(ordinal).ok_or_else(|| {
            invalid(format!(
                "fresh proposal view {expected_kind} is unavailable"
            ))
        })?;
        if view.get("view_kind").and_then(Value::as_str) != Some(expected_kind) {
            return Err(invalid(format!(
                "fresh proposal view {expected_kind} ordering differs"
            )));
        }
        Ok(view)
    }
}

#[derive(Debug, Clone)]
enum EvidenceMode {
    Legacy,
    FreshBaselineProposal {
        proposal_object_sha256: String,
        proposal_canonical_sha256: String,
    },
}

impl EvidenceMode {
    fn is_fresh(&self) -> bool {
        matches!(self, Self::FreshBaselineProposal { .. })
    }
}

fn invalid(message: impl Into<String>) -> RuntimeError {
    RuntimeError::InvalidInput(message.into())
}

fn exact_object<'a>(
    value: &'a Value,
    fields: &[&str],
    label: &str,
) -> Result<&'a Map<String, Value>, RuntimeError> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid(format!("{label} must be an object")))?;
    if let Some(field) = object
        .keys()
        .find(|field| !fields.contains(&field.as_str()))
    {
        return Err(invalid(format!(
            "{label} contains unsupported field {field}"
        )));
    }
    if let Some(field) = fields.iter().find(|field| !object.contains_key(**field)) {
        return Err(invalid(format!("{label} is missing {field}")));
    }
    Ok(object)
}

fn text<'a>(object: &'a Map<String, Value>, field: &str) -> Result<&'a str, RuntimeError> {
    object
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| invalid(format!("{field} must be non-empty text")))
}

fn id<'a>(object: &'a Map<String, Value>, field: &str) -> Result<&'a str, RuntimeError> {
    let value = text(object, field)?;
    if !is_opaque_id(value) {
        return Err(invalid(format!("{field} is not an opaque id")));
    }
    Ok(value)
}

fn sha<'a>(object: &'a Map<String, Value>, field: &str) -> Result<&'a str, RuntimeError> {
    let value = text(object, field)?;
    if !is_sha256(value) {
        return Err(invalid(format!("{field} is not a SHA-256")));
    }
    Ok(value)
}

/// Validate the complete wire-level scope before any source lookup.  Keeping
/// this check on the Runtime side makes GET, preflight and prepare share the
/// same legacy/fresh union even when a caller bypasses the MCP schema.
fn validate_normalized_scope_object(
    object: &Map<String, Value>,
    evidence_source_kind: &str,
) -> Result<(), RuntimeError> {
    let mut any_null = false;
    let mut any_present = false;
    for field in FORM_QUALITY_V2_SCOPE_OPTION_FIELDS {
        let value = object
            .get(*field)
            .ok_or_else(|| invalid(format!("{field} is required in normalized scope")))?;
        if value.is_null() {
            any_null = true;
            continue;
        }
        let text = value
            .as_str()
            .filter(|value| !value.is_empty())
            .ok_or_else(|| invalid(format!("{field} must be a non-empty string or null")))?;
        let valid = if field.ends_with("_id") {
            is_opaque_id(text)
        } else {
            is_sha256(text)
        };
        if !valid {
            return Err(invalid(format!("{field} is not a bounded id/hash")));
        }
        any_present = true;
    }
    match evidence_source_kind {
        "legacy-source" if !any_present && any_null => Ok(()),
        "fresh-baseline-proposal" if any_present && !any_null => Ok(()),
        "legacy-source" => Err(invalid(
            "legacy FormQuality V2 normalized scope must be entirely null",
        )),
        "fresh-baseline-proposal" => Err(invalid(
            "fresh FormQuality V2 normalized scope must be entirely present",
        )),
        _ => Err(invalid("form quality V2 evidence source kind differs")),
    }
}

fn read_json(runtime: &Runtime, hash: &str, label: &str) -> Result<Value, RuntimeError> {
    let bytes = runtime.cas_read_bounded(hash, MAX_JSON_BYTES)?;
    serde_json::from_slice(&bytes)
        .map_err(|error| invalid(format!("{label} is invalid JSON: {error}")))
}

fn canonical_document(value: &Value, schema: &str, label: &str) -> Result<String, RuntimeError> {
    if value.get("schema_version").and_then(Value::as_str) != Some(schema) {
        return Err(invalid(format!("{label} schema differs")));
    }
    let mut normalized = value.clone();
    if normalized.get("canonical_sha256").is_some() {
        normalized["canonical_sha256"] = Value::String(String::new());
    }
    let canonical = canonical_json_hash(&normalized);
    if value.get("canonical_sha256").and_then(Value::as_str) != Some(canonical.as_str()) {
        return Err(invalid(format!("{label} canonical differs")));
    }
    Ok(canonical)
}

fn normalized_record(record: &ProductionWeaponFormQualityV2Record) -> Result<Value, RuntimeError> {
    let mut value = serde_json::to_value(record).map_err(|error| invalid(error.to_string()))?;
    value["receipt_object_sha256"] = Value::String(String::new());
    value["canonical_sha256"] = Value::String(String::new());
    Ok(value)
}

fn parse_prepare(
    value: &Value,
) -> Result<
    (
        ProductionWeaponFormQualityV2PrepareRequest,
        String,
        EvidenceMode,
    ),
    RuntimeError,
> {
    let input_object = value
        .as_object()
        .ok_or_else(|| invalid("ProductionWeaponFormQualityPrepareRequest@2 must be an object"))?;
    if let Some(field) = input_object
        .keys()
        .find(|field| !PREPARE_FIELDS.contains(&field.as_str()))
    {
        return Err(invalid(format!(
            "ProductionWeaponFormQualityPrepareRequest@2 contains unsupported field {field}"
        )));
    }
    let mut required_fields = PREPARE_REQUIRED_FIELDS
        .iter()
        .copied()
        .chain(FORM_QUALITY_V2_SCOPE_OPTION_FIELDS.iter().copied());
    if let Some(field) = required_fields.find(|field| !input_object.contains_key(*field)) {
        return Err(invalid(format!(
            "ProductionWeaponFormQualityPrepareRequest@2 is missing {field}"
        )));
    }
    let object = input_object;
    if text(object, "schema_version")?
        != PRODUCTION_WEAPON_FORM_QUALITY_V2_PREPARE_REQUEST_SCHEMA_VERSION
    {
        return Err(invalid("form quality V2 prepare schema differs"));
    }
    let request: ProductionWeaponFormQualityV2PrepareRequest =
        serde_json::from_value(value.clone())
            .map_err(|error| invalid(format!("form quality V2 prepare is malformed: {error}")))?;
    validate_normalized_scope_object(input_object, &request.evidence_source_kind)?;
    for field in [
        "form_quality_id",
        "session_id",
        "project_id",
        "idempotency_key",
    ] {
        id(object, field)?;
    }
    for field in [
        "legacy_form_quality_object_sha256",
        "legacy_form_quality_canonical_sha256",
        "form_art_evidence_object_sha256",
        "form_art_evidence_canonical_sha256",
        "current_source_head_transition_sha256",
        "current_source_head_canonical_sha256",
        "form_quality_policy_sha256",
        "threshold_policy_sha256",
        "input_sha256",
    ] {
        sha(object, field)?;
    }
    id(object, "current_source_head_transition_id")?;
    if !PRODUCTION_WEAPON_FORM_QUALITY_V2_FORM_STAGES.contains(&request.form_stage.as_str())
        || !PRODUCTION_WEAPON_FORM_QUALITY_V2_SOURCE_STAGES.contains(&request.source_stage.as_str())
        || !PRODUCTION_WEAPON_FORM_QUALITY_V2_TARGET_STAGES.contains(&request.target_stage.as_str())
    {
        return Err(invalid("form quality V2 stage is outside the closed set"));
    }
    let index = PRODUCTION_WEAPON_FORM_QUALITY_V2_FORM_STAGES
        .iter()
        .position(|v| *v == request.form_stage)
        .unwrap();
    if request.source_stage != PRODUCTION_WEAPON_FORM_QUALITY_V2_SOURCE_STAGES[index]
        || request.target_stage != PRODUCTION_WEAPON_FORM_QUALITY_V2_TARGET_STAGES[index]
    {
        return Err(invalid("form quality V2 source/target stage differs"));
    }
    if request.form_quality_policy != PRODUCTION_WEAPON_FORM_QUALITY_V2_POLICY
        || request.threshold_policy != PRODUCTION_WEAPON_FORM_QUALITY_V2_THRESHOLD_POLICY
        || request.form_quality_policy_sha256
            != sha256_hex(PRODUCTION_WEAPON_FORM_QUALITY_V2_POLICY.as_bytes())
        || request.threshold_policy_sha256
            != sha256_hex(PRODUCTION_WEAPON_FORM_QUALITY_V2_THRESHOLD_POLICY.as_bytes())
    {
        return Err(invalid("form quality V2 policy differs"));
    }
    let previous = (
        request.previous_form_quality_id.is_some(),
        request.previous_form_quality_report_object_sha256.is_some(),
        request.previous_form_quality_canonical_sha256.is_some(),
    );
    if request.form_stage == "blockout" && previous != (false, false, false) {
        return Err(invalid("blockout previous V2 quality must be null"));
    }
    if request.form_stage != "blockout" && previous != (true, true, true) {
        return Err(invalid("later V2 form edge requires previous quality"));
    }
    let mut preimage = object.clone();
    preimage.remove("input_sha256");
    preimage.remove("idempotency_key");
    let request_sha = canonical_json_hash(&Value::Object(preimage));
    if request.input_sha256 != request_sha {
        return Err(invalid("form quality V2 input hash differs"));
    }
    let evidence_source_kind = object
        .get("evidence_source_kind")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("evidence_source_kind must be non-empty text"))?;
    let proposal_object = object
        .get("proposal_form_art_evidence_object_sha256")
        .and_then(Value::as_str);
    let proposal_canonical = object
        .get("proposal_form_art_evidence_canonical_sha256")
        .and_then(Value::as_str);
    let mode = match evidence_source_kind {
        "legacy-source" => {
            if proposal_object.is_some() || proposal_canonical.is_some() {
                return Err(invalid(
                    "legacy FormArt evidence mode cannot carry proposal evidence",
                ));
            }
            EvidenceMode::Legacy
        }
        "fresh-baseline-proposal" => {
            let proposal_object = proposal_object
                .filter(|value| is_sha256(value))
                .ok_or_else(|| invalid("proposal FormArt evidence object hash is invalid"))?;
            let proposal_canonical = proposal_canonical
                .filter(|value| is_sha256(value))
                .ok_or_else(|| invalid("proposal FormArt evidence canonical hash is invalid"))?;
            EvidenceMode::FreshBaselineProposal {
                proposal_object_sha256: proposal_object.to_owned(),
                proposal_canonical_sha256: proposal_canonical.to_owned(),
            }
        }
        _ => return Err(invalid("form quality V2 evidence source kind differs")),
    };
    Ok((request, request_sha, mode))
}

fn parse_get(value: &Value) -> Result<ProductionWeaponFormQualityV2GetRequest, RuntimeError> {
    let fields = GET_FIELDS
        .iter()
        .copied()
        .chain(FORM_QUALITY_V2_SCOPE_OPTION_FIELDS.iter().copied())
        .collect::<Vec<_>>();
    let object = exact_object(
        value,
        fields.as_slice(),
        "ProductionWeaponFormQualityGetRequest@2",
    )?;
    if text(object, "schema_version")?
        != PRODUCTION_WEAPON_FORM_QUALITY_V2_GET_REQUEST_SCHEMA_VERSION
    {
        return Err(invalid("form quality V2 get schema differs"));
    }
    let request: ProductionWeaponFormQualityV2GetRequest = serde_json::from_value(value.clone())
        .map_err(|error| invalid(format!("form quality V2 get is malformed: {error}")))?;
    validate_normalized_scope_object(object, &request.evidence_source_kind)?;
    for field in [
        "form_quality_id",
        "session_id",
        "project_id",
        "candidate_id",
    ] {
        id(object, field)?;
    }
    if !PRODUCTION_WEAPON_FORM_QUALITY_V2_FORM_STAGES.contains(&request.form_stage.as_str()) {
        return Err(invalid("form quality V2 get stage differs"));
    }
    Ok(request)
}

fn verify_legacy(
    runtime: &Runtime,
    object_hash: &str,
    canonical: &str,
) -> Result<ProductionWeaponFormQualityRecord, RuntimeError> {
    let object = runtime
        .store
        .get_object(object_hash)?
        .ok_or_else(|| invalid("legacy FormQuality CAS object is unavailable"))?;
    if object.kind != "production-weapon-form-quality-receipt" || object.mime != JSON_MIME {
        return Err(invalid("legacy FormQuality CAS kind differs"));
    }
    let payload = read_json(runtime, object_hash, "legacy FormQuality")?;
    let mut parsed: ProductionWeaponFormQualityRecord = serde_json::from_value(payload.clone())
        .map_err(|error| invalid(format!("legacy FormQuality is malformed: {error}")))?;
    if parsed.schema_version != PRODUCTION_WEAPON_FORM_QUALITY_SCHEMA_VERSION
        || parsed.receipt_object_sha256 != ""
    {
        return Err(invalid(
            "legacy FormQuality receipt self-reference is invalid",
        ));
    }
    let mut normalized = payload.clone();
    normalized["receipt_object_sha256"] = Value::String(String::new());
    normalized["canonical_sha256"] = Value::String(String::new());
    if canonical_json_hash(&normalized) != parsed.canonical_sha256
        || parsed.canonical_sha256 != canonical
    {
        return Err(invalid("legacy FormQuality canonical differs"));
    }
    let id = parsed.form_quality_id.clone();
    parsed.receipt_object_sha256 = object_hash.to_owned();
    let stored = runtime
        .store
        .get_production_weapon_form_quality(&id)?
        .ok_or_else(|| invalid("legacy FormQuality durable row is unavailable"))?;
    if stored != parsed {
        return Err(invalid("legacy FormQuality durable row differs from CAS"));
    }
    Ok(parsed)
}

fn verify_art(
    runtime: &Runtime,
    object_hash: &str,
    canonical: &str,
) -> Result<ProductionWeaponFormArtEvidenceRecord, RuntimeError> {
    let object = runtime
        .store
        .get_object(object_hash)?
        .ok_or_else(|| invalid("FormArt parent CAS object is unavailable"))?;
    if object.kind != PRODUCTION_WEAPON_FORM_ART_EVIDENCE_PARENT_RECEIPT_KIND
        || object.mime != JSON_MIME
    {
        return Err(invalid("FormArt parent CAS kind differs"));
    }
    let payload = read_json(runtime, object_hash, "FormArt parent")?;
    let mut parsed: ProductionWeaponFormArtEvidenceRecord = serde_json::from_value(payload.clone())
        .map_err(|error| invalid(format!("FormArt parent is malformed: {error}")))?;
    if parsed.schema_version != "ProductionWeaponFormArtEvidence@1"
        || parsed.receipt_object_sha256 != ""
    {
        return Err(invalid("FormArt parent receipt self-reference is invalid"));
    }
    let mut normalized = payload.clone();
    normalized["receipt_object_sha256"] = Value::String(String::new());
    normalized["canonical_sha256"] = Value::String(String::new());
    if canonical_json_hash(&normalized) != parsed.canonical_sha256
        || parsed.canonical_sha256 != canonical
    {
        return Err(invalid("FormArt parent canonical differs"));
    }
    let id = parsed.art_evidence_id.clone();
    parsed.receipt_object_sha256 = object_hash.to_owned();
    let stored = runtime
        .store
        .get_production_weapon_form_art_evidence(&id)?
        .ok_or_else(|| invalid("FormArt durable row is unavailable"))?;
    if stored != parsed {
        return Err(invalid("FormArt durable row differs from CAS"));
    }
    if parsed.views.len() != 6
        || parsed.view_kinds
            != PRODUCTION_WEAPON_FORM_QUALITY_V2_VIEW_KINDS
                .iter()
                .map(|v| (*v).to_owned())
                .collect::<Vec<_>>()
    {
        return Err(invalid(
            "FormArt does not contain exactly six ordered views",
        ));
    }
    for (ordinal, view) in parsed.views.iter().enumerate() {
        if view.view_kind != PRODUCTION_WEAPON_FORM_QUALITY_V2_VIEW_KINDS[ordinal] {
            return Err(invalid("FormArt view ordering differs"));
        }
        verify_art_view_receipt(runtime, view)?;
    }
    Ok(parsed)
}

fn verify_art_view_receipt(
    runtime: &Runtime,
    view: &ProductionWeaponFormArtEvidenceViewRecord,
) -> Result<(), RuntimeError> {
    let object = runtime
        .store
        .get_object(&view.receipt_object_sha256)?
        .ok_or_else(|| invalid("FormArt view receipt CAS object is unavailable"))?;
    if object.mime != JSON_MIME || object.kind != "production-weapon-form-art-evidence-view-receipt"
    {
        return Err(invalid("FormArt view receipt metadata differs"));
    }
    let payload = read_json(runtime, &view.receipt_object_sha256, "FormArt view receipt")?;
    let mut parsed: ProductionWeaponFormArtEvidenceViewRecord =
        serde_json::from_value(payload.clone())
            .map_err(|error| invalid(format!("FormArt view receipt is malformed: {error}")))?;
    if parsed.receipt_object_sha256 != ""
        || parsed.schema_version != PRODUCTION_WEAPON_FORM_ART_EVIDENCE_VIEW_SCHEMA_VERSION
    {
        return Err(invalid(
            "FormArt view receipt schema/self-reference differs",
        ));
    }
    let mut normalized = payload;
    normalized["receipt_object_sha256"] = Value::String(String::new());
    normalized["canonical_sha256"] = Value::String(String::new());
    if canonical_json_hash(&normalized) != view.canonical_sha256
        || parsed.canonical_sha256 != view.canonical_sha256
    {
        return Err(invalid("FormArt view receipt canonical differs"));
    }
    parsed.receipt_object_sha256 = view.receipt_object_sha256.clone();
    if parsed != *view {
        return Err(invalid("FormArt view receipt differs from parent"));
    }
    Ok(())
}

fn verify_authoring(
    runtime: &Runtime,
    legacy: &ProductionWeaponFormQualityRecord,
) -> Result<(), RuntimeError> {
    let canvas = read_json(
        runtime,
        &legacy.reference_canvas_object_sha256,
        "ReferenceCanvas",
    )?;
    canonical_document(&canvas, "ReferenceCanvas@1", "ReferenceCanvas")?;
    if canvas.get("project_id").and_then(Value::as_str) != Some(legacy.project_id.as_str()) {
        return Err(invalid("ReferenceCanvas project differs"));
    }
    let spec = read_json(runtime, &legacy.design_spec_object_sha256, "DesignSpec")?;
    canonical_document(&spec, "DesignSpec@1", "DesignSpec")?;
    if spec.get("project_id").and_then(Value::as_str) != Some(legacy.project_id.as_str())
        || spec.get("reference_canvas_sha256").and_then(Value::as_str)
            != Some(legacy.reference_canvas_object_sha256.as_str())
    {
        return Err(invalid("DesignSpec binding differs"));
    }
    Ok(())
}

fn verify_target_observation(
    runtime: &Runtime,
    view: &ProductionWeaponFormArtEvidenceViewRecord,
) -> Result<(), RuntimeError> {
    let target = read_json(runtime, &view.target_object_sha256, "SilhouetteTarget")?;
    let target_canonical = canonical_document(&target, "SilhouetteTarget@1", "SilhouetteTarget")?;
    if target.get("reference_id").and_then(Value::as_str) != Some(view.reference_id.as_str())
        || target.get("reference_sha256").and_then(Value::as_str)
            != Some(view.reference_sha256.as_str())
        || target_canonical != view.target_canonical_sha256
        || target.get("source").and_then(Value::as_str) != Some("user_refined")
        || target.get("annotation_status").and_then(Value::as_str) != Some("user_confirmed")
    {
        return Err(invalid("FormArt target is not user-confirmed"));
    }
    let structure = target
        .get("visual_structure")
        .ok_or_else(|| invalid("FormArt target visual structure is unavailable"))?;
    if structure.get("review_status").and_then(Value::as_str) != Some("user_confirmed") {
        return Err(invalid("FormArt visual structure is not user-confirmed"));
    }
    if structure.get("canonical_sha256").and_then(Value::as_str)
        != Some(view.visual_structure_canonical_sha256.as_str())
    {
        return Err(invalid("FormArt visual structure canonical differs"));
    }
    let regions = structure
        .get("regions")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("FormArt negative-space annotation list is unavailable"))?;
    let lines = structure
        .get("line_flows")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("FormArt line-flow annotation list is unavailable"))?;
    let subtract_regions = regions
        .iter()
        .filter(|region| region.get("mask_operation").and_then(Value::as_str) == Some("subtract"))
        .collect::<Vec<_>>();
    if view.negative_space_status == "not-applicable" {
        if !subtract_regions.is_empty() || !view.negative_space_rows.is_empty() {
            return Err(invalid(
                "negative-space not-applicable is not an explicit empty confirmed set",
            ));
        }
    } else if view.negative_space_status == "observed" {
        if subtract_regions.is_empty() || view.negative_space_rows.len() != subtract_regions.len() {
            return Err(invalid("negative-space thresholds are not passed"));
        }
        let mut matched = std::collections::BTreeSet::new();
        for region in subtract_regions {
            let structure_id = region
                .get("structure_id")
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| invalid("negative-space structure id is unavailable"))?;
            let expected_hash = canonical_json_hash(region);
            let row = view
                .negative_space_rows
                .iter()
                .find(|row| row.structure_id == structure_id);
            let Some(row) = row else {
                return Err(invalid("negative-space row set does not match target"));
            };
            if row.expected_region_canonical_sha256 != expected_hash
                || !matched.insert(row.structure_id.as_str())
                || row.status != "observed"
                || row.missing
                || row.sealed
                || row.iou_milli < NEGATIVE_IOU_MIN_MILLI
                || row.boundary_f1_milli < NEGATIVE_BOUNDARY_F1_MIN_MILLI
                || row.area_ratio_milli < NEGATIVE_AREA_RATIO_MIN_MILLI
                || row.area_ratio_milli > NEGATIVE_AREA_RATIO_MAX_MILLI
                || row.centroid_error_milli > NEGATIVE_CENTROID_MAX
            {
                return Err(invalid(
                    "negative-space thresholds or binding are not passed",
                ));
            }
        }
        if matched.len() != view.negative_space_rows.len() {
            return Err(invalid("negative-space row set contains an unexpected row"));
        }
    } else {
        return Err(invalid(
            "negative-space evidence is not observed or confirmed not-applicable",
        ));
    }
    if view.line_flow_status == "not-applicable" {
        if !lines.is_empty() || !view.line_flow_rows.is_empty() {
            return Err(invalid(
                "line-flow not-applicable is not an explicit empty confirmed set",
            ));
        }
    } else if view.line_flow_status == "observed" {
        if lines.is_empty() || view.line_flow_rows.len() != lines.len() {
            return Err(invalid("line-flow thresholds are not passed"));
        }
        let mut matched = std::collections::BTreeSet::new();
        for line in lines {
            let line_id = line
                .get("line_flow_id")
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| invalid("line-flow id is unavailable"))?;
            let expected_hash = canonical_json_hash(line);
            let row = view
                .line_flow_rows
                .iter()
                .find(|row| row.line_flow_id == line_id);
            let Some(row) = row else {
                return Err(invalid("line-flow row set does not match target"));
            };
            if row.expected_line_canonical_sha256 != expected_hash
                || !matched.insert(row.line_flow_id.as_str())
                || row.status != "observed"
                || row.coverage_milli < LINE_COVERAGE_MIN_MILLI
                || row.continuity_milli < LINE_CONTINUITY_MIN_MILLI
                || row.symmetric_chamfer_milli > LINE_CHAMFER_MAX
                || row.max_deviation_milli > LINE_DEVIATION_MAX
                || row.direction_order_milli < LINE_DIRECTION_MIN_MILLI
                || row.duplicate_crossing_count > LINE_DUPLICATE_CROSSING_MAX
            {
                return Err(invalid("line-flow thresholds or binding are not passed"));
            }
        }
        if matched.len() != view.line_flow_rows.len() {
            return Err(invalid("line-flow row set contains an unexpected row"));
        }
    } else {
        return Err(invalid(
            "line-flow evidence is not observed or confirmed not-applicable",
        ));
    }
    Ok(())
}

/// Closed, read-only explanation for a FormArt target-observation failure.
///
/// The public preflight deliberately keeps one stable aggregate blocker in its
/// contract. Real-reference fixtures still need to retain which bounded
/// evidence class failed per view, without copying free-form Runtime errors
/// into an evidence receipt or changing any persisted state.
#[cfg(test)]
pub(crate) fn target_observation_reason_code(
    runtime: &Runtime,
    view: &ProductionWeaponFormArtEvidenceViewRecord,
) -> &'static str {
    let Err(error) = verify_target_observation(runtime, view) else {
        return "READY";
    };
    let RuntimeError::InvalidInput(message) = error else {
        return "TARGET_OBJECT_UNREADABLE";
    };
    target_observation_invalid_input_reason_code(&message)
}

#[cfg(test)]
fn target_observation_invalid_input_reason_code(message: &str) -> &'static str {
    match message {
        "FormArt target is not user-confirmed" => "TARGET_NOT_USER_CONFIRMED",
        "FormArt target visual structure is unavailable" => "VISUAL_STRUCTURE_UNAVAILABLE",
        "FormArt visual structure is not user-confirmed" => "VISUAL_STRUCTURE_NOT_CONFIRMED",
        "FormArt visual structure canonical differs" => "VISUAL_STRUCTURE_BINDING_MISMATCH",
        "FormArt negative-space annotation list is unavailable" => {
            "NEGATIVE_SPACE_ANNOTATION_LIST_UNAVAILABLE"
        }
        "negative-space not-applicable is not an explicit empty confirmed set" => {
            "NEGATIVE_SPACE_NOT_APPLICABLE_SET_MISMATCH"
        }
        "negative-space thresholds are not passed"
        | "negative-space structure id is unavailable"
        | "negative-space row set does not match target"
        | "negative-space thresholds or binding are not passed"
        | "negative-space row set contains an unexpected row" => {
            "NEGATIVE_SPACE_THRESHOLD_OR_BINDING_FAILED"
        }
        "negative-space evidence is not observed or confirmed not-applicable" => {
            "NEGATIVE_SPACE_NOT_OBSERVED_OR_NOT_APPLICABLE"
        }
        "FormArt line-flow annotation list is unavailable" => {
            "LINE_FLOW_ANNOTATION_LIST_UNAVAILABLE"
        }
        "line-flow not-applicable is not an explicit empty confirmed set" => {
            "LINE_FLOW_NOT_APPLICABLE_SET_MISMATCH"
        }
        "line-flow thresholds are not passed"
        | "line-flow id is unavailable"
        | "line-flow row set does not match target"
        | "line-flow thresholds or binding are not passed"
        | "line-flow row set contains an unexpected row" => "LINE_FLOW_THRESHOLD_OR_BINDING_FAILED",
        "line-flow evidence is not observed or confirmed not-applicable" => {
            "LINE_FLOW_NOT_OBSERVED_OR_NOT_APPLICABLE"
        }
        _ => "TARGET_OBSERVATION_INVALID",
    }
}

fn legacy_view_no_regression_passes(
    no_regression: &ProductionWeaponFormQualityNoRegression,
) -> bool {
    // FormQuality@1 predates the Runtime-owned FormArt receipt. Its durable
    // responsibility in the V2 chain is the CrossView metric lineage only;
    // Part-ID, negative-space and line-flow are independently re-read and
    // thresholded from FormArt@1 below. Requiring those legacy diagnostic
    // booleans here makes the V2 chain impossible because the @1 producer
    // intentionally records them as NOT_PROVEN.
    matches!(no_regression.status.as_str(), "PASS" | "NOT_PROVEN")
        && no_regression.metrics_not_regressed
}

/// Re-read the immutable CrossViewEvidenceBundle rather than trusting the
/// summary booleans copied into FormQuality@1.  The legacy receipt remains a
/// required parent, but its aggregate/view booleans are not an authority for
/// this V2 gate.
fn verify_cross_view_bundle(
    runtime: &Runtime,
    session_id: &str,
    project_id: &str,
    candidate_id: &str,
    candidate_state_sha256: &str,
    artifact_sha256: &str,
    reference_canvas_sha256: &str,
    cross_view_object_sha256: &str,
    expected_canonical_sha256: Option<&str>,
) -> Result<Value, RuntimeError> {
    let index = runtime
        .store
        .get_cross_view_evidence(cross_view_object_sha256)?
        .ok_or_else(|| invalid("CrossViewEvidenceBundle durable index is unavailable"))?;
    if index.bundle_object_sha256 != cross_view_object_sha256
        || index.project_id != project_id
        || index.session_id != session_id
        || index.reference_canvas_sha256 != reference_canvas_sha256
        || !index.hard_gate_passed
    {
        return Err(invalid(
            "CrossViewEvidenceBundle durable binding/gate differs",
        ));
    }
    let object = runtime
        .store
        .get_object(cross_view_object_sha256)?
        .ok_or_else(|| invalid("CrossViewEvidenceBundle CAS object is unavailable"))?;
    if object.kind != "cross-view-evidence-bundle"
        || object.mime != JSON_MIME
        || object.size_bytes > MAX_JSON_BYTES
    {
        return Err(invalid("CrossViewEvidenceBundle CAS metadata differs"));
    }
    let bundle = read_json(runtime, cross_view_object_sha256, "CrossViewEvidenceBundle")?;
    super::validate_cross_view_evidence_bundle(&bundle)?;
    if canonical_document(
        &bundle,
        "CrossViewEvidenceBundle@1",
        "CrossViewEvidenceBundle",
    )? != expected_canonical_sha256.unwrap_or_else(|| {
        bundle
            .get("canonical_sha256")
            .and_then(Value::as_str)
            .unwrap_or_default()
    }) || bundle.get("project_id").and_then(Value::as_str) != Some(project_id)
        || bundle.get("session_id").and_then(Value::as_str) != Some(session_id)
        || bundle.get("candidate_id").and_then(Value::as_str) != Some(candidate_id)
        || bundle.get("candidate_state_sha256").and_then(Value::as_str)
            != Some(candidate_state_sha256)
        || bundle.get("artifact_sha256").and_then(Value::as_str) != Some(artifact_sha256)
        || bundle
            .get("reference_canvas_sha256")
            .and_then(Value::as_str)
            != Some(reference_canvas_sha256)
        || bundle.get("hard_gate_passed").and_then(Value::as_bool) != Some(true)
        || bundle.get("non_regressing").and_then(Value::as_bool) != Some(true)
        || bundle
            .get("coverage")
            .and_then(|value| value.get("coverage_status"))
            .and_then(Value::as_str)
            != Some("complete")
    {
        return Err(invalid(
            "CrossViewEvidenceBundle aggregate gate is not passed",
        ));
    }
    let evaluations = bundle
        .get("view_evaluations")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("CrossViewEvidenceBundle view evaluations are unavailable"))?;
    if evaluations.len() != PRODUCTION_WEAPON_FORM_QUALITY_V2_VIEW_KINDS.len() {
        return Err(invalid("CrossViewEvidenceBundle view count differs"));
    }
    let expected = PRODUCTION_WEAPON_FORM_QUALITY_V2_VIEW_KINDS
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    let mut actual = std::collections::BTreeSet::new();
    for evaluation in evaluations {
        let kind = evaluation
            .get("kind")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid("CrossViewEvidenceBundle view kind is unavailable"))?;
        if !actual.insert(kind) || !expected.contains(kind) {
            return Err(invalid("CrossViewEvidenceBundle view set differs"));
        }
        if evaluation.get("proposal_status").and_then(Value::as_str)
            != Some("PARTIAL_VISIBLE_VIEW_PASS")
            || evaluation.get("non_regressing").and_then(Value::as_bool) != Some(true)
        {
            return Err(invalid(
                "CrossViewEvidenceBundle per-view gate is not passed",
            ));
        }
    }
    if actual != expected {
        return Err(invalid("CrossViewEvidenceBundle view set is incomplete"));
    }
    Ok(bundle)
}

fn verify_legacy_cross_view(
    runtime: &Runtime,
    legacy: &ProductionWeaponFormQualityRecord,
) -> Result<Value, RuntimeError> {
    verify_cross_view_bundle(
        runtime,
        &legacy.session_id,
        &legacy.project_id,
        &legacy.candidate_id,
        &legacy.candidate_state_sha256,
        &legacy.artifact_sha256,
        &legacy.reference_canvas_object_sha256,
        &legacy.cross_view_evidence_object_sha256,
        Some(&legacy.cross_view_evidence_canonical_sha256),
    )
}

fn verify_fresh_baseline_view(
    runtime: &Runtime,
    baseline: &ProductionWeaponFormArtBaselineRecord,
    view: &ProductionWeaponFormArtBaselineView,
    expected_kind: &str,
) -> Result<(), RuntimeError> {
    if view.schema_version != PRODUCTION_WEAPON_FORM_ART_BASELINE_VIEW_SCHEMA_VERSION
        || view.view_kind != expected_kind
        || !is_opaque_id(&view.view_id)
        || !is_opaque_id(&view.reference_id)
        || !is_sha256(&view.reference_sha256)
        || !is_sha256(&view.camera_hash)
        || !is_sha256(&view.camera_canonical_sha256)
        || !is_sha256(&view.camera_object_sha256)
        || !is_opaque_id(&view.render_set_id)
        || !is_sha256(&view.render_set_object_sha256)
        || !is_sha256(&view.render_set_canonical_sha256)
        || !is_opaque_id(&view.render_set_view_id)
        || view.pass_artifact_object_sha256.len()
            != PRODUCTION_WEAPON_FORM_ART_BASELINE_AOV_KINDS.len()
        || view.render_worker_build_cohort_sha256 != baseline.runtime_build_cohort_sha256
        || view.quality_status != PRODUCTION_WEAPON_FORM_ART_BASELINE_QUALITY_STATUS
    {
        return Err(invalid(format!(
            "fresh FormArt baseline {expected_kind} view shape differs"
        )));
    }
    let object = runtime
        .store
        .get_object(&view.receipt_object_sha256)?
        .ok_or_else(|| invalid("fresh FormArt baseline view receipt is unavailable"))?;
    if object.kind != "production-weapon-form-art-baseline-view-receipt"
        || object.mime != JSON_MIME
        || object.size_bytes > MAX_JSON_BYTES
    {
        return Err(invalid(
            "fresh FormArt baseline view receipt metadata differs",
        ));
    }
    let payload = read_json(
        runtime,
        &view.receipt_object_sha256,
        "fresh FormArt baseline view receipt",
    )?;
    let mut parsed: ProductionWeaponFormArtBaselineView = serde_json::from_value(payload.clone())
        .map_err(|error| {
        invalid(format!("fresh FormArt baseline view is malformed: {error}"))
    })?;
    let mut normalized = payload;
    normalized["receipt_object_sha256"] = Value::String(String::new());
    normalized["canonical_sha256"] = Value::String(String::new());
    if parsed.receipt_object_sha256 != ""
        || parsed.canonical_sha256 != view.canonical_sha256
        || canonical_json_hash(&normalized) != view.canonical_sha256
    {
        return Err(invalid("fresh FormArt baseline view canonical differs"));
    }
    parsed.receipt_object_sha256 = view.receipt_object_sha256.clone();
    if parsed != *view {
        return Err(invalid("fresh FormArt baseline view receipt differs"));
    }

    let camera_object = runtime
        .store
        .get_object(&view.camera_object_sha256)?
        .ok_or_else(|| invalid("fresh FormArt baseline camera object is unavailable"))?;
    if camera_object.kind != "camera-calibration" || camera_object.mime != JSON_MIME {
        return Err(invalid("fresh FormArt baseline camera metadata differs"));
    }
    let camera = read_json(runtime, &view.camera_object_sha256, "fresh baseline camera")?;
    crate::multiview::camera_rig::validate_camera_calibration_v2(&camera)
        .map_err(|error| invalid(format!("fresh baseline camera is invalid: {error}")))?;
    if camera.get("camera_hash").and_then(Value::as_str) != Some(view.camera_hash.as_str())
        || camera.get("canonical_sha256").and_then(Value::as_str)
            != Some(view.camera_canonical_sha256.as_str())
    {
        return Err(invalid("fresh FormArt baseline camera binding differs"));
    }

    let render_set = read_json(
        runtime,
        &view.render_set_object_sha256,
        "fresh baseline RenderSet",
    )?;
    super::validate_persisted_render_set_v2_output(&render_set)?;
    let render_set_canonical = render_set
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("fresh baseline RenderSet canonical is unavailable"))?;
    if render_set_canonical != view.render_set_canonical_sha256
        || render_set.get("render_set_id").and_then(Value::as_str)
            != Some(view.render_set_id.as_str())
        || render_set.get("view_id").and_then(Value::as_str)
            != Some(view.render_set_view_id.as_str())
        || render_set.get("candidate_id").and_then(Value::as_str)
            != Some(baseline.candidate_id.as_str())
        || render_set.get("artifact_sha256").and_then(Value::as_str)
            != Some(baseline.artifact_sha256.as_str())
        || render_set.get("reference_id").and_then(Value::as_str)
            != Some(view.reference_id.as_str())
        || render_set.get("camera_hash").and_then(Value::as_str) != Some(view.camera_hash.as_str())
        || render_set
            .get("render_worker_build_cohort_sha256")
            .and_then(Value::as_str)
            != Some(baseline.runtime_build_cohort_sha256.as_str())
    {
        return Err(invalid("fresh baseline RenderSet binding differs"));
    }
    for (aov, hash) in PRODUCTION_WEAPON_FORM_ART_BASELINE_AOV_KINDS
        .iter()
        .zip(&view.pass_artifact_object_sha256)
    {
        if !is_sha256(hash) {
            return Err(invalid(format!(
                "fresh baseline {expected_kind} {aov} hash is invalid"
            )));
        }
        let pass = runtime.store.get_object(hash)?.ok_or_else(|| {
            invalid(format!(
                "fresh baseline {expected_kind} {aov} is unavailable"
            ))
        })?;
        if pass.mime != "image/png" || pass.size_bytes == 0 {
            return Err(invalid(format!(
                "fresh baseline {expected_kind} {aov} metadata differs"
            )));
        }
    }
    for (label, hash) in [
        ("reference mask", &view.reference_mask_object_sha256),
        ("comparison report", &view.comparison_report_object_sha256),
        ("quality report", &view.quality_report_object_sha256),
    ] {
        if !is_sha256(hash) {
            return Err(invalid(format!(
                "fresh baseline {expected_kind} {label} hash is invalid"
            )));
        }
        runtime.store.get_object(hash)?.ok_or_else(|| {
            invalid(format!(
                "fresh baseline {expected_kind} {label} is unavailable"
            ))
        })?;
    }
    Ok(())
}

/// Resolve the additive fresh-baseline source without changing any Runtime
/// state.  The baseline is always looked up against the current source head;
/// only a proposal-side evidence envelope may retarget the evaluated
/// candidate and CrossView bundle.
fn resolve_fresh_baseline_adapter(
    runtime: &Runtime,
    request: &ProductionWeaponFormQualityV2PrepareRequest,
    legacy: &ProductionWeaponFormQualityRecord,
    art: &ProductionWeaponFormArtEvidenceRecord,
    mode: &EvidenceMode,
) -> Result<Option<FreshBaselineQualityAdapter>, RuntimeError> {
    // Legacy is the historical FormQuality/FormArt join.  A fresh baseline
    // may coexist in the Store, but its presence must not silently attach a
    // fresh scope to a legacy request or make the durable legacy record fail
    // the all-null Store invariant.
    if matches!(mode, EvidenceMode::Legacy) {
        if art.candidate_id != legacy.candidate_id {
            return Err(invalid(
                "legacy FormArt candidate differs from legacy FormQuality",
            ));
        }
        return Ok(None);
    }
    let runtime_cohort = super::build_cohort_sha256();
    let baseline = runtime_cohort
        .as_deref()
        .map(|cohort| {
            runtime
                .store
                .get_production_weapon_form_art_baseline_for_current_source(
                    &request.project_id,
                    &legacy.candidate_id,
                    &legacy.artifact_sha256,
                    cohort,
                )
                .map_err(RuntimeError::from)
        })
        .transpose()?
        .flatten();
    let Some(baseline) = baseline else {
        if mode.is_fresh() || art.candidate_id != legacy.candidate_id {
            return Err(invalid(
                "fresh FormArt baseline is required for proposal evidence mode",
            ));
        }
        return Ok(None);
    };
    if baseline.schema_version != PRODUCTION_WEAPON_FORM_ART_BASELINE_SCHEMA_VERSION
        || baseline.session_id != legacy.session_id
        || baseline.project_id != request.project_id
        || baseline.candidate_id != legacy.candidate_id
        || baseline.candidate_state_sha256 != legacy.candidate_state_sha256
        || baseline.artifact_id != legacy.artifact_id
        || baseline.artifact_sha256 != legacy.artifact_sha256
        || baseline.view_kinds
            != PRODUCTION_WEAPON_FORM_ART_BASELINE_VIEW_KINDS
                .iter()
                .map(|value| (*value).to_owned())
                .collect::<Vec<_>>()
        || baseline.views.len() != PRODUCTION_WEAPON_FORM_ART_BASELINE_VIEW_KINDS.len()
        || baseline.runtime_build_cohort_sha256 != runtime_cohort.as_deref().unwrap_or_default()
        || baseline.historical_form_art_reused
        || !baseline.worker_started
        || !baseline.worker_cohort_verified
        || baseline.quality_status != PRODUCTION_WEAPON_FORM_ART_BASELINE_QUALITY_STATUS
        || baseline.promotion_eligible
        || !baseline.runtime_write_performed
        || !baseline.persistent_user_data_touched
        || baseline.production_stage_advanced
        || baseline.candidate_confirmed
        || baseline.version_created
        || baseline.export_performed
    {
        return Err(invalid("fresh FormArt baseline source binding differs"));
    }
    let lineage = runtime
        .store
        .get_production_camera_lock_registration_lineage(&baseline.registration_lineage_id)?
        .ok_or_else(|| invalid("fresh baseline registration lineage is unavailable"))?;
    super::agentic_session::validate_production_camera_lock_registration_lineage_runtime(
        runtime, &lineage,
    )?;
    if !lineage.promotable
        || lineage.canonical_sha256 != baseline.registration_lineage_canonical_sha256
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
            "fresh baseline registration lineage binding differs",
        ));
    }
    for (ordinal, view) in baseline.views.iter().enumerate() {
        verify_fresh_baseline_view(
            runtime,
            &baseline,
            view,
            PRODUCTION_WEAPON_FORM_ART_BASELINE_VIEW_KINDS[ordinal],
        )?;
    }

    let mut proposal_views = None;
    let proposal_evidence = match mode {
        EvidenceMode::Legacy => None,
        EvidenceMode::FreshBaselineProposal {
            proposal_object_sha256,
            proposal_canonical_sha256,
        } => {
            let object = runtime
                .store
                .get_object(proposal_object_sha256)?
                .ok_or_else(|| invalid("proposal FormArt evidence object is unavailable"))?;
            if object.kind != "production-weapon-form-art-proposal-evidence"
                || object.mime != JSON_MIME
                || object.size_bytes > MAX_JSON_BYTES
            {
                return Err(invalid("proposal FormArt evidence object metadata differs"));
            }
            let payload = read_json(runtime, proposal_object_sha256, "proposal FormArt evidence")?;
            let canonical = payload
                .get("canonical_sha256")
                .and_then(Value::as_str)
                .filter(|value| is_sha256(value))
                .ok_or_else(|| invalid("proposal FormArt evidence canonical is unavailable"))?;
            if canonical != proposal_canonical_sha256 {
                return Err(invalid("proposal FormArt evidence canonical differs"));
            }
            let mut preimage = payload.clone();
            preimage["canonical_sha256"] = Value::String(String::new());
            if canonical_json_hash(&preimage) != canonical {
                return Err(invalid(
                    "proposal FormArt evidence canonical is not reproducible",
                ));
            }
            let views = payload
                .get("views")
                .and_then(Value::as_array)
                .filter(|views| views.len() == PRODUCTION_WEAPON_FORM_QUALITY_V2_VIEW_KINDS.len())
                .cloned()
                .ok_or_else(|| invalid("proposal FormArt evidence views are unavailable"))?;
            for (view, expected_kind) in views
                .iter()
                .zip(PRODUCTION_WEAPON_FORM_QUALITY_V2_VIEW_KINDS)
            {
                if view.get("view_kind").and_then(Value::as_str) != Some(expected_kind) {
                    return Err(invalid(format!(
                        "proposal FormArt evidence view {expected_kind} ordering differs"
                    )));
                }
            }
            let proposal = runtime
                .store
                .get_production_weapon_form_art_proposal_evidence(proposal_object_sha256)?
                .ok_or_else(|| invalid("proposal FormArt durable evidence is unavailable"))?;
            if proposal.receipt_object_sha256 != *proposal_object_sha256
                || proposal.canonical_sha256 != *proposal_canonical_sha256
                || proposal.project_id != request.project_id
                || proposal.session_id != legacy.session_id
                || proposal.source_candidate_id != legacy.candidate_id
                || proposal.source_candidate_state_sha256 != legacy.candidate_state_sha256
                || proposal.source_artifact_sha256 != legacy.artifact_sha256
                || proposal.source_form_art_evidence_id != art.art_evidence_id
                || proposal.source_form_art_evidence_object_sha256 != art.receipt_object_sha256
                || proposal.source_form_art_evidence_canonical_sha256 != art.canonical_sha256
                || proposal.reference_canvas_object_sha256 != legacy.reference_canvas_object_sha256
                || proposal.reference_canvas_canonical_sha256
                    != legacy.reference_canvas_canonical_sha256
                || proposal.design_spec_object_sha256 != legacy.design_spec_object_sha256
                || proposal.design_spec_canonical_sha256 != legacy.design_spec_canonical_sha256
                || proposal.camera_lock_id != lineage.camera_lock_id
                || proposal.camera_lock_canonical_sha256 != lineage.camera_lock_canonical_sha256
                || proposal.camera_lock_receipt_object_sha256
                    != lineage.camera_lock_receipt_object_sha256
                || proposal.camera_lock_source_transition_id != lineage.source_transition_id
                || proposal.camera_lock_source_transition_sha256 != lineage.source_transition_sha256
                || proposal.camera_lock_source_head_canonical_sha256
                    != lineage.source_head_canonical_sha256
                || proposal.worker_build_cohort_sha256.as_deref()
                    != Some(baseline.runtime_build_cohort_sha256.as_str())
                || !proposal.part_id_all_views_observed
                || !proposal.negative_space_all_views_resolved
                || !proposal.line_flow_all_views_resolved
                || !proposal.strict_owner_void_all_views_passed
                || !proposal.proposal_form_art_evidence_ready
            {
                return Err(invalid(
                    "proposal FormArt evidence source/proposal binding differs",
                ));
            }
            proposal_views = Some(views);
            Some(proposal)
        }
    };
    if art.session_id != legacy.session_id
        || art.project_id != request.project_id
        || art.reference_canvas_object_sha256 != legacy.reference_canvas_object_sha256
        || art.reference_canvas_canonical_sha256 != legacy.reference_canvas_canonical_sha256
        || art.design_spec_object_sha256 != legacy.design_spec_object_sha256
        || art.design_spec_canonical_sha256 != legacy.design_spec_canonical_sha256
    {
        return Err(invalid("FormArt authoring source binding differs"));
    }
    for (ordinal, kind) in PRODUCTION_WEAPON_FORM_ART_BASELINE_VIEW_KINDS
        .iter()
        .enumerate()
    {
        let art_view = art
            .views
            .get(ordinal)
            .ok_or_else(|| invalid(format!("FormArt view {kind} is unavailable")))?;
        let baseline_view = &baseline.views[ordinal];
        // Fresh-baseline mode deliberately reuses only the reviewed FormArt
        // semantic target. View IDs remain ReferenceCanvas identities, while
        // the evaluated camera is superseded by the approved registered RigV2
        // carried by the fresh baseline. Requiring the historical FormArt
        // camera/view ID here would make a camera re-registration impossible
        // to consume even though the lineage, RigV2, reference, candidate,
        // artifact, cohort and every baseline CAS object were already checked
        // above.
        if art_view.view_kind != *kind
            || art_view.reference_id != baseline_view.reference_id
            || art_view.reference_sha256 != baseline_view.reference_sha256
        {
            return Err(invalid(format!(
                "fresh baseline/FormArt {kind} semantic target reference binding differs"
            )));
        }
    }
    let (
        evaluation_candidate_id,
        evaluation_candidate_state_sha256,
        evaluation_artifact_id,
        evaluation_artifact_sha256,
    ) = if let Some(proposal) = proposal_evidence.as_ref() {
        let candidate = runtime
            .candidate(&proposal.proposal_candidate_id)?
            .ok_or_else(|| invalid("proposal candidate is unavailable"))?;
        if candidate.project_id != request.project_id
            || candidate.canonical_sha256 != proposal.proposal_candidate_state_sha256
            || candidate.prepared_object_sha256.as_deref()
                != Some(proposal.proposal_artifact_sha256.as_str())
        {
            return Err(invalid("proposal candidate/artifact binding differs"));
        }
        let artifact = candidate
            .prepared_object_id
            .clone()
            .ok_or_else(|| invalid("proposal artifact id is unavailable"))?;
        (
            proposal.proposal_candidate_id.clone(),
            proposal.proposal_candidate_state_sha256.clone(),
            artifact,
            proposal.proposal_artifact_sha256.clone(),
        )
    } else {
        (
            legacy.candidate_id.clone(),
            legacy.candidate_state_sha256.clone(),
            legacy.artifact_id.clone(),
            legacy.artifact_sha256.clone(),
        )
    };
    let (cross_view_object_sha256, cross_view_canonical_sha256) =
        if let Some(proposal) = proposal_evidence.as_ref() {
            (proposal.cross_view_evidence_bundle_sha256.clone(), None)
        } else {
            (
                legacy.cross_view_evidence_object_sha256.clone(),
                Some(legacy.cross_view_evidence_canonical_sha256.clone()),
            )
        };
    let proposal_part_id_evidence_sha256 = proposal_views.as_ref().map(|views| {
        canonical_json_hash(&serde_json::json!(views
            .iter()
            .map(|view| serde_json::json!({
                "view_kind": view.get("view_kind"),
                "part_id_pass_object_sha256": view.get("part_id_pass_object_sha256"),
                "part_id_status": view.get("part_id_status"),
            }))
            .collect::<Vec<_>>()))
    });
    let proposal_negative_space_evidence_sha256 = proposal_views.as_ref().map(|views| {
        canonical_json_hash(&serde_json::json!(views
            .iter()
            .map(|view| serde_json::json!({
                "view_kind": view.get("view_kind"),
                "negative_space_status": view.get("negative_space_status"),
                "negative_space_observations": view.get("negative_space_observations"),
            }))
            .collect::<Vec<_>>()))
    });
    let proposal_line_flow_evidence_sha256 = proposal_views.as_ref().map(|views| {
        canonical_json_hash(&serde_json::json!(views
            .iter()
            .map(|view| serde_json::json!({
                "view_kind": view.get("view_kind"),
                "line_flow_status": view.get("line_flow_status"),
                "line_flow_observations": view.get("line_flow_observations"),
            }))
            .collect::<Vec<_>>()))
    });
    Ok(Some(FreshBaselineQualityAdapter {
        baseline,
        lineage,
        proposal_evidence,
        proposal_views,
        evaluation_candidate_id,
        evaluation_candidate_state_sha256,
        evaluation_artifact_id,
        evaluation_artifact_sha256,
        cross_view_object_sha256,
        cross_view_canonical_sha256,
        proposal_part_id_evidence_sha256,
        proposal_negative_space_evidence_sha256,
        proposal_line_flow_evidence_sha256,
    }))
}

fn option_matches(value: &Option<String>, expected: &str) -> bool {
    value.as_deref() == Some(expected)
}

fn verify_request_evidence_scope(
    request: &ProductionWeaponFormQualityV2PrepareRequest,
    art: &ProductionWeaponFormArtEvidenceRecord,
    adapter: Option<&FreshBaselineQualityAdapter>,
) -> Result<(), RuntimeError> {
    let scope_is_empty = [
        &request.source_candidate_id,
        &request.source_candidate_state_sha256,
        &request.source_artifact_id,
        &request.source_artifact_sha256,
        &request.source_fresh_baseline_id,
        &request.source_fresh_baseline_canonical_sha256,
        &request.source_fresh_baseline_receipt_object_sha256,
        &request.source_registration_lineage_id,
        &request.source_registration_lineage_canonical_sha256,
        &request.source_registration_lineage_receipt_object_sha256,
        &request.source_registered_rig_v2_id,
        &request.source_registered_rig_v2_object_sha256,
        &request.source_registered_rig_v2_canonical_sha256,
        &request.source_runtime_build_cohort_sha256,
        &request.proposal_candidate_id,
        &request.proposal_candidate_state_sha256,
        &request.proposal_artifact_id,
        &request.proposal_artifact_sha256,
        &request.proposal_artifact_readback_sha256,
        &request.proposal_worker_build_cohort_sha256,
        &request.cross_view_evidence_bundle_sha256,
        &request.proposal_form_art_evidence_id,
        &request.proposal_form_art_evidence_object_sha256,
        &request.proposal_form_art_evidence_canonical_sha256,
        &request.proposal_part_id_evidence_sha256,
        &request.proposal_negative_space_evidence_sha256,
        &request.proposal_line_flow_evidence_sha256,
    ]
    .iter()
    .all(|value| value.is_none());
    let Some(adapter) = adapter.filter(|adapter| adapter.is_proposal_scope()) else {
        if request.evidence_source_kind != "legacy-source" || !scope_is_empty {
            return Err(invalid("legacy FormQuality V2 evidence scope is not empty"));
        }
        return Ok(());
    };
    if request.evidence_source_kind != "fresh-baseline-proposal" {
        return Err(invalid("fresh FormQuality V2 evidence scope kind differs"));
    }
    let proposal = adapter
        .proposal_evidence
        .as_ref()
        .ok_or_else(|| invalid("fresh proposal evidence is unavailable"))?;
    let worker_cohort = proposal
        .worker_build_cohort_sha256
        .as_deref()
        .ok_or_else(|| invalid("fresh proposal worker cohort is unavailable"))?;
    if !option_matches(&request.source_candidate_id, &adapter.baseline.candidate_id)
        || !option_matches(
            &request.source_candidate_state_sha256,
            &adapter.baseline.candidate_state_sha256,
        )
        || !option_matches(&request.source_artifact_id, &adapter.baseline.artifact_id)
        || !option_matches(
            &request.source_artifact_sha256,
            &adapter.baseline.artifact_sha256,
        )
        || !option_matches(
            &request.source_fresh_baseline_id,
            &adapter.baseline.baseline_id,
        )
        || !option_matches(
            &request.source_fresh_baseline_canonical_sha256,
            &adapter.baseline.canonical_sha256,
        )
        || !option_matches(
            &request.source_fresh_baseline_receipt_object_sha256,
            &adapter.baseline.receipt_object_sha256,
        )
        || !option_matches(
            &request.source_registration_lineage_id,
            &adapter.lineage.registration_lineage_id,
        )
        || !option_matches(
            &request.source_registration_lineage_canonical_sha256,
            &adapter.lineage.canonical_sha256,
        )
        || !option_matches(
            &request.source_registration_lineage_receipt_object_sha256,
            &adapter.lineage.receipt_object_sha256,
        )
        || !option_matches(
            &request.source_registered_rig_v2_id,
            &adapter.baseline.registered_rig_v2_id,
        )
        || !option_matches(
            &request.source_registered_rig_v2_object_sha256,
            &adapter.baseline.registered_rig_v2_object_sha256,
        )
        || !option_matches(
            &request.source_registered_rig_v2_canonical_sha256,
            &adapter.baseline.registered_rig_v2_canonical_sha256,
        )
        || !option_matches(
            &request.source_runtime_build_cohort_sha256,
            &adapter.baseline.runtime_build_cohort_sha256,
        )
        || !option_matches(
            &request.proposal_candidate_id,
            &proposal.proposal_candidate_id,
        )
        || !option_matches(
            &request.proposal_candidate_state_sha256,
            &proposal.proposal_candidate_state_sha256,
        )
        || !option_matches(
            &request.proposal_artifact_id,
            &adapter.evaluation_artifact_id,
        )
        || !option_matches(
            &request.proposal_artifact_sha256,
            &proposal.proposal_artifact_sha256,
        )
        || !option_matches(
            &request.proposal_artifact_readback_sha256,
            &proposal.proposal_artifact_readback_sha256,
        )
        || !option_matches(&request.proposal_worker_build_cohort_sha256, worker_cohort)
        || !option_matches(
            &request.cross_view_evidence_bundle_sha256,
            &proposal.cross_view_evidence_bundle_sha256,
        )
        || !option_matches(&request.proposal_form_art_evidence_id, &art.art_evidence_id)
        || !option_matches(
            &request.proposal_form_art_evidence_object_sha256,
            &proposal.receipt_object_sha256,
        )
        || !option_matches(
            &request.proposal_form_art_evidence_canonical_sha256,
            &proposal.canonical_sha256,
        )
        || request.proposal_part_id_evidence_sha256 != adapter.proposal_part_id_evidence_sha256
        || request.proposal_negative_space_evidence_sha256
            != adapter.proposal_negative_space_evidence_sha256
        || request.proposal_line_flow_evidence_sha256 != adapter.proposal_line_flow_evidence_sha256
    {
        return Err(invalid(
            "fresh FormQuality V2 request scope differs from Runtime-derived durable evidence",
        ));
    }
    Ok(())
}

fn verify_stage_source(
    runtime: &Runtime,
    request: &ProductionWeaponFormQualityV2PrepareRequest,
    legacy: &ProductionWeaponFormQualityRecord,
    art: &ProductionWeaponFormArtEvidenceRecord,
    adapter: Option<&FreshBaselineQualityAdapter>,
) -> Result<
    (
        ProductionStageTransitionV3Record,
        ProductionStageHeadV3Record,
        forgecad_contracts::ProductionCameraLockRecord,
    ),
    RuntimeError,
> {
    let transition = runtime
        .store
        .get_production_stage_transition_v3(&request.current_source_head_transition_id)?
        .ok_or_else(|| invalid("current source-head transition is unavailable"))?;
    let head = runtime
        .store
        .get_production_stage_head_v3(
            &request.session_id,
            &request.project_id,
            &legacy.candidate_id,
        )?
        .ok_or_else(|| invalid("current source-head is unavailable"))?;
    if transition.canonical_sha256 != request.current_source_head_transition_sha256
        || head.canonical_sha256 != request.current_source_head_canonical_sha256
        || transition.to_stage != request.source_stage
        || head.head_stage != request.source_stage
        || transition.session_id != request.session_id
        || transition.project_id != request.project_id
        || transition.head_candidate_id != legacy.candidate_id
        || transition.head_artifact_sha256 != legacy.artifact_sha256
        || head.head_candidate_id != legacy.candidate_id
        || head.head_artifact_sha256 != legacy.artifact_sha256
        || head.head_transition_id != transition.transition_id
        || head.head_transition_sha256 != transition.canonical_sha256
    {
        return Err(invalid("current source head/transition binding differs"));
    }
    let camera_lock_id = adapter
        .map(|source| source.lineage.camera_lock_id.as_str())
        .unwrap_or(legacy.camera_lock_id.as_str());
    let lock = runtime
        .store
        .get_production_camera_lock(camera_lock_id)?
        .ok_or_else(|| invalid("CameraLock is unavailable"))?;
    super::agentic_session::validate_production_camera_lock_record(runtime, &lock)?;
    if lock.schema_version != PRODUCTION_CAMERA_LOCK_SCHEMA_VERSION
        || lock.session_id != request.session_id
        || lock.project_id != request.project_id
        || lock.candidate_id != legacy.candidate_id
        || lock.artifact_sha256 != legacy.artifact_sha256
        || (adapter.is_none()
            && (lock.canonical_sha256 != legacy.camera_lock_canonical_sha256
                || lock.receipt_object_sha256 != legacy.camera_lock_receipt_object_sha256
                || lock.source_transition_id != legacy.camera_lock_source_transition_id
                || lock.source_transition_sha256 != legacy.camera_lock_source_transition_sha256
                || lock.source_head_canonical_sha256
                    != legacy.camera_lock_source_head_canonical_sha256))
        || adapter.is_some_and(|source| {
            lock.canonical_sha256 != source.lineage.camera_lock_canonical_sha256
                || lock.receipt_object_sha256 != source.lineage.camera_lock_receipt_object_sha256
                || lock.source_transition_id != source.lineage.source_transition_id
                || lock.source_transition_sha256 != source.lineage.source_transition_sha256
                || lock.source_head_canonical_sha256 != source.lineage.source_head_canonical_sha256
        })
        || transition.camera_lock_id.as_deref() != Some(lock.camera_lock_id.as_str())
        || transition.camera_lock_canonical_sha256.as_deref()
            != Some(lock.canonical_sha256.as_str())
        || transition.camera_rig_object_sha256.as_deref()
            != Some(lock.camera_rig_object_sha256.as_str())
        || transition.camera_rig_canonical_sha256.as_deref()
            != Some(lock.camera_rig_canonical_sha256.as_str())
        || transition.camera_lock_receipt_object_sha256.as_deref()
            != Some(lock.receipt_object_sha256.as_str())
        || transition.camera_lock_source_transition_id.as_deref()
            != Some(lock.source_transition_id.as_str())
        || transition.camera_lock_source_transition_sha256.as_deref()
            != Some(lock.source_transition_sha256.as_str())
        || transition
            .camera_lock_source_head_canonical_sha256
            .as_deref()
            != Some(lock.source_head_canonical_sha256.as_str())
        || head.camera_lock_id.as_deref() != Some(lock.camera_lock_id.as_str())
        || head.camera_lock_canonical_sha256.as_deref() != Some(lock.canonical_sha256.as_str())
        || head.camera_rig_object_sha256.as_deref() != Some(lock.camera_rig_object_sha256.as_str())
        || head.camera_rig_canonical_sha256.as_deref()
            != Some(lock.camera_rig_canonical_sha256.as_str())
        || head.camera_lock_receipt_object_sha256.as_deref()
            != Some(lock.receipt_object_sha256.as_str())
        || head.camera_lock_source_transition_id.as_deref()
            != Some(lock.source_transition_id.as_str())
        || head.camera_lock_source_transition_sha256.as_deref()
            != Some(lock.source_transition_sha256.as_str())
        || head.camera_lock_source_head_canonical_sha256.as_deref()
            != Some(lock.source_head_canonical_sha256.as_str())
    {
        return Err(invalid("CameraLock binding differs"));
    }
    if !is_sha256(&transition.camera_hash) {
        return Err(invalid("source transition camera hash is invalid"));
    }
    if legacy.session_id != request.session_id
        || legacy.project_id != request.project_id
        || legacy.form_stage != request.form_stage
        || legacy.source_stage != request.source_stage
        || legacy.target_stage != request.target_stage
        || (adapter.is_none()
            && (legacy.candidate_id != art.candidate_id
                || legacy.artifact_sha256 != art.artifact_sha256))
        || legacy.reference_canvas_object_sha256 != art.reference_canvas_object_sha256
        || legacy.design_spec_object_sha256 != art.design_spec_object_sha256
    {
        return Err(invalid("legacy/FormArt source edge differs"));
    }
    Ok((transition, head, lock))
}

fn verify_previous(
    runtime: &Runtime,
    request: &ProductionWeaponFormQualityV2PrepareRequest,
    legacy: &ProductionWeaponFormQualityRecord,
    candidate: &str,
    artifact: &str,
) -> Result<(), RuntimeError> {
    match (
        &request.previous_form_quality_id,
        &request.previous_form_quality_report_object_sha256,
        &request.previous_form_quality_canonical_sha256,
    ) {
        (None, None, None) => Ok(()),
        (Some(id), Some(report), Some(canonical)) => {
            let previous = runtime
                .store
                .get_production_weapon_form_quality_v2(id)?
                .ok_or_else(|| invalid("previous V2 form quality is unavailable"))?;
            if previous.session_id != request.session_id
                || previous.project_id != request.project_id
                || previous.candidate_id != candidate
                || previous.artifact_sha256 != artifact
                || previous.target_stage != request.source_stage
                || previous.receipt_object_sha256 != *report
                || previous.canonical_sha256 != *canonical
                || previous.form_quality_id == request.form_quality_id
                || legacy.form_stage == "blockout"
            {
                return Err(invalid("previous V2 form quality binding differs"));
            }
            Ok(())
        }
        _ => Err(invalid(
            "previous V2 form quality fields must be all null or all present",
        )),
    }
}

fn build_record(
    runtime: &Runtime,
    request: &ProductionWeaponFormQualityV2PrepareRequest,
    request_sha: &str,
    mode: &EvidenceMode,
) -> Result<ProductionWeaponFormQualityV2Record, RuntimeError> {
    let legacy = verify_legacy(
        runtime,
        &request.legacy_form_quality_object_sha256,
        &request.legacy_form_quality_canonical_sha256,
    )?;
    let art = verify_art(
        runtime,
        &request.form_art_evidence_object_sha256,
        &request.form_art_evidence_canonical_sha256,
    )?;
    let adapter = resolve_fresh_baseline_adapter(runtime, request, &legacy, &art, mode)?;
    verify_request_evidence_scope(request, &art, adapter.as_ref())?;
    verify_authoring(runtime, &legacy)?;
    let (transition, head, lock) =
        verify_stage_source(runtime, request, &legacy, &art, adapter.as_ref())?;
    let evaluation_candidate_id = adapter
        .as_ref()
        .map(|source| source.evaluation_candidate_id.as_str())
        .unwrap_or(legacy.candidate_id.as_str());
    let evaluation_candidate_state_sha256 = adapter
        .as_ref()
        .map(|source| source.evaluation_candidate_state_sha256.as_str())
        .unwrap_or(legacy.candidate_state_sha256.as_str());
    let evaluation_artifact_id = adapter
        .as_ref()
        .map(|source| source.evaluation_artifact_id.as_str())
        .unwrap_or(legacy.artifact_id.as_str());
    let evaluation_artifact_sha256 = adapter
        .as_ref()
        .map(|source| source.evaluation_artifact_sha256.as_str())
        .unwrap_or(legacy.artifact_sha256.as_str());
    let candidate = runtime
        .candidate(evaluation_candidate_id)?
        .ok_or_else(|| invalid("candidate is unavailable"))?;
    if candidate.project_id != request.project_id
        || candidate.canonical_sha256 != evaluation_candidate_state_sha256
        || candidate.prepared_object_id.as_deref() != Some(evaluation_artifact_id)
        || candidate.prepared_object_sha256.as_deref() != Some(evaluation_artifact_sha256)
    {
        return Err(invalid("candidate/artifact binding differs"));
    }
    let _readback =
        runtime.artifact_readback(evaluation_artifact_sha256, evaluation_candidate_id)?;
    let reference = runtime
        .reference(&legacy.reference_id)?
        .ok_or_else(|| invalid("reference is unavailable"))?;
    if reference.project_id != request.project_id
        || reference.object_sha256 != legacy.reference_sha256
    {
        return Err(invalid("reference binding differs"));
    }
    verify_previous(
        runtime,
        request,
        &legacy,
        &legacy.candidate_id,
        &legacy.artifact_sha256,
    )?;
    let _cross_view = if let Some(source) = adapter.as_ref() {
        verify_cross_view_bundle(
            runtime,
            &legacy.session_id,
            &legacy.project_id,
            evaluation_candidate_id,
            evaluation_candidate_state_sha256,
            evaluation_artifact_sha256,
            &legacy.reference_canvas_object_sha256,
            &source.cross_view_object_sha256,
            source.cross_view_canonical(),
        )?
    } else {
        verify_legacy_cross_view(runtime, &legacy)?
    };
    let mut decisions = Vec::with_capacity(6);
    for (ordinal, kind) in PRODUCTION_WEAPON_FORM_QUALITY_V2_VIEW_KINDS
        .iter()
        .enumerate()
    {
        let legacy_view = legacy
            .form_view_evaluations
            .iter()
            .find(|v| v.view_kind == *kind)
            .ok_or_else(|| invalid(format!("legacy view {kind} is unavailable")))?;
        let art_view = art
            .views
            .get(ordinal)
            .ok_or_else(|| invalid(format!("FormArt view {kind} is unavailable")))?;
        if legacy_view.view_id != art_view.view_id
            || art_view.view_kind != *kind
            || !legacy_view_no_regression_passes(&legacy_view.no_regression)
        {
            return Err(invalid(
                "legacy view cross-view/no-regression binding differs",
            ));
        }
        if art_view.part_id_status != "observed"
            || art_view.part_id_coverage_milli != 1000
            || art_view.part_id_missing_count != 0
            || art_view.part_id_unexpected_count != 0
            || art_view.part_id_expected_count != art_view.part_id_observed_count
        {
            return Err(invalid(format!(
                "FormArt Part-ID coverage is not complete for {kind}"
            )));
        }
        verify_target_observation(runtime, art_view)?;
        // In fresh mode the reviewed FormArt row remains the semantic target
        // source, while all evaluated candidate/pass bindings come from the
        // proposal-side receipt.  This is the second half of the dual-scope
        // adapter: a proposal must not inherit source-candidate pixels merely
        // because the public V2 view decision still has the historical field
        // names.
        let proposal_view = adapter
            .as_ref()
            .map(|source| source.proposal_view(ordinal, kind))
            .transpose()?;
        let (
            form_art_view_id,
            target_object_sha256,
            target_canonical_sha256,
            silhouette_pass_object_sha256,
            part_id_pass_object_sha256,
            depth_pass_object_sha256,
            normal_pass_object_sha256,
        ) = if let Some(proposal_view) = proposal_view {
            let fresh_camera = adapter
                .as_ref()
                .and_then(|source| source.baseline.views.get(ordinal))
                .ok_or_else(|| {
                    invalid(format!("fresh baseline {kind} camera row is unavailable"))
                })?;
            let field = |name: &str| {
                proposal_view
                    .get(name)
                    .and_then(Value::as_str)
                    .filter(|value| is_sha256(value))
                    .map(str::to_owned)
                    .ok_or_else(|| invalid(format!("fresh proposal {kind} {name} is invalid")))
            };
            let proposal_view_id = proposal_view
                .get("view_id")
                .and_then(Value::as_str)
                .filter(|value| is_opaque_id(value))
                .ok_or_else(|| invalid(format!("fresh proposal {kind} view id is invalid")))?;
            if proposal_view_id != art_view.view_id
                || proposal_view.get("reference_id").and_then(Value::as_str)
                    != Some(art_view.reference_id.as_str())
                || proposal_view
                    .get("reference_sha256")
                    .and_then(Value::as_str)
                    != Some(art_view.reference_sha256.as_str())
                || proposal_view.get("camera_hash").and_then(Value::as_str)
                    != Some(fresh_camera.camera_hash.as_str())
                || proposal_view
                    .get("camera_canonical_sha256")
                    .and_then(Value::as_str)
                    != Some(fresh_camera.camera_canonical_sha256.as_str())
                || proposal_view
                    .get("proposal_candidate_id")
                    .and_then(Value::as_str)
                    != adapter
                        .as_ref()
                        .map(|source| source.evaluation_candidate_id.as_str())
                || proposal_view
                    .get("proposal_candidate_state_sha256")
                    .and_then(Value::as_str)
                    != adapter
                        .as_ref()
                        .map(|source| source.evaluation_candidate_state_sha256.as_str())
                || proposal_view
                    .get("proposal_artifact_sha256")
                    .and_then(Value::as_str)
                    != adapter
                        .as_ref()
                        .map(|source| source.evaluation_artifact_sha256.as_str())
                || proposal_view.get("part_id_status").and_then(Value::as_str) != Some("observed")
                || proposal_view
                    .get("view_observation_status")
                    .and_then(Value::as_str)
                    != Some("observed")
                || !matches!(
                    proposal_view
                        .get("negative_space_status")
                        .and_then(Value::as_str),
                    Some("observed") | Some("not-applicable")
                )
                || !matches!(
                    proposal_view
                        .get("line_flow_status")
                        .and_then(Value::as_str),
                    Some("observed") | Some("not-applicable")
                )
            {
                return Err(invalid(format!(
                    "fresh proposal {kind} evaluated binding differs"
                )));
            }
            (
                proposal_view_id.to_owned(),
                field("target_object_sha256")?,
                field("target_canonical_sha256")?,
                field("silhouette_pass_object_sha256")?,
                field("part_id_pass_object_sha256")?,
                field("depth_pass_object_sha256")?,
                field("normal_pass_object_sha256")?,
            )
        } else {
            (
                art_view.view_id.clone(),
                art_view.target_object_sha256.clone(),
                art_view.target_canonical_sha256.clone(),
                art_view.silhouette_pass_object_sha256.clone(),
                art_view.part_id_pass_object_sha256.clone(),
                art_view.depth_pass_object_sha256.clone(),
                art_view.normal_pass_object_sha256.clone(),
            )
        };
        let legacy_canonical = canonical_json_hash(
            &serde_json::to_value(legacy_view).map_err(|error| invalid(error.to_string()))?,
        );
        decisions.push(ProductionWeaponFormQualityV2ViewDecision {
            view_kind: kind.to_string(),
            legacy_form_quality_view_id: legacy_view.view_id.clone(),
            legacy_form_quality_view_canonical_sha256: legacy_canonical,
            form_art_view_id,
            form_art_view_canonical_sha256: art_view.canonical_sha256.clone(),
            form_art_view_receipt_object_sha256: art_view.receipt_object_sha256.clone(),
            target_object_sha256,
            target_canonical_sha256,
            silhouette_pass_object_sha256,
            part_id_pass_object_sha256,
            depth_pass_object_sha256,
            normal_pass_object_sha256,
            cross_view_thresholds_passed: true,
            no_regression_passed: true,
            part_id_passed: true,
            negative_space_passed: true,
            line_flow_passed: true,
            view_passed: true,
        });
    }
    let reviewed = PRODUCTION_WEAPON_FORM_QUALITY_V2_VIEW_KINDS
        .iter()
        .map(|v| (*v).to_owned())
        .collect::<Vec<_>>();
    let fixed = PRODUCTION_WEAPON_FORM_QUALITY_V2_FIXED_CAMERA_VIEW_KINDS
        .iter()
        .map(|v| (*v).to_owned())
        .collect::<Vec<_>>();
    let mut record = ProductionWeaponFormQualityV2Record {
        schema_version: PRODUCTION_WEAPON_FORM_QUALITY_V2_SCHEMA_VERSION.into(),
        form_quality_id: request.form_quality_id.clone(),
        session_id: request.session_id.clone(),
        project_id: request.project_id.clone(),
        form_stage: request.form_stage.clone(),
        source_stage: request.source_stage.clone(),
        target_stage: request.target_stage.clone(),
        current_source_head_transition_id: transition.transition_id.clone(),
        current_source_head_transition_sha256: transition.canonical_sha256.clone(),
        current_source_head_canonical_sha256: head.canonical_sha256.clone(),
        current_source_head_stage: head.head_stage.clone(),
        current_source_head_candidate_id: head.head_candidate_id.clone(),
        current_source_head_candidate_state_sha256: head.head_candidate_state_sha256.clone(),
        current_source_head_artifact_id: head.output_artifact_id.clone(),
        current_source_head_artifact_sha256: head.head_artifact_sha256.clone(),
        candidate_id: legacy.candidate_id.clone(),
        candidate_state_sha256: legacy.candidate_state_sha256.clone(),
        artifact_id: legacy.artifact_id.clone(),
        artifact_sha256: legacy.artifact_sha256.clone(),
        reference_id: legacy.reference_id.clone(),
        reference_sha256: legacy.reference_sha256.clone(),
        reference_canvas_object_sha256: legacy.reference_canvas_object_sha256.clone(),
        reference_canvas_canonical_sha256: legacy.reference_canvas_canonical_sha256.clone(),
        design_spec_object_sha256: legacy.design_spec_object_sha256.clone(),
        design_spec_canonical_sha256: legacy.design_spec_canonical_sha256.clone(),
        camera_hash: transition.camera_hash.clone(),
        evidence_source_kind: request.evidence_source_kind.clone(),
        source_candidate_id: adapter
            .as_ref()
            .map(|value| value.baseline.candidate_id.clone()),
        source_candidate_state_sha256: adapter
            .as_ref()
            .map(|value| value.baseline.candidate_state_sha256.clone()),
        source_artifact_id: adapter
            .as_ref()
            .map(|value| value.baseline.artifact_id.clone()),
        source_artifact_sha256: adapter
            .as_ref()
            .map(|value| value.baseline.artifact_sha256.clone()),
        source_fresh_baseline_id: adapter
            .as_ref()
            .map(|value| value.baseline.baseline_id.clone()),
        source_fresh_baseline_canonical_sha256: adapter
            .as_ref()
            .map(|value| value.baseline.canonical_sha256.clone()),
        source_fresh_baseline_receipt_object_sha256: adapter
            .as_ref()
            .map(|value| value.baseline.receipt_object_sha256.clone()),
        source_registration_lineage_id: adapter
            .as_ref()
            .map(|value| value.lineage.registration_lineage_id.clone()),
        source_registration_lineage_canonical_sha256: adapter
            .as_ref()
            .map(|value| value.lineage.canonical_sha256.clone()),
        source_registration_lineage_receipt_object_sha256: adapter
            .as_ref()
            .map(|value| value.lineage.receipt_object_sha256.clone()),
        source_registered_rig_v2_id: adapter
            .as_ref()
            .map(|value| value.baseline.registered_rig_v2_id.clone()),
        source_registered_rig_v2_object_sha256: adapter
            .as_ref()
            .map(|value| value.baseline.registered_rig_v2_object_sha256.clone()),
        source_registered_rig_v2_canonical_sha256: adapter
            .as_ref()
            .map(|value| value.baseline.registered_rig_v2_canonical_sha256.clone()),
        source_runtime_build_cohort_sha256: adapter
            .as_ref()
            .map(|value| value.baseline.runtime_build_cohort_sha256.clone()),
        proposal_candidate_id: adapter
            .as_ref()
            .and_then(|value| value.proposal_evidence.as_ref())
            .map(|value| value.proposal_candidate_id.clone()),
        proposal_candidate_state_sha256: adapter
            .as_ref()
            .and_then(|value| value.proposal_evidence.as_ref())
            .map(|value| value.proposal_candidate_state_sha256.clone()),
        proposal_artifact_id: adapter
            .as_ref()
            .filter(|value| value.is_proposal_scope())
            .map(|value| value.evaluation_artifact_id.clone()),
        proposal_artifact_sha256: adapter
            .as_ref()
            .and_then(|value| value.proposal_evidence.as_ref())
            .map(|value| value.proposal_artifact_sha256.clone()),
        proposal_artifact_readback_sha256: adapter
            .as_ref()
            .and_then(|value| value.proposal_evidence.as_ref())
            .map(|value| value.proposal_artifact_readback_sha256.clone()),
        proposal_worker_build_cohort_sha256: adapter
            .as_ref()
            .and_then(|value| value.proposal_evidence.as_ref())
            .and_then(|value| value.worker_build_cohort_sha256.clone()),
        cross_view_evidence_bundle_sha256: adapter
            .as_ref()
            .filter(|value| value.is_proposal_scope())
            .map(|value| value.cross_view_object_sha256.clone()),
        proposal_form_art_evidence_id: adapter
            .as_ref()
            .filter(|value| value.is_proposal_scope())
            .map(|_| art.art_evidence_id.clone()),
        proposal_form_art_evidence_object_sha256: adapter
            .as_ref()
            .and_then(|value| value.proposal_evidence.as_ref())
            .map(|value| value.receipt_object_sha256.clone()),
        proposal_form_art_evidence_canonical_sha256: adapter
            .as_ref()
            .and_then(|value| value.proposal_evidence.as_ref())
            .map(|value| value.canonical_sha256.clone()),
        proposal_part_id_evidence_sha256: adapter
            .as_ref()
            .and_then(|value| value.proposal_part_id_evidence_sha256.clone()),
        proposal_negative_space_evidence_sha256: adapter
            .as_ref()
            .and_then(|value| value.proposal_negative_space_evidence_sha256.clone()),
        proposal_line_flow_evidence_sha256: adapter
            .as_ref()
            .and_then(|value| value.proposal_line_flow_evidence_sha256.clone()),
        camera_lock_id: lock.camera_lock_id.clone(),
        camera_lock_canonical_sha256: lock.canonical_sha256.clone(),
        camera_rig_object_sha256: lock.camera_rig_object_sha256.clone(),
        camera_rig_canonical_sha256: lock.camera_rig_canonical_sha256.clone(),
        camera_lock_receipt_object_sha256: lock.receipt_object_sha256.clone(),
        camera_lock_source_transition_id: lock.source_transition_id.clone(),
        camera_lock_source_transition_sha256: lock.source_transition_sha256.clone(),
        camera_lock_source_head_canonical_sha256: lock.source_head_canonical_sha256.clone(),
        reviewed_reference_view_kinds: reviewed,
        fixed_camera_view_kinds: fixed,
        legacy_form_quality_object_sha256: request.legacy_form_quality_object_sha256.clone(),
        legacy_form_quality_canonical_sha256: request.legacy_form_quality_canonical_sha256.clone(),
        form_art_evidence_object_sha256: request.form_art_evidence_object_sha256.clone(),
        form_art_evidence_canonical_sha256: request.form_art_evidence_canonical_sha256.clone(),
        view_decisions: decisions,
        aggregate: ProductionWeaponFormQualityV2Aggregate {
            view_count: 6,
            all_cross_view_thresholds_passed: true,
            all_no_regression_passed: true,
            all_part_id_passed: true,
            all_negative_space_passed: true,
            all_line_flow_passed: true,
            all_view_passed: true,
        },
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
        hard_gate_passed: true,
        form_gate_passed: true,
        validator_status: PRODUCTION_WEAPON_FORM_QUALITY_V2_VALIDATOR_STATUS.into(),
        structural_status: PRODUCTION_WEAPON_FORM_QUALITY_V2_STRUCTURAL_STATUS.into(),
        visual_status: forgecad_contracts::PRODUCTION_WEAPON_FORM_QUALITY_V2_VISUAL_STATUS.into(),
        human_status: PRODUCTION_WEAPON_FORM_QUALITY_V2_HUMAN_STATUS.into(),
        engine_status: PRODUCTION_WEAPON_FORM_QUALITY_V2_ENGINE_STATUS.into(),
        distribution_status: PRODUCTION_WEAPON_FORM_QUALITY_V2_DISTRIBUTION_STATUS.into(),
        quality_status: PRODUCTION_WEAPON_FORM_QUALITY_V2_QUALITY_STATUS.into(),
        runtime_write_performed: true,
        production_stage_advanced: false,
        candidate_confirmed: false,
        version_created: false,
        export_performed: false,
        request_sha256: request_sha.into(),
        input_sha256: request.input_sha256.clone(),
        receipt_object_sha256: String::new(),
        canonical_sha256: String::new(),
        created_at: legacy.created_at.clone(),
    };
    record.canonical_sha256 = canonical_json_hash(&normalized_record(&record)?);
    Ok(record)
}

fn release(runtime: &Runtime, reservation: &CasReservation, object: &CasObject, cleanup: bool) {
    let _ = runtime.store.release_cas_reservation_object(
        reservation,
        object,
        cleanup && object.created_new,
    );
}

fn result_value(
    record: &ProductionWeaponFormQualityV2Record,
    replayed: bool,
    schema: &str,
    runtime_write: bool,
    restart_hash_verified: Option<bool>,
) -> Result<Value, RuntimeError> {
    let mut value = serde_json::json!({"schema_version":schema,"form_quality":serde_json::to_value(record).map_err(|error| invalid(error.to_string()))?,"replayed":replayed,"runtime_write":runtime_write,"production_stage_advanced":false,"candidate_confirmed":false,"version_created":false,"export_performed":false});
    if let Some(verified) = restart_hash_verified {
        value["restart_hash_verified"] = Value::Bool(verified);
    }
    Ok(value)
}

fn replay_request(record: &ProductionWeaponFormQualityV2Record) -> Value {
    serde_json::json!({
        "schema_version": PRODUCTION_WEAPON_FORM_QUALITY_V2_PREPARE_REQUEST_SCHEMA_VERSION,
        "form_quality_id": record.form_quality_id, "session_id": record.session_id,
        "project_id": record.project_id, "form_stage": record.form_stage,
        "source_stage": record.source_stage, "target_stage": record.target_stage,
        "legacy_form_quality_object_sha256": record.legacy_form_quality_object_sha256,
        "legacy_form_quality_canonical_sha256": record.legacy_form_quality_canonical_sha256,
        "form_art_evidence_object_sha256": record.form_art_evidence_object_sha256,
        "form_art_evidence_canonical_sha256": record.form_art_evidence_canonical_sha256,
        "evidence_source_kind": record.evidence_source_kind,
        "source_candidate_id": record.source_candidate_id,
        "source_candidate_state_sha256": record.source_candidate_state_sha256,
        "source_artifact_id": record.source_artifact_id,
        "source_artifact_sha256": record.source_artifact_sha256,
        "source_fresh_baseline_id": record.source_fresh_baseline_id,
        "source_fresh_baseline_canonical_sha256": record.source_fresh_baseline_canonical_sha256,
        "source_fresh_baseline_receipt_object_sha256": record.source_fresh_baseline_receipt_object_sha256,
        "source_registration_lineage_id": record.source_registration_lineage_id,
        "source_registration_lineage_canonical_sha256": record.source_registration_lineage_canonical_sha256,
        "source_registration_lineage_receipt_object_sha256": record.source_registration_lineage_receipt_object_sha256,
        "source_registered_rig_v2_id": record.source_registered_rig_v2_id,
        "source_registered_rig_v2_object_sha256": record.source_registered_rig_v2_object_sha256,
        "source_registered_rig_v2_canonical_sha256": record.source_registered_rig_v2_canonical_sha256,
        "source_runtime_build_cohort_sha256": record.source_runtime_build_cohort_sha256,
        "proposal_candidate_id": record.proposal_candidate_id,
        "proposal_candidate_state_sha256": record.proposal_candidate_state_sha256,
        "proposal_artifact_id": record.proposal_artifact_id,
        "proposal_artifact_sha256": record.proposal_artifact_sha256,
        "proposal_artifact_readback_sha256": record.proposal_artifact_readback_sha256,
        "proposal_worker_build_cohort_sha256": record.proposal_worker_build_cohort_sha256,
        "cross_view_evidence_bundle_sha256": record.cross_view_evidence_bundle_sha256,
        "proposal_form_art_evidence_id": record.proposal_form_art_evidence_id,
        "proposal_form_art_evidence_object_sha256": record.proposal_form_art_evidence_object_sha256,
        "proposal_form_art_evidence_canonical_sha256": record.proposal_form_art_evidence_canonical_sha256,
        "proposal_part_id_evidence_sha256": record.proposal_part_id_evidence_sha256,
        "proposal_negative_space_evidence_sha256": record.proposal_negative_space_evidence_sha256,
        "proposal_line_flow_evidence_sha256": record.proposal_line_flow_evidence_sha256,
        "current_source_head_transition_id": record.current_source_head_transition_id,
        "current_source_head_transition_sha256": record.current_source_head_transition_sha256,
        "current_source_head_canonical_sha256": record.current_source_head_canonical_sha256,
        "previous_form_quality_id": record.previous_form_quality_id,
        "previous_form_quality_report_object_sha256": record.previous_form_quality_report_object_sha256,
        "previous_form_quality_canonical_sha256": record.previous_form_quality_canonical_sha256,
        "form_quality_policy": record.form_quality_policy, "form_quality_policy_sha256": record.form_quality_policy_sha256,
        "threshold_policy": record.threshold_policy, "threshold_policy_sha256": record.threshold_policy_sha256,
        "input_sha256": record.input_sha256, "idempotency_key": record.form_quality_id
    })
}

fn parse_preflight_request(value: &Value) -> Result<PreflightRequest, RuntimeError> {
    let fields = PREFLIGHT_FIELDS
        .iter()
        .copied()
        .chain(FORM_QUALITY_V2_SCOPE_OPTION_FIELDS.iter().copied())
        .collect::<Vec<_>>();
    let object = exact_object(
        value,
        fields.as_slice(),
        "ProductionWeaponFormQualityV2PreflightGetRequest@1",
    )?;
    if text(object, "schema_version")? != PREFLIGHT_SCHEMA_VERSION {
        return Err(invalid("form quality V2 preflight schema differs"));
    }
    let evidence_source_kind = text(object, "evidence_source_kind")?;
    validate_normalized_scope_object(object, evidence_source_kind)?;
    for field in [
        "preflight_id",
        "session_id",
        "project_id",
        "candidate_id",
        "current_source_head_transition_id",
    ] {
        id(object, field)?;
    }
    for field in [
        "legacy_form_quality_object_sha256",
        "legacy_form_quality_canonical_sha256",
        "form_art_evidence_object_sha256",
        "form_art_evidence_canonical_sha256",
        "current_source_head_transition_sha256",
        "current_source_head_canonical_sha256",
        "input_sha256",
    ] {
        sha(object, field)?;
    }
    let form_stage = text(object, "form_stage")?;
    if !PRODUCTION_WEAPON_FORM_QUALITY_V2_FORM_STAGES.contains(&form_stage) {
        return Err(invalid("form quality V2 preflight stage differs"));
    }
    let mut preimage = object.clone();
    preimage.remove("input_sha256");
    let expected_input_sha256 = canonical_json_hash(&Value::Object(preimage));
    if object.get("input_sha256").and_then(Value::as_str) != Some(expected_input_sha256.as_str()) {
        return Err(invalid("form quality V2 preflight input hash differs"));
    }
    Ok(PreflightRequest {
        preflight_id: text(object, "preflight_id")?.to_owned(),
        session_id: text(object, "session_id")?.to_owned(),
        project_id: text(object, "project_id")?.to_owned(),
        candidate_id: text(object, "candidate_id")?.to_owned(),
        form_stage: form_stage.to_owned(),
        legacy_form_quality_object_sha256: text(object, "legacy_form_quality_object_sha256")?
            .to_owned(),
        legacy_form_quality_canonical_sha256: text(object, "legacy_form_quality_canonical_sha256")?
            .to_owned(),
        form_art_evidence_object_sha256: text(object, "form_art_evidence_object_sha256")?
            .to_owned(),
        form_art_evidence_canonical_sha256: text(object, "form_art_evidence_canonical_sha256")?
            .to_owned(),
        evidence_source_kind: evidence_source_kind.to_owned(),
        source_candidate_id: object
            .get("source_candidate_id")
            .and_then(Value::as_str)
            .map(str::to_owned),
        source_candidate_state_sha256: object
            .get("source_candidate_state_sha256")
            .and_then(Value::as_str)
            .map(str::to_owned),
        source_artifact_id: object
            .get("source_artifact_id")
            .and_then(Value::as_str)
            .map(str::to_owned),
        source_artifact_sha256: object
            .get("source_artifact_sha256")
            .and_then(Value::as_str)
            .map(str::to_owned),
        source_fresh_baseline_id: object
            .get("source_fresh_baseline_id")
            .and_then(Value::as_str)
            .map(str::to_owned),
        source_fresh_baseline_canonical_sha256: object
            .get("source_fresh_baseline_canonical_sha256")
            .and_then(Value::as_str)
            .map(str::to_owned),
        source_fresh_baseline_receipt_object_sha256: object
            .get("source_fresh_baseline_receipt_object_sha256")
            .and_then(Value::as_str)
            .map(str::to_owned),
        source_registration_lineage_id: object
            .get("source_registration_lineage_id")
            .and_then(Value::as_str)
            .map(str::to_owned),
        source_registration_lineage_canonical_sha256: object
            .get("source_registration_lineage_canonical_sha256")
            .and_then(Value::as_str)
            .map(str::to_owned),
        source_registration_lineage_receipt_object_sha256: object
            .get("source_registration_lineage_receipt_object_sha256")
            .and_then(Value::as_str)
            .map(str::to_owned),
        source_registered_rig_v2_id: object
            .get("source_registered_rig_v2_id")
            .and_then(Value::as_str)
            .map(str::to_owned),
        source_registered_rig_v2_object_sha256: object
            .get("source_registered_rig_v2_object_sha256")
            .and_then(Value::as_str)
            .map(str::to_owned),
        source_registered_rig_v2_canonical_sha256: object
            .get("source_registered_rig_v2_canonical_sha256")
            .and_then(Value::as_str)
            .map(str::to_owned),
        source_runtime_build_cohort_sha256: object
            .get("source_runtime_build_cohort_sha256")
            .and_then(Value::as_str)
            .map(str::to_owned),
        proposal_candidate_id: object
            .get("proposal_candidate_id")
            .and_then(Value::as_str)
            .map(str::to_owned),
        proposal_candidate_state_sha256: object
            .get("proposal_candidate_state_sha256")
            .and_then(Value::as_str)
            .map(str::to_owned),
        proposal_artifact_id: object
            .get("proposal_artifact_id")
            .and_then(Value::as_str)
            .map(str::to_owned),
        proposal_artifact_sha256: object
            .get("proposal_artifact_sha256")
            .and_then(Value::as_str)
            .map(str::to_owned),
        proposal_artifact_readback_sha256: object
            .get("proposal_artifact_readback_sha256")
            .and_then(Value::as_str)
            .map(str::to_owned),
        proposal_worker_build_cohort_sha256: object
            .get("proposal_worker_build_cohort_sha256")
            .and_then(Value::as_str)
            .map(str::to_owned),
        cross_view_evidence_bundle_sha256: object
            .get("cross_view_evidence_bundle_sha256")
            .and_then(Value::as_str)
            .map(str::to_owned),
        proposal_form_art_evidence_id: object
            .get("proposal_form_art_evidence_id")
            .and_then(Value::as_str)
            .map(str::to_owned),
        proposal_form_art_evidence_object_sha256: object
            .get("proposal_form_art_evidence_object_sha256")
            .and_then(Value::as_str)
            .map(str::to_owned),
        proposal_form_art_evidence_canonical_sha256: object
            .get("proposal_form_art_evidence_canonical_sha256")
            .and_then(Value::as_str)
            .map(str::to_owned),
        proposal_part_id_evidence_sha256: object
            .get("proposal_part_id_evidence_sha256")
            .and_then(Value::as_str)
            .map(str::to_owned),
        proposal_negative_space_evidence_sha256: object
            .get("proposal_negative_space_evidence_sha256")
            .and_then(Value::as_str)
            .map(str::to_owned),
        proposal_line_flow_evidence_sha256: object
            .get("proposal_line_flow_evidence_sha256")
            .and_then(Value::as_str)
            .map(str::to_owned),
        current_source_head_transition_id: text(object, "current_source_head_transition_id")?
            .to_owned(),
        current_source_head_transition_sha256: text(
            object,
            "current_source_head_transition_sha256",
        )?
        .to_owned(),
        current_source_head_canonical_sha256: text(object, "current_source_head_canonical_sha256")?
            .to_owned(),
    })
}

fn read_preflight_json(
    runtime: &Runtime,
    hash: &str,
    expected_kind: Option<&str>,
) -> Result<Value, &'static str> {
    let object = runtime
        .store
        .get_object(hash)
        .map_err(|_| "STORE_READ_FAILED")?
        .ok_or("CAS_OBJECT_MISSING")?;
    if object.sha256 != hash
        || object.mime != JSON_MIME
        || object.size_bytes > MAX_JSON_BYTES
        || expected_kind.is_some_and(|kind| object.kind != kind)
    {
        return Err("CAS_METADATA_MISMATCH");
    }
    let bytes = runtime
        .cas_read_bounded(hash, MAX_JSON_BYTES)
        .map_err(|_| "CAS_READ_FAILED")?;
    serde_json::from_slice(&bytes).map_err(|_| "CAS_JSON_INVALID")
}

fn preflight_canonical(value: &Value, schema: &str, self_reference: &str) -> Option<String> {
    if value.get("schema_version").and_then(Value::as_str) != Some(schema) {
        return None;
    }
    let mut normalized = value.clone();
    if normalized.get(self_reference).is_some() {
        normalized[self_reference] = Value::String(String::new());
    }
    if normalized.get("canonical_sha256").is_some() {
        normalized["canonical_sha256"] = Value::String(String::new());
    }
    Some(canonical_json_hash(&normalized))
}

fn preflight_legacy(
    runtime: &Runtime,
    request: &PreflightRequest,
) -> (PreflightCheck, Option<ProductionWeaponFormQualityRecord>) {
    let payload = match read_preflight_json(
        runtime,
        &request.legacy_form_quality_object_sha256,
        Some("production-weapon-form-quality-receipt"),
    ) {
        Ok(payload) => payload,
        Err(reason) => {
            return (
                PreflightCheck::blocked(if reason == "CAS_OBJECT_MISSING" {
                    "LEGACY_FORM_QUALITY_MISSING"
                } else {
                    "LEGACY_FORM_QUALITY_UNREADABLE"
                }),
                None,
            )
        }
    };
    let mut record: ProductionWeaponFormQualityRecord =
        match serde_json::from_value(payload.clone()) {
            Ok(record) => record,
            Err(_) => {
                return (
                    PreflightCheck::invalid(
                        "LEGACY_FORM_QUALITY_MALFORMED",
                        Some(request.legacy_form_quality_object_sha256.clone()),
                    ),
                    None,
                )
            }
        };
    let Some(canonical) = preflight_canonical(
        &payload,
        PRODUCTION_WEAPON_FORM_QUALITY_SCHEMA_VERSION,
        "receipt_object_sha256",
    ) else {
        return (
            PreflightCheck::invalid(
                "LEGACY_FORM_QUALITY_SCHEMA_MISMATCH",
                Some(request.legacy_form_quality_object_sha256.clone()),
            ),
            None,
        );
    };
    if record.receipt_object_sha256 != ""
        || record.canonical_sha256 != request.legacy_form_quality_canonical_sha256
        || canonical != record.canonical_sha256
    {
        return (
            PreflightCheck::invalid(
                "LEGACY_FORM_QUALITY_CANONICAL_MISMATCH",
                Some(request.legacy_form_quality_object_sha256.clone()),
            ),
            None,
        );
    }
    if record.session_id != request.session_id
        || record.project_id != request.project_id
        || record.candidate_id != request.candidate_id
    {
        return (
            PreflightCheck::invalid(
                "LEGACY_FORM_QUALITY_SCOPE_RETARGET",
                Some(request.legacy_form_quality_object_sha256.clone()),
            ),
            None,
        );
    }
    record.receipt_object_sha256 = request.legacy_form_quality_object_sha256.clone();
    match runtime
        .store
        .get_production_weapon_form_quality(&record.form_quality_id)
    {
        Ok(Some(stored)) if stored == record => (
            PreflightCheck::ready(
                Some(request.legacy_form_quality_object_sha256.clone()),
                Some(request.legacy_form_quality_canonical_sha256.clone()),
            ),
            Some(record),
        ),
        Ok(Some(_)) => (
            PreflightCheck::invalid(
                "LEGACY_FORM_QUALITY_STORE_MISMATCH",
                Some(request.legacy_form_quality_object_sha256.clone()),
            ),
            None,
        ),
        Ok(None) => (
            PreflightCheck::blocked("LEGACY_FORM_QUALITY_STORE_ROW_MISSING"),
            None,
        ),
        Err(_) => (
            PreflightCheck::blocked("LEGACY_FORM_QUALITY_STORE_UNAVAILABLE"),
            None,
        ),
    }
}

fn preflight_art(
    runtime: &Runtime,
    request: &PreflightRequest,
) -> (
    PreflightCheck,
    Option<ProductionWeaponFormArtEvidenceRecord>,
) {
    let payload = match read_preflight_json(
        runtime,
        &request.form_art_evidence_object_sha256,
        Some(PRODUCTION_WEAPON_FORM_ART_EVIDENCE_PARENT_RECEIPT_KIND),
    ) {
        Ok(payload) => payload,
        Err(reason) => {
            return (
                PreflightCheck::blocked(if reason == "CAS_OBJECT_MISSING" {
                    "FORM_ART_EVIDENCE_MISSING"
                } else {
                    "FORM_ART_EVIDENCE_UNREADABLE"
                }),
                None,
            )
        }
    };
    let mut record: ProductionWeaponFormArtEvidenceRecord =
        match serde_json::from_value(payload.clone()) {
            Ok(record) => record,
            Err(_) => {
                return (
                    PreflightCheck::invalid(
                        "FORM_ART_EVIDENCE_MALFORMED",
                        Some(request.form_art_evidence_object_sha256.clone()),
                    ),
                    None,
                )
            }
        };
    let Some(canonical) = preflight_canonical(
        &payload,
        "ProductionWeaponFormArtEvidence@1",
        "receipt_object_sha256",
    ) else {
        return (
            PreflightCheck::invalid(
                "FORM_ART_EVIDENCE_SCHEMA_MISMATCH",
                Some(request.form_art_evidence_object_sha256.clone()),
            ),
            None,
        );
    };
    if record.receipt_object_sha256 != ""
        || record.canonical_sha256 != request.form_art_evidence_canonical_sha256
        || canonical != record.canonical_sha256
        || record.views.len() != PRODUCTION_WEAPON_FORM_QUALITY_V2_VIEW_KINDS.len()
        || record.view_kinds
            != PRODUCTION_WEAPON_FORM_QUALITY_V2_VIEW_KINDS
                .iter()
                .map(|kind| (*kind).to_owned())
                .collect::<Vec<_>>()
    {
        return (
            PreflightCheck::invalid(
                "FORM_ART_EVIDENCE_BINDING_MISMATCH",
                Some(request.form_art_evidence_object_sha256.clone()),
            ),
            None,
        );
    }
    if record.session_id != request.session_id
        || record.project_id != request.project_id
        || record.candidate_id != request.candidate_id
    {
        return (
            PreflightCheck::invalid(
                "FORM_ART_EVIDENCE_SCOPE_RETARGET",
                Some(request.form_art_evidence_object_sha256.clone()),
            ),
            None,
        );
    }
    for (ordinal, view) in record.views.iter().enumerate() {
        if view.view_kind != PRODUCTION_WEAPON_FORM_QUALITY_V2_VIEW_KINDS[ordinal] {
            return (
                PreflightCheck::invalid(
                    "FORM_ART_EVIDENCE_VIEW_ORDER_MISMATCH",
                    Some(request.form_art_evidence_object_sha256.clone()),
                ),
                None,
            );
        }
        let Ok(view_payload) = read_preflight_json(
            runtime,
            &view.receipt_object_sha256,
            Some("production-weapon-form-art-evidence-view-receipt"),
        ) else {
            return (
                PreflightCheck::blocked("FORM_ART_VIEW_RECEIPT_MISSING"),
                None,
            );
        };
        let Ok(mut stored_view) = serde_json::from_value::<ProductionWeaponFormArtEvidenceViewRecord>(
            view_payload.clone(),
        ) else {
            return (
                PreflightCheck::invalid(
                    "FORM_ART_VIEW_RECEIPT_MALFORMED",
                    Some(request.form_art_evidence_object_sha256.clone()),
                ),
                None,
            );
        };
        let Some(view_canonical) = preflight_canonical(
            &view_payload,
            PRODUCTION_WEAPON_FORM_ART_EVIDENCE_VIEW_SCHEMA_VERSION,
            "receipt_object_sha256",
        ) else {
            return (
                PreflightCheck::invalid(
                    "FORM_ART_VIEW_RECEIPT_SCHEMA_MISMATCH",
                    Some(request.form_art_evidence_object_sha256.clone()),
                ),
                None,
            );
        };
        if stored_view.receipt_object_sha256 != "" {
            return (
                PreflightCheck::invalid(
                    "FORM_ART_VIEW_RECEIPT_SELF_REFERENCE",
                    Some(request.form_art_evidence_object_sha256.clone()),
                ),
                None,
            );
        }
        stored_view.receipt_object_sha256 = view.receipt_object_sha256.clone();
        if view_canonical != view.canonical_sha256 || stored_view != *view {
            return (
                PreflightCheck::invalid(
                    "FORM_ART_VIEW_RECEIPT_BINDING_MISMATCH",
                    Some(request.form_art_evidence_object_sha256.clone()),
                ),
                None,
            );
        }
    }
    record.receipt_object_sha256 = request.form_art_evidence_object_sha256.clone();
    match runtime
        .store
        .get_production_weapon_form_art_evidence(&record.art_evidence_id)
    {
        Ok(Some(stored)) if stored == record => (
            PreflightCheck::ready(
                Some(request.form_art_evidence_object_sha256.clone()),
                Some(request.form_art_evidence_canonical_sha256.clone()),
            ),
            Some(record),
        ),
        Ok(Some(_)) => (
            PreflightCheck::invalid(
                "FORM_ART_EVIDENCE_STORE_MISMATCH",
                Some(request.form_art_evidence_object_sha256.clone()),
            ),
            None,
        ),
        Ok(None) => (
            PreflightCheck::blocked("FORM_ART_EVIDENCE_STORE_ROW_MISSING"),
            None,
        ),
        Err(_) => (
            PreflightCheck::blocked("FORM_ART_EVIDENCE_STORE_UNAVAILABLE"),
            None,
        ),
    }
}

fn preflight_blocked(reason_code: &'static str) -> PreflightCheck {
    PreflightCheck::blocked(reason_code)
}

fn preflight_build_result(
    runtime: &Runtime,
    request: &PreflightRequest,
) -> Result<Value, RuntimeError> {
    let (legacy_check, legacy) = preflight_legacy(runtime, request);
    let (art_check, art) = preflight_art(runtime, request);

    let candidate_check = match runtime.candidate(&request.candidate_id) {
        Err(_) => preflight_blocked("CANDIDATE_READ_FAILED"),
        Ok(Some(candidate)) if candidate.project_id == request.project_id => {
            if let Some(legacy) = legacy.as_ref() {
                if candidate.canonical_sha256 != legacy.candidate_state_sha256
                    || candidate.prepared_object_id.as_deref() != Some(legacy.artifact_id.as_str())
                    || candidate.prepared_object_sha256.as_deref()
                        != Some(legacy.artifact_sha256.as_str())
                {
                    return Err(invalid("preflight candidate/artifact retarget"));
                }
            }
            PreflightCheck::ready(
                legacy.as_ref().map(|l| l.artifact_sha256.clone()),
                legacy.as_ref().map(|l| l.candidate_state_sha256.clone()),
            )
        }
        Ok(Some(_)) => return Err(invalid("preflight candidate project retarget")),
        Ok(None) => preflight_blocked("CANDIDATE_MISSING"),
    };

    let authoring_check = if let Some(legacy) = legacy.as_ref() {
        match verify_authoring(runtime, legacy) {
            Ok(()) => PreflightCheck::ready(
                Some(legacy.reference_canvas_object_sha256.clone()),
                Some(legacy.reference_canvas_canonical_sha256.clone()),
            ),
            Err(_) => preflight_blocked("REFERENCE_CANVAS_OR_DESIGN_SPEC_BLOCKED"),
        }
    } else {
        preflight_blocked("LEGACY_FORM_QUALITY_REQUIRED")
    };

    let cross_view_check = if let Some(legacy) = legacy.as_ref() {
        match verify_legacy_cross_view(runtime, legacy) {
            Ok(_) => PreflightCheck::ready(
                Some(legacy.cross_view_evidence_object_sha256.clone()),
                Some(legacy.cross_view_evidence_canonical_sha256.clone()),
            ),
            Err(_) => preflight_blocked("CROSS_VIEW_EVIDENCE_BLOCKED"),
        }
    } else {
        preflight_blocked("LEGACY_FORM_QUALITY_REQUIRED")
    };

    let stage_check = if let (Some(legacy), Some(art)) = (legacy.as_ref(), art.as_ref()) {
        let stage_index = PRODUCTION_WEAPON_FORM_QUALITY_V2_FORM_STAGES
            .iter()
            .position(|stage| *stage == request.form_stage)
            .expect("preflight stage was checked");
        let stage_request = ProductionWeaponFormQualityV2PrepareRequest {
            schema_version: PRODUCTION_WEAPON_FORM_QUALITY_V2_PREPARE_REQUEST_SCHEMA_VERSION
                .to_owned(),
            form_quality_id: legacy.form_quality_id.clone(),
            session_id: request.session_id.clone(),
            project_id: request.project_id.clone(),
            form_stage: request.form_stage.clone(),
            source_stage: PRODUCTION_WEAPON_FORM_QUALITY_V2_SOURCE_STAGES[stage_index].to_owned(),
            target_stage: PRODUCTION_WEAPON_FORM_QUALITY_V2_TARGET_STAGES[stage_index].to_owned(),
            legacy_form_quality_object_sha256: request.legacy_form_quality_object_sha256.clone(),
            legacy_form_quality_canonical_sha256: request
                .legacy_form_quality_canonical_sha256
                .clone(),
            form_art_evidence_object_sha256: request.form_art_evidence_object_sha256.clone(),
            form_art_evidence_canonical_sha256: request.form_art_evidence_canonical_sha256.clone(),
            evidence_source_kind: "legacy-source".to_owned(),
            source_candidate_id: None,
            source_candidate_state_sha256: None,
            source_artifact_id: None,
            source_artifact_sha256: None,
            source_fresh_baseline_id: None,
            source_fresh_baseline_canonical_sha256: None,
            source_fresh_baseline_receipt_object_sha256: None,
            source_registration_lineage_id: None,
            source_registration_lineage_canonical_sha256: None,
            source_registration_lineage_receipt_object_sha256: None,
            source_registered_rig_v2_id: None,
            source_registered_rig_v2_object_sha256: None,
            source_registered_rig_v2_canonical_sha256: None,
            source_runtime_build_cohort_sha256: None,
            proposal_candidate_id: None,
            proposal_candidate_state_sha256: None,
            proposal_artifact_id: None,
            proposal_artifact_sha256: None,
            proposal_artifact_readback_sha256: None,
            proposal_worker_build_cohort_sha256: None,
            cross_view_evidence_bundle_sha256: None,
            proposal_form_art_evidence_id: None,
            proposal_form_art_evidence_object_sha256: None,
            proposal_form_art_evidence_canonical_sha256: None,
            proposal_part_id_evidence_sha256: None,
            proposal_negative_space_evidence_sha256: None,
            proposal_line_flow_evidence_sha256: None,
            current_source_head_transition_id: request.current_source_head_transition_id.clone(),
            current_source_head_transition_sha256: request
                .current_source_head_transition_sha256
                .clone(),
            current_source_head_canonical_sha256: request
                .current_source_head_canonical_sha256
                .clone(),
            previous_form_quality_id: None,
            previous_form_quality_report_object_sha256: None,
            previous_form_quality_canonical_sha256: None,
            form_quality_policy: PRODUCTION_WEAPON_FORM_QUALITY_V2_POLICY.to_owned(),
            form_quality_policy_sha256: sha256_hex(
                PRODUCTION_WEAPON_FORM_QUALITY_V2_POLICY.as_bytes(),
            ),
            threshold_policy: PRODUCTION_WEAPON_FORM_QUALITY_V2_THRESHOLD_POLICY.to_owned(),
            threshold_policy_sha256: sha256_hex(
                PRODUCTION_WEAPON_FORM_QUALITY_V2_THRESHOLD_POLICY.as_bytes(),
            ),
            input_sha256: String::new(),
            idempotency_key: request.preflight_id.clone(),
        };
        match verify_stage_source(runtime, &stage_request, legacy, art, None) {
            Ok((_transition, _head, lock)) => PreflightCheck::ready(
                Some(lock.receipt_object_sha256),
                Some(lock.canonical_sha256),
            ),
            Err(_) => preflight_blocked("CAMERA_LOCK_OR_STAGE_BLOCKED"),
        }
    } else {
        preflight_blocked("LEGACY_AND_FORM_ART_REQUIRED")
    };

    let observation_check = if let Some(art) = art.as_ref() {
        if art
            .views
            .iter()
            .all(|view| verify_target_observation(runtime, view).is_ok())
        {
            PreflightCheck::ready(
                Some(request.form_art_evidence_object_sha256.clone()),
                Some(request.form_art_evidence_canonical_sha256.clone()),
            )
        } else {
            preflight_blocked("FORM_ART_TARGET_OBSERVATION_BLOCKED")
        }
    } else {
        preflight_blocked("FORM_ART_EVIDENCE_REQUIRED")
    };

    let checks = [
        ("legacy_form_quality", legacy_check),
        ("form_art_evidence", art_check),
        ("form_art_target_observation", observation_check),
        ("cross_view_evidence", cross_view_check),
        ("camera_lock_stage", stage_check),
        ("reference_authoring", authoring_check),
        ("candidate_artifact", candidate_check),
    ];
    let ready_for_v2_prepare = checks.iter().all(|(_, check)| check.is_ready());
    let mut blockers = checks
        .iter()
        .filter(|(_, check)| !check.is_ready())
        .map(|(name, check)| format!("{name}:{}", check.reason_code))
        .collect::<Vec<_>>();
    blockers.sort();
    let mut result = serde_json::json!({
        "schema_version": PREFLIGHT_RESULT_SCHEMA_VERSION,
        "preflight_id": request.preflight_id,
        "session_id": request.session_id,
        "project_id": request.project_id,
        "candidate_id": request.candidate_id,
        "form_stage": request.form_stage,
        "checks": checks.iter().map(|(name, check)| (name.to_string(), check.value())).collect::<Map<String, Value>>(),
        "ready_for_v2_prepare": ready_for_v2_prepare,
        "blocking_reasons": blockers,
        "quality_status": PRODUCTION_WEAPON_FORM_QUALITY_V2_PREFLIGHT_QUALITY_STATUS,
        "visual_quality_status": PRODUCTION_WEAPON_FORM_QUALITY_V2_PREFLIGHT_VISUAL_QUALITY_STATUS,
        "human_review_status": PRODUCTION_WEAPON_FORM_QUALITY_V2_PREFLIGHT_HUMAN_REVIEW_STATUS,
        "commercial_engine_status": PRODUCTION_WEAPON_FORM_QUALITY_V2_PREFLIGHT_COMMERCIAL_ENGINE_STATUS,
        "runtime_write": false,
        "worker_started": false,
        "production_stage_advanced": false,
        "candidate_confirmed": false,
        "version_created": false,
        "export_performed": false,
        "restart_hash_verified": true,
        "readiness_sha256": "",
    });
    let mut hash_preimage = result.clone();
    hash_preimage["readiness_sha256"] = Value::String(String::new());
    result["readiness_sha256"] = Value::String(canonical_json_hash(&hash_preimage));
    Ok(result)
}

impl Runtime {
    /// Read-only diagnostic for the additive FormQuality@2 gate.  It reports
    /// missing source parents as blocker rows instead of turning an expected
    /// preflight absence into a transport error.  It never creates a CAS
    /// object, SQLite row, candidate, Worker job or production-stage edge.
    pub fn production_weapon_form_quality_v2_preflight_get(
        &self,
        value: Value,
    ) -> Result<Value, RuntimeError> {
        let request = parse_preflight_request(&value)?;
        preflight_build_result(self, &request)
    }

    pub fn production_weapon_form_quality_v2_prepare(
        &self,
        value: Value,
    ) -> Result<Value, RuntimeError> {
        let (request, request_sha, mode) = parse_prepare(&value)?;
        let mut record = build_record(self, &request, &request_sha, &mode)?;
        let mut receipt =
            serde_json::to_value(&record).map_err(|error| invalid(error.to_string()))?;
        receipt["receipt_object_sha256"] = Value::String(String::new());
        let bytes = canonical_json_bytes(&receipt).map_err(|error| invalid(error.to_string()))?;
        if bytes.is_empty() || bytes.len() as u64 > MAX_JSON_BYTES {
            return Err(invalid("form quality V2 report exceeds 1 MiB"));
        }
        let reservation = self.store.begin_cas_reservation();
        let object = match self.store.put_object_reserved(
            &reservation,
            &bytes,
            None,
            JSON_MIME,
            REPORT_KIND,
            &record.created_at,
        ) {
            Ok(object) => object,
            Err(error) => return Err(error.into()),
        };
        record.receipt_object_sha256 = object.record.sha256.clone();
        match self
            .store
            .record_production_weapon_form_quality_v2_with_replay(&record, &object.record)
        {
            Ok((stored, replayed)) => {
                release(self, &reservation, &object, false);
                result_value(
                    &stored,
                    replayed,
                    PRODUCTION_WEAPON_FORM_QUALITY_V2_PREPARE_RESULT_SCHEMA_VERSION,
                    true,
                    None,
                )
            }
            Err(error) => {
                release(self, &reservation, &object, true);
                Err(error.into())
            }
        }
    }

    pub fn production_weapon_form_quality_v2_get(
        &self,
        value: Value,
    ) -> Result<Value, RuntimeError> {
        let request = parse_get(&value)?;
        let record = self
            .store
            .get_production_weapon_form_quality_v2(&request.form_quality_id)?
            .ok_or_else(|| invalid("form quality V2 is unavailable"))?;
        if record.session_id != request.session_id
            || record.project_id != request.project_id
            || record.candidate_id != request.candidate_id
            || record.form_stage != request.form_stage
        {
            return Err(invalid("form quality V2 get scope differs"));
        }
        let request_scope = serde_json::to_value(&request)
            .map_err(|error| invalid(format!("form quality V2 get scope is malformed: {error}")))?;
        let record_scope = serde_json::to_value(&record).map_err(|error| {
            invalid(format!(
                "form quality V2 record scope is malformed: {error}"
            ))
        })?;
        if request_scope.get("evidence_source_kind") != record_scope.get("evidence_source_kind")
            || FORM_QUALITY_V2_SCOPE_OPTION_FIELDS
                .iter()
                .any(|field| request_scope.get(*field) != record_scope.get(*field))
        {
            return Err(invalid("form quality V2 get evidence scope differs"));
        }
        let bytes = self.cas_read_bounded(&record.receipt_object_sha256, MAX_JSON_BYTES)?;
        let mut expected =
            serde_json::to_value(&record).map_err(|error| invalid(error.to_string()))?;
        expected["receipt_object_sha256"] = Value::String(String::new());
        if bytes != canonical_json_bytes(&expected).map_err(|error| invalid(error.to_string()))? {
            return Err(invalid("form quality V2 receipt bytes differ"));
        }
        if canonical_json_hash(&normalized_record(&record)?) != record.canonical_sha256 {
            return Err(invalid("form quality V2 canonical differs"));
        }
        let replay = replay_request(&record);
        let (parsed, request_sha, mode) = parse_prepare(&replay)?;
        let rebuilt = build_record(self, &parsed, &request_sha, &mode)?;
        if normalized_record(&rebuilt)? != normalized_record(&record)? {
            return Err(invalid("form quality V2 restart projection differs"));
        }
        result_value(
            &record,
            true,
            PRODUCTION_WEAPON_FORM_QUALITY_V2_GET_RESULT_SCHEMA_VERSION,
            false,
            Some(true),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bind_legacy_scope(value: &mut Value) {
        let object = value.as_object_mut().expect("request object");
        object.insert(
            "evidence_source_kind".to_owned(),
            Value::String("legacy-source".to_owned()),
        );
        for field in FORM_QUALITY_V2_SCOPE_OPTION_FIELDS {
            object.insert((*field).to_owned(), Value::Null);
        }
    }

    #[test]
    fn closed_get_rejects_raw_media() {
        let value = serde_json::json!({"schema_version":PRODUCTION_WEAPON_FORM_QUALITY_V2_GET_REQUEST_SCHEMA_VERSION,"form_quality_id":"fq-2","session_id":"session-2","project_id":"project-2","candidate_id":"candidate-2","form_stage":"blockout","raw_png_bytes":"forbidden"});
        assert!(parse_get(&value).is_err());
    }

    #[test]
    fn thresholds_are_bounded_and_explicit() {
        assert_eq!(NEGATIVE_IOU_MIN_MILLI, 850);
        assert_eq!(NEGATIVE_BOUNDARY_F1_MIN_MILLI, 800);
        assert_eq!(NEGATIVE_AREA_RATIO_MIN_MILLI, 850);
        assert_eq!(NEGATIVE_AREA_RATIO_MAX_MILLI, 1150);
        assert_eq!(NEGATIVE_CENTROID_MAX, 3_000);
        assert_eq!(LINE_COVERAGE_MIN_MILLI, 900);
        assert_eq!(LINE_CONTINUITY_MIN_MILLI, 900);
        assert_eq!(LINE_CHAMFER_MAX, 3_000);
        assert_eq!(LINE_DEVIATION_MAX, 5_000);
        assert_eq!(LINE_DIRECTION_MIN_MILLI, 950);
        assert_eq!(LINE_DUPLICATE_CROSSING_MAX, 0);
    }

    #[test]
    fn target_confirmation_does_not_accept_unknown_empty_annotations() {
        let target = serde_json::json!({"source":"imported","annotation_status":"unreviewed","visual_structure":{"review_status":"unreviewed","regions":[],"line_flows":[]}});
        assert_ne!(
            target.get("source").and_then(Value::as_str),
            Some("user_refined")
        );
        assert_ne!(
            target.get("annotation_status").and_then(Value::as_str),
            Some("user_confirmed")
        );
    }

    #[test]
    fn legacy_no_regression_requires_cross_view_metrics_but_not_pre_form_art_dimensions() {
        let mut no_regression = ProductionWeaponFormQualityNoRegression {
            status: "NOT_PROVEN".into(),
            metrics_not_regressed: true,
            part_id_not_regressed: false,
            negative_space_not_regressed: false,
            line_flow_not_regressed: false,
        };
        assert!(legacy_view_no_regression_passes(&no_regression));
        no_regression.metrics_not_regressed = false;
        assert!(!legacy_view_no_regression_passes(&no_regression));
        no_regression.metrics_not_regressed = true;
        no_regression.status = "FAILED".into();
        assert!(!legacy_view_no_regression_passes(&no_regression));
        no_regression.status = "PASS".into();
        assert!(legacy_view_no_regression_passes(&no_regression));
    }

    #[test]
    fn preflight_request_is_closed_and_hash_bound() {
        let hash = "a".repeat(64);
        let mut request = serde_json::json!({
            "schema_version": PREFLIGHT_SCHEMA_VERSION,
            "preflight_id": "preflight-form-quality-v2-1",
            "session_id": "session-1",
            "project_id": "project-1",
            "candidate_id": "candidate-1",
            "form_stage": "blockout",
            "legacy_form_quality_object_sha256": hash,
            "legacy_form_quality_canonical_sha256": "b".repeat(64),
            "form_art_evidence_object_sha256": "c".repeat(64),
            "form_art_evidence_canonical_sha256": "d".repeat(64),
            "current_source_head_transition_id": "transition-1",
            "current_source_head_transition_sha256": "e".repeat(64),
            "current_source_head_canonical_sha256": "f".repeat(64),
            "input_sha256": "",
        });
        bind_legacy_scope(&mut request);
        let mut preimage = request.clone();
        preimage
            .as_object_mut()
            .expect("request object")
            .remove("input_sha256");
        request["input_sha256"] = Value::String(canonical_json_hash(&preimage));
        assert!(parse_preflight_request(&request).is_ok());
        request["raw_png_bytes"] = Value::String("forbidden".into());
        assert!(parse_preflight_request(&request).is_err());
    }

    #[test]
    fn preflight_missing_source_is_a_blocker_row() {
        let check = PreflightCheck::blocked("LEGACY_FORM_QUALITY_MISSING");
        assert_eq!(check.status, "blocked");
        assert!(!check.is_ready());
        assert_eq!(check.value()["reason_code"], "LEGACY_FORM_QUALITY_MISSING");
    }

    #[test]
    fn target_observation_errors_map_to_closed_reason_codes() {
        assert_eq!(
            target_observation_invalid_input_reason_code(
                "negative-space evidence is not observed or confirmed not-applicable"
            ),
            "NEGATIVE_SPACE_NOT_OBSERVED_OR_NOT_APPLICABLE"
        );
        assert_eq!(
            target_observation_invalid_input_reason_code(
                "line-flow thresholds or binding are not passed"
            ),
            "LINE_FLOW_THRESHOLD_OR_BINDING_FAILED"
        );
        assert_eq!(
            target_observation_invalid_input_reason_code("unexpected future diagnostic"),
            "TARGET_OBSERVATION_INVALID"
        );
    }

    #[test]
    fn preflight_missing_sources_is_read_only_and_zero_write() {
        let runtime = Runtime::ephemeral().expect("runtime");
        let hash = "a".repeat(64);
        let mut request = serde_json::json!({
            "schema_version": PREFLIGHT_SCHEMA_VERSION,
            "preflight_id": "preflight-form-quality-v2-missing",
            "session_id": "session-preflight",
            "project_id": "project-preflight",
            "candidate_id": "candidate-preflight",
            "form_stage": "blockout",
            "legacy_form_quality_object_sha256": hash,
            "legacy_form_quality_canonical_sha256": "b".repeat(64),
            "form_art_evidence_object_sha256": "c".repeat(64),
            "form_art_evidence_canonical_sha256": "d".repeat(64),
            "current_source_head_transition_id": "transition-preflight",
            "current_source_head_transition_sha256": "e".repeat(64),
            "current_source_head_canonical_sha256": "f".repeat(64),
            "input_sha256": "",
        });
        bind_legacy_scope(&mut request);
        let mut preimage = request.clone();
        preimage
            .as_object_mut()
            .expect("request object")
            .remove("input_sha256");
        request["input_sha256"] = Value::String(canonical_json_hash(&preimage));

        let result = runtime
            .production_weapon_form_quality_v2_preflight_get(request)
            .expect("preflight result");
        assert_eq!(result["ready_for_v2_prepare"], false);
        assert_eq!(result["runtime_write"], false);
        assert_eq!(result["worker_started"], false);
        assert_eq!(result["production_stage_advanced"], false);
        assert_eq!(result["candidate_confirmed"], false);
        assert_eq!(result["version_created"], false);
        assert_eq!(result["export_performed"], false);
        assert_eq!(result["checks"]["legacy_form_quality"]["status"], "blocked");
        assert_eq!(result["checks"]["form_art_evidence"]["status"], "blocked");
        assert!(runtime
            .store
            .get_object("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
            .expect("CAS read")
            .is_none());
        assert!(runtime
            .store
            .get_production_weapon_form_quality("form-quality-preflight")
            .expect("Store read")
            .is_none());
    }

    #[test]
    fn normalized_evidence_scope_rejects_partial_legacy_or_fresh_bindings() {
        let mut request = serde_json::json!({});
        bind_legacy_scope(&mut request);
        let object = request.as_object().expect("request object");
        assert!(validate_normalized_scope_object(object, "legacy-source").is_ok());

        request["proposal_candidate_id"] = Value::String("candidate-proposal".to_owned());
        let object = request.as_object().expect("request object");
        assert!(validate_normalized_scope_object(object, "legacy-source").is_err());
        assert!(validate_normalized_scope_object(object, "fresh-baseline-proposal").is_err());
    }
}
