//! Projection-driven animated socket trails.
//!
//! This is a bounded, structural-only producer.  The Runtime derives the
//! local history and world-space trail inventory itself, asks the sibling
//! Worker to render it twice, and only then reserves the five per-frame CAS
//! objects represented by the durable sequence contract.

use super::{
    canonical_json_bytes, canonical_json_hash, exact_object, is_opaque_id, is_sha256, now_string,
    render_worker, sha256_hex, CasObject, Runtime, RuntimeError,
};
use forgecad_contracts::{
    FictionalEnergyVfxAnimatedSocketParticlesSequence, FictionalEnergyVfxAnimatedSocketTrail,
    FictionalEnergyVfxAnimatedSocketTrailPoint,
    FictionalEnergyVfxAnimatedSocketTrailsHistorySample,
    FictionalEnergyVfxAnimatedSocketTrailsSequence,
    FictionalEnergyVfxAnimatedSocketTrailsSequenceFrame,
    FictionalEnergyVfxAnimatedSocketTrailsSequenceFrameInput,
    FictionalEnergyVfxAnimatedSocketTrailsSequenceGetRequest,
    FictionalEnergyVfxAnimatedSocketTrailsSequencePrepareRequest,
    FictionalEnergyVfxBloomFrameLinkRecord, FictionalEnergyVfxFrameLinkRecord,
};
use serde_json::{json, Map, Value};

const PREPARE_SCHEMA: &str = "FictionalEnergyVfxAnimatedSocketTrailsSequencePrepareRequest@1";
const GET_SCHEMA: &str = "FictionalEnergyVfxAnimatedSocketTrailsSequenceGetRequest@1";
const PREPARE_RESULT_SCHEMA: &str = "FictionalEnergyVfxAnimatedSocketTrailsSequencePrepareResult@1";
const GET_RESULT_SCHEMA: &str = "FictionalEnergyVfxAnimatedSocketTrailsSequenceGetResult@1";
const SEQUENCE_SCHEMA: &str = "FictionalEnergyVfxAnimatedSocketTrailsSequence@1";
const FRAME_SCHEMA: &str = "FictionalEnergyVfxAnimatedSocketTrailsSequenceFrame@1";
const FRAME_RECEIPT_SCHEMA: &str = "FictionalEnergyVfxAnimatedSocketTrailsFrameReceipt@1";
const RENDER_SET_SCHEMA: &str = "FictionalEnergyVfxAnimatedSocketTrailsRenderSet@1";
const TRAIL_INVENTORY_SCHEMA: &str = "RenderWorkerAnimatedSocketTrailInventory@1";
const TRAIL_EMITTER_SCHEMA: &str = "RenderWorkerAnimatedSocketTrailEmitterBindings@1";
const TRAIL_SAMPLE_SET_SCHEMA: &str = "RenderWorkerAnimatedSocketTrailProjectionSamples@1";
const FRAME_SCOPE: &str = "lod0-animation-trails-source-frames-1-15@1";
const TRAILS_POLICY: &str = "projection-driven-animated-socket-trails@1";
const HISTORY_POLICY: &str =
    "one-to-eight-oldest-to-newest-ordinal-zero-last-immediate-previous-earlier-particle-frames-plus-current@1";
const HISTORY_PREROLL_POLICY: &str =
    "same-parent-sequence-source-frame-zero-is-preroll-output-frames-one-to-fifteen@1";
const STATUS: &str = "runtime-owned-durable-fictional-energy-vfx-animated-socket-trails-sequence";
const MAX_JSON_BYTES: u64 = 1024 * 1024;
const MAX_GLB_BYTES: u64 = 64 * 1024 * 1024;
const MAX_FRAMES: usize = 15;

const RENDER_SET_KIND: &str = "fictional-energy-vfx-animated-socket-trails-render-set";
const FRAME_RECEIPT_KIND: &str = "fictional-energy-vfx-animated-socket-trails-frame-receipt";
const COLOR_KIND: &str = "fictional-energy-vfx-animated-socket-trails-trail-color";
const ID_KIND: &str = "fictional-energy-vfx-animated-socket-trails-trail-id";
const DEPTH_KIND: &str = "fictional-energy-vfx-animated-socket-trails-trail-depth";

const PREPARE_FIELDS: &[&str] = &[
    "schema_version",
    "sequence_key_sha256",
    "project_id",
    "candidate_id",
    "candidate_state_sha256",
    "delivery_manifest_object_sha256",
    "source_artifact_sha256",
    "projection_key_sha256",
    "projection_object_sha256",
    "projection_canonical_sha256",
    "animated_socket_materialization_key_sha256",
    "animated_artifact_sha256",
    "animated_socket_anchor_set_object_sha256",
    "animated_socket_anchor_set_canonical_sha256",
    "animation_clip_id",
    "animation_clip_object_sha256",
    "animation_clip_canonical_sha256",
    "animation_receipt_object_sha256",
    "animation_receipt_canonical_sha256",
    "vfx_profile_object_sha256",
    "vfx_profile_canonical_sha256",
    "socket_node_id_encoding_sha256",
    "socket_roles_sha256",
    "camera_object_sha256",
    "camera_identity_sha256",
    "render_profile_sha256",
    "render_worker_build_cohort_sha256",
    "sample_schedule_sha256",
    "sample_count",
    "sample_time_ticks",
    "frame_scope",
    "trails_sequence_policy",
    "history_policy",
    "history_pre_roll_policy",
    "trail_count",
    "trail_emitter_roles",
    "frames",
    "input_sha256",
    "idempotency_key",
];
const GET_FIELDS: &[&str] = &[
    "schema_version",
    "sequence_key_sha256",
    "project_id",
    "candidate_id",
];

fn invalid(detail: impl Into<String>) -> RuntimeError {
    RuntimeError::InvalidInput(format!(
        "FICTIONAL_ENERGY_VFX_ANIMATED_SOCKET_TRAILS_INVALID: {}",
        detail.into()
    ))
}

fn text<'a>(object: &'a Map<String, Value>, field: &str) -> Result<&'a str, RuntimeError> {
    object
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| invalid(format!("{field} is invalid")))
}

fn sha<'a>(object: &'a Map<String, Value>, field: &str) -> Result<&'a str, RuntimeError> {
    let value = text(object, field)?;
    if !is_sha256(value) {
        return Err(invalid(format!("{field} is not a SHA-256")));
    }
    Ok(value)
}

fn id<'a>(object: &'a Map<String, Value>, field: &str) -> Result<&'a str, RuntimeError> {
    let value = text(object, field)?;
    if !is_opaque_id(value) {
        return Err(invalid(format!("{field} is not an opaque identifier")));
    }
    Ok(value)
}

fn parse_prepare(
    value: &Value,
) -> Result<FictionalEnergyVfxAnimatedSocketTrailsSequencePrepareRequest, RuntimeError> {
    let object = exact_object(value, PREPARE_FIELDS, PREPARE_SCHEMA)?;
    if text(object, "schema_version")? != PREPARE_SCHEMA {
        return Err(invalid("prepare schema differs"));
    }
    let request: FictionalEnergyVfxAnimatedSocketTrailsSequencePrepareRequest =
        serde_json::from_value(value.clone())
            .map_err(|error| invalid(format!("prepare request is malformed: {error}")))?;
    for field in [
        "sequence_key_sha256",
        "candidate_state_sha256",
        "delivery_manifest_object_sha256",
        "source_artifact_sha256",
        "projection_key_sha256",
        "projection_object_sha256",
        "projection_canonical_sha256",
        "animated_socket_materialization_key_sha256",
        "animated_artifact_sha256",
        "animated_socket_anchor_set_object_sha256",
        "animated_socket_anchor_set_canonical_sha256",
        "animation_clip_object_sha256",
        "animation_clip_canonical_sha256",
        "animation_receipt_object_sha256",
        "animation_receipt_canonical_sha256",
        "vfx_profile_object_sha256",
        "vfx_profile_canonical_sha256",
        "socket_node_id_encoding_sha256",
        "socket_roles_sha256",
        "camera_object_sha256",
        "camera_identity_sha256",
        "render_profile_sha256",
        "render_worker_build_cohort_sha256",
        "sample_schedule_sha256",
        "input_sha256",
    ] {
        sha(object, field)?;
    }
    for field in [
        "project_id",
        "candidate_id",
        "animation_clip_id",
        "idempotency_key",
    ] {
        id(object, field)?;
    }
    if request.schema_version != PREPARE_SCHEMA
        || request.frame_scope != FRAME_SCOPE
        || request.trails_sequence_policy != TRAILS_POLICY
        || request.history_policy != HISTORY_POLICY
        || request.history_pre_roll_policy != HISTORY_PREROLL_POLICY
        || request.sample_count == 0
        || request.sample_count as usize > MAX_FRAMES
        || request.sample_time_ticks.len() != request.sample_count as usize
        || request.frames.len() != request.sample_count as usize
        || request
            .sample_time_ticks
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        || request
            .sample_time_ticks
            .iter()
            .any(|tick| *tick > 1_000_000)
        || request.trail_count != 2
        || request.trail_emitter_roles != vec!["muzzle-vfx", "energy-core-vfx"]
    {
        return Err(invalid("sequence policy or bounded schedule differs"));
    }
    for (ordinal, frame) in request.frames.iter().enumerate() {
        validate_frame_input(frame, ordinal, request.sample_time_ticks[ordinal])?;
    }
    let mut preimage = object.clone();
    preimage.remove("sequence_key_sha256");
    preimage.remove("input_sha256");
    preimage.remove("idempotency_key");
    let expected = canonical_json_hash(&Value::Object(preimage));
    if request.input_sha256 != expected || request.sequence_key_sha256 != expected {
        return Err(invalid("sequence input/key hash differs"));
    }
    Ok(request)
}

fn parse_get(
    value: &Value,
) -> Result<FictionalEnergyVfxAnimatedSocketTrailsSequenceGetRequest, RuntimeError> {
    let object = exact_object(value, GET_FIELDS, GET_SCHEMA)?;
    if text(object, "schema_version")? != GET_SCHEMA {
        return Err(invalid("get schema differs"));
    }
    let request: FictionalEnergyVfxAnimatedSocketTrailsSequenceGetRequest =
        serde_json::from_value(value.clone())
            .map_err(|error| invalid(format!("get request is malformed: {error}")))?;
    sha(object, "sequence_key_sha256")?;
    id(object, "project_id")?;
    id(object, "candidate_id")?;
    Ok(request)
}

fn validate_frame_input(
    frame: &FictionalEnergyVfxAnimatedSocketTrailsSequenceFrameInput,
    ordinal: usize,
    sample_time_ticks: u64,
) -> Result<(), RuntimeError> {
    if frame.frame_index != ordinal as u64
        || frame.sample_time_ticks != sample_time_ticks
        || frame.history_origin != HISTORY_PREROLL_POLICY
        || frame.current_projection_frame_index != ordinal as u64 + 1
        || frame.current_particle_frame_index != ordinal as u64 + 1
        || frame.previous_projection_frame_index != ordinal as u64
        || frame.previous_particle_frame_index != ordinal as u64
        || !is_sha256(&frame.current_particle_key_sha256)
        || !is_sha256(&frame.current_particle_frame_canonical_sha256)
        || !is_sha256(&frame.current_projection_frame_canonical_sha256)
        || !is_sha256(&frame.current_projection_socket_transform_inventory_sha256)
        || !is_sha256(&frame.current_projection_socket_transform_readback_sha256)
        || !is_sha256(&frame.previous_particle_sequence_frame_canonical_sha256)
        || !is_sha256(&frame.previous_projection_frame_canonical_sha256)
        || !is_sha256(&frame.previous_projection_socket_transform_inventory_sha256)
        || !is_sha256(&frame.previous_projection_socket_transform_readback_sha256)
        || !is_sha256(&frame.particle_sequence_key_sha256)
        || !is_sha256(&frame.base_frame_key_sha256)
        || !is_sha256(&frame.bloom_key_sha256)
        || !is_sha256(&frame.camera_object_sha256)
        || !is_sha256(&frame.camera_identity_sha256)
        || !is_sha256(&frame.render_profile_sha256)
        || !is_sha256(&frame.render_worker_build_cohort_sha256)
    {
        return Err(invalid("trail frame input binding differs"));
    }
    Ok(())
}

