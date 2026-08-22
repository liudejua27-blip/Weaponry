//! Additive terminal Attachment@3 bridge.
//!
//! Attachment@3 is a structural, hash-only join over the already durable
//! Projection@2, Particles@2, Trails@2 and TrailsBloom@2 records.  It owns a
//! single canonical JSON receipt and deliberately does not create another
//! image, GLB, candidate, version or engine-import result.  Every dependency
//! is read and checked before the CAS reservation begins.

use super::{
    canonical_json_bytes, canonical_json_hash, exact_object, is_opaque_id, is_sha256, now_string,
    sha256_hex, CasObject, Runtime, RuntimeError,
};
use forgecad_contracts::{
    FictionalEnergyVfxAnimatedSocketAttachmentV3FrameRecord,
    FictionalEnergyVfxAnimatedSocketAttachmentV3GetRequest,
    FictionalEnergyVfxAnimatedSocketAttachmentV3PrepareRequest,
    FictionalEnergyVfxAnimatedSocketAttachmentV3Record,
    FictionalEnergyVfxAnimatedSocketParticlesSequenceV2,
    FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceV2,
    FictionalEnergyVfxAnimatedSocketTrailsSequenceV2,
    GameWeaponAnimatedGlbSocketTransformProjectionV2,
};
use serde_json::{json, Map, Value};

const PREPARE_SCHEMA: &str = "FictionalEnergyVfxAnimatedSocketAttachmentPrepareRequest@3";
const GET_SCHEMA: &str = "FictionalEnergyVfxAnimatedSocketAttachmentGetRequest@3";
const PREPARE_RESULT_SCHEMA: &str = "FictionalEnergyVfxAnimatedSocketAttachmentPrepareResult@3";
const GET_RESULT_SCHEMA: &str = "FictionalEnergyVfxAnimatedSocketAttachmentGetResult@3";
const RECORD_SCHEMA: &str = "FictionalEnergyVfxAnimatedSocketAttachment@3";
const FRAME_SCHEMA: &str = "FictionalEnergyVfxAnimatedSocketAttachmentFrame@3";
const RECEIPT_SCHEMA: &str = "FictionalEnergyVfxAnimatedSocketAttachmentReceipt@3";
const RECEIPT_KIND: &str = "fictional-energy-vfx-animated-socket-attachment-v3-receipt";
const RECEIPT_MIME: &str = "application/json";
const MAX_RECEIPT_BYTES: usize = 1024 * 1024;
const MAX_FRAMES: usize = 15;
const UPSTREAM_FRAMES: usize = MAX_FRAMES + 1;

const PREPARE_FIELDS: &[&str] = &[
    "schema_version",
    "attachment_key_sha256",
    "project_id",
    "geometry_candidate_id",
    "geometry_candidate_state_sha256",
    "geometry_delivery_manifest_object_sha256",
    "geometry_artifact_sha256",
    "appearance_candidate_id",
    "appearance_candidate_state_sha256",
    "appearance_delivery_manifest_object_sha256",
    "appearance_artifact_sha256",
    "material_surface_quality_id",
    "material_surface_quality_report_object_sha256",
    "material_surface_quality_canonical_sha256",
    "geometry_preservation_projection_sha256",
    "geometry_preservation_status",
    "animated_socket_materialization_key_sha256",
    "animated_artifact_sha256",
    "animated_socket_anchor_set_object_sha256",
    "animated_socket_anchor_set_canonical_sha256",
    "appearance_anchor_set_object_sha256",
    "appearance_anchor_set_canonical_sha256",
    "anchor_binding_policy",
    "animation_clip_id",
    "animation_clip_object_sha256",
    "animation_clip_canonical_sha256",
    "animation_receipt_object_sha256",
    "animation_receipt_canonical_sha256",
    "projection_key_sha256",
    "projection_object_sha256",
    "projection_canonical_sha256",
    "particle_sequence_key_sha256",
    "particle_sequence_canonical_sha256",
    "trail_sequence_key_sha256",
    "trail_sequence_canonical_sha256",
    "trail_bloom_sequence_key_sha256",
    "trail_bloom_sequence_canonical_sha256",
    "vfx_profile_object_sha256",
    "vfx_profile_canonical_sha256",
    "trail_bloom_profile_sha256",
    "socket_node_id_encoding_sha256",
    "socket_roles_sha256",
    "camera_object_sha256",
    "camera_identity_sha256",
    "render_profile_sha256",
    "render_worker_build_cohort_sha256",
    "sample_schedule_sha256",
    "sample_count",
    "sample_time_ticks",
    "attachment_policy",
    "frame_scope",
    "input_sha256",
    "idempotency_key",
];

const GET_FIELDS: &[&str] = &[
    "schema_version",
    "attachment_key_sha256",
    "project_id",
    "geometry_candidate_id",
    "appearance_candidate_id",
    "geometry_delivery_manifest_object_sha256",
    "appearance_delivery_manifest_object_sha256",
];

