//! Bounded authoring profile for an original, nonfunctional energy rifle.
//!
//! This module is deliberately a planning/validation boundary.  It turns a
//! typed visual brief into existing `ParametricDesignKitRequest@1` envelopes;
//! it never creates a project, candidate, Job, version or CAS object.  The
//! Runtime remains the only writer, and all permanent geometry still has to
//! go through the normal GeometryProgram@2 hash/readback/approval gates.

#![allow(dead_code)]

use forgecad_contracts::{is_opaque_id, is_sha256};
use forgecad_core::canonical_json_hash;
use serde_json::{json, Map, Value};
use std::collections::HashSet;

pub(crate) const PROFILE_SCHEMA: &str = "FictionalEnergyRifleProfile@1";
pub(crate) const PLAN_SCHEMA: &str = "FictionalEnergyRiflePlan@1";
pub(crate) const PDK_REQUEST_SCHEMA: &str = "ParametricDesignKitRequest@1";
pub(crate) const PROFILE_SCOPE: &str = "fictional-game-asset";
pub(crate) const HQ_360_BLOCKED: &str = "BLOCKED_REFERENCE_COVERAGE";

const STAGES: [&str; 3] = ["primary-form", "secondary-structure", "tertiary-detail"];
const MATERIALS: [&str; 8] = [
    "white-dielectric-clearcoat",
    "dark-painted-metal",
    "black-anodized-metal",
    "brushed-steel",
    "engineering-plastic",
    "joint-rubber",
    "warm-orange-emissive",
    "micro-scratch-coat",
];

/// Validate a hash-bound, nonfunctional visual-asset profile.
pub(crate) fn validate_profile(value: &Value) -> Result<(), String> {
    let object = exact_object(
        value,
        &[
            "schema_version",
            "profile_id",
            "project_id",
            "scope",
            "nonfunctional_asset",
            "subject_coordinate_frame_sha256",
            "operator_catalog_sha256",
            "representation_plan_sha256",
            "style_language",
            "reference_policy",
            "macro_intents",
            "quality_contract",
            "canonical_sha256",
        ],
        "FictionalEnergyRifleProfile@1",
    )?;
    if string(object, "schema_version")? != PROFILE_SCHEMA
        || string(object, "scope")? != PROFILE_SCOPE
        || object.get("nonfunctional_asset").and_then(Value::as_bool) != Some(true)
    {
        return Err(
            "FICTIONAL_WEAPON_PROFILE_INVALID: nonfunctional asset scope is required".to_owned(),
        );
    }
    id(object, "profile_id")?;
    id(object, "project_id")?;
    sha(object, "subject_coordinate_frame_sha256")?;
    sha(object, "operator_catalog_sha256")?;
    sha(object, "representation_plan_sha256")?;
    verify_canonical_hash(value, "FictionalEnergyRifleProfile@1")?;
    validate_style(object.get("style_language").ok_or_else(|| {
        "FICTIONAL_WEAPON_PROFILE_INVALID: style_language is required".to_owned()
    })?)?;
    validate_reference_policy(object.get("reference_policy").ok_or_else(|| {
        "FICTIONAL_WEAPON_PROFILE_INVALID: reference_policy is required".to_owned()
    })?)?;
    validate_quality_contract(object.get("quality_contract").ok_or_else(|| {
        "FICTIONAL_WEAPON_PROFILE_INVALID: quality_contract is required".to_owned()
    })?)?;

    let macros = object
        .get("macro_intents")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            "FICTIONAL_WEAPON_PROFILE_INVALID: macro_intents must be an array".to_owned()
        })?;
    if !(3..=12).contains(&macros.len()) {
        return Err(
            "FICTIONAL_WEAPON_PROFILE_INVALID: macro_intents must contain 3..12 entries".to_owned(),
        );
    }
    let mut part_ids = HashSet::new();
    let mut has_primary = false;
    for (index, macro_value) in macros.iter().enumerate() {
        validate_macro(macro_value, index)?;
        let macro_object = macro_value.as_object().expect("validated macro object");
        let part_id = id(macro_object, "part_id")?;
        if !part_ids.insert(part_id.to_owned()) {
            return Err(format!(
                "FICTIONAL_WEAPON_PROFILE_INVALID: duplicate semantic part_id {part_id}"
            ));
        }
        has_primary |= string(macro_object, "stage")? == "primary-form";
    }
    if !has_primary {
        return Err(
            "FICTIONAL_WEAPON_PROFILE_INVALID: at least one primary-form macro is required"
                .to_owned(),
        );
    }
    Ok(())
}