fn read_canonical_json(runtime: &Runtime, hash: &str, schema: &str) -> Result<Value, RuntimeError> {
    let bytes = runtime.cas_read_bounded(hash, MAX_JSON_BYTES)?;
    let value: Value = serde_json::from_slice(&bytes)
        .map_err(|error| invalid(format!("CAS JSON is malformed: {error}")))?;
    if value.get("schema_version").and_then(Value::as_str) != Some(schema) {
        return Err(invalid(format!("CAS JSON schema differs from {schema}")));
    }
    Ok(value)
}

pub(super) fn canonical_object(mut value: Value) -> Result<(Value, Vec<u8>), RuntimeError> {
    value
        .as_object_mut()
        .ok_or_else(|| invalid("CAS sidecar must be an object"))?
        .insert("canonical_sha256".to_owned(), Value::String(String::new()));
    let canonical = canonical_json_hash(&value);
    value["canonical_sha256"] = Value::String(canonical);
    let bytes = canonical_json_bytes(&value).map_err(|error| invalid(error.to_string()))?;
    Ok((value, bytes))
}

pub(super) fn read_owned_canonical_json(
    runtime: &Runtime,
    hash: &str,
    schema: &str,
) -> Result<Value, RuntimeError> {
    let value = read_canonical_json(runtime, hash, schema)?;
    verify_owned_canonical_json(&value)?;
    Ok(value)
}

fn verify_owned_canonical_json(value: &Value) -> Result<(), RuntimeError> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid("owned CAS JSON must be an object"))?;
    let stored_canonical = object
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .ok_or_else(|| invalid("owned CAS JSON canonical hash is missing"))?;
    let mut preimage = object.clone();
    preimage.insert("canonical_sha256".to_owned(), Value::String(String::new()));
    if canonical_json_hash(&Value::Object(preimage)) != stored_canonical {
        return Err(invalid("owned CAS JSON canonical hash differs"));
    }
    Ok(())
}

fn expect_same(value: &Value, field: &str, expected: &str) -> Result<(), RuntimeError> {
    if value.get(field).and_then(Value::as_str) != Some(expected) {
        return Err(invalid(format!("dependency field {field} differs")));
    }
    Ok(())
}

fn f32_array(
    value: &Value,
    field: &str,
    length: usize,
    max: f32,
) -> Result<Vec<f32>, RuntimeError> {
    let values = value
        .get(field)
        .and_then(Value::as_array)
        .filter(|values| values.len() == length)
        .ok_or_else(|| invalid(format!("{field} must contain exactly {length} values")))?;
    values
        .iter()
        .map(|value| {
            let number = value
                .as_f64()
                .filter(|number| number.is_finite() && number.abs() <= f64::from(max))
                .ok_or_else(|| invalid(format!("{field} contains an invalid number")))?;
            Ok(number as f32)
        })
        .collect()
}

fn f32_value(values: &[f32]) -> Value {
    Value::Array(values.iter().map(|value| json!(value)).collect())
}

fn vector_cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn vector_scale(a: [f32; 3], factor: f32) -> [f32; 3] {
    [a[0] * factor, a[1] * factor, a[2] * factor]
}

fn transform_point(translation: [f32; 3], rotation: [f32; 4], local: [f32; 3]) -> [f32; 3] {
    let q = [rotation[0], rotation[1], rotation[2]];
    let twice_cross = vector_scale(vector_cross(q, local), 2.0);
    [
        translation[0] + twice_cross[0] * rotation[3] + vector_cross(q, twice_cross)[0],
        translation[1] + twice_cross[1] * rotation[3] + vector_cross(q, twice_cross)[1],
        translation[2] + twice_cross[2] * rotation[3] + vector_cross(q, twice_cross)[2],
    ]
}

fn normalize(value: [f32; 3]) -> Result<[f32; 3], RuntimeError> {
    let length = (value[0] * value[0] + value[1] * value[1] + value[2] * value[2]).sqrt();
    if !length.is_finite() || length <= 1e-6 {
        return Err(invalid("camera basis is degenerate"));
    }
    Ok([value[0] / length, value[1] / length, value[2] / length])
}

fn camera_depth(camera: &Value, position: [f32; 3]) -> Result<f32, RuntimeError> {
    let transform = camera
        .get("transform")
        .ok_or_else(|| invalid("camera transform is unavailable"))?;
    let camera_position = f32_array(transform, "position_m", 3, 1_000.0)?;
    let target = f32_array(transform, "target_m", 3, 1_000.0)?;
    let up = f32_array(transform, "up", 3, 1_000.0)?;
    let camera_position = [camera_position[0], camera_position[1], camera_position[2]];
    let target = [target[0], target[1], target[2]];
    let up = [up[0], up[1], up[2]];
    let forward = normalize([
        target[0] - camera_position[0],
        target[1] - camera_position[1],
        target[2] - camera_position[2],
    ])?;
    let right = normalize(vector_cross(forward, up))?;
    let _ = normalize(vector_cross(right, forward))?;
    let near = camera
        .get("near_m")
        .and_then(Value::as_f64)
        .filter(|v| v.is_finite())
        .map(|v| v as f32)
        .ok_or_else(|| invalid("camera near is invalid"))?;
    let far = camera
        .get("far_m")
        .and_then(Value::as_f64)
        .filter(|v| v.is_finite())
        .map(|v| v as f32)
        .ok_or_else(|| invalid("camera far is invalid"))?;
    let relative = [
        position[0] - camera_position[0],
        position[1] - camera_position[1],
        position[2] - camera_position[2],
    ];
    let z = relative[0] * forward[0] + relative[1] * forward[1] + relative[2] * forward[2];
    if !(near > 0.0 && far > near && z > near && z < far) {
        return Err(invalid("trail point is outside camera clip range"));
    }
    let depth = (z - near) / (far - near);
    if !depth.is_finite() || !(0.0..=1.0).contains(&depth) {
        return Err(invalid("trail camera depth is invalid"));
    }
    Ok(depth)
}

fn projection_socket<'a>(
    frame: &'a Value,
    role: &str,
    node_id: &str,
    owner_part_id: &str,
) -> Result<&'a Value, RuntimeError> {
    let sockets = frame
        .get("socket_transforms")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("projection socket transforms are unavailable"))?;
    let socket = sockets
        .iter()
        .find(|socket| socket.get("role").and_then(Value::as_str) == Some(role))
        .ok_or_else(|| invalid(format!("projection role {role} is unavailable")))?;
    if socket.get("socket_node_id").and_then(Value::as_str) != Some(node_id)
        || socket.get("anchor_id").and_then(Value::as_str) != Some(node_id)
        || socket.get("owner_part_id").and_then(Value::as_str) != Some(owner_part_id)
        || socket.get("node_kind").and_then(Value::as_str) != Some("empty")
    {
        return Err(invalid(format!("projection role {role} is retargeted")));
    }
    let transform = socket
        .get("composed_world_transform")
        .ok_or_else(|| invalid("projection composed transform is unavailable"))?;
    let translation = f32_array(transform, "translation_m", 3, 100.0)?;
    let rotation = f32_array(transform, "rotation_quat_xyzw", 4, 1.0)?;
    let scale = f32_array(transform, "scale_xyz", 3, 2.0)?;
    let norm = rotation
        .iter()
        .map(|value| value * value)
        .sum::<f32>()
        .sqrt();
    if (norm - 1.0).abs() > 1e-3 || scale.iter().any(|value| (*value - 1.0).abs() > 1e-6) {
        return Err(invalid("projection composed transform is not unit TRS"));
    }
    let _ = translation;
    Ok(socket)
}

fn emitter_bindings(frame: &Value) -> Result<Value, RuntimeError> {
    let definitions = [
        (
            "muzzle-trail",
            "socket-muzzle-vfx",
            "muzzle-vfx",
            "barrel-assembly",
        ),
        (
            "energy-core-trail",
            "socket-energy-core-vfx",
            "energy-core-vfx",
            "energy-core",
        ),
    ];
    let mut emitters = Vec::with_capacity(2);
    for (emitter_id, node_id, role, owner_part_id) in definitions {
        let socket = projection_socket(frame, role, node_id, owner_part_id)?;
        let transform = socket
            .get("composed_world_transform")
            .ok_or_else(|| invalid("projection transform is unavailable"))?;
        let translation = f32_array(transform, "translation_m", 3, 100.0)?;
        let rotation = f32_array(transform, "rotation_quat_xyzw", 4, 1.0)?;
        let scale = f32_array(transform, "scale_xyz", 3, 2.0)?;
        emitters.push(json!({
            "emitter_id":emitter_id,
            "socket_node_id":node_id,
            "anchor_id":node_id,
            "role":role,
            "owner_part_id":owner_part_id,
            "composed_world_transform":{
                "translation_m":f32_value(&translation),
                "rotation_quat_xyzw":f32_value(&rotation),
                "scale_xyz":f32_value(&scale)
            }
        }));
    }
    Ok(json!({"schema_version":TRAIL_EMITTER_SCHEMA,"emitters":emitters}))
}

fn emitter_transform(bindings: &Value, index: usize) -> Result<([f32; 3], [f32; 4]), RuntimeError> {
    let emitter = bindings
        .get("emitters")
        .and_then(Value::as_array)
        .and_then(|values| values.get(index))
        .ok_or_else(|| invalid("emitter binding is unavailable"))?;
    let transform = emitter
        .get("composed_world_transform")
        .ok_or_else(|| invalid("emitter transform is unavailable"))?;
    let translation = f32_array(transform, "translation_m", 3, 100.0)?;
    let rotation = f32_array(transform, "rotation_quat_xyzw", 4, 1.0)?;
    Ok((
        [translation[0], translation[1], translation[2]],
        [rotation[0], rotation[1], rotation[2], rotation[3]],
    ))
}

fn projection_frame<'a>(projection: &'a Value, index: usize) -> Result<&'a Value, RuntimeError> {
    projection
        .get("frames")
        .and_then(Value::as_array)
        .and_then(|frames| frames.get(index))
        .ok_or_else(|| invalid(format!("projection frame {index} is unavailable")))
}

fn particle_frame<'a>(
    sequence: &'a FictionalEnergyVfxAnimatedSocketParticlesSequence,
    index: usize,
) -> Result<
    &'a forgecad_contracts::FictionalEnergyVfxAnimatedSocketParticlesSequenceFrame,
    RuntimeError,
> {
    sequence
        .frames
        .get(index)
        .ok_or_else(|| invalid(format!("particle frame {index} is unavailable")))
}

fn particle_local_offset(
    runtime: &Runtime,
    particle_frame: &forgecad_contracts::FictionalEnergyVfxAnimatedSocketParticlesSequenceFrame,
    source_id: u64,
) -> Result<[f32; 3], RuntimeError> {
    let receipt = read_canonical_json(
        runtime,
        &particle_frame.receipt_object_sha256,
        "FictionalEnergyVfxAnimatedSocketParticlesFrameReceipt@1",
    )?;
    let values = receipt
        .get("particles")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("particle receipt inventory is unavailable"))?;
    let particle = values
        .iter()
        .find(|value| value.get("id").and_then(Value::as_u64) == Some(source_id))
        .ok_or_else(|| {
            invalid(format!(
                "particle {source_id} is unavailable in source receipt"
            ))
        })?;
    let local = f32_array(particle, "local_offset_m", 3, 10.0)?;
    Ok([local[0], local[1], local[2]])
}

