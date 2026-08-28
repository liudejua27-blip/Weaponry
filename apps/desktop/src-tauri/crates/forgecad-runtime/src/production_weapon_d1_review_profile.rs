use crate::{canonical_json_hash, sha256_hex, Runtime, RuntimeError};
use serde_json::{json, Value};
use std::collections::HashSet;

const PROFILE_BYTES: &[u8] =
    include_bytes!("../assets/production-weapon-d1-reviewed-form-profile-v1.json");
const PROFILE_SCHEMA_VERSION: &str = "ProductionWeaponD1ReviewedFormProfile@1";
const PROFILE_ID: &str = "fps-form-04ay-d1-reviewed-form";
const PROFILE_REVISION: &str = "production-weapon-d1-reviewed-form-profile-v1";
const REFERENCE_SHA256: &str = "1964704a62ed7a841b4d49c370b8d46f4626e201daad29092a9c39a40b4c4109";
const CONFIRMATION_FILE_SHA256: &str =
    "5f01e6ed039f7870f0f285092995c2efc6b83895742a1b848bd00e21c9d21c37";
const EXPECTED_REAR_THREE_QUARTER_CAMERA_HASH: &str =
    "9d8e590e940967474213180edc714cfc279d88f3b06367d0817e1855205b3abb";
const VIEW_ORDER: [&str; 6] = [
    "front",
    "back",
    "left",
    "right",
    "top",
    "rear-three-quarter",
];
const NEGATIVE_SPACE_IDS: [&str; 6] = [
    "left.trigger-void",
    "left.open-stock-void",
    "right.open-stock-void",
    "right.trigger-void",
    "rear3q.open-stock-void",
    "rear3q.trigger-void",
];

fn required_array<'a>(value: &'a Value, key: &str) -> Result<&'a Vec<Value>, RuntimeError> {
    value.get(key).and_then(Value::as_array).ok_or_else(|| {
        RuntimeError::InvalidInput(format!("PRODUCTION_WEAPON_D1_REVIEW_PROFILE_{key}_INVALID"))
    })
}

fn validate_points(value: &Value, minimum: usize, label: &str) -> Result<(), RuntimeError> {
    let points = value
        .as_array()
        .filter(|rows| rows.len() >= minimum)
        .ok_or_else(|| {
            RuntimeError::InvalidInput(format!(
                "PRODUCTION_WEAPON_D1_REVIEW_PROFILE_{label}_INVALID"
            ))
        })?;
    if points.iter().any(|point| {
        point.as_array().is_none_or(|coordinates| {
            coordinates.len() != 2
                || coordinates.iter().any(|coordinate| {
                    coordinate
                        .as_f64()
                        .is_none_or(|number| !number.is_finite() || !(0.0..=1.0).contains(&number))
                })
        })
    }) {
        return Err(RuntimeError::InvalidInput(format!(
            "PRODUCTION_WEAPON_D1_REVIEW_PROFILE_{label}_OUT_OF_BOUNDS"
        )));
    }
    Ok(())
}

