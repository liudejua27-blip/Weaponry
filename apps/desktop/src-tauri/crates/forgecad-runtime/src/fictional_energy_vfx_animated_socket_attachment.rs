//! Runtime-owned structural attachment of the animated GLB socket to the
//! existing fictional-energy VFX stack.
//!
//! This slice is intentionally a composition/readback gate.  It does not
//! call the mechanical-animation GLB producer, alter a GLB, render a PNG, or
//! advance a production stage.  All source dependencies are read before the
//! single receipt reservation.  The current VFX receipts explicitly declare
//! `no-glb-socket-transform-execution`; those receipts therefore fail closed
//! here until a producer supplies an actual per-frame socket transform.

use super::{
    canonical_json_bytes, canonical_json_hash, exact_object, is_opaque_id, is_sha256, now_string,
    sha256_hex, CasObject, Runtime, RuntimeError,
};
use forgecad_contracts::{
    FictionalEnergyVfxAnimatedSocketAttachmentFrameRecord,
    FictionalEnergyVfxAnimatedSocketAttachmentGetRequest,
    FictionalEnergyVfxAnimatedSocketAttachmentPrepareRequest,
    FictionalEnergyVfxAnimatedSocketAttachmentRecord,
};
use serde_json::{json, Map, Value};
use std::collections::BTreeSet;

const PREPARE_SCHEMA: &str = "FictionalEnergyVfxAnimatedSocketAttachmentPrepareRequest@1";
const GET_SCHEMA: &str = "FictionalEnergyVfxAnimatedSocketAttachmentGetRequest@1";
const PREPARE_RESULT_SCHEMA: &str = "FictionalEnergyVfxAnimatedSocketAttachmentPrepareResult@1";
const GET_RESULT_SCHEMA: &str = "FictionalEnergyVfxAnimatedSocketAttachmentGetResult@1";
const RECORD_SCHEMA: &str = "FictionalEnergyVfxAnimatedSocketAttachment@1";
const RECEIPT_SCHEMA: &str = "FictionalEnergyVfxAnimatedSocketAttachmentReceipt@1";
const ATTACHMENT_POLICY: &str = "fictional-energy-vfx-animated-socket-attachment-structural-only@1";
const FRAME_SCOPE: &str = "lod0-animation-vfx-frame-range-1-16@1";
const ATTACHMENT_STATUS: &str =
    "runtime-owned-durable-fictional-energy-vfx-animated-socket-attachment";
const RECEIPT_KIND: &str = "fictional-energy-vfx-animated-socket-attachment-receipt";
const RECEIPT_MIME: &str = "application/json";
const MAX_RECEIPT_BYTES: usize = 1024 * 1024;
const MAX_FRAMES: usize = 16;

const PREPARE_FIELDS: &[&str] = &[
    "schema_version",
    "attachment_key_sha256",
    "project_id",
    "delivery_manifest_object_sha256",
    "candidate_id",
    "candidate_state_sha256",
    "source_artifact_sha256",
    "animated_socket_materialization_key_sha256",
    "animated_socket_anchor_set_object_sha256",
    "animated_socket_anchor_set_canonical_sha256",
    "animation_clip_id",
    "animation_clip_object_sha256",
    "animation_clip_canonical_sha256",
    "animated_artifact_sha256",
    "animation_receipt_object_sha256",
    "animation_receipt_canonical_sha256",
    "vfx_profile_object_sha256",
    "vfx_profile_canonical_sha256",
    "vfx_sequence_key_sha256",
    "vfx_sequence_canonical_sha256",
    "attachment_policy",
    "socket_node_id_encoding_sha256",
    "socket_roles_sha256",
    "frame_scope",
    "input_sha256",
    "idempotency_key",
];

const GET_FIELDS: &[&str] = &[
    "schema_version",
    "attachment_key_sha256",
    "project_id",
    "candidate_id",
];

fn invalid(detail: impl Into<String>) -> RuntimeError {
    RuntimeError::InvalidInput(format!(
        "FICTIONAL_ENERGY_VFX_ANIMATED_SOCKET_ATTACHMENT_INVALID: {}",
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
) -> Result<
    (
        FictionalEnergyVfxAnimatedSocketAttachmentPrepareRequest,
        String,
    ),
    RuntimeError,
> {
    let object = exact_object(value, PREPARE_FIELDS, PREPARE_SCHEMA)?;
    if text(object, "schema_version")? != PREPARE_SCHEMA {
        return Err(invalid("prepare schema version differs"));
    }
    let request: FictionalEnergyVfxAnimatedSocketAttachmentPrepareRequest =
        serde_json::from_value(value.clone())
            .map_err(|error| invalid(format!("prepare request is malformed: {error}")))?;
    for field in [
        "attachment_key_sha256",
        "delivery_manifest_object_sha256",
        "candidate_state_sha256",
        "source_artifact_sha256",
        "animated_socket_materialization_key_sha256",
        "animated_socket_anchor_set_object_sha256",
        "animated_socket_anchor_set_canonical_sha256",
        "animation_clip_object_sha256",
        "animation_clip_canonical_sha256",
        "animated_artifact_sha256",
        "animation_receipt_object_sha256",
        "animation_receipt_canonical_sha256",
        "vfx_profile_object_sha256",
        "vfx_profile_canonical_sha256",
        "vfx_sequence_key_sha256",
        "vfx_sequence_canonical_sha256",
        "socket_node_id_encoding_sha256",
        "socket_roles_sha256",
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
    if request.attachment_policy != ATTACHMENT_POLICY {
        return Err(invalid("attachment policy differs"));
    }
    if request.frame_scope != FRAME_SCOPE {
        return Err(invalid("frame scope differs"));
    }
    let mut preimage = object.clone();
    preimage.remove("attachment_key_sha256");
    preimage.remove("input_sha256");
    preimage.remove("idempotency_key");
    let input_sha256 = canonical_json_hash(&Value::Object(preimage));
    if request.input_sha256 != input_sha256 || request.attachment_key_sha256 != input_sha256 {
        return Err(invalid("input or attachment key hash differs"));
    }
    Ok((request, input_sha256))
}

fn parse_get(
    value: &Value,
) -> Result<FictionalEnergyVfxAnimatedSocketAttachmentGetRequest, RuntimeError> {
    let object = exact_object(value, GET_FIELDS, GET_SCHEMA)?;
    if text(object, "schema_version")? != GET_SCHEMA {
        return Err(invalid("get schema version differs"));
    }
    let request: FictionalEnergyVfxAnimatedSocketAttachmentGetRequest =
        serde_json::from_value(value.clone())
            .map_err(|error| invalid(format!("get request is malformed: {error}")))?;
    sha(object, "attachment_key_sha256")?;
    id(object, "project_id")?;
    id(object, "candidate_id")?;
    Ok(request)
}

fn require_link<'a>(value: &'a Value, label: &str) -> Result<&'a Map<String, Value>, RuntimeError> {
    value
        .get("link")
        .and_then(Value::as_object)
        .ok_or_else(|| invalid(format!("{label} link is unavailable")))
}

fn require_receipt<'a>(value: &'a Value, label: &str) -> Result<&'a Value, RuntimeError> {
    value
        .get("receipt")
        .ok_or_else(|| invalid(format!("{label} receipt is unavailable")))
}