fn camera_for_base(
    runtime: &Runtime,
    request: &FictionalEnergyVfxAnimatedSocketTrailsSequencePrepareRequest,
    base: &FictionalEnergyVfxFrameLinkRecord,
) -> Result<Value, RuntimeError> {
    if base.project_id != request.project_id
        || base.delivery_manifest_object_sha256 != request.delivery_manifest_object_sha256
        || base.source_candidate_id != request.candidate_id
        || base.source_artifact_sha256 != request.source_artifact_sha256
        || base.camera_object_sha256 != request.camera_object_sha256
        || base.camera_identity_sha256 != request.camera_identity_sha256
        || base.render_profile_sha256 != request.render_profile_sha256
        || base.render_worker_build_cohort_sha256 != request.render_worker_build_cohort_sha256
    {
        return Err(invalid("base frame camera/cohort binding differs"));
    }
    let camera = read_canonical_json(runtime, &base.camera_object_sha256, "CameraCalibration@1")
        .or_else(|_| {
            read_canonical_json(runtime, &base.camera_object_sha256, "CameraCalibration@2")
        })?;
    if camera
        .get("camera_identity_sha256")
        .and_then(Value::as_str)
        .is_some_and(|value| value != request.camera_identity_sha256)
    {
        return Err(invalid("camera identity differs"));
    }
    Ok(camera)
}

fn validate_projection_parent(
    request: &FictionalEnergyVfxAnimatedSocketTrailsSequencePrepareRequest,
    projection: &Value,
) -> Result<String, RuntimeError> {
    for (field, expected) in [
        ("project_id", request.project_id.as_str()),
        ("candidate_id", request.candidate_id.as_str()),
        (
            "candidate_state_sha256",
            request.candidate_state_sha256.as_str(),
        ),
        (
            "delivery_manifest_object_sha256",
            request.delivery_manifest_object_sha256.as_str(),
        ),
        (
            "source_artifact_sha256",
            request.source_artifact_sha256.as_str(),
        ),
        (
            "projection_key_sha256",
            request.projection_key_sha256.as_str(),
        ),
        (
            "projection_canonical_sha256",
            request.projection_canonical_sha256.as_str(),
        ),
        (
            "animated_socket_materialization_key_sha256",
            request.animated_socket_materialization_key_sha256.as_str(),
        ),
        (
            "animated_artifact_sha256",
            request.animated_artifact_sha256.as_str(),
        ),
        (
            "anchor_set_object_sha256",
            request.animated_socket_anchor_set_object_sha256.as_str(),
        ),
        (
            "anchor_set_canonical_sha256",
            request.animated_socket_anchor_set_canonical_sha256.as_str(),
        ),
        ("animation_clip_id", request.animation_clip_id.as_str()),
        (
            "animation_clip_object_sha256",
            request.animation_clip_object_sha256.as_str(),
        ),
        (
            "animation_clip_canonical_sha256",
            request.animation_clip_canonical_sha256.as_str(),
        ),
        (
            "animation_receipt_object_sha256",
            request.animation_receipt_object_sha256.as_str(),
        ),
        (
            "animation_receipt_canonical_sha256",
            request.animation_receipt_canonical_sha256.as_str(),
        ),
        (
            "socket_node_id_encoding_sha256",
            request.socket_node_id_encoding_sha256.as_str(),
        ),
        ("socket_roles_sha256", request.socket_roles_sha256.as_str()),
        (
            "sample_schedule_sha256",
            request.sample_schedule_sha256.as_str(),
        ),
    ] {
        expect_same(projection, field, expected)?;
    }
    if projection
        .get("sample_count")
        .and_then(Value::as_u64)
        .is_none_or(|count| count < request.sample_count + 1)
        || projection
            .get("frames")
            .and_then(Value::as_array)
            .is_none_or(|frames| frames.len() < request.sample_count as usize + 1)
        || projection
            .get("sample_time_ticks")
            .and_then(Value::as_array)
            .is_none_or(|ticks| ticks.len() < request.sample_count as usize + 1)
    {
        return Err(invalid("projection sample schedule differs"));
    }
    projection
        .get("input_sha256")
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .map(str::to_owned)
        .ok_or_else(|| invalid("projection input hash is unavailable"))
}

fn validate_base_bloom(
    request: &FictionalEnergyVfxAnimatedSocketTrailsSequencePrepareRequest,
    input: &FictionalEnergyVfxAnimatedSocketTrailsSequenceFrameInput,
    base: &FictionalEnergyVfxFrameLinkRecord,
    bloom: &FictionalEnergyVfxBloomFrameLinkRecord,
) -> Result<(), RuntimeError> {
    if base.frame_key_sha256 != input.base_frame_key_sha256
        || bloom.bloom_key_sha256 != input.bloom_key_sha256
        || bloom.base_frame_key_sha256 != base.frame_key_sha256
        || base.project_id != request.project_id
        || bloom.project_id != request.project_id
        || base.delivery_manifest_object_sha256 != request.delivery_manifest_object_sha256
        || bloom.delivery_manifest_object_sha256 != request.delivery_manifest_object_sha256
        || base.source_candidate_id != request.candidate_id
        || bloom.source_candidate_id != request.candidate_id
        || base.source_artifact_sha256 != request.source_artifact_sha256
        || bloom.source_artifact_sha256 != request.source_artifact_sha256
        || base.camera_object_sha256 != request.camera_object_sha256
        || bloom.camera_object_sha256 != request.camera_object_sha256
        || base.camera_identity_sha256 != request.camera_identity_sha256
        || bloom.camera_identity_sha256 != request.camera_identity_sha256
        || base.render_profile_sha256 != request.render_profile_sha256
        || bloom.render_profile_sha256 != request.render_profile_sha256
        || base.render_worker_build_cohort_sha256 != request.render_worker_build_cohort_sha256
        || bloom.render_worker_build_cohort_sha256 != request.render_worker_build_cohort_sha256
    {
        return Err(invalid("base/Bloom frame binding differs"));
    }
    Ok(())
}

#[derive(Debug, Clone)]
struct SampleContext {
    source_index: usize,
    sample_time_ticks: u64,
    projection_frame: Value,
    emitter_bindings: Value,
    particle_key_sha256: String,
    particle_frame_canonical_sha256: String,
    local_offsets: [[f32; 3]; 2],
}

#[derive(Debug, Clone)]
pub(super) struct TrailFrameContext {
    pub(super) input: FictionalEnergyVfxAnimatedSocketTrailsSequenceFrameInput,
    pub(super) projection_samples: Value,
    pub(super) trails_wire: Value,
    pub(super) trails: Vec<FictionalEnergyVfxAnimatedSocketTrail>,
    pub(super) history_samples: Vec<FictionalEnergyVfxAnimatedSocketTrailsHistorySample>,
    pub(super) worker: render_worker::RenderWorkerAnimatedSocketTrailsFrame,
    pub(super) camera: Value,
}

#[derive(Debug, Clone)]
pub(super) struct DependencyContext {
    pub(super) frames: Vec<TrailFrameContext>,
    pub(super) worker_cohort: String,
    pub(super) projection_input_sha256: String,
    pub(super) source_glb: Vec<u8>,
}

fn sample_value(sample: &SampleContext) -> Result<Value, RuntimeError> {
    Ok(json!({
        "frame_index":sample.source_index,
        "sample_time_ticks":sample.sample_time_ticks,
        "projection_frame_canonical_sha256":sample.projection_frame.get("canonical_sha256"),
        "projection_socket_transform_inventory_sha256":sample.projection_frame.get("socket_transform_inventory_sha256"),
        "projection_socket_transform_readback_sha256":sample.projection_frame.get("socket_transform_readback_sha256"),
        "emitters":sample.emitter_bindings.get("emitters")
    }))
}

fn trail_wire(sample_contexts: &[SampleContext]) -> Result<Value, RuntimeError> {
    if sample_contexts.len() < 2 || sample_contexts.len() > 9 {
        return Err(invalid("trail history window must contain 2..9 samples"));
    }
    let make_points = |trail_index: usize| -> Value {
        Value::Array(
            sample_contexts
                .iter()
                .map(|sample| {
                    json!({
                        "frame_index":sample.source_index,
                        "sample_time_ticks":sample.sample_time_ticks,
                        "source_particle_key_sha256":sample.particle_key_sha256,
                        "source_particle_id":if trail_index == 0 {10000_u64} else {20000_u64},
                        "local_offset_m":f32_value(&sample.local_offsets[trail_index])
                    })
                })
                .collect(),
        )
    };
    Ok(json!([
        {"emitter_id":"muzzle-trail","id":30000_u64,"local_points":make_points(0),"radius_px":2.0,"color_linear_rgb":[0.0,0.8,1.0],"alpha":0.85,"lifetime_ticks":120_u64},
        {"emitter_id":"energy-core-trail","id":31000_u64,"local_points":make_points(1),"radius_px":2.0,"color_linear_rgb":[1.0,0.35,0.05],"alpha":0.8,"lifetime_ticks":180_u64}
    ]))
}

fn world_trails_and_inventory(
    camera: &Value,
    sample_contexts: &[SampleContext],
    trails_wire: &Value,
) -> Result<(Vec<FictionalEnergyVfxAnimatedSocketTrail>, Value, String), RuntimeError> {
    let wire = trails_wire
        .as_array()
        .filter(|values| values.len() == 2)
        .ok_or_else(|| invalid("trail wire inventory is malformed"))?;
    let mut trails = Vec::with_capacity(2);
    let mut inventory_trails = Vec::with_capacity(2);
    for (trail_index, definition) in wire.iter().enumerate() {
        let points = definition
            .get("local_points")
            .and_then(Value::as_array)
            .ok_or_else(|| invalid("trail local points are unavailable"))?;
        let mut record_points = Vec::with_capacity(points.len());
        let mut inventory_points = Vec::with_capacity(points.len());
        for (point_index, point) in points.iter().enumerate() {
            let sample = &sample_contexts[point_index];
            let local = f32_array(point, "local_offset_m", 3, 10.0)?;
            let local = [local[0], local[1], local[2]];
            let (translation, rotation) = emitter_transform(&sample.emitter_bindings, trail_index)?;
            let world = transform_point(translation, rotation, local);
            let depth = camera_depth(camera, world)?;
            let particle_id = if trail_index == 0 { 10_000 } else { 20_000 };
            record_points.push(FictionalEnergyVfxAnimatedSocketTrailPoint {
                source_frame_index: sample.source_index as u64,
                sample_time_ticks: sample.sample_time_ticks,
                source_particle_key_sha256: sample.particle_key_sha256.clone(),
                source_particle_frame_index: sample.source_index as u64,
                source_particle_id: particle_id,
                local_offset_micrometers: [
                    (f64::from(local[0]) * 1_000_000.0).round() as i64,
                    (f64::from(local[1]) * 1_000_000.0).round() as i64,
                    (f64::from(local[2]) * 1_000_000.0).round() as i64,
                ],
                world_position_micrometers: [
                    (f64::from(world[0]) * 1_000_000.0).round() as i64,
                    (f64::from(world[1]) * 1_000_000.0).round() as i64,
                    (f64::from(world[2]) * 1_000_000.0).round() as i64,
                ],
                depth_micrometers: (f64::from(depth) * 1_000_000.0).round() as u64,
            });
            inventory_points.push(json!({
                "frame_index":sample.source_index,
                "sample_time_ticks":sample.sample_time_ticks,
                "source_particle_key_sha256":sample.particle_key_sha256,
                "source_particle_id":particle_id,
                "local_offset_m":f32_value(&local),
                "world_position_m":f32_value(&world),
                "camera_depth":depth,
                "projection_frame_canonical_sha256":sample.projection_frame.get("canonical_sha256")
            }));
        }
        let emitter_role = if trail_index == 0 {
            "muzzle-vfx"
        } else {
            "energy-core-vfx"
        };
        trails.push(FictionalEnergyVfxAnimatedSocketTrail {
            emitter_role: emitter_role.to_owned(),
            trail_id: if trail_index == 0 { 30_000 } else { 31_000 },
            points: record_points,
        });
        inventory_trails.push(json!({
            "emitter_id":definition.get("emitter_id"),
            "id":definition.get("id"),
            "radius_px":definition.get("radius_px"),
            "color_linear_rgb":definition.get("color_linear_rgb"),
            "alpha":definition.get("alpha"),
            "lifetime_ticks":definition.get("lifetime_ticks"),
            "points":inventory_points
        }));
    }
    let mut inventory = json!({
        "schema_version":TRAIL_INVENTORY_SCHEMA,
        "projection_key_sha256":"",
        "current_frame_index":sample_contexts.last().map(|sample| sample.source_index),
        "current_sample_time_ticks":sample_contexts.last().map(|sample| sample.sample_time_ticks),
        "sample_count":sample_contexts.len(),
        "seed_sha256":"",
        "trails":inventory_trails,
        "canonical_sha256":""
    });
    let mut preimage = inventory.as_object().expect("inventory is object").clone();
    preimage.remove("canonical_sha256");
    preimage.remove("seed_sha256");
    let hash = canonical_json_hash(&Value::Object(preimage));
    inventory["canonical_sha256"] = Value::String(hash.clone());
    Ok((trails, inventory, hash))
}

