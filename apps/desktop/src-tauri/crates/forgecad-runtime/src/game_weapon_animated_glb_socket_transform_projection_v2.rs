//! Additive appearance-aware six-socket transform projection.
//!
//! This module is deliberately independent from the historical V1 projection
//! producer.  It consumes only the durable `MechanicalAnimationGlb@2` and
//! `GameWeaponAnimatedGlbSocket@2` get/receipt surfaces, reuses the bounded V1
//! accessor sampling math, and owns one V2 projection CAS object.  All source
//! validation happens before the CAS reservation is opened.

use super::{
    canonical_json_bytes, canonical_json_hash, game_asset_delivery,
    game_weapon_animated_glb_socket_v2, mechanical_animation_clip_v2, mechanical_animation_glb_v2,
    now_string, rigid_animation_glb, sha256_hex, Runtime, RuntimeError,
};
use forgecad_contracts::{
    GameWeaponAnimatedGlbSocketMaterializationV2LinkRecord,
    GameWeaponAnimatedGlbSocketMaterializationV2ReceiptRecord,
    GameWeaponAnimatedGlbSocketTransformProjectionV2,
    GameWeaponAnimatedGlbSocketTransformProjectionV2Frame, MechanicalAnimationClipV2LinkRecord,
    MechanicalAnimationClipV2Record, MechanicalAnimationGlbV2LinkRecord,
    MechanicalAnimationGlbV2ReceiptRecord,
};
use serde_json::{json, Map, Value};

const MAX_GLB_BYTES: usize = 64 * 1024 * 1024;
const MAX_JSON_BYTES: usize = 1024 * 1024;
const PREPARE_SCHEMA: &str = "GameWeaponAnimatedGlbSocketTransformProjectionPrepareRequest@2";
const GET_SCHEMA: &str = "GameWeaponAnimatedGlbSocketTransformProjectionGetRequest@2";
const PREPARE_RESULT_SCHEMA: &str = "GameWeaponAnimatedGlbSocketTransformProjectionPrepareResult@2";
const GET_RESULT_SCHEMA: &str = "GameWeaponAnimatedGlbSocketTransformProjectionGetResult@2";
const PROJECTION_SCHEMA: &str = "GameWeaponAnimatedGlbSocketTransformProjection@2";
const FRAME_SCHEMA: &str = "GameWeaponAnimatedGlbSocketTransformProjectionFrame@2";
const PROJECTION_KIND: &str = "game-weapon-animated-glb-v2-socket-transform-projection";
const PROJECTION_STATUS: &str =
    "runtime-owned-durable-game-weapon-animated-glb-socket-transform-projection-v2";
const PART_HIERARCHY_POLICY: &str = "flat-identity-rest-part-hierarchy-only@2";
const TRANSFORM_REPRESENTATION_POLICY: &str = "trs-quaternion-no-shear-plus-column-major-matrix@2";
const FRAME_SCOPE: &str = "lod0-animation-frame-range-1-16@2";
const TRANSFORM_PROJECTION_POLICY: &str =
    "glb-animation-linear-nlerp-flat-part-hierarchy-composed-world-trs-matrix@2";
const COORDINATE_SYSTEM: &str = "forgecad-rh-y-up-m@1";
const TRANSFORM_CONVENTION: &str = "column-vector-parent-world-times-trs-quaternion-xyzw@1";
const FLOAT_POLICY: &str = "f32-round-nearest-canonical-json@1";
const SOCKET_ROLES: [&str; 6] = [
    "weapon-root",
    "grip-primary",
    "muzzle-vfx",
    "magazine-well",
    "sight-primary",
    "energy-core-vfx",
];
const LIMITATIONS: [&str; 9] = [
    "appearance-candidate-bound-mechanical-animation-glb-v2-and-animated-socket-v2",
    "parent-clip-sample-exact-or-bounded-sixteen-samples",
    "flat-identity-rest-part-hierarchy-only",
    "nested-part-hierarchy-rejected",
    "matrix-and-shear-rejected",
    "structural-transform-readback-only",
    "no-visual-quality-or-likeness-pass",
    "no-commercial-engine-roundtrip",
    "no-functional-weapon-semantics",
];

const PREPARE_FIELDS: &[&str] = &[
    "schema_version",
    "projection_key_sha256",
    "project_id",
    "appearance_candidate_id",
    "appearance_candidate_state_sha256",
    "appearance_delivery_manifest_object_sha256",
    "appearance_artifact_sha256",
    "appearance_artifact_readback_sha256",
    "animation_clip_id",
    "animation_clip_object_sha256",
    "animation_clip_canonical_sha256",
    "animation_glb_key_sha256",
    "animated_artifact_sha256",
    "animated_artifact_readback_sha256",
    "animation_receipt_object_sha256",
    "animation_receipt_canonical_sha256",
    "animated_socket_materialization_key_sha256",
    "derived_animated_socket_artifact_sha256",
    "derived_animated_socket_artifact_readback_sha256",
    "derived_animated_socket_receipt_object_sha256",
    "derived_animated_socket_receipt_canonical_sha256",
    "anchor_set_object_sha256",
    "anchor_set_canonical_sha256",
    "socket_node_id_encoding_sha256",
    "socket_node_inventory_sha256",
    "socket_roles_sha256",
    "socket_roles",
    "part_hierarchy_sha256",
    "part_hierarchy_policy",
    "transform_representation_policy",
    "sampling_policy_sha256",
    "sample_schedule_sha256",
    "sample_count",
    "sample_time_ticks",
    "frame_scope",
    "timebase_hz",
    "transform_projection_policy",
    "coordinate_system",
    "transform_convention",
    "float_quantization_policy",
    "input_sha256",
    "idempotency_key",
];
const GET_FIELDS: &[&str] = &[
    "schema_version",
    "projection_key_sha256",
    "project_id",
    "appearance_candidate_id",
    "animation_clip_id",
];

type ProjectionRequest =
    forgecad_contracts::GameWeaponAnimatedGlbSocketTransformProjectionV2PrepareRequest;

fn invalid(detail: impl Into<String>) -> RuntimeError {
    RuntimeError::InvalidInput(format!(
        "GAME_WEAPON_ANIMATED_GLB_SOCKET_TRANSFORM_PROJECTION_V2_INVALID: {}",
        detail.into()
    ))
}

fn exact_object<'a>(
    value: &'a Value,
    fields: &[&str],
    context: &str,
) -> Result<&'a Map<String, Value>, RuntimeError> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid(format!("{context} must be an object")))?;
    if object.len() != fields.len()
        || fields.iter().any(|field| !object.contains_key(*field))
        || object.keys().any(|field| !fields.contains(&field.as_str()))
    {
        return Err(invalid(format!("{context} field set differs")));
    }
    Ok(object)
}

fn valid_id(value: &str) -> bool {
    forgecad_contracts::is_opaque_id(value)
}

fn valid_hash(value: &str) -> bool {
    forgecad_contracts::is_sha256(value)
}