fn field_str<'a>(
    object: &'a Map<String, Value>,
    field: &str,
    label: &str,
) -> Result<&'a str, RuntimeError> {
    object
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid(format!("{label}.{field} is unavailable")))
}

fn same_field(
    object: &Map<String, Value>,
    field: &str,
    expected: &str,
    label: &str,
) -> Result<(), RuntimeError> {
    if field_str(object, field, label)? != expected {
        return Err(invalid(format!("{label}.{field} binding differs")));
    }
    Ok(())
}

fn same_link_field(
    left: &Map<String, Value>,
    right: &Map<String, Value>,
    field: &str,
    label: &str,
) -> Result<(), RuntimeError> {
    if left.get(field).and_then(Value::as_str) != right.get(field).and_then(Value::as_str) {
        return Err(invalid(format!(
            "{label}.{field} binding differs across VFX layers"
        )));
    }
    Ok(())
}

fn socket_role_digest(socket_nodes: &Value) -> Result<String, RuntimeError> {
    let nodes = socket_nodes
        .as_array()
        .filter(|nodes| nodes.len() == 6)
        .ok_or_else(|| invalid("animated socket receipt must contain six socket nodes"))?;
    let mut roles = Vec::with_capacity(nodes.len());
    for node in nodes {
        let object = node
            .as_object()
            .ok_or_else(|| invalid("socket node is not an object"))?;
        roles.push(json!({
            "socket_node_id": field_str(object, "socket_node_id", "socket node")?,
            "anchor_id": field_str(object, "anchor_id", "socket node")?,
            "role": field_str(object, "role", "socket node")?,
            "parent_kind": field_str(object, "parent_kind", "socket node")?,
            "parent_node_name": object.get("parent_node_name").cloned().unwrap_or(Value::Null),
            "owner_part_id": object.get("owner_part_id").cloned().unwrap_or(Value::Null),
        }));
    }
    Ok(canonical_json_hash(&Value::Array(roles)))
}

fn validate_socket_nodes(
    socket_receipt: &Value,
    request: &FictionalEnergyVfxAnimatedSocketAttachmentPrepareRequest,
) -> Result<Value, RuntimeError> {
    let nodes = socket_receipt
        .get("socket_nodes")
        .filter(|value| value.is_array())
        .ok_or_else(|| invalid("animated socket receipt omits socket_nodes"))?;
    let nodes_array = nodes
        .as_array()
        .filter(|values| values.len() == 6)
        .ok_or_else(|| invalid("animated socket receipt must contain exactly six nodes"))?;
    let expected_roles = [
        "weapon-root",
        "grip-primary",
        "muzzle-vfx",
        "magazine-well",
        "sight-primary",
        "energy-core-vfx",
    ];
    let expected_anchor_ids = [
        "weapon-root",
        "grip-primary",
        "socket-muzzle-vfx",
        "socket-magazine-well",
        "socket-sight-primary",
        "socket-energy-core-vfx",
    ];
    let mut seen = BTreeSet::new();
    for (index, node) in nodes_array.iter().enumerate() {
        let object = node
            .as_object()
            .ok_or_else(|| invalid("animated socket node is not an object"))?;
        let role = field_str(object, "role", "socket node")?;
        let anchor_id = field_str(object, "anchor_id", "socket node")?;
        if role != expected_roles[index] || anchor_id != expected_anchor_ids[index] {
            return Err(invalid("animated socket role or anchor mapping differs"));
        }
        if !seen.insert(anchor_id.to_owned()) {
            return Err(invalid("animated socket nodes duplicate an anchor"));
        }
        if field_str(object, "node_kind", "socket node")? != "empty" {
            return Err(invalid("animated socket node is renderable"));
        }
        let parent_kind = field_str(object, "parent_kind", "socket node")?;
        if !matches!(parent_kind, "synthetic-scene-root" | "part-node") {
            return Err(invalid("animated socket parent kind differs"));
        }
        if parent_kind == "part-node"
            && object
                .get("owner_part_id")
                .and_then(Value::as_str)
                .filter(|value| is_opaque_id(value))
                .is_none()
        {
            return Err(invalid("animated socket Part owner is unavailable"));
        }
        for (field, length) in [
            ("local_translation_m", 3usize),
            ("local_rotation_quat_xyzw", 4usize),
            ("local_scale_xyz", 3usize),
        ] {
            let values = object
                .get(field)
                .and_then(Value::as_array)
                .filter(|values| values.len() == length)
                .ok_or_else(|| invalid(format!("animated socket {field} is invalid")))?;
            if values.iter().any(|value| {
                value
                    .as_f64()
                    .is_none_or(|number| !number.is_finite() || number.abs() > 10.0)
            }) {
                return Err(invalid(format!("animated socket {field} is non-finite")));
            }
        }
        if object.get("local_scale_xyz") != Some(&json!([1.0, 1.0, 1.0])) {
            return Err(invalid("animated socket scale is not unit"));
        }

        // The source animated-socket receipt currently exposes only static
        // local TRS.  It does not expose a composed/world transform for the
        // owning Part at this frame.  Do not turn that sidecar projection
        // into a false attachment PASS: a producer must provide a verified
        // composed transform readback before this gate can write anything.
        let composed = object
            .get("composed_world_transform")
            .or_else(|| object.get("world_transform"))
            .ok_or_else(|| {
                invalid(
                    "animated socket receipt does not expose composed owner transform; ".to_owned()
                        + "per-frame attachment is unsupported",
                )
            })?;
        let composed_values = composed
            .as_array()
            .filter(|values| values.len() == 16)
            .ok_or_else(|| invalid("animated socket composed owner transform is invalid"))?;
        if composed_values.iter().any(|value| {
            value
                .as_f64()
                .is_none_or(|number| !number.is_finite() || number.abs() > 1.0e6)
        }) {
            return Err(invalid(
                "animated socket composed owner transform is non-finite",
            ));
        }
    }
    let digest = socket_role_digest(nodes)?;
    if digest != request.socket_roles_sha256 {
        return Err(invalid("animated socket role digest differs"));
    }
    Ok(nodes.clone())
}