fn worker_seed(
    projection_key: &str,
    current_frame_index: u64,
    sample_time_ticks: u64,
    projection_input_sha256: &str,
    projection_sample_set_sha256: &str,
    emitter_binding_sha256: &str,
    inventory: &Value,
) -> Result<String, RuntimeError> {
    let local_inventory = inventory
        .get("trails")
        .cloned()
        .ok_or_else(|| invalid("trail inventory has no trails"))?;
    Ok(canonical_json_hash(&json!({
        "schema_version":"RenderWorkerAnimatedSocketTrailSeed@1",
        "projection_key_sha256":projection_key,
        "current_frame_index":current_frame_index,
        "current_sample_time_ticks":sample_time_ticks,
        "projection_input_sha256":projection_input_sha256,
        "projection_sample_set_sha256":projection_sample_set_sha256,
        "emitter_binding_sha256":emitter_binding_sha256,
        "local_trail_inventory":local_inventory
    })))
}

fn expected_inventory(
    mut inventory: Value,
    projection_key: &str,
    seed: &str,
) -> Result<(Value, String), RuntimeError> {
    inventory["projection_key_sha256"] = Value::String(projection_key.to_owned());
    inventory["seed_sha256"] = Value::String(seed.to_owned());
    let mut preimage = inventory.as_object().expect("inventory is object").clone();
    preimage.remove("canonical_sha256");
    preimage.remove("seed_sha256");
    let hash = canonical_json_hash(&Value::Object(preimage));
    inventory["canonical_sha256"] = Value::String(hash.clone());
    Ok((inventory, hash))
}

fn compare_worker_replay(
    first: &render_worker::RenderWorkerAnimatedSocketTrailsFrame,
    second: &render_worker::RenderWorkerAnimatedSocketTrailsFrame,
    request: &FictionalEnergyVfxAnimatedSocketTrailsSequencePrepareRequest,
    input: &FictionalEnergyVfxAnimatedSocketTrailsSequenceFrameInput,
    projection_input_sha256: &str,
    expected_sample_set_sha256: &str,
    expected_emitter_binding_sha256: &str,
    expected_seed_sha256: &str,
    expected_inventory: &Value,
    expected_inventory_sha256: &str,
) -> Result<(), RuntimeError> {
    if first.build_cohort_sha256.is_none()
        || first.build_cohort_sha256 != second.build_cohort_sha256
        || first.render_profile != second.render_profile
        || first.trail_count != 2
        || second.trail_count != first.trail_count
        || first.segment_count
            != 2 * (expected_inventory
                .get("sample_count")
                .and_then(Value::as_u64)
                .unwrap_or(0)
                .saturating_sub(1) as usize)
        || second.segment_count != first.segment_count
        || first.emitter_counts != [1, 1]
        || second.emitter_counts != first.emitter_counts
        || first.seed_sha256 != expected_seed_sha256
        || second.seed_sha256 != first.seed_sha256
        || first.projection_key_sha256 != request.projection_key_sha256
        || second.projection_key_sha256 != first.projection_key_sha256
        || first.current_frame_index != input.current_projection_frame_index
        || second.current_frame_index != first.current_frame_index
        || first.current_sample_time_ticks != input.sample_time_ticks
        || second.current_sample_time_ticks != first.current_sample_time_ticks
        || first.projection_input_sha256 != projection_input_sha256
        || second.projection_input_sha256 != first.projection_input_sha256
        || first.projection_sample_set_sha256 != expected_sample_set_sha256
        || second.projection_sample_set_sha256 != first.projection_sample_set_sha256
        || first.emitter_binding_sha256 != expected_emitter_binding_sha256
        || second.emitter_binding_sha256 != first.emitter_binding_sha256
        || first.trail_inventory_sha256 != expected_inventory_sha256
        || second.trail_inventory_sha256 != first.trail_inventory_sha256
        || first.trail_inventory != *expected_inventory
        || second.trail_inventory != first.trail_inventory
        || first.trail_passes.len() != 3
        || second.trail_passes.len() != first.trail_passes.len()
        || first
            .trail_passes
            .iter()
            .zip(&second.trail_passes)
            .any(|(left, right)| left.pass != right.pass || left.png != right.png)
    {
        return Err(invalid(
            "animated socket trail Worker replay is not byte exact",
        ));
    }
    Ok(())
}

