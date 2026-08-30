//! Read-only before/after raster calibration for the rejected 04BE-E repair.
//!
//! The caller can bind only durable identities and hashes. Runtime replays the
//! exact failure diagnostic, reads the two immutable FormArt proposal evidence
//! objects, then asks the isolated Render Worker for pixel-to-triangle source
//! ownership under the already registered cameras. No raster bytes, camera,
//! geometry, path, URL or executable input is accepted from the caller.

use super::production_weapon_form_art_evidence::{
    fixed_aov_changed_mask, registration_preflight_mask_sha256,
    reviewed_region_owner_audit_masks_with_rotation,
};
use super::render_worker::{
    render_glb_raster_attribution, RasterAttributionSource, RenderWorkerRasterAttribution,
};
use super::{canonical_json_hash, is_opaque_id, is_sha256, sha256_hex, Runtime, RuntimeError};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;

const REQUEST_SCHEMA: &str = "ProductionWeaponFormArtVisibilityCalibrationGetRequest@1";
const RESULT_SCHEMA: &str = "ProductionWeaponFormArtVisibilityCalibrationGetResult@1";
const OPERATION: &str = "forgecad.production.weapon.form-art-visibility-calibration-get@1";
const CALIBRATION_POLICY: &str =
    "exact-before-after-triangle-owner-depth-and-side-aperture-calibration@1";
const FAILURE_POLICY: &str = "exact-parent-proposal-cross-view-form-art-delta-diagnostic@1";
const CANONICALIZATION_POLICY: &str = "canonical-json-sha256-excluding-input-sha256@1";
const MAX_RESPONSE_BYTES: u64 = 1_048_576;
const MAX_JSON_BYTES: u64 = 1_048_576;
const MAX_GLB_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct GetRequest {
    schema_version: String,
    operation: String,
    calibration_id: String,
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
    calibration_policy: String,
    canonicalization_policy: String,
    input_sha256: String,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct WinnerKey {
    semantic_part_id: String,
    source_node_id: String,
    mesh_index: u32,
    primitive_index: u32,
    triangle_index_in_primitive: u32,
}

fn invalid(reason: impl Into<String>) -> RuntimeError {
    let reason = reason.into();
    RuntimeError::InvalidInput(format!(
        "PRODUCTION_WEAPON_FORM_ART_VISIBILITY_CALIBRATION_INVALID: {}",
        reason
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
        || request.calibration_policy != CALIBRATION_POLICY
        || request.canonicalization_policy != CANONICALIZATION_POLICY
    {
        return Err(invalid("request schema, operation or policy differs"));
    }
    for id in [
        &request.calibration_id,
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

pub(crate) fn read_json(
    runtime: &Runtime,
    sha256: &str,
    label: &str,
) -> Result<Value, RuntimeError> {
    let bytes = runtime.cas_read_bounded(sha256, MAX_JSON_BYTES)?;
    if sha256_hex(&bytes) != sha256 {
        return Err(invalid(format!("{label} object hash differs")));
    }
    serde_json::from_slice(&bytes)
        .map_err(|error| invalid(format!("{label} JSON is invalid: {error}")))
}

pub(crate) fn embedded_canonical(
    value: &Value,
    schema: &str,
    label: &str,
) -> Result<String, RuntimeError> {
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

pub(crate) fn required_string<'a>(value: &'a Value, field: &str) -> Result<&'a str, RuntimeError> {
    value
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid(format!("{field} is unavailable")))
}

pub(crate) fn view_by_kind<'a>(evidence: &'a Value, kind: &str) -> Result<&'a Value, RuntimeError> {
    evidence
        .get("views")
        .and_then(Value::as_array)
        .and_then(|views| {
            views
                .iter()
                .find(|view| view.get("view_kind").and_then(Value::as_str) == Some(kind))
        })
        .ok_or_else(|| invalid(format!("{kind} FormArt view is unavailable")))
}

pub(crate) fn canvas_view_by_id<'a>(
    canvas: &'a Value,
    view_id: &str,
) -> Result<&'a Value, RuntimeError> {
    canvas
        .get("views")
        .and_then(Value::as_array)
        .and_then(|views| {
            views
                .iter()
                .find(|view| view.get("view_id").and_then(Value::as_str) == Some(view_id))
        })
        .ok_or_else(|| invalid(format!("ReferenceCanvas view {view_id} is unavailable")))
}