fn validate_socket_nodes_against_anchor_set(
    socket_nodes: &Value,
    anchor_set: &Value,
) -> Result<(), RuntimeError> {
    let anchors = anchor_set
        .get("anchors")
        .and_then(Value::as_array)
        .filter(|values| values.len() == 6)
        .ok_or_else(|| invalid("AnchorSet does not expose exactly six anchors"))?;
    let nodes = socket_nodes
        .as_array()
        .filter(|values| values.len() == 6)
        .ok_or_else(|| invalid("animated socket node projection is incomplete"))?;
    for (index, (node, anchor)) in nodes.iter().zip(anchors).enumerate() {
        let node = node
            .as_object()
            .ok_or_else(|| invalid("animated socket node is not an object"))?;
        let anchor = anchor
            .as_object()
            .ok_or_else(|| invalid("AnchorSet anchor is not an object"))?;
        for field in [
            "anchor_id",
            "role",
            "parent_kind",
            "owner_part_id",
            "local_translation_m",
            "local_rotation_quat_xyzw",
            "local_scale_xyz",
        ] {
            if node.get(field) != anchor.get(field) {
                return Err(invalid(format!(
                    "animated socket node {index} differs from AnchorSet.{field}"
                )));
            }
        }
    }
    Ok(())
}

fn ensure_common_layer_binding(
    result: &Value,
    label: &str,
    request: &FictionalEnergyVfxAnimatedSocketAttachmentPrepareRequest,
) -> Result<(), RuntimeError> {
    let link = require_link(result, label)?;
    for (field, expected) in [
        ("project_id", request.project_id.as_str()),
        (
            "delivery_manifest_object_sha256",
            request.delivery_manifest_object_sha256.as_str(),
        ),
        (
            "vfx_profile_object_sha256",
            request.vfx_profile_object_sha256.as_str(),
        ),
        ("source_candidate_id", request.candidate_id.as_str()),
        (
            "source_artifact_sha256",
            request.source_artifact_sha256.as_str(),
        ),
    ] {
        same_field(link, field, expected, label)?;
    }
    for field in [
        "camera_object_sha256",
        "camera_identity_sha256",
        "render_profile_sha256",
        "render_worker_build_cohort_sha256",
    ] {
        let value = field_str(link, field, label)?;
        if !is_sha256(value) {
            return Err(invalid(format!("{label}.{field} is not a SHA-256")));
        }
    }
    // Individual frame/effect links call this field `sample_request_sha256`;
    // the immutable sequence parent uses its canonical `request_sha256`.
    let request_hash = link
        .get("sample_request_sha256")
        .or_else(|| link.get("request_sha256"))
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .ok_or_else(|| invalid(format!("{label}.request hash is unavailable")))?;
    let _ = request_hash;
    Ok(())
}

fn has_limitation(value: &Value, limitation: &str) -> bool {
    value
        .get("limitations")
        .and_then(Value::as_array)
        .is_some_and(|values| values.iter().any(|item| item.as_str() == Some(limitation)))
}

fn validate_socket_execution(result: &Value, label: &str) -> Result<(), RuntimeError> {
    let receipt = require_receipt(result, label)?;
    if has_limitation(receipt, "no-glb-socket-transform-execution")
        || receipt
            .get("anchor_is_runtime_sidecar_not_glb_socket")
            .and_then(Value::as_bool)
            == Some(true)
        || receipt
            .get("glb_socket_transform_executed")
            .and_then(Value::as_bool)
            != Some(true)
    {
        return Err(invalid(format!(
            "{label} is sidecar-only and does not prove GLB socket transform execution"
        )));
    }
    Ok(())
}

fn validate_frame_order(
    sequence_receipt: &Value,
) -> Result<Vec<(usize, String, u64)>, RuntimeError> {
    let frames = sequence_receipt
        .get("frames")
        .and_then(Value::as_array)
        .filter(|frames| !frames.is_empty() && frames.len() <= MAX_FRAMES)
        .ok_or_else(|| invalid("VFX sequence frame list is outside the bound"))?;
    let mut result = Vec::with_capacity(frames.len());
    let mut last_tick = None;
    for (ordinal, frame) in frames.iter().enumerate() {
        let object = frame
            .as_object()
            .ok_or_else(|| invalid("VFX sequence frame entry is not an object"))?;
        if object.get("ordinal").and_then(Value::as_u64) != Some(ordinal as u64) {
            return Err(invalid("VFX sequence frame ordinals are not contiguous"));
        }
        let key = object
            .get("frame_key_sha256")
            .and_then(Value::as_str)
            .filter(|value| is_sha256(value))
            .ok_or_else(|| invalid("VFX sequence frame key is invalid"))?;
        let tick = object
            .get("sample_time_ticks")
            .and_then(Value::as_u64)
            .ok_or_else(|| invalid("VFX sequence frame sample time is invalid"))?;
        if last_tick.is_some_and(|previous| previous >= tick) {
            return Err(invalid(
                "VFX sequence sample times are not strictly increasing",
            ));
        }
        last_tick = Some(tick);
        result.push((ordinal, key.to_owned(), tick));
    }
    Ok(result)
}

struct Dependencies {
    socket: Value,
    frames: Vec<Value>,
}