/// Expand a profile into existing PDK requests without any Runtime write.
/// The returned plan is intentionally structural-only and keeps all visual
/// likeness, PBR, human-review and 360-degree claims locked.
pub(crate) fn expand_profile(value: &Value) -> Result<Value, String> {
    validate_profile(value)?;
    let object = value
        .as_object()
        .expect("validate_profile accepted an object");
    let profile_sha256 = object
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .expect("canonical hash was validated");
    let project_id = id(object, "project_id")?.to_owned();
    let representation_plan_sha256 = sha(object, "representation_plan_sha256")?.to_owned();
    let subject_coordinate_frame_sha256 =
        sha(object, "subject_coordinate_frame_sha256")?.to_owned();
    let operator_catalog_sha256 = sha(object, "operator_catalog_sha256")?.to_owned();
    let reference_state = object
        .get("reference_policy")
        .and_then(Value::as_object)
        .and_then(|policy| policy.get("reference_state"))
        .and_then(Value::as_str)
        .expect("reference policy was validated");

    let macros = object
        .get("macro_intents")
        .and_then(Value::as_array)
        .expect("macro intents were validated");
    let mut macro_requests = Vec::with_capacity(macros.len());
    for macro_value in macros {
        let macro_object = macro_value.as_object().expect("macro object was validated");
        let mut request = json!({
            "schema_version": PDK_REQUEST_SCHEMA,
            "project_id": project_id,
            "representation_plan_sha256": representation_plan_sha256,
            "kit_id": macro_object.get("kit_id").expect("kit id was validated"),
            "part_id": macro_object.get("part_id").expect("part id was validated"),
            "material_zone_id": macro_object.get("material_zone_id").expect("material zone was validated"),
            "intent": macro_object.get("intent").expect("intent was validated"),
            "input_sha256": ""
        });
        let mut input_binding = request.clone();
        input_binding
            .as_object_mut()
            .expect("request is an object")
            .remove("input_sha256");
        request["input_sha256"] = Value::String(canonical_json_hash(&input_binding));
        macro_requests.push(request);
    }

    let mut plan = json!({
        "schema_version": PLAN_SCHEMA,
        "profile_sha256": profile_sha256,
        "project_id": project_id,
        "representation_plan_sha256": representation_plan_sha256,
        "subject_coordinate_frame_sha256": subject_coordinate_frame_sha256,
        "operator_catalog_sha256": operator_catalog_sha256,
        "scope": PROFILE_SCOPE,
        "reference_state": reference_state,
        "macro_requests": macro_requests,
        "stage_order": STAGES,
        "quality_status": "structural_only",
        "limitations": [
            "candidate_not_created",
            "runtime_write_not_performed",
            "strict_glb_readback_required_after_geometry_prepare",
            "joint_multiview_compare_required_before_visual_claim",
            "pbr_and_texture_likeness_not_evaluated",
            "human_visual_review_required",
            "hq_360_blocked_until_reference_coverage"
        ],
        "hq_360_status": HQ_360_BLOCKED,
        "candidate_created": false,
        "runtime_write_performed": false,
        "canonical_sha256": ""
    });
    plan["canonical_sha256"] = Value::String(canonical_json_hash(&plan));
    validate_plan(&plan)?;
    Ok(plan)
}

