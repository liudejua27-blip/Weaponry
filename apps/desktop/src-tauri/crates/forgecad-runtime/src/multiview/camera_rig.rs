use forgecad_core::canonical_json_hash;
use serde_json::{json, Value};

use crate::weapon::coordinate_frame::{is_weapon_view_kind, WEAPON_ORTHOGRAPHIC_VIEW_KINDS};

pub(crate) const CAMERA_V2_SCHEMA: &str = "CameraCalibration@2";
pub(crate) const CAMERA_RIG_SCHEMA: &str = "CameraRigCalibration@1";
const RENDERER_REVISION: &str = "forgecad-renderer-2";

fn finite_vec3(value: Option<&Value>) -> bool {
    value.and_then(Value::as_array).is_some_and(|values| {
        values.len() == 3
            && values
                .iter()
                .all(|value| value.as_f64().is_some_and(f64::is_finite))
    })
}

pub(crate) fn camera_v2_hashes(mut camera: Value) -> Value {
    camera["camera_hash"] = Value::String(String::new());
    camera["canonical_sha256"] = Value::String(String::new());
    camera["camera_hash"] = Value::String(canonical_json_hash(&camera));
    camera["canonical_sha256"] = Value::String(canonical_json_hash(&camera));
    camera
}

pub(crate) fn validate_camera_calibration_v2(value: &Value) -> Result<(), String> {
    let object = value
        .as_object()
        .ok_or_else(|| "CAMERA_CALIBRATION_V2_INVALID: object required".to_owned())?;
    const KEYS: [&str; 11] = [
        "schema_version",
        "camera_hash",
        "projection",
        "transform",
        "fov_y_degrees",
        "ortho_scale",
        "near_m",
        "far_m",
        "resolution",
        "coordinate_system",
        "renderer_revision",
    ];
    if object.len() != KEYS.len() + 1
        || object
            .keys()
            .any(|key| !KEYS.contains(&key.as_str()) && key.as_str() != "canonical_sha256")
    {
        return Err("CAMERA_CALIBRATION_V2_INVALID: unknown field".to_owned());
    }
    if object.get("schema_version").and_then(Value::as_str) != Some(CAMERA_V2_SCHEMA)
        || object.get("coordinate_system").and_then(Value::as_str)
            != Some("right-handed-y-up-meter")
        || object
            .get("resolution")
            .and_then(|value| value.get("width"))
            .and_then(Value::as_u64)
            != Some(512)
        || object
            .get("resolution")
            .and_then(|value| value.get("height"))
            .and_then(Value::as_u64)
            != Some(512)
        || object.get("renderer_revision").and_then(Value::as_str) != Some(RENDERER_REVISION)
        || !finite_vec3(value.pointer("/transform/position_m"))
        || !finite_vec3(value.pointer("/transform/target_m"))
        || !finite_vec3(value.pointer("/transform/up"))
    {
        return Err("CAMERA_CALIBRATION_V2_INVALID: fixed transform contract".to_owned());
    }
    let transform = object
        .get("transform")
        .and_then(Value::as_object)
        .ok_or_else(|| "CAMERA_CALIBRATION_V2_INVALID: transform".to_owned())?;
    if transform.len() != 3
        || transform
            .keys()
            .any(|key| !["position_m", "target_m", "up"].contains(&key.as_str()))
    {
        return Err("CAMERA_CALIBRATION_V2_INVALID: transform fields".to_owned());
    }
    let resolution = object
        .get("resolution")
        .and_then(Value::as_object)
        .ok_or_else(|| "CAMERA_CALIBRATION_V2_INVALID: resolution".to_owned())?;
    if resolution.len() != 2
        || resolution
            .keys()
            .any(|key| !["width", "height"].contains(&key.as_str()))
    {
        return Err("CAMERA_CALIBRATION_V2_INVALID: resolution fields".to_owned());
    }
    let projection = object
        .get("projection")
        .and_then(Value::as_str)
        .ok_or_else(|| "CAMERA_CALIBRATION_V2_INVALID: projection".to_owned())?;
    let fov = object.get("fov_y_degrees");
    let ortho = object.get("ortho_scale");
    match projection {
        "perspective" => {
            if !fov
                .and_then(Value::as_f64)
                .is_some_and(|value| value.is_finite() && (1.0..179.0).contains(&value))
                || !ortho.is_some_and(Value::is_null)
            {
                return Err("CAMERA_CALIBRATION_V2_INVALID: perspective projection".to_owned());
            }
        }
        "orthographic" => {
            if !ortho
                .and_then(Value::as_f64)
                .is_some_and(|value| value.is_finite() && (0.001..=100.0).contains(&value))
                || !fov.is_some_and(Value::is_null)
            {
                return Err("CAMERA_CALIBRATION_V2_INVALID: orthographic projection".to_owned());
            }
        }
        _ => return Err("CAMERA_CALIBRATION_V2_INVALID: projection".to_owned()),
    }
    let near = object.get("near_m").and_then(Value::as_f64).unwrap_or(0.0);
    let far = object.get("far_m").and_then(Value::as_f64).unwrap_or(0.0);
    if !near.is_finite() || !far.is_finite() || near <= 0.0 || far <= near {
        return Err("CAMERA_CALIBRATION_V2_INVALID: clipping range".to_owned());
    }
    for key in ["camera_hash", "canonical_sha256"] {
        if !object
            .get(key)
            .and_then(Value::as_str)
            .is_some_and(forgecad_contracts::is_sha256)
        {
            return Err(format!("CAMERA_CALIBRATION_V2_INVALID: {key}"));
        }
    }
    let mut identity = value.clone();
    identity["camera_hash"] = Value::String(String::new());
    identity["canonical_sha256"] = Value::String(String::new());
    if canonical_json_hash(&identity) != object["camera_hash"] {
        return Err("CAMERA_CALIBRATION_V2_CAMERA_HASH_MISMATCH".to_owned());
    }
    let mut canonical = value.clone();
    canonical["canonical_sha256"] = Value::String(String::new());
    if canonical_json_hash(&canonical) != object["canonical_sha256"] {
        return Err("CAMERA_CALIBRATION_V2_CANONICAL_HASH_MISMATCH".to_owned());
    }
    Ok(())
}