fn validate_dependencies(
    runtime: &Runtime,
    request: &FictionalEnergyVfxAnimatedSocketAttachmentPrepareRequest,
) -> Result<Dependencies, RuntimeError> {
    let delivery = runtime.game_asset_delivery_get(&json!({
        "schema_version":"GameAssetDeliveryGetRequest@1",
        "project_id":request.project_id,
        "delivery_manifest_object_sha256":request.delivery_manifest_object_sha256
    }))?;
    let delivery_link = require_link(&delivery, "delivery")?;
    same_field(delivery_link, "project_id", &request.project_id, "delivery")?;
    let levels = delivery
        .get("lod_receipt")
        .and_then(|value| value.get("levels"))
        .and_then(Value::as_array)
        .filter(|levels| levels.len() == 3)
        .ok_or_else(|| invalid("delivery LOD set is incomplete"))?;
    let lod0 = levels
        .first()
        .ok_or_else(|| invalid("delivery LOD0 is missing"))?;
    for (field, expected) in [
        ("candidate_id", request.candidate_id.as_str()),
        (
            "candidate_state_sha256",
            request.candidate_state_sha256.as_str(),
        ),
        ("artifact_sha256", request.source_artifact_sha256.as_str()),
    ] {
        same_field(
            lod0.as_object()
                .ok_or_else(|| invalid("delivery LOD0 is not an object"))?,
            field,
            expected,
            "delivery LOD0",
        )?;
    }

    let socket = runtime.game_weapon_animated_glb_socket_get(&json!({
        "schema_version":"GameWeaponAnimatedGlbSocketMaterializationGetRequest@1",
        "project_id":request.project_id,
        "animated_socket_materialization_key_sha256":request.animated_socket_materialization_key_sha256
    }))?;
    let socket_link = require_link(&socket, "animated socket")?;
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
            "lod0_artifact_sha256",
            request.source_artifact_sha256.as_str(),
        ),
        (
            "animated_artifact_sha256",
            request.animated_artifact_sha256.as_str(),
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
            "anchor_set_object_sha256",
            request.animated_socket_anchor_set_object_sha256.as_str(),
        ),
        (
            "anchor_set_canonical_sha256",
            request.animated_socket_anchor_set_canonical_sha256.as_str(),
        ),
    ] {
        same_field(socket_link, field, expected, "animated socket")?;
    }
    if socket.get("restart_hash_verified").and_then(Value::as_bool) != Some(true) {
        return Err(invalid("animated socket restart readback is not verified"));
    }
    let socket_receipt = require_receipt(&socket, "animated socket")?;
    let socket_nodes = validate_socket_nodes(socket_receipt, request)?;
    let anchor = runtime.game_weapon_anchor_get(&json!({
        "schema_version":"GameWeaponAnchorGetRequest@1",
        "project_id":request.project_id,
        "delivery_manifest_object_sha256":request.delivery_manifest_object_sha256
    }))?;
    let anchor_link = require_link(&anchor, "AnchorSet")?;
    if anchor_link
        .get("anchor_set_object_sha256")
        .and_then(Value::as_str)
        != Some(request.animated_socket_anchor_set_object_sha256.as_str())
    {
        return Err(invalid("AnchorSet object binding differs"));
    }
    let anchor_set = anchor
        .get("anchor_set")
        .ok_or_else(|| invalid("AnchorSet payload is unavailable"))?;
    if anchor_set.get("canonical_sha256").and_then(Value::as_str)
        != Some(request.animated_socket_anchor_set_canonical_sha256.as_str())
        || anchor_set.get("project_id").and_then(Value::as_str) != Some(request.project_id.as_str())
        || anchor_set
            .get("delivery_manifest_object_sha256")
            .and_then(Value::as_str)
            != Some(request.delivery_manifest_object_sha256.as_str())
    {
        return Err(invalid(
            "AnchorSet canonical/project/delivery binding differs",
        ));
    }
    validate_socket_nodes_against_anchor_set(&socket_nodes, anchor_set)?;
    if socket_receipt
        .get("socket_node_id_encoding_sha256")
        .and_then(Value::as_str)
        != Some(request.socket_node_id_encoding_sha256.as_str())
    {
        return Err(invalid("animated socket node ID encoding differs"));
    }
    if socket_receipt
        .get("anchor_set_object_sha256")
        .and_then(Value::as_str)
        != Some(request.animated_socket_anchor_set_object_sha256.as_str())
    {
        return Err(invalid("animated socket AnchorSet differs"));
    }

    let clip = runtime.mechanical_animation_clip_get(&json!({
        "schema_version":"MechanicalAnimationClipGetRequest@1",
        "project_id":request.project_id,
        "candidate_id":request.candidate_id,
        "clip_id":request.animation_clip_id
    }))?;
    let clip_link = require_link(&clip, "animation clip")?;
    for (field, expected) in [
        ("project_id", request.project_id.as_str()),
        ("candidate_id", request.candidate_id.as_str()),
        ("artifact_id", request.source_artifact_sha256.as_str()),
        (
            "clip_object_sha256",
            request.animation_clip_object_sha256.as_str(),
        ),
        (
            "clip_sha256",
            request.animation_clip_canonical_sha256.as_str(),
        ),
    ] {
        same_field(clip_link, field, expected, "animation clip")?;
    }

    let profile = runtime.fictional_energy_vfx_get(&json!({
        "schema_version":"FictionalEnergyVfxGetRequest@1",
        "project_id":request.project_id,
        "delivery_manifest_object_sha256":request.delivery_manifest_object_sha256
    }))?;
    let profile_link = require_link(&profile, "VFX profile")?;
    same_field(
        profile_link,
        "vfx_profile_object_sha256",
        &request.vfx_profile_object_sha256,
        "VFX profile",
    )?;
    let profile_value = profile
        .get("vfx_profile")
        .ok_or_else(|| invalid("VFX profile payload is unavailable"))?;
    if profile_value
        .get("canonical_sha256")
        .and_then(Value::as_str)
        != Some(request.vfx_profile_canonical_sha256.as_str())
    {
        return Err(invalid("VFX profile canonical hash differs"));
    }

    let sequence = runtime.fictional_energy_vfx_rendered_sequence_get(&json!({
        "schema_version":"FictionalEnergyVfxRenderedSequenceGetRequest@1",
        "project_id":request.project_id,
        "sequence_key_sha256":request.vfx_sequence_key_sha256
    }))?;
    let sequence_link = require_link(&sequence, "VFX sequence")?;
    same_field(
        sequence_link,
        "sequence_key_sha256",
        &request.vfx_sequence_key_sha256,
        "VFX sequence",
    )?;
    same_field(
        sequence_link,
        "canonical_sha256",
        &request.vfx_sequence_canonical_sha256,
        "VFX sequence",
    )?;
    ensure_common_layer_binding(&sequence, "VFX sequence", request)?;
    let sequence_receipt = require_receipt(&sequence, "VFX sequence")?;
    if sequence_receipt
        .get("sampled_emissive_sequence_rendered")
        .and_then(Value::as_bool)
        != Some(true)
        || sequence_receipt
            .get("emissive_animation_sequence_rendered")
            .and_then(Value::as_bool)
            != Some(false)
    {
        return Err(invalid("VFX sequence sampling semantics differ"));
    }
    // The current sequence is deliberately sidecar-only.  Keep the failure
    // explicit and before any downstream key lookup or reservation; a future
    // producer must prove the actual animated socket execution here.
    validate_socket_execution(&sequence, "VFX sequence")?;
    let frame_entries = validate_frame_order(sequence_receipt)?;
    let sequence_nodes = socket_nodes;
    let _ = sequence_nodes;
    let mut frame_results = Vec::with_capacity(frame_entries.len());
    for (ordinal, frame_key, sample_time_ticks) in frame_entries {
        let frame = runtime.fictional_energy_vfx_rendered_frame_get(&json!({
            "schema_version":"FictionalEnergyVfxRenderedFrameGetRequest@1",
            "project_id":request.project_id,
            "frame_key_sha256":frame_key
        }))?;
        ensure_common_layer_binding(&frame, "VFX frame", request)?;
        let frame_link = require_link(&frame, "VFX frame")?;
        let frame_receipt = require_receipt(&frame, "VFX frame")?;
        if frame_receipt
            .get("sample_time_ticks")
            .and_then(Value::as_u64)
            != Some(sample_time_ticks)
        {
            return Err(invalid("VFX frame sample time differs from sequence"));
        }
        let bloom_key = effect_key(&frame, "bloom_key_sha256", "VFX frame")?;
        let bloom = runtime.fictional_energy_vfx_hdr_bloom_get(&json!({
            "schema_version":"FictionalEnergyVfxHdrBloomFrameGetRequest@1",
            "project_id":request.project_id,
            "bloom_key_sha256":bloom_key
        }))?;
        ensure_common_layer_binding(&bloom, "VFX bloom", request)?;
        let bloom_link = require_link(&bloom, "VFX bloom")?;
        let particle_key = effect_key(&frame, "particle_key_sha256", "VFX frame")?;
        let particles = runtime.fictional_energy_vfx_particles_get(&json!({
            "schema_version":"FictionalEnergyVfxParticlesFrameGetRequest@1",
            "project_id":request.project_id,
            "particle_key_sha256":particle_key
        }))?;
        ensure_common_layer_binding(&particles, "VFX particles", request)?;
        let particle_link = require_link(&particles, "VFX particles")?;
        let trail_key = effect_key(&particles, "trail_key_sha256", "VFX particles")?;
        let trails = runtime.fictional_energy_vfx_trails_get(&json!({
            "schema_version":"FictionalEnergyVfxTrailsFrameGetRequest@1",
            "project_id":request.project_id,
            "trail_key_sha256":trail_key
        }))?;
        ensure_common_layer_binding(&trails, "VFX trails", request)?;
        let trail_link = require_link(&trails, "VFX trails")?;
        let trail_bloom_key = effect_key(&trails, "trail_bloom_key_sha256", "VFX trails")?;
        let trail_bloom = runtime.fictional_energy_vfx_trails_bloom_get(&json!({
            "schema_version":"FictionalEnergyVfxTrailsBloomFrameGetRequest@1",
            "project_id":request.project_id,
            "trail_bloom_key_sha256":trail_bloom_key
        }))?;
        ensure_common_layer_binding(&trail_bloom, "VFX trail bloom", request)?;
        let trail_bloom_link = require_link(&trail_bloom, "VFX trail bloom")?;

        // Every getter above owns its own recursive CAS/SQLite readback.  The
        // attachment gate additionally requires one immutable cross-layer
        // cohort: no camera/profile/worker/candidate/artifact retargeting is
        // allowed between frame, bloom, particles, trails and trail bloom.
        for (index, link) in [
            frame_link,
            bloom_link,
            particle_link,
            trail_link,
            trail_bloom_link,
        ]
        .iter()
        .enumerate()
        {
            for field in [
                "project_id",
                "delivery_manifest_object_sha256",
                "vfx_profile_object_sha256",
                "source_candidate_id",
                "source_artifact_sha256",
                "camera_object_sha256",
                "camera_identity_sha256",
                "render_profile_sha256",
                "render_worker_build_cohort_sha256",
            ] {
                same_link_field(sequence_link, link, field, &format!("VFX layer {index}"))?;
            }
        }

        // sample_request_sha256 is intentionally not guessed or synthesized:
        // the contract has no common sample key for this attachment.  A
        // producer that reaches this point must expose the exact same sample
        // request on all five links; otherwise the dependency is unsupported.
        let sample_request = frame_link
            .get("sample_request_sha256")
            .and_then(Value::as_str)
            .filter(|value| is_sha256(value))
            .ok_or_else(|| invalid("VFX frame sample request is unavailable"))?;
        for (index, link) in [bloom_link, particle_link, trail_link, trail_bloom_link]
            .iter()
            .enumerate()
        {
            if link.get("sample_request_sha256").and_then(Value::as_str) != Some(sample_request) {
                return Err(invalid(format!(
                    "VFX layer {index} sample request differs; attachment contract has no safe sample-key projection"
                )));
            }
        }

        // This is intentionally checked after all ordinary getters have
        // independently revalidated their own CAS/SQLite links.  Current
        // source receipts fail at this point because they advertise the
        // sidecar-only boundary; no reservation has been opened.
        for (label, value) in [
            ("VFX frame", &frame),
            ("VFX bloom", &bloom),
            ("VFX particles", &particles),
            ("VFX trails", &trails),
            ("VFX trail bloom", &trail_bloom),
        ] {
            validate_socket_execution(value, label)?;
        }
        if frame_link.get("frame_key_sha256").and_then(Value::as_str) != Some(frame_key.as_str())
            || bloom_link
                .get("base_frame_key_sha256")
                .and_then(Value::as_str)
                != Some(frame_key.as_str())
            || particle_link
                .get("base_frame_key_sha256")
                .and_then(Value::as_str)
                != Some(frame_key.as_str())
            || trail_link
                .get("base_frame_key_sha256")
                .and_then(Value::as_str)
                != Some(frame_key.as_str())
            || trail_bloom_link
                .get("base_frame_key_sha256")
                .and_then(Value::as_str)
                != Some(frame_key.as_str())
            || particle_link
                .get("bloom_key_sha256")
                .and_then(Value::as_str)
                != bloom_link.get("bloom_key_sha256").and_then(Value::as_str)
            || trail_link.get("bloom_key_sha256").and_then(Value::as_str)
                != bloom_link.get("bloom_key_sha256").and_then(Value::as_str)
            || trail_link
                .get("current_particle_key_sha256")
                .and_then(Value::as_str)
                != particle_link
                    .get("particle_key_sha256")
                    .and_then(Value::as_str)
            || trail_bloom_link
                .get("bloom_key_sha256")
                .and_then(Value::as_str)
                != bloom_link.get("bloom_key_sha256").and_then(Value::as_str)
            || trail_bloom_link
                .get("source_trail_key_sha256")
                .and_then(Value::as_str)
                != trail_link.get("trail_key_sha256").and_then(Value::as_str)
        {
            return Err(invalid("VFX layer frame parent binding differs"));
        }
        frame_results.push(json!({
            "ordinal":ordinal,
            "frame_key_sha256":frame_key,
            "sample_time_ticks":sample_time_ticks,
            "frame":frame,
            "bloom":bloom,
            "particles":particles,
            "trails":trails,
            "trail_bloom":trail_bloom,
        }));
    }
    Ok(Dependencies {
        socket,
        frames: frame_results,
    })
}

