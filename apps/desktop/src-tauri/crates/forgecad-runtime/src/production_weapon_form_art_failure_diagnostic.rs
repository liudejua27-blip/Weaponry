//! Read-only, evidence-bound diagnosis for a rejected production weapon
//! FormArt repair attempt.
//!
//! Runtime binds the exact durable composite proposal/evidence, compares the
//! current-base and proposal GeometryPrograms and FormArt receipts, then
//! separates geometry response, registered-camera owner attribution, trigger
//! aperture visibility and line-flow failures.  No Worker is started and no
//! SQLite/CAS or product state is written.

use super::{canonical_json_hash, is_opaque_id, is_sha256, sha256_hex, Runtime, RuntimeError};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;

const REQUEST_SCHEMA: &str = "ProductionWeaponFormArtFailureDiagnosticGetRequest@1";
const RESULT_SCHEMA: &str = "ProductionWeaponFormArtFailureDiagnosticGetResult@1";
const OPERATION: &str = "forgecad.production.weapon.form-art-failure-diagnostic-get@1";
const DIAGNOSTIC_POLICY: &str = "exact-parent-proposal-cross-view-form-art-delta-diagnostic@1";
const CANONICALIZATION_POLICY: &str = "canonical-json-sha256-excluding-input-sha256@1";
const MAX_RESPONSE_BYTES: u64 = 1_048_576;
const MAX_JSON_BYTES: u64 = 1_048_576;
const EPSILON: f64 = 1.0e-9;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct GetRequest {
    schema_version: String,
    operation: String,
    diagnostic_id: String,
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
    diagnostic_policy: String,
    canonicalization_policy: String,
    input_sha256: String,
}