pub(super) fn build_context(
    runtime: &Runtime,
    request: &FictionalEnergyVfxAnimatedSocketTrailsSequencePrepareRequest,
) -> Result<DependencyContext, RuntimeError> {
    let projection_result =
        runtime.game_weapon_animated_glb_socket_transform_projection_get(&json!({
            "schema_version":"GameWeaponAnimatedGlbSocketTransformProjectionGetRequest@1",
            "projection_key_sha256":request.projection_key_sha256,
            "project_id":request.project_id,
            "candidate_id":request.candidate_id
        }))?;
    let projection = projection_result
        .get("projection")
        .cloned()
        .ok_or_else(|| invalid("projection payload is unavailable"))?;
    if projection_result
        .get("projection_object_sha256")
        .and_then(Value::as_str)
        != Some(request.projection_object_sha256.as_str())
    {
        return Err(invalid("projection object binding differs"));
    }
    let projection_input_sha256 = validate_projection_parent(request, &projection)?;
    let vfx = runtime.fictional_energy_vfx_get(&json!({
        "schema_version":"FictionalEnergyVfxGetRequest@1",
        "project_id":request.project_id,
        "delivery_manifest_object_sha256":request.delivery_manifest_object_sha256
    }))?;
    let vfx_link = vfx
        .get("link")
        .ok_or_else(|| invalid("VFX profile link is unavailable"))?;
    if vfx_link.get("project_id").and_then(Value::as_str) != Some(request.project_id.as_str())
        || vfx_link
            .get("delivery_manifest_object_sha256")
            .and_then(Value::as_str)
            != Some(request.delivery_manifest_object_sha256.as_str())
        || vfx_link
            .get("vfx_profile_object_sha256")
            .and_then(Value::as_str)
            != Some(request.vfx_profile_object_sha256.as_str())
    {
        return Err(invalid("VFX profile link binding differs"));
    }
    let profile = vfx
        .get("vfx_profile")
        .ok_or_else(|| invalid("VFX profile is unavailable"))?;
    if profile.get("canonical_sha256").and_then(Value::as_str)
        != Some(request.vfx_profile_canonical_sha256.as_str())
    {
        return Err(invalid("VFX profile binding differs"));
    }
    let particle_value =
        runtime.fictional_energy_vfx_animated_socket_particles_sequence_get(&json!({
            "schema_version":"FictionalEnergyVfxAnimatedSocketParticlesSequenceGetRequest@1",
            "sequence_key_sha256":request.frames[0].particle_sequence_key_sha256,
            "project_id":request.project_id,
            "candidate_id":request.candidate_id
        }))?;
    let particles: FictionalEnergyVfxAnimatedSocketParticlesSequence = serde_json::from_value(
        particle_value
            .get("sequence")
            .cloned()
            .ok_or_else(|| invalid("particle sequence is unavailable"))?,
    )
    .map_err(|error| invalid(format!("particle sequence is malformed: {error}")))?;
    if particles.project_id != request.project_id
        || particles.candidate_id != request.candidate_id
        || particles.sequence_key_sha256 != request.frames[0].particle_sequence_key_sha256
        || particles.delivery_manifest_object_sha256 != request.delivery_manifest_object_sha256
        || particles.source_artifact_sha256 != request.source_artifact_sha256
        || particles.projection_key_sha256 != request.projection_key_sha256
        || particles.animated_socket_materialization_key_sha256
            != request.animated_socket_materialization_key_sha256
        || particles.animated_artifact_sha256 != request.animated_artifact_sha256
        || particles.frames.len() < request.sample_count as usize + 1
    {
        return Err(invalid("particle sequence parent binding differs"));
    }
    let source_glb = runtime.cas_read_bounded(&request.source_artifact_sha256, MAX_GLB_BYTES)?;
    let mut frames = Vec::with_capacity(MAX_FRAMES);
    let mut worker_cohort = None::<String>;
    let mut camera_for_all = None::<Value>;
    for (ordinal, input) in request.frames.iter().enumerate() {
        let current_index = ordinal + 1;
        let previous_index = ordinal;
        let current_projection = projection_frame(&projection, current_index)?.clone();
        let previous_projection = projection_frame(&projection, previous_index)?.clone();
        if current_projection
            .get("frame_index")
            .and_then(Value::as_u64)
            != Some(current_index as u64)
            || previous_projection
                .get("frame_index")
                .and_then(Value::as_u64)
                != Some(previous_index as u64)
            || current_projection
                .get("sample_time_ticks")
                .and_then(Value::as_u64)
                != Some(input.sample_time_ticks)
            || previous_projection
                .get("sample_time_ticks")
                .and_then(Value::as_u64)
                >= Some(input.sample_time_ticks)
            || current_projection
                .get("canonical_sha256")
                .and_then(Value::as_str)
                != Some(input.current_projection_frame_canonical_sha256.as_str())
            || current_projection
                .get("socket_transform_inventory_sha256")
                .and_then(Value::as_str)
                != Some(
                    input
                        .current_projection_socket_transform_inventory_sha256
                        .as_str(),
                )
            || current_projection
                .get("socket_transform_readback_sha256")
                .and_then(Value::as_str)
                != Some(
                    input
                        .current_projection_socket_transform_readback_sha256
                        .as_str(),
                )
            || previous_projection
                .get("canonical_sha256")
                .and_then(Value::as_str)
                != Some(input.previous_projection_frame_canonical_sha256.as_str())
            || previous_projection
                .get("socket_transform_inventory_sha256")
                .and_then(Value::as_str)
                != Some(
                    input
                        .previous_projection_socket_transform_inventory_sha256
                        .as_str(),
                )
            || previous_projection
                .get("socket_transform_readback_sha256")
                .and_then(Value::as_str)
                != Some(
                    input
                        .previous_projection_socket_transform_readback_sha256
                        .as_str(),
                )
        {
            return Err(invalid("current/previous projection frame binding differs"));
        }
        let current_particle = particle_frame(&particles, current_index)?;
        let previous_particle = particle_frame(&particles, previous_index)?;
        if current_particle.frame_index != current_index as u64
            || previous_particle.frame_index != previous_index as u64
            || current_particle.sample_time_ticks != input.sample_time_ticks
            || current_particle.particle_key_sha256 != input.current_particle_key_sha256
            || current_particle.canonical_sha256 != input.current_particle_frame_canonical_sha256
            || previous_particle.canonical_sha256
                != input.previous_particle_sequence_frame_canonical_sha256
        {
            return Err(invalid("current/previous particle frame binding differs"));
        }
        let base_value = runtime.fictional_energy_vfx_rendered_frame_get(&json!({
            "schema_version":"FictionalEnergyVfxRenderedFrameGetRequest@1",
            "project_id":request.project_id,
            "frame_key_sha256":input.base_frame_key_sha256
        }))?;
        let base: FictionalEnergyVfxFrameLinkRecord = serde_json::from_value(
            base_value
                .get("link")
                .cloned()
                .ok_or_else(|| invalid("base frame link unavailable"))?,
        )
        .map_err(|error| invalid(format!("base frame link malformed: {error}")))?;
        let bloom_value = runtime.fictional_energy_vfx_hdr_bloom_get(&json!({
            "schema_version":"FictionalEnergyVfxHdrBloomFrameGetRequest@1",
            "project_id":request.project_id,
            "bloom_key_sha256":input.bloom_key_sha256
        }))?;
        let bloom: FictionalEnergyVfxBloomFrameLinkRecord = serde_json::from_value(
            bloom_value
                .get("link")
                .cloned()
                .ok_or_else(|| invalid("Bloom frame link unavailable"))?,
        )
        .map_err(|error| invalid(format!("Bloom frame link malformed: {error}")))?;
        validate_base_bloom(request, input, &base, &bloom)?;
        let camera = if let Some(camera) = &camera_for_all {
            camera.clone()
        } else {
            let camera = camera_for_base(runtime, request, &base)?;
            camera_for_all = Some(camera.clone());
            camera
        };
        let first_source = current_index.saturating_sub(8);
        let mut samples = Vec::with_capacity(current_index - first_source + 1);
        for source_index in first_source..=current_index {
            let projection_source = projection_frame(&projection, source_index)?.clone();
            let particle_source = particle_frame(&particles, source_index)?;
            if particle_source.sample_time_ticks
                != projection_source
                    .get("sample_time_ticks")
                    .and_then(Value::as_u64)
                    .ok_or_else(|| invalid("projection sample tick unavailable"))?
            {
                return Err(invalid("projection/particle history tick differs"));
            }
            let emitter = emitter_bindings(&projection_source)?;
            let local_offsets = [
                particle_local_offset(runtime, particle_source, 10_000)?,
                particle_local_offset(runtime, particle_source, 20_000)?,
            ];
            samples.push(SampleContext {
                source_index,
                sample_time_ticks: projection_source
                    .get("sample_time_ticks")
                    .and_then(Value::as_u64)
                    .ok_or_else(|| invalid("projection sample tick unavailable"))?,
                projection_frame: projection_source,
                emitter_bindings: emitter,
                particle_key_sha256: particle_source.particle_key_sha256.clone(),
                particle_frame_canonical_sha256: particle_source.canonical_sha256.clone(),
                local_offsets,
            });
        }
        let sample_values = samples
            .iter()
            .map(sample_value)
            .collect::<Result<Vec<_>, _>>()?;
        let projection_samples = Value::Array(sample_values);
        let emitter_value = json!({
            "schema_version":TRAIL_EMITTER_SCHEMA,
            "projection_key_sha256":request.projection_key_sha256,
            "samples":samples.iter().map(|sample| json!({
                "frame_index":sample.source_index,
                "sample_time_ticks":sample.sample_time_ticks,
                "emitters":sample.emitter_bindings.get("emitters")
            })).collect::<Vec<_>>()
        });
        let emitter_binding_sha256 = canonical_json_hash(&emitter_value);
        let sample_set_value = json!({
            "schema_version":TRAIL_SAMPLE_SET_SCHEMA,
            "projection_key_sha256":request.projection_key_sha256,
            "current_frame_index":current_index,
            "current_sample_time_ticks":input.sample_time_ticks,
            "samples":samples.iter().map(|sample| json!({
                "frame_index":sample.source_index,
                "sample_time_ticks":sample.sample_time_ticks,
                "projection_frame_canonical_sha256":sample.projection_frame.get("canonical_sha256"),
                "projection_socket_transform_inventory_sha256":sample.projection_frame.get("socket_transform_inventory_sha256"),
                "projection_socket_transform_readback_sha256":sample.projection_frame.get("socket_transform_readback_sha256"),
                "emitter_binding":sample.emitter_bindings.get("emitters")
            })).collect::<Vec<_>>()
        });
        let expected_sample_set_sha256 = canonical_json_hash(&sample_set_value);
        let trails_wire = trail_wire(&samples)?;
        let (_trail_models, inventory_without_seed, _inventory_hash) =
            world_trails_and_inventory(&camera, &samples, &trails_wire)?;
        let (mut inventory_seedless, _) =
            expected_inventory(inventory_without_seed, &request.projection_key_sha256, "")?;
        inventory_seedless["projection_key_sha256"] =
            Value::String(request.projection_key_sha256.clone());
        let seed_sha256 = worker_seed(
            &request.projection_key_sha256,
            current_index as u64,
            input.sample_time_ticks,
            &projection_input_sha256,
            &expected_sample_set_sha256,
            &emitter_binding_sha256,
            &inventory_seedless,
        )?;
        let (expected_inventory, expected_inventory_sha256) = expected_inventory(
            inventory_seedless,
            &request.projection_key_sha256,
            &seed_sha256,
        )?;
        let trail_models = world_trails_and_inventory(&camera, &samples, &trails_wire)?.0;
        let first = render_worker::render_typed_animated_socket_trails_with_worker_identity(
            &source_glb,
            &camera,
            &request.projection_key_sha256,
            &projection_input_sha256,
            current_index as u64,
            input.sample_time_ticks,
            &projection_samples,
            &trails_wire,
            &seed_sha256,
        )
        .map_err(|error| invalid(format!("animated trail Worker render failed: {error}")))?;
        let second = render_worker::render_typed_animated_socket_trails_with_worker_identity(
            &source_glb,
            &camera,
            &request.projection_key_sha256,
            &projection_input_sha256,
            current_index as u64,
            input.sample_time_ticks,
            &projection_samples,
            &trails_wire,
            &seed_sha256,
        )
        .map_err(|error| invalid(format!("animated trail Worker replay failed: {error}")))?;
        compare_worker_replay(
            &first,
            &second,
            request,
            input,
            &projection_input_sha256,
            &expected_sample_set_sha256,
            &emitter_binding_sha256,
            &seed_sha256,
            &expected_inventory,
            &expected_inventory_sha256,
        )?;
        let cohort = first
            .build_cohort_sha256
            .clone()
            .ok_or_else(|| invalid("Worker cohort unavailable"))?;
        if cohort != request.render_worker_build_cohort_sha256
            || first
                .render_profile
                .get("canonical_sha256")
                .and_then(Value::as_str)
                != Some(request.render_profile_sha256.as_str())
        {
            return Err(invalid("Worker cohort/profile differs"));
        }
        if worker_cohort
            .as_deref()
            .is_some_and(|value| value != cohort.as_str())
        {
            return Err(invalid("Worker cohort changes across frames"));
        }
        worker_cohort = Some(cohort);
        let history_samples = samples[..samples.len() - 1]
            .iter()
            .enumerate()
            .map(
                |(history_ordinal, sample)| FictionalEnergyVfxAnimatedSocketTrailsHistorySample {
                    history_ordinal: history_ordinal as u64,
                    projection_key_sha256: request.projection_key_sha256.clone(),
                    projection_frame_index: sample.source_index as u64,
                    projection_frame_canonical_sha256: sample
                        .projection_frame
                        .get("canonical_sha256")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_owned(),
                    projection_socket_transform_inventory_sha256: sample
                        .projection_frame
                        .get("socket_transform_inventory_sha256")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_owned(),
                    projection_socket_transform_readback_sha256: sample
                        .projection_frame
                        .get("socket_transform_readback_sha256")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_owned(),
                    particle_sequence_key_sha256: input.particle_sequence_key_sha256.clone(),
                    particle_frame_index: sample.source_index as u64,
                    particle_key_sha256: sample.particle_key_sha256.clone(),
                    particle_frame_canonical_sha256: sample.particle_frame_canonical_sha256.clone(),
                    sample_time_ticks: sample.sample_time_ticks,
                },
            )
            .collect::<Vec<_>>();
        frames.push(TrailFrameContext {
            input: input.clone(),
            projection_samples,
            trails_wire,
            trails: trail_models,
            history_samples,
            worker: first,
            camera,
        });
    }
    Ok(DependencyContext {
        frames,
        worker_cohort: worker_cohort.ok_or_else(|| invalid("Worker cohort missing"))?,
        projection_input_sha256,
        source_glb,
    })
}

pub(super) fn pass_metadata(hash: &str, size_bytes: u64, pass: &str) -> Value {
    json!({
        "pass":pass,
        "sha256":hash,
        "mime":"image/png",
        "size_bytes":size_bytes,
        "width":512,
        "height":512,
        "channels":"rgba8",
        "color_space":"data"
    })
}

pub(super) fn frame_without_receipt(
    frame: &FictionalEnergyVfxAnimatedSocketTrailsSequenceFrame,
) -> Value {
    let mut value = serde_json::to_value(frame).expect("frame serialization is infallible");
    if let Some(object) = value.as_object_mut() {
        object.insert(
            "receipt_object_sha256".to_owned(),
            Value::String(String::new()),
        );
        object.insert("canonical_sha256".to_owned(), Value::String(String::new()));
    }
    let canonical = canonical_json_hash(&value);
    value["canonical_sha256"] = Value::String(canonical);
    value
}

fn make_frame_record(
    request: &FictionalEnergyVfxAnimatedSocketTrailsSequencePrepareRequest,
    context: &TrailFrameContext,
    pass_hashes: [&str; 3],
    render_set_hash: &str,
    receipt_hash: &str,
    created_at: &str,
) -> FictionalEnergyVfxAnimatedSocketTrailsSequenceFrame {
    let input_sha256 = canonical_json_hash(
        &serde_json::to_value(&context.input).expect("frame input serialization is infallible"),
    );
    let trail_seed_sha256 = context.worker.seed_sha256.clone();
    let trail_key_sha256 = canonical_json_hash(&json!({
        "schema_version":"FictionalEnergyVfxAnimatedSocketTrailKey@1",
        "sequence_key_sha256":request.sequence_key_sha256,
        "frame_index":context.input.frame_index,
        "input_sha256":input_sha256,
        "projection_sample_set_sha256":context.worker.projection_sample_set_sha256,
        "emitter_binding_sha256":context.worker.emitter_binding_sha256,
        "trail_seed_sha256":trail_seed_sha256,
        "trail_inventory_sha256":context.worker.trail_inventory_sha256,
        "trail_passes":pass_hashes
    }));
    let trail_id_encoding_sha256 = canonical_json_hash(&json!({
        "schema_version":"FictionalEnergyVfxAnimatedSocketTrailIdEncoding@1",
        "muzzle-vfx":30000_u64,
        "energy-core-vfx":31000_u64,
        "source_particle_ids":{"muzzle-vfx":10000_u64,"energy-core-vfx":20000_u64}
    }));
    let mut frame = FictionalEnergyVfxAnimatedSocketTrailsSequenceFrame {
        schema_version: FRAME_SCHEMA.to_owned(),
        frame_index: context.input.frame_index,
        sample_time_ticks: context.input.sample_time_ticks,
        history_origin: HISTORY_PREROLL_POLICY.to_owned(),
        current_projection_frame_index: context.input.current_projection_frame_index,
        current_particle_frame_index: context.input.current_particle_frame_index,
        current_particle_key_sha256: context.input.current_particle_key_sha256.clone(),
        current_particle_frame_canonical_sha256: context
            .input
            .current_particle_frame_canonical_sha256
            .clone(),
        current_projection_frame_canonical_sha256: context
            .input
            .current_projection_frame_canonical_sha256
            .clone(),
        current_projection_socket_transform_inventory_sha256: context
            .input
            .current_projection_socket_transform_inventory_sha256
            .clone(),
        current_projection_socket_transform_readback_sha256: context
            .input
            .current_projection_socket_transform_readback_sha256
            .clone(),
        previous_projection_frame_index: context.input.previous_projection_frame_index,
        previous_particle_frame_index: context.input.previous_particle_frame_index,
        previous_particle_sequence_frame_canonical_sha256: context
            .input
            .previous_particle_sequence_frame_canonical_sha256
            .clone(),
        previous_projection_frame_canonical_sha256: context
            .input
            .previous_projection_frame_canonical_sha256
            .clone(),
        previous_projection_socket_transform_inventory_sha256: context
            .input
            .previous_projection_socket_transform_inventory_sha256
            .clone(),
        previous_projection_socket_transform_readback_sha256: context
            .input
            .previous_projection_socket_transform_readback_sha256
            .clone(),
        projection_sample_set_sha256: context.worker.projection_sample_set_sha256.clone(),
        particle_sequence_key_sha256: context.input.particle_sequence_key_sha256.clone(),
        base_frame_key_sha256: context.input.base_frame_key_sha256.clone(),
        bloom_key_sha256: context.input.bloom_key_sha256.clone(),
        camera_object_sha256: request.camera_object_sha256.clone(),
        camera_identity_sha256: request.camera_identity_sha256.clone(),
        render_profile_sha256: request.render_profile_sha256.clone(),
        render_worker_build_cohort_sha256: request.render_worker_build_cohort_sha256.clone(),
        history_samples: context.history_samples.clone(),
        trail_count: 2,
        trail_emitter_roles: vec!["muzzle-vfx".to_owned(), "energy-core-vfx".to_owned()],
        trails: context.trails.clone(),
        trail_key_sha256,
        trail_seed_sha256,
        trail_inventory_sha256: context.worker.trail_inventory_sha256.clone(),
        trail_id_encoding_sha256,
        emitter_binding_sha256: context.worker.emitter_binding_sha256.clone(),
        trail_color_object_sha256: pass_hashes[0].to_owned(),
        trail_id_object_sha256: pass_hashes[1].to_owned(),
        trail_depth_object_sha256: pass_hashes[2].to_owned(),
        render_set_object_sha256: render_set_hash.to_owned(),
        receipt_object_sha256: receipt_hash.to_owned(),
        canonical_sha256: String::new(),
        created_at: created_at.to_owned(),
    };
    frame.canonical_sha256 = canonical_json_hash(
        &serde_json::to_value(&frame).expect("frame serialization is infallible"),
    );
    frame
}