fn require_request_hashes(request: &ProjectionRequest) -> Result<(), RuntimeError> {
    for hash in [
        &request.projection_key_sha256,
        &request.appearance_candidate_state_sha256,
        &request.appearance_delivery_manifest_object_sha256,
        &request.appearance_artifact_sha256,
        &request.appearance_artifact_readback_sha256,
        &request.animation_clip_object_sha256,
        &request.animation_clip_canonical_sha256,
        &request.animation_glb_key_sha256,
        &request.animated_artifact_sha256,
        &request.animated_artifact_readback_sha256,
        &request.animation_receipt_object_sha256,
        &request.animation_receipt_canonical_sha256,
        &request.animated_socket_materialization_key_sha256,
        &request.derived_animated_socket_artifact_sha256,
        &request.derived_animated_socket_artifact_readback_sha256,
        &request.derived_animated_socket_receipt_object_sha256,
        &request.derived_animated_socket_receipt_canonical_sha256,
        &request.anchor_set_object_sha256,
        &request.anchor_set_canonical_sha256,
        &request.socket_node_id_encoding_sha256,
        &request.socket_node_inventory_sha256,
        &request.socket_roles_sha256,
        &request.part_hierarchy_sha256,
        &request.sampling_policy_sha256,
        &request.sample_schedule_sha256,
        &request.input_sha256,
    ] {
        if !valid_hash(hash) {
            return Err(invalid("projection request contains an invalid SHA-256"));
        }
    }
    for id in [
        &request.project_id,
        &request.appearance_candidate_id,
        &request.animation_clip_id,
        &request.idempotency_key,
    ] {
        if !valid_id(id) {
            return Err(invalid("projection request contains an invalid identifier"));
        }
    }
    Ok(())
}

fn parse_prepare(value: &Value) -> Result<ProjectionRequest, RuntimeError> {
    exact_object(value, PREPARE_FIELDS, PREPARE_SCHEMA)?;
    let request: ProjectionRequest = serde_json::from_value(value.clone())
        .map_err(|error| invalid(format!("projection V2 request is malformed: {error}")))?;
    require_request_hashes(&request)?;
    if request.schema_version != PREPARE_SCHEMA
        || request
            .socket_roles
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
            != SOCKET_ROLES
        || request.part_hierarchy_policy != PART_HIERARCHY_POLICY
        || request.transform_representation_policy != TRANSFORM_REPRESENTATION_POLICY
        || request.frame_scope != FRAME_SCOPE
        || request.timebase_hz != 1000
        || request.transform_projection_policy != TRANSFORM_PROJECTION_POLICY
        || request.coordinate_system != COORDINATE_SYSTEM
        || request.transform_convention != TRANSFORM_CONVENTION
        || request.float_quantization_policy != FLOAT_POLICY
        || request.sample_count as usize != request.sample_time_ticks.len()
        || !(1..=16).contains(&request.sample_time_ticks.len())
        || request
            .sample_time_ticks
            .iter()
            .any(|tick| *tick > 1_000_000)
        || request
            .sample_time_ticks
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
    {
        return Err(invalid("projection V2 policy or bounded schedule differs"));
    }
    let object = value
        .as_object()
        .ok_or_else(|| invalid("projection V2 request is not an object"))?;
    let mut preimage = Value::Object(object.clone());
    let preimage_object = preimage
        .as_object_mut()
        .ok_or_else(|| invalid("projection V2 request is not an object"))?;
    preimage_object.remove("projection_key_sha256");
    preimage_object.remove("input_sha256");
    preimage_object.remove("idempotency_key");
    let expected_input = canonical_json_hash(&preimage);
    if request.input_sha256 != expected_input || request.projection_key_sha256 != expected_input {
        return Err(invalid("projection V2 input/key hash differs"));
    }
    if request.socket_roles_sha256
        != canonical_json_hash(
            object
                .get("socket_roles")
                .ok_or_else(|| invalid("projection V2 socket_roles is missing"))?,
        )
    {
        return Err(invalid("projection V2 socket role hash differs"));
    }
    let schedule_hash = canonical_json_hash(&json!({
        "frame_scope": request.frame_scope,
        "sample_time_ticks": request.sample_time_ticks,
        "timebase_hz": request.timebase_hz,
    }));
    if request.sample_schedule_sha256 != schedule_hash {
        return Err(invalid("projection V2 sample schedule hash differs"));
    }
    Ok(request)
}

fn parse_get(value: &Value) -> Result<(String, String, String, String), RuntimeError> {
    let object = exact_object(value, GET_FIELDS, GET_SCHEMA)?;
    let schema = object
        .get("schema_version")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("projection V2 get schema is missing"))?;
    let key = object
        .get("projection_key_sha256")
        .and_then(Value::as_str)
        .filter(|value| valid_hash(value))
        .ok_or_else(|| invalid("projection V2 get key is invalid"))?;
    let project = object
        .get("project_id")
        .and_then(Value::as_str)
        .filter(|value| valid_id(value))
        .ok_or_else(|| invalid("projection V2 get project is invalid"))?;
    let candidate = object
        .get("appearance_candidate_id")
        .and_then(Value::as_str)
        .filter(|value| valid_id(value))
        .ok_or_else(|| invalid("projection V2 get appearance candidate is invalid"))?;
    let clip = object
        .get("animation_clip_id")
        .and_then(Value::as_str)
        .filter(|value| valid_id(value))
        .ok_or_else(|| invalid("projection V2 get clip is invalid"))?;
    if schema != GET_SCHEMA {
        return Err(invalid("projection V2 get schema differs"));
    }
    Ok((
        project.to_owned(),
        candidate.to_owned(),
        clip.to_owned(),
        key.to_owned(),
    ))
}

fn parse_link<T: serde::de::DeserializeOwned>(
    value: &Value,
    field: &str,
    label: &str,
) -> Result<T, RuntimeError> {
    serde_json::from_value(
        value
            .get(field)
            .cloned()
            .ok_or_else(|| invalid(format!("{label} {field} is unavailable")))?,
    )
    .map_err(|error| invalid(format!("{label} {field} is malformed: {error}")))
}

fn require_canonical_value(value: &Value, label: &str) -> Result<(), RuntimeError> {
    let declared = value
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .filter(|hash| valid_hash(hash))
        .ok_or_else(|| invalid(format!("{label} canonical hash is invalid")))?;
    let mut preimage = value.clone();
    preimage["canonical_sha256"] = Value::String(String::new());
    if canonical_json_hash(&preimage) != declared {
        return Err(invalid(format!("{label} canonical hash differs")));
    }
    Ok(())
}

fn read_canonical_json(
    runtime: &Runtime,
    hash: &str,
    schema: &str,
    label: &str,
) -> Result<Value, RuntimeError> {
    let bytes = runtime.cas_read_bounded(hash, MAX_JSON_BYTES as u64)?;
    if sha256_hex(&bytes) != hash {
        return Err(invalid(format!("{label} CAS hash differs")));
    }
    let value: Value = serde_json::from_slice(&bytes)
        .map_err(|error| invalid(format!("{label} JSON is malformed: {error}")))?;
    if value.get("schema_version").and_then(Value::as_str) != Some(schema) {
        return Err(invalid(format!("{label} schema differs")));
    }
    require_canonical_value(&value, label)?;
    if canonical_json_bytes(&value).map_err(|error| invalid(error.to_string()))? != bytes {
        return Err(invalid(format!("{label} is not canonical JSON")));
    }
    Ok(value)
}

fn ensure_cas_object(
    runtime: &Runtime,
    hash: &str,
    mime: &str,
    kinds: &[&str],
    max_bytes: u64,
    label: &str,
) -> Result<(), RuntimeError> {
    let object = runtime
        .store
        .get_object(hash)?
        .ok_or_else(|| invalid(format!("{label} CAS object is unavailable")))?;
    if object.mime != mime
        || !kinds.iter().any(|kind| object.kind == *kind)
        || object.size_bytes == 0
        || object.size_bytes > max_bytes
    {
        return Err(invalid(format!("{label} CAS metadata differs")));
    }
    Ok(())
}