pub(crate) fn registered_camera<'a>(rig: &'a Value, kind: &str) -> Result<&'a Value, RuntimeError> {
    rig.get("renderer_views")
        .and_then(Value::as_array)
        .and_then(|views| {
            views
                .iter()
                .find(|view| view.get("kind").and_then(Value::as_str) == Some(kind))
        })
        .and_then(|view| view.get("registered_camera"))
        .ok_or_else(|| invalid(format!("registered camera {kind} is unavailable")))
}

pub(crate) fn negative_space_row<'a>(
    view: &'a Value,
    structure_id: &str,
) -> Result<&'a Value, RuntimeError> {
    view.get("negative_space_rows")
        .and_then(Value::as_array)
        .and_then(|rows| {
            rows.iter()
                .find(|row| row.get("structure_id").and_then(Value::as_str) == Some(structure_id))
        })
        .ok_or_else(|| invalid(format!("negative-space row {structure_id} is unavailable")))
}

pub(crate) fn ranked_transform(view: &Value) -> Result<&str, RuntimeError> {
    view.get("owner_evidence")
        .and_then(|owner| owner.get("ranked_transform"))
        .and_then(Value::as_str)
        .filter(|value| {
            matches!(
                *value,
                "identity" | "horizontal-flip" | "vertical-flip" | "both-flip"
            )
        })
        .ok_or_else(|| invalid("owner ranked transform is unavailable"))
}

pub(crate) fn transform_mask(mask: &[bool], transform: &str) -> Result<Vec<bool>, RuntimeError> {
    if mask.len() != 512 * 512 {
        return Err(invalid("mask resolution differs"));
    }
    let mut result = vec![false; mask.len()];
    for y in 0..512_usize {
        for x in 0..512_usize {
            let tx = if matches!(transform, "horizontal-flip" | "both-flip") {
                511 - x
            } else {
                x
            };
            let ty = if matches!(transform, "vertical-flip" | "both-flip") {
                511 - y
            } else {
                y
            };
            result[ty * 512 + tx] = mask[y * 512 + x];
        }
    }
    Ok(result)
}

fn source_for_pixel<'a>(
    attribution: &'a RenderWorkerRasterAttribution,
    index: usize,
) -> Result<Option<&'a RasterAttributionSource>, RuntimeError> {
    let offset = index * 4;
    let bytes = attribution
        .triangle_ids_le
        .get(offset..offset + 4)
        .ok_or_else(|| invalid("triangle raster length differs"))?;
    let triangle = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    if triangle == u32::MAX {
        return Ok(None);
    }
    attribution
        .sources
        .get(triangle as usize)
        .filter(|source| source.triangle_index == triangle)
        .map(Some)
        .ok_or_else(|| invalid("triangle source is unavailable"))
}

fn winner_key(source: Option<&RasterAttributionSource>) -> Option<WinnerKey> {
    source.map(|source| WinnerKey {
        semantic_part_id: source.semantic_part_id.clone(),
        source_node_id: source.source_node_id.clone(),
        mesh_index: source.mesh_index,
        primitive_index: source.primitive_index,
        triangle_index_in_primitive: source.triangle_index_in_primitive,
    })
}

