use forgecad_core::canonical_json_hash;
use serde_json::{json, Value};
use std::collections::HashSet;

use crate::weapon::coordinate_frame::{is_weapon_view_kind, WEAPON_ORTHOGRAPHIC_VIEW_KINDS};

pub(crate) const CAMERA_V2_SCHEMA: &str = "CameraCalibration@2";
pub(crate) const CAMERA_RIG_SCHEMA: &str = "CameraRigCalibration@1";
pub(crate) const PRODUCTION_WEAPON_SUBJECT_FRAME_REGISTRATION_SCHEMA: &str =
    forgecad_contracts::PRODUCTION_WEAPON_SUBJECT_FRAME_REGISTRATION_SCHEMA_VERSION;
pub(crate) const REGISTERED_CAMERA_RIG_CALIBRATION_SCHEMA: &str =
    forgecad_contracts::REGISTERED_CAMERA_RIG_CALIBRATION_SCHEMA_VERSION;
pub(crate) const PRODUCTION_WEAPON_SEMANTIC_LANDMARK_ORDERING_SCHEMA: &str =
    forgecad_contracts::PRODUCTION_WEAPON_SEMANTIC_LANDMARK_ORDERING_SCHEMA_VERSION;
pub(crate) const PRODUCTION_WEAPON_AUTHORED_VIEW_ORIENTATION_SCHEMA: &str =
    forgecad_contracts::PRODUCTION_WEAPON_AUTHORED_VIEW_ORIENTATION_SCHEMA_VERSION;
pub(crate) const REGISTERED_CAMERA_RIG_CALIBRATION_V2_SCHEMA: &str =
    forgecad_contracts::REGISTERED_CAMERA_RIG_CALIBRATION_V2_SCHEMA_VERSION;
const RENDERER_REVISION: &str = "forgecad-renderer-2";
const PRODUCTION_WEAPON_SUBJECT_FRAME_REGISTRATION_POLICY: &str =
    forgecad_contracts::PRODUCTION_WEAPON_SUBJECT_FRAME_REGISTRATION_POLICY;
const AXIS_SIGN_EPSILON_M: f64 = 1.0e-6;

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

fn exact_program_node_position_axis(
    program: &Value,
    node_id: &str,
    axis_index: usize,
) -> Result<f64, String> {
    let nodes = program
        .get("nodes")
        .and_then(Value::as_array)
        .ok_or_else(|| "SUBJECT_FRAME_REGISTRATION_INVALID: program nodes".to_owned())?;
    let matches = nodes
        .iter()
        .filter(|node| node.get("node_id").and_then(Value::as_str) == Some(node_id))
        .collect::<Vec<_>>();
    if matches.len() != 1 {
        return Err(format!(
            "SUBJECT_FRAME_REGISTRATION_INVALID: exact node {node_id}"
        ));
    }
    matches[0]
        .pointer(&format!("/parameters/position_m/{axis_index}"))
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite())
        .ok_or_else(|| {
            format!("SUBJECT_FRAME_REGISTRATION_INVALID: node {node_id} position axis {axis_index}")
        })
}

fn require_exact_part_sources(
    program: &Value,
    part_id: &str,
    expected_sources: &[&str],
) -> Result<(), String> {
    let outputs = program
        .get("part_outputs")
        .and_then(Value::as_array)
        .ok_or_else(|| "SUBJECT_FRAME_REGISTRATION_INVALID: part outputs".to_owned())?;
    let matches = outputs
        .iter()
        .filter(|output| output.get("part_id").and_then(Value::as_str) == Some(part_id))
        .collect::<Vec<_>>();
    if matches.len() != 1 {
        return Err(format!(
            "SUBJECT_FRAME_REGISTRATION_INVALID: exact PartOutput {part_id}"
        ));
    }
    let sources = matches[0]
        .get("input_node_ids")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            format!("SUBJECT_FRAME_REGISTRATION_INVALID: PartOutput {part_id} sources")
        })?;
    if sources.len() != expected_sources.len()
        || sources
            .iter()
            .zip(expected_sources)
            .any(|(actual, expected)| actual.as_str() != Some(*expected))
    {
        return Err(format!(
            "SUBJECT_FRAME_REGISTRATION_INVALID: PartOutput {part_id} source binding"
        ));
    }
    Ok(())
}

/// Derive the closed geometry-to-subject registration for the production FPS
/// weapon from exact semantic anchor Parts.  This is a pure projection: it
/// does not rewrite GeometryProgram nodes, start a Worker, or touch Runtime
/// state.  The current D1 source predates SubjectCoordinateFrame@1 and places
/// its muzzle on -X and stock on +X, so the only admissible correction is a
/// right-handed 180-degree yaw with zero translation and unit scale.
pub(crate) fn production_weapon_subject_frame_registration(
    program: &Value,
) -> Result<Value, String> {
    if program.get("schema_version").and_then(Value::as_str) != Some("GeometryProgram@2") {
        return Err("SUBJECT_FRAME_REGISTRATION_INVALID: GeometryProgram@2 required".to_owned());
    }
    let program_sha256 = program
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .filter(|value| forgecad_contracts::is_sha256(value))
        .ok_or_else(|| "SUBJECT_FRAME_REGISTRATION_INVALID: program canonical hash".to_owned())?;
    let mut program_without_hash = program.clone();
    program_without_hash
        .as_object_mut()
        .ok_or_else(|| "SUBJECT_FRAME_REGISTRATION_INVALID: program object".to_owned())?
        .remove("canonical_sha256");
    if canonical_json_hash(&program_without_hash) != program_sha256 {
        return Err(
            "SUBJECT_FRAME_REGISTRATION_INVALID: program canonical hash mismatch".to_owned(),
        );
    }
    require_exact_part_sources(
        program,
        "rear-stock",
        &["rear-stock", "rear-stock-lower-beam"],
    )?;
    for part_id in ["muzzle-shroud", "muzzle-emitter", "muzzle-core"] {
        require_exact_part_sources(program, part_id, &[part_id])?;
    }
    for part_id in ["side-light-left", "side-light-right"] {
        require_exact_part_sources(program, part_id, &[part_id])?;
    }
    let stock_node_ids = ["rear-stock", "rear-stock-lower-beam"];
    let muzzle_node_ids = ["muzzle-shroud", "muzzle-emitter", "muzzle-core"];
    let stock_x = stock_node_ids
        .iter()
        .map(|node_id| exact_program_node_position_axis(program, node_id, 0))
        .collect::<Result<Vec<_>, _>>()?;
    let muzzle_x = muzzle_node_ids
        .iter()
        .map(|node_id| exact_program_node_position_axis(program, node_id, 0))
        .collect::<Result<Vec<_>, _>>()?;
    let side_left_z = exact_program_node_position_axis(program, "side-light-left", 2)?;
    let side_right_z = exact_program_node_position_axis(program, "side-light-right", 2)?;
    let canonical_axes = muzzle_x.iter().all(|value| *value > AXIS_SIGN_EPSILON_M)
        && stock_x.iter().all(|value| *value < -AXIS_SIGN_EPSILON_M)
        && side_left_z < -AXIS_SIGN_EPSILON_M
        && side_right_z > AXIS_SIGN_EPSILON_M;
    let inverted_axes = muzzle_x.iter().all(|value| *value < -AXIS_SIGN_EPSILON_M)
        && stock_x.iter().all(|value| *value > AXIS_SIGN_EPSILON_M)
        && side_left_z > AXIS_SIGN_EPSILON_M
        && side_right_z < -AXIS_SIGN_EPSILON_M;
    let (registration_kind, rotation_y_rad, geometry_muzzle_axis, geometry_stock_axis) =
        match (canonical_axes, inverted_axes) {
            (true, false) => ("identity", 0.0, "+X", "-X"),
            (false, true) => (
                "yaw-180-y",
                std::f64::consts::PI,
                "-X",
                "+X",
            ),
            _ => {
                return Err(
                    "SUBJECT_FRAME_REGISTRATION_BLOCKED: semantic anchors do not define one exact longitudinal axis"
                        .to_owned(),
                )
            }
        };
    let subject_frame = crate::weapon::coordinate_frame::standard_frame();
    let mut registration = json!({
        "schema_version":PRODUCTION_WEAPON_SUBJECT_FRAME_REGISTRATION_SCHEMA,
        "registration_id":"fps-weapon-geometry-to-subject-frame",
        "geometry_program_sha256":program_sha256,
        "subject_coordinate_frame_sha256":subject_frame["canonical_sha256"],
        "derivation_policy":PRODUCTION_WEAPON_SUBJECT_FRAME_REGISTRATION_POLICY,
        "geometry_semantic_axes":{
            "muzzle":geometry_muzzle_axis,
            "stock":geometry_stock_axis,
            "top":"+Y"
        },
        "subject_semantic_axes":{
            "muzzle":"+X",
            "stock":"-X",
            "top":"+Y"
        },
        "anchor_evidence":{
            "stock_node_ids":stock_node_ids,
            "stock_position_x_m":stock_x,
            "muzzle_node_ids":muzzle_node_ids,
            "muzzle_position_x_m":muzzle_x,
            "side_left_node_id":"side-light-left",
            "side_left_position_z_m":side_left_z,
            "side_right_node_id":"side-light-right",
            "side_right_position_z_m":side_right_z
        },
        "transform":{
            "direction":"geometry-to-subject",
            "kind":registration_kind,
            "rotation_rad":[0.0,rotation_y_rad,0.0],
            "translation_m":[0.0,0.0,0.0],
            "scale":[1.0,1.0,1.0]
        },
        "read_only":true,
        "geometry_program_modified":false,
        "depth_modified":false,
        "canonical_sha256":""
    });
    registration["canonical_sha256"] = Value::String(canonical_json_hash(&registration));
    Ok(registration)
}

pub(crate) fn validate_production_weapon_subject_frame_registration(
    registration: &Value,
    program: &Value,
) -> Result<(), String> {
    let expected = production_weapon_subject_frame_registration(program)?;
    if registration != &expected {
        return Err(
            "SUBJECT_FRAME_REGISTRATION_INVALID: registration differs from exact derived projection"
                .to_owned(),
        );
    }
    Ok(())
}

