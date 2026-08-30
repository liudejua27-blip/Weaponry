//! Read-only target-region owner attribution for the rejected 04BE-L receiver
//! upper trials. The caller binds durable candidate/FormArt hashes only;
//! Runtime derives the registered right camera, reference target mask and all
//! raster ownership/depth/Part-ID/silhouette deltas.

use super::production_weapon_form_art_evidence::{
    registration_preflight_mask_sha256, reviewed_region_owner_audit_masks_with_rotation,
};
use super::production_weapon_form_art_visibility_calibration::{
    aov_changed, canvas_view_by_id, count_intersection, count_mask, embedded_canonical,
    negative_space_row, ranked_transform, read_json, registered_camera, required_string,
    source_counts, transform_mask, view_by_kind, winner_changed_mask,
};
use super::render_worker::render_glb_raster_attribution;
use super::{canonical_json_hash, is_opaque_id, is_sha256, sha256_hex, Runtime, RuntimeError};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

const REQUEST_SCHEMA: &str = "ProductionWeaponFormArtTargetOcclusionAttributionGetRequest@1";
const RESULT_SCHEMA: &str = "ProductionWeaponFormArtTargetOcclusionAttributionGetResult@1";
const OPERATION: &str = "forgecad.production.weapon.form-art-target-occlusion-attribution-get@1";
const ATTRIBUTION_POLICY: &str =
    "exact-parent-closed-receiver-upper-family-right-trigger-void-attribution@1";