pub(crate) fn materialize() -> Result<Value, RuntimeError> {
    let profile: Value = serde_json::from_slice(PROFILE_BYTES).map_err(|error| {
        RuntimeError::InvalidInput(format!(
            "PRODUCTION_WEAPON_D1_REVIEW_PROFILE_EMBEDDED_JSON_INVALID: {error}"
        ))
    })?;
    if profile["schema_version"] != PROFILE_SCHEMA_VERSION
        || profile["profile_id"] != PROFILE_ID
        || profile["profile_revision"] != PROFILE_REVISION
        || profile["reference_sha256"] != REFERENCE_SHA256
        || profile["reference_dimensions"] != json!({"width":1491,"height":1055})
        || profile["view_order"] != json!(VIEW_ORDER)
        || profile["approval_binding"]["confirmation_file_sha256"] != CONFIRMATION_FILE_SHA256
        || profile["approval_binding"]["accepted_line_flow_count"] != 25
        || profile["approval_binding"]["rear_three_quarter_rotation_degrees"] != 0
        || profile["approval_binding"]["rear_three_quarter_screen_order"]
            != "stock-left-muzzle-right"
        || profile["approval_binding"]["rear_three_quarter_upright"] != true
        || profile["approval_binding"]["runtime_camera_orbit_degrees"] != 180
        || profile["approval_binding"]["expected_runtime_camera_hash"]
            != EXPECTED_REAR_THREE_QUARTER_CAMERA_HASH
        || profile["depth_status"] != "UNKNOWN"
        || profile["quality_status"] != "NOT_PROVEN"
        || profile["promotion_eligible"] != false
        || profile["candidate_confirmed"] != false
        || profile["version_created"] != false
        || profile["export_performed"] != false
    {
        return Err(RuntimeError::InvalidInput(
            "PRODUCTION_WEAPON_D1_REVIEW_PROFILE_BINDING_MISMATCH".to_owned(),
        ));
    }

    let mut canonical = profile.clone();
    canonical["canonical_sha256"] = Value::String(String::new());
    if profile["canonical_sha256"].as_str() != Some(canonical_json_hash(&canonical).as_str()) {
        return Err(RuntimeError::InvalidInput(
            "PRODUCTION_WEAPON_D1_REVIEW_PROFILE_CANONICAL_MISMATCH".to_owned(),
        ));
    }

    let views = required_array(&profile, "views")?;
    if views.len() != VIEW_ORDER.len() {
        return Err(RuntimeError::InvalidInput(
            "PRODUCTION_WEAPON_D1_REVIEW_PROFILE_VIEW_COUNT_INVALID".to_owned(),
        ));
    }
    let mut line_flow_ids = HashSet::new();
    let mut negative_space_ids = Vec::new();
    for (index, view) in views.iter().enumerate() {
        if view["view_kind"] != VIEW_ORDER[index] {
            return Err(RuntimeError::InvalidInput(
                "PRODUCTION_WEAPON_D1_REVIEW_PROFILE_VIEW_ORDER_INVALID".to_owned(),
            ));
        }
        let outer = view.get("outer_contour_points").ok_or_else(|| {
            RuntimeError::InvalidInput(
                "PRODUCTION_WEAPON_D1_REVIEW_PROFILE_OUTER_CONTOUR_MISSING".to_owned(),
            )
        })?;
        validate_points(outer, 192, "OUTER_CONTOUR")?;
        if outer.as_array().map(Vec::len) != Some(192) {
            return Err(RuntimeError::InvalidInput(
                "PRODUCTION_WEAPON_D1_REVIEW_PROFILE_OUTER_CONTOUR_COUNT_INVALID".to_owned(),
            ));
        }
        for flow in required_array(view, "line_flows_v2")? {
            let flow_id = flow["line_flow_id"].as_str().ok_or_else(|| {
                RuntimeError::InvalidInput(
                    "PRODUCTION_WEAPON_D1_REVIEW_PROFILE_LINE_FLOW_ID_INVALID".to_owned(),
                )
            })?;
            if !line_flow_ids.insert(flow_id.to_owned())
                || !matches!(
                    flow["runtime_kind_candidate"].as_str(),
                    Some("ridge" | "seam" | "occlusion-edge" | "light-channel")
                )
                || flow["continuity_group_id"] != format!("lineflow.{flow_id}")
            {
                return Err(RuntimeError::InvalidInput(
                    "PRODUCTION_WEAPON_D1_REVIEW_PROFILE_LINE_FLOW_BINDING_INVALID".to_owned(),
                ));
            }
            validate_points(&flow["points"], 2, "LINE_FLOW_POINTS")?;
        }
        for region in view
            .pointer("/negative_space_v2/regions")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                RuntimeError::InvalidInput(
                    "PRODUCTION_WEAPON_D1_REVIEW_PROFILE_NEGATIVE_SPACE_INVALID".to_owned(),
                )
            })?
        {
            let structure_id = region["structure_id"].as_str().ok_or_else(|| {
                RuntimeError::InvalidInput(
                    "PRODUCTION_WEAPON_D1_REVIEW_PROFILE_NEGATIVE_SPACE_ID_INVALID".to_owned(),
                )
            })?;
            negative_space_ids.push(structure_id.to_owned());
            validate_points(&region["closed_contour_points"], 3, "NEGATIVE_SPACE_POINTS")?;
        }
    }
    if line_flow_ids.len() != 25
        || negative_space_ids
            != NEGATIVE_SPACE_IDS
                .iter()
                .map(|value| (*value).to_owned())
                .collect::<Vec<_>>()
    {
        return Err(RuntimeError::InvalidInput(
            "PRODUCTION_WEAPON_D1_REVIEW_PROFILE_CONTENT_COUNT_INVALID".to_owned(),
        ));
    }
    Ok(profile)
}

pub(crate) fn manifest() -> Result<Value, RuntimeError> {
    let profile = materialize()?;
    let mut value = json!({
        "schema_version":"ProductionWeaponD1ReviewedFormProfileManifest@1",
        "profile_id":PROFILE_ID,
        "profile_revision":PROFILE_REVISION,
        "profile_bytes_sha256":sha256_hex(PROFILE_BYTES),
        "profile_canonical_sha256":profile["canonical_sha256"],
        "reference_sha256":REFERENCE_SHA256,
        "view_count":6,
        "outer_contour_point_count":1152,
        "line_flow_count":25,
        "negative_space_count":6,
        "depth_status":"UNKNOWN",
        "quality_status":"NOT_PROVEN",
        "promotion_eligible":false,
        "candidate_confirmed":false,
        "version_created":false,
        "export_performed":false,
        "canonical_sha256":""
    });
    value["canonical_sha256"] = Value::String(canonical_json_hash(&value));
    Ok(value)
}

impl Runtime {
    /// Read-only identity for the closed D1 form-review input. This records
    /// approved reference annotations only; it is not candidate evidence and
    /// cannot advance a production stage by itself.
    pub fn production_weapon_d1_review_profile_manifest(&self) -> Result<Value, RuntimeError> {
        manifest()
    }
}