pub(crate) fn validate_plan(value: &Value) -> Result<(), String> {
    let object = exact_object(
        value,
        &[
            "schema_version",
            "profile_sha256",
            "project_id",
            "representation_plan_sha256",
            "subject_coordinate_frame_sha256",
            "operator_catalog_sha256",
            "scope",
            "reference_state",
            "macro_requests",
            "stage_order",
            "quality_status",
            "limitations",
            "hq_360_status",
            "candidate_created",
            "runtime_write_performed",
            "canonical_sha256",
        ],
        PLAN_SCHEMA,
    )?;
    if string(object, "schema_version")? != PLAN_SCHEMA
        || string(object, "scope")? != PROFILE_SCOPE
        || string(object, "quality_status")? != "structural_only"
        || string(object, "hq_360_status")? != HQ_360_BLOCKED
        || object.get("candidate_created").and_then(Value::as_bool) != Some(false)
        || object
            .get("runtime_write_performed")
            .and_then(Value::as_bool)
            != Some(false)
    {
        return Err("FICTIONAL_WEAPON_PLAN_INVALID: structural-only constants drifted".to_owned());
    }
    sha(object, "profile_sha256")?;
    id(object, "project_id")?;
    sha(object, "representation_plan_sha256")?;
    sha(object, "subject_coordinate_frame_sha256")?;
    sha(object, "operator_catalog_sha256")?;
    let reference_state = string(object, "reference_state")?;
    if !matches!(
        reference_state,
        "original-brief" | "authorized-reference-cas-bound" | "authorized-reference-partial"
    ) {
        return Err("FICTIONAL_WEAPON_PLAN_INVALID: reference_state is unsupported".to_owned());
    }
    let stage_order = object
        .get("stage_order")
        .and_then(Value::as_array)
        .ok_or_else(|| "FICTIONAL_WEAPON_PLAN_INVALID: stage_order must be an array".to_owned())?;
    if stage_order.len() != STAGES.len()
        || stage_order
            .iter()
            .zip(STAGES)
            .any(|(value, expected)| value.as_str() != Some(expected))
    {
        return Err("FICTIONAL_WEAPON_PLAN_INVALID: stage_order is not canonical".to_owned());
    }
    let requests = object
        .get("macro_requests")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            "FICTIONAL_WEAPON_PLAN_INVALID: macro_requests must be an array".to_owned()
        })?;
    if !(3..=12).contains(&requests.len()) {
        return Err(
            "FICTIONAL_WEAPON_PLAN_INVALID: macro_requests must contain 3..12 entries".to_owned(),
        );
    }
    let mut parts = HashSet::new();
    for request in requests {
        validate_pdk_request(request)?;
        let request_object = request.as_object().expect("validated request object");
        let part = id(request_object, "part_id")?;
        if !parts.insert(part.to_owned()) {
            return Err(format!(
                "FICTIONAL_WEAPON_PLAN_INVALID: duplicate part_id {part}"
            ));
        }
    }
    let limitations = object
        .get("limitations")
        .and_then(Value::as_array)
        .ok_or_else(|| "FICTIONAL_WEAPON_PLAN_INVALID: limitations must be an array".to_owned())?;
    if limitations.is_empty()
        || limitations.len() > 16
        || limitations.iter().any(|item| {
            item.as_str()
                .is_none_or(|text| text.is_empty() || text.len() > 240)
        })
    {
        return Err("FICTIONAL_WEAPON_PLAN_INVALID: limitations are invalid".to_owned());
    }
    verify_canonical_hash(value, PLAN_SCHEMA)
}

fn validate_style(value: &Value) -> Result<(), String> {
    let object = exact_object(
        value,
        &[
            "silhouette",
            "surface_language",
            "shell_material_id",
            "dark_material_id",
            "accent_material_id",
            "accent_placement",
            "detail_density",
        ],
        "FictionalEnergyRifleProfile@1.style_language",
    )?;
    enum_value(
        object,
        "silhouette",
        &["long-forward", "compact-forward", "triangular-spine"],
    )?;
    enum_value(
        object,
        "surface_language",
        &[
            "layered-hard-surface",
            "clean-panel-ridge",
            "inset-energy-channel",
        ],
    )?;
    material(object, "shell_material_id")?;
    material(object, "dark_material_id")?;
    if string(object, "accent_material_id")? != "warm-orange-emissive" {
        return Err(
            "FICTIONAL_WEAPON_PROFILE_INVALID: accent material is not in the active AssetPack"
                .to_owned(),
        );
    }
    enum_value(
        object,
        "accent_placement",
        &[
            "receiver-core-and-channel",
            "receiver-core-only",
            "channel-only",
        ],
    )?;
    enum_value(
        object,
        "detail_density",
        &["primary", "secondary", "tertiary"],
    )
}

fn validate_reference_policy(value: &Value) -> Result<(), String> {
    let object = exact_object(
        value,
        &[
            "reference_state",
            "unseen_regions",
            "hq_360_status",
            "quality_claim_allowed",
            "visual_match_claim_allowed",
            "requires_user_approval_before_geometry_prepare",
        ],
        "FictionalEnergyRifleProfile@1.reference_policy",
    )?;
    enum_value(
        object,
        "reference_state",
        &[
            "original-brief",
            "authorized-reference-cas-bound",
            "authorized-reference-partial",
        ],
    )?;
    if string(object, "hq_360_status")? != HQ_360_BLOCKED
        || object.get("quality_claim_allowed").and_then(Value::as_bool) != Some(false)
        || object
            .get("visual_match_claim_allowed")
            .and_then(Value::as_bool)
            != Some(false)
        || object
            .get("requires_user_approval_before_geometry_prepare")
            .and_then(Value::as_bool)
            != Some(true)
    {
        return Err(
            "FICTIONAL_WEAPON_PROFILE_INVALID: reference policy must fail closed".to_owned(),
        );
    }
    let unseen = object
        .get("unseen_regions")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            "FICTIONAL_WEAPON_PROFILE_INVALID: unseen_regions must be an array".to_owned()
        })?;
    if unseen.len() > 32 {
        return Err("FICTIONAL_WEAPON_PROFILE_INVALID: unseen_regions exceeds bound".to_owned());
    }
    for item in unseen {
        if !is_opaque_id(item.as_str().unwrap_or_default()) {
            return Err(
                "FICTIONAL_WEAPON_PROFILE_INVALID: unseen region must be an opaque id".to_owned(),
            );
        }
    }
    Ok(())
}

