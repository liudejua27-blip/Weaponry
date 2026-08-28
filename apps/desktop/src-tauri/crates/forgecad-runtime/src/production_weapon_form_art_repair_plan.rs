//! Read-only, evidence-bound repair planning for the current production FPS
//! weapon FormArt candidate.
//!
//! The caller supplies durable identities and hashes only. Runtime re-reads
//! the immutable composite evidence sidecar, CrossView bundle, proposal
//! FormArt receipt and composed GeometryProgram, verifies the current
//! quarter-Y/flat-Z rear-stock profile, then derives one registered half-Y /
//! flat-Z repair plan. This tool never edits a mesh, invokes a Worker, writes
//! SQLite/CAS, promotes a Stage, confirms, versions or exports.

use super::{canonical_json_hash, is_opaque_id, is_sha256, sha256_hex, Runtime, RuntimeError};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

const REQUEST_SCHEMA: &str = "ProductionWeaponFormArtRepairPlanGetRequest@1";
const RESULT_SCHEMA: &str = "ProductionWeaponFormArtRepairPlanGetResult@1";
const OPERATION: &str = "forgecad.production.weapon.form-art-repair-plan-get@1";
const DERIVATION_POLICY: &str = "durable-cross-view-form-art-owner-void-repair-plan@1";
const CANONICALIZATION_POLICY: &str = "canonical-json-sha256-excluding-input-sha256@1";
const STRATEGY_ID: &str = "rear-stock-owner-void-half-y-flat-z@1";
const CURRENT_PROFILE_ID: &str = "registered-boundary-bridge-quarter-y-flat-z-relaxation@1";
const TARGET_PROFILE_ID: &str = "registered-boundary-bridge-half-y-flat-z-owner-void@1";
const MAX_RESPONSE_BYTES: u64 = 1_048_576;
const MAX_JSON_BYTES: u64 = 1_048_576;
const POSITION_TOLERANCE_M: f64 = 1.0e-6;

const CURRENT_Y_OFFSETS_M: [f64; 5] = [0.0, -0.003, -0.0045, -0.003, 0.0];
const TARGET_Y_OFFSETS_M: [f64; 5] = [0.0, -0.006, -0.009, -0.006, 0.0];
const STATION_RATIOS_MILLI: [u64; 5] = [0, 250, 500, 750, 1000];
const STATION_ROLES: [&str; 5] = [
    "endpoint-near",
    "quarter-near",
    "center",
    "quarter-far",
    "endpoint-far",
];

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct GetRequest {
    schema_version: String,
    operation: String,
    repair_plan_id: String,
    composite_evidence_id: String,
    proposal_id: String,
    session_id: String,
    project_id: String,
    composite_evidence_record_canonical_sha256: String,
    composite_evidence_receipt_object_sha256: String,
    cross_view_evidence_bundle_sha256: String,
    proposal_form_art_evidence_receipt_object_sha256: String,
    max_response_bytes: u64,
    runtime_write_performed: bool,
    derivation_policy: String,
    canonicalization_policy: String,
    input_sha256: String,
}

fn invalid(reason: impl Into<String>) -> RuntimeError {
    RuntimeError::InvalidInput(format!(
        "PRODUCTION_WEAPON_FORM_ART_REPAIR_PLAN_INVALID: {}",
        reason.into()
    ))
}

fn request_input_sha256(request: &GetRequest) -> Result<String, RuntimeError> {
    let mut value = serde_json::to_value(request).map_err(|error| invalid(error.to_string()))?;
    value
        .as_object_mut()
        .ok_or_else(|| invalid("request object is unavailable"))?
        .remove("input_sha256");
    Ok(canonical_json_hash(&value))
}