fn registration_transform_vec3(
    value: &Value,
    registration_kind: &str,
    context: &str,
) -> Result<Value, String> {
    let input = value
        .as_array()
        .filter(|values| values.len() == 3)
        .ok_or_else(|| format!("SUBJECT_FRAME_REGISTRATION_INVALID: {context}"))?;
    let mut result = [0.0_f64; 3];
    for (index, value) in input.iter().enumerate() {
        result[index] = value
            .as_f64()
            .filter(|value| value.is_finite())
            .ok_or_else(|| format!("SUBJECT_FRAME_REGISTRATION_INVALID: {context}"))?;
    }
    match registration_kind {
        "identity" => {}
        "yaw-180-y" => {
            result[0] = if result[0].abs() <= f64::EPSILON {
                0.0
            } else {
                -result[0]
            };
            result[2] = if result[2].abs() <= f64::EPSILON {
                0.0
            } else {
                -result[2]
            };
        }
        _ => {
            return Err(
                "SUBJECT_FRAME_REGISTRATION_INVALID: transform kind is not closed".to_owned(),
            )
        }
    }
    Ok(json!(result))
}

/// Convert a subject-frame camera into the GeometryProgram coordinate frame.
/// The registration is rigid and self-inverse for the only non-identity case
/// (180-degree Y yaw), so the fixed renderer can consume the returned camera
/// without any geometry mutation.
pub(crate) fn materialize_registered_weapon_camera(
    subject_camera: &Value,
    registration: &Value,
    program: &Value,
) -> Result<Value, String> {
    validate_camera_calibration_v2(subject_camera)?;
    validate_production_weapon_subject_frame_registration(registration, program)?;
    let registration_kind = registration
        .pointer("/transform/kind")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            "SUBJECT_FRAME_REGISTRATION_INVALID: transform kind is unavailable".to_owned()
        })?;
    let mut camera = subject_camera.clone();
    for field in ["position_m", "target_m", "up"] {
        camera["transform"][field] = registration_transform_vec3(
            &subject_camera["transform"][field],
            registration_kind,
            field,
        )?;
    }
    camera = camera_v2_hashes(camera);
    validate_camera_calibration_v2(&camera)?;
    Ok(camera)
}

/// Wrap a canonical subject-space CameraRigCalibration@1 with the exact
/// geometry-space cameras consumed by the fixed renderer.  Both layers remain
/// present and independently hash-bound so a registered camera can never be
/// persisted while masquerading as a subject-space camera.
pub(crate) fn materialize_registered_weapon_camera_rig(
    subject_camera_rig: &Value,
    registration: &Value,
    program: &Value,
    registered_rig_id: &str,
    candidate_state_sha256: &str,
    artifact_id: &str,
    artifact_sha256: &str,
    geometry_program_object_sha256: &str,
    subject_camera_rig_object_sha256: &str,
) -> Result<Value, String> {
    let project_id = subject_camera_rig
        .get("project_id")
        .and_then(Value::as_str)
        .ok_or_else(|| "REGISTERED_CAMERA_RIG_INVALID: project_id".to_owned())?;
    let candidate_id = subject_camera_rig
        .get("candidate_id")
        .and_then(Value::as_str)
        .ok_or_else(|| "REGISTERED_CAMERA_RIG_INVALID: candidate_id".to_owned())?;
    validate_camera_rig(subject_camera_rig, project_id, candidate_id)?;
    validate_production_weapon_subject_frame_registration(registration, program)?;
    if !forgecad_contracts::is_opaque_id(registered_rig_id)
        || !forgecad_contracts::is_opaque_id(artifact_id)
        || !forgecad_contracts::is_sha256(candidate_state_sha256)
        || !forgecad_contracts::is_sha256(artifact_sha256)
        || !forgecad_contracts::is_sha256(geometry_program_object_sha256)
        || !forgecad_contracts::is_sha256(subject_camera_rig_object_sha256)
    {
        return Err("REGISTERED_CAMERA_RIG_INVALID: lineage identity".to_owned());
    }
    let subject_rig_canonical = subject_camera_rig
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| "REGISTERED_CAMERA_RIG_INVALID: subject rig hash".to_owned())?;
    let registration_canonical = registration
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| "REGISTERED_CAMERA_RIG_INVALID: registration hash".to_owned())?;
    let geometry_program_sha256 = registration
        .get("geometry_program_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| "REGISTERED_CAMERA_RIG_INVALID: geometry program hash".to_owned())?;
    let operator_catalog_sha256 = program
        .get("operator_catalog_sha256")
        .and_then(Value::as_str)
        .filter(|value| forgecad_contracts::is_sha256(value))
        .ok_or_else(|| "REGISTERED_CAMERA_RIG_INVALID: operator catalog hash".to_owned())?;
    let renderer_views = subject_camera_rig
        .get("views")
        .and_then(Value::as_array)
        .ok_or_else(|| "REGISTERED_CAMERA_RIG_INVALID: subject views".to_owned())?
        .iter()
        .map(|view| {
            let registered_camera =
                materialize_registered_weapon_camera(&view["camera"], registration, program)?;
            Ok(json!({
                "view_id":view["view_id"],
                "kind":view["kind"],
                "subject_camera_hash":view["camera_hash"],
                "registered_camera_hash":registered_camera["camera_hash"],
                "registered_camera":registered_camera,
                "registration_canonical_sha256":registration_canonical,
                "weight":view["weight"],
                "primary":view["primary"]
            }))
        })
        .collect::<Result<Vec<_>, String>>()?;
    let mut registered_rig = json!({
        "schema_version":REGISTERED_CAMERA_RIG_CALIBRATION_SCHEMA,
        "registered_rig_id":registered_rig_id,
        "project_id":project_id,
        "candidate_id":candidate_id,
        "candidate_state_sha256":candidate_state_sha256,
        "artifact_id":artifact_id,
        "artifact_sha256":artifact_sha256,
        "geometry_program_object_sha256":geometry_program_object_sha256,
        "geometry_program_sha256":geometry_program_sha256,
        "operator_catalog_sha256":operator_catalog_sha256,
        "subject_camera_rig":subject_camera_rig,
        "subject_camera_rig_object_sha256":subject_camera_rig_object_sha256,
        "subject_camera_rig_canonical_sha256":subject_rig_canonical,
        "subject_frame_registration":registration,
        "subject_frame_registration_canonical_sha256":registration_canonical,
        "renderer_views":renderer_views,
        "read_only":true,
        "runtime_write":false,
        "depth_status":"UNKNOWN",
        "quality_status":forgecad_contracts::REGISTERED_CAMERA_RIG_QUALITY_STATUS,
        "production_stage_advanced":false,
        "candidate_confirmed":false,
        "version_created":false,
        "export_performed":false,
        "canonical_sha256":""
    });
    registered_rig["canonical_sha256"] = Value::String(canonical_json_hash(&registered_rig));
    Ok(registered_rig)
}

pub(crate) fn validate_registered_weapon_camera_rig(
    registered_rig: &Value,
    program: &Value,
    project_id: &str,
    candidate_id: &str,
    candidate_state_sha256: &str,
    artifact_id: &str,
    artifact_sha256: &str,
    geometry_program_object_sha256: &str,
    subject_camera_rig_object_sha256: &str,
) -> Result<(), String> {
    if registered_rig.get("project_id").and_then(Value::as_str) != Some(project_id)
        || registered_rig.get("candidate_id").and_then(Value::as_str) != Some(candidate_id)
        || registered_rig
            .get("candidate_state_sha256")
            .and_then(Value::as_str)
            != Some(candidate_state_sha256)
        || registered_rig.get("artifact_id").and_then(Value::as_str) != Some(artifact_id)
        || registered_rig
            .get("artifact_sha256")
            .and_then(Value::as_str)
            != Some(artifact_sha256)
        || registered_rig
            .get("geometry_program_object_sha256")
            .and_then(Value::as_str)
            != Some(geometry_program_object_sha256)
        || registered_rig
            .get("subject_camera_rig_object_sha256")
            .and_then(Value::as_str)
            != Some(subject_camera_rig_object_sha256)
    {
        return Err("REGISTERED_CAMERA_RIG_INVALID: expected lineage mismatch".to_owned());
    }
    let registered_rig_id = registered_rig
        .get("registered_rig_id")
        .and_then(Value::as_str)
        .ok_or_else(|| "REGISTERED_CAMERA_RIG_INVALID: registered_rig_id".to_owned())?;
    let expected = materialize_registered_weapon_camera_rig(
        registered_rig
            .get("subject_camera_rig")
            .ok_or_else(|| "REGISTERED_CAMERA_RIG_INVALID: subject rig".to_owned())?,
        registered_rig
            .get("subject_frame_registration")
            .ok_or_else(|| "REGISTERED_CAMERA_RIG_INVALID: registration".to_owned())?,
        program,
        registered_rig_id,
        candidate_state_sha256,
        artifact_id,
        artifact_sha256,
        geometry_program_object_sha256,
        subject_camera_rig_object_sha256,
    )?;
    if registered_rig != &expected {
        return Err(
            "REGISTERED_CAMERA_RIG_INVALID: lineage differs from exact materialization".to_owned(),
        );
    }
    Ok(())
}

