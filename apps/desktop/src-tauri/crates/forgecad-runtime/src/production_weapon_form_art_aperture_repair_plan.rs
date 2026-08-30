//! Read-only, hash-bound sequential side-aperture repair planning.
//!
//! Runtime replays the exact 04BE-G visibility calibration, re-derives its
//! parent failure diagnostic, and reads the immutable proposal GeometryProgram.
//! It then emits two ordered, bounded one-Part sensitivity steps. The tool
//! never mutates geometry, writes Store/CAS, creates a candidate, or advances
//! any production gate.

use super::{canonical_json_hash, is_opaque_id, is_sha256, sha256_hex, Runtime, RuntimeError};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

const REQUEST_SCHEMA: &str = "ProductionWeaponFormArtApertureRepairPlanGetRequest@1";
const RESULT_SCHEMA: &str = "ProductionWeaponFormArtApertureRepairPlanGetResult@1";
const OPERATION: &str = "forgecad.production.weapon.form-art-aperture-repair-plan-get@1";
const DERIVATION_POLICY: &str = "exact-raster-calibrated-sequential-aperture-sensitivity-plan@1";
const CALIBRATION_POLICY: &str =
    "exact-before-after-triangle-owner-depth-and-side-aperture-calibration@1";
const FAILURE_POLICY: &str = "exact-parent-proposal-cross-view-form-art-delta-diagnostic@1";
const CANONICALIZATION_POLICY: &str = "canonical-json-sha256-excluding-input-sha256@1";
const MAX_RESPONSE_BYTES: u64 = 1_048_576;
const MAX_JSON_BYTES: u64 = 1_048_576;
const POSITION_TOLERANCE_M: f64 = 1.0e-9;
const TRIAL_DELTAS_M: [f64; 2] = [0.02, 0.04];

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct GetRequest {
    schema_version: String,
    operation: String,
    aperture_repair_plan_id: String,
    visibility_calibration_id: String,
    visibility_calibration_canonical_sha256: String,
    visibility_calibration_input_sha256: String,
    failure_diagnostic_id: String,
    failure_diagnostic_canonical_sha256: String,
    failure_diagnostic_input_sha256: String,
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
    persistent_user_data_touched: bool,
    derivation_policy: String,
    canonicalization_policy: String,
    input_sha256: String,
}

fn invalid(reason: impl Into<String>) -> RuntimeError {
    RuntimeError::InvalidInput(format!(
        "PRODUCTION_WEAPON_FORM_ART_APERTURE_REPAIR_PLAN_INVALID: {}",
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
        || request.persistent_user_data_touched
        || request.derivation_policy != DERIVATION_POLICY
        || request.canonicalization_policy != CANONICALIZATION_POLICY
    {
        return Err(invalid("request schema, operation or policy differs"));
    }
    for id in [
        &request.aperture_repair_plan_id,
        &request.visibility_calibration_id,
        &request.failure_diagnostic_id,
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
        &request.visibility_calibration_canonical_sha256,
        &request.visibility_calibration_input_sha256,
        &request.failure_diagnostic_canonical_sha256,
        &request.failure_diagnostic_input_sha256,
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

fn required_string<'a>(value: &'a Value, field: &str) -> Result<&'a str, RuntimeError> {
    value
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid(format!("{field} is unavailable")))
}

fn read_json(runtime: &Runtime, sha256: &str, label: &str) -> Result<Value, RuntimeError> {
    let bytes = runtime.cas_read_bounded(sha256, MAX_JSON_BYTES)?;
    if sha256_hex(&bytes) != sha256 {
        return Err(invalid(format!("{label} object hash differs")));
    }
    serde_json::from_slice(&bytes)
        .map_err(|error| invalid(format!("{label} JSON is invalid: {error}")))
}

fn view_by_kind<'a>(calibration: &'a Value, kind: &str) -> Result<&'a Value, RuntimeError> {
    calibration
        .get("views")
        .and_then(Value::as_array)
        .and_then(|views| {
            views
                .iter()
                .find(|view| view.get("view_kind").and_then(Value::as_str) == Some(kind))
        })
        .ok_or_else(|| invalid(format!("{kind} calibration view is unavailable")))
}