fn parse_request(value: &Value) -> Result<GetRequest, RuntimeError> {
    let request: GetRequest =
        serde_json::from_value(value.clone()).map_err(|error| invalid(error.to_string()))?;
    if request.schema_version != REQUEST_SCHEMA
        || request.operation != OPERATION
        || request.max_response_bytes != MAX_RESPONSE_BYTES
        || request.runtime_write_performed
        || request.derivation_policy != DERIVATION_POLICY
        || request.canonicalization_policy != CANONICALIZATION_POLICY
    {
        return Err(invalid("request schema, operation or policy differs"));
    }
    for id in [
        &request.repair_plan_id,
        &request.composite_evidence_id,
        &request.proposal_id,
        &request.session_id,
        &request.project_id,
    ] {
        if !is_opaque_id(id) {
            return Err(invalid("request identity is invalid"));
        }
    }
    for hash in [
        &request.composite_evidence_record_canonical_sha256,
        &request.composite_evidence_receipt_object_sha256,
        &request.cross_view_evidence_bundle_sha256,
        &request.proposal_form_art_evidence_receipt_object_sha256,
        &request.input_sha256,
    ] {
        if !is_sha256(hash) {
            return Err(invalid("request hash is invalid"));
        }
    }
    if request.input_sha256 != request_input_sha256(&request)? {
        return Err(invalid("request input hash differs"));
    }
    Ok(request)
}

fn read_json(runtime: &Runtime, sha256: &str, label: &str) -> Result<Value, RuntimeError> {
    let bytes = runtime.cas_read_bounded(sha256, MAX_JSON_BYTES)?;
    if sha256_hex(&bytes) != sha256 {
        return Err(invalid(format!("{label} object hash differs")));
    }
    serde_json::from_slice(&bytes)
        .map_err(|error| invalid(format!("{label} JSON is invalid: {error}")))
}

fn embedded_canonical(value: &Value, schema: &str, label: &str) -> Result<String, RuntimeError> {
    if value.get("schema_version").and_then(Value::as_str) != Some(schema) {
        return Err(invalid(format!("{label} schema differs")));
    }
    let actual = value
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .filter(|hash| is_sha256(hash))
        .ok_or_else(|| invalid(format!("{label} canonical hash is unavailable")))?;
    let mut normalized = value.clone();
    normalized["canonical_sha256"] = Value::String(String::new());
    let expected = canonical_json_hash(&normalized);
    if actual != expected {
        return Err(invalid(format!("{label} canonical hash differs")));
    }
    Ok(expected)
}

fn view_by_kind<'a>(value: &'a Value, field: &str, kind: &str) -> Result<&'a Value, RuntimeError> {
    value
        .get(field)
        .and_then(Value::as_array)
        .and_then(|rows| {
            rows.iter().find(|row| {
                row.get("kind")
                    .or_else(|| row.get("view_kind"))
                    .and_then(Value::as_str)
                    == Some(kind)
            })
        })
        .ok_or_else(|| invalid(format!("{field} {kind} row is unavailable")))
}

fn finite_f64(value: &Value, field: &str) -> Result<f64, RuntimeError> {
    value
        .get(field)
        .and_then(Value::as_f64)
        .filter(|number| number.is_finite())
        .ok_or_else(|| invalid(format!("{field} is unavailable or non-finite")))
}

fn owner_metric(view: &Value, field: &str) -> Result<f64, RuntimeError> {
    let owner = view
        .get("owner_evidence")
        .and_then(Value::as_object)
        .ok_or_else(|| invalid("owner evidence is unavailable"))?;
    if owner.get("owner_part_id").and_then(Value::as_str) != Some("rear-stock")
        || owner
            .get("strict_owner_void_passed")
            .and_then(Value::as_bool)
            != Some(false)
        || owner
            .get("registered_camera_lineage_verified")
            .and_then(Value::as_bool)
            != Some(true)
        || owner.get("status").and_then(Value::as_str)
            != Some("BLOCKED_PROPOSAL_OWNER_VOID_BINDING")
    {
        return Err(invalid("owner evidence binding or failure status differs"));
    }
    owner
        .get(field)
        .and_then(Value::as_f64)
        .filter(|number| number.is_finite())
        .ok_or_else(|| invalid(format!("owner metric {field} is unavailable")))
}