fn validate_quality_contract(value: &Value) -> Result<(), String> {
    let object = exact_object(
        value,
        &[
            "strict_glb_readback",
            "joint_multiview_compare",
            "pbr_after_silhouette_gate",
            "human_review_required",
            "confirm_export_requires_user",
            "max_primary_form_rounds",
            "max_macro_requests",
        ],
        "FictionalEnergyRifleProfile@1.quality_contract",
    )?;
    for key in [
        "strict_glb_readback",
        "joint_multiview_compare",
        "pbr_after_silhouette_gate",
        "human_review_required",
        "confirm_export_requires_user",
    ] {
        if object.get(key).and_then(Value::as_bool) != Some(true) {
            return Err(format!(
                "FICTIONAL_WEAPON_PROFILE_INVALID: quality contract {key} must be true"
            ));
        }
    }
    let rounds = object
        .get("max_primary_form_rounds")
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            "FICTIONAL_WEAPON_PROFILE_INVALID: max_primary_form_rounds is required".to_owned()
        })?;
    if !(1..=5).contains(&rounds)
        || object.get("max_macro_requests").and_then(Value::as_u64) != Some(12)
    {
        return Err("FICTIONAL_WEAPON_PROFILE_INVALID: quality budget is out of bounds".to_owned());
    }
    Ok(())
}

fn validate_macro(value: &Value, index: usize) -> Result<(), String> {
    let context = format!("FictionalEnergyRifleProfile@1.macro_intents[{index}]");
    let object = exact_object(
        value,
        &[
            "part_id",
            "kit_id",
            "operator_id",
            "material_zone_id",
            "stage",
            "visibility",
            "symmetry",
            "intent",
        ],
        &context,
    )?;
    id(object, "part_id")?;
    let kit_id = string(object, "kit_id")?;
    let expected_operator = operator_for_kit(kit_id)?;
    if string(object, "operator_id")? != expected_operator {
        return Err(format!(
            "FICTIONAL_WEAPON_PROFILE_INVALID: {context}.operator_id does not match kit_id"
        ));
    }
    material(object, "material_zone_id")?;
    enum_value(object, "stage", &STAGES)?;
    enum_value(object, "visibility", &["observed", "inferred", "unknown"])?;
    enum_value(
        object,
        "symmetry",
        &["independent", "mirror-left-right", "paired"],
    )?;
    let intent = object
        .get("intent")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            format!("FICTIONAL_WEAPON_PROFILE_INVALID: {context}.intent must be an object")
        })?;
    validate_kit_intent(kit_id, intent)
}

fn validate_pdk_request(value: &Value) -> Result<(), String> {
    let object = exact_object(
        value,
        &[
            "schema_version",
            "project_id",
            "representation_plan_sha256",
            "kit_id",
            "part_id",
            "material_zone_id",
            "intent",
            "input_sha256",
        ],
        "FictionalEnergyRiflePlan@1.macro_requests[]",
    )?;
    if string(object, "schema_version")? != PDK_REQUEST_SCHEMA {
        return Err("FICTIONAL_WEAPON_PLAN_INVALID: PDK request schema drifted".to_owned());
    }
    id(object, "project_id")?;
    sha(object, "representation_plan_sha256")?;
    let kit_id = string(object, "kit_id")?;
    operator_for_kit(kit_id)?;
    id(object, "part_id")?;
    material(object, "material_zone_id")?;
    let intent = object
        .get("intent")
        .and_then(Value::as_object)
        .ok_or_else(|| "FICTIONAL_WEAPON_PLAN_INVALID: PDK intent must be an object".to_owned())?;
    validate_kit_intent(kit_id, intent)?;
    let input_sha256 = sha(object, "input_sha256")?;
    let mut binding = value.clone();
    binding
        .as_object_mut()
        .expect("request object was validated")
        .remove("input_sha256");
    if canonical_json_hash(&binding) != input_sha256 {
        return Err("FICTIONAL_WEAPON_PLAN_INPUT_HASH_MISMATCH".to_owned());
    }
    Ok(())
}