fn trigger_structure<'a>(view: &'a Value, structure_id: &str) -> Result<&'a Value, RuntimeError> {
    view.get("structures")
        .and_then(Value::as_array)
        .and_then(|structures| {
            structures.iter().find(|structure| {
                structure.get("structure_id").and_then(Value::as_str) == Some(structure_id)
            })
        })
        .ok_or_else(|| invalid(format!("{structure_id} calibration is unavailable")))
}

fn node_by_id<'a>(program: &'a Value, node_id: &str) -> Result<&'a Value, RuntimeError> {
    program
        .get("nodes")
        .and_then(Value::as_array)
        .and_then(|nodes| {
            nodes
                .iter()
                .find(|node| node.get("node_id").and_then(Value::as_str) == Some(node_id))
        })
        .ok_or_else(|| invalid(format!("{node_id} GeometryProgram node is unavailable")))
}

fn vec3(parameters: &Value, field: &str) -> Result<[f64; 3], RuntimeError> {
    let values = parameters
        .get(field)
        .and_then(Value::as_array)
        .filter(|values| values.len() == 3)
        .ok_or_else(|| invalid(format!("{field} is not a three-vector")))?;
    let mut result = [0.0; 3];
    for (index, value) in values.iter().enumerate() {
        result[index] = value
            .as_f64()
            .filter(|number| number.is_finite())
            .ok_or_else(|| invalid(format!("{field} contains a non-finite value")))?;
    }
    Ok(result)
}

fn approx(left: f64, right: f64) -> bool {
    (left - right).abs() <= POSITION_TOLERANCE_M
}

fn verify_node(
    node: &Value,
    node_id: &str,
    operator_id: &str,
    expected_shape: &str,
    expected_position: [f64; 3],
    expected_size: [f64; 3],
) -> Result<([f64; 3], [f64; 3]), RuntimeError> {
    if node.get("node_id").and_then(Value::as_str) != Some(node_id)
        || node.get("operator_id").and_then(Value::as_str) != Some(operator_id)
    {
        return Err(invalid(format!(
            "{node_id} node identity or operator differs"
        )));
    }
    let parameters = node
        .get("parameters")
        .ok_or_else(|| invalid(format!("{node_id} parameters are unavailable")))?;
    if parameters.get("shape").and_then(Value::as_str) != Some(expected_shape) {
        return Err(invalid(format!("{node_id} shape differs")));
    }
    let position = vec3(parameters, "position_m")?;
    let size = vec3(parameters, "size_m")?;
    if position
        .iter()
        .zip(expected_position)
        .any(|(actual, expected)| !approx(*actual, expected))
        || size
            .iter()
            .zip(expected_size)
            .any(|(actual, expected)| !approx(*actual, expected))
    {
        return Err(invalid(format!(
            "{node_id} current parameter binding differs"
        )));
    }
    Ok((position, size))
}

fn trial_variants(position: [f64; 3], size: [f64; 3]) -> Vec<Value> {
    let mut variants = Vec::new();
    for delta in TRIAL_DELTAS_M {
        let next_size_x = size[0] - delta;
        variants.push(json!({
            "variant_id":format!("retract-min-x-{:.0}mm",delta*1000.0),
            "retracted_boundary":"min-x",
            "preserved_boundary":"max-x",
            "retraction_m":delta,
            "position_m":[position[0]+delta/2.0,position[1],position[2]],
            "size_m":[next_size_x,size[1],size[2]]
        }));
        variants.push(json!({
            "variant_id":format!("retract-max-x-{:.0}mm",delta*1000.0),
            "retracted_boundary":"max-x",
            "preserved_boundary":"min-x",
            "retraction_m":delta,
            "position_m":[position[0]-delta/2.0,position[1],position[2]],
            "size_m":[next_size_x,size[1],size[2]]
        }));
    }
    variants
}