fn frame_record(
    _attachment_key: &str,
    _frame: &Value,
    _socket_nodes: &Value,
) -> Result<FictionalEnergyVfxAnimatedSocketAttachmentFrameRecord, RuntimeError> {
    // The current attachment contract contains only hashes and keys; it has
    // no per-frame composed socket transforms. Hashing the static six-node
    // receipt here would make every frame look sampled while proving nothing
    // about animation. Keep this producer fail-closed until a typed,
    // independently verified frame transform projection is available.
    Err(invalid(
        "per-frame composed socket transform readback is not represented by the current attachment contract; animated attachment prepare is unsupported",
    ))
}

fn effect_key(value: &Value, field: &str, label: &str) -> Result<String, RuntimeError> {
    for candidate in [
        value.get(field),
        value.get("link").and_then(|link| link.get(field)),
        value.get("receipt").and_then(|receipt| receipt.get(field)),
        value
            .get("render_set")
            .and_then(|render_set| render_set.get(field)),
    ] {
        if let Some(key) = candidate
            .and_then(Value::as_str)
            .filter(|key| is_sha256(key))
        {
            return Ok(key.to_owned());
        }
    }
    Err(invalid(format!(
        "{label} does not expose explicit {field} binding; the current contract has no downstream key field and Runtime will not guess a Store key"
    )))
}