fn build_sequence(
    request: &FictionalEnergyVfxAnimatedSocketTrailsSequencePrepareRequest,
    context: &DependencyContext,
    frames: Vec<FictionalEnergyVfxAnimatedSocketTrailsSequenceFrame>,
) -> FictionalEnergyVfxAnimatedSocketTrailsSequence {
    let mut sequence = FictionalEnergyVfxAnimatedSocketTrailsSequence {
        schema_version: SEQUENCE_SCHEMA.to_owned(),
        sequence_key_sha256: request.sequence_key_sha256.clone(),
        project_id: request.project_id.clone(),
        candidate_id: request.candidate_id.clone(),
        candidate_state_sha256: request.candidate_state_sha256.clone(),
        delivery_manifest_object_sha256: request.delivery_manifest_object_sha256.clone(),
        source_artifact_sha256: request.source_artifact_sha256.clone(),
        projection_key_sha256: request.projection_key_sha256.clone(),
        projection_object_sha256: request.projection_object_sha256.clone(),
        projection_canonical_sha256: request.projection_canonical_sha256.clone(),
        animated_socket_materialization_key_sha256: request
            .animated_socket_materialization_key_sha256
            .clone(),
        animated_artifact_sha256: request.animated_artifact_sha256.clone(),
        animated_socket_anchor_set_object_sha256: request
            .animated_socket_anchor_set_object_sha256
            .clone(),
        animated_socket_anchor_set_canonical_sha256: request
            .animated_socket_anchor_set_canonical_sha256
            .clone(),
        animation_clip_id: request.animation_clip_id.clone(),
        animation_clip_object_sha256: request.animation_clip_object_sha256.clone(),
        animation_clip_canonical_sha256: request.animation_clip_canonical_sha256.clone(),
        animation_receipt_object_sha256: request.animation_receipt_object_sha256.clone(),
        animation_receipt_canonical_sha256: request.animation_receipt_canonical_sha256.clone(),
        vfx_profile_object_sha256: request.vfx_profile_object_sha256.clone(),
        vfx_profile_canonical_sha256: request.vfx_profile_canonical_sha256.clone(),
        socket_node_id_encoding_sha256: request.socket_node_id_encoding_sha256.clone(),
        socket_roles_sha256: request.socket_roles_sha256.clone(),
        camera_object_sha256: request.camera_object_sha256.clone(),
        camera_identity_sha256: request.camera_identity_sha256.clone(),
        render_profile_sha256: request.render_profile_sha256.clone(),
        render_worker_build_cohort_sha256: context.worker_cohort.clone(),
        sample_schedule_sha256: request.sample_schedule_sha256.clone(),
        sample_count: request.sample_count,
        sample_time_ticks: request.sample_time_ticks.clone(),
        frame_scope: FRAME_SCOPE.to_owned(),
        trails_sequence_policy: TRAILS_POLICY.to_owned(),
        history_policy: HISTORY_POLICY.to_owned(),
        history_pre_roll_policy: HISTORY_PREROLL_POLICY.to_owned(),
        trail_count: 2,
        trail_emitter_roles: vec!["muzzle-vfx".to_owned(), "energy-core-vfx".to_owned()],
        frames,
        sequence_status: STATUS.to_owned(),
        quality_status: "structural_only".to_owned(),
        visual_quality_status: "NOT_PROVEN".to_owned(),
        commercial_fps_quality_status: "NOT_PROVEN".to_owned(),
        human_review_status: "NOT_RUN".to_owned(),
        commercial_engine_status: "NOT_RUN".to_owned(),
        runtime_write_performed: true,
        restart_hash_verified: true,
        candidate_confirmed: false,
        version_created: false,
        export_performed: false,
        actual_engine_roundtrip: false,
        production_stage_advanced: false,
        input_sha256: request.input_sha256.clone(),
        canonical_sha256: String::new(),
        created_at: now_string(),
    };
    sequence.canonical_sha256 = canonical_json_hash(
        &serde_json::to_value(&sequence).expect("sequence serialization is infallible"),
    );
    sequence
}

fn frame_input_matches(
    input: &FictionalEnergyVfxAnimatedSocketTrailsSequenceFrameInput,
    frame: &FictionalEnergyVfxAnimatedSocketTrailsSequenceFrame,
) -> bool {
    input.frame_index == frame.frame_index
        && input.sample_time_ticks == frame.sample_time_ticks
        && input.history_origin == frame.history_origin
        && input.current_projection_frame_index == frame.current_projection_frame_index
        && input.current_particle_frame_index == frame.current_particle_frame_index
        && input.current_particle_key_sha256 == frame.current_particle_key_sha256
        && input.current_particle_frame_canonical_sha256
            == frame.current_particle_frame_canonical_sha256
        && input.current_projection_frame_canonical_sha256
            == frame.current_projection_frame_canonical_sha256
        && input.current_projection_socket_transform_inventory_sha256
            == frame.current_projection_socket_transform_inventory_sha256
        && input.current_projection_socket_transform_readback_sha256
            == frame.current_projection_socket_transform_readback_sha256
        && input.previous_projection_frame_index == frame.previous_projection_frame_index
        && input.previous_particle_frame_index == frame.previous_particle_frame_index
        && input.previous_particle_sequence_frame_canonical_sha256
            == frame.previous_particle_sequence_frame_canonical_sha256
        && input.previous_projection_frame_canonical_sha256
            == frame.previous_projection_frame_canonical_sha256
        && input.previous_projection_socket_transform_inventory_sha256
            == frame.previous_projection_socket_transform_inventory_sha256
        && input.previous_projection_socket_transform_readback_sha256
            == frame.previous_projection_socket_transform_readback_sha256
        && input.particle_sequence_key_sha256 == frame.particle_sequence_key_sha256
        && input.base_frame_key_sha256 == frame.base_frame_key_sha256
        && input.bloom_key_sha256 == frame.bloom_key_sha256
        && input.camera_object_sha256 == frame.camera_object_sha256
        && input.camera_identity_sha256 == frame.camera_identity_sha256
        && input.render_profile_sha256 == frame.render_profile_sha256
        && input.render_worker_build_cohort_sha256 == frame.render_worker_build_cohort_sha256
}

fn request_matches_sequence(
    request: &FictionalEnergyVfxAnimatedSocketTrailsSequencePrepareRequest,
    sequence: &FictionalEnergyVfxAnimatedSocketTrailsSequence,
) -> bool {
    request.sequence_key_sha256 == sequence.sequence_key_sha256
        && request.project_id == sequence.project_id
        && request.candidate_id == sequence.candidate_id
        && request.candidate_state_sha256 == sequence.candidate_state_sha256
        && request.delivery_manifest_object_sha256 == sequence.delivery_manifest_object_sha256
        && request.source_artifact_sha256 == sequence.source_artifact_sha256
        && request.projection_key_sha256 == sequence.projection_key_sha256
        && request.projection_object_sha256 == sequence.projection_object_sha256
        && request.projection_canonical_sha256 == sequence.projection_canonical_sha256
        && request.animated_socket_materialization_key_sha256
            == sequence.animated_socket_materialization_key_sha256
        && request.animated_artifact_sha256 == sequence.animated_artifact_sha256
        && request.animated_socket_anchor_set_object_sha256
            == sequence.animated_socket_anchor_set_object_sha256
        && request.animated_socket_anchor_set_canonical_sha256
            == sequence.animated_socket_anchor_set_canonical_sha256
        && request.animation_clip_id == sequence.animation_clip_id
        && request.animation_clip_object_sha256 == sequence.animation_clip_object_sha256
        && request.animation_clip_canonical_sha256 == sequence.animation_clip_canonical_sha256
        && request.animation_receipt_object_sha256 == sequence.animation_receipt_object_sha256
        && request.animation_receipt_canonical_sha256 == sequence.animation_receipt_canonical_sha256
        && request.vfx_profile_object_sha256 == sequence.vfx_profile_object_sha256
        && request.vfx_profile_canonical_sha256 == sequence.vfx_profile_canonical_sha256
        && request.socket_node_id_encoding_sha256 == sequence.socket_node_id_encoding_sha256
        && request.socket_roles_sha256 == sequence.socket_roles_sha256
        && request.camera_object_sha256 == sequence.camera_object_sha256
        && request.camera_identity_sha256 == sequence.camera_identity_sha256
        && request.render_profile_sha256 == sequence.render_profile_sha256
        && request.render_worker_build_cohort_sha256 == sequence.render_worker_build_cohort_sha256
        && request.sample_schedule_sha256 == sequence.sample_schedule_sha256
        && request.sample_count == sequence.sample_count
        && request.sample_time_ticks == sequence.sample_time_ticks
        && request.frame_scope == sequence.frame_scope
        && request.trails_sequence_policy == sequence.trails_sequence_policy
        && request.history_policy == sequence.history_policy
        && request.history_pre_roll_policy == sequence.history_pre_roll_policy
        && request.trail_count == sequence.trail_count
        && request.trail_emitter_roles == sequence.trail_emitter_roles
        && request.input_sha256 == sequence.input_sha256
        && request.frames.len() == sequence.frames.len()
        && request
            .frames
            .iter()
            .zip(&sequence.frames)
            .all(|(input, frame)| frame_input_matches(input, frame))
}