fn source_binding(
    view_kind: &str,
    structure_id: &str,
    structure: &Value,
    expected_part_id: &str,
    expected_node_id: &str,
) -> Result<Value, RuntimeError> {
    if structure.get("sealed").and_then(Value::as_bool) != Some(true)
        || structure.get("classification").and_then(Value::as_str)
            != Some("SEALED_BY_UNIQUE_VISIBLE_SOURCE")
        || structure
            .get("unique_highest_proposal_source")
            .and_then(Value::as_bool)
            != Some(true)
        || structure
            .get("winner_changed_pixel_count")
            .and_then(Value::as_u64)
            != Some(0)
        || structure
            .get("depth_changed_pixel_count")
            .and_then(Value::as_u64)
            != Some(0)
        || structure
            .get("part_id_changed_pixel_count")
            .and_then(Value::as_u64)
            != Some(0)
        || structure
            .get("silhouette_changed_pixel_count")
            .and_then(Value::as_u64)
            != Some(0)
    {
        return Err(invalid(format!(
            "{structure_id} sealed zero-response binding differs"
        )));
    }
    let highest = structure
        .get("highest_proposal_source")
        .ok_or_else(|| invalid(format!("{structure_id} highest source is unavailable")))?;
    if highest.get("semantic_part_id").and_then(Value::as_str) != Some(expected_part_id)
        || highest.get("source_node_id").and_then(Value::as_str) != Some(expected_node_id)
    {
        return Err(invalid(format!(
            "{structure_id} calibrated primary source differs"
        )));
    }
    let pixel_count = highest
        .get("pixel_count")
        .and_then(Value::as_u64)
        .ok_or_else(|| invalid(format!("{structure_id} primary pixel count is unavailable")))?;
    let void_count = structure
        .get("ranked_expected_void_pixel_count")
        .and_then(Value::as_u64)
        .filter(|count| *count > 0)
        .ok_or_else(|| invalid(format!("{structure_id} void pixel count is unavailable")))?;
    Ok(json!({
        "view_kind":view_kind,
        "structure_id":structure_id,
        "semantic_part_id":expected_part_id,
        "source_node_id":expected_node_id,
        "primary_visible_pixel_count":pixel_count,
        "expected_void_pixel_count":void_count,
        "primary_coverage_milli":pixel_count*1000/void_count,
        "proposal_sources":structure.get("proposal_sources").cloned().unwrap_or_else(|| json!([])),
        "winner_changed_pixel_count":0,
        "depth_changed_pixel_count":0,
        "part_id_changed_pixel_count":0,
        "silhouette_changed_pixel_count":0
    }))
}

fn non_primary_sources(
    view_kind: &str,
    structure: &Value,
    primary_node_id: &str,
) -> Result<Vec<Value>, RuntimeError> {
    let rows = structure
        .get("proposal_sources")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("proposal source rows are unavailable"))?;
    let planned_nodes = ["side-panel-a", "receiver-upper"];
    Ok(rows
        .iter()
        .filter(|row| row.get("source_node_id").and_then(Value::as_str) != Some(primary_node_id))
        .map(|row| {
            let node_id = row
                .get("source_node_id")
                .and_then(Value::as_str)
                .unwrap_or_default();
            json!({
                "view_kind":view_kind,
                "semantic_part_id":row.get("semantic_part_id").cloned().unwrap_or(Value::Null),
                "source_node_id":node_id,
                "pixel_count":row.get("pixel_count").cloned().unwrap_or(Value::Null),
                "covered_by_primary_plan_step":planned_nodes.contains(&node_id),
                "edit_authorized_by_this_plan":false
            })
        })
        .collect())
}