/// Verify that a registered rig is a read-only sibling projection of the exact
/// immutable CameraLock@1 rig. This does not rewrite or re-hash CameraLock@1;
/// durable CAS/readback validation remains owned by agentic_session.
pub(crate) fn validate_registered_weapon_camera_rig_camera_lock_link(
    registered_rig: &Value,
    camera_lock: &Value,
) -> Result<(), String> {
    let lock = camera_lock
        .as_object()
        .ok_or_else(|| "REGISTERED_CAMERA_LOCK_LINK_INVALID: lock object".to_owned())?;
    if lock.get("schema_version").and_then(Value::as_str) != Some("ProductionCameraLock@1")
        || lock.get("calibration_status").and_then(Value::as_str) != Some("passed")
        || lock.get("visual_status").and_then(Value::as_str) != Some("QUALITY_TARGET_NOT_MET")
        || lock.get("human_status").and_then(Value::as_str) != Some("NOT_RUN")
    {
        return Err("REGISTERED_CAMERA_LOCK_LINK_INVALID: frozen lock truth".to_owned());
    }
    for field in [
        "project_id",
        "candidate_id",
        "candidate_state_sha256",
        "artifact_id",
        "artifact_sha256",
        "camera_rig_object_sha256",
        "camera_rig_canonical_sha256",
    ] {
        let registered_field = match field {
            "camera_rig_object_sha256" => "subject_camera_rig_object_sha256",
            "camera_rig_canonical_sha256" => "subject_camera_rig_canonical_sha256",
            _ => field,
        };
        if lock.get(field) != registered_rig.get(registered_field) {
            return Err(format!(
                "REGISTERED_CAMERA_LOCK_LINK_INVALID: {field} mismatch"
            ));
        }
    }
    let expected_kinds = json!([
        "front",
        "back",
        "left",
        "right",
        "top",
        "bottom",
        "rear-three-quarter"
    ]);
    if lock.get("required_camera_view_kinds") != Some(&expected_kinds) {
        return Err("REGISTERED_CAMERA_LOCK_LINK_INVALID: camera view set".to_owned());
    }
    let renderer_views = registered_rig
        .get("renderer_views")
        .and_then(Value::as_array)
        .ok_or_else(|| "REGISTERED_CAMERA_LOCK_LINK_INVALID: renderer views".to_owned())?;
    let required_kinds = expected_kinds.as_array().expect("closed camera kinds");
    if renderer_views.len() != required_kinds.len()
        || required_kinds.iter().any(|kind| {
            renderer_views
                .iter()
                .filter(|view| view.get("kind") == Some(kind))
                .count()
                != 1
        })
    {
        return Err("REGISTERED_CAMERA_LOCK_LINK_INVALID: registered camera coverage".to_owned());
    }
    let primary_view_kind = lock
        .get("primary_view_kind")
        .and_then(Value::as_str)
        .ok_or_else(|| "REGISTERED_CAMERA_LOCK_LINK_INVALID: primary view kind".to_owned())?;
    if primary_view_kind != "left"
        || renderer_views
            .iter()
            .filter(|view| view.get("primary").and_then(Value::as_bool) == Some(true))
            .count()
            != 1
        || renderer_views.iter().any(|view| {
            view.get("primary").and_then(Value::as_bool) == Some(true)
                && view.get("kind").and_then(Value::as_str) != Some(primary_view_kind)
        })
    {
        return Err("REGISTERED_CAMERA_LOCK_LINK_INVALID: primary view binding".to_owned());
    }
    let source_rig = registered_rig
        .get("subject_camera_rig")
        .ok_or_else(|| "REGISTERED_CAMERA_LOCK_LINK_INVALID: subject rig".to_owned())?;
    if source_rig.get("canonical_sha256")
        != registered_rig.get("subject_camera_rig_canonical_sha256")
    {
        return Err("REGISTERED_CAMERA_LOCK_LINK_INVALID: embedded rig hash".to_owned());
    }
    if registered_rig.get("depth_status").and_then(Value::as_str) != Some("UNKNOWN")
        || registered_rig
            .get("production_stage_advanced")
            .and_then(Value::as_bool)
            != Some(false)
        || registered_rig
            .get("candidate_confirmed")
            .and_then(Value::as_bool)
            != Some(false)
        || registered_rig
            .get("version_created")
            .and_then(Value::as_bool)
            != Some(false)
        || registered_rig
            .get("export_performed")
            .and_then(Value::as_bool)
            != Some(false)
    {
        return Err("REGISTERED_CAMERA_LOCK_LINK_INVALID: projection promoted truth".to_owned());
    }
    Ok(())
}

/// Validate 3D source ordering without claiming that target-image landmarks
/// exist. This guard can prevent semantic flips, but can never satisfy a
/// landmark coverage or NME quality gate.
pub(crate) fn validate_production_weapon_semantic_landmark_ordering(
    ordering: &Value,
    registered_rig_v1: &Value,
    camera_lock: &Value,
) -> Result<(), String> {
    if ordering.get("schema_version").and_then(Value::as_str)
        != Some(PRODUCTION_WEAPON_SEMANTIC_LANDMARK_ORDERING_SCHEMA)
        || ordering.get("ordering_policy").and_then(Value::as_str)
            != Some(forgecad_contracts::PRODUCTION_WEAPON_SEMANTIC_ORDERING_POLICY)
        || ordering
            .get("target_landmark_arrays_present")
            .and_then(Value::as_bool)
            != Some(false)
        || ordering
            .get("target_landmark_metrics_status")
            .and_then(Value::as_str)
            != Some("NOT_PRESENT")
        || ordering.get("ordering_status").and_then(Value::as_str)
            != Some("PASS_DERIVED_SOURCE_ORDER_ONLY")
        || ordering
            .get("authored_orientation_status")
            .and_then(Value::as_str)
            != Some("BLOCKED_REQUIRED")
        || ordering.get("read_only").and_then(Value::as_bool) != Some(true)
        || ordering.get("runtime_write").and_then(Value::as_bool) != Some(false)
        || ordering
            .get("production_stage_advanced")
            .and_then(Value::as_bool)
            != Some(false)
        || ordering.get("candidate_confirmed").and_then(Value::as_bool) != Some(false)
        || ordering.get("version_created").and_then(Value::as_bool) != Some(false)
        || ordering.get("export_performed").and_then(Value::as_bool) != Some(false)
    {
        return Err("SEMANTIC_LANDMARK_ORDERING_INVALID: frozen truth".to_owned());
    }
    validate_registered_weapon_camera_rig_camera_lock_link(registered_rig_v1, camera_lock)?;
    for field in [
        "project_id",
        "candidate_id",
        "candidate_state_sha256",
        "artifact_id",
        "artifact_sha256",
    ] {
        if ordering.get(field) != registered_rig_v1.get(field)
            || ordering.get(field) != camera_lock.get(field)
        {
            return Err(format!(
                "SEMANTIC_LANDMARK_ORDERING_INVALID: {field} lineage"
            ));
        }
    }
    if ordering.get("reference_sha256") != camera_lock.get("reference_sha256")
        || ordering.get("subject_camera_rig_object_sha256")
            != registered_rig_v1.get("subject_camera_rig_object_sha256")
        || ordering.get("subject_camera_rig_canonical_sha256")
            != registered_rig_v1.get("subject_camera_rig_canonical_sha256")
        || ordering.get("registered_camera_rig_canonical_sha256")
            != registered_rig_v1.get("canonical_sha256")
    {
        return Err("SEMANTIC_LANDMARK_ORDERING_INVALID: registered rig binding".to_owned());
    }
    let identity_views = [
        "front",
        "back",
        "left",
        "right",
        "top",
        "rear-three-quarter",
    ];
    let camera_views = [
        "front",
        "back",
        "left",
        "right",
        "top",
        "bottom",
        "rear-three-quarter",
    ];
    if ordering.get("identity_view_kinds") != Some(&json!(identity_views))
        || ordering.get("camera_view_kinds") != Some(&json!(camera_views))
        || ordering.get("primary_view_kind").and_then(Value::as_str) != Some("left")
        || ordering.get("subject_longitudinal_order") != Some(&json!(["stock", "muzzle"]))
    {
        return Err("SEMANTIC_LANDMARK_ORDERING_INVALID: closed ordering".to_owned());
    }
    let anchors = ordering
        .get("anchors")
        .and_then(Value::as_array)
        .filter(|anchors| anchors.len() == 4)
        .ok_or_else(|| "SEMANTIC_LANDMARK_ORDERING_INVALID: anchors".to_owned())?;
    let mut seen = HashSet::new();
    for anchor in anchors {
        let anchor_id = anchor
            .get("anchor_id")
            .and_then(Value::as_str)
            .filter(|id| ["muzzle", "stock", "side-left", "side-right"].contains(id))
            .ok_or_else(|| "SEMANTIC_LANDMARK_ORDERING_INVALID: anchor id".to_owned())?;
        let part_ids = anchor
            .get("part_ids")
            .and_then(Value::as_array)
            .filter(|ids| !ids.is_empty() && ids.iter().all(|id| id.as_str().is_some()))
            .ok_or_else(|| "SEMANTIC_LANDMARK_ORDERING_INVALID: part ids".to_owned())?;
        let source_node_ids = anchor
            .get("source_node_ids")
            .and_then(Value::as_array)
            .filter(|ids| !ids.is_empty() && ids.iter().all(|id| id.as_str().is_some()))
            .ok_or_else(|| "SEMANTIC_LANDMARK_ORDERING_INVALID: source node ids".to_owned())?;
        let positions = anchor
            .get("source_positions_m")
            .and_then(Value::as_array)
            .filter(|values| {
                !values.is_empty()
                    && values.len() == source_node_ids.len()
                    && values
                        .iter()
                        .all(|value| value.as_f64().is_some_and(f64::is_finite))
            })
            .ok_or_else(|| "SEMANTIC_LANDMARK_ORDERING_INVALID: source positions".to_owned())?;
        if !seen.insert(anchor_id)
            || part_ids.is_empty()
            || positions.is_empty()
            || anchor
                .get("geometry_axis")
                .and_then(Value::as_str)
                .is_none()
            || anchor.get("subject_axis").and_then(Value::as_str).is_none()
            || anchor
                .get("semantic_role")
                .and_then(Value::as_str)
                .is_none()
            || anchor.get("tie_policy").and_then(Value::as_str).is_none()
        {
            return Err("SEMANTIC_LANDMARK_ORDERING_INVALID: anchor lineage".to_owned());
        }
    }
    if seen
        != ["muzzle", "stock", "side-left", "side-right"]
            .into_iter()
            .collect()
    {
        return Err("SEMANTIC_LANDMARK_ORDERING_INVALID: anchor coverage".to_owned());
    }
    let mut normalized = ordering.clone();
    normalized["canonical_sha256"] = Value::String(String::new());
    let expected_hash = canonical_json_hash(&normalized);
    if ordering.get("canonical_sha256").and_then(Value::as_str) != Some(expected_hash.as_str()) {
        return Err("SEMANTIC_LANDMARK_ORDERING_INVALID: canonical hash".to_owned());
    }
    Ok(())
}