fn build_record(
    request: &FictionalEnergyVfxAnimatedSocketAttachmentPrepareRequest,
    deps: &Dependencies,
    request_sha256: &str,
) -> Result<FictionalEnergyVfxAnimatedSocketAttachmentRecord, RuntimeError> {
    let socket_receipt = require_receipt(&deps.socket, "animated socket")?;
    let socket_nodes = validate_socket_nodes(socket_receipt, request)?;
    let frames = deps
        .frames
        .iter()
        .map(|frame| frame_record(&request.attachment_key_sha256, frame, &socket_nodes))
        .collect::<Result<Vec<_>, _>>()?;
    let mut record = FictionalEnergyVfxAnimatedSocketAttachmentRecord {
        schema_version: RECORD_SCHEMA.to_owned(),
        attachment_key_sha256: request.attachment_key_sha256.clone(),
        project_id: request.project_id.clone(),
        delivery_manifest_object_sha256: request.delivery_manifest_object_sha256.clone(),
        candidate_id: request.candidate_id.clone(),
        candidate_state_sha256: request.candidate_state_sha256.clone(),
        source_artifact_sha256: request.source_artifact_sha256.clone(),
        animated_socket_materialization_key_sha256: request
            .animated_socket_materialization_key_sha256
            .clone(),
        animated_socket_anchor_set_object_sha256: request
            .animated_socket_anchor_set_object_sha256
            .clone(),
        animated_socket_anchor_set_canonical_sha256: request
            .animated_socket_anchor_set_canonical_sha256
            .clone(),
        animation_clip_id: request.animation_clip_id.clone(),
        animation_clip_object_sha256: request.animation_clip_object_sha256.clone(),
        animation_clip_canonical_sha256: request.animation_clip_canonical_sha256.clone(),
        animated_artifact_sha256: request.animated_artifact_sha256.clone(),
        animation_receipt_object_sha256: request.animation_receipt_object_sha256.clone(),
        animation_receipt_canonical_sha256: request.animation_receipt_canonical_sha256.clone(),
        vfx_profile_object_sha256: request.vfx_profile_object_sha256.clone(),
        vfx_profile_canonical_sha256: request.vfx_profile_canonical_sha256.clone(),
        vfx_sequence_key_sha256: request.vfx_sequence_key_sha256.clone(),
        vfx_sequence_canonical_sha256: request.vfx_sequence_canonical_sha256.clone(),
        attachment_policy: ATTACHMENT_POLICY.to_owned(),
        socket_node_id_encoding_sha256: request.socket_node_id_encoding_sha256.clone(),
        socket_roles_sha256: request.socket_roles_sha256.clone(),
        frame_scope: FRAME_SCOPE.to_owned(),
        frames,
        attachment_status: ATTACHMENT_STATUS.to_owned(),
        canonical_sha256: String::new(),
        created_at: now_string(),
    };
    let mut value = serde_json::to_value(&record).map_err(|error| invalid(error.to_string()))?;
    value["canonical_sha256"] = Value::String(String::new());
    record.canonical_sha256 = canonical_json_hash(&value);
    if request_sha256 != request.input_sha256 {
        return Err(invalid(
            "attachment request hash changed while building record",
        ));
    }
    Ok(record)
}

fn replay_equivalent(
    left: &FictionalEnergyVfxAnimatedSocketAttachmentRecord,
    right: &FictionalEnergyVfxAnimatedSocketAttachmentRecord,
) -> Result<bool, RuntimeError> {
    fn normalize(value: &mut Value) {
        let Some(object) = value.as_object_mut() else {
            return;
        };
        object.insert("canonical_sha256".to_owned(), Value::String(String::new()));
        object.insert("created_at".to_owned(), Value::String(String::new()));
        if let Some(frames) = object.get_mut("frames").and_then(Value::as_array_mut) {
            for frame in frames {
                if let Some(frame_object) = frame.as_object_mut() {
                    frame_object
                        .insert("canonical_sha256".to_owned(), Value::String(String::new()));
                    frame_object.insert("created_at".to_owned(), Value::String(String::new()));
                }
            }
        }
    }

    let mut left = serde_json::to_value(left).map_err(|error| invalid(error.to_string()))?;
    let mut right = serde_json::to_value(right).map_err(|error| invalid(error.to_string()))?;
    normalize(&mut left);
    normalize(&mut right);
    Ok(left == right)
}