fn approx(left: f64, right: f64) -> bool {
    (left - right).abs() <= POSITION_TOLERANCE_M
}

fn verify_current_profile(program: &Value) -> Result<(), RuntimeError> {
    if program.get("schema_version").and_then(Value::as_str) != Some("GeometryProgram@2") {
        return Err(invalid("composed GeometryProgram schema differs"));
    }
    let node = program
        .get("nodes")
        .and_then(Value::as_array)
        .and_then(|nodes| {
            nodes
                .iter()
                .find(|node| node.get("node_id").and_then(Value::as_str) == Some("rear-stock"))
        })
        .ok_or_else(|| invalid("rear-stock source node is unavailable"))?;
    if node.get("operator_id").and_then(Value::as_str) != Some("forgecad.geometry.authoring-mesh@1")
    {
        return Err(invalid("rear-stock source operator differs"));
    }
    let vertices = node
        .get("parameters")
        .and_then(|parameters| parameters.get("vertices"))
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("rear-stock authoring vertices are unavailable"))?;
    let mut positions = Vec::<[f64; 3]>::new();
    for vertex in vertices {
        let values = vertex
            .get("position_m")
            .and_then(Value::as_array)
            .filter(|values| values.len() == 3)
            .ok_or_else(|| invalid("rear-stock vertex position is invalid"))?;
        let mut position = [0.0; 3];
        for (index, value) in values.iter().enumerate() {
            position[index] = value
                .as_f64()
                .filter(|number| number.is_finite())
                .ok_or_else(|| invalid("rear-stock vertex position is non-finite"))?;
        }
        positions.push(position);
    }
    if positions.len() != 20 {
        return Err(invalid("rear-stock bridge topology vertex count differs"));
    }
    let min_x = positions
        .iter()
        .map(|position| position[0])
        .fold(f64::INFINITY, f64::min);
    let max_x = positions
        .iter()
        .map(|position| position[0])
        .fold(f64::NEG_INFINITY, f64::max);
    let span_x = max_x - min_x;
    if !span_x.is_finite() || span_x <= 0.0 {
        return Err(invalid("rear-stock longitudinal span is invalid"));
    }
    let endpoint_y = positions
        .iter()
        .filter(|position| approx(position[0], min_x) || approx(position[0], max_x))
        .map(|position| position[1])
        .fold(f64::INFINITY, f64::min);
    if !endpoint_y.is_finite() {
        return Err(invalid("rear-stock endpoint plane is unavailable"));
    }
    for station_index in 0..5 {
        let station_x = min_x + span_x * (STATION_RATIOS_MILLI[station_index] as f64 / 1000.0);
        let expected_y = endpoint_y + CURRENT_Y_OFFSETS_M[station_index];
        let matches = positions
            .iter()
            .filter(|position| approx(position[0], station_x) && approx(position[1], expected_y))
            .collect::<Vec<_>>();
        if matches.len() != 2
            || matches
                .iter()
                .any(|position| !approx(position[2].abs(), 0.43))
        {
            return Err(invalid(format!(
                "rear-stock current profile station {station_index} differs"
            )));
        }
    }
    Ok(())
}

fn profile(profile_id: &str, topology_application: &str) -> Value {
    let station_parameters = STATION_RATIOS_MILLI
        .iter()
        .enumerate()
        .map(|(index, ratio)| {
            json!({
                "station_ratio_milli":ratio,
                "current_y_offset_m":CURRENT_Y_OFFSETS_M[index],
                "target_y_offset_m":TARGET_Y_OFFSETS_M[index],
                "delta_y_m":TARGET_Y_OFFSETS_M[index]-CURRENT_Y_OFFSETS_M[index],
                "current_z_wedge_m":0.0,
                "target_z_wedge_m":0.0
            })
        })
        .collect::<Vec<_>>();
    json!({
        "profile_id":profile_id,
        "selection_policy":"runtime-derived-boundary-bridge-station-roles@1",
        "coordinate_space":"source-local",
        "station_roles":STATION_ROLES,
        "station_parameters":station_parameters,
        "topology_application":topology_application
    })
}