pub(crate) fn source_counts(
    attribution: &RenderWorkerRasterAttribution,
    mask: &[bool],
) -> Result<Vec<Value>, RuntimeError> {
    let mut counts = BTreeMap::<(String, String), u64>::new();
    for (index, selected) in mask.iter().enumerate() {
        if !*selected {
            continue;
        }
        let Some(source) = source_for_pixel(attribution, index)? else {
            continue;
        };
        let entry = counts
            .entry((
                source.semantic_part_id.clone(),
                source.source_node_id.clone(),
            ))
            .or_default();
        *entry += 1;
    }
    let mut rows = counts
        .into_iter()
        .map(|((semantic_part_id, source_node_id), pixel_count)| {
            json!({
                "semantic_part_id":semantic_part_id,
                "source_node_id":source_node_id,
                "pixel_count":pixel_count
            })
        })
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| {
        right
            .get("pixel_count")
            .and_then(Value::as_u64)
            .cmp(&left.get("pixel_count").and_then(Value::as_u64))
            .then_with(|| {
                left.get("semantic_part_id")
                    .and_then(Value::as_str)
                    .cmp(&right.get("semantic_part_id").and_then(Value::as_str))
            })
            .then_with(|| {
                left.get("source_node_id")
                    .and_then(Value::as_str)
                    .cmp(&right.get("source_node_id").and_then(Value::as_str))
            })
    });
    Ok(rows)
}

pub(crate) fn count_mask(mask: &[bool]) -> u64 {
    mask.iter().filter(|pixel| **pixel).count() as u64
}

pub(crate) fn count_intersection(left: &[bool], right: &[bool]) -> u64 {
    left.iter()
        .zip(right.iter())
        .filter(|(left, right)| **left && **right)
        .count() as u64
}

pub(crate) fn winner_changed_mask(
    baseline: &RenderWorkerRasterAttribution,
    proposal: &RenderWorkerRasterAttribution,
) -> Result<Vec<bool>, RuntimeError> {
    let mut result = vec![false; 512 * 512];
    for (index, changed) in result.iter_mut().enumerate() {
        *changed = winner_key(source_for_pixel(baseline, index)?)
            != winner_key(source_for_pixel(proposal, index)?);
    }
    Ok(result)
}