fn plan_step(
    sequence: u64,
    operation_id: &str,
    view_kind: &str,
    structure_id: &str,
    part_id: &str,
    source_node_id: &str,
    operator_id: &str,
    node: &Value,
    position: [f64; 3],
    size: [f64; 3],
    dependency: Value,
) -> Value {
    json!({
        "sequence":sequence,
        "operation_id":operation_id,
        "primary_view_kind":view_kind,
        "target_structure_id":structure_id,
        "semantic_part_id":part_id,
        "source_node_id":source_node_id,
        "operator_id":operator_id,
        "current_node_canonical_sha256":canonical_json_hash(node),
        "current_position_m":position,
        "current_size_m":size,
        "mutation_family":"bounded-longitudinal-boundary-retraction-search@1",
        "controlled_parameter_paths":["parameters.position_m[0]","parameters.size_m[0]"],
        "trial_variants":trial_variants(position,size),
        "maximum_trial_count":4,
        "dependency":dependency,
        "selection_metric_order":[
            "target-expected-void-background-pixel-gain",
            "target-primary-source-visible-pixel-reduction",
            "boundary-f1-4px-six-view-non-regression",
            "silhouette-iou-six-view-non-regression",
            "bbox-centroid-six-view-non-regression",
            "strict-owner-void-negative-space-line-flow"
        ],
        "execution_status":"NOT_RUN",
        "candidate_budget":4,
        "single_part_only":true
    })
}