pub(crate) fn validate_camera_rig(
    value: &Value,
    project_id: &str,
    candidate_id: &str,
) -> Result<(), String> {
    let object = value
        .as_object()
        .ok_or_else(|| "CAMERA_RIG_INVALID: object required".to_owned())?;
    let required = [
        "schema_version",
        "rig_id",
        "project_id",
        "candidate_id",
        "subject_coordinate_frame",
        "origin_m",
        "object_scale_m",
        "renderer_revision",
        "views",
        "canonical_sha256",
    ];
    if object.len() != required.len() || object.keys().any(|key| !required.contains(&key.as_str()))
    {
        return Err("CAMERA_RIG_INVALID: unknown field".to_owned());
    }
    if object.get("schema_version").and_then(Value::as_str) != Some(CAMERA_RIG_SCHEMA)
        || object.get("project_id").and_then(Value::as_str) != Some(project_id)
        || object.get("candidate_id").and_then(Value::as_str) != Some(candidate_id)
        || object
            .get("object_scale_m")
            .and_then(Value::as_f64)
            .is_none_or(|value| !value.is_finite() || value <= 0.0 || value > 100.0)
        || object.get("renderer_revision").and_then(Value::as_str) != Some(RENDERER_REVISION)
        || !object
            .get("origin_m")
            .and_then(Value::as_array)
            .is_some_and(|values| {
                values.len() == 3
                    && values
                        .iter()
                        .all(|v| v.as_f64().is_some_and(f64::is_finite))
            })
    {
        return Err("CAMERA_RIG_INVALID: binding or scale".to_owned());
    }
    crate::weapon::coordinate_frame::validate_frame(
        object
            .get("subject_coordinate_frame")
            .ok_or_else(|| "CAMERA_RIG_INVALID: coordinate frame".to_owned())?,
    )?;
    let views = object
        .get("views")
        .and_then(Value::as_array)
        .filter(|values| (6..=8).contains(&values.len()))
        .ok_or_else(|| "CAMERA_RIG_INVALID: views".to_owned())?;
    let mut ids = std::collections::HashSet::new();
    let mut primary = 0usize;
    let mut kinds = std::collections::HashSet::new();
    for view in views {
        let view_object = view
            .as_object()
            .ok_or_else(|| "CAMERA_RIG_INVALID: view object".to_owned())?;
        let keys = [
            "view_id",
            "kind",
            "camera",
            "camera_hash",
            "weight",
            "primary",
        ];
        if view_object.len() != keys.len()
            || view_object.keys().any(|key| !keys.contains(&key.as_str()))
        {
            return Err("CAMERA_RIG_INVALID: view fields".to_owned());
        }
        let id = view_object
            .get("view_id")
            .and_then(Value::as_str)
            .ok_or_else(|| "CAMERA_RIG_INVALID: view_id".to_owned())?;
        let kind = view_object
            .get("kind")
            .and_then(Value::as_str)
            .ok_or_else(|| "CAMERA_RIG_INVALID: kind".to_owned())?;
        if !is_weapon_view_kind(kind)
            || !ids.insert(id.to_owned())
            || !kinds.insert(kind.to_owned())
        {
            return Err("CAMERA_RIG_INVALID: duplicate or unsupported view".to_owned());
        }
        let camera = view_object
            .get("camera")
            .ok_or_else(|| "CAMERA_RIG_INVALID: camera".to_owned())?;
        validate_camera_calibration_v2(camera)?;
        if view_object.get("camera_hash") != camera.get("camera_hash") {
            return Err("CAMERA_RIG_CAMERA_BINDING_MISMATCH".to_owned());
        }
        let weight = view_object
            .get("weight")
            .and_then(Value::as_f64)
            .ok_or_else(|| "CAMERA_RIG_INVALID: weight".to_owned())?;
        if !weight.is_finite() || !(0.0..=1.0).contains(&weight) || weight == 0.0 {
            return Err("CAMERA_RIG_INVALID: weight".to_owned());
        }
        if view_object
            .get("primary")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            primary += 1;
        }
    }
    if primary != 1
        || !WEAPON_ORTHOGRAPHIC_VIEW_KINDS
            .iter()
            .all(|kind| kinds.contains(*kind))
    {
        return Err("CAMERA_RIG_INVALID: primary or orthographic coverage".to_owned());
    }
    let canonical = object
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| "CAMERA_RIG_INVALID: canonical hash".to_owned())?;
    if !forgecad_contracts::is_sha256(canonical) {
        return Err("CAMERA_RIG_INVALID: canonical hash".to_owned());
    }
    let mut input = value.clone();
    input["canonical_sha256"] = Value::String(String::new());
    if canonical_json_hash(&input) != canonical {
        return Err("CAMERA_RIG_CANONICAL_HASH_MISMATCH".to_owned());
    }
    Ok(())
}