fn validate_kit_intent(kit_id: &str, intent: &Map<String, Value>) -> Result<(), String> {
    match kit_id {
        "forgecad.kit.housing@1" | "forgecad.kit.panel@1" | "forgecad.kit.frame@1" => {
            exact_keys(
                intent,
                &[
                    "size_m",
                    "thickness_m",
                    "bevel_m",
                    "position_m",
                    "rotation_rad",
                ],
                kit_id,
            )?;
            let size = vector3(intent, "size_m", kit_id)?;
            if size.iter().any(|value| *value <= 0.0 || *value > 10.0) {
                return Err(format!(
                    "FICTIONAL_WEAPON_PROFILE_INVALID: {kit_id}.size_m is out of bounds"
                ));
            }
            let thickness = positive_number(intent, "thickness_m", 10.0, kit_id)?;
            let bevel = nonnegative_number(intent, "bevel_m", 5.0, kit_id)?;
            if thickness > size[2] || bevel * 2.0 >= size[0].min(size[1]) {
                return Err(format!(
                    "FICTIONAL_WEAPON_PROFILE_INVALID: {kit_id} panel relationship is invalid"
                ));
            }
            vector3(intent, "position_m", kit_id)?;
            vector3(intent, "rotation_rad", kit_id)?;
        }
        "forgecad.kit.vent@1" => {
            exact_keys(
                intent,
                &[
                    "width_m",
                    "height_m",
                    "depth_m",
                    "slot_count",
                    "slot_width_m",
                    "slot_spacing_m",
                    "position_m",
                    "rotation_rad",
                ],
                kit_id,
            )?;
            let width = positive_number(intent, "width_m", 10.0, kit_id)?;
            positive_number(intent, "height_m", 10.0, kit_id)?;
            positive_number(intent, "depth_m", 10.0, kit_id)?;
            let slot_count = integer(intent, "slot_count", 1, 32, kit_id)?;
            let slot_width = positive_number(intent, "slot_width_m", 10.0, kit_id)?;
            let slot_spacing = nonnegative_number(intent, "slot_spacing_m", 10.0, kit_id)?;
            if slot_width * slot_count as f64 + slot_spacing * slot_count.saturating_sub(1) as f64
                > width
            {
                return Err(format!(
                    "FICTIONAL_WEAPON_PROFILE_INVALID: {kit_id} slots exceed width_m"
                ));
            }
            vector3(intent, "position_m", kit_id)?;
            vector3(intent, "rotation_rad", kit_id)?;
        }
        "forgecad.kit.joint@1" => {
            exact_keys(
                intent,
                &[
                    "radius_m",
                    "depth_m",
                    "ring_count",
                    "ring_spacing_m",
                    "radial_segments",
                    "position_m",
                    "rotation_rad",
                ],
                kit_id,
            )?;
            positive_number(intent, "radius_m", 5.0, kit_id)?;
            let depth = positive_number(intent, "depth_m", 10.0, kit_id)?;
            let ring_count = integer(intent, "ring_count", 1, 16, kit_id)?;
            let ring_spacing = nonnegative_number(intent, "ring_spacing_m", 10.0, kit_id)?;
            integer(intent, "radial_segments", 8, 64, kit_id)?;
            if ring_spacing * ring_count.saturating_sub(1) as f64 > depth {
                return Err(format!(
                    "FICTIONAL_WEAPON_PROFILE_INVALID: {kit_id} rings exceed depth_m"
                ));
            }
            vector3(intent, "position_m", kit_id)?;
            vector3(intent, "rotation_rad", kit_id)?;
        }
        "forgecad.kit.sensor@1" => {
            exact_keys(
                intent,
                &[
                    "radius_m",
                    "height_m",
                    "radial_segments",
                    "position_m",
                    "rotation_rad",
                ],
                kit_id,
            )?;
            positive_number(intent, "radius_m", 5.0, kit_id)?;
            positive_number(intent, "height_m", 10.0, kit_id)?;
            integer(intent, "radial_segments", 8, 64, kit_id)?;
            vector3(intent, "position_m", kit_id)?;
            vector3(intent, "rotation_rad", kit_id)?;
        }
        _ => {
            return Err(format!(
                "FICTIONAL_WEAPON_PROFILE_INVALID: unsupported kit_id {kit_id}"
            ))
        }
    }
    Ok(())
}