fn receipt_value(
    record: &FictionalEnergyVfxAnimatedSocketAttachmentRecord,
    deps: &Dependencies,
) -> Result<Value, RuntimeError> {
    let mut value = serde_json::to_value(record).map_err(|error| invalid(error.to_string()))?;
    let object = value
        .as_object_mut()
        .ok_or_else(|| invalid("attachment receipt is not an object"))?;
    // The durable parent uses RECORD_SCHEMA; the owned CAS projection has a
    // distinct schema so Store/get can reject a record-shaped or stale
    // receipt.  Keep the parent fields unchanged and only replace the
    // projection schema marker.
    object.insert(
        "schema_version".to_owned(),
        Value::String(RECEIPT_SCHEMA.to_owned()),
    );
    object.insert(
        "frame_count".to_owned(),
        Value::Number((record.frames.len() as u64).into()),
    );
    object.insert(
        "socket_nodes".to_owned(),
        deps.socket["receipt"]["socket_nodes"].clone(),
    );
    object.insert(
        "frame_dependencies".to_owned(),
        Value::Array(deps.frames.clone()),
    );
    object.insert("runtime_write_performed".to_owned(), Value::Bool(true));
    object.insert("restart_hash_verified".to_owned(), Value::Bool(true));
    object.insert("candidate_confirmed".to_owned(), Value::Bool(false));
    object.insert("export_performed".to_owned(), Value::Bool(false));
    object.insert("actual_engine_roundtrip".to_owned(), Value::Bool(false));
    object.insert(
        "quality_status".to_owned(),
        Value::String("structural_only".to_owned()),
    );
    object.insert(
        "visual_quality_status".to_owned(),
        Value::String("NOT_PROVEN".to_owned()),
    );
    object.insert(
        "commercial_fps_quality_status".to_owned(),
        Value::String("NOT_PROVEN".to_owned()),
    );
    object.insert(
        "human_review_status".to_owned(),
        Value::String("NOT_RUN".to_owned()),
    );
    object.insert(
        "commercial_engine_status".to_owned(),
        Value::String("NOT_RUN".to_owned()),
    );
    // Keep the typed parent canonical.  Store treats this receipt as a
    // projection of the parent and requires this field to match the parent
    // record exactly; the full projection is still covered by its CAS hash
    // and compared independently on restart.
    Ok(value)
}

fn validate_attachment_receipt(
    runtime: &Runtime,
    receipt_hash: &str,
    record: &FictionalEnergyVfxAnimatedSocketAttachmentRecord,
) -> Result<Value, RuntimeError> {
    let bytes = runtime.cas_read_bounded(receipt_hash, MAX_RECEIPT_BYTES as u64)?;
    if sha256_hex(&bytes) != receipt_hash {
        return Err(invalid(
            "animated socket attachment receipt bytes are tampered",
        ));
    }
    let value: Value = serde_json::from_slice(&bytes).map_err(|error| {
        invalid(format!(
            "animated socket attachment receipt is malformed: {error}"
        ))
    })?;
    let object = value
        .as_object()
        .ok_or_else(|| invalid("animated socket attachment receipt is not an object"))?;
    if object.get("schema_version").and_then(Value::as_str) != Some(RECEIPT_SCHEMA)
        || object.get("attachment_key_sha256").and_then(Value::as_str)
            != Some(record.attachment_key_sha256.as_str())
        || object.get("canonical_sha256").and_then(Value::as_str)
            != Some(record.canonical_sha256.as_str())
        || object.get("frame_count").and_then(Value::as_u64) != Some(record.frames.len() as u64)
        || object
            .get("frames")
            .and_then(Value::as_array)
            .map_or(true, |frames| frames.len() != record.frames.len())
        || object
            .get("frame_dependencies")
            .and_then(Value::as_array)
            .map_or(true, |frames| frames.len() != record.frames.len())
    {
        return Err(invalid(
            "animated socket attachment receipt parent binding differs",
        ));
    }
    Ok(value)
}