/// Materialize the only admissible semantic ordering from the exact
/// candidate-owned GeometryProgram.  The result intentionally contains scalar
/// source-axis positions rather than image-space landmarks: it prevents a
/// stock/muzzle or left/right swap without pretending that the reference board
/// supplied 2D landmark observations.
pub(crate) fn materialize_production_weapon_semantic_landmark_ordering(
    registered_rig_v1: &Value,
    camera_lock: &Value,
    program: &Value,
    ordering_id: &str,
) -> Result<Value, String> {
    if !forgecad_contracts::is_opaque_id(ordering_id) {
        return Err("SEMANTIC_LANDMARK_ORDERING_INVALID: ordering id".to_owned());
    }
    validate_registered_weapon_camera_rig_camera_lock_link(registered_rig_v1, camera_lock)?;
    let registration = registered_rig_v1
        .get("subject_frame_registration")
        .ok_or_else(|| "SEMANTIC_LANDMARK_ORDERING_INVALID: registration".to_owned())?;
    validate_production_weapon_subject_frame_registration(registration, program)?;
    if registered_rig_v1.get("geometry_program_sha256") != program.get("canonical_sha256") {
        return Err("SEMANTIC_LANDMARK_ORDERING_INVALID: geometry program binding".to_owned());
    }

    require_exact_part_sources(
        program,
        "rear-stock",
        &["rear-stock", "rear-stock-lower-beam"],
    )?;
    for part_id in [
        "muzzle-shroud",
        "muzzle-emitter",
        "muzzle-core",
        "side-light-left",
        "side-light-right",
    ] {
        require_exact_part_sources(program, part_id, &[part_id])?;
    }

    let stock_node_ids = ["rear-stock", "rear-stock-lower-beam"];
    let muzzle_node_ids = ["muzzle-shroud", "muzzle-emitter", "muzzle-core"];
    let stock_positions = stock_node_ids
        .iter()
        .map(|node_id| exact_program_node_position_axis(program, node_id, 0))
        .collect::<Result<Vec<_>, _>>()?;
    let muzzle_positions = muzzle_node_ids
        .iter()
        .map(|node_id| exact_program_node_position_axis(program, node_id, 0))
        .collect::<Result<Vec<_>, _>>()?;
    let side_left_position = exact_program_node_position_axis(program, "side-light-left", 2)?;
    let side_right_position = exact_program_node_position_axis(program, "side-light-right", 2)?;
    let geometry_muzzle_axis = registration
        .pointer("/geometry_semantic_axes/muzzle")
        .and_then(Value::as_str)
        .ok_or_else(|| "SEMANTIC_LANDMARK_ORDERING_INVALID: muzzle axis".to_owned())?;
    let geometry_stock_axis = registration
        .pointer("/geometry_semantic_axes/stock")
        .and_then(Value::as_str)
        .ok_or_else(|| "SEMANTIC_LANDMARK_ORDERING_INVALID: stock axis".to_owned())?;
    let (geometry_left_axis, geometry_right_axis) = if side_left_position < -AXIS_SIGN_EPSILON_M
        && side_right_position > AXIS_SIGN_EPSILON_M
    {
        ("-Z", "+Z")
    } else if side_left_position > AXIS_SIGN_EPSILON_M && side_right_position < -AXIS_SIGN_EPSILON_M
    {
        ("+Z", "-Z")
    } else {
        return Err(
            "SEMANTIC_LANDMARK_ORDERING_BLOCKED: side anchors do not define one exact lateral axis"
                .to_owned(),
        );
    };

    let mut ordering = json!({
        "schema_version":PRODUCTION_WEAPON_SEMANTIC_LANDMARK_ORDERING_SCHEMA,
        "ordering_id":ordering_id,
        "project_id":registered_rig_v1["project_id"],
        "candidate_id":registered_rig_v1["candidate_id"],
        "candidate_state_sha256":registered_rig_v1["candidate_state_sha256"],
        "artifact_id":registered_rig_v1["artifact_id"],
        "artifact_sha256":registered_rig_v1["artifact_sha256"],
        "reference_sha256":camera_lock["reference_sha256"],
        "subject_camera_rig_object_sha256":registered_rig_v1["subject_camera_rig_object_sha256"],
        "subject_camera_rig_canonical_sha256":registered_rig_v1["subject_camera_rig_canonical_sha256"],
        "registered_camera_rig_canonical_sha256":registered_rig_v1["canonical_sha256"],
        "ordering_policy":forgecad_contracts::PRODUCTION_WEAPON_SEMANTIC_ORDERING_POLICY,
        "identity_view_kinds":["front","back","left","right","top","rear-three-quarter"],
        "camera_view_kinds":["front","back","left","right","top","bottom","rear-three-quarter"],
        "primary_view_kind":"left",
        "subject_longitudinal_order":["stock","muzzle"],
        "anchors":[
            {
                "anchor_id":"muzzle",
                "semantic_role":"muzzle",
                "part_ids":["muzzle-shroud","muzzle-emitter","muzzle-core"],
                "source_node_ids":muzzle_node_ids,
                "geometry_axis":geometry_muzzle_axis,
                "subject_axis":"+X",
                "source_positions_m":muzzle_positions,
                "tie_policy":"exact-part-output-source-order"
            },
            {
                "anchor_id":"stock",
                "semantic_role":"stock",
                "part_ids":["rear-stock"],
                "source_node_ids":stock_node_ids,
                "geometry_axis":geometry_stock_axis,
                "subject_axis":"-X",
                "source_positions_m":stock_positions,
                "tie_policy":"exact-part-output-source-order"
            },
            {
                "anchor_id":"side-left",
                "semantic_role":"left",
                "part_ids":["side-light-left"],
                "source_node_ids":["side-light-left"],
                "geometry_axis":geometry_left_axis,
                "subject_axis":"-Z",
                "source_positions_m":[side_left_position],
                "tie_policy":"explicit-source-group"
            },
            {
                "anchor_id":"side-right",
                "semantic_role":"right",
                "part_ids":["side-light-right"],
                "source_node_ids":["side-light-right"],
                "geometry_axis":geometry_right_axis,
                "subject_axis":"+Z",
                "source_positions_m":[side_right_position],
                "tie_policy":"explicit-source-group"
            }
        ],
        "target_landmark_arrays_present":false,
        "target_landmark_metrics_status":"NOT_PRESENT",
        "ordering_status":"PASS_DERIVED_SOURCE_ORDER_ONLY",
        "authored_orientation_status":"BLOCKED_REQUIRED",
        "read_only":true,
        "runtime_write":false,
        "production_stage_advanced":false,
        "candidate_confirmed":false,
        "version_created":false,
        "export_performed":false,
        "canonical_sha256":""
    });
    ordering["canonical_sha256"] = Value::String(canonical_json_hash(&ordering));
    validate_production_weapon_semantic_landmark_ordering(
        &ordering,
        registered_rig_v1,
        camera_lock,
    )?;
    Ok(ordering)
}