fn operator_for_kit(kit_id: &str) -> Result<&'static str, String> {
    match kit_id {
        "forgecad.kit.housing@1" | "forgecad.kit.panel@1" | "forgecad.kit.frame@1" => {
            Ok("forgecad.geometry.panel@1")
        }
        "forgecad.kit.vent@1" => Ok("forgecad.geometry.vent-array@1"),
        "forgecad.kit.joint@1" => Ok("forgecad.geometry.joint-stack@1"),
        "forgecad.kit.sensor@1" => Ok("forgecad.geometry.primitive@2"),
        _ => Err(format!(
            "FICTIONAL_WEAPON_PROFILE_INVALID: unsupported kit_id {kit_id}"
        )),
    }
}

fn exact_object<'a>(
    value: &'a Value,
    required: &[&str],
    context: &str,
) -> Result<&'a Map<String, Value>, String> {
    let object = value
        .as_object()
        .ok_or_else(|| format!("FICTIONAL_WEAPON_PROFILE_INVALID: {context} must be an object"))?;
    if object.len() != required.len()
        || required.iter().any(|key| !object.contains_key(*key))
        || object.keys().any(|key| !required.contains(&key.as_str()))
    {
        return Err(format!(
            "FICTIONAL_WEAPON_PROFILE_INVALID: {context} has an unexpected field set"
        ));
    }
    Ok(object)
}

fn exact_keys(object: &Map<String, Value>, required: &[&str], context: &str) -> Result<(), String> {
    if object.len() != required.len()
        || required.iter().any(|key| !object.contains_key(*key))
        || object.keys().any(|key| !required.contains(&key.as_str()))
    {
        return Err(format!(
            "FICTIONAL_WEAPON_PROFILE_INVALID: {context} intent has an unexpected field set"
        ));
    }
    Ok(())
}

fn string<'a>(object: &'a Map<String, Value>, key: &str) -> Result<&'a str, String> {
    object
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("FICTIONAL_WEAPON_PROFILE_INVALID: {key} must be a string"))
}

fn id<'a>(object: &'a Map<String, Value>, key: &str) -> Result<&'a str, String> {
    let value = string(object, key)?;
    if !is_opaque_id(value) {
        return Err(format!(
            "FICTIONAL_WEAPON_PROFILE_INVALID: {key} must be an opaque id"
        ));
    }
    Ok(value)
}

fn sha<'a>(object: &'a Map<String, Value>, key: &str) -> Result<&'a str, String> {
    let value = string(object, key)?;
    if !is_sha256(value) {
        return Err(format!(
            "FICTIONAL_WEAPON_PROFILE_INVALID: {key} must be SHA-256"
        ));
    }
    Ok(value)
}

fn material(object: &Map<String, Value>, key: &str) -> Result<(), String> {
    let value = string(object, key)?;
    if !MATERIALS.contains(&value) {
        return Err(format!(
            "FICTIONAL_WEAPON_PROFILE_INVALID: {key} is not in the offline AssetPack"
        ));
    }
    Ok(())
}

fn enum_value(object: &Map<String, Value>, key: &str, allowed: &[&str]) -> Result<(), String> {
    let value = string(object, key)?;
    if !allowed.contains(&value) {
        return Err(format!(
            "FICTIONAL_WEAPON_PROFILE_INVALID: {key} is unsupported"
        ));
    }
    Ok(())
}

fn vector3(object: &Map<String, Value>, key: &str, context: &str) -> Result<[f64; 3], String> {
    let values = object.get(key).and_then(Value::as_array).ok_or_else(|| {
        format!("FICTIONAL_WEAPON_PROFILE_INVALID: {context}.{key} must be a vector3")
    })?;
    if values.len() != 3 {
        return Err(format!(
            "FICTIONAL_WEAPON_PROFILE_INVALID: {context}.{key} must contain exactly three values"
        ));
    }
    let mut result = [0.0; 3];
    for (index, value) in values.iter().enumerate() {
        let number = value.as_f64().ok_or_else(|| {
            format!("FICTIONAL_WEAPON_PROFILE_INVALID: {context}.{key} contains a non-number")
        })?;
        if !number.is_finite() || number.abs() > 10.0 {
            return Err(format!(
                "FICTIONAL_WEAPON_PROFILE_INVALID: {context}.{key} is out of bounded envelope"
            ));
        }
        result[index] = number;
    }
    Ok(result)
}