const CANONICALIZATION_POLICY: &str = "canonical-json-sha256-excluding-input-sha256@1";
const TARGET_STRUCTURE_ID: &str = "right.trigger-void";
const TARGET_VIEW_KIND: &str = "right";
const MAX_RESPONSE_BYTES: u64 = 1_048_576;
const MAX_GLB_BYTES: u64 = 64 * 1024 * 1024;
const REGISTERED_PROFILES: [&str; 4] = [
    "receiver-upper-retract-min-x-20mm@1",
    "receiver-upper-retract-max-x-20mm@1",
    "receiver-upper-retract-min-x-40mm@1",
    "receiver-upper-retract-max-x-40mm@1",
];
const REGISTERED_NOTCH_PROFILES: [&str; 4] = [
    "receiver-upper-target-notch-narrow@1",
    "receiver-upper-target-notch-calibrated@1",
    "receiver-upper-target-notch-raised@1",
    "receiver-upper-target-notch-wide@1",
];
const REGISTERED_CAMERA_TARGET_PROFILES: [&str; 4] = [
    "receiver-upper-camera-target-notch-narrow@2",
    "receiver-upper-camera-target-notch-calibrated@2",
    "receiver-upper-camera-target-notch-raised@2",
    "receiver-upper-camera-target-notch-wide@2",
];

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct CandidateBinding {
    candidate_id: String,
    candidate_state_sha256: String,
    artifact_sha256: String,
    form_art_evidence_receipt_object_sha256: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct TrialBinding {
    registered_profile_id: String,
    proposal_id: String,
    composite_evidence_id: String,
    candidate_id: String,
    candidate_state_sha256: String,
    artifact_sha256: String,
    form_art_evidence_receipt_object_sha256: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct GetRequest {
    schema_version: String,
    operation: String,
    attribution_id: String,
    project_id: String,
    session_id: String,
    parent: CandidateBinding,
    trials: Vec<TrialBinding>,
    max_response_bytes: u64,
    runtime_write_performed: bool,
    persistent_user_data_touched: bool,
    attribution_policy: String,
    canonicalization_policy: String,
    input_sha256: String,
}

fn invalid(reason: impl Into<String>) -> RuntimeError {
    RuntimeError::InvalidInput(format!(
        "PRODUCTION_WEAPON_FORM_ART_TARGET_OCCLUSION_ATTRIBUTION_INVALID: {}",
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

fn validate_candidate(binding: &CandidateBinding) -> Result<(), RuntimeError> {
    if !is_opaque_id(&binding.candidate_id)
        || !is_sha256(&binding.candidate_state_sha256)
        || !is_sha256(&binding.artifact_sha256)
        || !is_sha256(&binding.form_art_evidence_receipt_object_sha256)
    {
        return Err(invalid("candidate binding differs"));
    }
    Ok(())
}

fn parse_request(value: &Value) -> Result<GetRequest, RuntimeError> {
    let request: GetRequest =
        serde_json::from_value(value.clone()).map_err(|error| invalid(error.to_string()))?;
    if request.schema_version != REQUEST_SCHEMA
        || request.operation != OPERATION
        || request.max_response_bytes != MAX_RESPONSE_BYTES
        || request.runtime_write_performed
        || request.persistent_user_data_touched
        || request.attribution_policy != ATTRIBUTION_POLICY
        || request.canonicalization_policy != CANONICALIZATION_POLICY
        || !is_opaque_id(&request.attribution_id)
        || !is_opaque_id(&request.project_id)
        || !is_opaque_id(&request.session_id)
        || !is_sha256(&request.input_sha256)
        || request.trials.len() != REGISTERED_PROFILES.len()
    {
        return Err(invalid("request identity, shape or policy differs"));
    }
    validate_candidate(&request.parent)?;
    let profile_sequence = request
        .trials
        .iter()
        .map(|trial| trial.registered_profile_id.as_str())
        .collect::<Vec<_>>();
    if profile_sequence != REGISTERED_PROFILES
        && profile_sequence != REGISTERED_NOTCH_PROFILES
        && profile_sequence != REGISTERED_CAMERA_TARGET_PROFILES
    {
        return Err(invalid("closed trial profile family differs"));
    }
    for trial in &request.trials {
        if !is_opaque_id(&trial.proposal_id)
            || !is_opaque_id(&trial.composite_evidence_id)
            || !is_opaque_id(&trial.candidate_id)
            || !is_sha256(&trial.candidate_state_sha256)
            || !is_sha256(&trial.artifact_sha256)
            || !is_sha256(&trial.form_art_evidence_receipt_object_sha256)
        {
            return Err(invalid("trial binding or closed profile order differs"));
        }
    }
    if request.input_sha256 != request_input_sha256(&request)? {
        return Err(invalid("request input hash differs"));
    }
    Ok(request)
}

fn read_glb(runtime: &Runtime, sha256: &str, label: &str) -> Result<Vec<u8>, RuntimeError> {
    let bytes = runtime.cas_read_bounded(sha256, MAX_GLB_BYTES)?;
    if sha256_hex(&bytes) != sha256 || bytes.get(0..4) != Some(b"glTF") {
        return Err(invalid(format!("{label} GLB binding differs")));
    }
    Ok(bytes)
}

fn validate_evidence_binding(
    evidence: &Value,
    request: &GetRequest,
    candidate_id: &str,
    candidate_state_sha256: &str,
    artifact_sha256: &str,
    label: &str,
) -> Result<(), RuntimeError> {
    embedded_canonical(evidence, "ProductionWeaponFormArtProposalEvidence@1", label)?;
    if required_string(evidence, "project_id")? != request.project_id
        || required_string(evidence, "session_id")? != request.session_id
        || required_string(evidence, "proposal_candidate_id")? != candidate_id
        || required_string(evidence, "proposal_candidate_state_sha256")? != candidate_state_sha256
        || required_string(evidence, "proposal_artifact_sha256")? != artifact_sha256
        || evidence.get("quality_status").and_then(Value::as_str) != Some("QUALITY_TARGET_NOT_MET")
    {
        return Err(invalid(format!("{label} durable binding differs")));
    }
    Ok(())
}

fn total_source_pixels(rows: &[Value]) -> u64 {
    rows.iter()
        .filter_map(|row| row.get("pixel_count").and_then(Value::as_u64))
        .sum()
}

fn part_pixels(rows: &[Value], part_id: &str) -> u64 {
    rows.iter()
        .filter(|row| row.get("semantic_part_id").and_then(Value::as_str) == Some(part_id))
        .filter_map(|row| row.get("pixel_count").and_then(Value::as_u64))
        .sum()
}

pub(crate) fn get(runtime: &Runtime, value: &Value) -> Result<Value, RuntimeError> {
    let request = parse_request(value)?;
    let parent_evidence = read_json(
        runtime,
        &request.parent.form_art_evidence_receipt_object_sha256,
        "parent FormArt evidence",
    )?;
    validate_evidence_binding(
        &parent_evidence,
        &request,
        &request.parent.candidate_id,
        &request.parent.candidate_state_sha256,
        &request.parent.artifact_sha256,
        "parent FormArt evidence",
    )?;
    let parent_glb = read_glb(runtime, &request.parent.artifact_sha256, "parent")?;
    let parent_view = view_by_kind(&parent_evidence, TARGET_VIEW_KIND)?;
    let rig_hash = required_string(&parent_evidence, "camera_rig_object_sha256")?;
    let rig = read_json(runtime, rig_hash, "registered CameraRig")?;
    if rig.get("canonical_sha256").and_then(Value::as_str)
        != Some(required_string(
            &parent_evidence,
            "camera_rig_canonical_sha256",
        )?)
    {
        return Err(invalid("CameraRig binding differs"));
    }
    let camera = registered_camera(&rig, TARGET_VIEW_KIND)?;
    if camera.get("camera_hash").and_then(Value::as_str)
        != Some(required_string(parent_view, "camera_hash")?)
    {
        return Err(invalid("registered right camera differs"));
    }
    let canvas_hash = required_string(&parent_evidence, "reference_canvas_object_sha256")?;
    let canvas = read_json(runtime, canvas_hash, "ReferenceCanvas")?;
    if embedded_canonical(&canvas, "ReferenceCanvas@1", "ReferenceCanvas")?
        != required_string(&parent_evidence, "reference_canvas_canonical_sha256")?
    {
        return Err(invalid("ReferenceCanvas binding differs"));
    }
    let view_id = required_string(parent_view, "view_id")?;
    let canvas_view = canvas_view_by_id(&canvas, view_id)?;
    let target_hash = required_string(canvas_view, "target_sha256")?;
    let target = runtime.read_silhouette_target(target_hash)?;
    let view_spec = canvas_view
        .get("view_spec")
        .ok_or_else(|| invalid("right view spec is unavailable"))?;
    let crop = super::reference_view_crop(view_spec)?;
    let rotation = super::reference_view_rotation_degrees(view_spec)?;
    let target_mask = runtime.target_mask(target_hash, &target)?.mask;
    let projected_target = super::project_reference_mask_to_view(&target_mask, view_spec, true)?;
    let visual_structure = target
        .get("visual_structure")
        .ok_or_else(|| invalid("right visual structure is unavailable"))?;
    let (region_sha256, _region, expected_void, _) =
        reviewed_region_owner_audit_masks_with_rotation(
            visual_structure,
            &projected_target,
            crop,
            rotation,
            TARGET_STRUCTURE_ID,
        )?;
    let transform = ranked_transform(parent_view)?;
    let ranked_expected_void = transform_mask(&expected_void, transform)?;
    let target_pixel_count = count_mask(&ranked_expected_void);
    let parent_attribution = render_glb_raster_attribution(&parent_glb, camera)
        .map_err(|_| invalid("parent attribution Worker failed"))?;
    let parent_sources = source_counts(&parent_attribution, &ranked_expected_void)?;
    let parent_visible = total_source_pixels(&parent_sources);
    let parent_highest = parent_sources.first().cloned();
    let parent_sealed = negative_space_row(parent_view, TARGET_STRUCTURE_ID)?
        .get("sealed")
        .and_then(Value::as_bool)
        .ok_or_else(|| invalid("parent sealed flag is unavailable"))?;

    let mut trial_rows = Vec::new();
    let mut unique_non_receiver_parts = Vec::<String>::new();
    let mut receiver_is_highest_count = 0_u64;
    for trial in &request.trials {
        let evidence = read_json(
            runtime,
            &trial.form_art_evidence_receipt_object_sha256,
            "trial FormArt evidence",
        )?;
        validate_evidence_binding(
            &evidence,
            &request,
            &trial.candidate_id,
            &trial.candidate_state_sha256,
            &trial.artifact_sha256,
            "trial FormArt evidence",
        )?;
        if required_string(&evidence, "camera_rig_object_sha256")? != rig_hash
            || required_string(&evidence, "reference_canvas_object_sha256")? != canvas_hash
        {
            return Err(invalid("trial camera or ReferenceCanvas cohort differs"));
        }
        let trial_view = view_by_kind(&evidence, TARGET_VIEW_KIND)?;
        if required_string(trial_view, "camera_hash")?
            != required_string(parent_view, "camera_hash")?
            || ranked_transform(trial_view)? != transform
        {
            return Err(invalid("trial right camera or ranked transform differs"));
        }
        let trial_glb = read_glb(runtime, &trial.artifact_sha256, "trial")?;
        let trial_attribution = render_glb_raster_attribution(&trial_glb, camera)
            .map_err(|_| invalid("trial attribution Worker failed"))?;
        let trial_sources = source_counts(&trial_attribution, &ranked_expected_void)?;
        let visible = total_source_pixels(&trial_sources);
        let highest = trial_sources.first().cloned();
        let highest_count = highest
            .as_ref()
            .and_then(|row| row.get("pixel_count"))
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let second_count = trial_sources
            .get(1)
            .and_then(|row| row.get("pixel_count"))
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let unique_highest = highest_count > second_count && highest_count > 0;
        let highest_part = highest
            .as_ref()
            .and_then(|row| row.get("semantic_part_id"))
            .and_then(Value::as_str)
            .unwrap_or("");
        if unique_highest && highest_part == "receiver-upper" {
            receiver_is_highest_count += 1;
        } else if unique_highest && !highest_part.is_empty() {
            unique_non_receiver_parts.push(highest_part.to_owned());
        }
        let winner_changed = winner_changed_mask(&parent_attribution, &trial_attribution)?;
        let depth_changed = aov_changed(
            runtime,
            parent_view,
            trial_view,
            "depth_pass_object_sha256",
            "right depth",
        )?;
        let part_changed = aov_changed(
            runtime,
            parent_view,
            trial_view,
            "part_id_pass_object_sha256",
            "right part-id",
        )?;
        let silhouette_changed = aov_changed(
            runtime,
            parent_view,
            trial_view,
            "silhouette_pass_object_sha256",
            "right silhouette",
        )?;
        let sealed = negative_space_row(trial_view, TARGET_STRUCTURE_ID)?
            .get("sealed")
            .and_then(Value::as_bool)
            .ok_or_else(|| invalid("trial sealed flag is unavailable"))?;
        trial_rows.push(json!({
            "registered_profile_id":trial.registered_profile_id,
            "proposal_id":trial.proposal_id,
            "composite_evidence_id":trial.composite_evidence_id,
            "candidate_id":trial.candidate_id,
            "candidate_state_sha256":trial.candidate_state_sha256,
            "artifact_sha256":trial.artifact_sha256,
            "form_art_evidence_receipt_object_sha256":trial.form_art_evidence_receipt_object_sha256,
            "sealed":sealed,
            "target_pixel_count":target_pixel_count,
            "visible_source_pixel_count":visible,
            "background_pixel_count":target_pixel_count.saturating_sub(visible),
            "receiver_upper_pixel_count":part_pixels(&trial_sources,"receiver-upper"),
            "receiver_upper_pixel_delta":part_pixels(&trial_sources,"receiver-upper") as i64-part_pixels(&parent_sources,"receiver-upper") as i64,
            "highest_source":highest,
            "unique_highest_source":unique_highest,
            "sources":trial_sources,
            "winner_changed_pixel_count":count_intersection(&winner_changed,&ranked_expected_void),
            "depth_changed_pixel_count":count_intersection(&depth_changed,&ranked_expected_void),
            "part_id_changed_pixel_count":count_intersection(&part_changed,&ranked_expected_void),
            "silhouette_changed_pixel_count":count_intersection(&silhouette_changed,&ranked_expected_void),
            "triangle_ids_sha256":trial_attribution.triangle_ids_sha256,
            "source_table_sha256":trial_attribution.source_table_sha256
        }));
    }

    unique_non_receiver_parts.sort();
    unique_non_receiver_parts.dedup();
    let all_sealed = parent_sealed
        && trial_rows
            .iter()
            .all(|row| row.get("sealed").and_then(Value::as_bool) == Some(true));
    let zero_target_response = trial_rows.iter().all(|row| {
        row.get("winner_changed_pixel_count")
            .and_then(Value::as_u64)
            == Some(0)
            && row.get("depth_changed_pixel_count").and_then(Value::as_u64) == Some(0)
            && row
                .get("part_id_changed_pixel_count")
                .and_then(Value::as_u64)
                == Some(0)
            && row
                .get("silhouette_changed_pixel_count")
                .and_then(Value::as_u64)
                == Some(0)
    });
    let (diagnostic_status, next_atomic_action, attributed_part_id) =
        if unique_non_receiver_parts.len() == 1 {
            (
                "NON_RECEIVER_TARGET_OCCLUDER_ATTRIBUTED",
                "AUTHOR_ONE_HASH_BOUND_TYPED_PART_LOCAL_TOPOLOGY_REPAIR_FOR_ATTRIBUTED_OCCLUDER",
                Value::String(unique_non_receiver_parts[0].clone()),
            )
        } else if receiver_is_highest_count == request.trials.len() as u64 && zero_target_response {
            (
                "RECEIVER_UPPER_REMAINS_OCCLUDER_X_RETRACTION_MISSES_OCCLUSION_MECHANISM",
                "AUTHOR_DIFFERENT_AXIS_OR_TOPOLOGY_FAMILY_FOR_RECEIVER_UPPER_TARGET_APERTURE",
                Value::String("receiver-upper".to_owned()),
            )
        } else if zero_target_response {
            (
                "TARGET_REGION_UNCHANGED_OCCLUDER_REMAINS_AMBIGUOUS",
                "BLOCK_GEOMETRY_UNTIL_TARGET_REGION_OCCLUDER_IS_UNAMBIGUOUS",
                Value::Null,
            )
        } else {
            (
                "TARGET_REGION_RESPONDED_WITHOUT_SINGLE_STABLE_OCCLUDER",
                "AUTHOR_BOUNDED_TWO_AXIS_DIAGNOSTIC_BEFORE_ANY_NEW_GEOMETRY_PROFILE",
                Value::Null,
            )
        };

    let mut result = json!({
        "schema_version":RESULT_SCHEMA,
        "operation":OPERATION,
        "attribution_id":request.attribution_id,
        "project_id":request.project_id,
        "session_id":request.session_id,
        "target_view_kind":TARGET_VIEW_KIND,
        "target_structure_id":TARGET_STRUCTURE_ID,
        "camera_hash":required_string(parent_view,"camera_hash")?,
        "camera_canonical_sha256":required_string(parent_view,"camera_canonical_sha256")?,
        "camera_rig_object_sha256":rig_hash,
        "reference_canvas_object_sha256":canvas_hash,
        "reviewed_region_canonical_sha256":region_sha256,
        "ranked_transform":transform,
        "ranked_expected_void_mask_sha256":registration_preflight_mask_sha256(&ranked_expected_void)?,
        "target_pixel_count":target_pixel_count,
        "parent":{
            "candidate_id":request.parent.candidate_id,
            "candidate_state_sha256":request.parent.candidate_state_sha256,
            "artifact_sha256":request.parent.artifact_sha256,
            "form_art_evidence_receipt_object_sha256":request.parent.form_art_evidence_receipt_object_sha256,
            "sealed":parent_sealed,
            "visible_source_pixel_count":parent_visible,
            "background_pixel_count":target_pixel_count.saturating_sub(parent_visible),
            "receiver_upper_pixel_count":part_pixels(&parent_sources,"receiver-upper"),
            "highest_source":parent_highest,
            "sources":parent_sources,
            "triangle_ids_sha256":parent_attribution.triangle_ids_sha256,
            "source_table_sha256":parent_attribution.source_table_sha256
        },
        "trials":trial_rows,
        "all_parent_and_trials_sealed":all_sealed,
        "all_trials_zero_target_response":zero_target_response,
        "attributed_part_id":attributed_part_id,
        "diagnostic_status":diagnostic_status,
        "next_atomic_action":next_atomic_action,
        "geometry_repair_authorized":false,
        "appearance_uv_pbr_write_authorized":false,
        "topology_stage_unlocked":false,
        "quality_status":"QUALITY_TARGET_NOT_MET",
        "form_quality_v2_status":"NOT_CREATED",
        "candidate_confirm_allowed":false,
        "secondary_form_approved":"NOT_CREATED",
        "production_stage_advanced":false,
        "runtime_write_performed":false,
        "persistent_user_data_touched":false,
        "worker_started":true,
        "attribution_policy":ATTRIBUTION_POLICY,
        "canonical_sha256":""
    });
    result["canonical_sha256"] = Value::String(canonical_json_hash(&result));
    let bytes = serde_json::to_vec(&result).map_err(|error| invalid(error.to_string()))?;
    if bytes.len() as u64 > request.max_response_bytes {
        return Err(invalid("response exceeds max_response_bytes"));
    }
    Ok(result)
}
