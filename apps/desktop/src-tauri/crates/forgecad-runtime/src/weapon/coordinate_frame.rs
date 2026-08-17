use forgecad_core::canonical_json_hash;
use serde_json::Value;

pub(crate) const WEAPON_FRAME_SCHEMA: &str = "SubjectCoordinateFrame@1";
pub(crate) const WEAPON_FRAME_ID: &str = "weapon-right-handed-x-muzzle-y-up-z-right";
pub(crate) const WEAPON_VIEW_KINDS: [&str; 8] = [
    "left",
    "right",
    "top",
    "bottom",
    "front",
    "back",
    "front-three-quarter",
    "rear-three-quarter",
];
pub(crate) const WEAPON_ORTHOGRAPHIC_VIEW_KINDS: [&str; 6] =
    ["left", "right", "top", "bottom", "front", "back"];

pub(crate) fn is_weapon_view_kind(kind: &str) -> bool {
    WEAPON_VIEW_KINDS.contains(&kind)
}

pub(crate) fn is_orthographic_view_kind(kind: &str) -> bool {
    WEAPON_ORTHOGRAPHIC_VIEW_KINDS.contains(&kind)
}

pub(crate) fn standard_frame() -> Value {
    let mut frame = serde_json::json!({
        "schema_version": WEAPON_FRAME_SCHEMA,
        "frame_id": WEAPON_FRAME_ID,
        "handedness": "right-handed",
        "units": "meter",
        "up_axis": "+Y",
        "forward_axis": "+X",
        "side_axis": "+Z",
        "semantic_axes": {
            "muzzle": "+X",
            "stock": "-X",
            "top": "+Y",
            "bottom": "-Y",
            "right": "+Z",
            "left": "-Z"
        },
        "canonical_sha256": ""
    });
    frame["canonical_sha256"] = Value::String(canonical_json_hash(&frame));
    frame
}

pub(crate) fn validate_frame(value: &Value) -> Result<(), String> {
    let object = value
        .as_object()
        .ok_or_else(|| "SUBJECT_COORDINATE_FRAME_INVALID: object required".to_owned())?;
    const KEYS: [&str; 9] = [
        "schema_version",
        "frame_id",
        "handedness",
        "units",
        "up_axis",
        "forward_axis",
        "side_axis",
        "semantic_axes",
        "canonical_sha256",
    ];
    if object.len() != KEYS.len() || object.keys().any(|key| !KEYS.contains(&key.as_str())) {
        return Err("SUBJECT_COORDINATE_FRAME_INVALID: unknown field".to_owned());
    }
    if object.get("schema_version").and_then(Value::as_str) != Some(WEAPON_FRAME_SCHEMA)
        || object.get("frame_id").and_then(Value::as_str) != Some(WEAPON_FRAME_ID)
        || object.get("handedness").and_then(Value::as_str) != Some("right-handed")
        || object.get("units").and_then(Value::as_str) != Some("meter")
        || object.get("up_axis").and_then(Value::as_str) != Some("+Y")
        || object.get("forward_axis").and_then(Value::as_str) != Some("+X")
        || object.get("side_axis").and_then(Value::as_str) != Some("+Z")
    {
        return Err("SUBJECT_COORDINATE_FRAME_INVALID: axis definition".to_owned());
    }
    let axes = object
        .get("semantic_axes")
        .and_then(Value::as_object)
        .ok_or_else(|| "SUBJECT_COORDINATE_FRAME_INVALID: semantic axes".to_owned())?;
    let expected = [
        ("muzzle", "+X"),
        ("stock", "-X"),
        ("top", "+Y"),
        ("bottom", "-Y"),
        ("right", "+Z"),
        ("left", "-Z"),
    ];
    if axes.len() != expected.len()
        || expected
            .iter()
            .any(|(key, axis)| axes.get(*key).and_then(Value::as_str) != Some(*axis))
    {
        return Err("SUBJECT_COORDINATE_FRAME_INVALID: semantic axis mapping".to_owned());
    }
    let canonical = object
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| "SUBJECT_COORDINATE_FRAME_INVALID: canonical hash".to_owned())?;
    if !forgecad_contracts::is_sha256(canonical) {
        return Err("SUBJECT_COORDINATE_FRAME_INVALID: canonical hash".to_owned());
    }
    let mut input = value.clone();
    input["canonical_sha256"] = Value::String(String::new());
    if canonical_json_hash(&input) != canonical {
        return Err("SUBJECT_COORDINATE_FRAME_CANONICAL_HASH_MISMATCH".to_owned());
    }
    Ok(())
}