fn positive_number(
    object: &Map<String, Value>,
    key: &str,
    maximum: f64,
    context: &str,
) -> Result<f64, String> {
    let value = object.get(key).and_then(Value::as_f64).ok_or_else(|| {
        format!("FICTIONAL_WEAPON_PROFILE_INVALID: {context}.{key} must be a number")
    })?;
    if !value.is_finite() || value <= 0.0 || value > maximum {
        return Err(format!(
            "FICTIONAL_WEAPON_PROFILE_INVALID: {context}.{key} is out of bounds"
        ));
    }
    Ok(value)
}

fn nonnegative_number(
    object: &Map<String, Value>,
    key: &str,
    maximum: f64,
    context: &str,
) -> Result<f64, String> {
    let value = object.get(key).and_then(Value::as_f64).ok_or_else(|| {
        format!("FICTIONAL_WEAPON_PROFILE_INVALID: {context}.{key} must be a number")
    })?;
    if !value.is_finite() || value < 0.0 || value > maximum {
        return Err(format!(
            "FICTIONAL_WEAPON_PROFILE_INVALID: {context}.{key} is out of bounds"
        ));
    }
    Ok(value)
}

fn integer(
    object: &Map<String, Value>,
    key: &str,
    minimum: u64,
    maximum: u64,
    context: &str,
) -> Result<u64, String> {
    let value = object.get(key).and_then(Value::as_u64).ok_or_else(|| {
        format!("FICTIONAL_WEAPON_PROFILE_INVALID: {context}.{key} must be an integer")
    })?;
    if !(minimum..=maximum).contains(&value) {
        return Err(format!(
            "FICTIONAL_WEAPON_PROFILE_INVALID: {context}.{key} is out of bounds"
        ));
    }
    Ok(value)
}