fn invalid(detail: impl Into<String>) -> RuntimeError {
    RuntimeError::InvalidInput(format!(
        "FICTIONAL_ENERGY_VFX_ANIMATED_SOCKET_ATTACHMENT_V3_INVALID: {}",
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

fn sha(object: &Map<String, Value>, field: &str) -> Result<(), RuntimeError> {
    if !is_sha256(text(object, field)?) {
        return Err(invalid(format!("{field} is not a SHA-256")));
    }
    Ok(())
}

fn id(object: &Map<String, Value>, field: &str) -> Result<(), RuntimeError> {
    if !is_opaque_id(text(object, field)?) {
        return Err(invalid(format!("{field} is not an opaque identifier")));
    }
    Ok(())
}

fn parse_prepare(
    value: &Value,
) -> Result<FictionalEnergyVfxAnimatedSocketAttachmentV3PrepareRequest, RuntimeError> {
    let object = exact_object(value, PREPARE_FIELDS, PREPARE_SCHEMA)?;
    if text(object, "schema_version")? != PREPARE_SCHEMA {
        return Err(invalid("prepare schema differs"));
    }
    let request: FictionalEnergyVfxAnimatedSocketAttachmentV3PrepareRequest =
        serde_json::from_value(value.clone())
            .map_err(|error| invalid(format!("prepare request is malformed: {error}")))?;
    for field in [
        "attachment_key_sha256",
        "geometry_candidate_state_sha256",
        "geometry_delivery_manifest_object_sha256",
        "geometry_artifact_sha256",
        "appearance_candidate_state_sha256",
        "appearance_delivery_manifest_object_sha256",
        "appearance_artifact_sha256",
        "material_surface_quality_report_object_sha256",
        "material_surface_quality_canonical_sha256",
        "geometry_preservation_projection_sha256",
        "animated_socket_materialization_key_sha256",
        "animated_artifact_sha256",
        "animated_socket_anchor_set_object_sha256",
        "animated_socket_anchor_set_canonical_sha256",
        "appearance_anchor_set_object_sha256",
        "appearance_anchor_set_canonical_sha256",
        "animation_clip_object_sha256",
        "animation_clip_canonical_sha256",
        "animation_receipt_object_sha256",
        "animation_receipt_canonical_sha256",
        "projection_key_sha256",
        "projection_object_sha256",
        "projection_canonical_sha256",
        "particle_sequence_key_sha256",
        "particle_sequence_canonical_sha256",
        "trail_sequence_key_sha256",
        "trail_sequence_canonical_sha256",
        "trail_bloom_sequence_key_sha256",
        "trail_bloom_sequence_canonical_sha256",
        "vfx_profile_object_sha256",
        "vfx_profile_canonical_sha256",
        "trail_bloom_profile_sha256",
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
        "geometry_candidate_id",
        "appearance_candidate_id",
        "material_surface_quality_id",
        "animation_clip_id",
        "idempotency_key",
    ] {
        id(object, field)?;
    }
    if request.geometry_candidate_id == request.appearance_candidate_id
        || request.geometry_artifact_sha256 == request.appearance_artifact_sha256
        || request.attachment_policy
            != forgecad_contracts::FICTIONAL_ENERGY_VFX_ANIMATED_SOCKET_ATTACHMENT_V3_POLICY
        || request.frame_scope
            != forgecad_contracts::FICTIONAL_ENERGY_VFX_ANIMATED_SOCKET_ATTACHMENT_V3_FRAME_SCOPE
        || request.sample_count != MAX_FRAMES as u64
        || request.sample_time_ticks.len() != MAX_FRAMES
        || request
            .sample_time_ticks
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        || request
            .sample_time_ticks
            .iter()
            .any(|tick| *tick > 1_000_000)
        || request.geometry_preservation_status != "source-output-renderable-geometry-byte-exact"
        || request.anchor_binding_policy != "geometry-appearance-anchor-role-owner-trs-equivalent@1"
    {
        return Err(invalid(
            "V3 policy, lineage or exact 15-tick schedule differs",
        ));
    }
    let mut preimage = object.clone();
    preimage.remove("attachment_key_sha256");
    preimage.remove("input_sha256");
    preimage.remove("idempotency_key");
    let expected = canonical_json_hash(&Value::Object(preimage));
    if request.input_sha256 != expected || request.attachment_key_sha256 != expected {
        return Err(invalid("input or attachment key hash differs"));
    }
    Ok(request)
}

fn parse_get(
    value: &Value,
) -> Result<FictionalEnergyVfxAnimatedSocketAttachmentV3GetRequest, RuntimeError> {
    let object = exact_object(value, GET_FIELDS, GET_SCHEMA)?;
    if text(object, "schema_version")? != GET_SCHEMA {
        return Err(invalid("get schema differs"));
    }
    for field in [
        "attachment_key_sha256",
        "geometry_delivery_manifest_object_sha256",
        "appearance_delivery_manifest_object_sha256",
    ] {
        sha(object, field)?;
    }
    for field in [
        "project_id",
        "geometry_candidate_id",
        "appearance_candidate_id",
    ] {
        id(object, field)?;
    }
    let request: FictionalEnergyVfxAnimatedSocketAttachmentV3GetRequest =
        serde_json::from_value(value.clone())
            .map_err(|error| invalid(format!("get request is malformed: {error}")))?;
    if request.geometry_candidate_id == request.appearance_candidate_id {
        return Err(invalid("V3 candidates must remain distinct"));
    }
    Ok(request)
}

fn expect(label: &str, actual: Option<&str>, expected: &str) -> Result<(), RuntimeError> {
    if actual != Some(expected) {
        return Err(invalid(format!("{label} binding differs")));
    }
    Ok(())
}

fn require_read_only(value: &Value, label: &str) -> Result<(), RuntimeError> {
    let write = value
        .get("runtime_write")
        .or_else(|| value.get("runtime_write_performed"))
        .and_then(Value::as_bool);
    if write != Some(false)
        || value.get("restart_hash_verified").and_then(Value::as_bool) != Some(true)
    {
        return Err(invalid(format!(
            "{label} is not a verified read-only result"
        )));
    }
    if value
        .get("actual_engine_roundtrip")
        .and_then(Value::as_bool)
        == Some(true)
    {
        return Err(invalid(format!("{label} claims an engine roundtrip")));
    }
    Ok(())
}

fn validate_projection(
    value: &Value,
    request: &FictionalEnergyVfxAnimatedSocketAttachmentV3PrepareRequest,
) -> Result<GameWeaponAnimatedGlbSocketTransformProjectionV2, RuntimeError> {
    require_read_only(value, "Projection@2")?;
    expect(
        "Projection@2 schema",
        value.get("schema_version").and_then(Value::as_str),
        "GameWeaponAnimatedGlbSocketTransformProjectionGetResult@2",
    )?;
    expect(
        "Projection@2 object",
        value
            .get("projection_object_sha256")
            .and_then(Value::as_str),
        &request.projection_object_sha256,
    )?;
    let projection: GameWeaponAnimatedGlbSocketTransformProjectionV2 = serde_json::from_value(
        value
            .get("projection")
            .cloned()
            .ok_or_else(|| invalid("Projection@2 payload is unavailable"))?,
    )
    .map_err(|error| invalid(format!("Projection@2 payload is malformed: {error}")))?;
    for (label, actual, expected) in [
        (
            "projection key",
            projection.projection_key_sha256.as_str(),
            request.projection_key_sha256.as_str(),
        ),
        (
            "projection project",
            projection.project_id.as_str(),
            request.project_id.as_str(),
        ),
        (
            "projection candidate",
            projection.appearance_candidate_id.as_str(),
            request.appearance_candidate_id.as_str(),
        ),
        (
            "projection candidate state",
            projection.appearance_candidate_state_sha256.as_str(),
            request.appearance_candidate_state_sha256.as_str(),
        ),
        (
            "projection delivery",
            projection
                .appearance_delivery_manifest_object_sha256
                .as_str(),
            request.appearance_delivery_manifest_object_sha256.as_str(),
        ),
        (
            "projection artifact",
            projection.appearance_artifact_sha256.as_str(),
            request.appearance_artifact_sha256.as_str(),
        ),
        (
            "projection canonical",
            projection.canonical_sha256.as_str(),
            request.projection_canonical_sha256.as_str(),
        ),
        (
            "projection clip",
            projection.animation_clip_id.as_str(),
            request.animation_clip_id.as_str(),
        ),
        (
            "projection animated socket",
            projection
                .animated_socket_materialization_key_sha256
                .as_str(),
            request.animated_socket_materialization_key_sha256.as_str(),
        ),
        (
            "projection anchor",
            projection.anchor_set_object_sha256.as_str(),
            request.appearance_anchor_set_object_sha256.as_str(),
        ),
        (
            "projection anchor canonical",
            projection.anchor_set_canonical_sha256.as_str(),
            request.appearance_anchor_set_canonical_sha256.as_str(),
        ),
    ] {
        if actual != expected {
            return Err(invalid(format!("{label} binding differs")));
        }
    }
    if projection.frames.len() != UPSTREAM_FRAMES
        || projection.sample_count != UPSTREAM_FRAMES as u64
        || projection.sample_time_ticks.len() != UPSTREAM_FRAMES
        || projection.sample_time_ticks[1..] != request.sample_time_ticks
        || projection.frames.iter().enumerate().any(|(index, frame)| {
            frame.frame_index != index as u64
                || frame.sample_time_ticks != projection.sample_time_ticks[index]
        })
        || projection.quality_status != "structural_only"
        || projection.visual_quality_status != "NOT_PROVEN"
        || projection.commercial_fps_quality_status != "NOT_PROVEN"
        || projection.human_review_status != "NOT_RUN"
        || projection.commercial_engine_status != "NOT_RUN"
    {
        return Err(invalid("Projection@2 schedule or truth boundary differs"));
    }
    Ok(projection)
}

fn validate_clip_and_socket(
    runtime: &Runtime,
    request: &FictionalEnergyVfxAnimatedSocketAttachmentV3PrepareRequest,
) -> Result<(), RuntimeError> {
    let clip = runtime.mechanical_animation_clip_v2_get(&json!({
        "schema_version":"MechanicalAnimationClipGetRequest@2",
        "project_id":request.project_id,
        "appearance_candidate_id":request.appearance_candidate_id,
        "clip_id":request.animation_clip_id
    }))?;
    require_read_only(&clip, "Clip@2")?;
    expect(
        "Clip@2 schema",
        clip.get("schema_version").and_then(Value::as_str),
        "MechanicalAnimationClipGetResult@2",
    )?;
    let link = clip
        .get("durable_link")
        .ok_or_else(|| invalid("Clip@2 durable link is unavailable"))?;
    for (field, expected) in [
        ("project_id", request.project_id.as_str()),
        (
            "appearance_candidate_id",
            request.appearance_candidate_id.as_str(),
        ),
        ("clip_id", request.animation_clip_id.as_str()),
        (
            "clip_object_sha256",
            request.animation_clip_object_sha256.as_str(),
        ),
        (
            "clip_sha256",
            request.animation_clip_canonical_sha256.as_str(),
        ),
    ] {
        expect(
            &format!("Clip@2 {field}"),
            link.get(field).and_then(Value::as_str),
            expected,
        )?;
    }
    expect(
        "Clip@2 payload canonical",
        clip.get("clip")
            .and_then(|value| value.get("canonical_sha256"))
            .and_then(Value::as_str),
        &request.animation_clip_canonical_sha256,
    )?;
    let socket = runtime.game_weapon_animated_glb_socket_v2_get(&json!({
        "schema_version":"GameWeaponAnimatedGlbSocketMaterializationGetRequest@2",
        "project_id":request.project_id,
        "appearance_candidate_id":request.appearance_candidate_id,
        "clip_id":request.animation_clip_id,
        "animated_socket_materialization_key_sha256":request.animated_socket_materialization_key_sha256
    }))?;
    require_read_only(&socket, "AnimatedSocket@2")?;
    expect(
        "AnimatedSocket@2 schema",
        socket.get("schema_version").and_then(Value::as_str),
        "GameWeaponAnimatedGlbSocketMaterializationGetResult@2",
    )?;
    let socket_link = socket
        .get("durable_link")
        .ok_or_else(|| invalid("AnimatedSocket@2 durable link is unavailable"))?;
    for (field, expected) in [
        ("project_id", request.project_id.as_str()),
        (
            "appearance_candidate_id",
            request.appearance_candidate_id.as_str(),
        ),
        (
            "appearance_candidate_state_sha256",
            request.appearance_candidate_state_sha256.as_str(),
        ),
        (
            "appearance_delivery_manifest_object_sha256",
            request.appearance_delivery_manifest_object_sha256.as_str(),
        ),
        (
            "appearance_artifact_sha256",
            request.appearance_artifact_sha256.as_str(),
        ),
        (
            "animated_artifact_sha256",
            request.animated_artifact_sha256.as_str(),
        ),
        (
            "anchor_set_object_sha256",
            request.appearance_anchor_set_object_sha256.as_str(),
        ),
        (
            "anchor_set_canonical_sha256",
            request.appearance_anchor_set_canonical_sha256.as_str(),
        ),
        (
            "animation_receipt_object_sha256",
            request.animation_receipt_object_sha256.as_str(),
        ),
    ] {
        expect(
            &format!("AnimatedSocket@2 {field}"),
            socket_link.get(field).and_then(Value::as_str),
            expected,
        )?;
    }
    Ok(())
}

fn validate_appearance_glb(
    runtime: &Runtime,
    request: &FictionalEnergyVfxAnimatedSocketAttachmentV3PrepareRequest,
) -> Result<(), RuntimeError> {
    let object = runtime
        .store
        .get_object(&request.appearance_artifact_sha256)?
        .ok_or_else(|| invalid("appearance artifact object is unavailable"))?;
    if object.schema_version != "CasObject@1"
        || object.sha256 != request.appearance_artifact_sha256
        || object.mime != "model/gltf-binary"
        || object.size_bytes == 0
        || object.size_bytes > 64 * 1024 * 1024
        || !matches!(object.kind.as_str(), "appearance-glb" | "appearance-v2-glb")
    {
        return Err(invalid("appearance artifact metadata differs"));
    }
    let bytes = runtime.cas_read_bounded(&request.appearance_artifact_sha256, 64 * 1024 * 1024)?;
    if bytes.len() as u64 != object.size_bytes
        || sha256_hex(&bytes) != request.appearance_artifact_sha256
    {
        return Err(invalid("appearance artifact bytes differ"));
    }
    Ok(())
}

fn validate_dependencies(
    runtime: &Runtime,
    request: &FictionalEnergyVfxAnimatedSocketAttachmentV3PrepareRequest,
) -> Result<
    (
        GameWeaponAnimatedGlbSocketTransformProjectionV2,
        FictionalEnergyVfxAnimatedSocketParticlesSequenceV2,
        FictionalEnergyVfxAnimatedSocketTrailsSequenceV2,
        FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceV2,
    ),
    RuntimeError,
> {
    validate_appearance_glb(runtime, request)?;
    validate_clip_and_socket(runtime, request)?;
    let projection_value =
        runtime.game_weapon_animated_glb_socket_transform_projection_v2_get(&json!({
            "schema_version":"GameWeaponAnimatedGlbSocketTransformProjectionGetRequest@2",
            "projection_key_sha256":request.projection_key_sha256,
            "project_id":request.project_id,
            "appearance_candidate_id":request.appearance_candidate_id,
            "animation_clip_id":request.animation_clip_id
        }))?;
    let projection = validate_projection(&projection_value, request)?;

    let particle_value = runtime.fictional_energy_vfx_animated_socket_particles_sequence_v2_get(&json!({
        "schema_version":"FictionalEnergyVfxAnimatedSocketParticlesSequenceGetRequest@2",
        "sequence_key_sha256":request.particle_sequence_key_sha256,
        "project_id":request.project_id,
        "geometry_candidate_id":request.geometry_candidate_id,
        "appearance_candidate_id":request.appearance_candidate_id,
        "geometry_delivery_manifest_object_sha256":request.geometry_delivery_manifest_object_sha256,
        "appearance_delivery_manifest_object_sha256":request.appearance_delivery_manifest_object_sha256
    }))?;
    require_read_only(&particle_value, "Particles@2")?;
    expect(
        "Particles@2 schema",
        particle_value.get("schema_version").and_then(Value::as_str),
        "FictionalEnergyVfxAnimatedSocketParticlesSequenceGetResult@2",
    )?;
    let particles: FictionalEnergyVfxAnimatedSocketParticlesSequenceV2 = serde_json::from_value(
        particle_value
            .get("sequence")
            .cloned()
            .ok_or_else(|| invalid("Particles@2 payload is unavailable"))?,
    )
    .map_err(|error| invalid(format!("Particles@2 payload is malformed: {error}")))?;
    if particles.sequence_key_sha256 != request.particle_sequence_key_sha256
        || particles.canonical_sha256 != request.particle_sequence_canonical_sha256
        || particles.project_id != request.project_id
        || particles.geometry_candidate_id != request.geometry_candidate_id
        || particles.appearance_candidate_id != request.appearance_candidate_id
        || particles.geometry_delivery_manifest_object_sha256
            != request.geometry_delivery_manifest_object_sha256
        || particles.appearance_delivery_manifest_object_sha256
            != request.appearance_delivery_manifest_object_sha256
        || particles.geometry_artifact_sha256 != request.geometry_artifact_sha256
        || particles.appearance_artifact_sha256 != request.appearance_artifact_sha256
        || particles.material_surface_quality_id != request.material_surface_quality_id
        || particles.material_surface_quality_report_object_sha256
            != request.material_surface_quality_report_object_sha256
        || particles.material_surface_quality_canonical_sha256
            != request.material_surface_quality_canonical_sha256
        || particles.geometry_preservation_projection_sha256
            != request.geometry_preservation_projection_sha256
        || particles.geometry_preservation_status != request.geometry_preservation_status
        || particles.projection_key_sha256 != request.projection_key_sha256
        || particles.projection_object_sha256 != request.projection_object_sha256
        || particles.projection_canonical_sha256 != request.projection_canonical_sha256
        || particles.animated_socket_materialization_key_sha256
            != request.animated_socket_materialization_key_sha256
        || particles.animated_artifact_sha256 != request.animated_artifact_sha256
        || particles.animated_socket_anchor_set_object_sha256
            != request.animated_socket_anchor_set_object_sha256
        || particles.animated_socket_anchor_set_canonical_sha256
            != request.animated_socket_anchor_set_canonical_sha256
        || particles.appearance_anchor_set_object_sha256
            != request.appearance_anchor_set_object_sha256
        || particles.appearance_anchor_set_canonical_sha256
            != request.appearance_anchor_set_canonical_sha256
        || particles.anchor_binding_policy != request.anchor_binding_policy
        || particles.animation_clip_id != request.animation_clip_id
        || particles.animation_clip_object_sha256 != request.animation_clip_object_sha256
        || particles.animation_clip_canonical_sha256 != request.animation_clip_canonical_sha256
        || particles.animation_receipt_object_sha256 != request.animation_receipt_object_sha256
        || particles.animation_receipt_canonical_sha256
            != request.animation_receipt_canonical_sha256
        || particles.vfx_profile_object_sha256 != request.vfx_profile_object_sha256
        || particles.vfx_profile_canonical_sha256 != request.vfx_profile_canonical_sha256
        || particles.socket_node_id_encoding_sha256 != request.socket_node_id_encoding_sha256
        || particles.socket_roles_sha256 != request.socket_roles_sha256
        || particles.camera_object_sha256 != request.camera_object_sha256
        || particles.camera_identity_sha256 != request.camera_identity_sha256
        || particles.render_profile_sha256 != request.render_profile_sha256
        || particles.render_worker_build_cohort_sha256 != request.render_worker_build_cohort_sha256
        || particles.sample_schedule_sha256 != request.sample_schedule_sha256
        || particles.sample_count != UPSTREAM_FRAMES as u64
        || particles.sample_time_ticks.len() != UPSTREAM_FRAMES
        || particles.sample_time_ticks[1..] != request.sample_time_ticks
        || particles.frames.iter().enumerate().any(|(index, frame)| {
            frame.frame_index != index as u64
                || frame.sample_time_ticks != particles.sample_time_ticks[index]
        })
        || particles.particles_sequence_policy
            != "projection-v2-driven-animated-socket-particles-dual-candidate@2"
        || particles.transform_projection_policy
            != "glb-animation-linear-nlerp-flat-part-hierarchy-composed-world-trs-matrix@2"
        || particles.quality_status != "structural_only"
        || particles.visual_quality_status != "NOT_PROVEN"
        || particles.commercial_fps_quality_status != "NOT_PROVEN"
    {
        return Err(invalid(
            "Particles@2 dual-candidate or Projection@2 lineage differs",
        ));
    }

    let trails_value = runtime.fictional_energy_vfx_animated_socket_trails_sequence_v2_get(&json!({
        "schema_version":"FictionalEnergyVfxAnimatedSocketTrailsSequenceGetRequest@2",
        "sequence_key_sha256":request.trail_sequence_key_sha256,
        "project_id":request.project_id,
        "geometry_candidate_id":request.geometry_candidate_id,
        "appearance_candidate_id":request.appearance_candidate_id,
        "geometry_delivery_manifest_object_sha256":request.geometry_delivery_manifest_object_sha256,
        "appearance_delivery_manifest_object_sha256":request.appearance_delivery_manifest_object_sha256
    }))?;
    require_read_only(&trails_value, "Trails@2")?;
    expect(
        "Trails@2 schema",
        trails_value.get("schema_version").and_then(Value::as_str),
        "FictionalEnergyVfxAnimatedSocketTrailsSequenceGetResult@2",
    )?;
    let trails: FictionalEnergyVfxAnimatedSocketTrailsSequenceV2 = serde_json::from_value(
        trails_value
            .get("sequence")
            .cloned()
            .ok_or_else(|| invalid("Trails@2 payload is unavailable"))?,
    )
    .map_err(|error| invalid(format!("Trails@2 payload is malformed: {error}")))?;
    if trails.sequence_key_sha256 != request.trail_sequence_key_sha256
        || trails.canonical_sha256 != request.trail_sequence_canonical_sha256
        || trails.project_id != request.project_id
        || trails.geometry_candidate_id != request.geometry_candidate_id
        || trails.appearance_candidate_id != request.appearance_candidate_id
        || trails.geometry_delivery_manifest_object_sha256
            != request.geometry_delivery_manifest_object_sha256
        || trails.appearance_delivery_manifest_object_sha256
            != request.appearance_delivery_manifest_object_sha256
        || trails.geometry_artifact_sha256 != request.geometry_artifact_sha256
        || trails.appearance_artifact_sha256 != request.appearance_artifact_sha256
        || trails.material_surface_quality_id != request.material_surface_quality_id
        || trails.material_surface_quality_report_object_sha256
            != request.material_surface_quality_report_object_sha256
        || trails.material_surface_quality_canonical_sha256
            != request.material_surface_quality_canonical_sha256
        || trails.geometry_preservation_projection_sha256
            != request.geometry_preservation_projection_sha256
        || trails.geometry_preservation_status != request.geometry_preservation_status
        || trails.particle_sequence_key_sha256 != request.particle_sequence_key_sha256
        || trails.particle_sequence_canonical_sha256 != request.particle_sequence_canonical_sha256
        || trails.projection_key_sha256 != request.projection_key_sha256
        || trails.projection_canonical_sha256 != request.projection_canonical_sha256
        || trails.animated_socket_materialization_key_sha256
            != request.animated_socket_materialization_key_sha256
        || trails.animated_artifact_sha256 != request.animated_artifact_sha256
        || trails.animated_socket_anchor_set_object_sha256
            != request.animated_socket_anchor_set_object_sha256
        || trails.animated_socket_anchor_set_canonical_sha256
            != request.animated_socket_anchor_set_canonical_sha256
        || trails.appearance_anchor_set_object_sha256
            != request.appearance_anchor_set_object_sha256
        || trails.appearance_anchor_set_canonical_sha256
            != request.appearance_anchor_set_canonical_sha256
        || trails.anchor_binding_policy != request.anchor_binding_policy
        || trails.animation_clip_id != request.animation_clip_id
        || trails.animation_clip_object_sha256 != request.animation_clip_object_sha256
        || trails.animation_clip_canonical_sha256 != request.animation_clip_canonical_sha256
        || trails.animation_receipt_object_sha256 != request.animation_receipt_object_sha256
        || trails.animation_receipt_canonical_sha256
            != request.animation_receipt_canonical_sha256
        || trails.vfx_profile_object_sha256 != request.vfx_profile_object_sha256
        || trails.vfx_profile_canonical_sha256 != request.vfx_profile_canonical_sha256
        || trails.socket_node_id_encoding_sha256 != request.socket_node_id_encoding_sha256
        || trails.socket_roles_sha256 != request.socket_roles_sha256
        || trails.camera_object_sha256 != request.camera_object_sha256
        || trails.camera_identity_sha256 != request.camera_identity_sha256
        || trails.render_profile_sha256 != request.render_profile_sha256
        || trails.render_worker_build_cohort_sha256 != request.render_worker_build_cohort_sha256
        || trails.sample_schedule_sha256 != request.sample_schedule_sha256
        || trails.sample_count != MAX_FRAMES as u64
        || trails.sample_time_ticks != request.sample_time_ticks
        || trails.frames.len() != MAX_FRAMES
        || trails.trails_sequence_policy
            != "projection-v2-driven-animated-socket-trails-dual-candidate@2"
        || trails.history_policy
            != "particles-v2-history-oldest-to-newest-ordinal-zero-last-immediate-previous-earlier-particle-frames-plus-current@2"
        || trails.history_pre_roll_policy
            != "same-parent-particles-v2-frame-zero-is-preroll-output-frames-one-to-fifteen@2"
        || trails.quality_status != "structural_only"
        || trails.visual_quality_status != "NOT_PROVEN"
        || trails.commercial_fps_quality_status != "NOT_PROVEN"
    {
        return Err(invalid(
            "Trails@2 dual-candidate or Particles@2 lineage differs",
        ));
    }

    let bloom_value = runtime.fictional_energy_vfx_animated_socket_trails_bloom_sequence_v2_get(&json!({
        "schema_version":"FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceGetRequest@2",
        "sequence_key_sha256":request.trail_bloom_sequence_key_sha256,
        "project_id":request.project_id,
        "geometry_candidate_id":request.geometry_candidate_id,
        "appearance_candidate_id":request.appearance_candidate_id,
        "geometry_delivery_manifest_object_sha256":request.geometry_delivery_manifest_object_sha256,
        "appearance_delivery_manifest_object_sha256":request.appearance_delivery_manifest_object_sha256
    }))?;
    require_read_only(&bloom_value, "TrailsBloom@2")?;
    expect(
        "TrailsBloom@2 schema",
        bloom_value.get("schema_version").and_then(Value::as_str),
        "FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceGetResult@2",
    )?;
    let bloom: FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceV2 = serde_json::from_value(
        bloom_value
            .get("sequence")
            .cloned()
            .ok_or_else(|| invalid("TrailsBloom@2 payload is unavailable"))?,
    )
    .map_err(|error| invalid(format!("TrailsBloom@2 payload is malformed: {error}")))?;
    if bloom.sequence_key_sha256 != request.trail_bloom_sequence_key_sha256
        || bloom.canonical_sha256 != request.trail_bloom_sequence_canonical_sha256
        || bloom.project_id != request.project_id
        || bloom.geometry_candidate_id != request.geometry_candidate_id
        || bloom.appearance_candidate_id != request.appearance_candidate_id
        || bloom.geometry_delivery_manifest_object_sha256
            != request.geometry_delivery_manifest_object_sha256
        || bloom.appearance_delivery_manifest_object_sha256
            != request.appearance_delivery_manifest_object_sha256
        || bloom.geometry_artifact_sha256 != request.geometry_artifact_sha256
        || bloom.appearance_artifact_sha256 != request.appearance_artifact_sha256
        || bloom.material_surface_quality_id != request.material_surface_quality_id
        || bloom.material_surface_quality_report_object_sha256
            != request.material_surface_quality_report_object_sha256
        || bloom.material_surface_quality_canonical_sha256
            != request.material_surface_quality_canonical_sha256
        || bloom.geometry_preservation_projection_sha256
            != request.geometry_preservation_projection_sha256
        || bloom.geometry_preservation_status != request.geometry_preservation_status
        || bloom.projection_key_sha256 != request.projection_key_sha256
        || bloom.projection_canonical_sha256 != request.projection_canonical_sha256
        || bloom.particle_sequence_key_sha256 != request.particle_sequence_key_sha256
        || bloom.particle_sequence_canonical_sha256 != request.particle_sequence_canonical_sha256
        || bloom.trail_sequence_key_sha256 != request.trail_sequence_key_sha256
        || bloom.trail_sequence_canonical_sha256 != request.trail_sequence_canonical_sha256
        || bloom.animated_socket_materialization_key_sha256
            != request.animated_socket_materialization_key_sha256
        || bloom.animated_artifact_sha256 != request.animated_artifact_sha256
        || bloom.animated_socket_anchor_set_object_sha256
            != request.animated_socket_anchor_set_object_sha256
        || bloom.animated_socket_anchor_set_canonical_sha256
            != request.animated_socket_anchor_set_canonical_sha256
        || bloom.appearance_anchor_set_object_sha256
            != request.appearance_anchor_set_object_sha256
        || bloom.appearance_anchor_set_canonical_sha256
            != request.appearance_anchor_set_canonical_sha256
        || bloom.anchor_binding_policy != request.anchor_binding_policy
        || bloom.animation_clip_id != request.animation_clip_id
        || bloom.animation_clip_object_sha256 != request.animation_clip_object_sha256
        || bloom.animation_clip_canonical_sha256 != request.animation_clip_canonical_sha256
        || bloom.animation_receipt_object_sha256 != request.animation_receipt_object_sha256
        || bloom.animation_receipt_canonical_sha256
            != request.animation_receipt_canonical_sha256
        || bloom.vfx_profile_object_sha256 != request.vfx_profile_object_sha256
        || bloom.vfx_profile_canonical_sha256 != request.vfx_profile_canonical_sha256
        || bloom.socket_node_id_encoding_sha256 != request.socket_node_id_encoding_sha256
        || bloom.socket_roles_sha256 != request.socket_roles_sha256
        || bloom.camera_object_sha256 != request.camera_object_sha256
        || bloom.camera_identity_sha256 != request.camera_identity_sha256
        || bloom.render_profile_sha256 != request.render_profile_sha256
        || bloom.render_worker_build_cohort_sha256 != request.render_worker_build_cohort_sha256
        || bloom.sample_schedule_sha256 != request.sample_schedule_sha256
        || bloom.sample_count != MAX_FRAMES as u64
        || bloom.sample_time_ticks != request.sample_time_ticks
        || bloom.frames.len() != MAX_FRAMES
        || bloom.trails_bloom_sequence_policy
            != "projection-v2-driven-animated-socket-trails-bloom-dual-candidate@2"
        || bloom.history_policy
            != "particles-v2-history-oldest-to-newest-ordinal-zero-last-immediate-previous-earlier-particle-frames-plus-current@2"
        || bloom.history_pre_roll_policy
            != "same-parent-particles-v2-frame-zero-is-preroll-output-frames-one-to-fifteen@2"
        || bloom.trail_key_scope != "animated-socket-trails-sequence-v2-frame-binding@2"
        || bloom.trail_bloom_profile_sha256 != request.trail_bloom_profile_sha256
        || bloom.quality_status != "structural_only"
        || bloom.visual_quality_status != "NOT_PROVEN"
        || bloom.commercial_fps_quality_status != "NOT_PROVEN"
    {
        return Err(invalid(
            "TrailsBloom@2 dual-candidate or Trails@2 lineage differs",
        ));
    }

    validate_source_output_schedule(
        &projection
            .frames
            .iter()
            .map(|frame| frame.frame_index)
            .collect::<Vec<_>>(),
        &projection
            .frames
            .iter()
            .map(|frame| frame.sample_time_ticks)
            .collect::<Vec<_>>(),
        &particles
            .frames
            .iter()
            .map(|frame| frame.frame_index)
            .collect::<Vec<_>>(),
        &particles
            .frames
            .iter()
            .map(|frame| frame.sample_time_ticks)
            .collect::<Vec<_>>(),
        &trails
            .frames
            .iter()
            .map(|frame| frame.frame_index)
            .collect::<Vec<_>>(),
        &trails
            .frames
            .iter()
            .map(|frame| frame.sample_time_ticks)
            .collect::<Vec<_>>(),
        &bloom
            .frames
            .iter()
            .map(|frame| frame.frame_index)
            .collect::<Vec<_>>(),
        &bloom
            .frames
            .iter()
            .map(|frame| frame.sample_time_ticks)
            .collect::<Vec<_>>(),
        &request.sample_time_ticks,
    )?;

    // Recheck the independent six-node projection readback and all frame
    // mappings.  Upstream getters already replay their Worker inputs, but the
    // terminal bridge must never infer an ordinal from an array position.
    for index in 0..MAX_FRAMES {
        let p = projection
            .frames
            .get(index + 1)
            .ok_or_else(|| invalid("Projection@2 frame is missing"))?;
        let particle = particles
            .frames
            .get(index + 1)
            .ok_or_else(|| invalid("Particles@2 current frame is missing"))?;
        let trail = trails
            .frames
            .get(index)
            .ok_or_else(|| invalid("Trails@2 frame is missing"))?;
        let bloom_frame = bloom
            .frames
            .get(index)
            .ok_or_else(|| invalid("TrailsBloom@2 frame is missing"))?;
        if p.frame_index != index as u64 + 1
            || particle.frame_index != index as u64 + 1
            || trail.frame_index != index as u64
            || bloom_frame.frame_index != index as u64
            || p.sample_time_ticks != request.sample_time_ticks[index]
            || particle.sample_time_ticks != p.sample_time_ticks
            || trail.sample_time_ticks != p.sample_time_ticks
            || bloom_frame.sample_time_ticks != p.sample_time_ticks
            || trail.current_projection_frame_index != p.frame_index
            || trail.current_particle_frame_index != particle.frame_index
            || bloom_frame.current_projection_frame_index != p.frame_index
            || bloom_frame.current_particle_frame_index != particle.frame_index
            || trail.current_projection_frame_canonical_sha256
                != p.projection_frame_canonical_sha256
            || bloom_frame.current_projection_frame_canonical_sha256
                != p.projection_frame_canonical_sha256
            || bloom_frame.trail_frame_canonical_sha256 != trail.canonical_sha256
        {
            return Err(invalid(format!("frame {index} V2 source mapping differs")));
        }
    }
    Ok((projection, particles, trails, bloom))
}

fn validate_source_output_schedule(
    projection_frame_indices: &[u64],
    projection_ticks: &[u64],
    particle_frame_indices: &[u64],
    particle_ticks: &[u64],
    trail_frame_indices: &[u64],
    trail_ticks: &[u64],
    bloom_frame_indices: &[u64],
    bloom_ticks: &[u64],
    request_ticks: &[u64],
) -> Result<(), RuntimeError> {
    if projection_frame_indices.len() != UPSTREAM_FRAMES
        || particle_frame_indices.len() != UPSTREAM_FRAMES
        || trail_frame_indices.len() != MAX_FRAMES
        || bloom_frame_indices.len() != MAX_FRAMES
        || projection_ticks.len() != UPSTREAM_FRAMES
        || particle_ticks.len() != UPSTREAM_FRAMES
        || trail_ticks.len() != MAX_FRAMES
        || bloom_ticks.len() != MAX_FRAMES
        || request_ticks.len() != MAX_FRAMES
        || projection_ticks[1..] != request_ticks[..]
        || particle_ticks[1..] != request_ticks[..]
        || trail_ticks != request_ticks
        || bloom_ticks != request_ticks
        || projection_frame_indices
            .iter()
            .enumerate()
            .any(|(index, frame)| *frame != index as u64)
        || particle_frame_indices
            .iter()
            .enumerate()
            .any(|(index, frame)| *frame != index as u64)
        || trail_frame_indices
            .iter()
            .enumerate()
            .any(|(index, frame)| *frame != index as u64)
        || bloom_frame_indices
            .iter()
            .enumerate()
            .any(|(index, frame)| *frame != index as u64)
    {
        return Err(invalid(
            "Attachment@3 requires Projection/Particles 0..15 and Trails/TrailsBloom 0..14",
        ));
    }
    Ok(())
}

fn frame_records(
    request: &FictionalEnergyVfxAnimatedSocketAttachmentV3PrepareRequest,
    bloom: &FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceV2,
    created_at: &str,
) -> Result<Vec<FictionalEnergyVfxAnimatedSocketAttachmentV3FrameRecord>, RuntimeError> {
    let mut frames = Vec::with_capacity(MAX_FRAMES);
    for (index, source) in bloom.frames.iter().enumerate() {
        if index >= MAX_FRAMES || source.frame_index != index as u64 {
            return Err(invalid("TrailsBloom@2 frame ordinals are not contiguous"));
        }
        let mut frame = FictionalEnergyVfxAnimatedSocketAttachmentV3FrameRecord {
            schema_version: FRAME_SCHEMA.to_owned(),
            attachment_key_sha256: request.attachment_key_sha256.clone(),
            frame_index: source.frame_index,
            sample_time_ticks: source.sample_time_ticks,
            projection_frame_index: source.current_projection_frame_index,
            particle_sequence_frame_index: source.current_particle_frame_index,
            trail_frame_index: source.trail_frame_index,
            trail_bloom_frame_index: source.frame_index,
            projection_frame_canonical_sha256: source
                .current_projection_frame_canonical_sha256
                .clone(),
            projection_socket_transform_inventory_sha256: source
                .current_projection_socket_transform_inventory_sha256
                .clone(),
            projection_socket_transform_readback_sha256: source
                .current_projection_socket_transform_readback_sha256
                .clone(),
            particle_sequence_key_sha256: source.particle_sequence_key_sha256.clone(),
            particle_sequence_frame_canonical_sha256: source
                .particle_sequence_frame_canonical_sha256
                .clone(),
            trail_sequence_key_sha256: source.trail_sequence_key_sha256.clone(),
            trail_sequence_frame_canonical_sha256: source.trail_frame_canonical_sha256.clone(),
            trail_key_sha256: source.trail_key_sha256.clone(),
            trail_inventory_sha256: source.trail_inventory_sha256.clone(),
            trail_id_encoding_sha256: source.trail_id_encoding_sha256.clone(),
            emitter_binding_sha256: source.emitter_binding_sha256.clone(),
            trail_bloom_sequence_key_sha256: request.trail_bloom_sequence_key_sha256.clone(),
            trail_bloom_sequence_frame_canonical_sha256: source.canonical_sha256.clone(),
            trail_bloom_key_sha256: source.trail_bloom_key_sha256.clone(),
            trail_bloom_seed_sha256: source.trail_bloom_seed_sha256.clone(),
            base_frame_key_sha256: source.base_frame_key_sha256.clone(),
            bloom_key_sha256: source.bloom_key_sha256.clone(),
            camera_object_sha256: source.camera_object_sha256.clone(),
            camera_identity_sha256: source.camera_identity_sha256.clone(),
            render_profile_sha256: source.render_profile_sha256.clone(),
            render_worker_build_cohort_sha256: source.render_worker_build_cohort_sha256.clone(),
            canonical_sha256: String::new(),
            created_at: created_at.to_owned(),
        };
        frame.canonical_sha256 = canonical_json_hash(
            &serde_json::to_value(&frame).map_err(|error| invalid(error.to_string()))?,
        );
        frames.push(frame);
    }
    if frames.len() != MAX_FRAMES {
        return Err(invalid("Attachment@3 requires exactly 15 frames"));
    }
    Ok(frames)
}

fn build_record(
    request: &FictionalEnergyVfxAnimatedSocketAttachmentV3PrepareRequest,
    trails: &FictionalEnergyVfxAnimatedSocketTrailsSequenceV2,
    bloom: &FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceV2,
) -> Result<FictionalEnergyVfxAnimatedSocketAttachmentV3Record, RuntimeError> {
    let created_at = now_string();
    let frames = frame_records(request, bloom, &created_at)?;
    let record = FictionalEnergyVfxAnimatedSocketAttachmentV3Record {
        schema_version: RECORD_SCHEMA.to_owned(),
        attachment_key_sha256: request.attachment_key_sha256.clone(),
        project_id: request.project_id.clone(),
        geometry_candidate_id: request.geometry_candidate_id.clone(),
        geometry_candidate_state_sha256: request.geometry_candidate_state_sha256.clone(),
        geometry_delivery_manifest_object_sha256: request
            .geometry_delivery_manifest_object_sha256
            .clone(),
        geometry_artifact_sha256: request.geometry_artifact_sha256.clone(),
        appearance_candidate_id: request.appearance_candidate_id.clone(),
        appearance_candidate_state_sha256: request.appearance_candidate_state_sha256.clone(),
        appearance_delivery_manifest_object_sha256: request
            .appearance_delivery_manifest_object_sha256
            .clone(),
        appearance_artifact_sha256: request.appearance_artifact_sha256.clone(),
        material_surface_quality_id: request.material_surface_quality_id.clone(),
        material_surface_quality_report_object_sha256: request
            .material_surface_quality_report_object_sha256
            .clone(),
        material_surface_quality_canonical_sha256: request
            .material_surface_quality_canonical_sha256
            .clone(),
        geometry_preservation_projection_sha256: request
            .geometry_preservation_projection_sha256
            .clone(),
        geometry_preservation_status: request.geometry_preservation_status.clone(),
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
        appearance_anchor_set_object_sha256: request.appearance_anchor_set_object_sha256.clone(),
        appearance_anchor_set_canonical_sha256: request
            .appearance_anchor_set_canonical_sha256
            .clone(),
        anchor_binding_policy: request.anchor_binding_policy.clone(),
        anchor_binding_sha256: trails.anchor_binding_sha256.clone(),
        animation_clip_id: request.animation_clip_id.clone(),
        animation_clip_object_sha256: request.animation_clip_object_sha256.clone(),
        animation_clip_canonical_sha256: request.animation_clip_canonical_sha256.clone(),
        animation_receipt_object_sha256: request.animation_receipt_object_sha256.clone(),
        animation_receipt_canonical_sha256: request.animation_receipt_canonical_sha256.clone(),
        projection_key_sha256: request.projection_key_sha256.clone(),
        projection_object_sha256: request.projection_object_sha256.clone(),
        projection_canonical_sha256: request.projection_canonical_sha256.clone(),
        particle_sequence_key_sha256: request.particle_sequence_key_sha256.clone(),
        particle_sequence_canonical_sha256: request.particle_sequence_canonical_sha256.clone(),
        trail_sequence_key_sha256: request.trail_sequence_key_sha256.clone(),
        trail_sequence_canonical_sha256: request.trail_sequence_canonical_sha256.clone(),
        trail_bloom_sequence_key_sha256: request.trail_bloom_sequence_key_sha256.clone(),
        trail_bloom_sequence_canonical_sha256: request
            .trail_bloom_sequence_canonical_sha256
            .clone(),
        vfx_profile_object_sha256: request.vfx_profile_object_sha256.clone(),
        vfx_profile_canonical_sha256: request.vfx_profile_canonical_sha256.clone(),
        trail_bloom_profile_sha256: request.trail_bloom_profile_sha256.clone(),
        socket_node_id_encoding_sha256: request.socket_node_id_encoding_sha256.clone(),
        socket_roles_sha256: request.socket_roles_sha256.clone(),
        camera_object_sha256: request.camera_object_sha256.clone(),
        camera_identity_sha256: request.camera_identity_sha256.clone(),
        render_profile_sha256: request.render_profile_sha256.clone(),
        render_worker_build_cohort_sha256: request.render_worker_build_cohort_sha256.clone(),
        sample_schedule_sha256: request.sample_schedule_sha256.clone(),
        sample_count: request.sample_count,
        sample_time_ticks: request.sample_time_ticks.clone(),
        attachment_policy: request.attachment_policy.clone(),
        frame_scope: request.frame_scope.clone(),
        attachment_receipt_object_sha256: String::new(),
        attachment_receipt_canonical_sha256: String::new(),
        frames,
        attachment_status:
            forgecad_contracts::FICTIONAL_ENERGY_VFX_ANIMATED_SOCKET_ATTACHMENT_V3_STATUS.to_owned(),
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
        created_at,
    };
    Ok(record)
}

fn receipt_projection(record: &FictionalEnergyVfxAnimatedSocketAttachmentV3Record) -> Value {
    let mut value = serde_json::to_value(record).expect("Attachment@3 serialization is infallible");
    if let Some(object) = value.as_object_mut() {
        object.remove("attachment_receipt_object_sha256");
        object.remove("attachment_receipt_canonical_sha256");
        object.remove("canonical_sha256");
        object.insert(
            "schema_version".to_owned(),
            Value::String(RECEIPT_SCHEMA.to_owned()),
        );
        object.insert("canonical_sha256".to_owned(), Value::String(String::new()));
    }
    value
}

fn finish_record_and_receipt(
    mut record: FictionalEnergyVfxAnimatedSocketAttachmentV3Record,
) -> Result<(FictionalEnergyVfxAnimatedSocketAttachmentV3Record, Vec<u8>), RuntimeError> {
    let mut projection = receipt_projection(&record);
    let receipt_canonical = canonical_json_hash(&projection);
    record.attachment_receipt_canonical_sha256 = receipt_canonical.clone();
    record.canonical_sha256 = canonical_json_hash(
        &serde_json::to_value(&record).map_err(|error| invalid(error.to_string()))?,
    );
    let frames =
        serde_json::to_value(&record.frames).map_err(|error| invalid(error.to_string()))?;
    projection["canonical_sha256"] = Value::String(record.canonical_sha256.clone());
    projection["frame_count"] = Value::from(record.frames.len() as u64);
    projection["frame_dependencies"] = frames;
    projection["attachment_key_sha256"] = Value::String(record.attachment_key_sha256.clone());
    projection["attachment_canonical_sha256"] = Value::String(record.canonical_sha256.clone());
    projection["receipt_canonical_sha256"] = Value::String(receipt_canonical.clone());
    let bytes = canonical_json_bytes(&projection).map_err(|error| invalid(error.to_string()))?;
    if bytes.is_empty() || bytes.len() > MAX_RECEIPT_BYTES {
        return Err(invalid("Attachment@3 receipt exceeds one MiB"));
    }
    Ok((record, bytes))
}

fn validate_receipt(
    runtime: &Runtime,
    receipt_hash: &str,
    record: &FictionalEnergyVfxAnimatedSocketAttachmentV3Record,
) -> Result<(), RuntimeError> {
    let bytes = runtime.cas_read_bounded(receipt_hash, MAX_RECEIPT_BYTES as u64)?;
    if bytes.is_empty() || sha256_hex(&bytes) != receipt_hash {
        return Err(invalid("Attachment@3 receipt bytes are tampered"));
    }
    let value: Value = serde_json::from_slice(&bytes)
        .map_err(|error| invalid(format!("Attachment@3 receipt is malformed: {error}")))?;
    if canonical_json_bytes(&value).map_err(|error| invalid(error.to_string()))? != bytes {
        return Err(invalid("Attachment@3 receipt is not canonical JSON"));
    }
    expect(
        "Attachment@3 receipt schema",
        value.get("schema_version").and_then(Value::as_str),
        RECEIPT_SCHEMA,
    )?;
    expect(
        "Attachment@3 receipt key",
        value.get("attachment_key_sha256").and_then(Value::as_str),
        &record.attachment_key_sha256,
    )?;
    expect(
        "Attachment@3 receipt parent canonical",
        value.get("canonical_sha256").and_then(Value::as_str),
        &record.canonical_sha256,
    )?;
    expect(
        "Attachment@3 receipt parent canonical alias",
        value
            .get("attachment_canonical_sha256")
            .and_then(Value::as_str),
        &record.canonical_sha256,
    )?;
    expect(
        "Attachment@3 receipt canonical",
        value
            .get("receipt_canonical_sha256")
            .and_then(Value::as_str),
        &record.attachment_receipt_canonical_sha256,
    )?;
    // `attachment_receipt_canonical_sha256` is deliberately computed from a
    // projection that excludes both the receipt object hash and the record's
    // own canonical hash.  Including either would create a self-referential
    // digest once the reserved CAS object is attached.
    let expected = canonical_json_hash(&receipt_projection(record));
    if expected != record.attachment_receipt_canonical_sha256 {
        return Err(invalid("Attachment@3 receipt canonical projection differs"));
    }
    let mut receipt_record = record.clone();
    receipt_record.attachment_receipt_object_sha256.clear();
    receipt_record.canonical_sha256.clear();
    let (recomputed, expected_bytes) = finish_record_and_receipt(receipt_record)?;
    if recomputed.canonical_sha256 != record.canonical_sha256
        || recomputed.attachment_receipt_canonical_sha256
            != record.attachment_receipt_canonical_sha256
        || expected_bytes != bytes
    {
        return Err(invalid(
            "Attachment@3 receipt bytes differ from the canonical durable projection",
        ));
    }
    let expected_frames =
        serde_json::to_value(&record.frames).map_err(|error| invalid(error.to_string()))?;
    if value.get("frame_count").and_then(Value::as_u64) != Some(15)
        || value.get("frames") != Some(&expected_frames)
        || value.get("frame_dependencies") != Some(&expected_frames)
    {
        return Err(invalid(
            "Attachment@3 receipt frame projection differs from the durable rows",
        ));
    }
    Ok(())
}

fn normalize_for_compare(value: &mut Value) {
    if let Some(object) = value.as_object_mut() {
        object.insert("canonical_sha256".to_owned(), Value::String(String::new()));
        object.insert("created_at".to_owned(), Value::String(String::new()));
        object.insert(
            "attachment_receipt_object_sha256".to_owned(),
            Value::String(String::new()),
        );
        if let Some(frames) = object.get_mut("frames").and_then(Value::as_array_mut) {
            for frame in frames {
                if let Some(frame) = frame.as_object_mut() {
                    frame.insert("canonical_sha256".to_owned(), Value::String(String::new()));
                    frame.insert("created_at".to_owned(), Value::String(String::new()));
                }
            }
        }
    }
}

fn replay_equivalent(
    left: &FictionalEnergyVfxAnimatedSocketAttachmentV3Record,
    right: &FictionalEnergyVfxAnimatedSocketAttachmentV3Record,
) -> Result<bool, RuntimeError> {
    let mut left = serde_json::to_value(left).map_err(|error| invalid(error.to_string()))?;
    let mut right = serde_json::to_value(right).map_err(|error| invalid(error.to_string()))?;
    normalize_for_compare(&mut left);
    normalize_for_compare(&mut right);
    Ok(left == right)
}

fn result_value(
    record: &FictionalEnergyVfxAnimatedSocketAttachmentV3Record,
    replayed: bool,
    schema: &str,
    runtime_write: bool,
) -> Value {
    json!({
        "schema_version":schema,
        "attachment_key_sha256":record.attachment_key_sha256,
        "attachment":record,
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
    })
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

pub(super) fn prepare(runtime: &Runtime, value: &Value) -> Result<Value, RuntimeError> {
    let request = parse_prepare(value)?;
    let (_projection, _particles, trails, bloom) = validate_dependencies(runtime, &request)?;
    let (mut record, receipt_bytes) =
        finish_record_and_receipt(build_record(&request, &trails, &bloom)?)?;
    if let Some((existing, receipt_hash)) = runtime
        .store
        .get_fictional_energy_vfx_animated_socket_attachment_v3_link(
            &request.attachment_key_sha256,
        )?
    {
        validate_receipt(runtime, &receipt_hash, &existing)?;
        if replay_equivalent(&existing, &record)? {
            // PrepareResult@3 deliberately exposes the producer contract's
            // `runtime_write=true` flag even for an idempotent replay.  The
            // `replayed` bit and the durable record's stable identity explain
            // that no new CAS/SQLite row was created on this invocation.
            return Ok(result_value(&existing, true, PREPARE_RESULT_SCHEMA, true));
        }
        return Err(invalid(
            "Attachment@3 key is already bound to different content",
        ));
    }
    let reservation = runtime.store.begin_cas_reservation();
    let receipt_object = match runtime.store.put_object_reserved(
        &reservation,
        &receipt_bytes,
        None,
        RECEIPT_MIME,
        RECEIPT_KIND,
        &record.created_at,
    ) {
        Ok(object) => object,
        Err(error) => return Err(error.into()),
    };
    record.attachment_receipt_object_sha256 = receipt_object.record.sha256.clone();
    // The receipt object hash is an owned-CAS reachability edge, not part of
    // the parent canonical binding digest.  Keeping it out of the digest
    // avoids a receipt/parent self-reference and is normalized by Store's V3
    // canonical validator.
    // Receipt validation is intentionally after the object is reserved and
    // before SQLite commit; it still leaves no durable state on failure.
    if let Err(error) = validate_receipt(runtime, &receipt_object.record.sha256, &record) {
        release_receipt(runtime, &reservation, &receipt_object, true);
        return Err(error);
    }
    match runtime
        .store
        .record_fictional_energy_vfx_animated_socket_attachment_v3_link(
            &record,
            &receipt_object.record,
        ) {
        Ok(stored) => {
            release_receipt(runtime, &reservation, &receipt_object, false);
            Ok(result_value(&stored, false, PREPARE_RESULT_SCHEMA, true))
        }
        Err(error) => {
            release_receipt(runtime, &reservation, &receipt_object, true);
            Err(error.into())
        }
    }
}

pub(super) fn get(runtime: &Runtime, value: &Value) -> Result<Value, RuntimeError> {
    let request = parse_get(value)?;
    let (record, receipt_hash) = runtime
        .store
        .get_fictional_energy_vfx_animated_socket_attachment_v3_link(
            &request.attachment_key_sha256,
        )?
        .ok_or_else(|| invalid("durable Attachment@3 is unavailable"))?;
    if record.project_id != request.project_id
        || record.geometry_candidate_id != request.geometry_candidate_id
        || record.appearance_candidate_id != request.appearance_candidate_id
        || record.geometry_delivery_manifest_object_sha256
            != request.geometry_delivery_manifest_object_sha256
        || record.appearance_delivery_manifest_object_sha256
            != request.appearance_delivery_manifest_object_sha256
    {
        return Err(invalid("Attachment@3 get scope differs"));
    }
    let replay_request = FictionalEnergyVfxAnimatedSocketAttachmentV3PrepareRequest {
        schema_version: PREPARE_SCHEMA.to_owned(),
        attachment_key_sha256: record.attachment_key_sha256.clone(),
        project_id: record.project_id.clone(),
        geometry_candidate_id: record.geometry_candidate_id.clone(),
        geometry_candidate_state_sha256: record.geometry_candidate_state_sha256.clone(),
        geometry_delivery_manifest_object_sha256: record
            .geometry_delivery_manifest_object_sha256
            .clone(),
        geometry_artifact_sha256: record.geometry_artifact_sha256.clone(),
        appearance_candidate_id: record.appearance_candidate_id.clone(),
        appearance_candidate_state_sha256: record.appearance_candidate_state_sha256.clone(),
        appearance_delivery_manifest_object_sha256: record
            .appearance_delivery_manifest_object_sha256
            .clone(),
        appearance_artifact_sha256: record.appearance_artifact_sha256.clone(),
        material_surface_quality_id: record.material_surface_quality_id.clone(),
        material_surface_quality_report_object_sha256: record
            .material_surface_quality_report_object_sha256
            .clone(),
        material_surface_quality_canonical_sha256: record
            .material_surface_quality_canonical_sha256
            .clone(),
        geometry_preservation_projection_sha256: record
            .geometry_preservation_projection_sha256
            .clone(),
        geometry_preservation_status: record.geometry_preservation_status.clone(),
        animated_socket_materialization_key_sha256: record
            .animated_socket_materialization_key_sha256
            .clone(),
        animated_artifact_sha256: record.animated_artifact_sha256.clone(),
        animated_socket_anchor_set_object_sha256: record
            .animated_socket_anchor_set_object_sha256
            .clone(),
        animated_socket_anchor_set_canonical_sha256: record
            .animated_socket_anchor_set_canonical_sha256
            .clone(),
        appearance_anchor_set_object_sha256: record.appearance_anchor_set_object_sha256.clone(),
        appearance_anchor_set_canonical_sha256: record
            .appearance_anchor_set_canonical_sha256
            .clone(),
        anchor_binding_policy: record.anchor_binding_policy.clone(),
        animation_clip_id: record.animation_clip_id.clone(),
        animation_clip_object_sha256: record.animation_clip_object_sha256.clone(),
        animation_clip_canonical_sha256: record.animation_clip_canonical_sha256.clone(),
        animation_receipt_object_sha256: record.animation_receipt_object_sha256.clone(),
        animation_receipt_canonical_sha256: record.animation_receipt_canonical_sha256.clone(),
        projection_key_sha256: record.projection_key_sha256.clone(),
        projection_object_sha256: record.projection_object_sha256.clone(),
        projection_canonical_sha256: record.projection_canonical_sha256.clone(),
        particle_sequence_key_sha256: record.particle_sequence_key_sha256.clone(),
        particle_sequence_canonical_sha256: record.particle_sequence_canonical_sha256.clone(),
        trail_sequence_key_sha256: record.trail_sequence_key_sha256.clone(),
        trail_sequence_canonical_sha256: record.trail_sequence_canonical_sha256.clone(),
        trail_bloom_sequence_key_sha256: record.trail_bloom_sequence_key_sha256.clone(),
        trail_bloom_sequence_canonical_sha256: record.trail_bloom_sequence_canonical_sha256.clone(),
        vfx_profile_object_sha256: record.vfx_profile_object_sha256.clone(),
        vfx_profile_canonical_sha256: record.vfx_profile_canonical_sha256.clone(),
        trail_bloom_profile_sha256: record.trail_bloom_profile_sha256.clone(),
        socket_node_id_encoding_sha256: record.socket_node_id_encoding_sha256.clone(),
        socket_roles_sha256: record.socket_roles_sha256.clone(),
        camera_object_sha256: record.camera_object_sha256.clone(),
        camera_identity_sha256: record.camera_identity_sha256.clone(),
        render_profile_sha256: record.render_profile_sha256.clone(),
        render_worker_build_cohort_sha256: record.render_worker_build_cohort_sha256.clone(),
        sample_schedule_sha256: record.sample_schedule_sha256.clone(),
        sample_count: record.sample_count,
        sample_time_ticks: record.sample_time_ticks.clone(),
        attachment_policy: record.attachment_policy.clone(),
        frame_scope: record.frame_scope.clone(),
        input_sha256: record.input_sha256.clone(),
        idempotency_key: record.attachment_key_sha256.clone(),
    };
    let (_projection, _particles, trails, bloom) = validate_dependencies(runtime, &replay_request)?;
    let (recomputed, _receipt_bytes) =
        finish_record_and_receipt(build_record(&replay_request, &trails, &bloom)?)?;
    if !replay_equivalent(&recomputed, &record)? {
        return Err(invalid(
            "durable Attachment@3 differs from dependency replay",
        ));
    }
    validate_receipt(runtime, &receipt_hash, &record)?;
    Ok(result_value(&record, true, GET_RESULT_SCHEMA, false))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(ch: char) -> String {
        std::iter::repeat(ch).take(64).collect()
    }

    fn valid_prepare_value() -> Value {
        let mut object = Map::new();
        for field in PREPARE_FIELDS {
            let value = match *field {
                "schema_version" => Value::String(PREPARE_SCHEMA.to_owned()),
                "project_id" => Value::String("project-1".to_owned()),
                "geometry_candidate_id" => Value::String("geometry-candidate".to_owned()),
                "appearance_candidate_id" => Value::String("appearance-candidate".to_owned()),
                "material_surface_quality_id" => Value::String("material-quality".to_owned()),
                "animation_clip_id" => Value::String("clip-1".to_owned()),
                "idempotency_key" => Value::String("idempotency-1".to_owned()),
                "sample_count" => Value::from(MAX_FRAMES as u64),
                "sample_time_ticks" =>
                    Value::Array((1..=MAX_FRAMES as u64).map(Value::from).collect()),
                "attachment_policy" => Value::String(
                    forgecad_contracts::FICTIONAL_ENERGY_VFX_ANIMATED_SOCKET_ATTACHMENT_V3_POLICY
                        .to_owned(),
                ),
                "frame_scope" => Value::String(
                    forgecad_contracts::FICTIONAL_ENERGY_VFX_ANIMATED_SOCKET_ATTACHMENT_V3_FRAME_SCOPE
                        .to_owned(),
                ),
                "geometry_preservation_status" => {
                    Value::String("source-output-renderable-geometry-byte-exact".to_owned())
                },
                "anchor_binding_policy" => {
                    Value::String("geometry-appearance-anchor-role-owner-trs-equivalent@1".to_owned())
                },
                "geometry_artifact_sha256" => Value::String(digest('b')),
                "appearance_artifact_sha256" => Value::String(digest('c')),
                _ => Value::String(digest('a')),
            };
            object.insert((*field).to_owned(), value);
        }
        let mut preimage = object.clone();
        preimage.remove("attachment_key_sha256");
        preimage.remove("input_sha256");
        preimage.remove("idempotency_key");
        let key = canonical_json_hash(&Value::Object(preimage));
        object.insert(
            "attachment_key_sha256".to_owned(),
            Value::String(key.clone()),
        );
        object.insert("input_sha256".to_owned(), Value::String(key));
        Value::Object(object)
    }

    fn test_frame(
        attachment_key: &str,
        index: u64,
    ) -> FictionalEnergyVfxAnimatedSocketAttachmentV3FrameRecord {
        let mut value = json!({
            "schema_version": FRAME_SCHEMA,
            "attachment_key_sha256": attachment_key,
            "frame_index": index,
            "sample_time_ticks": index + 1,
            "projection_frame_index": index + 1,
            "particle_sequence_frame_index": index + 1,
            "trail_frame_index": index,
            "trail_bloom_frame_index": index,
            "projection_frame_canonical_sha256": digest('a'),
            "projection_socket_transform_inventory_sha256": digest('a'),
            "projection_socket_transform_readback_sha256": digest('a'),
            "particle_sequence_key_sha256": digest('a'),
            "particle_sequence_frame_canonical_sha256": digest('a'),
            "trail_sequence_key_sha256": digest('a'),
            "trail_sequence_frame_canonical_sha256": digest('a'),
            "trail_key_sha256": digest('a'),
            "trail_inventory_sha256": digest('a'),
            "trail_id_encoding_sha256": digest('a'),
            "emitter_binding_sha256": digest('a'),
            "trail_bloom_sequence_key_sha256": digest('a'),
            "trail_bloom_sequence_frame_canonical_sha256": digest('a'),
            "trail_bloom_key_sha256": digest('a'),
            "trail_bloom_seed_sha256": digest('a'),
            "base_frame_key_sha256": digest('a'),
            "bloom_key_sha256": digest('a'),
            "camera_object_sha256": digest('a'),
            "camera_identity_sha256": digest('a'),
            "render_profile_sha256": digest('a'),
            "render_worker_build_cohort_sha256": digest('a'),
            "canonical_sha256": "",
            "created_at": "2026-01-01T00:00:00Z"
        });
        let canonical = canonical_json_hash(&value);
        value["canonical_sha256"] = Value::String(canonical);
        serde_json::from_value(value).expect("test Attachment@3 frame is valid")
    }

    fn test_record() -> FictionalEnergyVfxAnimatedSocketAttachmentV3Record {
        let request: FictionalEnergyVfxAnimatedSocketAttachmentV3PrepareRequest =
            serde_json::from_value(valid_prepare_value()).expect("test prepare is valid");
        let frames: Vec<_> = (0..MAX_FRAMES as u64)
            .map(|index| test_frame(&request.attachment_key_sha256, index))
            .collect();
        let mut value = serde_json::to_value(&request).expect("test request serializes");
        let object = value.as_object_mut().expect("test request object");
        object.remove("idempotency_key");
        object.insert(
            "schema_version".to_owned(),
            Value::String(RECORD_SCHEMA.to_owned()),
        );
        object.insert(
            "attachment_receipt_object_sha256".to_owned(),
            Value::String(String::new()),
        );
        object.insert(
            "attachment_receipt_canonical_sha256".to_owned(),
            Value::String(String::new()),
        );
        object.insert(
            "anchor_binding_sha256".to_owned(),
            Value::String(digest('a')),
        );
        object.insert("frames".to_owned(), serde_json::to_value(frames).unwrap());
        object.insert(
            "attachment_status".to_owned(),
            Value::String(
                forgecad_contracts::FICTIONAL_ENERGY_VFX_ANIMATED_SOCKET_ATTACHMENT_V3_STATUS
                    .to_owned(),
            ),
        );
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
        object.insert("runtime_write_performed".to_owned(), Value::Bool(true));
        object.insert("restart_hash_verified".to_owned(), Value::Bool(true));
        object.insert("candidate_confirmed".to_owned(), Value::Bool(false));
        object.insert("version_created".to_owned(), Value::Bool(false));
        object.insert("export_performed".to_owned(), Value::Bool(false));
        object.insert("actual_engine_roundtrip".to_owned(), Value::Bool(false));
        object.insert("production_stage_advanced".to_owned(), Value::Bool(false));
        object.insert("canonical_sha256".to_owned(), Value::String(String::new()));
        object.insert(
            "created_at".to_owned(),
            Value::String("2026-01-01T00:00:00Z".to_owned()),
        );
        serde_json::from_value(value).expect("test Attachment@3 record is valid")
    }

    #[test]
    fn v3_policy_and_frame_scope_are_frozen() {
        assert_eq!(
            forgecad_contracts::FICTIONAL_ENERGY_VFX_ANIMATED_SOCKET_ATTACHMENT_V3_POLICY,
            "projection-v2-particles-v2-trails-v2-trails-bloom-v2-animated-socket-attachment-bridge@3"
        );
        assert_eq!(MAX_FRAMES, 15);
        assert!(is_sha256(&digest('a')));
    }

    #[test]
    fn replay_normalization_ignores_only_runtime_identity_fields() {
        let mut left = json!({
            "canonical_sha256": digest('a'),
            "created_at":"2026-01-01T00:00:00Z",
            "attachment_receipt_object_sha256":digest('b'),
            "frames":[{"canonical_sha256":digest('c'),"created_at":"a","frame_index":0}]
        });
        let mut right = json!({
            "canonical_sha256": digest('d'),
            "created_at":"2026-01-02T00:00:00Z",
            "attachment_receipt_object_sha256":digest('e'),
            "frames":[{"canonical_sha256":digest('f'),"created_at":"b","frame_index":0}]
        });
        normalize_for_compare(&mut left);
        normalize_for_compare(&mut right);
        assert_eq!(left, right);
    }

    #[test]
    fn prepare_parser_is_closed_and_requires_exact_fifteen_ticks() {
        let valid = valid_prepare_value();
        assert!(parse_prepare(&valid).is_ok());

        let mut unknown = valid.clone();
        unknown
            .as_object_mut()
            .unwrap()
            .insert("unknown_field".to_owned(), Value::Null);
        assert!(parse_prepare(&unknown).is_err());

        let mut wrong_count = valid;
        wrong_count
            .as_object_mut()
            .unwrap()
            .insert("sample_count".to_owned(), Value::from(14_u64));
        assert!(parse_prepare(&wrong_count).is_err());
    }

    #[test]
    fn source_schedule_requires_sixteen_sources_and_maps_fifteen_outputs() {
        let projection_indices: Vec<_> = (0..UPSTREAM_FRAMES as u64).collect();
        let particle_indices = projection_indices.clone();
        let trail_indices: Vec<_> = (0..MAX_FRAMES as u64).collect();
        let bloom_indices = trail_indices.clone();
        let projection_ticks: Vec<_> = (0..=MAX_FRAMES as u64).map(|tick| 100 + tick).collect();
        let particle_ticks = projection_ticks.clone();
        let output_ticks: Vec<_> = projection_ticks[1..].to_vec();

        assert!(validate_source_output_schedule(
            &projection_indices,
            &projection_ticks,
            &particle_indices,
            &particle_ticks,
            &trail_indices,
            &output_ticks,
            &bloom_indices,
            &output_ticks,
            &output_ticks,
        )
        .is_ok());

        let missing_preroll = projection_indices[1..].to_vec();
        assert!(validate_source_output_schedule(
            &missing_preroll,
            &output_ticks,
            &particle_indices,
            &particle_ticks,
            &trail_indices,
            &output_ticks,
            &bloom_indices,
            &output_ticks,
            &output_ticks,
        )
        .is_err());
    }

    #[test]
    fn receipt_projection_is_replayable_without_receipt_object_self_reference() {
        let record = test_record();
        let (record, bytes) = finish_record_and_receipt(record).expect("receipt should build");
        let value: Value = serde_json::from_slice(&bytes).expect("receipt should be JSON");
        assert_eq!(canonical_json_bytes(&value).unwrap(), bytes);
        assert_eq!(
            value.get("canonical_sha256").and_then(Value::as_str),
            Some(record.canonical_sha256.as_str())
        );
        assert_eq!(value.get("frame_count").and_then(Value::as_u64), Some(15));
        assert_eq!(
            value.get("frames").and_then(Value::as_array).map(Vec::len),
            Some(15)
        );
        assert_eq!(
            value
                .get("frame_dependencies")
                .and_then(Value::as_array)
                .map(Vec::len),
            Some(15)
        );
        assert_eq!(
            canonical_json_hash(&receipt_projection(&record)),
            record.attachment_receipt_canonical_sha256
        );

        let mut normalized = record.clone();
        normalized.attachment_receipt_object_sha256 = digest('z');
        normalized.attachment_receipt_object_sha256.clear();
        normalized.canonical_sha256.clear();
        assert_eq!(
            canonical_json_hash(&serde_json::to_value(normalized).unwrap()),
            record.canonical_sha256
        );
    }
}