fn result_value(
    sequence: &FictionalEnergyVfxAnimatedSocketTrailsSequence,
    replayed: bool,
    schema: &str,
    runtime_write: bool,
) -> Result<Value, RuntimeError> {
    Ok(json!({
        "schema_version":schema,
        "sequence_key_sha256":sequence.sequence_key_sha256,
        "sequence":sequence,
        "replayed":replayed,
        "restart_hash_verified":true,
        "runtime_write":runtime_write,
        "quality_status":"structural_only",
        "visual_quality_status":"NOT_PROVEN",
        "commercial_fps_quality_status":"NOT_PROVEN",
        "human_review_status":"NOT_RUN",
        "commercial_engine_status":"NOT_RUN",
        "actual_engine_roundtrip":false,
        "production_stage_advanced":false,
        "candidate_confirmed":false,
        "version_created":false,
        "export_performed":false
    }))
}

pub(super) fn prepare(runtime: &Runtime, value: &Value) -> Result<Value, RuntimeError> {
    let request = parse_prepare(value)?;
    let context = build_context(runtime, &request)?;
    if let Some(existing) = runtime
        .store
        .get_fictional_energy_vfx_animated_socket_trails_sequence(&request.sequence_key_sha256)?
    {
        if !request_matches_sequence(&request, &existing) {
            return Err(invalid("existing animated trail sequence binding differs"));
        }
        return result_value(&existing, true, PREPARE_RESULT_SCHEMA, true);
    }

    let reservation = runtime.store.begin_cas_reservation();
    let mut reserved_objects: Vec<CasObject> = Vec::new();
    let operation = (|| -> Result<Value, RuntimeError> {
        let mut frame_records = Vec::with_capacity(context.frames.len());
        for frame in &context.frames {
            let created_at = now_string();
            let mut pass_hashes = [String::new(), String::new(), String::new()];
            for (index, pass) in frame.worker.trail_passes.iter().enumerate() {
                let kind = match pass.pass.as_str() {
                    "trail-color" => COLOR_KIND,
                    "trail-id" => ID_KIND,
                    "trail-depth" => DEPTH_KIND,
                    _ => return Err(invalid("Worker trail pass inventory differs")),
                };
                let object = runtime.store.put_object_reserved(
                    &reservation,
                    &pass.png,
                    None,
                    "image/png",
                    kind,
                    &created_at,
                )?;
                pass_hashes[index] = object.record.sha256.clone();
                reserved_objects.push(object);
            }
            let temporary = make_frame_record(
                &request,
                frame,
                [&pass_hashes[0], &pass_hashes[1], &pass_hashes[2]],
                "",
                "",
                &created_at,
            );
            let render_set_value = canonical_object(json!({
                "schema_version":RENDER_SET_SCHEMA,
                "sequence_key_sha256":request.sequence_key_sha256,
                "frame_index":frame.input.frame_index,
                "sample_time_ticks":frame.input.sample_time_ticks,
                "history_origin":HISTORY_PREROLL_POLICY,
                "projection_key_sha256":request.projection_key_sha256,
                "projection_samples":frame.projection_samples,
                "trail_key_sha256":temporary.trail_key_sha256,
                "trail_seed_sha256":temporary.trail_seed_sha256,
                "trail_inventory_sha256":temporary.trail_inventory_sha256,
                "trail_inventory":frame.worker.trail_inventory,
                "trail_id_encoding_sha256":temporary.trail_id_encoding_sha256,
                "emitter_binding_sha256":temporary.emitter_binding_sha256,
                "trails":frame.trails,
                "base_frame_key_sha256":frame.input.base_frame_key_sha256,
                "bloom_key_sha256":frame.input.bloom_key_sha256,
                "camera_object_sha256":request.camera_object_sha256,
                "camera_identity_sha256":request.camera_identity_sha256,
                "render_profile_sha256":request.render_profile_sha256,
                "render_worker_build_cohort_sha256":request.render_worker_build_cohort_sha256,
                "passes":["trail-color","trail-id","trail-depth"],
                "pass_artifacts":[
                    pass_metadata(&pass_hashes[0], frame.worker.trail_passes[0].png.len() as u64, "trail-color"),
                    pass_metadata(&pass_hashes[1], frame.worker.trail_passes[1].png.len() as u64, "trail-id"),
                    pass_metadata(&pass_hashes[2], frame.worker.trail_passes[2].png.len() as u64, "trail-depth")
                ],
                "canonical_sha256":""
            }))?;
            let render_set_object = runtime.store.put_object_reserved(
                &reservation,
                &render_set_value.1,
                None,
                "application/json",
                RENDER_SET_KIND,
                &created_at,
            )?;
            let render_set_hash = render_set_object.record.sha256.clone();
            reserved_objects.push(render_set_object);
            let frame_without_receipt_value = make_frame_record(
                &request,
                frame,
                [&pass_hashes[0], &pass_hashes[1], &pass_hashes[2]],
                &render_set_hash,
                "",
                &created_at,
            );
            let receipt_value = canonical_object(json!({
                "schema_version":FRAME_RECEIPT_SCHEMA,
                "sequence_key_sha256":request.sequence_key_sha256,
                "frame":frame_without_receipt_value,
                "history_samples":frame.history_samples,
                "projection_samples":frame.projection_samples,
                "trails":frame.trails,
                "trail_inventory":frame.worker.trail_inventory,
                "trail_inventory_sha256":frame.worker.trail_inventory_sha256,
                "emitter_binding_sha256":frame.worker.emitter_binding_sha256,
                "base_frame_key_sha256":frame.input.base_frame_key_sha256,
                "bloom_key_sha256":frame.input.bloom_key_sha256,
                "camera_object_sha256":request.camera_object_sha256,
                "camera_identity_sha256":request.camera_identity_sha256,
                "render_profile_sha256":request.render_profile_sha256,
                "render_worker_build_cohort_sha256":request.render_worker_build_cohort_sha256,
                "worker_replay_byte_exact":true,
                "runtime_write_performed":true,
                "candidate_confirmed":false,
                "actual_engine_roundtrip":false,
                "quality_status":"structural_only",
                "visual_quality_status":"NOT_PROVEN",
                "canonical_sha256":""
            }))?;
            let receipt_object = runtime.store.put_object_reserved(
                &reservation,
                &receipt_value.1,
                None,
                "application/json",
                FRAME_RECEIPT_KIND,
                &created_at,
            )?;
            let receipt_hash = receipt_object.record.sha256.clone();
            reserved_objects.push(receipt_object);
            frame_records.push(make_frame_record(
                &request,
                frame,
                [&pass_hashes[0], &pass_hashes[1], &pass_hashes[2]],
                &render_set_hash,
                &receipt_hash,
                &created_at,
            ));
        }
        let sequence = build_sequence(&request, &context, frame_records);
        let stored = runtime
            .store
            .record_fictional_energy_vfx_animated_socket_trails_sequence(&sequence)?;
        for object in &reserved_objects {
            runtime
                .store
                .release_cas_reservation_object(&reservation, object, false)?;
        }
        result_value(&stored, false, PREPARE_RESULT_SCHEMA, true)
    })();
    if let Err(error) = operation {
        let mut rollback_errors = Vec::new();
        for object in reserved_objects.iter().rev() {
            if let Err(rollback_error) = runtime.store.release_cas_reservation_object(
                &reservation,
                object,
                object.created_new,
            ) {
                rollback_errors.push(rollback_error.to_string());
            }
        }
        if !rollback_errors.is_empty() {
            return Err(invalid(format!(
                "{error}; CAS reservation rollback failed: {}",
                rollback_errors.join(" | ")
            )));
        }
        return Err(error);
    }
    operation
}

pub(super) fn replay_request(
    stored: &FictionalEnergyVfxAnimatedSocketTrailsSequence,
) -> FictionalEnergyVfxAnimatedSocketTrailsSequencePrepareRequest {
    let frames = stored
        .frames
        .iter()
        .map(
            |frame| FictionalEnergyVfxAnimatedSocketTrailsSequenceFrameInput {
                frame_index: frame.frame_index,
                sample_time_ticks: frame.sample_time_ticks,
                history_origin: frame.history_origin.clone(),
                current_projection_frame_index: frame.current_projection_frame_index,
                current_particle_frame_index: frame.current_particle_frame_index,
                current_particle_key_sha256: frame.current_particle_key_sha256.clone(),
                current_particle_frame_canonical_sha256: frame
                    .current_particle_frame_canonical_sha256
                    .clone(),
                current_projection_frame_canonical_sha256: frame
                    .current_projection_frame_canonical_sha256
                    .clone(),
                current_projection_socket_transform_inventory_sha256: frame
                    .current_projection_socket_transform_inventory_sha256
                    .clone(),
                current_projection_socket_transform_readback_sha256: frame
                    .current_projection_socket_transform_readback_sha256
                    .clone(),
                previous_projection_frame_index: frame.previous_projection_frame_index,
                previous_particle_frame_index: frame.previous_particle_frame_index,
                previous_particle_sequence_frame_canonical_sha256: frame
                    .previous_particle_sequence_frame_canonical_sha256
                    .clone(),
                previous_projection_frame_canonical_sha256: frame
                    .previous_projection_frame_canonical_sha256
                    .clone(),
                previous_projection_socket_transform_inventory_sha256: frame
                    .previous_projection_socket_transform_inventory_sha256
                    .clone(),
                previous_projection_socket_transform_readback_sha256: frame
                    .previous_projection_socket_transform_readback_sha256
                    .clone(),
                particle_sequence_key_sha256: frame.particle_sequence_key_sha256.clone(),
                base_frame_key_sha256: frame.base_frame_key_sha256.clone(),
                bloom_key_sha256: frame.bloom_key_sha256.clone(),
                camera_object_sha256: frame.camera_object_sha256.clone(),
                camera_identity_sha256: frame.camera_identity_sha256.clone(),
                render_profile_sha256: frame.render_profile_sha256.clone(),
                render_worker_build_cohort_sha256: frame.render_worker_build_cohort_sha256.clone(),
            },
        )
        .collect::<Vec<_>>();
    FictionalEnergyVfxAnimatedSocketTrailsSequencePrepareRequest {
        schema_version: PREPARE_SCHEMA.to_owned(),
        sequence_key_sha256: stored.sequence_key_sha256.clone(),
        project_id: stored.project_id.clone(),
        candidate_id: stored.candidate_id.clone(),
        candidate_state_sha256: stored.candidate_state_sha256.clone(),
        delivery_manifest_object_sha256: stored.delivery_manifest_object_sha256.clone(),
        source_artifact_sha256: stored.source_artifact_sha256.clone(),
        projection_key_sha256: stored.projection_key_sha256.clone(),
        projection_object_sha256: stored.projection_object_sha256.clone(),
        projection_canonical_sha256: stored.projection_canonical_sha256.clone(),
        animated_socket_materialization_key_sha256: stored
            .animated_socket_materialization_key_sha256
            .clone(),
        animated_artifact_sha256: stored.animated_artifact_sha256.clone(),
        animated_socket_anchor_set_object_sha256: stored
            .animated_socket_anchor_set_object_sha256
            .clone(),
        animated_socket_anchor_set_canonical_sha256: stored
            .animated_socket_anchor_set_canonical_sha256
            .clone(),
        animation_clip_id: stored.animation_clip_id.clone(),
        animation_clip_object_sha256: stored.animation_clip_object_sha256.clone(),
        animation_clip_canonical_sha256: stored.animation_clip_canonical_sha256.clone(),
        animation_receipt_object_sha256: stored.animation_receipt_object_sha256.clone(),
        animation_receipt_canonical_sha256: stored.animation_receipt_canonical_sha256.clone(),
        vfx_profile_object_sha256: stored.vfx_profile_object_sha256.clone(),
        vfx_profile_canonical_sha256: stored.vfx_profile_canonical_sha256.clone(),
        socket_node_id_encoding_sha256: stored.socket_node_id_encoding_sha256.clone(),
        socket_roles_sha256: stored.socket_roles_sha256.clone(),
        camera_object_sha256: stored.camera_object_sha256.clone(),
        camera_identity_sha256: stored.camera_identity_sha256.clone(),
        render_profile_sha256: stored.render_profile_sha256.clone(),
        render_worker_build_cohort_sha256: stored.render_worker_build_cohort_sha256.clone(),
        sample_schedule_sha256: stored.sample_schedule_sha256.clone(),
        sample_count: stored.sample_count,
        sample_time_ticks: stored.sample_time_ticks.clone(),
        frame_scope: stored.frame_scope.clone(),
        trails_sequence_policy: stored.trails_sequence_policy.clone(),
        history_policy: stored.history_policy.clone(),
        history_pre_roll_policy: stored.history_pre_roll_policy.clone(),
        trail_count: stored.trail_count,
        trail_emitter_roles: stored.trail_emitter_roles.clone(),
        frames,
        input_sha256: stored.input_sha256.clone(),
        idempotency_key: stored.sequence_key_sha256.clone(),
    }
}