fn invalid(reason: impl Into<String>) -> RuntimeError {
    RuntimeError::InvalidInput(format!(
        "PRODUCTION_WEAPON_FORM_ART_FAILURE_DIAGNOSTIC_INVALID: {}",
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
        || request.diagnostic_policy != DIAGNOSTIC_POLICY
        || request.canonicalization_policy != CANONICALIZATION_POLICY
    {
        return Err(invalid("request schema, operation or policy differs"));
    }
    for id in [
        &request.diagnostic_id,
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

fn owner(value: &Value) -> Result<&Value, RuntimeError> {
    value
        .get("owner_evidence")
        .filter(|owner| owner.get("owner_part_id").and_then(Value::as_str) == Some("rear-stock"))
        .ok_or_else(|| invalid("rear-stock owner evidence is unavailable"))
}

fn number(value: &Value, field: &str) -> Result<f64, RuntimeError> {
    value
        .get(field)
        .and_then(Value::as_f64)
        .filter(|number| number.is_finite())
        .ok_or_else(|| invalid(format!("{field} is unavailable or non-finite")))
}

fn integer(value: &Value, field: &str) -> Result<u64, RuntimeError> {
    value
        .get(field)
        .and_then(Value::as_u64)
        .ok_or_else(|| invalid(format!("{field} is unavailable")))
}

fn string<'a>(value: &'a Value, field: &str) -> Result<&'a str, RuntimeError> {
    value
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid(format!("{field} is unavailable")))
}

fn bbox(value: &Value, field: &str) -> Result<Vec<u64>, RuntimeError> {
    value
        .get(field)
        .and_then(Value::as_array)
        .filter(|items| items.len() == 4)
        .ok_or_else(|| invalid(format!("{field} is unavailable")))?
        .iter()
        .map(|item| {
            item.as_u64()
                .ok_or_else(|| invalid(format!("{field} is invalid")))
        })
        .collect()
}

fn owner_delta(
    before_form_art: &Value,
    after_form_art: &Value,
    kind: &str,
) -> Result<Value, RuntimeError> {
    let before = owner(view_by_kind(before_form_art, "views", kind)?)?;
    let after = owner(view_by_kind(after_form_art, "views", kind)?)?;
    if before
        .get("registered_camera_lineage_verified")
        .and_then(Value::as_bool)
        != Some(true)
        || after
            .get("registered_camera_lineage_verified")
            .and_then(Value::as_bool)
            != Some(true)
        || before
            .get("strict_owner_void_passed")
            .and_then(Value::as_bool)
            != Some(false)
        || after
            .get("strict_owner_void_passed")
            .and_then(Value::as_bool)
            != Some(false)
    {
        return Err(invalid(format!("{kind} owner evidence status differs")));
    }
    let before_owner_bbox = bbox(before, "owner_bbox_px")?;
    let after_owner_bbox = bbox(after, "owner_bbox_px")?;
    let before_expected_bbox = bbox(before, "expected_void_bbox_px")?;
    let after_expected_bbox = bbox(after, "expected_void_bbox_px")?;
    let before_overlap = integer(before, "owner_expected_void_overlap_milli")?;
    let after_overlap = integer(after, "owner_expected_void_overlap_milli")?;
    let before_adjacency = integer(before, "owner_boundary_adjacency_milli")?;
    let after_adjacency = integer(after, "owner_boundary_adjacency_milli")?;
    let before_pixels = integer(before, "owner_region_pixel_count")?;
    let after_pixels = integer(after, "owner_region_pixel_count")?;
    Ok(json!({
        "view_kind":kind,
        "before":{
            "expected_void_bbox_px":before_expected_bbox,
            "owner_bbox_px":before_owner_bbox,
            "owner_region_pixel_count":before_pixels,
            "owner_expected_void_overlap_milli":before_overlap,
            "owner_boundary_adjacency_milli":before_adjacency,
            "ranked_transform":string(before,"ranked_transform")?
        },
        "after":{
            "expected_void_bbox_px":after_expected_bbox,
            "owner_bbox_px":after_owner_bbox,
            "owner_region_pixel_count":after_pixels,
            "owner_expected_void_overlap_milli":after_overlap,
            "owner_boundary_adjacency_milli":after_adjacency,
            "ranked_transform":string(after,"ranked_transform")?
        },
        "owner_bbox_changed":before_owner_bbox != after_owner_bbox,
        "owner_region_pixel_delta":after_pixels as i64-before_pixels as i64,
        "owner_overlap_delta_milli":after_overlap as i64-before_overlap as i64,
        "owner_adjacency_delta_milli":after_adjacency as i64-before_adjacency as i64
    }))
}

fn rear_stock_vertices(program: &Value) -> Result<BTreeMap<String, [f64; 3]>, RuntimeError> {
    if program.get("schema_version").and_then(Value::as_str) != Some("GeometryProgram@2") {
        return Err(invalid("GeometryProgram schema differs"));
    }
    let node = program
        .get("nodes")
        .and_then(Value::as_array)
        .and_then(|nodes| {
            nodes
                .iter()
                .find(|node| node.get("node_id").and_then(Value::as_str) == Some("rear-stock"))
        })
        .ok_or_else(|| invalid("rear-stock node is unavailable"))?;
    if node.get("operator_id").and_then(Value::as_str) != Some("forgecad.geometry.authoring-mesh@1")
    {
        return Err(invalid("rear-stock operator differs"));
    }
    let vertices = node
        .get("parameters")
        .and_then(|parameters| parameters.get("vertices"))
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("rear-stock vertices are unavailable"))?;
    let mut result = BTreeMap::new();
    for vertex in vertices {
        let id = string(vertex, "element_id")?.to_owned();
        let values = vertex
            .get("position_m")
            .and_then(Value::as_array)
            .filter(|values| values.len() == 3)
            .ok_or_else(|| invalid("rear-stock vertex position is invalid"))?;
        let position = [
            values[0]
                .as_f64()
                .ok_or_else(|| invalid("rear-stock x is invalid"))?,
            values[1]
                .as_f64()
                .ok_or_else(|| invalid("rear-stock y is invalid"))?,
            values[2]
                .as_f64()
                .ok_or_else(|| invalid("rear-stock z is invalid"))?,
        ];
        if position.iter().any(|component| !component.is_finite())
            || result.insert(id, position).is_some()
        {
            return Err(invalid("rear-stock vertex identity or coordinate differs"));
        }
    }
    if result.len() != 20 {
        return Err(invalid("rear-stock vertex count differs"));
    }
    Ok(result)
}

fn geometry_delta(before_program: &Value, after_program: &Value) -> Result<Value, RuntimeError> {
    let before = rear_stock_vertices(before_program)?;
    let after = rear_stock_vertices(after_program)?;
    if before.keys().ne(after.keys()) {
        return Err(invalid("rear-stock stable vertex identity differs"));
    }
    let mut changed = Vec::new();
    for (id, before_position) in &before {
        let after_position = after
            .get(id)
            .ok_or_else(|| invalid("rear-stock vertex disappeared"))?;
        let delta = [
            after_position[0] - before_position[0],
            after_position[1] - before_position[1],
            after_position[2] - before_position[2],
        ];
        if delta.iter().any(|component| component.abs() > EPSILON) {
            changed.push(json!({
                "element_id":id,
                "before_position_m":before_position,
                "after_position_m":after_position,
                "delta_m":delta
            }));
        }
    }
    if changed.len() != 6
        || changed.iter().any(|row| {
            let delta = row
                .get("delta_m")
                .and_then(Value::as_array)
                .expect("constructed delta");
            delta[0].as_f64().unwrap_or(1.0).abs() > EPSILON
                || delta[2].as_f64().unwrap_or(1.0).abs() > EPSILON
                || delta[1].as_f64().unwrap_or(0.0) >= -EPSILON
        })
    {
        return Err(invalid(
            "repair is not the exact six-vertex negative source-local Y delta",
        ));
    }
    Ok(json!({
        "target_part_id":"rear-stock",
        "coordinate_space":"source-local",
        "changed_vertex_count":changed.len(),
        "topology_preserved":true,
        "stable_vertex_ids_preserved":true,
        "x_unchanged":true,
        "z_unchanged":true,
        "y_delta_direction":"negative",
        "changed_vertices":changed
    }))
}