pub(crate) fn get(runtime: &Runtime, value: &Value) -> Result<Value, RuntimeError> {
    let request = parse_request(value)?;
    let record = runtime
        .store
        .get_production_weapon_form_art_composite_evidence(
            &request.project_id,
            &request.proposal_id,
        )?
        .ok_or_else(|| invalid("composite evidence sidecar is unavailable"))?;
    if record.attachment_id != request.composite_evidence_id
        || record.canonical_sha256 != request.composite_evidence_record_canonical_sha256
        || record.attachment_receipt_object_sha256
            != request.composite_evidence_receipt_object_sha256
        || record.cross_view_evidence_bundle_sha256 != request.cross_view_evidence_bundle_sha256
        || record.proposal_form_art_evidence_receipt_object_sha256
            != request.proposal_form_art_evidence_receipt_object_sha256
        || record.status != "SIX_VIEW_EVIDENCE_BOUND_NOT_PROMOTED"
        || record.quality_status != "QUALITY_TARGET_NOT_MET"
        || record.candidate_confirm_allowed
    {
        return Err(invalid("composite evidence sidecar binding differs"));
    }
    let parent = runtime
        .store
        .get_production_weapon_form_art_composite_proposal(
            &request.project_id,
            &request.proposal_id,
        )?
        .ok_or_else(|| invalid("composite proposal parent is unavailable"))?;
    if parent.session_id != request.session_id
        || parent.project_id != request.project_id
        || parent.canonical_sha256 != record.parent_record_canonical_sha256
        || parent.receipt_object_sha256 != record.parent_receipt_object_sha256
        || parent.candidate_confirm_allowed
        || parent.production_stage_advanced
        || parent.candidate_confirmed
        || parent.version_created
        || parent.export_performed
    {
        return Err(invalid("composite proposal parent scope differs"));
    }

    let cross_view = read_json(
        runtime,
        &request.cross_view_evidence_bundle_sha256,
        "CrossView evidence",
    )?;
    super::validate_cross_view_evidence_bundle(&cross_view)?;
    let cross_view_canonical = embedded_canonical(
        &cross_view,
        "CrossViewEvidenceBundle@1",
        "CrossView evidence",
    )?;
    if cross_view.get("project_id").and_then(Value::as_str) != Some(request.project_id.as_str())
        || cross_view.get("session_id").and_then(Value::as_str) != Some(request.session_id.as_str())
        || cross_view.get("candidate_id").and_then(Value::as_str)
            != Some(parent.proposal_candidate_id.as_str())
        || cross_view
            .get("candidate_state_sha256")
            .and_then(Value::as_str)
            != Some(parent.proposal_candidate_state_sha256.as_str())
        || cross_view.get("artifact_sha256").and_then(Value::as_str)
            != Some(parent.proposal_artifact_sha256.as_str())
        || cross_view.get("program_sha256").and_then(Value::as_str)
            != Some(parent.composed_geometry_program_sha256.as_str())
        || cross_view.get("aggregate_status").and_then(Value::as_str)
            != Some("QUALITY_TARGET_NOT_MET")
        || cross_view.get("hard_gate_passed").and_then(Value::as_bool) != Some(false)
        || cross_view.get("non_regressing").and_then(Value::as_bool) != Some(false)
        || cross_view
            .get("promotion")
            .and_then(|promotion| promotion.get("status"))
            .and_then(Value::as_str)
            != Some("rejected-regression")
    {
        return Err(invalid("CrossView candidate or failure binding differs"));
    }

    let form_art = read_json(
        runtime,
        &request.proposal_form_art_evidence_receipt_object_sha256,
        "proposal FormArt evidence",
    )?;
    let form_art_canonical = embedded_canonical(
        &form_art,
        "ProductionWeaponFormArtProposalEvidence@1",
        "proposal FormArt evidence",
    )?;
    if form_art.get("project_id").and_then(Value::as_str) != Some(request.project_id.as_str())
        || form_art.get("session_id").and_then(Value::as_str) != Some(request.session_id.as_str())
        || form_art
            .get("proposal_candidate_id")
            .and_then(Value::as_str)
            != Some(parent.proposal_candidate_id.as_str())
        || form_art
            .get("proposal_candidate_state_sha256")
            .and_then(Value::as_str)
            != Some(parent.proposal_candidate_state_sha256.as_str())
        || form_art
            .get("proposal_artifact_sha256")
            .and_then(Value::as_str)
            != Some(parent.proposal_artifact_sha256.as_str())
        || form_art
            .get("cross_view_evidence_bundle_sha256")
            .and_then(Value::as_str)
            != Some(request.cross_view_evidence_bundle_sha256.as_str())
        || form_art.get("owner_part_id").and_then(Value::as_str) != Some("rear-stock")
        || form_art
            .get("part_id_all_views_observed")
            .and_then(Value::as_bool)
            != Some(true)
        || form_art
            .get("proposal_form_art_evidence_ready")
            .and_then(Value::as_bool)
            != Some(false)
        || form_art.get("quality_status").and_then(Value::as_str) != Some("QUALITY_TARGET_NOT_MET")
    {
        return Err(invalid(
            "proposal FormArt candidate or failure binding differs",
        ));
    }

    let program = read_json(
        runtime,
        &parent.composed_geometry_program_object_sha256,
        "composed GeometryProgram",
    )?;
    if parent.composed_geometry_program_object_sha256 != parent.composed_geometry_program_sha256 {
        return Err(invalid("composed GeometryProgram hash binding differs"));
    }
    verify_current_profile(&program)?;

    let left_cross = view_by_kind(&cross_view, "view_evaluations", "left")?;
    let left_baseline_boundary = finite_f64(
        left_cross
            .get("baseline_metrics")
            .ok_or_else(|| invalid("left baseline metrics are unavailable"))?,
        "boundary_f1_4px",
    )?;
    let left_proposal_boundary = finite_f64(
        left_cross
            .get("proposal_metrics")
            .ok_or_else(|| invalid("left proposal metrics are unavailable"))?,
        "boundary_f1_4px",
    )?;
    if left_cross.get("non_regressing").and_then(Value::as_bool) != Some(false)
        || left_proposal_boundary >= left_baseline_boundary
    {
        return Err(invalid("left boundary is not the recorded regression"));
    }
    let left_form = view_by_kind(&form_art, "views", "left")?;
    let right_form = view_by_kind(&form_art, "views", "right")?;
    let rear_form = view_by_kind(&form_art, "views", "rear-three-quarter")?;
    let left_overlap = owner_metric(left_form, "owner_expected_void_overlap_milli")?;
    let right_overlap = owner_metric(right_form, "owner_expected_void_overlap_milli")?;
    let rear_owner_pixels = owner_metric(rear_form, "owner_region_pixel_count")?;
    let rear_adjacency = owner_metric(rear_form, "owner_boundary_adjacency_milli")?;
    if left_overlap <= 0.0
        || right_overlap <= 0.0
        || rear_owner_pixels != 0.0
        || rear_adjacency != 0.0
    {
        return Err(invalid(
            "owner-void failure metrics differ from the repair aperture",
        ));
    }

    let mut result = json!({
        "schema_version":RESULT_SCHEMA,
        "operation":OPERATION,
        "repair_plan_id":request.repair_plan_id,
        "composite_evidence_id":record.attachment_id,
        "proposal_id":parent.proposal_id,
        "session_id":parent.session_id,
        "project_id":parent.project_id,
        "proposal_candidate_id":parent.proposal_candidate_id,
        "proposal_candidate_state_sha256":parent.proposal_candidate_state_sha256,
        "proposal_artifact_sha256":parent.proposal_artifact_sha256,
        "composed_geometry_program_sha256":parent.composed_geometry_program_sha256,
        "composite_evidence_record_canonical_sha256":record.canonical_sha256,
        "composite_evidence_receipt_object_sha256":record.attachment_receipt_object_sha256,
        "cross_view_evidence_bundle_sha256":record.cross_view_evidence_bundle_sha256,
        "cross_view_canonical_sha256":cross_view_canonical,
        "proposal_form_art_evidence_receipt_object_sha256":record.proposal_form_art_evidence_receipt_object_sha256,
        "proposal_form_art_evidence_canonical_sha256":form_art_canonical,
        "target_part_id":"rear-stock",
        "source_node_id":"rear-stock",
        "strategy_id":STRATEGY_ID,
        "current_profile":profile(CURRENT_PROFILE_ID,"existing-profile-readback"),
        "target_profile":profile(TARGET_PROFILE_ID,"registered-next-repair-only"),
        "evidence_issues":[
            {
                "issue_id":"left-boundary-f1-regression",
                "view_kind":"left",
                "metric":"boundary_f1_4px",
                "observed":left_proposal_boundary,
                "required":left_baseline_boundary,
                "evidence_sha256":record.cross_view_evidence_bundle_sha256,
                "status":"FAIL"
            },
            {
                "issue_id":"left-owner-void-overlap",
                "view_kind":"left",
                "metric":"owner_expected_void_overlap_milli",
                "observed":left_overlap,
                "required":0.0,
                "evidence_sha256":record.proposal_form_art_evidence_receipt_object_sha256,
                "status":"FAIL"
            },
            {
                "issue_id":"right-owner-void-overlap",
                "view_kind":"right",
                "metric":"owner_expected_void_overlap_milli",
                "observed":right_overlap,
                "required":0.0,
                "evidence_sha256":record.proposal_form_art_evidence_receipt_object_sha256,
                "status":"FAIL"
            },
            {
                "issue_id":"rear-three-quarter-owner-attribution",
                "view_kind":"rear-three-quarter",
                "metric":"owner_region_pixel_count",
                "observed":rear_owner_pixels,
                "required":128.0,
                "evidence_sha256":record.proposal_form_art_evidence_receipt_object_sha256,
                "status":"FAIL"
            }
        ],
        "mandatory_revalidation_gates":[
            "same-original-fresh-baseline",
            "same-approved-six-camera-rig",
            "six-view-no-regression",
            "left-boundary-f1-at-least-baseline",
            "strict-owner-void-all-owner-views",
            "negative-space-and-line-flow-resolved",
            "form-quality-v2-fresh-scope"
        ],
        "preserved_invariants":[
            "camera-lock",
            "reference-canvas",
            "original-fresh-baseline",
            "current-composite-base",
            "trigger-guard-aperture",
            "rear-stock-endpoints",
            "rear-stock-lower-beam-and-rear-cap",
            "all-non-rear-stock-parts"
        ],
        "plan_status":"READY_EVIDENCE_BOUND_TYPED_REPAIR_PLAN",
        "quality_status":"QUALITY_TARGET_NOT_MET",
        "repair_execution_status":"NOT_RUN",
        "repair_execution_allowed_by_this_tool":false,
        "candidate_confirm_allowed":false,
        "secondary_form_approved":"NOT_CREATED",
        "production_stage_advanced":false,
        "candidate_confirmed":false,
        "version_created":false,
        "export_performed":false,
        "runtime_write_performed":false,
        "persistent_user_data_touched":false,
        "derivation_policy":DERIVATION_POLICY,
        "canonical_sha256":""
    });
    result["canonical_sha256"] = Value::String(canonical_json_hash(&result));
    Ok(result)
}