fn verify_canonical_hash(value: &Value, context: &str) -> Result<(), String> {
    let actual = value
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .filter(|hash| is_sha256(hash))
        .ok_or_else(|| {
            format!("FICTIONAL_WEAPON_PROFILE_INVALID: {context}.canonical_sha256 is invalid")
        })?;
    let mut input = value.clone();
    input["canonical_sha256"] = Value::String(String::new());
    if canonical_json_hash(&input) != actual {
        return Err(format!(
            "FICTIONAL_WEAPON_PROFILE_CANONICAL_HASH_MISMATCH: {context}"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile_fixture() -> Value {
        let mut profile = json!({
            "schema_version": PROFILE_SCHEMA,
            "profile_id": "fictional-energy-rifle-v1",
            "project_id": "fictional-energy-rifle-project",
            "scope": PROFILE_SCOPE,
            "nonfunctional_asset": true,
            "subject_coordinate_frame_sha256": "b".repeat(64),
            "operator_catalog_sha256": "c".repeat(64),
            "representation_plan_sha256": "d".repeat(64),
            "style_language": {
                "silhouette": "long-forward",
                "surface_language": "layered-hard-surface",
                "shell_material_id": "white-dielectric-clearcoat",
                "dark_material_id": "black-anodized-metal",
                "accent_material_id": "warm-orange-emissive",
                "accent_placement": "receiver-core-and-channel",
                "detail_density": "secondary"
            },
            "reference_policy": {
                "reference_state": "original-brief",
                "unseen_regions": ["rear-internals", "internal-energy-path"],
                "hq_360_status": HQ_360_BLOCKED,
                "quality_claim_allowed": false,
                "visual_match_claim_allowed": false,
                "requires_user_approval_before_geometry_prepare": true
            },
            "macro_intents": [
                {
                    "part_id": "receiver-shell",
                    "kit_id": "forgecad.kit.housing@1",
                    "operator_id": "forgecad.geometry.panel@1",
                    "material_zone_id": "white-dielectric-clearcoat",
                    "stage": "primary-form",
                    "visibility": "inferred",
                    "symmetry": "mirror-left-right",
                    "intent": {"size_m":[1.4,0.48,0.42],"thickness_m":0.08,"bevel_m":0.04,"position_m":[0.0,0.0,0.0],"rotation_rad":[0.0,0.0,0.0]}
                },
                {
                    "part_id": "forward-shroud",
                    "kit_id": "forgecad.kit.panel@1",
                    "operator_id": "forgecad.geometry.panel@1",
                    "material_zone_id": "dark-painted-metal",
                    "stage": "primary-form",
                    "visibility": "inferred",
                    "symmetry": "mirror-left-right",
                    "intent": {"size_m":[2.0,0.3,0.3],"thickness_m":0.06,"bevel_m":0.025,"position_m":[1.5,0.08,0.0],"rotation_rad":[0.0,0.0,0.0]}
                },
                {
                    "part_id": "energy-core",
                    "kit_id": "forgecad.kit.sensor@1",
                    "operator_id": "forgecad.geometry.primitive@2",
                    "material_zone_id": "warm-orange-emissive",
                    "stage": "secondary-structure",
                    "visibility": "inferred",
                    "symmetry": "independent",
                    "intent": {"radius_m":0.18,"height_m":0.12,"radial_segments":24,"position_m":[0.15,0.25,0.0],"rotation_rad":[0.0,0.0,0.0]}
                },
                {
                    "part_id": "vent-line",
                    "kit_id": "forgecad.kit.vent@1",
                    "operator_id": "forgecad.geometry.vent-array@1",
                    "material_zone_id": "black-anodized-metal",
                    "stage": "tertiary-detail",
                    "visibility": "inferred",
                    "symmetry": "mirror-left-right",
                    "intent": {"width_m":0.6,"height_m":0.08,"depth_m":0.04,"slot_count":4,"slot_width_m":0.08,"slot_spacing_m":0.04,"position_m":[0.8,0.0,0.0],"rotation_rad":[0.0,0.0,0.0]}
                }
            ],
            "quality_contract": {
                "strict_glb_readback": true,
                "joint_multiview_compare": true,
                "pbr_after_silhouette_gate": true,
                "human_review_required": true,
                "confirm_export_requires_user": true,
                "max_primary_form_rounds": 5,
                "max_macro_requests": 12
            },
            "canonical_sha256": ""
        });
        profile["canonical_sha256"] = Value::String(canonical_json_hash(&profile));
        profile
    }

    #[test]
    fn fictional_profile_expands_deterministically_to_bounded_pdk_requests() {
        let profile = profile_fixture();
        validate_profile(&profile).expect("fixture must pass");
        let plan = expand_profile(&profile).expect("profile expansion");
        validate_plan(&plan).expect("plan must pass");
        assert_eq!(plan["schema_version"], PLAN_SCHEMA);
        assert_eq!(plan["quality_status"], "structural_only");
        assert_eq!(plan["candidate_created"], false);
        assert_eq!(plan["runtime_write_performed"], false);
        assert_eq!(plan["hq_360_status"], HQ_360_BLOCKED);
        assert_eq!(plan["macro_requests"].as_array().unwrap().len(), 4);
        assert_eq!(
            plan["macro_requests"][0]["schema_version"],
            PDK_REQUEST_SCHEMA
        );
        assert!(is_sha256(
            plan["macro_requests"][0]["input_sha256"].as_str().unwrap()
        ));
        assert_eq!(plan, expand_profile(&profile).expect("repeat expansion"));
    }

    #[test]
    fn fictional_profile_rejects_functional_scope_and_operator_drift() {
        let profile = profile_fixture();
        let mut functional = profile.clone();
        functional["nonfunctional_asset"] = Value::Bool(false);
        functional["canonical_sha256"] = Value::String(String::new());
        functional["canonical_sha256"] = Value::String(canonical_json_hash(&functional));
        let error = validate_profile(&functional).expect_err("functional scope must fail closed");
        assert!(error.contains("nonfunctional asset scope"));

        let mut operator_drift = profile;
        operator_drift["macro_intents"][0]["operator_id"] = json!("forgecad.geometry.vent-array@1");
        operator_drift["canonical_sha256"] = Value::String(String::new());
        operator_drift["canonical_sha256"] = Value::String(canonical_json_hash(&operator_drift));
        let error = validate_profile(&operator_drift).expect_err("operator drift must fail closed");
        assert!(error.contains("does not match kit_id"), "{error}");
    }

    #[test]
    fn fictional_profile_rejects_unbounded_relationship_and_canonical_drift() {
        let profile = profile_fixture();
        let mut relation = profile.clone();
        relation["macro_intents"][3]["intent"]["slot_width_m"] = json!(0.2);
        relation["canonical_sha256"] = Value::String(String::new());
        relation["canonical_sha256"] = Value::String(canonical_json_hash(&relation));
        let error = validate_profile(&relation).expect_err("vent relation must fail closed");
        assert!(error.contains("slots exceed width_m"), "{error}");

        let mut canonical = profile;
        canonical["profile_id"] = json!("changed-after-hash");
        let error = validate_profile(&canonical).expect_err("canonical drift must fail closed");
        assert!(error.contains("CANONICAL_HASH_MISMATCH"));
    }
}