pub(crate) fn get(runtime: &Runtime, value: &Value) -> Result<Value, RuntimeError> {
    let request = parse_request(value)?;
    let calibration_request = json!({
        "schema_version":"ProductionWeaponFormArtVisibilityCalibrationGetRequest@1",
        "operation":"forgecad.production.weapon.form-art-visibility-calibration-get@1",
        "calibration_id":request.visibility_calibration_id,
        "failure_diagnostic_id":request.failure_diagnostic_id,
        "failure_diagnostic_canonical_sha256":request.failure_diagnostic_canonical_sha256,
        "failure_diagnostic_input_sha256":request.failure_diagnostic_input_sha256,
        "composite_evidence_id":request.composite_evidence_id,
        "proposal_id":request.proposal_id,
        "session_id":request.session_id,
        "project_id":request.project_id,
        "composite_evidence_record_canonical_sha256":request.composite_evidence_record_canonical_sha256,
        "composite_evidence_receipt_object_sha256":request.composite_evidence_receipt_object_sha256,
        "cross_view_evidence_bundle_sha256":request.cross_view_evidence_bundle_sha256,
        "proposal_form_art_evidence_receipt_object_sha256":request.proposal_form_art_evidence_receipt_object_sha256,
        "max_response_bytes":MAX_RESPONSE_BYTES,
        "runtime_write_performed":false,
        "persistent_user_data_touched":false,
        "calibration_policy":CALIBRATION_POLICY,
        "canonicalization_policy":CANONICALIZATION_POLICY,
        "input_sha256":request.visibility_calibration_input_sha256
    });
    let calibration =
        runtime.production_weapon_form_art_visibility_calibration_get(&calibration_request)?;
    if calibration.get("canonical_sha256").and_then(Value::as_str)
        != Some(request.visibility_calibration_canonical_sha256.as_str())
        || calibration
            .get("side_aperture_occluders_calibrated")
            .and_then(Value::as_bool)
            != Some(true)
        || calibration
            .get("single_common_side_aperture_occluder")
            .and_then(Value::as_bool)
            != Some(false)
        || calibration.get("repair_plan_authorized").and_then(Value::as_bool) != Some(true)
        || calibration
            .get("geometry_repair_authorized")
            .and_then(Value::as_bool)
            != Some(false)
        || calibration.get("next_atomic_action").and_then(Value::as_str)
            != Some("AUTHOR_HASH_BOUND_TYPED_TWO_VIEW_SIDE_APERTURE_REPAIR_PLAN_FOR_CALIBRATED_OCCLUDERS")
    {
        return Err(invalid("visibility calibration binding differs"));
    }

    let failure_request = json!({
        "schema_version":"ProductionWeaponFormArtFailureDiagnosticGetRequest@1",
        "operation":"forgecad.production.weapon.form-art-failure-diagnostic-get@1",
        "diagnostic_id":request.failure_diagnostic_id,
        "composite_evidence_id":request.composite_evidence_id,
        "proposal_id":request.proposal_id,
        "session_id":request.session_id,
        "project_id":request.project_id,
        "composite_evidence_record_canonical_sha256":request.composite_evidence_record_canonical_sha256,
        "composite_evidence_receipt_object_sha256":request.composite_evidence_receipt_object_sha256,
        "cross_view_evidence_bundle_sha256":request.cross_view_evidence_bundle_sha256,
        "proposal_form_art_evidence_receipt_object_sha256":request.proposal_form_art_evidence_receipt_object_sha256,
        "max_response_bytes":MAX_RESPONSE_BYTES,
        "runtime_write_performed":false,
        "persistent_user_data_touched":false,
        "diagnostic_policy":FAILURE_POLICY,
        "canonicalization_policy":CANONICALIZATION_POLICY,
        "input_sha256":request.failure_diagnostic_input_sha256
    });
    let failure = runtime.production_weapon_form_art_failure_diagnostic_get(&failure_request)?;
    if failure.get("canonical_sha256").and_then(Value::as_str)
        != Some(request.failure_diagnostic_canonical_sha256.as_str())
    {
        return Err(invalid("failure diagnostic binding differs"));
    }
    let program_sha256 = required_string(&failure, "proposal_geometry_program_sha256")?;
    let program = read_json(runtime, program_sha256, "proposal GeometryProgram")?;
    // GeometryProgram@2 CAS payloads are content-addressed by their object
    // bytes and intentionally omit an embedded canonical_sha256 field.
    if program.get("schema_version").and_then(Value::as_str) != Some("GeometryProgram@2") {
        return Err(invalid(
            "proposal GeometryProgram canonical binding differs",
        ));
    }

    let left_view = view_by_kind(&calibration, "left")?;
    let right_view = view_by_kind(&calibration, "right")?;
    let left_structure = trigger_structure(left_view, "left.trigger-void")?;
    let right_structure = trigger_structure(right_view, "right.trigger-void")?;
    let left_binding = source_binding(
        "left",
        "left.trigger-void",
        left_structure,
        "side-panel-a",
        "side-panel-a",
    )?;
    let right_binding = source_binding(
        "right",
        "right.trigger-void",
        right_structure,
        "receiver-upper",
        "receiver-upper",
    )?;

    let side_panel = node_by_id(&program, "side-panel-a")?;
    let receiver_upper = node_by_id(&program, "receiver-upper")?;
    let (side_position, side_size) = verify_node(
        side_panel,
        "side-panel-a",
        "forgecad.geometry.panel@2",
        "panel",
        [0.62, 1.78, 0.47],
        [1.3, 0.25, 0.1],
    )?;
    let (receiver_position, receiver_size) = verify_node(
        receiver_upper,
        "receiver-upper",
        "forgecad.geometry.primitive@2",
        "box",
        [-0.25, 1.88, 0.0],
        [2.85, 0.2, 0.92],
    )?;

    let mut non_primary = non_primary_sources("left", left_structure, "side-panel-a")?;
    non_primary.extend(non_primary_sources(
        "right",
        right_structure,
        "receiver-upper",
    )?);
    let steps = vec![
        plan_step(
            1,
            "left-side-panel-a-boundary-sensitivity",
            "left",
            "left.trigger-void",
            "side-panel-a",
            "side-panel-a",
            "forgecad.geometry.panel@2",
            side_panel,
            side_position,
            side_size,
            Value::Null,
        ),
        plan_step(
            2,
            "right-receiver-upper-boundary-sensitivity",
            "right",
            "right.trigger-void",
            "receiver-upper",
            "receiver-upper",
            "forgecad.geometry.primitive@2",
            receiver_upper,
            receiver_position,
            receiver_size,
            json!({
                "requires_operation_id":"left-side-panel-a-boundary-sensitivity",
                "required_status":"RETAINED_SIX_VIEW_NON_REGRESSING_APERTURE_RESPONSE"
            }),
        ),
    ];

    let mut result = json!({
        "schema_version":RESULT_SCHEMA,
        "operation":OPERATION,
        "aperture_repair_plan_id":request.aperture_repair_plan_id,
        "visibility_calibration_id":request.visibility_calibration_id,
        "visibility_calibration_canonical_sha256":request.visibility_calibration_canonical_sha256,
        "failure_diagnostic_id":request.failure_diagnostic_id,
        "failure_diagnostic_canonical_sha256":request.failure_diagnostic_canonical_sha256,
        "composite_evidence_id":request.composite_evidence_id,
        "proposal_id":request.proposal_id,
        "session_id":request.session_id,
        "project_id":request.project_id,
        "proposal_candidate_id":required_string(&calibration,"proposal_candidate_id")?,
        "proposal_candidate_state_sha256":required_string(&calibration,"proposal_candidate_state_sha256")?,
        "proposal_artifact_sha256":required_string(&calibration,"proposal_artifact_sha256")?,
        "proposal_geometry_program_sha256":program_sha256,
        "camera_rig_object_sha256":required_string(&calibration,"camera_rig_object_sha256")?,
        "camera_rig_canonical_sha256":required_string(&calibration,"camera_rig_canonical_sha256")?,
        "target_structure_ids":["left.trigger-void","right.trigger-void"],
        "calibrated_source_bindings":[left_binding,right_binding],
        "non_primary_visible_sources":non_primary,
        "plan_steps":steps,
        "total_trial_candidate_budget":8,
        "execution_policy":"strictly-sequential-one-part-four-trial-maximum@1",
        "preserved_invariants":[
            "original-reference-and-reference-canvas",
            "approved-six-camera-rig",
            "current-proposal-as-parent",
            "all-non-target-node-parameters",
            "target-node-y-z-position-and-y-z-size",
            "material-zone-and-part-output-bindings",
            "rear-stock-and-trigger-guard",
            "no-residual-source-edit-without-new-calibration"
        ],
        "mandatory_revalidation_gates":[
            "strict-glb-readback",
            "same-approved-six-camera-54-aov",
            "target-aperture-primary-source-pixel-reduction",
            "target-aperture-background-pixel-gain",
            "six-view-boundary-f1-and-silhouette-non-regression",
            "strict-owner-void-negative-space-line-flow",
            "fresh-form-quality-v2-preflight-only-after-form-art-ready"
        ],
        "plan_status":"READY_HASH_BOUND_SEQUENTIAL_TWO_PART_APERTURE_SENSITIVITY_PLAN",
        "next_trial_registration_authorized":true,
        "repair_execution_allowed_by_this_tool":false,
        "geometry_repair_performed":false,
        "quality_status":"QUALITY_TARGET_NOT_MET",
        "form_quality_v2_status":"NOT_CREATED",
        "candidate_confirm_allowed":false,
        "secondary_form_approved":"NOT_CREATED",
        "production_stage_advanced":false,
        "candidate_confirmed":false,
        "version_created":false,
        "export_performed":false,
        "runtime_write_performed":false,
        "persistent_user_data_touched":false,
        "worker_started":true,
        "derivation_policy":DERIVATION_POLICY,
        "next_atomic_action":"REGISTER_AND_EXECUTE_STEP_1_SIDE_PANEL_A_BOUNDED_SENSITIVITY_TRIALS_ONLY",
        "canonical_sha256":""
    });
    result["canonical_sha256"] = Value::String(canonical_json_hash(&result));
    let bytes = serde_json::to_vec(&result).map_err(|error| invalid(error.to_string()))?;
    if bytes.len() as u64 > request.max_response_bytes {
        return Err(invalid("response exceeds max_response_bytes"));
    }
    Ok(result)
}