fn structure_calibration(
    baseline: &RenderWorkerRasterAttribution,
    proposal: &RenderWorkerRasterAttribution,
    region_mask: &[bool],
    expected_void_mask: &[bool],
    ranked_region_mask: &[bool],
    ranked_expected_void_mask: &[bool],
    depth_changed: &[bool],
    part_changed: &[bool],
    silhouette_changed: &[bool],
    winner_changed: &[bool],
    evidence_row: &Value,
    structure_id: &str,
    transform: &str,
    region_sha256: &str,
) -> Result<Value, RuntimeError> {
    let proposal_sources = source_counts(proposal, ranked_expected_void_mask)?;
    let baseline_sources = source_counts(baseline, ranked_expected_void_mask)?;
    let reference_proposal_sources = source_counts(proposal, expected_void_mask)?;
    let reference_baseline_sources = source_counts(baseline, expected_void_mask)?;
    let highest = proposal_sources.first();
    let highest_count = highest
        .and_then(|row| row.get("pixel_count"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let second_count = proposal_sources
        .get(1)
        .and_then(|row| row.get("pixel_count"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let unique_highest = highest_count > 0 && highest_count > second_count;
    let sealed = evidence_row
        .get("sealed")
        .and_then(Value::as_bool)
        .ok_or_else(|| invalid("negative-space sealed flag is unavailable"))?;
    let classification = if sealed && unique_highest {
        "SEALED_BY_UNIQUE_VISIBLE_SOURCE"
    } else if sealed && highest_count == 0 {
        "SEALED_WITHOUT_VISIBLE_SOURCE_AT_RANKED_EXPECTED_VOID"
    } else if sealed {
        "SEALED_BY_AMBIGUOUS_VISIBLE_SOURCES"
    } else if highest_count == 0 {
        "OPEN_EXPECTED_VOID_BACKGROUND"
    } else {
        "OPEN_EVIDENCE_WITH_VISIBLE_SOURCE_OVERLAP"
    };
    Ok(json!({
        "structure_id":structure_id,
        "evidence_status":required_string(evidence_row,"status")?,
        "sealed":sealed,
        "missing":evidence_row.get("missing").and_then(Value::as_bool).ok_or_else(|| invalid("negative-space missing flag is unavailable"))?,
        "ranked_transform":transform,
        "reviewed_region_canonical_sha256":region_sha256,
        "reference_region_mask_sha256":registration_preflight_mask_sha256(region_mask)?,
        "reference_expected_void_mask_sha256":registration_preflight_mask_sha256(expected_void_mask)?,
        "ranked_region_mask_sha256":registration_preflight_mask_sha256(ranked_region_mask)?,
        "ranked_expected_void_mask_sha256":registration_preflight_mask_sha256(ranked_expected_void_mask)?,
        "ranked_expected_void_pixel_count":count_mask(ranked_expected_void_mask),
        "baseline_visible_source_pixel_count":baseline_sources.iter().filter_map(|row| row.get("pixel_count").and_then(Value::as_u64)).sum::<u64>(),
        "proposal_visible_source_pixel_count":proposal_sources.iter().filter_map(|row| row.get("pixel_count").and_then(Value::as_u64)).sum::<u64>(),
        "reference_baseline_visible_source_pixel_count":reference_baseline_sources.iter().filter_map(|row| row.get("pixel_count").and_then(Value::as_u64)).sum::<u64>(),
        "reference_proposal_visible_source_pixel_count":reference_proposal_sources.iter().filter_map(|row| row.get("pixel_count").and_then(Value::as_u64)).sum::<u64>(),
        "winner_changed_pixel_count":count_intersection(winner_changed,ranked_expected_void_mask),
        "depth_changed_pixel_count":count_intersection(depth_changed,ranked_expected_void_mask),
        "part_id_changed_pixel_count":count_intersection(part_changed,ranked_expected_void_mask),
        "silhouette_changed_pixel_count":count_intersection(silhouette_changed,ranked_expected_void_mask),
        "baseline_sources":baseline_sources,
        "proposal_sources":proposal_sources,
        "reference_baseline_sources":reference_baseline_sources,
        "reference_proposal_sources":reference_proposal_sources,
        "highest_proposal_source":highest.cloned(),
        "unique_highest_proposal_source":unique_highest,
        "classification":classification
    }))
}

pub(crate) fn aov_changed(
    runtime: &Runtime,
    before_view: &Value,
    after_view: &Value,
    field: &str,
    label: &str,
) -> Result<Vec<bool>, RuntimeError> {
    let before_hash = required_string(before_view, field)?;
    let after_hash = required_string(after_view, field)?;
    if !is_sha256(before_hash) || !is_sha256(after_hash) {
        return Err(invalid(format!("{label} AOV hash differs")));
    }
    fixed_aov_changed_mask(
        &runtime.cas_read_bounded(before_hash, MAX_JSON_BYTES)?,
        &runtime.cas_read_bounded(after_hash, MAX_JSON_BYTES)?,
        label,
    )
}

pub(crate) fn get(runtime: &Runtime, value: &Value) -> Result<Value, RuntimeError> {
    let request = parse_request(value)?;
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
        || failure.get("quality_status").and_then(Value::as_str)
            != Some("QUALITY_TARGET_NOT_MET")
        || failure.get("next_atomic_action").and_then(Value::as_str)
            != Some("RUN_HASH_BOUND_OWNER_ATTRIBUTION_AND_SIDE_APERTURE_VISIBILITY_CALIBRATION_BEFORE_ANY_NEW_REGISTERED_GEOMETRY_PROFILE")
    {
        return Err(invalid("failure diagnostic binding differs"));
    }

    let before_receipt = required_string(
        &failure,
        "current_base_form_art_evidence_receipt_object_sha256",
    )?;
    let after_receipt =
        required_string(&failure, "proposal_form_art_evidence_receipt_object_sha256")?;
    let before = read_json(
        runtime,
        before_receipt,
        "current-base FormArt proposal evidence",
    )?;
    let after = read_json(runtime, after_receipt, "proposal FormArt proposal evidence")?;
    let before_canonical = embedded_canonical(
        &before,
        "ProductionWeaponFormArtProposalEvidence@1",
        "current-base FormArt proposal evidence",
    )?;
    let after_canonical = embedded_canonical(
        &after,
        "ProductionWeaponFormArtProposalEvidence@1",
        "proposal FormArt proposal evidence",
    )?;
    if before_canonical
        != required_string(&failure, "current_base_form_art_evidence_canonical_sha256")?
        || after_canonical
            != required_string(&failure, "proposal_form_art_evidence_canonical_sha256")?
        || required_string(&before, "session_id")? != request.session_id
        || required_string(&after, "session_id")? != request.session_id
        || required_string(&before, "project_id")? != request.project_id
        || required_string(&after, "project_id")? != request.project_id
        || required_string(&before, "reference_canvas_object_sha256")?
            != required_string(&after, "reference_canvas_object_sha256")?
        || required_string(&before, "camera_rig_object_sha256")?
            != required_string(&after, "camera_rig_object_sha256")?
    {
        return Err(invalid("before/after FormArt binding differs"));
    }

    let before_artifact = required_string(&before, "proposal_artifact_sha256")?;
    let after_artifact = required_string(&after, "proposal_artifact_sha256")?;
    if before_artifact == after_artifact {
        return Err(invalid("before/after artifact binding differs"));
    }
    let baseline_glb = runtime.cas_read_bounded(before_artifact, MAX_GLB_BYTES)?;
    let proposal_glb = runtime.cas_read_bounded(after_artifact, MAX_GLB_BYTES)?;
    let canvas_hash = required_string(&after, "reference_canvas_object_sha256")?;
    let canvas = read_json(runtime, canvas_hash, "ReferenceCanvas")?;
    if embedded_canonical(&canvas, "ReferenceCanvas@1", "ReferenceCanvas")?
        != required_string(&after, "reference_canvas_canonical_sha256")?
    {
        return Err(invalid("ReferenceCanvas binding differs"));
    }
    let rig_hash = required_string(&after, "camera_rig_object_sha256")?;
    let rig = read_json(runtime, rig_hash, "registered CameraRig")?;
    if rig.get("canonical_sha256").and_then(Value::as_str)
        != Some(required_string(&after, "camera_rig_canonical_sha256")?)
    {
        return Err(invalid("CameraRig binding differs"));
    }

    let mut view_rows = Vec::new();
    let mut side_highest = Vec::<(String, String, String)>::new();
    for kind in ["left", "right", "rear-three-quarter"] {
        let before_view = view_by_kind(&before, kind)?;
        let after_view = view_by_kind(&after, kind)?;
        for field in ["view_id", "camera_hash", "camera_canonical_sha256"] {
            if required_string(before_view, field)? != required_string(after_view, field)? {
                return Err(invalid(format!("{kind} {field} differs")));
            }
        }
        let camera = registered_camera(&rig, kind)?;
        if camera.get("camera_hash").and_then(Value::as_str)
            != Some(required_string(after_view, "camera_hash")?)
            || camera.get("canonical_sha256").and_then(Value::as_str)
                != Some(required_string(after_view, "camera_canonical_sha256")?)
        {
            return Err(invalid(format!("{kind} registered camera binding differs")));
        }
        let baseline_attribution = render_glb_raster_attribution(&baseline_glb, camera)
            .map_err(|_| invalid(format!("{kind} baseline attribution Worker failed")))?;
        let proposal_attribution = render_glb_raster_attribution(&proposal_glb, camera)
            .map_err(|_| invalid(format!("{kind} proposal attribution Worker failed")))?;
        let depth_changed = aov_changed(
            runtime,
            before_view,
            after_view,
            "depth_pass_object_sha256",
            "depth",
        )?;
        let part_changed = aov_changed(
            runtime,
            before_view,
            after_view,
            "part_id_pass_object_sha256",
            "part-id",
        )?;
        let silhouette_changed = aov_changed(
            runtime,
            before_view,
            after_view,
            "silhouette_pass_object_sha256",
            "silhouette",
        )?;
        let winner_changed = winner_changed_mask(&baseline_attribution, &proposal_attribution)?;
        let transform = ranked_transform(after_view)?;
        let view_id = required_string(after_view, "view_id")?;
        let canvas_view = canvas_view_by_id(&canvas, view_id)?;
        if canvas_view.get("kind").and_then(Value::as_str) != Some(kind) {
            return Err(invalid(format!("{kind} ReferenceCanvas binding differs")));
        }
        let target_hash = required_string(canvas_view, "target_sha256")?;
        let target = runtime.read_silhouette_target(target_hash)?;
        let view_spec = canvas_view
            .get("view_spec")
            .ok_or_else(|| invalid(format!("{kind} view spec is unavailable")))?;
        let crop = super::reference_view_crop(view_spec)?;
        let rotation = super::reference_view_rotation_degrees(view_spec)?;
        let target_mask = runtime.target_mask(target_hash, &target)?.mask;
        let projected_target =
            super::project_reference_mask_to_view(&target_mask, view_spec, true)?;
        let visual_structure = target
            .get("visual_structure")
            .ok_or_else(|| invalid(format!("{kind} visual structure is unavailable")))?;
        let structure_ids: [&str; 2] = match kind {
            "left" => ["left.open-stock-void", "left.trigger-void"],
            "right" => ["right.open-stock-void", "right.trigger-void"],
            _ => ["rear3q.open-stock-void", "rear3q.trigger-void"],
        };
        let mut structures = Vec::new();
        for structure_id in structure_ids {
            let (region_sha256, region, expected_void, _) =
                reviewed_region_owner_audit_masks_with_rotation(
                    visual_structure,
                    &projected_target,
                    crop,
                    rotation,
                    structure_id,
                )?;
            let ranked_region = transform_mask(&region, transform)?;
            let ranked_expected_void = transform_mask(&expected_void, transform)?;
            let evidence_row = negative_space_row(after_view, structure_id)?;
            let calibrated = structure_calibration(
                &baseline_attribution,
                &proposal_attribution,
                &region,
                &expected_void,
                &ranked_region,
                &ranked_expected_void,
                &depth_changed,
                &part_changed,
                &silhouette_changed,
                &winner_changed,
                evidence_row,
                structure_id,
                transform,
                &region_sha256,
            )?;
            if matches!(kind, "left" | "right") && structure_id.ends_with("trigger-void") {
                if calibrated
                    .get("unique_highest_proposal_source")
                    .and_then(Value::as_bool)
                    == Some(true)
                {
                    let highest = calibrated
                        .get("highest_proposal_source")
                        .ok_or_else(|| invalid("highest proposal source is unavailable"))?;
                    side_highest.push((
                        kind.to_owned(),
                        required_string(highest, "semantic_part_id")?.to_owned(),
                        required_string(highest, "source_node_id")?.to_owned(),
                    ));
                }
            }
            structures.push(calibrated);
        }
        view_rows.push(json!({
            "view_kind":kind,
            "view_id":view_id,
            "camera_hash":required_string(after_view,"camera_hash")?,
            "camera_canonical_sha256":required_string(after_view,"camera_canonical_sha256")?,
            "ranked_transform":transform,
            "baseline_triangle_ids_sha256":baseline_attribution.triangle_ids_sha256,
            "baseline_source_table_sha256":baseline_attribution.source_table_sha256,
            "proposal_triangle_ids_sha256":proposal_attribution.triangle_ids_sha256,
            "proposal_source_table_sha256":proposal_attribution.source_table_sha256,
            "winner_changed_pixel_count":count_mask(&winner_changed),
            "depth_changed_pixel_count":count_mask(&depth_changed),
            "part_id_changed_pixel_count":count_mask(&part_changed),
            "silhouette_changed_pixel_count":count_mask(&silhouette_changed),
            "structures":structures
        }));
    }

    let side_occluders_calibrated = side_highest.len() == 2;
    let single_common_side_occluder = side_occluders_calibrated
        && side_highest[0].1 == side_highest[1].1
        && side_highest[0].2 == side_highest[1].2;
    let next_atomic_action = if side_occluders_calibrated {
        "AUTHOR_HASH_BOUND_TYPED_TWO_VIEW_SIDE_APERTURE_REPAIR_PLAN_FOR_CALIBRATED_OCCLUDERS"
    } else {
        "BLOCK_NEW_GEOMETRY_PROFILE_UNTIL_SIDE_APERTURE_OCCLUDER_IS_UNAMBIGUOUS"
    };
    let diagnostic_status = if side_occluders_calibrated {
        "TWO_VIEW_SIDE_APERTURE_OCCLUDERS_CALIBRATED_REPAIR_PLAN_ONLY"
    } else {
        "VISIBILITY_CALIBRATION_COMPLETE_OCCLUDER_REMAINS_AMBIGUOUS"
    };
    let mut result = json!({
        "schema_version":RESULT_SCHEMA,
        "operation":OPERATION,
        "calibration_id":request.calibration_id,
        "failure_diagnostic_id":request.failure_diagnostic_id,
        "failure_diagnostic_canonical_sha256":request.failure_diagnostic_canonical_sha256,
        "composite_evidence_id":request.composite_evidence_id,
        "proposal_id":request.proposal_id,
        "session_id":request.session_id,
        "project_id":request.project_id,
        "current_base_candidate_id":required_string(&failure,"current_base_candidate_id")?,
        "current_base_candidate_state_sha256":required_string(&failure,"current_base_candidate_state_sha256")?,
        "proposal_candidate_id":required_string(&failure,"proposal_candidate_id")?,
        "proposal_candidate_state_sha256":required_string(&failure,"proposal_candidate_state_sha256")?,
        "current_base_artifact_sha256":before_artifact,
        "proposal_artifact_sha256":after_artifact,
        "current_base_form_art_evidence_receipt_object_sha256":before_receipt,
        "proposal_form_art_evidence_receipt_object_sha256":after_receipt,
        "camera_rig_object_sha256":rig_hash,
        "camera_rig_canonical_sha256":required_string(&after,"camera_rig_canonical_sha256")?,
        "views":view_rows,
        "side_aperture_occluders_calibrated":side_occluders_calibrated,
        "single_common_side_aperture_occluder":single_common_side_occluder,
        "calibrated_side_aperture_sources":side_highest.iter().map(|(view_kind,semantic_part_id,source_node_id)| json!({"view_kind":view_kind,"semantic_part_id":semantic_part_id,"source_node_id":source_node_id})).collect::<Vec<_>>(),
        "repair_plan_authorized":side_occluders_calibrated,
        "geometry_repair_authorized":false,
        "next_atomic_action":next_atomic_action,
        "diagnostic_status":diagnostic_status,
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
        "calibration_policy":CALIBRATION_POLICY,
        "canonical_sha256":""
    });
    result["canonical_sha256"] = Value::String(canonical_json_hash(&result));
    let bytes = serde_json::to_vec(&result).map_err(|error| invalid(error.to_string()))?;
    if bytes.len() as u64 > request.max_response_bytes {
        return Err(invalid("response exceeds max_response_bytes"));
    }
    Ok(result)
}