pub(crate) fn inferred_weapon_camera(kind: &str, ortho_scale: f64) -> Result<Value, String> {
    if !is_weapon_view_kind(kind) {
        return Err("CAMERA_RIG_VIEW_KIND_INVALID".to_owned());
    }
    let (position, target, up, projection) = match kind {
        "front" => (
            [20.0, 0.0, 0.0],
            [0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            "orthographic",
        ),
        "back" => (
            [-20.0, 0.0, 0.0],
            [0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            "orthographic",
        ),
        "left" => (
            [0.0, 0.0, -20.0],
            [0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            "orthographic",
        ),
        "right" => (
            [0.0, 0.0, 20.0],
            [0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            "orthographic",
        ),
        "top" => (
            [0.0, 20.0, 0.0],
            [0.0, 0.0, 0.0],
            [0.0, 0.0, -1.0],
            "orthographic",
        ),
        "bottom" => (
            [0.0, -20.0, 0.0],
            [0.0, 0.0, 0.0],
            [0.0, 0.0, 1.0],
            "orthographic",
        ),
        "front-three-quarter" => (
            [10.0, 5.0, 10.0],
            [0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            "perspective",
        ),
        "rear-three-quarter" => (
            [-10.0, 5.0, -10.0],
            [0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            "perspective",
        ),
        _ => unreachable!(),
    };
    let mut camera = json!({
        "schema_version":CAMERA_V2_SCHEMA,
        "camera_hash":"",
        "projection":projection,
        "transform":{"position_m":position,"target_m":target,"up":up},
        "fov_y_degrees":if projection == "perspective" { Value::from(42.0) } else { Value::Null },
        "ortho_scale":if projection == "orthographic" { Value::from(ortho_scale) } else { Value::Null },
        "near_m":0.05,
        "far_m":100.0,
        "resolution":{"width":512,"height":512},
        "coordinate_system":"right-handed-y-up-meter",
        "renderer_revision":"forgecad-renderer-2",
        "canonical_sha256":""
    });
    camera = camera_v2_hashes(camera);
    validate_camera_calibration_v2(&camera)?;
    Ok(camera)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::weapon::coordinate_frame::standard_frame;

    fn rig_with_views() -> Value {
        let kinds = ["left", "right", "top", "bottom", "front", "back"];
        let views = kinds
            .iter()
            .enumerate()
            .map(|(index, kind)| {
                let camera = inferred_weapon_camera(kind, 2.4).expect("bounded weapon camera");
                let camera_hash = camera["camera_hash"].clone();
                json!({
                    "view_id":format!("weapon-{kind}"),
                    "kind":kind,
                    "camera":camera,
                    "camera_hash":camera_hash,
                    "weight":1.0,
                    "primary":index == 0
                })
            })
            .collect::<Vec<_>>();
        let mut rig = json!({
            "schema_version":CAMERA_RIG_SCHEMA,
            "rig_id":"weapon-rig",
            "project_id":"project",
            "candidate_id":"candidate",
            "subject_coordinate_frame":standard_frame(),
            "origin_m":[0.0,0.0,0.0],
            "object_scale_m":2.4,
            "renderer_revision":"forgecad-renderer-2",
            "views":views,
            "canonical_sha256":""
        });
        rig["canonical_sha256"] = Value::String(canonical_json_hash(&rig));
        rig
    }

    #[test]
    fn inferred_weapon_orthographic_camera_is_hash_stable() {
        let first = inferred_weapon_camera("left", 2.4).expect("camera");
        let second = inferred_weapon_camera("left", 2.4).expect("camera");
        assert_eq!(first, second);
        assert_eq!(first["projection"], "orthographic");
        validate_camera_calibration_v2(&first).expect("camera contract");
    }

    #[test]
    fn camera_v2_rejects_nested_unknown_fields_and_renderer_drift() {
        let mut extra = inferred_weapon_camera("left", 2.4).expect("camera");
        extra["transform"]["unexpected"] = Value::from(1);
        let error = validate_camera_calibration_v2(&extra).expect_err("nested fields are closed");
        assert!(error.contains("transform fields"));

        let mut drifted = inferred_weapon_camera("left", 2.4).expect("camera");
        drifted["renderer_revision"] = Value::String("forgecad-renderer-1".to_owned());
        let error = validate_camera_calibration_v2(&drifted).expect_err("renderer is hash-bound");
        assert!(error.contains("fixed transform contract"));
    }

    #[test]
    fn weapon_camera_rig_requires_all_orthographic_views() {
        let rig = rig_with_views();
        validate_camera_rig(&rig, "project", "candidate").expect("camera rig contract");

        let mut missing = rig;
        missing["views"].as_array_mut().expect("views").remove(0);
        missing["canonical_sha256"] = Value::String(canonical_json_hash(&missing));
        let error = validate_camera_rig(&missing, "project", "candidate")
            .expect_err("front/back/left/right/top/bottom coverage must be bounded");
        assert!(error.contains("primary or orthographic coverage") || error.contains("views"));
    }

    #[test]
    fn weapon_camera_rig_rejects_repeated_view_kinds() {
        let mut rig = rig_with_views();
        let views = rig["views"].as_array_mut().expect("views");
        for (index, view) in views.iter_mut().enumerate() {
            view["kind"] = Value::String("left".to_owned());
            view["view_id"] = Value::String(format!("duplicate-left-{index}"));
        }
        rig["canonical_sha256"] = Value::String(canonical_json_hash(&rig));
        let error = validate_camera_rig(&rig, "project", "candidate")
            .expect_err("six distinct orthographic kinds are required");
        assert!(error.contains("duplicate") || error.contains("coverage"));
    }
}