pub(super) fn get(runtime: &Runtime, value: &Value) -> Result<Value, RuntimeError> {
    let request = parse_get(value)?;
    let stored = runtime
        .store
        .get_fictional_energy_vfx_animated_socket_trails_sequence(&request.sequence_key_sha256)?
        .ok_or_else(|| invalid("animated socket trail sequence is unavailable"))?;
    if stored.project_id != request.project_id || stored.candidate_id != request.candidate_id {
        return Err(invalid("animated socket trail sequence scope differs"));
    }
    let replay = replay_request(&stored);
    let context = build_context(runtime, &replay)?;
    for (computed, expected) in context.frames.iter().zip(&stored.frames) {
        if computed.worker.projection_sample_set_sha256 != expected.projection_sample_set_sha256
            || computed.worker.emitter_binding_sha256 != expected.emitter_binding_sha256
            || computed.worker.seed_sha256 != expected.trail_seed_sha256
            || computed.worker.trail_inventory_sha256 != expected.trail_inventory_sha256
            || computed.trails != expected.trails
            || computed.history_samples != expected.history_samples
        {
            return Err(invalid("animated socket trail replay binding differs"));
        }
        for (pass, hash) in computed.worker.trail_passes.iter().zip([
            &expected.trail_color_object_sha256,
            &expected.trail_id_object_sha256,
            &expected.trail_depth_object_sha256,
        ]) {
            let bytes = runtime.cas_read_bounded(hash, 4 * 1024 * 1024)?;
            if sha256_hex(&bytes) != *hash || bytes != pass.png {
                return Err(invalid(
                    "animated socket trail pass bytes differ after restart",
                ));
            }
        }
        let render_set = read_owned_canonical_json(
            runtime,
            &expected.render_set_object_sha256,
            RENDER_SET_SCHEMA,
        )?;
        let pass_artifacts = render_set
            .get("pass_artifacts")
            .and_then(Value::as_array)
            .filter(|values| values.len() == 3)
            .ok_or_else(|| invalid("trail render set pass inventory is malformed"))?;
        if render_set
            .get("sequence_key_sha256")
            .and_then(Value::as_str)
            != Some(stored.sequence_key_sha256.as_str())
            || render_set.get("frame_index").and_then(Value::as_u64) != Some(expected.frame_index)
            || render_set.get("trail_key_sha256").and_then(Value::as_str)
                != Some(expected.trail_key_sha256.as_str())
            || render_set.get("trail_inventory").is_none()
            || pass_artifacts
                .iter()
                .zip([
                    &expected.trail_color_object_sha256,
                    &expected.trail_id_object_sha256,
                    &expected.trail_depth_object_sha256,
                ])
                .any(|(pass, hash)| {
                    pass.get("sha256").and_then(Value::as_str) != Some(hash.as_str())
                })
        {
            return Err(invalid("animated socket trail render set differs"));
        }
        let receipt = read_owned_canonical_json(
            runtime,
            &expected.receipt_object_sha256,
            FRAME_RECEIPT_SCHEMA,
        )?;
        let expected_frame = frame_without_receipt(expected);
        if receipt.get("frame") != Some(&expected_frame)
            || receipt.get("trails")
                != Some(
                    &serde_json::to_value(&computed.trails)
                        .map_err(|error| invalid(error.to_string()))?,
                )
            || receipt.get("history_samples")
                != Some(
                    &serde_json::to_value(&computed.history_samples)
                        .map_err(|error| invalid(error.to_string()))?,
                )
            || receipt.get("trail_inventory") != Some(&computed.worker.trail_inventory)
            || receipt
                .get("worker_replay_byte_exact")
                .and_then(Value::as_bool)
                != Some(true)
        {
            return Err(invalid("animated socket trail frame receipt differs"));
        }
    }
    result_value(&stored, true, GET_RESULT_SCHEMA, false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn transform_frame() -> Value {
        json!({
            "canonical_sha256":"a".repeat(64),
            "socket_transform_inventory_sha256":"b".repeat(64),
            "socket_transform_readback_sha256":"c".repeat(64),
            "socket_transforms":[
                {"socket_node_id":"socket-muzzle-vfx","anchor_id":"socket-muzzle-vfx","role":"muzzle-vfx","node_kind":"empty","owner_part_id":"barrel-assembly","composed_world_transform":{"translation_m":[0.0,0.0,2.0],"rotation_quat_xyzw":[0.0,0.0,0.0,1.0],"scale_xyz":[1.0,1.0,1.0]}},
                {"socket_node_id":"socket-energy-core-vfx","anchor_id":"socket-energy-core-vfx","role":"energy-core-vfx","node_kind":"empty","owner_part_id":"energy-core","composed_world_transform":{"translation_m":[0.0,0.0,2.0],"rotation_quat_xyzw":[0.0,0.0,0.0,1.0],"scale_xyz":[1.0,1.0,1.0]}}
            ]
        })
    }

    #[test]
    fn animated_trail_emitters_are_fixed_and_trs_sensitive() {
        let first = emitter_bindings(&transform_frame()).unwrap();
        let mut changed = transform_frame();
        changed["socket_transforms"][0]["composed_world_transform"]["translation_m"] =
            json!([0.2, 0.0, 2.0]);
        assert_ne!(first, emitter_bindings(&changed).unwrap());
        assert_eq!(first["emitters"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn animated_trail_rejects_retargeted_role() {
        let mut frame = transform_frame();
        frame["socket_transforms"][1]["role"] = json!("muzzle-vfx");
        assert!(emitter_bindings(&frame).is_err());
    }

    #[test]
    fn animated_trail_id_encoding_is_stable() {
        let first =
            canonical_json_hash(&json!({"muzzle-vfx":30000_u64,"energy-core-vfx":31000_u64}));
        let second =
            canonical_json_hash(&json!({"muzzle-vfx":30000_u64,"energy-core-vfx":31000_u64}));
        assert_eq!(first, second);
        assert!(is_sha256(&first));
    }

    fn trail_input_fixture() -> FictionalEnergyVfxAnimatedSocketTrailsSequenceFrameInput {
        let hash = || "a".repeat(64);
        FictionalEnergyVfxAnimatedSocketTrailsSequenceFrameInput {
            frame_index: 0,
            sample_time_ticks: 10,
            history_origin: HISTORY_PREROLL_POLICY.to_owned(),
            current_projection_frame_index: 1,
            current_particle_frame_index: 1,
            current_particle_key_sha256: hash(),
            current_particle_frame_canonical_sha256: hash(),
            current_projection_frame_canonical_sha256: hash(),
            current_projection_socket_transform_inventory_sha256: hash(),
            current_projection_socket_transform_readback_sha256: hash(),
            previous_projection_frame_index: 0,
            previous_particle_frame_index: 0,
            previous_particle_sequence_frame_canonical_sha256: hash(),
            previous_projection_frame_canonical_sha256: hash(),
            previous_projection_socket_transform_inventory_sha256: hash(),
            previous_projection_socket_transform_readback_sha256: hash(),
            particle_sequence_key_sha256: hash(),
            base_frame_key_sha256: hash(),
            bloom_key_sha256: hash(),
            camera_object_sha256: hash(),
            camera_identity_sha256: hash(),
            render_profile_sha256: hash(),
            render_worker_build_cohort_sha256: hash(),
        }
    }

    fn persisted_frame_fixture(
        input: &FictionalEnergyVfxAnimatedSocketTrailsSequenceFrameInput,
    ) -> FictionalEnergyVfxAnimatedSocketTrailsSequenceFrame {
        let mut value = serde_json::to_value(input).expect("fixture input serializes");
        let object = value.as_object_mut().expect("fixture is an object");
        let hash = "b".repeat(64);
        object.insert("schema_version".to_owned(), json!(FRAME_SCHEMA));
        object.insert("history_samples".to_owned(), json!([]));
        object.insert("trail_count".to_owned(), json!(2));
        object.insert(
            "trail_emitter_roles".to_owned(),
            json!(["muzzle-vfx", "energy-core-vfx"]),
        );
        object.insert(
            "trails".to_owned(),
            json!([
                {"emitter_role":"muzzle-vfx","trail_id":30000,"points":[{"source_frame_index":0,"sample_time_ticks":1,"source_particle_key_sha256":hash,"source_particle_frame_index":0,"source_particle_id":10000,"local_offset_micrometers":[0,0,0],"world_position_micrometers":[0,0,0],"depth_micrometers":1}]},
                {"emitter_role":"energy-core-vfx","trail_id":31000,"points":[{"source_frame_index":0,"sample_time_ticks":1,"source_particle_key_sha256":hash,"source_particle_frame_index":0,"source_particle_id":20000,"local_offset_micrometers":[0,0,0],"world_position_micrometers":[0,0,0],"depth_micrometers":1}]}
            ]),
        );
        for field in [
            "projection_sample_set_sha256",
            "trail_key_sha256",
            "trail_seed_sha256",
            "trail_inventory_sha256",
            "trail_id_encoding_sha256",
            "emitter_binding_sha256",
            "trail_color_object_sha256",
            "trail_id_object_sha256",
            "trail_depth_object_sha256",
            "render_set_object_sha256",
            "receipt_object_sha256",
        ] {
            object.insert(field.to_owned(), json!(hash));
        }
        object.insert("canonical_sha256".to_owned(), json!(hash));
        object.insert("created_at".to_owned(), json!("2026-08-22T00:00:00Z"));
        serde_json::from_value(value).expect("fixture frame deserializes")
    }

    #[test]
    fn frame_input_retarget_cannot_fast_replay() {
        let input = trail_input_fixture();
        let frame = persisted_frame_fixture(&input);
        assert!(frame_input_matches(&input, &frame));

        let mut changed = input.clone();
        changed.history_origin = "retargeted-history@1".to_owned();
        assert!(!frame_input_matches(&changed, &frame));
        changed = input.clone();
        changed.current_projection_frame_index = 2;
        assert!(!frame_input_matches(&changed, &frame));
        changed = input.clone();
        changed.previous_particle_frame_index = 1;
        assert!(!frame_input_matches(&changed, &frame));
        changed = input.clone();
        changed.current_projection_socket_transform_inventory_sha256 = "c".repeat(64);
        assert!(!frame_input_matches(&changed, &frame));
        changed = input.clone();
        changed.particle_sequence_key_sha256 = "d".repeat(64);
        assert!(!frame_input_matches(&changed, &frame));
        changed = input.clone();
        changed.camera_object_sha256 = "e".repeat(64);
        assert!(!frame_input_matches(&changed, &frame));
        changed = input.clone();
        changed.render_profile_sha256 = "f".repeat(64);
        assert!(!frame_input_matches(&changed, &frame));
        changed = input;
        changed.render_worker_build_cohort_sha256 = "0".repeat(64);
        assert!(!frame_input_matches(&changed, &frame));
    }

    #[test]
    fn owned_render_sidecar_canonical_hash_is_fail_closed() {
        let (value, _) = canonical_object(json!({
            "schema_version": RENDER_SET_SCHEMA,
            "payload": "immutable"
        }))
        .unwrap();
        verify_owned_canonical_json(&value).unwrap();
        let mut tampered = value;
        tampered["payload"] = json!("retargeted");
        assert!(verify_owned_canonical_json(&tampered).is_err());
    }
}