fn compare_clip(
    request: &ProjectionRequest,
    clip: &MechanicalAnimationClipV2Record,
    link: &MechanicalAnimationClipV2LinkRecord,
) -> Result<Vec<u64>, RuntimeError> {
    if clip.schema_version != "MechanicalAnimationClip@2"
        || link.schema_version != "MechanicalAnimationClipLink@2"
        || clip.project_id != request.project_id
        || clip.appearance_candidate_id != request.appearance_candidate_id
        || clip.appearance_candidate_state_sha256 != request.appearance_candidate_state_sha256
        || clip.clip_id != request.animation_clip_id
        || link.project_id != request.project_id
        || link.appearance_candidate_id != request.appearance_candidate_id
        || link.appearance_candidate_state_sha256 != request.appearance_candidate_state_sha256
        || link.clip_id != request.animation_clip_id
        || link.clip_object_sha256 != request.animation_clip_object_sha256
        || link.clip_sha256 != request.animation_clip_canonical_sha256
        || clip.canonical_sha256 != request.animation_clip_canonical_sha256
    {
        return Err(invalid("Clip@2 binding differs"));
    }
    let ticks = clip
        .sampling_policy
        .get("sample_time_ticks")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("Clip@2 sample schedule is unavailable"))?
        .iter()
        .map(|value| {
            value
                .as_u64()
                .ok_or_else(|| invalid("Clip@2 sample tick is invalid"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if ticks != request.sample_time_ticks {
        return Err(invalid("Clip@2 sample schedule differs"));
    }
    if canonical_json_hash(&clip.sampling_policy) != request.sampling_policy_sha256 {
        return Err(invalid("Clip@2 sampling policy hash differs"));
    }
    Ok(ticks)
}

fn compare_animation(
    request: &ProjectionRequest,
    link: &MechanicalAnimationGlbV2LinkRecord,
    receipt: &MechanicalAnimationGlbV2ReceiptRecord,
) -> Result<(), RuntimeError> {
    if link.schema_version != "MechanicalAnimationGlbLink@2"
        || receipt.schema_version != "MechanicalAnimationGlbReceipt@2"
        || link.animation_glb_key_sha256 != request.animation_glb_key_sha256
        || link.project_id != request.project_id
        || link.appearance_candidate_id != request.appearance_candidate_id
        || link.appearance_candidate_state_sha256 != request.appearance_candidate_state_sha256
        || link.appearance_artifact_sha256 != request.appearance_artifact_sha256
        || link.appearance_artifact_readback_sha256 != request.appearance_artifact_readback_sha256
        || link.clip_id != request.animation_clip_id
        || link.clip_object_sha256 != request.animation_clip_object_sha256
        || link.clip_sha256 != request.animation_clip_canonical_sha256
        || link.animated_artifact_sha256 != request.animated_artifact_sha256
        || link.animated_artifact_readback_sha256 != request.animated_artifact_readback_sha256
        || link.receipt_object_sha256 != request.animation_receipt_object_sha256
        || link.receipt_canonical_sha256 != request.animation_receipt_canonical_sha256
        || receipt.animation_glb_key_sha256 != request.animation_glb_key_sha256
        || receipt.animated_artifact_sha256 != request.animated_artifact_sha256
        || receipt.animated_artifact_readback_sha256 != request.animated_artifact_readback_sha256
        || receipt.canonical_sha256 != request.animation_receipt_canonical_sha256
        || receipt.sample_time_ticks != request.sample_time_ticks
        || receipt.timebase_hz != 1000
        || receipt.interpolation != "LINEAR"
        || receipt.sampling_policy_sha256 != request.sampling_policy_sha256
    {
        return Err(invalid("MechanicalAnimationGlb@2 binding differs"));
    }
    Ok(())
}

fn compare_socket(
    request: &ProjectionRequest,
    link: &GameWeaponAnimatedGlbSocketMaterializationV2LinkRecord,
    receipt: &GameWeaponAnimatedGlbSocketMaterializationV2ReceiptRecord,
) -> Result<(), RuntimeError> {
    if link.schema_version != "GameWeaponAnimatedGlbSocketMaterializationLink@2"
        || receipt.schema_version != "GameWeaponAnimatedGlbSocketMaterializationReceipt@2"
        || link.animated_socket_materialization_key_sha256
            != request.animated_socket_materialization_key_sha256
        || link.project_id != request.project_id
        || link.appearance_candidate_id != request.appearance_candidate_id
        || link.appearance_candidate_state_sha256 != request.appearance_candidate_state_sha256
        || link.appearance_delivery_manifest_object_sha256
            != request.appearance_delivery_manifest_object_sha256
        || link.appearance_artifact_sha256 != request.appearance_artifact_sha256
        || link.appearance_artifact_readback_sha256 != request.appearance_artifact_readback_sha256
        || link.animation_glb_key_sha256 != request.animation_glb_key_sha256
        || link.animated_artifact_sha256 != request.animated_artifact_sha256
        || link.animated_artifact_readback_sha256 != request.animated_artifact_readback_sha256
        || link.animation_receipt_object_sha256 != request.animation_receipt_object_sha256
        || link.animation_receipt_canonical_sha256 != request.animation_receipt_canonical_sha256
        || link.clip_id != request.animation_clip_id
        || link.clip_object_sha256 != request.animation_clip_object_sha256
        || link.clip_sha256 != request.animation_clip_canonical_sha256
        || link.anchor_set_object_sha256 != request.anchor_set_object_sha256
        || link.anchor_set_canonical_sha256 != request.anchor_set_canonical_sha256
        || link.socket_node_id_encoding_sha256 != request.socket_node_id_encoding_sha256
        || link.derived_animated_socket_artifact_sha256
            != request.derived_animated_socket_artifact_sha256
        || link.derived_animated_socket_artifact_readback_sha256
            != request.derived_animated_socket_artifact_readback_sha256
        || link.receipt_object_sha256 != request.derived_animated_socket_receipt_object_sha256
        || receipt.canonical_sha256 != request.derived_animated_socket_receipt_canonical_sha256
        || receipt.socket_node_inventory_sha256 != request.socket_node_inventory_sha256
        || receipt.socket_node_id_encoding_sha256 != request.socket_node_id_encoding_sha256
        || receipt.sample_time_ticks != request.sample_time_ticks
        || receipt.socket_node_count != 6
        || receipt.socket_materialization_policy
            != "appearance-aware-animation-v2-socket-node-materialization-preserve-renderable-content@2"
        || receipt.lod_scope != "lod0-appearance-animated-source-only@2"
    {
        return Err(invalid("AnimatedSocket@2 binding differs"));
    }
    Ok(())
}

fn part_hierarchy_hash(part_ids: &[String]) -> String {
    canonical_json_hash(&json!({
        "schema_version":"GameWeaponAnimatedGlbSocketPartHierarchy@2",
        "policy":PART_HIERARCHY_POLICY,
        "nodes":part_ids.iter().enumerate().map(|(index, part_id)| json!({
            "node_index":index,
            "part_id":part_id,
            "parent_node_index":-1,
            "parent_node_name":Value::Null,
            "children":[]
        })).collect::<Vec<_>>()
    }))
}

fn matrix_from_pose(pose: rigid_animation_glb::ProjectionPose) -> Vec<f64> {
    let [x, y, z, w] = pose.rotation;
    let xx = x * x;
    let yy = y * y;
    let zz = z * z;
    let xy = x * y;
    let xz = x * z;
    let yz = y * z;
    let xw = x * w;
    let yw = y * w;
    let zw = z * w;
    [
        1.0 - 2.0 * (yy + zz),
        2.0 * (xy + zw),
        2.0 * (xz - yw),
        0.0,
        2.0 * (xy - zw),
        1.0 - 2.0 * (xx + zz),
        2.0 * (yz + xw),
        0.0,
        2.0 * (xz + yw),
        2.0 * (yz - xw),
        1.0 - 2.0 * (xx + yy),
        0.0,
        pose.translation[0],
        pose.translation[1],
        pose.translation[2],
        1.0,
    ]
    .into_iter()
    .map(rigid_animation_glb::projection_f32_json_number)
    .collect()
}

fn pose_value(pose: rigid_animation_glb::ProjectionPose) -> Value {
    json!({
        "translation_m": pose.translation.map(rigid_animation_glb::projection_f32_json_number),
        "rotation_quat_xyzw": pose.rotation.map(rigid_animation_glb::projection_f32_json_number),
        "scale_xyz":[1.0,1.0,1.0]
    })
}

fn build_frame(
    projection_key: &str,
    frame_index: u64,
    sample_time_ticks: u64,
    animation: &rigid_animation_glb::ProjectionAnimation,
    sockets: &[rigid_animation_glb::ProjectionSocketNode],
    part_ids: &[String],
    animated_artifact_sha256: &str,
    derived_artifact_sha256: &str,
) -> Result<GameWeaponAnimatedGlbSocketTransformProjectionV2Frame, RuntimeError> {
    let mut part_poses = vec![rigid_animation_glb::projection_identity_pose(); part_ids.len()];
    for (node_index, pose) in part_poses.iter_mut().enumerate() {
        let translation = animation
            .channels
            .iter()
            .find(|channel| channel.node_index == node_index && channel.path == "translation")
            .map(|channel| {
                rigid_animation_glb::sample_projection_channel(channel, sample_time_ticks)
            })
            .transpose()?
            .unwrap_or_else(|| vec![0.0, 0.0, 0.0]);
        let rotation = animation
            .channels
            .iter()
            .find(|channel| channel.node_index == node_index && channel.path == "rotation")
            .map(|channel| {
                rigid_animation_glb::sample_projection_channel(channel, sample_time_ticks)
            })
            .transpose()?
            .unwrap_or_else(|| vec![0.0, 0.0, 0.0, 1.0]);
        *pose = rigid_animation_glb::ProjectionPose {
            translation: rigid_animation_glb::projection_f32_array(
                &Value::Array(translation.into_iter().map(Value::from).collect()),
                1000.0,
                "sampled Part translation",
            )?,
            rotation: rigid_animation_glb::projection_normalize_quaternion(rotation)?,
        };
    }

    let part_sample = part_poses
        .iter()
        .enumerate()
        .map(|(node_index, pose)| {
            json!({
                "node_index":node_index,
                "part_id":part_ids[node_index],
                "local_transform":pose_value(*pose),
                "world_transform":pose_value(*pose)
            })
        })
        .collect::<Vec<_>>();
    let source_animation_sample_sha256 = canonical_json_hash(&json!({
        "schema_version":"GameWeaponAnimatedGlbSocketTransformProjectionSourceSample@2",
        "animated_artifact_sha256":animated_artifact_sha256,
        "sample_time_ticks":sample_time_ticks,
        "animation_projection_sha256":animation.source_animation_projection_sha256,
        "parts":part_sample
    }));

    let socket_values = sockets
        .iter()
        .map(|socket| {
            let parent_world = if socket.parent_node_index < 0 {
                rigid_animation_glb::projection_identity_pose()
            } else {
                *part_poses
                    .get(socket.parent_node_index as usize)
                    .ok_or_else(|| invalid("socket owner Part index overflows"))?
            };
            let composed = rigid_animation_glb::projection_compose(parent_world, socket.local);
            Ok(json!({
                "socket_node_id":socket.socket_node_id,
                "anchor_id":socket.anchor_id,
                "role":socket.role,
                "node_index":socket.node_index,
                "parent_node_index":socket.parent_node_index,
                "node_name":socket.node_name,
                "parent_node_name":socket.parent_node_name,
                "node_kind":"empty",
                "parent_kind":socket.parent_kind,
                "owner_part_id":socket.owner_part_id,
                "local_transform":pose_value(socket.local),
                "parent_world_transform":pose_value(parent_world),
                "composed_world_transform":pose_value(composed),
                "local_matrix_4x4":matrix_from_pose(socket.local),
                "parent_world_matrix_4x4":matrix_from_pose(parent_world),
                "composed_world_matrix_4x4":matrix_from_pose(composed)
            }))
        })
        .collect::<Result<Vec<_>, RuntimeError>>()?;
    let inventory = socket_values
        .iter()
        .map(|value| {
            let object = value
                .as_object()
                .ok_or_else(|| invalid("socket transform inventory is invalid"))?;
            Ok(json!({
                "socket_node_id":object.get("socket_node_id"),
                "anchor_id":object.get("anchor_id"),
                "role":object.get("role"),
                "node_index":object.get("node_index"),
                "parent_node_index":object.get("parent_node_index"),
                "node_name":object.get("node_name"),
                "parent_node_name":object.get("parent_node_name"),
                "node_kind":object.get("node_kind"),
                "parent_kind":object.get("parent_kind"),
                "owner_part_id":object.get("owner_part_id")
            }))
        })
        .collect::<Result<Vec<_>, RuntimeError>>()?;
    let socket_transform_inventory_sha256 = canonical_json_hash(&Value::Array(inventory));
    let derived_socket_sample_sha256 = canonical_json_hash(&json!({
        "schema_version":"GameWeaponAnimatedGlbSocketTransformProjectionDerivedSample@2",
        "derived_animated_socket_artifact_sha256":derived_artifact_sha256,
        "sample_time_ticks":sample_time_ticks,
        "animation_projection_sha256":animation.derived_animation_projection_sha256,
        "socket_transforms":socket_values
    }));

    let mut frame_value = json!({
        "schema_version":FRAME_SCHEMA,
        "projection_key_sha256":projection_key,
        "frame_index":frame_index,
        "sample_time_ticks":sample_time_ticks,
        "source_animation_sample_sha256":source_animation_sample_sha256,
        "derived_socket_sample_sha256":derived_socket_sample_sha256,
        "socket_transform_inventory_sha256":socket_transform_inventory_sha256,
        "socket_transform_readback_sha256":"",
        "projection_frame_canonical_sha256":"",
        "socket_transforms":socket_values,
        "canonical_sha256":"",
        "created_at":now_string()
    });
    let mut readback = frame_value.clone();
    readback["created_at"] = Value::String(String::new());
    readback["canonical_sha256"] = Value::String(String::new());
    readback["projection_frame_canonical_sha256"] = Value::String(String::new());
    frame_value["socket_transform_readback_sha256"] = Value::String(canonical_json_hash(&readback));
    let mut projection_frame = frame_value.clone();
    projection_frame["created_at"] = Value::String(String::new());
    projection_frame["canonical_sha256"] = Value::String(String::new());
    frame_value["projection_frame_canonical_sha256"] =
        Value::String(canonical_json_hash(&projection_frame));
    let mut canonical = frame_value.clone();
    canonical["canonical_sha256"] = Value::String(String::new());
    let canonical_sha256 = canonical_json_hash(&canonical);
    frame_value["canonical_sha256"] = Value::String(canonical_sha256);
    serde_json::from_value(frame_value)
        .map_err(|error| invalid(format!("V2 projection frame is malformed: {error}")))
}

fn build_projection_value(
    request: &ProjectionRequest,
    animation_link: &MechanicalAnimationGlbV2LinkRecord,
    socket_link: &GameWeaponAnimatedGlbSocketMaterializationV2LinkRecord,
    socket_receipt: &GameWeaponAnimatedGlbSocketMaterializationV2ReceiptRecord,
    frames: Vec<GameWeaponAnimatedGlbSocketTransformProjectionV2Frame>,
    part_hierarchy_sha256: &str,
) -> Result<GameWeaponAnimatedGlbSocketTransformProjectionV2, RuntimeError> {
    let mut projection = GameWeaponAnimatedGlbSocketTransformProjectionV2 {
        schema_version: PROJECTION_SCHEMA.to_owned(),
        projection_key_sha256: request.projection_key_sha256.clone(),
        project_id: request.project_id.clone(),
        appearance_candidate_id: request.appearance_candidate_id.clone(),
        appearance_candidate_state_sha256: request.appearance_candidate_state_sha256.clone(),
        appearance_delivery_manifest_object_sha256: request
            .appearance_delivery_manifest_object_sha256
            .clone(),
        appearance_artifact_sha256: request.appearance_artifact_sha256.clone(),
        appearance_artifact_readback_sha256: request.appearance_artifact_readback_sha256.clone(),
        animation_clip_id: request.animation_clip_id.clone(),
        animation_clip_object_sha256: request.animation_clip_object_sha256.clone(),
        animation_clip_canonical_sha256: request.animation_clip_canonical_sha256.clone(),
        animation_glb_key_sha256: request.animation_glb_key_sha256.clone(),
        animated_artifact_sha256: request.animated_artifact_sha256.clone(),
        animated_artifact_readback_sha256: request.animated_artifact_readback_sha256.clone(),
        animation_receipt_object_sha256: request.animation_receipt_object_sha256.clone(),
        animation_receipt_canonical_sha256: request.animation_receipt_canonical_sha256.clone(),
        animated_socket_materialization_key_sha256: request
            .animated_socket_materialization_key_sha256
            .clone(),
        derived_animated_socket_artifact_sha256: request
            .derived_animated_socket_artifact_sha256
            .clone(),
        derived_animated_socket_artifact_readback_sha256: request
            .derived_animated_socket_artifact_readback_sha256
            .clone(),
        derived_animated_socket_receipt_object_sha256: request
            .derived_animated_socket_receipt_object_sha256
            .clone(),
        derived_animated_socket_receipt_canonical_sha256: request
            .derived_animated_socket_receipt_canonical_sha256
            .clone(),
        anchor_set_object_sha256: request.anchor_set_object_sha256.clone(),
        anchor_set_canonical_sha256: request.anchor_set_canonical_sha256.clone(),
        socket_node_id_encoding_sha256: request.socket_node_id_encoding_sha256.clone(),
        socket_node_inventory_sha256: request.socket_node_inventory_sha256.clone(),
        socket_roles_sha256: request.socket_roles_sha256.clone(),
        socket_roles: SOCKET_ROLES.iter().map(|role| (*role).to_owned()).collect(),
        part_hierarchy_sha256: part_hierarchy_sha256.to_owned(),
        part_hierarchy_policy: request.part_hierarchy_policy.clone(),
        transform_representation_policy: request.transform_representation_policy.clone(),
        sampling_policy_sha256: request.sampling_policy_sha256.clone(),
        sample_schedule_sha256: request.sample_schedule_sha256.clone(),
        sample_count: request.sample_count,
        sample_time_ticks: request.sample_time_ticks.clone(),
        frame_scope: request.frame_scope.clone(),
        timebase_hz: request.timebase_hz,
        transform_projection_policy: request.transform_projection_policy.clone(),
        coordinate_system: request.coordinate_system.clone(),
        transform_convention: request.transform_convention.clone(),
        float_quantization_policy: request.float_quantization_policy.clone(),
        input_sha256: request.input_sha256.clone(),
        frames,
        projection_status: PROJECTION_STATUS.to_owned(),
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
        limitations: LIMITATIONS
            .iter()
            .map(|value| (*value).to_owned())
            .collect(),
        canonical_sha256: String::new(),
        created_at: socket_receipt.created_at.clone(),
    };
    if animation_link.animation_glb_key_sha256 != request.animation_glb_key_sha256
        || socket_link.animated_socket_materialization_key_sha256
            != request.animated_socket_materialization_key_sha256
    {
        return Err(invalid(
            "projection parent keys differ while building record",
        ));
    }
    let mut value = serde_json::to_value(&projection)
        .map_err(|error| invalid(format!("V2 projection serialization failed: {error}")))?;
    value["canonical_sha256"] = Value::String(String::new());
    projection.canonical_sha256 = canonical_json_hash(&value);
    Ok(projection)
}

fn replay_equivalent(
    left: &GameWeaponAnimatedGlbSocketTransformProjectionV2,
    right: &GameWeaponAnimatedGlbSocketTransformProjectionV2,
) -> bool {
    let mut left = left.clone();
    let mut right = right.clone();
    left.created_at.clear();
    right.created_at.clear();
    left.canonical_sha256.clear();
    right.canonical_sha256.clear();
    for frame in &mut left.frames {
        frame.created_at.clear();
        frame.canonical_sha256.clear();
    }
    for frame in &mut right.frames {
        frame.created_at.clear();
        frame.canonical_sha256.clear();
    }
    left == right
}

fn projection_bytes(
    projection: &GameWeaponAnimatedGlbSocketTransformProjectionV2,
) -> Result<Vec<u8>, RuntimeError> {
    let bytes = canonical_json_bytes(
        &serde_json::to_value(projection)
            .map_err(|error| invalid(format!("V2 projection serialization failed: {error}")))?,
    )
    .map_err(|error| invalid(format!("V2 projection canonical JSON failed: {error}")))?;
    if bytes.is_empty() || bytes.len() > MAX_JSON_BYTES {
        return Err(invalid("V2 projection exceeds its JSON budget"));
    }
    Ok(bytes)
}

fn result_value(
    schema: &str,
    projection: &GameWeaponAnimatedGlbSocketTransformProjectionV2,
    projection_object_sha256: &str,
    replayed: bool,
    runtime_write: bool,
) -> Result<Value, RuntimeError> {
    if !valid_hash(projection_object_sha256) {
        return Err(invalid("V2 projection object hash is invalid"));
    }
    Ok(json!({
        "schema_version":schema,
        "projection_key_sha256":projection.projection_key_sha256,
        "projection_object_sha256":projection_object_sha256,
        "projection":projection,
        "replayed":replayed,
        "restart_hash_verified":true,
        "runtime_write_performed":runtime_write,
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

fn build_projection(
    runtime: &Runtime,
    request: &ProjectionRequest,
) -> Result<
    (
        GameWeaponAnimatedGlbSocketTransformProjectionV2,
        Option<String>,
    ),
    RuntimeError,
> {
    let existing = runtime
        .store
        .get_game_weapon_animated_glb_socket_transform_projection_v2(
            &request.projection_key_sha256,
        )?;

    let candidate = runtime
        .candidate(&request.appearance_candidate_id)?
        .ok_or_else(|| invalid("appearance candidate is unavailable"))?;
    if candidate.project_id != request.project_id
        || candidate.canonical_sha256 != request.appearance_candidate_state_sha256
        || candidate.prepared_object_sha256.as_deref()
            != Some(request.appearance_artifact_sha256.as_str())
        || candidate.manifest_hash.as_deref() != Some(request.appearance_artifact_sha256.as_str())
    {
        return Err(invalid("appearance candidate binding differs"));
    }

    let clip_result = mechanical_animation_clip_v2::get(
        runtime,
        &json!({
            "schema_version":"MechanicalAnimationClipGetRequest@2",
            "project_id":request.project_id,
            "appearance_candidate_id":request.appearance_candidate_id,
            "clip_id":request.animation_clip_id
        }),
    )?;
    let clip: MechanicalAnimationClipV2Record = parse_link(&clip_result, "clip", "Clip@2")?;
    let clip_link: MechanicalAnimationClipV2LinkRecord =
        parse_link(&clip_result, "durable_link", "Clip@2")?;
    let _ticks = compare_clip(request, &clip, &clip_link)?;

    let animation_result = mechanical_animation_glb_v2::get(
        runtime,
        &json!({
            "schema_version":"MechanicalAnimationGlbGetRequest@2",
            "project_id":request.project_id,
            "appearance_candidate_id":request.appearance_candidate_id,
            "clip_id":request.animation_clip_id
        }),
    )?;
    let animation_link: MechanicalAnimationGlbV2LinkRecord = parse_link(
        &animation_result,
        "durable_link",
        "MechanicalAnimationGlb@2",
    )?;
    let animation_receipt: MechanicalAnimationGlbV2ReceiptRecord =
        parse_link(&animation_result, "receipt", "MechanicalAnimationGlb@2")?;
    compare_animation(request, &animation_link, &animation_receipt)?;

    let socket_result = game_weapon_animated_glb_socket_v2::get(
        runtime,
        &json!({
            "schema_version":"GameWeaponAnimatedGlbSocketMaterializationGetRequest@2",
            "project_id":request.project_id,
            "appearance_candidate_id":request.appearance_candidate_id,
            "clip_id":request.animation_clip_id,
            "animated_socket_materialization_key_sha256":request.animated_socket_materialization_key_sha256
        }),
    )?;
    let socket_link: GameWeaponAnimatedGlbSocketMaterializationV2LinkRecord =
        parse_link(&socket_result, "durable_link", "AnimatedSocket@2")?;
    let socket_receipt: GameWeaponAnimatedGlbSocketMaterializationV2ReceiptRecord =
        parse_link(&socket_result, "receipt", "AnimatedSocket@2")?;
    compare_socket(request, &socket_link, &socket_receipt)?;

    let animation_receipt_value = read_canonical_json(
        runtime,
        &request.animation_receipt_object_sha256,
        "MechanicalAnimationGlbReceipt@2",
        "MechanicalAnimationGlb@2 receipt",
    )?;
    if animation_receipt_value
        != serde_json::to_value(&animation_receipt)
            .map_err(|error| invalid(format!("MechanicalAnimationGlb@2 receipt: {error}")))?
    {
        return Err(invalid(
            "MechanicalAnimationGlb@2 receipt CAS replay differs",
        ));
    }
    let socket_receipt_value = read_canonical_json(
        runtime,
        &request.derived_animated_socket_receipt_object_sha256,
        "GameWeaponAnimatedGlbSocketMaterializationReceipt@2",
        "AnimatedSocket@2 receipt",
    )?;
    if socket_receipt_value
        != serde_json::to_value(&socket_receipt)
            .map_err(|error| invalid(format!("AnimatedSocket@2 receipt: {error}")))?
    {
        return Err(invalid("AnimatedSocket@2 receipt CAS replay differs"));
    }

    let delivery = game_asset_delivery::get(
        runtime,
        &json!({
            "schema_version":"GameAssetDeliveryGetRequest@1",
            "project_id":request.project_id,
            "delivery_manifest_object_sha256":request.appearance_delivery_manifest_object_sha256
        }),
    )?;
    let levels = delivery
        .get("lod_receipt")
        .and_then(|value| value.get("levels"))
        .and_then(Value::as_array)
        .filter(|levels| levels.len() == 3)
        .ok_or_else(|| invalid("appearance delivery LOD receipt is incomplete"))?;
    let lod0 = levels
        .first()
        .ok_or_else(|| invalid("appearance delivery LOD0 is unavailable"))?;
    if lod0.get("level").and_then(Value::as_u64) != Some(0)
        || lod0.get("candidate_id").and_then(Value::as_str)
            != Some(request.appearance_candidate_id.as_str())
        || lod0.get("candidate_state_sha256").and_then(Value::as_str)
            != Some(request.appearance_candidate_state_sha256.as_str())
        || lod0.get("artifact_sha256").and_then(Value::as_str)
            != Some(request.appearance_artifact_sha256.as_str())
        || lod0.get("artifact_readback_sha256").and_then(Value::as_str)
            != Some(request.appearance_artifact_readback_sha256.as_str())
    {
        return Err(invalid("appearance delivery LOD0 binding differs"));
    }

    let anchor_result = game_asset_delivery::weapon_anchor_get(
        runtime,
        &json!({
            "schema_version":"GameWeaponAnchorGetRequest@1",
            "project_id":request.project_id,
            "delivery_manifest_object_sha256":request.appearance_delivery_manifest_object_sha256
        }),
    )?;
    let anchor_link = anchor_result
        .get("link")
        .and_then(Value::as_object)
        .ok_or_else(|| invalid("AnchorSet durable link is unavailable"))?;
    if anchor_link
        .get("anchor_set_object_sha256")
        .and_then(Value::as_str)
        != Some(request.anchor_set_object_sha256.as_str())
    {
        return Err(invalid("AnchorSet object binding differs"));
    }
    let anchor_set = anchor_result
        .get("anchor_set")
        .cloned()
        .ok_or_else(|| invalid("AnchorSet is unavailable"))?;
    if anchor_set.get("canonical_sha256").and_then(Value::as_str)
        != Some(request.anchor_set_canonical_sha256.as_str())
    {
        return Err(invalid("AnchorSet canonical binding differs"));
    }
    let anchor_ids = game_asset_delivery::socket_anchor_ids(&anchor_set)?;
    if anchor_ids.len() != SOCKET_ROLES.len() {
        return Err(invalid("AnchorSet must contain six closed socket roles"));
    }
    let part_ids = socket_receipt.part_ids.clone();
    if part_ids.is_empty() || part_ids.len() > 64 {
        return Err(invalid(
            "AnimatedSocket@2 Part inventory is outside the bound",
        ));
    }

    ensure_cas_object(
        runtime,
        &request.appearance_artifact_sha256,
        "model/gltf-binary",
        &["appearance-glb", "appearance-v2-glb"],
        MAX_GLB_BYTES as u64,
        "appearance artifact",
    )?;
    ensure_cas_object(
        runtime,
        &request.animated_artifact_sha256,
        "model/gltf-binary",
        &["mechanical-animation-glb-v2"],
        MAX_GLB_BYTES as u64,
        "MechanicalAnimationGlb@2 artifact",
    )?;
    ensure_cas_object(
        runtime,
        &request.animation_receipt_object_sha256,
        "application/json",
        &["mechanical-animation-glb-v2-receipt"],
        MAX_JSON_BYTES as u64,
        "MechanicalAnimationGlb@2 receipt",
    )?;
    ensure_cas_object(
        runtime,
        &request.animation_clip_object_sha256,
        "application/json",
        &["mechanical-animation-clip-v2"],
        MAX_JSON_BYTES as u64,
        "Clip@2",
    )?;
    ensure_cas_object(
        runtime,
        &request.anchor_set_object_sha256,
        "application/json",
        &["game-weapon-anchor-set"],
        MAX_JSON_BYTES as u64,
        "AnchorSet",
    )?;
    ensure_cas_object(
        runtime,
        &request.derived_animated_socket_artifact_sha256,
        "model/gltf-binary",
        &["game-weapon-animated-glb-v2-socket-materialized-glb"],
        MAX_GLB_BYTES as u64,
        "AnimatedSocket@2 artifact",
    )?;
    ensure_cas_object(
        runtime,
        &request.derived_animated_socket_receipt_object_sha256,
        "application/json",
        &["game-weapon-animated-glb-v2-socket-materialization-receipt"],
        MAX_JSON_BYTES as u64,
        "AnimatedSocket@2 receipt",
    )?;

    let animated_glb =
        runtime.cas_read_bounded(&request.animated_artifact_sha256, MAX_GLB_BYTES as u64)?;
    let derived_glb = runtime.cas_read_bounded(
        &request.derived_animated_socket_artifact_sha256,
        MAX_GLB_BYTES as u64,
    )?;
    if sha256_hex(&animated_glb) != request.animated_artifact_sha256
        || sha256_hex(&derived_glb) != request.derived_animated_socket_artifact_sha256
    {
        return Err(invalid("V2 animation/socket GLB bytes differ"));
    }

    let materialized = game_weapon_animated_glb_socket_v2::materialize_socket_glb_v2(
        &animated_glb,
        &request.animated_artifact_sha256,
        &request.anchor_set_object_sha256,
        &anchor_set,
        &part_ids,
        &anchor_ids,
        Some((
            &animation_receipt.material_pack_id,
            &animation_receipt.material_pack_manifest_sha256,
        )),
    )?;
    if materialized.glb != derived_glb
        || materialized.source_node_count + SOCKET_ROLES.len() != materialized.derived_node_count
        || materialized.socket_node_inventory_sha256 != request.socket_node_inventory_sha256
        || materialized.source_bin_sha256 != materialized.derived_bin_sha256
    {
        return Err(invalid("AnimatedSocket@2 derived GLB replay differs"));
    }

    let (animated_root, animated_binary) = rigid_animation_glb::parse_glb(&animated_glb)?;
    let (derived_root, derived_binary) = rigid_animation_glb::parse_glb(&derived_glb)?;
    let node_part_ids =
        rigid_animation_glb::projection_part_ids_in_node_order(&animated_root, &part_ids)?;
    rigid_animation_glb::validate_projection_flat_source_root(&animated_root, &node_part_ids)?;
    let socket_nodes = rigid_animation_glb::validate_projection_socket_nodes(
        &derived_root,
        &node_part_ids,
        &anchor_set,
        &anchor_ids,
        animated_root
            .get("nodes")
            .and_then(Value::as_array)
            .ok_or_else(|| invalid("MechanicalAnimationGlb@2 nodes are unavailable"))?
            .len(),
    )?;
    if animated_binary != derived_binary {
        return Err(invalid("AnimatedSocket@2 BIN bytes are not exact"));
    }
    let animation = rigid_animation_glb::parse_projection_animation(
        &animated_root,
        &derived_root,
        &animated_binary,
        &request.sample_time_ticks,
        &node_part_ids,
    )?;
    if socket_receipt.source_animation_projection_sha256
        != animation.source_animation_projection_sha256
        || socket_receipt.derived_animation_projection_sha256
            != animation.derived_animation_projection_sha256
    {
        return Err(invalid(
            "AnimatedSocket@2 animation projection hash differs",
        ));
    }
    let hierarchy_hash = part_hierarchy_hash(&node_part_ids);
    if hierarchy_hash != request.part_hierarchy_sha256 {
        return Err(invalid("V2 Part hierarchy hash differs"));
    }

    let frames = request
        .sample_time_ticks
        .iter()
        .enumerate()
        .map(|(index, tick)| {
            build_frame(
                &request.projection_key_sha256,
                index as u64,
                *tick,
                &animation,
                &socket_nodes,
                &node_part_ids,
                &request.animated_artifact_sha256,
                &request.derived_animated_socket_artifact_sha256,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let projection = build_projection_value(
        request,
        &animation_link,
        &socket_link,
        &socket_receipt,
        frames,
        &hierarchy_hash,
    )?;

    if let Some(existing) = existing {
        if !replay_equivalent(&existing, &projection) {
            return Err(RuntimeError::InvalidInput(
                "GAME_WEAPON_ANIMATED_GLB_SOCKET_TRANSFORM_PROJECTION_V2_CONFLICT".to_owned(),
            ));
        }
        let expected_bytes = projection_bytes(&existing)?;
        let object_sha256 = runtime
            .store
            .get_game_weapon_animated_glb_socket_transform_projection_v2_object_sha256(
                &request.projection_key_sha256,
            )?
            .ok_or_else(|| invalid("V2 projection CAS object is unavailable"))?;
        let stored_bytes = runtime.cas_read_bounded(&object_sha256, MAX_JSON_BYTES as u64)?;
        if sha256_hex(&stored_bytes) != object_sha256 || stored_bytes != expected_bytes {
            return Err(invalid("V2 projection CAS bytes differ after replay"));
        }
        return Ok((existing, Some(object_sha256)));
    }
    Ok((projection, None))
}

pub(super) fn prepare(runtime: &Runtime, value: &Value) -> Result<Value, RuntimeError> {
    let request = parse_prepare(value)?;
    let (projection, existing_object_sha256) = build_projection(runtime, &request)?;
    if let Some(object_sha256) = existing_object_sha256 {
        return result_value(
            PREPARE_RESULT_SCHEMA,
            &projection,
            &object_sha256,
            true,
            true,
        );
    }
    let projection_bytes = projection_bytes(&projection)?;
    let reservation = runtime.store.begin_cas_reservation();
    let projection_object = runtime.store.put_object_reserved(
        &reservation,
        &projection_bytes,
        None,
        "application/json",
        PROJECTION_KIND,
        &projection.created_at,
    )?;
    match runtime
        .store
        .record_game_weapon_animated_glb_socket_transform_projection_v2(
            &projection,
            &projection_object.record,
        ) {
        Ok(stored) => {
            runtime.store.release_cas_reservation_object(
                &reservation,
                &projection_object,
                false,
            )?;
            result_value(
                PREPARE_RESULT_SCHEMA,
                &stored,
                &projection_object.record.sha256,
                false,
                true,
            )
        }
        Err(error) => {
            let rollback = runtime.store.release_cas_reservation_object(
                &reservation,
                &projection_object,
                true,
            );
            if let Err(rollback_error) = rollback {
                return Err(invalid(format!(
                    "V2 projection Store commit failed ({error}); reservation rollback failed ({rollback_error})"
                )));
            }
            Err(error.into())
        }
    }
}

pub(super) fn get(runtime: &Runtime, value: &Value) -> Result<Value, RuntimeError> {
    let (project_id, appearance_candidate_id, animation_clip_id, key) = parse_get(value)?;
    let projection = runtime
        .store
        .get_game_weapon_animated_glb_socket_transform_projection_v2(&key)?
        .ok_or_else(|| invalid("V2 transform projection is unavailable"))?;
    if projection.project_id != project_id
        || projection.appearance_candidate_id != appearance_candidate_id
        || projection.animation_clip_id != animation_clip_id
        || projection.projection_key_sha256 != key
    {
        return Err(invalid("V2 transform projection scope differs"));
    }
    let request = request_from_projection(&projection)?;
    let (replayed, object_sha256) = build_projection(runtime, &request)?;
    let object_sha256 = object_sha256
        .ok_or_else(|| invalid("V2 transform projection CAS object is unavailable after replay"))?;
    if !replay_equivalent(&projection, &replayed) {
        return Err(invalid("V2 transform projection receipt is tampered"));
    }
    let bytes = projection_bytes(&projection)?;
    if sha256_hex(&bytes) != object_sha256 {
        return Err(invalid(
            "V2 transform projection CAS hash differs after replay",
        ));
    }
    result_value(GET_RESULT_SCHEMA, &replayed, &object_sha256, true, false)
}

fn request_from_projection(
    projection: &GameWeaponAnimatedGlbSocketTransformProjectionV2,
) -> Result<ProjectionRequest, RuntimeError> {
    let value = serde_json::to_value(projection).map_err(|error| {
        invalid(format!(
            "V2 projection request reconstruction failed: {error}"
        ))
    })?;
    let mut object = value
        .as_object()
        .cloned()
        .ok_or_else(|| invalid("V2 projection is not an object"))?;
    object.insert(
        "schema_version".to_owned(),
        Value::String(PREPARE_SCHEMA.to_owned()),
    );
    object.insert(
        "idempotency_key".to_owned(),
        Value::String(projection.projection_key_sha256.clone()),
    );
    object.remove("frames");
    object.remove("projection_status");
    object.remove("quality_status");
    object.remove("visual_quality_status");
    object.remove("commercial_fps_quality_status");
    object.remove("human_review_status");
    object.remove("commercial_engine_status");
    object.remove("runtime_write_performed");
    object.remove("restart_hash_verified");
    object.remove("candidate_confirmed");
    object.remove("version_created");
    object.remove("export_performed");
    object.remove("actual_engine_roundtrip");
    object.remove("production_stage_advanced");
    object.remove("limitations");
    object.remove("canonical_sha256");
    object.remove("created_at");
    let mut request = object;
    request.insert("input_sha256".to_owned(), Value::String(String::new()));
    request["input_sha256"] = Value::String(canonical_json_hash(&{
        let mut preimage = Value::Object(request.clone());
        preimage
            .as_object_mut()
            .expect("V2 projection request object")
            .remove("projection_key_sha256");
        preimage
            .as_object_mut()
            .expect("V2 projection request object")
            .remove("input_sha256");
        preimage
            .as_object_mut()
            .expect("V2 projection request object")
            .remove("idempotency_key");
        preimage
    }));
    serde_json::from_value(Value::Object(request)).map_err(|error| {
        invalid(format!(
            "V2 projection request reconstruction failed: {error}"
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(seed: char) -> String {
        seed.to_string().repeat(64)
    }

    fn valid_request() -> Value {
        let roles = json!(SOCKET_ROLES);
        let ticks = vec![0_u64, 500, 1000];
        let frame_scope = FRAME_SCOPE;
        let schedule_hash = canonical_json_hash(&json!({
            "frame_scope":frame_scope,
            "sample_time_ticks":ticks,
            "timebase_hz":1000
        }));
        let mut request = json!({
            "schema_version":PREPARE_SCHEMA,
            "projection_key_sha256":"",
            "project_id":"project-test",
            "appearance_candidate_id":"appearance-candidate",
            "appearance_candidate_state_sha256":hash('a'),
            "appearance_delivery_manifest_object_sha256":hash('b'),
            "appearance_artifact_sha256":hash('c'),
            "appearance_artifact_readback_sha256":hash('d'),
            "animation_clip_id":"clip-test",
            "animation_clip_object_sha256":hash('e'),
            "animation_clip_canonical_sha256":hash('f'),
            "animation_glb_key_sha256":hash('0'),
            "animated_artifact_sha256":hash('1'),
            "animated_artifact_readback_sha256":hash('2'),
            "animation_receipt_object_sha256":hash('3'),
            "animation_receipt_canonical_sha256":hash('4'),
            "animated_socket_materialization_key_sha256":hash('5'),
            "derived_animated_socket_artifact_sha256":hash('6'),
            "derived_animated_socket_artifact_readback_sha256":hash('7'),
            "derived_animated_socket_receipt_object_sha256":hash('8'),
            "derived_animated_socket_receipt_canonical_sha256":hash('9'),
            "anchor_set_object_sha256":hash('a'),
            "anchor_set_canonical_sha256":hash('b'),
            "socket_node_id_encoding_sha256":hash('c'),
            "socket_node_inventory_sha256":hash('d'),
            "socket_roles_sha256":canonical_json_hash(&roles),
            "socket_roles":roles,
            "part_hierarchy_sha256":hash('e'),
            "part_hierarchy_policy":PART_HIERARCHY_POLICY,
            "transform_representation_policy":TRANSFORM_REPRESENTATION_POLICY,
            "sampling_policy_sha256":hash('f'),
            "sample_schedule_sha256":schedule_hash,
            "sample_count":3,
            "sample_time_ticks":ticks,
            "frame_scope":frame_scope,
            "timebase_hz":1000,
            "transform_projection_policy":TRANSFORM_PROJECTION_POLICY,
            "coordinate_system":COORDINATE_SYSTEM,
            "transform_convention":TRANSFORM_CONVENTION,
            "float_quantization_policy":FLOAT_POLICY,
            "input_sha256":"",
            "idempotency_key":"projection-test-idempotency"
        });
        let mut preimage = request.clone();
        let object = preimage.as_object_mut().expect("projection request object");
        object.remove("projection_key_sha256");
        object.remove("input_sha256");
        object.remove("idempotency_key");
        let input = canonical_json_hash(&preimage);
        request["projection_key_sha256"] = Value::String(input.clone());
        request["input_sha256"] = Value::String(input);
        request
    }

    #[test]
    fn v2_parser_accepts_closed_request_and_rejects_extra_fields() {
        let request = valid_request();
        assert!(parse_prepare(&request).is_ok());
        let mut extra = request;
        extra["raw_glb_bytes"] = json!("forbidden");
        assert!(parse_prepare(&extra).is_err());
    }

    #[test]
    fn v2_parser_rejects_unsorted_schedule_and_wrong_schedule_hash() {
        let mut unsorted = valid_request();
        unsorted["sample_time_ticks"] = json!([500_u64, 0, 1000]);
        assert!(parse_prepare(&unsorted).is_err());

        let mut wrong_hash = valid_request();
        wrong_hash["sample_schedule_sha256"] = Value::String(hash('0'));
        assert!(parse_prepare(&wrong_hash).is_err());
    }

    #[test]
    fn v2_get_parser_is_closed_and_scope_bound() {
        let request = valid_request();
        let get = json!({
            "schema_version":GET_SCHEMA,
            "projection_key_sha256":request["projection_key_sha256"],
            "project_id":request["project_id"],
            "appearance_candidate_id":request["appearance_candidate_id"],
            "animation_clip_id":request["animation_clip_id"]
        });
        let parsed = parse_get(&get).expect("valid V2 get");
        assert_eq!(parsed.0, "project-test");
        assert_eq!(parsed.1, "appearance-candidate");
        assert_eq!(parsed.2, "clip-test");
        let mut extra = get;
        extra["candidate_id"] = json!("wrong-field");
        assert!(parse_get(&extra).is_err());
    }

    #[test]
    fn v2_projection_compose_preserves_non_unit_translation_and_rotation_in_column_major_matrix() {
        let half_sqrt_two = 2.0_f32.sqrt() / 2.0;
        let parent = rigid_animation_glb::ProjectionPose {
            translation: [1.2, -0.7, 0.3],
            rotation: [0.0, 0.0, half_sqrt_two, half_sqrt_two],
        };
        let local = rigid_animation_glb::ProjectionPose {
            translation: [0.2, 0.3, 0.4],
            rotation: [0.38268343, 0.0, 0.0, 0.9238795],
        };
        let composed = rigid_animation_glb::projection_compose(parent, local);

        // A 90-degree parent rotation must preserve the non-unit local
        // translation magnitude, then apply the parent world translation.
        for (actual, expected) in composed
            .translation
            .iter()
            .zip([0.9_f32, -0.5_f32, 0.7_f32])
        {
            assert!((actual - expected).abs() < 1.0e-5, "{actual} != {expected}");
        }
        let composed_rotation_norm = composed
            .rotation
            .iter()
            .map(|component| component * component)
            .sum::<f32>();
        assert!((composed_rotation_norm - 1.0).abs() < 1.0e-5);

        // The emitted arrays are column-major.  The product of the parent
        // and local matrices must equal the matrix reconstructed from the
        // composed TRS, including its translated fourth column.
        let parent_matrix = matrix_from_pose(parent);
        let local_matrix = matrix_from_pose(local);
        let composed_matrix = matrix_from_pose(composed);
        let mut product = vec![0.0_f64; 16];
        for column in 0..4 {
            for row in 0..4 {
                product[column * 4 + row] = (0..4)
                    .map(|index| parent_matrix[index * 4 + row] * local_matrix[column * 4 + index])
                    .sum();
            }
        }
        for (actual, expected) in composed_matrix.iter().zip(product) {
            assert!((actual - expected).abs() < 1.0e-5, "{actual} != {expected}");
        }
        assert_eq!(pose_value(composed)["scale_xyz"], json!([1.0, 1.0, 1.0]));
    }
}