fn result_value(
    record: &FictionalEnergyVfxAnimatedSocketAttachmentRecord,
    replayed: bool,
    schema_version: &str,
    runtime_write: bool,
) -> Result<Value, RuntimeError> {
    Ok(json!({
        "schema_version":schema_version,
        "attachment_key_sha256":record.attachment_key_sha256,
        "attachment":serde_json::to_value(record).map_err(|error| invalid(error.to_string()))?,
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

fn release_receipt(
    runtime: &Runtime,
    reservation: &forgecad_store::CasReservation,
    object: &CasObject,
    cleanup: bool,
) {
    let _ = runtime.store.release_cas_reservation_object(
        reservation,
        object,
        cleanup && object.created_new,
    );
}

pub(super) fn prepare(runtime: &Runtime, request: &Value) -> Result<Value, RuntimeError> {
    let (request, request_sha256) = parse_prepare(request)?;
    let dependencies = validate_dependencies(runtime, &request)?;
    let record = build_record(&request, &dependencies, &request_sha256)?;

    // Replay is resolved only after the full read-only dependency walk.  A
    // fresh timestamp/canonical projection must not force a second receipt;
    // conversely, a retargeted binding fails closed without opening a CAS
    // reservation or touching SQLite.
    if let Some(existing) = runtime
        .store
        .get_fictional_energy_vfx_animated_socket_attachment_link(&request.attachment_key_sha256)?
    {
        if replay_equivalent(&existing, &record)? {
            return result_value(&existing, true, PREPARE_RESULT_SCHEMA, false);
        }
        return Err(invalid(
            "animated socket attachment key is already bound to different content",
        ));
    }
    let receipt = receipt_value(&record, &dependencies)?;
    let bytes = canonical_json_bytes(&receipt).map_err(|error| invalid(error.to_string()))?;
    if bytes.is_empty() || bytes.len() > MAX_RECEIPT_BYTES || sha256_hex(&bytes).is_empty() {
        return Err(invalid("attachment receipt exceeds the one MiB bound"));
    }
    let reservation = runtime.store.begin_cas_reservation();
    let receipt_object = runtime.store.put_object_reserved(
        &reservation,
        &bytes,
        None,
        RECEIPT_MIME,
        RECEIPT_KIND,
        &record.created_at,
    )?;
    if let Err(error) = validate_attachment_receipt(runtime, &receipt_object.record.sha256, &record)
    {
        release_receipt(runtime, &reservation, &receipt_object, true);
        return Err(error);
    }
    let result = runtime
        .store
        .record_fictional_energy_vfx_animated_socket_attachment_link(
            &record,
            &receipt_object.record,
        );
    match result {
        Ok(stored) => {
            release_receipt(runtime, &reservation, &receipt_object, false);
            result_value(&stored, false, PREPARE_RESULT_SCHEMA, true)
        }
        Err(error) => {
            release_receipt(runtime, &reservation, &receipt_object, true);
            Err(error.into())
        }
    }
}

pub(super) fn get(runtime: &Runtime, request: &Value) -> Result<Value, RuntimeError> {
    let request = parse_get(request)?;
    let record = runtime
        .store
        .get_fictional_energy_vfx_animated_socket_attachment_link(&request.attachment_key_sha256)?
        .ok_or_else(|| invalid("durable animated socket attachment is unavailable"))?;
    if record.project_id != request.project_id || record.candidate_id != request.candidate_id {
        return Err(invalid("animated socket attachment scope differs"));
    }
    // get is read-only: no reservation and no CAS writes.  Reconstructing the
    // same dependency chain independently catches stale frame order, source
    // retargets and tampered receipt bytes after a Runtime restart.
    let replay_request = FictionalEnergyVfxAnimatedSocketAttachmentPrepareRequest {
        schema_version: PREPARE_SCHEMA.to_owned(),
        attachment_key_sha256: record.attachment_key_sha256.clone(),
        project_id: record.project_id.clone(),
        delivery_manifest_object_sha256: record.delivery_manifest_object_sha256.clone(),
        candidate_id: record.candidate_id.clone(),
        candidate_state_sha256: record.candidate_state_sha256.clone(),
        source_artifact_sha256: record.source_artifact_sha256.clone(),
        animated_socket_materialization_key_sha256: record
            .animated_socket_materialization_key_sha256
            .clone(),
        animated_socket_anchor_set_object_sha256: record
            .animated_socket_anchor_set_object_sha256
            .clone(),
        animated_socket_anchor_set_canonical_sha256: record
            .animated_socket_anchor_set_canonical_sha256
            .clone(),
        animation_clip_id: record.animation_clip_id.clone(),
        animation_clip_object_sha256: record.animation_clip_object_sha256.clone(),
        animation_clip_canonical_sha256: record.animation_clip_canonical_sha256.clone(),
        animated_artifact_sha256: record.animated_artifact_sha256.clone(),
        animation_receipt_object_sha256: record.animation_receipt_object_sha256.clone(),
        animation_receipt_canonical_sha256: record.animation_receipt_canonical_sha256.clone(),
        vfx_profile_object_sha256: record.vfx_profile_object_sha256.clone(),
        vfx_profile_canonical_sha256: record.vfx_profile_canonical_sha256.clone(),
        vfx_sequence_key_sha256: record.vfx_sequence_key_sha256.clone(),
        vfx_sequence_canonical_sha256: record.vfx_sequence_canonical_sha256.clone(),
        attachment_policy: record.attachment_policy.clone(),
        socket_node_id_encoding_sha256: record.socket_node_id_encoding_sha256.clone(),
        socket_roles_sha256: record.socket_roles_sha256.clone(),
        frame_scope: record.frame_scope.clone(),
        input_sha256: record.attachment_key_sha256.clone(),
        idempotency_key: record.attachment_key_sha256.clone(),
    };
    let dependencies = validate_dependencies(runtime, &replay_request)?;
    let recomputed = build_record(
        &replay_request,
        &dependencies,
        &record.attachment_key_sha256,
    )?;
    if !replay_equivalent(&recomputed, &record)? {
        return Err(invalid("animated socket attachment receipt is tampered"));
    }
    result_value(&record, true, GET_RESULT_SCHEMA, false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sidecar_only_receipts_are_rejected() {
        let value = json!({
            "receipt": {
                "anchor_is_runtime_sidecar_not_glb_socket": true,
                "limitations": ["anchor-sidecar-not-glb-socket"]
            }
        });
        assert!(validate_socket_execution(&value, "trail").is_err());
    }

    #[test]
    fn sequence_frame_order_is_strict_and_bounded() {
        let good = json!({
            "frames": [
                {"ordinal":0,"frame_key_sha256":"a".repeat(64),"sample_time_ticks":10},
                {"ordinal":1,"frame_key_sha256":"b".repeat(64),"sample_time_ticks":20}
            ]
        });
        assert_eq!(validate_frame_order(&good).unwrap().len(), 2);
        let bad = json!({
            "frames": [
                {"ordinal":0,"frame_key_sha256":"a".repeat(64),"sample_time_ticks":20},
                {"ordinal":1,"frame_key_sha256":"b".repeat(64),"sample_time_ticks":10}
            ]
        });
        assert!(validate_frame_order(&bad).is_err());
    }

    #[test]
    fn request_parser_is_closed_and_policy_bound() {
        let mut request = json!({
            "schema_version":PREPARE_SCHEMA,
            "attachment_key_sha256":"a".repeat(64),
            "project_id":"p",
            "delivery_manifest_object_sha256":"b".repeat(64),
            "candidate_id":"c",
            "candidate_state_sha256":"d".repeat(64),
            "source_artifact_sha256":"e".repeat(64),
            "animated_socket_materialization_key_sha256":"f".repeat(64),
            "animated_socket_anchor_set_object_sha256":"0".repeat(64),
            "animated_socket_anchor_set_canonical_sha256":"1".repeat(64),
            "animation_clip_id":"clip",
            "animation_clip_object_sha256":"2".repeat(64),
            "animation_clip_canonical_sha256":"3".repeat(64),
            "animated_artifact_sha256":"4".repeat(64),
            "animation_receipt_object_sha256":"5".repeat(64),
            "animation_receipt_canonical_sha256":"6".repeat(64),
            "vfx_profile_object_sha256":"7".repeat(64),
            "vfx_profile_canonical_sha256":"8".repeat(64),
            "vfx_sequence_key_sha256":"9".repeat(64),
            "vfx_sequence_canonical_sha256":"a".repeat(64),
            "attachment_policy":ATTACHMENT_POLICY,
            "socket_node_id_encoding_sha256":"b".repeat(64),
            "socket_roles_sha256":"c".repeat(64),
            "frame_scope":FRAME_SCOPE,
            "input_sha256":"",
            "idempotency_key":"id"
        });
        let input = {
            let object = request.as_object_mut().unwrap();
            let mut preimage = object.clone();
            preimage.remove("attachment_key_sha256");
            preimage.remove("input_sha256");
            preimage.remove("idempotency_key");
            canonical_json_hash(&Value::Object(preimage))
        };
        request["attachment_key_sha256"] = Value::String(input.clone());
        request["input_sha256"] = Value::String(input);
        assert!(parse_prepare(&request).is_ok());
        request["unknown"] = Value::Bool(true);
        assert!(parse_prepare(&request).is_err());
    }

    #[test]
    fn static_socket_projection_cannot_become_a_frame_readback() {
        let frame = json!({
            "frame_key_sha256":"a".repeat(64),
            "sample_time_ticks":1,
            "ordinal":0,
            "particles":{"receipt":{"owner_world_transforms":[]}},
            "trails":{"receipt":{"owner_world_transforms":[]}}
        });
        let socket_nodes = json!([
            {"socket_node_id":"n0","anchor_id":"weapon-root","role":"weapon-root"}
        ]);
        let error = frame_record("b".repeat(64).as_str(), &frame, &socket_nodes)
            .expect_err("static socket projection must be rejected");
        assert!(error
            .to_string()
            .contains("per-frame composed socket transform"));
    }

    #[test]
    fn downstream_key_lookup_is_explicit_only() {
        let error = effect_key(&json!({"link":{}}), "bloom_key_sha256", "VFX frame")
            .expect_err("attachment must not guess a downstream key");
        assert!(error.to_string().contains("will not guess a Store key"));
    }
}