fn negative_space_row<'a>(view: &'a Value, structure_id: &str) -> Result<&'a Value, RuntimeError> {
    view.get("negative_space_rows")
        .and_then(Value::as_array)
        .and_then(|rows| {
            rows.iter()
                .find(|row| row.get("structure_id").and_then(Value::as_str) == Some(structure_id))
        })
        .ok_or_else(|| invalid(format!("negative-space row {structure_id} is unavailable")))
}

fn negative_space_summary(row: &Value, view_kind: &str) -> Result<Value, RuntimeError> {
    Ok(json!({
        "view_kind":view_kind,
        "structure_id":string(row,"structure_id")?,
        "status":string(row,"status")?,
        "sealed":row.get("sealed").and_then(Value::as_bool).ok_or_else(|| invalid("sealed is unavailable"))?,
        "missing":row.get("missing").and_then(Value::as_bool).ok_or_else(|| invalid("missing is unavailable"))?,
        "iou_milli":integer(row,"iou_milli")?,
        "boundary_f1_milli":integer(row,"boundary_f1_milli")?,
        "centroid_error_milli":integer(row,"centroid_error_milli")?
    }))
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
    if cross_view.get("candidate_id").and_then(Value::as_str)
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
            .and_then(|value| value.get("status"))
            .and_then(Value::as_str)
            != Some("rejected-regression")
    {
        return Err(invalid("CrossView failure binding differs"));
    }

    let after_form_art = read_json(
        runtime,
        &request.proposal_form_art_evidence_receipt_object_sha256,
        "proposal FormArt evidence",
    )?;
    let after_form_art_canonical = embedded_canonical(
        &after_form_art,
        "ProductionWeaponFormArtProposalEvidence@1",
        "proposal FormArt evidence",
    )?;
    let before_form_art = read_json(
        runtime,
        &parent.current_base_proposal_evidence_receipt_object_sha256,
        "current-base FormArt evidence",
    )?;
    let before_form_art_canonical = embedded_canonical(
        &before_form_art,
        "ProductionWeaponFormArtProposalEvidence@1",
        "current-base FormArt evidence",
    )?;
    for form_art in [&before_form_art, &after_form_art] {
        if form_art.get("project_id").and_then(Value::as_str) != Some(request.project_id.as_str())
            || form_art.get("session_id").and_then(Value::as_str)
                != Some(request.session_id.as_str())
            || form_art.get("owner_part_id").and_then(Value::as_str) != Some("rear-stock")
            || form_art
                .get("part_id_all_views_observed")
                .and_then(Value::as_bool)
                != Some(true)
            || form_art
                .get("proposal_form_art_evidence_ready")
                .and_then(Value::as_bool)
                != Some(false)
            || form_art
                .get("negative_space_all_views_resolved")
                .and_then(Value::as_bool)
                != Some(false)
            || form_art
                .get("line_flow_all_views_resolved")
                .and_then(Value::as_bool)
                != Some(false)
            || form_art
                .get("strict_owner_void_all_views_passed")
                .and_then(Value::as_bool)
                != Some(false)
            || form_art.get("quality_status").and_then(Value::as_str)
                != Some("QUALITY_TARGET_NOT_MET")
        {
            return Err(invalid("FormArt failure scope differs"));
        }
    }
    if after_form_art
        .get("proposal_candidate_id")
        .and_then(Value::as_str)
        != Some(parent.proposal_candidate_id.as_str())
        || after_form_art
            .get("proposal_candidate_state_sha256")
            .and_then(Value::as_str)
            != Some(parent.proposal_candidate_state_sha256.as_str())
        || after_form_art
            .get("proposal_artifact_sha256")
            .and_then(Value::as_str)
            != Some(parent.proposal_artifact_sha256.as_str())
        || after_form_art
            .get("cross_view_evidence_bundle_sha256")
            .and_then(Value::as_str)
            != Some(request.cross_view_evidence_bundle_sha256.as_str())
        || before_form_art
            .get("proposal_candidate_id")
            .and_then(Value::as_str)
            != Some(parent.current_base_candidate_id.as_str())
        || before_form_art
            .get("proposal_candidate_state_sha256")
            .and_then(Value::as_str)
            != Some(parent.current_base_candidate_state_sha256.as_str())
        || before_form_art
            .get("proposal_artifact_sha256")
            .and_then(Value::as_str)
            != Some(parent.current_base_artifact_sha256.as_str())
    {
        return Err(invalid("FormArt candidate binding differs"));
    }

    let before_program = read_json(
        runtime,
        &parent.current_base_geometry_program_object_sha256,
        "current-base GeometryProgram",
    )?;
    let after_program = read_json(
        runtime,
        &parent.composed_geometry_program_object_sha256,
        "proposal GeometryProgram",
    )?;
    if parent.current_base_geometry_program_object_sha256
        != parent.current_base_geometry_program_sha256
        || parent.composed_geometry_program_object_sha256 != parent.composed_geometry_program_sha256
    {
        return Err(invalid("GeometryProgram object binding differs"));
    }
    let geometry_delta = geometry_delta(&before_program, &after_program)?;
    let owner_deltas = ["left", "right", "rear-three-quarter"]
        .iter()
        .map(|kind| owner_delta(&before_form_art, &after_form_art, kind))
        .collect::<Result<Vec<_>, _>>()?;

    let left_cross = view_by_kind(&cross_view, "view_evaluations", "left")?;
    let left_baseline_boundary = number(
        left_cross
            .get("baseline_metrics")
            .ok_or_else(|| invalid("left baseline metrics are unavailable"))?,
        "boundary_f1_4px",
    )?;
    let left_proposal_boundary = number(
        left_cross
            .get("proposal_metrics")
            .ok_or_else(|| invalid("left proposal metrics are unavailable"))?,
        "boundary_f1_4px",
    )?;
    if left_cross.get("non_regressing").and_then(Value::as_bool) != Some(false)
        || left_proposal_boundary >= left_baseline_boundary
    {
        return Err(invalid("left CrossView regression differs"));
    }
    let left_after = view_by_kind(&after_form_art, "views", "left")?;
    let right_after = view_by_kind(&after_form_art, "views", "right")?;
    let rear_after = view_by_kind(&after_form_art, "views", "rear-three-quarter")?;
    let side_trigger_visibility = vec![
        negative_space_summary(negative_space_row(left_after, "left.trigger-void")?, "left")?,
        negative_space_summary(
            negative_space_row(right_after, "right.trigger-void")?,
            "right",
        )?,
    ];
    let rear_negative_space = vec![
        negative_space_summary(
            negative_space_row(rear_after, "rear3q.open-stock-void")?,
            "rear-three-quarter",
        )?,
        negative_space_summary(
            negative_space_row(rear_after, "rear3q.trigger-void")?,
            "rear-three-quarter",
        )?,
    ];
    if side_trigger_visibility
        .iter()
        .any(|row| row.get("sealed").and_then(Value::as_bool) != Some(true))
        || rear_negative_space
            .iter()
            .any(|row| row.get("status").and_then(Value::as_str) != Some("observed"))
    {
        return Err(invalid(
            "trigger or rear-three-quarter negative-space evidence differs",
        ));
    }
    let line_flow_views = [
        "front",
        "back",
        "left",
        "right",
        "top",
        "rear-three-quarter",
    ]
    .iter()
    .map(|kind| {
        let view = view_by_kind(&after_form_art, "views", kind)?;
        let rows = view
            .get("line_flow_rows")
            .and_then(Value::as_array)
            .ok_or_else(|| invalid("line-flow rows are unavailable"))?;
        let unknown_count = rows
            .iter()
            .filter(|row| row.get("status").and_then(Value::as_str) == Some("unknown"))
            .count();
        let inferred_count = rows
            .iter()
            .filter(|row| row.get("status").and_then(Value::as_str) == Some("inferred"))
            .count();
        let observed_count = rows
            .iter()
            .filter(|row| row.get("status").and_then(Value::as_str) == Some("observed"))
            .count();
        Ok(json!({
            "view_kind":kind,
            "status":string(view,"line_flow_status")?,
            "row_count":rows.len(),
            "unknown_count":unknown_count,
            "inferred_count":inferred_count,
            "observed_count":observed_count
        }))
    })
    .collect::<Result<Vec<_>, RuntimeError>>()?;

    let rear_owner_after = owner(rear_after)?;
    if integer(rear_owner_after, "owner_region_pixel_count")? != 0
        || string(rear_owner_after, "ranked_transform")? != "vertical-flip"
        || integer(
            negative_space_row(rear_after, "rear3q.open-stock-void")?,
            "iou_milli",
        )? < 900
    {
        return Err(invalid(
            "rear-three-quarter attribution conflict signature differs",
        ));
    }

    let mut result = json!({
        "schema_version":RESULT_SCHEMA,
        "operation":OPERATION,
        "diagnostic_id":request.diagnostic_id,
        "composite_evidence_id":record.attachment_id,
        "proposal_id":parent.proposal_id,
        "session_id":parent.session_id,
        "project_id":parent.project_id,
        "current_base_candidate_id":parent.current_base_candidate_id,
        "current_base_candidate_state_sha256":parent.current_base_candidate_state_sha256,
        "proposal_candidate_id":parent.proposal_candidate_id,
        "proposal_candidate_state_sha256":parent.proposal_candidate_state_sha256,
        "current_base_geometry_program_sha256":parent.current_base_geometry_program_sha256,
        "proposal_geometry_program_sha256":parent.composed_geometry_program_sha256,
        "composite_evidence_record_canonical_sha256":record.canonical_sha256,
        "composite_evidence_receipt_object_sha256":record.attachment_receipt_object_sha256,
        "cross_view_evidence_bundle_sha256":record.cross_view_evidence_bundle_sha256,
        "cross_view_canonical_sha256":cross_view_canonical,
        "current_base_form_art_evidence_receipt_object_sha256":parent.current_base_proposal_evidence_receipt_object_sha256,
        "current_base_form_art_evidence_canonical_sha256":before_form_art_canonical,
        "proposal_form_art_evidence_receipt_object_sha256":record.proposal_form_art_evidence_receipt_object_sha256,
        "proposal_form_art_evidence_canonical_sha256":after_form_art_canonical,
        "geometry_delta":geometry_delta,
        "cross_view_delta":{
            "baseline_score":number(&cross_view,"baseline_score")?,
            "proposal_score":number(&cross_view,"proposal_score")?,
            "hard_gate_passed":false,
            "non_regressing":false,
            "promotion_status":"rejected-regression",
            "left_boundary_f1_4px_before":left_baseline_boundary,
            "left_boundary_f1_4px_after":left_proposal_boundary,
            "left_boundary_f1_4px_delta":left_proposal_boundary-left_baseline_boundary
        },
        "owner_deltas":owner_deltas,
        "rear_three_quarter_negative_space":rear_negative_space,
        "side_trigger_visibility":side_trigger_visibility,
        "line_flow_views":line_flow_views,
        "diagnoses":[
            {
                "diagnosis_id":"rear-stock-negative-y-profile-effect",
                "status":"REJECTED_LEFT_BOUNDARY_REGRESSION_NO_LEFT_OWNER_BBOX_EFFECT_RIGHT_OWNER_INTRUSION_INCREASED",
                "next_geometry_repair_allowed":false
            },
            {
                "diagnosis_id":"rear-three-quarter-owner-attribution",
                "status":"ATTRIBUTION_CALIBRATION_CONFLICT_GEOMETRY_VOID_OBSERVED_OWNER_REGION_ZERO_VERTICAL_FLIP",
                "next_geometry_repair_allowed":false
            },
            {
                "diagnosis_id":"side-trigger-aperture-visibility",
                "status":"SEALED_IN_LEFT_RIGHT_OBSERVED_IN_REAR_THREE_QUARTER",
                "next_geometry_repair_allowed":false
            },
            {
                "diagnosis_id":"line-flow-separability",
                "status":"INDEPENDENTLY_UNRESOLVED_NOT_REPAIRABLE_BY_REAR_STOCK_PROFILE",
                "next_geometry_repair_allowed":false
            }
        ],
        "next_atomic_action":"RUN_HASH_BOUND_OWNER_ATTRIBUTION_AND_SIDE_APERTURE_VISIBILITY_CALIBRATION_BEFORE_ANY_NEW_REGISTERED_GEOMETRY_PROFILE",
        "diagnostic_status":"FAILURE_ROOT_CAUSES_SEPARATED_NO_GEOMETRY_REPAIR_AUTHORIZED",
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
        "worker_started":false,
        "diagnostic_policy":DIAGNOSTIC_POLICY,
        "canonical_sha256":""
    });
    result["canonical_sha256"] = Value::String(canonical_json_hash(&result));
    Ok(result)
}