pub(crate) fn validate_production_weapon_authored_view_orientation(
    orientation: &Value,
    camera_lock: &Value,
    require_promotable: bool,
) -> Result<(), String> {
    let object = orientation
        .as_object()
        .ok_or_else(|| "AUTHORED_VIEW_ORIENTATION_INVALID: object required".to_owned())?;
    const ORIENTATION_KEYS: [&str; 27] = [
        "schema_version",
        "orientation_id",
        "project_id",
        "candidate_id",
        "candidate_state_sha256",
        "artifact_id",
        "artifact_sha256",
        "reference_id",
        "reference_sha256",
        "view_kind",
        "source_view",
        "reference_view_spec_canonical_sha256",
        "source_crop",
        "reference_to_subject_view",
        "subject_screen_order",
        "registered_camera_orbit",
        "post_render_transform",
        "target_landmark_status",
        "orientation_provenance",
        "status",
        "promotable",
        "read_only",
        "runtime_write",
        "production_stage_advanced",
        "candidate_confirmed",
        "version_created",
        "export_performed",
    ];
    if object.len() != ORIENTATION_KEYS.len() + 1
        || object.keys().any(|key| {
            !ORIENTATION_KEYS.contains(&key.as_str()) && key.as_str() != "canonical_sha256"
        })
        || object
            .get("orientation_id")
            .and_then(Value::as_str)
            .is_none_or(|id| !forgecad_contracts::is_opaque_id(id))
    {
        return Err("AUTHORED_VIEW_ORIENTATION_INVALID: closed shape".to_owned());
    }
    if orientation.get("schema_version").and_then(Value::as_str)
        != Some(PRODUCTION_WEAPON_AUTHORED_VIEW_ORIENTATION_SCHEMA)
        || orientation.get("view_kind").and_then(Value::as_str) != Some("rear-three-quarter")
        || orientation.get("source_view").and_then(Value::as_str) != Some("rear-three-quarter")
        || orientation
            .get("post_render_transform")
            .and_then(Value::as_str)
            != Some("identity")
        || orientation.get("read_only").and_then(Value::as_bool) != Some(true)
        || orientation.get("runtime_write").and_then(Value::as_bool) != Some(false)
        || orientation
            .get("production_stage_advanced")
            .and_then(Value::as_bool)
            != Some(false)
        || orientation
            .get("candidate_confirmed")
            .and_then(Value::as_bool)
            != Some(false)
        || orientation.get("version_created").and_then(Value::as_bool) != Some(false)
        || orientation.get("export_performed").and_then(Value::as_bool) != Some(false)
    {
        return Err("AUTHORED_VIEW_ORIENTATION_INVALID: frozen truth".to_owned());
    }
    for field in [
        "project_id",
        "candidate_id",
        "candidate_state_sha256",
        "artifact_id",
        "artifact_sha256",
        "reference_id",
        "reference_sha256",
    ] {
        if orientation.get(field) != camera_lock.get(field) {
            return Err(format!(
                "AUTHORED_VIEW_ORIENTATION_INVALID: {field} lineage"
            ));
        }
    }
    for field in [
        "candidate_state_sha256",
        "artifact_sha256",
        "reference_sha256",
        "reference_view_spec_canonical_sha256",
        "canonical_sha256",
    ] {
        if orientation
            .get(field)
            .and_then(Value::as_str)
            .is_none_or(|hash| !forgecad_contracts::is_sha256(hash))
        {
            return Err(format!("AUTHORED_VIEW_ORIENTATION_INVALID: {field}"));
        }
    }
    let crop = orientation
        .get("source_crop")
        .and_then(Value::as_object)
        .ok_or_else(|| "AUTHORED_VIEW_ORIENTATION_INVALID: source crop".to_owned())?;
    const CROP_KEYS: [&str; 4] = [
        "board_size_px",
        "crop_xywh_px",
        "source_crop_sha256",
        "runtime_crop_png_sha256",
    ];
    if crop.len() != CROP_KEYS.len() || crop.keys().any(|key| !CROP_KEYS.contains(&key.as_str())) {
        return Err("AUTHORED_VIEW_ORIENTATION_INVALID: source crop shape".to_owned());
    }
    let board = crop
        .get("board_size_px")
        .and_then(Value::as_array)
        .filter(|values| values.len() == 2)
        .ok_or_else(|| "AUTHORED_VIEW_ORIENTATION_INVALID: board size".to_owned())?;
    let crop_xywh = crop
        .get("crop_xywh_px")
        .and_then(Value::as_array)
        .filter(|values| values.len() == 4)
        .ok_or_else(|| "AUTHORED_VIEW_ORIENTATION_INVALID: crop bounds".to_owned())?;
    let board_width = board[0]
        .as_u64()
        .filter(|value| *value > 0)
        .ok_or_else(|| "AUTHORED_VIEW_ORIENTATION_INVALID: board width".to_owned())?;
    let board_height = board[1]
        .as_u64()
        .filter(|value| *value > 0)
        .ok_or_else(|| "AUTHORED_VIEW_ORIENTATION_INVALID: board height".to_owned())?;
    let x = crop_xywh[0]
        .as_u64()
        .ok_or_else(|| "AUTHORED_VIEW_ORIENTATION_INVALID: crop x".to_owned())?;
    let y = crop_xywh[1]
        .as_u64()
        .ok_or_else(|| "AUTHORED_VIEW_ORIENTATION_INVALID: crop y".to_owned())?;
    let width = crop_xywh[2]
        .as_u64()
        .filter(|value| *value > 0)
        .ok_or_else(|| "AUTHORED_VIEW_ORIENTATION_INVALID: crop width".to_owned())?;
    let height = crop_xywh[3]
        .as_u64()
        .filter(|value| *value > 0)
        .ok_or_else(|| "AUTHORED_VIEW_ORIENTATION_INVALID: crop height".to_owned())?;
    if x.checked_add(width).is_none_or(|right| right > board_width)
        || y.checked_add(height)
            .is_none_or(|bottom| bottom > board_height)
        || crop
            .get("source_crop_sha256")
            .and_then(Value::as_str)
            .is_none_or(|hash| !forgecad_contracts::is_sha256(hash))
        || crop
            .get("runtime_crop_png_sha256")
            .and_then(Value::as_str)
            .is_none_or(|hash| !forgecad_contracts::is_sha256(hash))
    {
        return Err("AUTHORED_VIEW_ORIENTATION_INVALID: crop binding".to_owned());
    }
    let provenance = orientation
        .get("orientation_provenance")
        .ok_or_else(|| "AUTHORED_VIEW_ORIENTATION_INVALID: provenance".to_owned())?;
    let rotation_degrees = orientation
        .pointer("/reference_to_subject_view/rotation_degrees")
        .and_then(Value::as_i64)
        .filter(|value| [-180, -90, 0, 90, 180].contains(value))
        .ok_or_else(|| "AUTHORED_VIEW_ORIENTATION_INVALID: rotation".to_owned())?;
    let transform = orientation
        .get("reference_to_subject_view")
        .and_then(Value::as_object)
        .ok_or_else(|| "AUTHORED_VIEW_ORIENTATION_INVALID: transform object".to_owned())?;
    const TRANSFORM_KEYS: [&str; 6] = [
        "coordinate_space",
        "kind",
        "rotation_degrees",
        "matrix_3x3",
        "translation",
        "scale",
    ];
    if transform.len() != TRANSFORM_KEYS.len()
        || transform
            .keys()
            .any(|key| !TRANSFORM_KEYS.contains(&key.as_str()))
    {
        return Err("AUTHORED_VIEW_ORIENTATION_INVALID: transform shape".to_owned());
    }
    let transform_kind = transform.get("kind").and_then(Value::as_str);
    if orientation
        .pointer("/reference_to_subject_view/coordinate_space")
        .and_then(Value::as_str)
        != Some("crop-local-normalized-image")
        || !matches!(transform_kind, Some("identity" | "rotate-clockwise"))
        || (rotation_degrees == 0 && transform_kind != Some("identity"))
        || (rotation_degrees != 0 && transform_kind != Some("rotate-clockwise"))
        || orientation
            .get("reference_view_spec_canonical_sha256")
            .and_then(Value::as_str)
            .is_none_or(|hash| !forgecad_contracts::is_sha256(hash))
    {
        return Err("AUTHORED_VIEW_ORIENTATION_INVALID: transform".to_owned());
    }
    let expected_matrix: [[f64; 3]; 3] = match rotation_degrees {
        0 => [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        90 => [[0.0, -1.0, 1.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]],
        -90 => [[0.0, 1.0, 0.0], [-1.0, 0.0, 1.0], [0.0, 0.0, 1.0]],
        180 | -180 => [[-1.0, 0.0, 1.0], [0.0, -1.0, 1.0], [0.0, 0.0, 1.0]],
        _ => unreachable!("rotation enum checked above"),
    };
    let matrix = transform
        .get("matrix_3x3")
        .and_then(Value::as_array)
        .filter(|rows| rows.len() == 3)
        .ok_or_else(|| "AUTHORED_VIEW_ORIENTATION_INVALID: transform matrix".to_owned())?;
    let matrix_matches = matrix.iter().enumerate().all(|(row_index, row)| {
        row.as_array().is_some_and(|columns| {
            columns.len() == 3
                && columns.iter().enumerate().all(|(column_index, value)| {
                    value.as_f64().is_some_and(|actual| {
                        actual.is_finite()
                            && (actual - expected_matrix[row_index][column_index]).abs() <= 1e-12
                    })
                })
        })
    });
    let exact_vec2 = |field: &str, expected: [f64; 2]| {
        transform
            .get(field)
            .and_then(Value::as_array)
            .is_some_and(|values| {
                values.len() == 2
                    && values.iter().enumerate().all(|(index, value)| {
                        value.as_f64().is_some_and(|actual| {
                            actual.is_finite() && (actual - expected[index]).abs() <= 1e-12
                        })
                    })
            })
    };
    if !matrix_matches
        || !exact_vec2("translation", [0.0, 0.0])
        || !exact_vec2("scale", [1.0, 1.0])
        || orientation
            .get("target_landmark_status")
            .and_then(Value::as_str)
            != Some("NOT_PRESENT")
    {
        return Err("AUTHORED_VIEW_ORIENTATION_INVALID: transform truth mismatch".to_owned());
    }
    let subject_screen_order = orientation
        .get("subject_screen_order")
        .and_then(Value::as_str)
        .filter(|value| ["stock-left-muzzle-right", "muzzle-left-stock-right"].contains(value))
        .ok_or_else(|| "AUTHORED_VIEW_ORIENTATION_INVALID: subject screen order".to_owned())?;
    let camera_orbit = orientation
        .get("registered_camera_orbit")
        .and_then(Value::as_object)
        .ok_or_else(|| "AUTHORED_VIEW_ORIENTATION_INVALID: camera orbit".to_owned())?;
    let camera_orbit_degrees = camera_orbit
        .get("yaw_degrees")
        .and_then(Value::as_i64)
        .filter(|value| [0, 180].contains(value))
        .ok_or_else(|| "AUTHORED_VIEW_ORIENTATION_INVALID: camera orbit yaw".to_owned())?;
    if camera_orbit.len() != 3
        || camera_orbit.get("coordinate_space").and_then(Value::as_str)
            != Some("registered-geometry-y-up")
        || camera_orbit.get("kind").and_then(Value::as_str)
            != Some(if camera_orbit_degrees == 0 {
                "identity"
            } else {
                "yaw-around-world-origin"
            })
        || (subject_screen_order == "stock-left-muzzle-right" && camera_orbit_degrees == 0)
        || (subject_screen_order == "muzzle-left-stock-right" && camera_orbit_degrees != 0)
    {
        return Err("AUTHORED_VIEW_ORIENTATION_INVALID: semantic camera alignment".to_owned());
    }
    let is_promotable = orientation.get("promotable").and_then(Value::as_bool) == Some(true)
        && orientation.get("status").and_then(Value::as_str)
            == Some("APPROVED_AUTHORED_REFERENCE_ORIENTATION")
        && provenance
            .get("orientation_explicitly_authored")
            .and_then(Value::as_bool)
            == Some(true)
        && provenance.get("source").and_then(Value::as_str)
            == Some("user-authored-orientation-receipt")
        && provenance
            .get("authored_receipt_sha256")
            .and_then(Value::as_str)
            .is_some_and(forgecad_contracts::is_sha256);
    if require_promotable && !is_promotable {
        return Err("BLOCKED_AUTHORED_REAR_THREE_QUARTER_ORIENTATION".to_owned());
    }
    if !is_promotable
        && (orientation.get("promotable").and_then(Value::as_bool) != Some(false)
            || orientation.get("status").and_then(Value::as_str)
                != Some("BLOCKED_AUTHORED_ORIENTATION")
            || provenance
                .get("orientation_explicitly_authored")
                .and_then(Value::as_bool)
                != Some(false)
            || provenance.get("source").and_then(Value::as_str)
                != Some("diagnostic-transform-discovery")
            || !provenance
                .get("authored_receipt_sha256")
                .is_some_and(Value::is_null))
    {
        return Err("AUTHORED_VIEW_ORIENTATION_INVALID: authority".to_owned());
    }
    let mut normalized = orientation.clone();
    normalized["canonical_sha256"] = Value::String(String::new());
    let expected_hash = canonical_json_hash(&normalized);
    if orientation.get("canonical_sha256").and_then(Value::as_str) != Some(expected_hash.as_str()) {
        return Err("AUTHORED_VIEW_ORIENTATION_INVALID: canonical hash".to_owned());
    }
    Ok(())
}

/// Build the complete orientation authority from the one artistic decision a
/// user actually makes: the rear-three-quarter board rotation.  Every lineage,
/// crop and matrix field is Runtime materialized so an MCP caller cannot pair a
/// visible angle with a different transform or retarget the approval to another
/// candidate/reference.
#[allow(clippy::too_many_arguments)]
pub(crate) fn materialize_production_weapon_authored_view_orientation(
    camera_lock: &Value,
    orientation_id: &str,
    reference_view_spec_canonical_sha256: &str,
    board_size_px: [u64; 2],
    crop_xywh_px: [u64; 4],
    source_crop_sha256: &str,
    runtime_crop_png_sha256: &str,
    rotation_degrees: i64,
    authored_receipt_sha256: &str,
    subject_screen_order: &str,
    camera_orbit_degrees: i64,
) -> Result<Value, String> {
    if !forgecad_contracts::is_opaque_id(orientation_id)
        || !forgecad_contracts::is_sha256(reference_view_spec_canonical_sha256)
        || !forgecad_contracts::is_sha256(source_crop_sha256)
        || !forgecad_contracts::is_sha256(runtime_crop_png_sha256)
        || !forgecad_contracts::is_sha256(authored_receipt_sha256)
        || !["stock-left-muzzle-right", "muzzle-left-stock-right"].contains(&subject_screen_order)
        || ![0, 180].contains(&camera_orbit_degrees)
        || (subject_screen_order == "stock-left-muzzle-right" && camera_orbit_degrees == 0)
        || (subject_screen_order == "muzzle-left-stock-right" && camera_orbit_degrees != 0)
        || board_size_px.iter().any(|value| *value == 0)
        || crop_xywh_px[2] == 0
        || crop_xywh_px[3] == 0
        || crop_xywh_px[0]
            .checked_add(crop_xywh_px[2])
            .is_none_or(|right| right > board_size_px[0])
        || crop_xywh_px[1]
            .checked_add(crop_xywh_px[3])
            .is_none_or(|bottom| bottom > board_size_px[1])
    {
        return Err("AUTHORED_VIEW_ORIENTATION_INVALID: materialization input".to_owned());
    }
    let matrix = match rotation_degrees {
        0 => json!([[1, 0, 0], [0, 1, 0], [0, 0, 1]]),
        90 => json!([[0, -1, 1], [1, 0, 0], [0, 0, 1]]),
        -90 => json!([[0, 1, 0], [-1, 0, 1], [0, 0, 1]]),
        180 | -180 => json!([[-1, 0, 1], [0, -1, 1], [0, 0, 1]]),
        _ => return Err("AUTHORED_VIEW_ORIENTATION_INVALID: rotation".to_owned()),
    };
    let mut orientation = json!({
        "schema_version":PRODUCTION_WEAPON_AUTHORED_VIEW_ORIENTATION_SCHEMA,
        "orientation_id":orientation_id,
        "project_id":camera_lock["project_id"],
        "candidate_id":camera_lock["candidate_id"],
        "candidate_state_sha256":camera_lock["candidate_state_sha256"],
        "artifact_id":camera_lock["artifact_id"],
        "artifact_sha256":camera_lock["artifact_sha256"],
        "reference_id":camera_lock["reference_id"],
        "reference_sha256":camera_lock["reference_sha256"],
        "view_kind":"rear-three-quarter",
        "source_view":"rear-three-quarter",
        "reference_view_spec_canonical_sha256":reference_view_spec_canonical_sha256,
        "source_crop":{
            "board_size_px":board_size_px,
            "crop_xywh_px":crop_xywh_px,
            "source_crop_sha256":source_crop_sha256,
            "runtime_crop_png_sha256":runtime_crop_png_sha256
        },
        "reference_to_subject_view":{
            "coordinate_space":"crop-local-normalized-image",
            "kind":if rotation_degrees == 0 { "identity" } else { "rotate-clockwise" },
            "rotation_degrees":rotation_degrees,
            "matrix_3x3":matrix,
            "translation":[0,0],
            "scale":[1,1]
        },
        "subject_screen_order":subject_screen_order,
        "registered_camera_orbit":{
            "coordinate_space":"registered-geometry-y-up",
            "kind":if camera_orbit_degrees == 0 { "identity" } else { "yaw-around-world-origin" },
            "yaw_degrees":camera_orbit_degrees
        },
        "post_render_transform":"identity",
        "target_landmark_status":"NOT_PRESENT",
        "orientation_provenance":{
            "existing_confirmation_scope":"orientation-specific-user-approval",
            "orientation_explicitly_authored":true,
            "source":"user-authored-orientation-receipt",
            "authored_receipt_sha256":authored_receipt_sha256
        },
        "status":"APPROVED_AUTHORED_REFERENCE_ORIENTATION",
        "promotable":true,
        "read_only":true,
        "runtime_write":false,
        "production_stage_advanced":false,
        "candidate_confirmed":false,
        "version_created":false,
        "export_performed":false,
        "canonical_sha256":""
    });
    orientation["canonical_sha256"] = Value::String(canonical_json_hash(&orientation));
    validate_production_weapon_authored_view_orientation(&orientation, camera_lock, true)?;
    Ok(orientation)
}

/// Orbit one already-registered camera around the geometry-space Y axis while
/// preserving its upright vector. This fixes a semantic rear/front oblique
/// mismatch without rotating or mirroring the authorized reference pixels.
/// The operation is deliberately closed to identity or the opposite oblique.
fn materialize_registered_camera_orbit(
    registered_camera: &Value,
    yaw_degrees: i64,
) -> Result<Value, String> {
    validate_camera_calibration_v2(registered_camera)?;
    if ![0, 180].contains(&yaw_degrees) {
        return Err("REGISTERED_CAMERA_RIG_V2_INVALID: camera orbit".to_owned());
    }
    if yaw_degrees == 0 {
        return Ok(registered_camera.clone());
    }
    let mut camera = registered_camera.clone();
    for field in ["position_m", "target_m", "up"] {
        camera["transform"][field] = registration_transform_vec3(
            &registered_camera["transform"][field],
            "yaw-180-y",
            field,
        )?;
    }
    camera = camera_v2_hashes(camera);
    validate_camera_calibration_v2(&camera)?;
    Ok(camera)
}

fn camera_vec3(camera: &Value, field: &str) -> Result<[f64; 3], String> {
    let values = camera
        .pointer(&format!("/transform/{field}"))
        .and_then(Value::as_array)
        .filter(|values| values.len() == 3)
        .ok_or_else(|| "REGISTERED_CAMERA_RIG_V2_INVALID: camera vector".to_owned())?;
    let mut result = [0.0; 3];
    for (index, value) in values.iter().enumerate() {
        result[index] = value
            .as_f64()
            .filter(|value| value.is_finite())
            .ok_or_else(|| "REGISTERED_CAMERA_RIG_V2_INVALID: camera vector value".to_owned())?;
    }
    Ok(result)
}

fn normalize_vec3(value: [f64; 3]) -> Result<[f64; 3], String> {
    let length = (value[0] * value[0] + value[1] * value[1] + value[2] * value[2]).sqrt();
    if !length.is_finite() || length <= 1e-9 {
        return Err("REGISTERED_CAMERA_RIG_V2_INVALID: degenerate camera basis".to_owned());
    }
    Ok([value[0] / length, value[1] / length, value[2] / length])
}

fn cross_vec3(lhs: [f64; 3], rhs: [f64; 3]) -> [f64; 3] {
    [
        lhs[1] * rhs[2] - lhs[2] * rhs[1],
        lhs[2] * rhs[0] - lhs[0] * rhs[2],
        lhs[0] * rhs[1] - lhs[1] * rhs[0],
    ]
}

fn semantic_anchor_mean_x(ordering: &Value, anchor_id: &str) -> Result<f64, String> {
    let positions = ordering
        .get("anchors")
        .and_then(Value::as_array)
        .and_then(|anchors| {
            anchors
                .iter()
                .find(|anchor| anchor.get("anchor_id").and_then(Value::as_str) == Some(anchor_id))
        })
        .and_then(|anchor| anchor.get("source_positions_m"))
        .and_then(Value::as_array)
        .filter(|values| !values.is_empty())
        .ok_or_else(|| "REGISTERED_CAMERA_RIG_V2_INVALID: semantic anchor".to_owned())?;
    let values = positions
        .iter()
        .map(|value| {
            value
                .as_f64()
                .filter(|value| value.is_finite())
                .ok_or_else(|| "REGISTERED_CAMERA_RIG_V2_INVALID: semantic position".to_owned())
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(values.iter().sum::<f64>() / values.len() as f64)
}

fn evaluate_rear_three_quarter_semantic_orientation_proof(
    camera: &Value,
    semantic_ordering: &Value,
    expected_order: &str,
) -> Result<Value, String> {
    let position = camera_vec3(camera, "position_m")?;
    let target = camera_vec3(camera, "target_m")?;
    let authored_up = camera_vec3(camera, "up")?;
    let forward = normalize_vec3([
        target[0] - position[0],
        target[1] - position[1],
        target[2] - position[2],
    ])?;
    let screen_right = normalize_vec3(cross_vec3(forward, authored_up))?;
    let screen_up = normalize_vec3(cross_vec3(screen_right, forward))?;
    let stock_x = semantic_anchor_mean_x(semantic_ordering, "stock")?;
    let muzzle_x = semantic_anchor_mean_x(semantic_ordering, "muzzle")?;
    let stock_minus_muzzle_screen_x = (stock_x - muzzle_x) * screen_right[0];
    let actual_order = if stock_minus_muzzle_screen_x < -1e-9 {
        "stock-left-muzzle-right"
    } else if stock_minus_muzzle_screen_x > 1e-9 {
        "muzzle-left-stock-right"
    } else {
        return Err("REGISTERED_CAMERA_RIG_V2_INVALID: ambiguous semantic screen order".to_owned());
    };
    if !["stock-left-muzzle-right", "muzzle-left-stock-right"].contains(&expected_order) {
        return Err("REGISTERED_CAMERA_RIG_V2_INVALID: authored screen order".to_owned());
    }
    let upright_dot_milli = (screen_up[1] * 1000.0).round() as i64;
    let order_delta_milli = (stock_minus_muzzle_screen_x * 1000.0).round() as i64;
    let passed = actual_order == expected_order && upright_dot_milli > 0;
    let mut proof = json!({
        "policy":"runtime-projected-stock-muzzle-screen-order-and-world-y-upright@1",
        "camera_hash":camera["camera_hash"],
        "expected_subject_screen_order":expected_order,
        "projected_subject_screen_order":actual_order,
        "stock_minus_muzzle_screen_x_milli":order_delta_milli,
        "world_y_screen_up_dot_milli":upright_dot_milli,
        "screen_up":"world-positive-y",
        "passed":passed,
        "canonical_sha256":""
    });
    proof["canonical_sha256"] = Value::String(canonical_json_hash(&proof));
    Ok(proof)
}

fn materialize_rear_three_quarter_semantic_orientation_proof(
    camera: &Value,
    semantic_ordering: &Value,
    authored_orientation: &Value,
) -> Result<Value, String> {
    let expected_order = authored_orientation
        .get("subject_screen_order")
        .and_then(Value::as_str)
        .ok_or_else(|| "REGISTERED_CAMERA_RIG_V2_INVALID: authored screen order".to_owned())?;
    let proof = evaluate_rear_three_quarter_semantic_orientation_proof(
        camera,
        semantic_ordering,
        expected_order,
    )?;
    if proof.get("passed").and_then(Value::as_bool) != Some(true) {
        return Err(
            "REGISTERED_CAMERA_RIG_V2_BLOCKED: semantic order or upright proof failed".to_owned(),
        );
    }
    Ok(proof)
}

/// Produce the exact semantic camera proposal that a user reviews before a
/// durable authored-orientation receipt exists. This is a zero-write Runtime
/// projection over the candidate-owned registered rig and semantic anchors.
pub(crate) fn rear_three_quarter_camera_orbit_for_screen_order(
    expected_order: &str,
) -> Result<i64, String> {
    match expected_order {
        "stock-left-muzzle-right" => Ok(180),
        "muzzle-left-stock-right" => Ok(0),
        _ => Err("REGISTERED_CAMERA_PREFLIGHT_INVALID: semantic screen order".to_owned()),
    }
}

pub(crate) fn materialize_rear_three_quarter_semantic_camera_preview(
    registered_rig_v1: &Value,
    semantic_ordering: &Value,
    expected_order: &str,
) -> Result<Value, String> {
    let camera_orbit_degrees = rear_three_quarter_camera_orbit_for_screen_order(expected_order)?;
    let source = registered_rig_v1
        .get("renderer_views")
        .and_then(Value::as_array)
        .and_then(|views| {
            views
                .iter()
                .find(|view| view.get("kind").and_then(Value::as_str) == Some("rear-three-quarter"))
        })
        .ok_or_else(|| "REGISTERED_CAMERA_PREFLIGHT_INVALID: rear view missing".to_owned())?;
    let camera =
        materialize_registered_camera_orbit(&source["registered_camera"], camera_orbit_degrees)?;
    let proof = evaluate_rear_three_quarter_semantic_orientation_proof(
        &camera,
        semantic_ordering,
        expected_order,
    )?;
    let mut preview = json!({
        "policy":"runtime-derived-rear-three-quarter-semantic-camera-preflight@1",
        "camera_orbit_degrees":camera_orbit_degrees,
        "derived_registered_camera_hash":camera["camera_hash"],
        "derived_registered_camera_canonical_sha256":camera["canonical_sha256"],
        "upright_proof":proof.clone(),
        "projected_subject_screen_order":proof["projected_subject_screen_order"],
        "stock_minus_muzzle_screen_x_milli":proof["stock_minus_muzzle_screen_x_milli"],
        "world_y_screen_up_dot_milli":proof["world_y_screen_up_dot_milli"],
        "semantic_orientation_proof_passed":proof["passed"],
        "semantic_orientation_proof_sha256":proof["canonical_sha256"],
        "runtime_write":false,
        "canonical_sha256":""
    });
    preview["canonical_sha256"] = Value::String(canonical_json_hash(&preview));
    Ok(preview)
}

pub(crate) fn materialize_registered_weapon_camera_rig_v2(
    registered_rig_v1: &Value,
    camera_lock: &Value,
    semantic_ordering: &Value,
    semantic_ordering_object_sha256: &str,
    authored_orientation: &Value,
    authored_orientation_object_sha256: &str,
    reference_views: &Value,
    registered_rig_v2_id: &str,
    rear_three_quarter_camera_orbit_degrees: i64,
) -> Result<Value, String> {
    if !forgecad_contracts::is_sha256(semantic_ordering_object_sha256)
        || !forgecad_contracts::is_sha256(authored_orientation_object_sha256)
        || registered_rig_v2_id.is_empty()
    {
        return Err("REGISTERED_CAMERA_RIG_V2_INVALID: identity".to_owned());
    }
    validate_production_weapon_semantic_landmark_ordering(
        semantic_ordering,
        registered_rig_v1,
        camera_lock,
    )?;
    validate_production_weapon_authored_view_orientation(authored_orientation, camera_lock, true)?;
    if authored_orientation
        .pointer("/registered_camera_orbit/yaw_degrees")
        .and_then(Value::as_i64)
        != Some(rear_three_quarter_camera_orbit_degrees)
    {
        return Err("REGISTERED_CAMERA_RIG_V2_INVALID: camera orbit authority mismatch".to_owned());
    }
    let reference_views = reference_views
        .as_array()
        .filter(|views| views.len() == 6)
        .ok_or_else(|| "REGISTERED_CAMERA_RIG_V2_INVALID: reference views".to_owned())?;
    let required_reference_kinds = [
        "front",
        "back",
        "left",
        "right",
        "top",
        "rear-three-quarter",
    ];
    let mut reference_kind_set = HashSet::new();
    for reference_view in reference_views {
        let kind = reference_view
            .get("view_kind")
            .and_then(Value::as_str)
            .filter(|kind| required_reference_kinds.contains(kind))
            .ok_or_else(|| "REGISTERED_CAMERA_RIG_V2_INVALID: reference kind".to_owned())?;
        let rotation = reference_view
            .get("rotation_degrees")
            .and_then(Value::as_i64)
            .filter(|value| [-180, -90, 0, 90, 180].contains(value));
        if !reference_kind_set.insert(kind)
            || reference_view
                .get("reference_view_id")
                .and_then(Value::as_str)
                .is_none_or(|id| !forgecad_contracts::is_opaque_id(id))
            || reference_view
                .get("reference_view_spec_canonical_sha256")
                .and_then(Value::as_str)
                .is_none_or(|hash| !forgecad_contracts::is_sha256(hash))
            || rotation.is_none()
        {
            return Err("REGISTERED_CAMERA_RIG_V2_INVALID: reference binding".to_owned());
        }
    }
    if reference_kind_set != required_reference_kinds.into_iter().collect() {
        return Err("REGISTERED_CAMERA_RIG_V2_INVALID: reference coverage".to_owned());
    }
    let rear_reference = reference_views
        .iter()
        .find(|view| view.get("view_kind").and_then(Value::as_str) == Some("rear-three-quarter"))
        .expect("rear-three-quarter reference validated");
    if rear_reference.get("reference_view_spec_canonical_sha256")
        != authored_orientation.get("reference_view_spec_canonical_sha256")
        || rear_reference.get("rotation_degrees")
            != authored_orientation.pointer("/reference_to_subject_view/rotation_degrees")
    {
        return Err("REGISTERED_CAMERA_RIG_V2_INVALID: authored rear view mismatch".to_owned());
    }
    let source_views = registered_rig_v1["renderer_views"]
        .as_array()
        .ok_or_else(|| "REGISTERED_CAMERA_RIG_V2_INVALID: source views".to_owned())?;
    let mut renderer_views = Vec::with_capacity(source_views.len());
    for source in source_views {
        let kind = source
            .get("kind")
            .and_then(Value::as_str)
            .ok_or_else(|| "REGISTERED_CAMERA_RIG_V2_INVALID: source kind".to_owned())?;
        let reference_view = reference_views
            .iter()
            .find(|view| view.get("view_kind").and_then(Value::as_str) == Some(kind));
        if kind != "bottom" && reference_view.is_none() {
            return Err("REGISTERED_CAMERA_RIG_V2_INVALID: missing reference view".to_owned());
        }
        let rear_three_quarter = kind == "rear-three-quarter";
        let registered_camera = if rear_three_quarter {
            materialize_registered_camera_orbit(
                &source["registered_camera"],
                rear_three_quarter_camera_orbit_degrees,
            )?
        } else {
            source["registered_camera"].clone()
        };
        let rotation = if rear_three_quarter {
            authored_orientation["reference_to_subject_view"]["rotation_degrees"].clone()
        } else {
            reference_view
                .map(|view| view["rotation_degrees"].clone())
                .unwrap_or(Value::Null)
        };
        renderer_views.push(json!({
            "view_id":source["view_id"],
            "kind":kind,
            "registered_camera_hash":registered_camera["camera_hash"],
            "registered_camera":registered_camera,
            "reference_view_id":reference_view.map(|view| view["reference_view_id"].clone()).unwrap_or(Value::Null),
            "reference_view_spec_canonical_sha256":reference_view.map(|view| view["reference_view_spec_canonical_sha256"].clone()).unwrap_or(Value::Null),
            "authored_reference_rotation_degrees":rotation,
            "authored_subject_screen_order":if rear_three_quarter { authored_orientation["subject_screen_order"].clone() } else { Value::Null },
            "registered_camera_orbit_degrees":if rear_three_quarter { json!(rear_three_quarter_camera_orbit_degrees) } else { Value::Null },
            "orientation_authority":if kind == "bottom" { "camera-only" } else if rear_three_quarter { "authored-orientation-receipt" } else { "reference-view-spec" },
            "authored_orientation_canonical_sha256":if rear_three_quarter { authored_orientation["canonical_sha256"].clone() } else { Value::Null },
            "semantic_view_ordering_canonical_sha256":if kind == "bottom" { Value::Null } else { semantic_ordering["canonical_sha256"].clone() },
            "post_render_transform":"identity",
            "weight":source["weight"],
            "primary":source["primary"]
        }));
    }
    let rear_renderer_view = renderer_views
        .iter()
        .find(|view| view.get("kind").and_then(Value::as_str) == Some("rear-three-quarter"))
        .expect("rear-three-quarter renderer view validated");
    let semantic_orientation_proof = materialize_rear_three_quarter_semantic_orientation_proof(
        &rear_renderer_view["registered_camera"],
        semantic_ordering,
        authored_orientation,
    )?;
    let mut result = json!({
        "schema_version":REGISTERED_CAMERA_RIG_CALIBRATION_V2_SCHEMA,
        "registered_rig_v2_id":registered_rig_v2_id,
        "project_id":registered_rig_v1["project_id"],
        "candidate_id":registered_rig_v1["candidate_id"],
        "candidate_state_sha256":registered_rig_v1["candidate_state_sha256"],
        "artifact_id":registered_rig_v1["artifact_id"],
        "artifact_sha256":registered_rig_v1["artifact_sha256"],
        "camera_lock_id":camera_lock["camera_lock_id"],
        "camera_lock_canonical_sha256":camera_lock["canonical_sha256"],
        "registered_rig_v1":registered_rig_v1,
        "registered_rig_v1_canonical_sha256":registered_rig_v1["canonical_sha256"],
        "semantic_landmark_ordering":semantic_ordering,
        "semantic_landmark_ordering_object_sha256":semantic_ordering_object_sha256,
        "semantic_landmark_ordering_canonical_sha256":semantic_ordering["canonical_sha256"],
        "rear_three_quarter_authored_orientation":authored_orientation,
        "rear_three_quarter_authored_orientation_object_sha256":authored_orientation_object_sha256,
        "rear_three_quarter_authored_orientation_canonical_sha256":authored_orientation["canonical_sha256"],
        "rear_three_quarter_semantic_orientation_proof":semantic_orientation_proof,
        "renderer_views":renderer_views,
        "read_only":true,
        "runtime_write":false,
        "depth_status":"UNKNOWN",
        "quality_status":"NOT_EVALUATED",
        "production_stage_advanced":false,
        "candidate_confirmed":false,
        "version_created":false,
        "export_performed":false,
        "canonical_sha256":""
    });
    result["canonical_sha256"] = Value::String(canonical_json_hash(&result));
    Ok(result)
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

    fn subject_frame_program(stock_x: f64, muzzle_x: f64) -> Value {
        let inverted = stock_x > 0.0 && muzzle_x < 0.0;
        let side_left_z = if inverted { 0.47 } else { -0.47 };
        let side_right_z = -side_left_z;
        let mut program = json!({
            "schema_version":"GeometryProgram@2",
            "operator_catalog_sha256":"eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
            "nodes":[
                {"node_id":"rear-stock","parameters":{"position_m":[stock_x,0.0,0.0]}},
                {"node_id":"rear-stock-lower-beam","parameters":{"position_m":[stock_x,0.0,0.0]}},
                {"node_id":"muzzle-shroud","parameters":{"position_m":[muzzle_x,0.0,0.0]}},
                {"node_id":"muzzle-emitter","parameters":{"position_m":[muzzle_x,0.0,0.0]}},
                {"node_id":"muzzle-core","parameters":{"position_m":[muzzle_x,0.0,0.0]}},
                {"node_id":"side-light-left","parameters":{"position_m":[0.0,0.0,side_left_z]}},
                {"node_id":"side-light-right","parameters":{"position_m":[0.0,0.0,side_right_z]}}
            ],
            "part_outputs":[
                {"part_id":"rear-stock","input_node_ids":["rear-stock","rear-stock-lower-beam"]},
                {"part_id":"muzzle-shroud","input_node_ids":["muzzle-shroud"]},
                {"part_id":"muzzle-emitter","input_node_ids":["muzzle-emitter"]},
                {"part_id":"muzzle-core","input_node_ids":["muzzle-core"]},
                {"part_id":"side-light-left","input_node_ids":["side-light-left"]},
                {"part_id":"side-light-right","input_node_ids":["side-light-right"]}
            ]
        });
        program["canonical_sha256"] = Value::String(canonical_json_hash(&program));
        program
    }

    fn rig_with_views() -> Value {
        let kinds = [
            "left",
            "right",
            "top",
            "bottom",
            "front",
            "back",
            "rear-three-quarter",
        ];
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
    fn production_weapon_subject_frame_registration_is_closed_and_hash_stable() {
        let canonical = subject_frame_program(-2.0, 2.0);
        let identity = production_weapon_subject_frame_registration(&canonical)
            .expect("canonical semantic axes");
        assert_eq!(
            identity.pointer("/transform/kind"),
            Some(&json!("identity"))
        );
        assert_eq!(
            identity.pointer("/transform/rotation_rad"),
            Some(&json!([0.0, 0.0, 0.0]))
        );
        assert_eq!(identity["read_only"], true);
        assert_eq!(identity["geometry_program_modified"], false);
        assert_eq!(identity["depth_modified"], false);
        validate_production_weapon_subject_frame_registration(&identity, &canonical)
            .expect("identity registration projection");

        let inverted = subject_frame_program(2.0, -2.0);
        let first = production_weapon_subject_frame_registration(&inverted)
            .expect("inverted semantic axes");
        let second = production_weapon_subject_frame_registration(&inverted)
            .expect("stable inverted semantic axes");
        assert_eq!(first, second);
        assert_eq!(first.pointer("/transform/kind"), Some(&json!("yaw-180-y")));
        assert_eq!(
            first.pointer("/transform/rotation_rad/1"),
            Some(&json!(std::f64::consts::PI))
        );
        validate_production_weapon_subject_frame_registration(&first, &inverted)
            .expect("yaw registration projection");
    }

    #[test]
    fn production_weapon_subject_frame_registration_rejects_ambiguous_or_tampered_inputs() {
        let mut hash_tampered = subject_frame_program(-2.0, 2.0);
        hash_tampered["canonical_sha256"] = Value::String("b".repeat(64));
        let error = production_weapon_subject_frame_registration(&hash_tampered)
            .expect_err("caller-supplied program hash cannot replace content truth");
        assert!(error.contains("canonical hash mismatch"));

        let ambiguous = subject_frame_program(2.0, 2.0);
        let error = production_weapon_subject_frame_registration(&ambiguous)
            .expect_err("anchors on the same axis side are ambiguous");
        assert!(error.contains("BLOCKED"));

        let inverted = subject_frame_program(2.0, -2.0);
        let mut registration = production_weapon_subject_frame_registration(&inverted)
            .expect("inverted semantic axes");
        registration["transform"]["kind"] = Value::String("identity".to_owned());
        let error = validate_production_weapon_subject_frame_registration(&registration, &inverted)
            .expect_err("registration cannot be authored outside the closed derivation");
        assert!(error.contains("differs from exact derived projection"));

        let mut wrong_binding = inverted;
        wrong_binding["part_outputs"][0]["input_node_ids"] = json!(["rear-stock"]);
        let mut wrong_binding_without_hash = wrong_binding.clone();
        wrong_binding_without_hash
            .as_object_mut()
            .expect("program")
            .remove("canonical_sha256");
        wrong_binding["canonical_sha256"] =
            Value::String(canonical_json_hash(&wrong_binding_without_hash));
        let error = production_weapon_subject_frame_registration(&wrong_binding)
            .expect_err("semantic anchors require exact PartOutput bindings");
        assert!(error.contains("source binding"));
    }

    #[test]
    fn registered_weapon_camera_materializes_subject_views_without_geometry_write() {
        let program = subject_frame_program(2.0, -2.0);
        let registration =
            production_weapon_subject_frame_registration(&program).expect("inverted semantic axes");

        let left = inferred_weapon_camera("left", 2.4).expect("left subject camera");
        let registered_left = materialize_registered_weapon_camera(&left, &registration, &program)
            .expect("registered left camera");
        assert_eq!(
            registered_left.pointer("/transform/position_m"),
            Some(&json!([0.0, 0.0, 20.0]))
        );
        assert_eq!(
            registered_left.pointer("/transform/up"),
            Some(&json!([0.0, 1.0, 0.0]))
        );
        assert_ne!(registered_left["camera_hash"], left["camera_hash"]);
        validate_camera_calibration_v2(&registered_left).expect("registered camera contract");

        let front = inferred_weapon_camera("front", 2.4).expect("front subject camera");
        let registered_front =
            materialize_registered_weapon_camera(&front, &registration, &program)
                .expect("registered front camera");
        assert_eq!(
            registered_front.pointer("/transform/position_m"),
            Some(&json!([-20.0, 0.0, 0.0]))
        );

        let top = inferred_weapon_camera("top", 2.4).expect("top subject camera");
        let registered_top = materialize_registered_weapon_camera(&top, &registration, &program)
            .expect("registered top camera");
        assert_eq!(
            registered_top.pointer("/transform/up"),
            Some(&json!([0.0, 0.0, 1.0]))
        );
        assert_eq!(registration["geometry_program_modified"], false);
        assert_eq!(registration["depth_modified"], false);
    }

    #[test]
    fn registered_weapon_camera_rig_retains_subject_truth_and_rejects_lineage_tamper() {
        let program = subject_frame_program(2.0, -2.0);
        let registration =
            production_weapon_subject_frame_registration(&program).expect("inverted semantic axes");
        let subject_rig = rig_with_views();
        let registered = materialize_registered_weapon_camera_rig(
            &subject_rig,
            &registration,
            &program,
            "registered-weapon-rig",
            &"a".repeat(64),
            "artifact",
            &"b".repeat(64),
            &"c".repeat(64),
            &"d".repeat(64),
        )
        .expect("registered rig");
        assert_eq!(
            registered["schema_version"],
            REGISTERED_CAMERA_RIG_CALIBRATION_SCHEMA
        );
        assert_eq!(registered["subject_camera_rig"], subject_rig);
        assert_eq!(
            registered["subject_camera_rig_canonical_sha256"],
            subject_rig["canonical_sha256"]
        );
        assert_eq!(registered["subject_frame_registration"], registration);
        assert_eq!(registered["read_only"], true);
        assert_eq!(registered["runtime_write"], false);
        assert_eq!(registered["depth_status"], "UNKNOWN");
        assert_eq!(registered["quality_status"], "NOT_EVALUATED");
        assert_eq!(registered["production_stage_advanced"], false);
        assert_eq!(registered["candidate_confirmed"], false);
        assert_eq!(registered["version_created"], false);
        assert_eq!(registered["export_performed"], false);
        assert_eq!(
            registered["renderer_views"].as_array().map(Vec::len),
            Some(7)
        );
        validate_registered_weapon_camera_rig(
            &registered,
            &program,
            "project",
            "candidate",
            &"a".repeat(64),
            "artifact",
            &"b".repeat(64),
            &"c".repeat(64),
            &"d".repeat(64),
        )
        .expect("exact registered rig lineage");

        let camera_lock = json!({
            "schema_version":"ProductionCameraLock@1",
            "project_id":"project",
            "candidate_id":"candidate",
            "candidate_state_sha256":"a".repeat(64),
            "artifact_id":"artifact",
            "artifact_sha256":"b".repeat(64),
            "camera_rig_object_sha256":"d".repeat(64),
            "camera_rig_canonical_sha256":subject_rig["canonical_sha256"],
            "required_camera_view_kinds":["front","back","left","right","top","bottom","rear-three-quarter"],
            "primary_view_kind":"left",
            "calibration_status":"passed",
            "visual_status":"QUALITY_TARGET_NOT_MET",
            "human_status":"NOT_RUN"
        });
        validate_registered_weapon_camera_rig_camera_lock_link(&registered, &camera_lock)
            .expect("exact CameraLock sibling link");
        let mut stale_lock = camera_lock.clone();
        stale_lock["candidate_state_sha256"] = Value::String("f".repeat(64));
        let error =
            validate_registered_weapon_camera_rig_camera_lock_link(&registered, &stale_lock)
                .expect_err("stale CameraLock head cannot link");
        assert!(error.contains("candidate_state_sha256 mismatch"));

        let mut wrong_primary = registered.clone();
        for view in wrong_primary["renderer_views"].as_array_mut().unwrap() {
            view["primary"] = Value::Bool(view["kind"].as_str() == Some("front"));
        }
        wrong_primary["canonical_sha256"] = Value::String(canonical_json_hash(&wrong_primary));
        let error =
            validate_registered_weapon_camera_rig_camera_lock_link(&wrong_primary, &camera_lock)
                .expect_err("registered primary must remain the CameraLock left view");
        assert!(error.contains("primary view binding"));

        let mut camera_tampered = registered.clone();
        camera_tampered["renderer_views"][0]["registered_camera"]["transform"]["position_m"] =
            json!([1.0, 2.0, 3.0]);
        camera_tampered["canonical_sha256"] = Value::String(canonical_json_hash(&camera_tampered));
        let error = validate_registered_weapon_camera_rig(
            &camera_tampered,
            &program,
            "project",
            "candidate",
            &"a".repeat(64),
            "artifact",
            &"b".repeat(64),
            &"c".repeat(64),
            &"d".repeat(64),
        )
        .expect_err("renderer camera cannot drift from exact registration");
        assert!(error.contains("lineage differs"));

        let mut cross_artifact = registered;
        cross_artifact["artifact_sha256"] = Value::String("c".repeat(64));
        cross_artifact["canonical_sha256"] = Value::String(canonical_json_hash(&cross_artifact));
        let error = validate_registered_weapon_camera_rig(
            &cross_artifact,
            &program,
            "project",
            "candidate",
            &"a".repeat(64),
            "artifact",
            &"b".repeat(64),
            &"c".repeat(64),
            &"d".repeat(64),
        )
        .expect_err("artifact retarget requires a new exact lineage projection");
        assert!(error.contains("expected lineage mismatch"));
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
