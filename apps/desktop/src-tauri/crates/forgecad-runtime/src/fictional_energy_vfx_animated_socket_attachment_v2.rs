//! Projection-bound animated-socket VFX attachment.
//!
//! V1 intentionally remains fail-closed because its source receipts only
//! describe sidecar socket intent. V2 composes the durable animated GLB socket
//! transform projection with the projection-aware particle, trail and trail
//! Bloom sequences. All dependencies are re-read before the single receipt is
//! reserved, so an invalid or retargeted request performs no CAS/SQLite write.

use super::{
    canonical_json_bytes, canonical_json_hash, exact_object, is_opaque_id, is_sha256, now_string,
    sha256_hex, CasObject, Runtime, RuntimeError,
};
use forgecad_contracts::{
    FictionalEnergyVfxAnimatedSocketAttachmentV2FrameRecord,
    FictionalEnergyVfxAnimatedSocketAttachmentV2GetRequest,
    FictionalEnergyVfxAnimatedSocketAttachmentV2PrepareRequest,
    FictionalEnergyVfxAnimatedSocketAttachmentV2Record,
    FictionalEnergyVfxAnimatedSocketParticlesSequence,
    FictionalEnergyVfxAnimatedSocketParticlesSequenceGetResult,
    FictionalEnergyVfxAnimatedSocketTrailsBloomSequence,
    FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceGetResult,
    FictionalEnergyVfxAnimatedSocketTrailsSequence,
    FictionalEnergyVfxAnimatedSocketTrailsSequenceGetResult,
    GameWeaponAnimatedGlbSocketTransformProjection,
    GameWeaponAnimatedGlbSocketTransformProjectionGetResult,
};
use serde_json::{json, Map, Value};

const PREPARE_SCHEMA: &str = "FictionalEnergyVfxAnimatedSocketAttachmentPrepareRequest@2";
const GET_SCHEMA: &str = "FictionalEnergyVfxAnimatedSocketAttachmentGetRequest@2";
const PREPARE_RESULT_SCHEMA: &str = "FictionalEnergyVfxAnimatedSocketAttachmentPrepareResult@2";
const GET_RESULT_SCHEMA: &str = "FictionalEnergyVfxAnimatedSocketAttachmentGetResult@2";
const RECORD_SCHEMA: &str = "FictionalEnergyVfxAnimatedSocketAttachment@2";
const FRAME_SCHEMA: &str = "FictionalEnergyVfxAnimatedSocketAttachmentFrame@2";
const RECEIPT_SCHEMA: &str = "FictionalEnergyVfxAnimatedSocketAttachmentReceipt@2";
const ATTACHMENT_POLICY: &str =
    "fictional-energy-vfx-animated-socket-attachment-projection-bound@2";
const FRAME_SCOPE: &str = "lod0-animation-vfx-trail-frame-range-1-15@2";
const ATTACHMENT_STATUS: &str =
    "runtime-owned-durable-fictional-energy-vfx-animated-socket-attachment-v2";
const RECEIPT_KIND: &str = "fictional-energy-vfx-animated-socket-attachment-v2-receipt";
const RECEIPT_MIME: &str = "application/json";
const MAX_RECEIPT_BYTES: usize = 1024 * 1024;
const MAX_FRAMES: usize = 15;

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
    "projection_key_sha256",
    "projection_object_sha256",
    "projection_canonical_sha256",
    "particle_sequence_key_sha256",
    "particle_sequence_canonical_sha256",
    "trail_sequence_key_sha256",
    "trail_sequence_canonical_sha256",
    "trail_bloom_sequence_key_sha256",
    "trail_bloom_sequence_canonical_sha256",
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
        "FICTIONAL_ENERGY_VFX_ANIMATED_SOCKET_ATTACHMENT_V2_INVALID: {}",
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
) -> Result<
    (
        FictionalEnergyVfxAnimatedSocketAttachmentV2PrepareRequest,
        String,
    ),
    RuntimeError,
> {
    let object = exact_object(value, PREPARE_FIELDS, PREPARE_SCHEMA)?;
    if text(object, "schema_version")? != PREPARE_SCHEMA {
        return Err(invalid("prepare schema version differs"));
    }
    let request: FictionalEnergyVfxAnimatedSocketAttachmentV2PrepareRequest =
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
        "projection_key_sha256",
        "projection_object_sha256",
        "projection_canonical_sha256",
        "particle_sequence_key_sha256",
        "particle_sequence_canonical_sha256",
        "trail_sequence_key_sha256",
        "trail_sequence_canonical_sha256",
        "trail_bloom_sequence_key_sha256",
        "trail_bloom_sequence_canonical_sha256",
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
    if request.attachment_policy != ATTACHMENT_POLICY || request.frame_scope != FRAME_SCOPE {
        return Err(invalid("attachment policy or frame scope differs"));
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
) -> Result<FictionalEnergyVfxAnimatedSocketAttachmentV2GetRequest, RuntimeError> {
    let object = exact_object(value, GET_FIELDS, GET_SCHEMA)?;
    if text(object, "schema_version")? != GET_SCHEMA {
        return Err(invalid("get schema version differs"));
    }
    sha(object, "attachment_key_sha256")?;
    id(object, "project_id")?;
    id(object, "candidate_id")?;
    serde_json::from_value(value.clone())
        .map_err(|error| invalid(format!("get request is malformed: {error}")))
}

#[derive(Debug)]
struct Dependencies {
    projection: GameWeaponAnimatedGlbSocketTransformProjection,
    particles: FictionalEnergyVfxAnimatedSocketParticlesSequence,
    trails: FictionalEnergyVfxAnimatedSocketTrailsSequence,
    trail_bloom: FictionalEnergyVfxAnimatedSocketTrailsBloomSequence,
}

fn require_read_result(value: &Value, label: &str) -> Result<(), RuntimeError> {
    if value.get("runtime_write").and_then(Value::as_bool) != Some(false)
        || value.get("restart_hash_verified").and_then(Value::as_bool) != Some(true)
    {
        return Err(invalid(format!(
            "{label} is not a verified read-only result"
        )));
    }
    Ok(())
}

fn exact(label: &str, actual: &str, expected: &str) -> Result<(), RuntimeError> {
    if actual != expected {
        return Err(invalid(format!("{label} binding differs")));
    }
    Ok(())
}

fn validate_common_projection(
    request: &FictionalEnergyVfxAnimatedSocketAttachmentV2PrepareRequest,
    projection: &GameWeaponAnimatedGlbSocketTransformProjection,
) -> Result<(), RuntimeError> {
    exact(
        "projection project",
        &projection.project_id,
        &request.project_id,
    )?;
    exact(
        "projection candidate",
        &projection.candidate_id,
        &request.candidate_id,
    )?;
    exact(
        "projection candidate state",
        &projection.candidate_state_sha256,
        &request.candidate_state_sha256,
    )?;
    exact(
        "projection delivery",
        &projection.delivery_manifest_object_sha256,
        &request.delivery_manifest_object_sha256,
    )?;
    exact(
        "projection source artifact",
        &projection.source_artifact_sha256,
        &request.source_artifact_sha256,
    )?;
    exact(
        "projection animated socket",
        &projection.animated_socket_materialization_key_sha256,
        &request.animated_socket_materialization_key_sha256,
    )?;
    exact(
        "projection anchor set object",
        &projection.anchor_set_object_sha256,
        &request.animated_socket_anchor_set_object_sha256,
    )?;
    exact(
        "projection anchor set canonical",
        &projection.anchor_set_canonical_sha256,
        &request.animated_socket_anchor_set_canonical_sha256,
    )?;
    exact(
        "projection clip",
        &projection.animation_clip_id,
        &request.animation_clip_id,
    )?;
    exact(
        "projection clip object",
        &projection.animation_clip_object_sha256,
        &request.animation_clip_object_sha256,
    )?;
    exact(
        "projection clip canonical",
        &projection.animation_clip_canonical_sha256,
        &request.animation_clip_canonical_sha256,
    )?;
    exact(
        "projection animated artifact",
        &projection.animated_artifact_sha256,
        &request.animated_artifact_sha256,
    )?;
    exact(
        "projection animation receipt object",
        &projection.animation_receipt_object_sha256,
        &request.animation_receipt_object_sha256,
    )?;
    exact(
        "projection animation receipt canonical",
        &projection.animation_receipt_canonical_sha256,
        &request.animation_receipt_canonical_sha256,
    )?;
    exact(
        "projection socket node encoding",
        &projection.socket_node_id_encoding_sha256,
        &request.socket_node_id_encoding_sha256,
    )?;
    exact(
        "projection socket roles",
        &projection.socket_roles_sha256,
        &request.socket_roles_sha256,
    )?;
    if projection.quality_status != "structural_only"
        || projection.visual_quality_status != "NOT_PROVEN"
        || projection.commercial_fps_quality_status != "NOT_PROVEN"
        || projection.human_review_status != "NOT_RUN"
        || projection.commercial_engine_status != "NOT_RUN"
    {
        return Err(invalid("projection truth boundary differs"));
    }
    Ok(())
}

fn validate_sequence_common(
    request: &FictionalEnergyVfxAnimatedSocketAttachmentV2PrepareRequest,
    label: &str,
    project_id: &str,
    candidate_id: &str,
    candidate_state: &str,
    delivery: &str,
    source_artifact: &str,
    projection_key: &str,
    projection_object: &str,
    projection_canonical: &str,
    animated_socket_key: &str,
    animated_artifact: &str,
    anchor_object: &str,
    anchor_canonical: &str,
    clip_id: &str,
    clip_object: &str,
    clip_canonical: &str,
    animation_receipt_object: &str,
    animation_receipt_canonical: &str,
    profile_object: &str,
    profile_canonical: &str,
    node_encoding: &str,
    roles: &str,
    quality_status: &str,
    visual_status: &str,
    commercial_status: &str,
    human_status: &str,
    engine_status: &str,
) -> Result<(), RuntimeError> {
    for (field, actual, expected) in [
        ("project", project_id, request.project_id.as_str()),
        ("candidate", candidate_id, request.candidate_id.as_str()),
        (
            "candidate state",
            candidate_state,
            request.candidate_state_sha256.as_str(),
        ),
        (
            "delivery",
            delivery,
            request.delivery_manifest_object_sha256.as_str(),
        ),
        (
            "source artifact",
            source_artifact,
            request.source_artifact_sha256.as_str(),
        ),
        (
            "projection key",
            projection_key,
            request.projection_key_sha256.as_str(),
        ),
        (
            "projection object",
            projection_object,
            request.projection_object_sha256.as_str(),
        ),
        (
            "projection canonical",
            projection_canonical,
            request.projection_canonical_sha256.as_str(),
        ),
        (
            "animated socket",
            animated_socket_key,
            request.animated_socket_materialization_key_sha256.as_str(),
        ),
        (
            "animated artifact",
            animated_artifact,
            request.animated_artifact_sha256.as_str(),
        ),
        (
            "anchor object",
            anchor_object,
            request.animated_socket_anchor_set_object_sha256.as_str(),
        ),
        (
            "anchor canonical",
            anchor_canonical,
            request.animated_socket_anchor_set_canonical_sha256.as_str(),
        ),
        ("clip", clip_id, request.animation_clip_id.as_str()),
        (
            "clip object",
            clip_object,
            request.animation_clip_object_sha256.as_str(),
        ),
        (
            "clip canonical",
            clip_canonical,
            request.animation_clip_canonical_sha256.as_str(),
        ),
        (
            "animation receipt object",
            animation_receipt_object,
            request.animation_receipt_object_sha256.as_str(),
        ),
        (
            "animation receipt canonical",
            animation_receipt_canonical,
            request.animation_receipt_canonical_sha256.as_str(),
        ),
        (
            "VFX profile object",
            profile_object,
            request.vfx_profile_object_sha256.as_str(),
        ),
        (
            "VFX profile canonical",
            profile_canonical,
            request.vfx_profile_canonical_sha256.as_str(),
        ),
        (
            "socket node encoding",
            node_encoding,
            request.socket_node_id_encoding_sha256.as_str(),
        ),
        ("socket roles", roles, request.socket_roles_sha256.as_str()),
    ] {
        exact(&format!("{label} {field}"), actual, expected)?;
    }
    if quality_status != "structural_only"
        || visual_status != "NOT_PROVEN"
        || commercial_status != "NOT_PROVEN"
        || human_status != "NOT_RUN"
        || engine_status != "NOT_RUN"
    {
        return Err(invalid(format!("{label} truth boundary differs")));
    }
    Ok(())
}

fn validate_dependencies(
    runtime: &Runtime,
    request: &FictionalEnergyVfxAnimatedSocketAttachmentV2PrepareRequest,
) -> Result<Dependencies, RuntimeError> {
    let projection_value =
        runtime.game_weapon_animated_glb_socket_transform_projection_get(&json!({
            "schema_version":"GameWeaponAnimatedGlbSocketTransformProjectionGetRequest@1",
            "projection_key_sha256":request.projection_key_sha256,
            "project_id":request.project_id,
            "candidate_id":request.candidate_id
        }))?;
    require_read_result(&projection_value, "transform projection")?;
    let projection_result: GameWeaponAnimatedGlbSocketTransformProjectionGetResult =
        serde_json::from_value(projection_value)
            .map_err(|error| invalid(format!("projection result is malformed: {error}")))?;
    exact(
        "projection object",
        &projection_result.projection_object_sha256,
        &request.projection_object_sha256,
    )?;
    exact(
        "projection canonical",
        &projection_result.projection.canonical_sha256,
        &request.projection_canonical_sha256,
    )?;
    validate_common_projection(request, &projection_result.projection)?;

    let particle_value =
        runtime.fictional_energy_vfx_animated_socket_particles_sequence_get(&json!({
            "schema_version":"FictionalEnergyVfxAnimatedSocketParticlesSequenceGetRequest@1",
            "sequence_key_sha256":request.particle_sequence_key_sha256,
            "project_id":request.project_id,
            "candidate_id":request.candidate_id
        }))?;
    require_read_result(&particle_value, "animated particles sequence")?;
    let particle_result: FictionalEnergyVfxAnimatedSocketParticlesSequenceGetResult =
        serde_json::from_value(particle_value)
            .map_err(|error| invalid(format!("particle sequence result is malformed: {error}")))?;
    let particles = particle_result.sequence;
    exact(
        "particle sequence key",
        &particles.sequence_key_sha256,
        &request.particle_sequence_key_sha256,
    )?;
    exact(
        "particle sequence canonical",
        &particles.canonical_sha256,
        &request.particle_sequence_canonical_sha256,
    )?;
    validate_sequence_common(
        request,
        "particle sequence",
        &particles.project_id,
        &particles.candidate_id,
        &particles.candidate_state_sha256,
        &particles.delivery_manifest_object_sha256,
        &particles.source_artifact_sha256,
        &particles.projection_key_sha256,
        &particles.projection_object_sha256,
        &particles.projection_canonical_sha256,
        &particles.animated_socket_materialization_key_sha256,
        &particles.animated_artifact_sha256,
        &particles.animated_socket_anchor_set_object_sha256,
        &particles.animated_socket_anchor_set_canonical_sha256,
        &particles.animation_clip_id,
        &particles.animation_clip_object_sha256,
        &particles.animation_clip_canonical_sha256,
        &particles.animation_receipt_object_sha256,
        &particles.animation_receipt_canonical_sha256,
        &particles.vfx_profile_object_sha256,
        &particles.vfx_profile_canonical_sha256,
        &particles.socket_node_id_encoding_sha256,
        &particles.socket_roles_sha256,
        &particles.quality_status,
        &particles.visual_quality_status,
        &particles.commercial_fps_quality_status,
        &particles.human_review_status,
        &particles.commercial_engine_status,
    )?;

    let trail_value = runtime.fictional_energy_vfx_animated_socket_trails_sequence_get(&json!({
        "schema_version":"FictionalEnergyVfxAnimatedSocketTrailsSequenceGetRequest@1",
        "sequence_key_sha256":request.trail_sequence_key_sha256,
        "project_id":request.project_id,
        "candidate_id":request.candidate_id
    }))?;
    require_read_result(&trail_value, "animated trails sequence")?;
    let trail_result: FictionalEnergyVfxAnimatedSocketTrailsSequenceGetResult =
        serde_json::from_value(trail_value)
            .map_err(|error| invalid(format!("trail sequence result is malformed: {error}")))?;
    let trails = trail_result.sequence;
    exact(
        "trail sequence key",
        &trails.sequence_key_sha256,
        &request.trail_sequence_key_sha256,
    )?;
    exact(
        "trail sequence canonical",
        &trails.canonical_sha256,
        &request.trail_sequence_canonical_sha256,
    )?;
    exact(
        "trail particle sequence",
        &trails
            .frames
            .first()
            .ok_or_else(|| invalid("trail sequence has no frames"))?
            .particle_sequence_key_sha256,
        &request.particle_sequence_key_sha256,
    )?;
    validate_sequence_common(
        request,
        "trail sequence",
        &trails.project_id,
        &trails.candidate_id,
        &trails.candidate_state_sha256,
        &trails.delivery_manifest_object_sha256,
        &trails.source_artifact_sha256,
        &trails.projection_key_sha256,
        &trails.projection_object_sha256,
        &trails.projection_canonical_sha256,
        &trails.animated_socket_materialization_key_sha256,
        &trails.animated_artifact_sha256,
        &trails.animated_socket_anchor_set_object_sha256,
        &trails.animated_socket_anchor_set_canonical_sha256,
        &trails.animation_clip_id,
        &trails.animation_clip_object_sha256,
        &trails.animation_clip_canonical_sha256,
        &trails.animation_receipt_object_sha256,
        &trails.animation_receipt_canonical_sha256,
        &trails.vfx_profile_object_sha256,
        &trails.vfx_profile_canonical_sha256,
        &trails.socket_node_id_encoding_sha256,
        &trails.socket_roles_sha256,
        &trails.quality_status,
        &trails.visual_quality_status,
        &trails.commercial_fps_quality_status,
        &trails.human_review_status,
        &trails.commercial_engine_status,
    )?;

    let bloom_value =
        runtime.fictional_energy_vfx_animated_socket_trails_bloom_sequence_get(&json!({
            "schema_version":"FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceGetRequest@1",
            "sequence_key_sha256":request.trail_bloom_sequence_key_sha256,
            "project_id":request.project_id,
            "candidate_id":request.candidate_id
        }))?;
    require_read_result(&bloom_value, "animated trails Bloom sequence")?;
    let bloom_result: FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceGetResult =
        serde_json::from_value(bloom_value).map_err(|error| {
            invalid(format!("trail Bloom sequence result is malformed: {error}"))
        })?;
    let trail_bloom = bloom_result.sequence;
    exact(
        "trail Bloom sequence key",
        &trail_bloom.sequence_key_sha256,
        &request.trail_bloom_sequence_key_sha256,
    )?;
    exact(
        "trail Bloom sequence canonical",
        &trail_bloom.canonical_sha256,
        &request.trail_bloom_sequence_canonical_sha256,
    )?;
    exact(
        "trail Bloom source trail key",
        &trail_bloom.trail_sequence_key_sha256,
        &request.trail_sequence_key_sha256,
    )?;
    exact(
        "trail Bloom source trail canonical",
        &trail_bloom.trail_sequence_canonical_sha256,
        &request.trail_sequence_canonical_sha256,
    )?;
    validate_sequence_common(
        request,
        "trail Bloom sequence",
        &trail_bloom.project_id,
        &trail_bloom.candidate_id,
        &trail_bloom.candidate_state_sha256,
        &trail_bloom.delivery_manifest_object_sha256,
        &trail_bloom.source_artifact_sha256,
        &trail_bloom.projection_key_sha256,
        &trail_bloom.projection_object_sha256,
        &trail_bloom.projection_canonical_sha256,
        &trail_bloom.animated_socket_materialization_key_sha256,
        &trail_bloom.animated_artifact_sha256,
        &trail_bloom.animated_socket_anchor_set_object_sha256,
        &trail_bloom.animated_socket_anchor_set_canonical_sha256,
        &trail_bloom.animation_clip_id,
        &trail_bloom.animation_clip_object_sha256,
        &trail_bloom.animation_clip_canonical_sha256,
        &trail_bloom.animation_receipt_object_sha256,
        &trail_bloom.animation_receipt_canonical_sha256,
        &trail_bloom.vfx_profile_object_sha256,
        &trail_bloom.vfx_profile_canonical_sha256,
        &trail_bloom.socket_node_id_encoding_sha256,
        &trail_bloom.socket_roles_sha256,
        &trail_bloom.quality_status,
        &trail_bloom.visual_quality_status,
        &trail_bloom.commercial_fps_quality_status,
        &trail_bloom.human_review_status,
        &trail_bloom.commercial_engine_status,
    )?;

    if trail_bloom.frames.is_empty()
        || trail_bloom.frames.len() > MAX_FRAMES
        || trail_bloom.frames.len() != trails.frames.len()
    {
        return Err(invalid(
            "attachment frame count is outside 1..15 or differs from trails",
        ));
    }

    Ok(Dependencies {
        projection: projection_result.projection,
        particles,
        trails,
        trail_bloom,
    })
}

fn frame_records(
    attachment_key: &str,
    dependencies: &Dependencies,
    created_at: &str,
) -> Result<Vec<FictionalEnergyVfxAnimatedSocketAttachmentV2FrameRecord>, RuntimeError> {
    let mut records = Vec::with_capacity(dependencies.trail_bloom.frames.len());
    for (ordinal, bloom) in dependencies.trail_bloom.frames.iter().enumerate() {
        let trail = dependencies
            .trails
            .frames
            .get(ordinal)
            .ok_or_else(|| invalid("trail frame is unavailable"))?;
        if bloom.frame_index != ordinal as u64
            || trail.frame_index != bloom.frame_index
            || trail.current_projection_frame_index != ordinal as u64 + 1
            || trail.current_particle_frame_index != ordinal as u64 + 1
        {
            return Err(invalid("trail or trail Bloom frame order differs"));
        }
        let projection_index = usize::try_from(trail.current_projection_frame_index)
            .map_err(|_| invalid("projection frame index overflows"))?;
        let particle_index = usize::try_from(trail.current_particle_frame_index)
            .map_err(|_| invalid("particle frame index overflows"))?;
        let projection = dependencies
            .projection
            .frames
            .get(projection_index)
            .ok_or_else(|| invalid("current projection frame is unavailable"))?;
        let particle = dependencies
            .particles
            .frames
            .get(particle_index)
            .ok_or_else(|| invalid("current particle frame is unavailable"))?;
        if projection.frame_index != trail.current_projection_frame_index
            || particle.frame_index != trail.current_particle_frame_index
            || projection.sample_time_ticks != trail.sample_time_ticks
            || particle.sample_time_ticks != trail.sample_time_ticks
            || bloom.sample_time_ticks != trail.sample_time_ticks
            || particle.projection_frame_canonical_sha256 != projection.canonical_sha256
            || particle.projection_socket_transform_inventory_sha256
                != projection.socket_transform_inventory_sha256
            || particle.projection_socket_transform_readback_sha256
                != projection.socket_transform_readback_sha256
            || trail.current_projection_frame_canonical_sha256 != projection.canonical_sha256
            || trail.current_projection_socket_transform_inventory_sha256
                != projection.socket_transform_inventory_sha256
            || trail.current_projection_socket_transform_readback_sha256
                != projection.socket_transform_readback_sha256
            || trail.current_particle_frame_canonical_sha256 != particle.canonical_sha256
            || trail.current_particle_key_sha256 != particle.particle_key_sha256
            || bloom.trail_frame_canonical_sha256 != trail.canonical_sha256
            || bloom.particle_sequence_frame_canonical_sha256 != particle.canonical_sha256
            || bloom.base_frame_key_sha256 != trail.base_frame_key_sha256
            || bloom.bloom_key_sha256 != trail.bloom_key_sha256
            || trail.base_frame_key_sha256 != particle.base_frame_key_sha256
            || trail.bloom_key_sha256 != particle.bloom_key_sha256
            || bloom.trail_sequence_key_sha256 != dependencies.trails.sequence_key_sha256
            || bloom.trail_sequence_canonical_sha256 != dependencies.trails.canonical_sha256
        {
            return Err(invalid(format!(
                "frame {ordinal} projection/VFX binding differs"
            )));
        }
        let mut record = FictionalEnergyVfxAnimatedSocketAttachmentV2FrameRecord {
            schema_version: FRAME_SCHEMA.to_owned(),
            attachment_key_sha256: attachment_key.to_owned(),
            frame_index: bloom.frame_index,
            projection_frame_index: projection.frame_index,
            particle_sequence_frame_index: particle.frame_index,
            sample_time_ticks: bloom.sample_time_ticks,
            animation_pose_readback_sha256: projection.source_animation_sample_sha256.clone(),
            socket_transform_inventory_sha256: projection.socket_transform_inventory_sha256.clone(),
            socket_transform_readback_sha256: projection.socket_transform_readback_sha256.clone(),
            emitter_socket_bindings_sha256: particle.emitter_socket_bindings_sha256.clone(),
            trail_socket_bindings_sha256: trail.emitter_binding_sha256.clone(),
            base_frame_key_sha256: bloom.base_frame_key_sha256.clone(),
            bloom_key_sha256: bloom.bloom_key_sha256.clone(),
            particle_key_sha256: particle.particle_key_sha256.clone(),
            trail_key_sha256: trail.trail_key_sha256.clone(),
            trail_bloom_key_sha256: bloom.trail_bloom_key_sha256.clone(),
            projection_frame_canonical_sha256: projection.canonical_sha256.clone(),
            particle_sequence_frame_canonical_sha256: particle.canonical_sha256.clone(),
            trail_sequence_frame_canonical_sha256: trail.canonical_sha256.clone(),
            trail_bloom_sequence_frame_canonical_sha256: bloom.canonical_sha256.clone(),
            canonical_sha256: String::new(),
            created_at: created_at.to_owned(),
        };
        let mut value =
            serde_json::to_value(&record).map_err(|error| invalid(error.to_string()))?;
        value["canonical_sha256"] = Value::String(String::new());
        record.canonical_sha256 = canonical_json_hash(&value);
        records.push(record);
    }
    Ok(records)
}

fn build_record(
    request: &FictionalEnergyVfxAnimatedSocketAttachmentV2PrepareRequest,
    dependencies: &Dependencies,
    request_sha256: &str,
) -> Result<FictionalEnergyVfxAnimatedSocketAttachmentV2Record, RuntimeError> {
    if request_sha256 != request.input_sha256 {
        return Err(invalid("attachment request hash changed"));
    }
    let created_at = now_string();
    let mut record = FictionalEnergyVfxAnimatedSocketAttachmentV2Record {
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
        attachment_policy: ATTACHMENT_POLICY.to_owned(),
        socket_node_id_encoding_sha256: request.socket_node_id_encoding_sha256.clone(),
        socket_roles_sha256: request.socket_roles_sha256.clone(),
        frame_scope: FRAME_SCOPE.to_owned(),
        frames: frame_records(&request.attachment_key_sha256, dependencies, &created_at)?,
        attachment_status: ATTACHMENT_STATUS.to_owned(),
        canonical_sha256: String::new(),
        created_at,
    };
    let mut value = serde_json::to_value(&record).map_err(|error| invalid(error.to_string()))?;
    value["canonical_sha256"] = Value::String(String::new());
    record.canonical_sha256 = canonical_json_hash(&value);
    Ok(record)
}

fn replay_equivalent(
    left: &FictionalEnergyVfxAnimatedSocketAttachmentV2Record,
    right: &FictionalEnergyVfxAnimatedSocketAttachmentV2Record,
) -> Result<bool, RuntimeError> {
    fn normalize(value: &mut Value) {
        let Some(object) = value.as_object_mut() else {
            return;
        };
        object.insert("canonical_sha256".to_owned(), Value::String(String::new()));
        object.insert("created_at".to_owned(), Value::String(String::new()));
        if let Some(frames) = object.get_mut("frames").and_then(Value::as_array_mut) {
            for frame in frames {
                if let Some(frame) = frame.as_object_mut() {
                    frame.insert("canonical_sha256".to_owned(), Value::String(String::new()));
                    frame.insert("created_at".to_owned(), Value::String(String::new()));
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
    record: &FictionalEnergyVfxAnimatedSocketAttachmentV2Record,
) -> Result<Value, RuntimeError> {
    let frame_dependencies = record
        .frames
        .iter()
        .map(|frame| {
            json!({
                "frame_index":frame.frame_index,
                "projection_frame_index":frame.projection_frame_index,
                "particle_sequence_frame_index":frame.particle_sequence_frame_index,
                "projection_frame_canonical_sha256":frame.projection_frame_canonical_sha256,
                "particle_sequence_frame_canonical_sha256":frame.particle_sequence_frame_canonical_sha256,
                "trail_sequence_frame_canonical_sha256":frame.trail_sequence_frame_canonical_sha256,
                "trail_bloom_sequence_frame_canonical_sha256":frame.trail_bloom_sequence_frame_canonical_sha256
            })
        })
        .collect::<Vec<_>>();
    Ok(json!({
        "schema_version":RECEIPT_SCHEMA,
        "attachment_key_sha256":record.attachment_key_sha256,
        "canonical_sha256":record.canonical_sha256,
        "frame_count":record.frames.len(),
        "projection_key_sha256":record.projection_key_sha256,
        "projection_canonical_sha256":record.projection_canonical_sha256,
        "particle_sequence_key_sha256":record.particle_sequence_key_sha256,
        "particle_sequence_canonical_sha256":record.particle_sequence_canonical_sha256,
        "trail_sequence_key_sha256":record.trail_sequence_key_sha256,
        "trail_sequence_canonical_sha256":record.trail_sequence_canonical_sha256,
        "trail_bloom_sequence_key_sha256":record.trail_bloom_sequence_key_sha256,
        "trail_bloom_sequence_canonical_sha256":record.trail_bloom_sequence_canonical_sha256,
        "frames":record.frames,
        "frame_dependencies":frame_dependencies,
        "runtime_write_performed":true,
        "restart_hash_verified":true,
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

fn validate_receipt(
    runtime: &Runtime,
    receipt_hash: &str,
    record: &FictionalEnergyVfxAnimatedSocketAttachmentV2Record,
) -> Result<(), RuntimeError> {
    let bytes = runtime.cas_read_bounded(receipt_hash, MAX_RECEIPT_BYTES as u64)?;
    if sha256_hex(&bytes) != receipt_hash {
        return Err(invalid("attachment receipt bytes are tampered"));
    }
    let value: Value = serde_json::from_slice(&bytes)
        .map_err(|error| invalid(format!("attachment receipt is malformed: {error}")))?;
    if value.get("schema_version").and_then(Value::as_str) != Some(RECEIPT_SCHEMA)
        || value.get("attachment_key_sha256").and_then(Value::as_str)
            != Some(record.attachment_key_sha256.as_str())
        || value.get("canonical_sha256").and_then(Value::as_str)
            != Some(record.canonical_sha256.as_str())
        || value.get("frame_count").and_then(Value::as_u64) != Some(record.frames.len() as u64)
        || value.get("frames").and_then(Value::as_array).map(Vec::len) != Some(record.frames.len())
    {
        return Err(invalid("attachment receipt parent binding differs"));
    }
    Ok(())
}

fn result_value(
    record: &FictionalEnergyVfxAnimatedSocketAttachmentV2Record,
    replayed: bool,
    schema_version: &str,
    runtime_write: bool,
) -> Result<Value, RuntimeError> {
    Ok(json!({
        "schema_version":schema_version,
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
    if let Some((existing, receipt_hash)) = runtime
        .store
        .get_fictional_energy_vfx_animated_socket_attachment_v2_link(
            &request.attachment_key_sha256,
        )?
    {
        validate_receipt(runtime, &receipt_hash, &existing)?;
        if replay_equivalent(&existing, &record)? {
            return result_value(&existing, true, PREPARE_RESULT_SCHEMA, false);
        }
        return Err(invalid(
            "attachment key is already bound to different content",
        ));
    }

    let receipt = receipt_value(&record)?;
    let bytes = canonical_json_bytes(&receipt).map_err(|error| invalid(error.to_string()))?;
    if bytes.is_empty() || bytes.len() > MAX_RECEIPT_BYTES {
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
    if let Err(error) = validate_receipt(runtime, &receipt_object.record.sha256, &record) {
        release_receipt(runtime, &reservation, &receipt_object, true);
        return Err(error);
    }
    match runtime
        .store
        .record_fictional_energy_vfx_animated_socket_attachment_v2_link(
            &record,
            &receipt_object.record,
        ) {
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
    let (record, receipt_hash) = runtime
        .store
        .get_fictional_energy_vfx_animated_socket_attachment_v2_link(
            &request.attachment_key_sha256,
        )?
        .ok_or_else(|| invalid("durable attachment is unavailable"))?;
    if record.project_id != request.project_id || record.candidate_id != request.candidate_id {
        return Err(invalid("attachment scope differs"));
    }
    let replay_request = FictionalEnergyVfxAnimatedSocketAttachmentV2PrepareRequest {
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
        projection_key_sha256: record.projection_key_sha256.clone(),
        projection_object_sha256: record.projection_object_sha256.clone(),
        projection_canonical_sha256: record.projection_canonical_sha256.clone(),
        particle_sequence_key_sha256: record.particle_sequence_key_sha256.clone(),
        particle_sequence_canonical_sha256: record.particle_sequence_canonical_sha256.clone(),
        trail_sequence_key_sha256: record.trail_sequence_key_sha256.clone(),
        trail_sequence_canonical_sha256: record.trail_sequence_canonical_sha256.clone(),
        trail_bloom_sequence_key_sha256: record.trail_bloom_sequence_key_sha256.clone(),
        trail_bloom_sequence_canonical_sha256: record.trail_bloom_sequence_canonical_sha256.clone(),
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
        return Err(invalid("durable attachment differs from dependency replay"));
    }
    validate_receipt(runtime, &receipt_hash, &record)?;
    result_value(&record, true, GET_RESULT_SCHEMA, false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use forgecad_contracts::{
        FictionalEnergyVfxAnimatedSocketParticlesSequenceFrame,
        FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceFrame,
        FictionalEnergyVfxAnimatedSocketTrailsSequenceFrame,
        GameWeaponAnimatedGlbSocketTransformProjectionFrame,
    };

    fn digest(ch: char) -> String {
        std::iter::repeat(ch).take(64).collect()
    }

    fn projection_frame(
        frame_index: u64,
        sample_time_ticks: u64,
        canonical: &str,
    ) -> GameWeaponAnimatedGlbSocketTransformProjectionFrame {
        GameWeaponAnimatedGlbSocketTransformProjectionFrame {
            schema_version: "GameWeaponAnimatedGlbSocketTransformProjectionFrame@1".to_owned(),
            projection_key_sha256: digest('p'),
            frame_index,
            sample_time_ticks,
            source_animation_sample_sha256: digest('a'),
            derived_socket_sample_sha256: digest('d'),
            socket_transform_inventory_sha256: digest('i'),
            socket_transform_readback_sha256: digest('r'),
            socket_transforms: Vec::new(),
            canonical_sha256: canonical.to_owned(),
            created_at: "2026-08-22T00:00:00Z".to_owned(),
        }
    }

    fn particle_frame(
        frame_index: u64,
        sample_time_ticks: u64,
        projection_canonical: &str,
        canonical: &str,
        particle_key: &str,
    ) -> FictionalEnergyVfxAnimatedSocketParticlesSequenceFrame {
        FictionalEnergyVfxAnimatedSocketParticlesSequenceFrame {
            schema_version: "FictionalEnergyVfxAnimatedSocketParticlesSequenceFrame@1".to_owned(),
            frame_index,
            sample_time_ticks,
            projection_frame_canonical_sha256: projection_canonical.to_owned(),
            projection_socket_transform_inventory_sha256: digest('i'),
            projection_socket_transform_readback_sha256: digest('r'),
            base_frame_key_sha256: digest('b'),
            bloom_key_sha256: digest('l'),
            emitter_socket_bindings_sha256: digest('e'),
            input_sha256: digest('n'),
            particle_key_sha256: particle_key.to_owned(),
            particle_seed_sha256: digest('s'),
            render_set_object_sha256: digest('q'),
            receipt_object_sha256: digest('t'),
            particle_color_object_sha256: digest('c'),
            particle_id_object_sha256: digest('u'),
            particle_depth_object_sha256: digest('v'),
            canonical_sha256: canonical.to_owned(),
            created_at: "2026-08-22T00:00:00Z".to_owned(),
        }
    }

    fn synthetic_dependencies() -> Dependencies {
        let projection_frames = vec![
            projection_frame(0, 0, "projection-frame-zero"),
            projection_frame(1, 10, "projection-frame-one"),
        ];
        let particle_frames = vec![
            particle_frame(
                0,
                0,
                "projection-frame-zero",
                "particle-frame-zero",
                "particle-zero",
            ),
            particle_frame(
                1,
                10,
                "projection-frame-one",
                "particle-frame-one",
                "particle-one",
            ),
        ];
        let trail_frame = FictionalEnergyVfxAnimatedSocketTrailsSequenceFrame {
            schema_version: "FictionalEnergyVfxAnimatedSocketTrailsSequenceFrame@1".to_owned(),
            frame_index: 0,
            sample_time_ticks: 10,
            history_origin: "projection-pre-roll-frame-zero".to_owned(),
            current_projection_frame_index: 1,
            current_particle_frame_index: 1,
            current_particle_key_sha256: "particle-one".to_owned(),
            current_particle_frame_canonical_sha256: "particle-frame-one".to_owned(),
            current_projection_frame_canonical_sha256: "projection-frame-one".to_owned(),
            current_projection_socket_transform_inventory_sha256: digest('i'),
            current_projection_socket_transform_readback_sha256: digest('r'),
            previous_projection_frame_index: 0,
            previous_particle_frame_index: 0,
            previous_particle_sequence_frame_canonical_sha256: "particle-frame-zero".to_owned(),
            previous_projection_frame_canonical_sha256: "projection-frame-zero".to_owned(),
            previous_projection_socket_transform_inventory_sha256: digest('i'),
            previous_projection_socket_transform_readback_sha256: digest('r'),
            projection_sample_set_sha256: digest('x'),
            particle_sequence_key_sha256: digest('k'),
            base_frame_key_sha256: digest('b'),
            bloom_key_sha256: digest('l'),
            camera_object_sha256: digest('o'),
            camera_identity_sha256: digest('m'),
            render_profile_sha256: digest('f'),
            render_worker_build_cohort_sha256: digest('w'),
            history_samples: Vec::new(),
            trail_count: 1,
            trail_emitter_roles: vec!["muzzle".to_owned()],
            trails: Vec::new(),
            trail_key_sha256: "trail-one".to_owned(),
            trail_seed_sha256: digest('z'),
            trail_inventory_sha256: digest('j'),
            trail_id_encoding_sha256: digest('y'),
            emitter_binding_sha256: "trail-emitter-binding-one".to_owned(),
            trail_color_object_sha256: digest('g'),
            trail_id_object_sha256: digest('h'),
            trail_depth_object_sha256: digest('d'),
            render_set_object_sha256: digest('q'),
            receipt_object_sha256: digest('t'),
            canonical_sha256: "trail-frame-one".to_owned(),
            created_at: "2026-08-22T00:00:00Z".to_owned(),
        };
        let bloom_frame = FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceFrame {
            schema_version: "FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceFrame@1".to_owned(),
            frame_index: 0,
            sample_time_ticks: 10,
            trail_sequence_key_sha256: digest('t'),
            trail_sequence_canonical_sha256: "trails-sequence".to_owned(),
            trail_frame_canonical_sha256: "trail-frame-one".to_owned(),
            trail_color_object_sha256: digest('g'),
            trail_id_object_sha256: digest('h'),
            trail_depth_object_sha256: digest('d'),
            particle_sequence_frame_canonical_sha256: "particle-frame-one".to_owned(),
            base_frame_key_sha256: digest('b'),
            bloom_key_sha256: digest('l'),
            camera_object_sha256: digest('o'),
            camera_identity_sha256: digest('m'),
            render_profile_sha256: digest('f'),
            render_worker_build_cohort_sha256: digest('w'),
            trail_bloom_profile_sha256: digest('v'),
            base_opaque_depth_object_sha256: digest('n'),
            base_aov_byte_exact_verified: true,
            base_opaque_depth_byte_exact_reused: true,
            bloom_pass_byte_exact_reused: true,
            particle_passes_byte_exact_reused: true,
            trail_passes_byte_exact_reused: true,
            base_bloom_mutated: false,
            particle_passes_mutated: false,
            trail_passes_mutated: false,
            trail_bloom_input: true,
            trail_emissive_source_rendered: true,
            trail_bloom_contribution_rendered: true,
            trail_bloom_rendered: true,
            trail_bloom_key_sha256: "trail-bloom-one".to_owned(),
            trail_bloom_seed_sha256: digest('s'),
            trail_emissive_source_object_sha256: digest('e'),
            trail_bloom_contribution_object_sha256: digest('c'),
            render_set_object_sha256: digest('q'),
            receipt_object_sha256: digest('t'),
            canonical_sha256: "trail-bloom-frame-one".to_owned(),
            created_at: "2026-08-22T00:00:00Z".to_owned(),
        };

        Dependencies {
            projection: GameWeaponAnimatedGlbSocketTransformProjection {
                schema_version: "GameWeaponAnimatedGlbSocketTransformProjection@1".to_owned(),
                projection_key_sha256: digest('p'),
                project_id: "project-1".to_owned(),
                candidate_id: "candidate-1".to_owned(),
                candidate_state_sha256: digest('s'),
                delivery_manifest_object_sha256: digest('d'),
                source_artifact_sha256: digest('a'),
                source_artifact_readback_sha256: digest('b'),
                animated_artifact_sha256: digest('c'),
                animated_artifact_readback_sha256: digest('e'),
                animation_receipt_object_sha256: digest('f'),
                animation_receipt_canonical_sha256: digest('g'),
                animated_socket_materialization_key_sha256: digest('h'),
                derived_animated_socket_artifact_sha256: digest('i'),
                derived_animated_socket_artifact_readback_sha256: digest('j'),
                derived_animated_socket_receipt_object_sha256: digest('k'),
                derived_animated_socket_receipt_canonical_sha256: digest('l'),
                anchor_set_object_sha256: digest('m'),
                anchor_set_canonical_sha256: digest('n'),
                animation_clip_id: "clip-1".to_owned(),
                animation_clip_object_sha256: digest('o'),
                animation_clip_canonical_sha256: digest('q'),
                socket_node_id_encoding_sha256: digest('r'),
                socket_node_inventory_sha256: digest('u'),
                socket_roles_sha256: digest('v'),
                socket_roles: vec!["muzzle".to_owned()],
                part_hierarchy_sha256: digest('w'),
                part_hierarchy_policy: "flat-part".to_owned(),
                transform_representation_policy: "trs".to_owned(),
                sample_schedule_sha256: digest('x'),
                sample_count: 2,
                sample_time_ticks: vec![0, 10],
                frame_scope: "lod0-animation-socket-sample-range-0-1@1".to_owned(),
                timebase_hz: 60,
                transform_projection_policy: "parent-world-compose".to_owned(),
                coordinate_system: "right-handed-y-up".to_owned(),
                transform_convention: "column-vector".to_owned(),
                float_quantization_policy: "f64-canonical".to_owned(),
                input_sha256: digest('q'),
                frames: projection_frames,
                projection_status: "runtime-owned-durable-animated-glb-socket-transform-projection"
                    .to_owned(),
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
                limitations: Vec::new(),
                canonical_sha256: digest('z'),
                created_at: "2026-08-22T00:00:00Z".to_owned(),
            },
            particles: FictionalEnergyVfxAnimatedSocketParticlesSequence {
                schema_version: "FictionalEnergyVfxAnimatedSocketParticlesSequence@1".to_owned(),
                sequence_key_sha256: digest('k'),
                project_id: "project-1".to_owned(),
                candidate_id: "candidate-1".to_owned(),
                candidate_state_sha256: digest('s'),
                delivery_manifest_object_sha256: digest('d'),
                source_artifact_sha256: digest('a'),
                projection_key_sha256: digest('p'),
                projection_object_sha256: digest('b'),
                projection_canonical_sha256: digest('z'),
                animated_socket_materialization_key_sha256: digest('h'),
                animated_artifact_sha256: digest('c'),
                animated_socket_anchor_set_object_sha256: digest('m'),
                animated_socket_anchor_set_canonical_sha256: digest('n'),
                animation_clip_id: "clip-1".to_owned(),
                animation_clip_object_sha256: digest('o'),
                animation_clip_canonical_sha256: digest('q'),
                animation_receipt_object_sha256: digest('f'),
                animation_receipt_canonical_sha256: digest('g'),
                vfx_profile_object_sha256: digest('a'),
                vfx_profile_canonical_sha256: digest('b'),
                socket_node_id_encoding_sha256: digest('r'),
                socket_roles_sha256: digest('v'),
                camera_object_sha256: digest('o'),
                camera_identity_sha256: digest('m'),
                render_profile_sha256: digest('f'),
                render_worker_build_cohort_sha256: digest('w'),
                sample_schedule_sha256: digest('x'),
                sample_count: 2,
                sample_time_ticks: vec![0, 10],
                frame_scope: "lod0-animation-vfx-particle-frame-range-0-1@1".to_owned(),
                particles_sequence_policy: "projection-bound".to_owned(),
                emitter_binding_policy: "fixed-socket-roles".to_owned(),
                transform_projection_policy: "parent-world-compose".to_owned(),
                frames: particle_frames,
                sequence_status:
                    "runtime-owned-durable-fictional-energy-vfx-animated-socket-particles"
                        .to_owned(),
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
                input_sha256: digest('q'),
                canonical_sha256: digest('t'),
                created_at: "2026-08-22T00:00:00Z".to_owned(),
            },
            trails: FictionalEnergyVfxAnimatedSocketTrailsSequence {
                schema_version: "FictionalEnergyVfxAnimatedSocketTrailsSequence@1".to_owned(),
                sequence_key_sha256: digest('t'),
                project_id: "project-1".to_owned(),
                candidate_id: "candidate-1".to_owned(),
                candidate_state_sha256: digest('s'),
                delivery_manifest_object_sha256: digest('d'),
                source_artifact_sha256: digest('a'),
                projection_key_sha256: digest('p'),
                projection_object_sha256: digest('b'),
                projection_canonical_sha256: digest('z'),
                animated_socket_materialization_key_sha256: digest('h'),
                animated_artifact_sha256: digest('c'),
                animated_socket_anchor_set_object_sha256: digest('m'),
                animated_socket_anchor_set_canonical_sha256: digest('n'),
                animation_clip_id: "clip-1".to_owned(),
                animation_clip_object_sha256: digest('o'),
                animation_clip_canonical_sha256: digest('q'),
                animation_receipt_object_sha256: digest('f'),
                animation_receipt_canonical_sha256: digest('g'),
                vfx_profile_object_sha256: digest('a'),
                vfx_profile_canonical_sha256: digest('b'),
                socket_node_id_encoding_sha256: digest('r'),
                socket_roles_sha256: digest('v'),
                camera_object_sha256: digest('o'),
                camera_identity_sha256: digest('m'),
                render_profile_sha256: digest('f'),
                render_worker_build_cohort_sha256: digest('w'),
                sample_schedule_sha256: digest('x'),
                sample_count: 1,
                sample_time_ticks: vec![10],
                frame_scope: "lod0-animation-vfx-trail-frame-range-0@1".to_owned(),
                trails_sequence_policy: "projection-bound".to_owned(),
                history_policy: "one-frame".to_owned(),
                history_pre_roll_policy: "source-frame-zero".to_owned(),
                trail_count: 1,
                trail_emitter_roles: vec!["muzzle".to_owned()],
                frames: vec![trail_frame],
                sequence_status:
                    "runtime-owned-durable-fictional-energy-vfx-animated-socket-trails".to_owned(),
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
                input_sha256: digest('q'),
                canonical_sha256: "trails-sequence".to_owned(),
                created_at: "2026-08-22T00:00:00Z".to_owned(),
            },
            trail_bloom: FictionalEnergyVfxAnimatedSocketTrailsBloomSequence {
                schema_version: "FictionalEnergyVfxAnimatedSocketTrailsBloomSequence@1".to_owned(),
                sequence_key_sha256: digest('u'),
                project_id: "project-1".to_owned(),
                candidate_id: "candidate-1".to_owned(),
                candidate_state_sha256: digest('s'),
                delivery_manifest_object_sha256: digest('d'),
                source_artifact_sha256: digest('a'),
                projection_key_sha256: digest('p'),
                projection_object_sha256: digest('b'),
                projection_canonical_sha256: digest('z'),
                animated_socket_materialization_key_sha256: digest('h'),
                animated_artifact_sha256: digest('c'),
                animated_socket_anchor_set_object_sha256: digest('m'),
                animated_socket_anchor_set_canonical_sha256: digest('n'),
                animation_clip_id: "clip-1".to_owned(),
                animation_clip_object_sha256: digest('o'),
                animation_clip_canonical_sha256: digest('q'),
                animation_receipt_object_sha256: digest('f'),
                animation_receipt_canonical_sha256: digest('g'),
                vfx_profile_object_sha256: digest('a'),
                vfx_profile_canonical_sha256: digest('b'),
                socket_node_id_encoding_sha256: digest('r'),
                socket_roles_sha256: digest('v'),
                camera_object_sha256: digest('o'),
                camera_identity_sha256: digest('m'),
                render_profile_sha256: digest('f'),
                render_worker_build_cohort_sha256: digest('w'),
                sample_schedule_sha256: digest('x'),
                sample_count: 1,
                sample_time_ticks: vec![10],
                frame_scope: "lod0-animation-vfx-trail-bloom-frame-range-0@1".to_owned(),
                trails_bloom_sequence_policy: "trail-bound".to_owned(),
                trail_sequence_key_sha256: digest('t'),
                trail_sequence_canonical_sha256: "trails-sequence".to_owned(),
                trail_key_scope: "all".to_owned(),
                trail_count: 1,
                trail_emitter_roles: vec!["muzzle".to_owned()],
                trail_bloom_profile_sha256: digest('v'),
                trail_bloom_profile: serde_json::json!({"effect":"muzzle"}),
                frames: vec![bloom_frame],
                sequence_status:
                    "runtime-owned-durable-fictional-energy-vfx-animated-socket-trails-bloom"
                        .to_owned(),
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
                input_sha256: digest('q'),
                canonical_sha256: digest('u'),
                created_at: "2026-08-22T00:00:00Z".to_owned(),
            },
        }
    }

    fn synthetic_record(
        frames: Vec<FictionalEnergyVfxAnimatedSocketAttachmentV2FrameRecord>,
    ) -> FictionalEnergyVfxAnimatedSocketAttachmentV2Record {
        FictionalEnergyVfxAnimatedSocketAttachmentV2Record {
            schema_version: RECORD_SCHEMA.to_owned(),
            attachment_key_sha256: digest('a'),
            project_id: "project-1".to_owned(),
            delivery_manifest_object_sha256: digest('d'),
            candidate_id: "candidate-1".to_owned(),
            candidate_state_sha256: digest('s'),
            source_artifact_sha256: digest('g'),
            animated_socket_materialization_key_sha256: digest('h'),
            animated_socket_anchor_set_object_sha256: digest('m'),
            animated_socket_anchor_set_canonical_sha256: digest('n'),
            animation_clip_id: "clip-1".to_owned(),
            animation_clip_object_sha256: digest('o'),
            animation_clip_canonical_sha256: digest('q'),
            animated_artifact_sha256: digest('c'),
            animation_receipt_object_sha256: digest('f'),
            animation_receipt_canonical_sha256: digest('g'),
            vfx_profile_object_sha256: digest('v'),
            vfx_profile_canonical_sha256: digest('w'),
            projection_key_sha256: digest('p'),
            projection_object_sha256: digest('b'),
            projection_canonical_sha256: digest('z'),
            particle_sequence_key_sha256: digest('k'),
            particle_sequence_canonical_sha256: digest('l'),
            trail_sequence_key_sha256: digest('t'),
            trail_sequence_canonical_sha256: "trails-sequence".to_owned(),
            trail_bloom_sequence_key_sha256: digest('u'),
            trail_bloom_sequence_canonical_sha256: digest('u'),
            attachment_policy: ATTACHMENT_POLICY.to_owned(),
            socket_node_id_encoding_sha256: digest('r'),
            socket_roles_sha256: digest('v'),
            frame_scope: FRAME_SCOPE.to_owned(),
            frames,
            attachment_status: ATTACHMENT_STATUS.to_owned(),
            canonical_sha256: digest('c'),
            created_at: "2026-08-22T00:00:00Z".to_owned(),
        }
    }

    #[test]
    fn frame_records_use_explicit_current_projection_and_particle_preroll_mapping() {
        let dependencies = synthetic_dependencies();
        let records = frame_records(&digest('a'), &dependencies, "created").unwrap();
        assert_eq!(records.len(), 1);
        let frame = &records[0];
        assert_eq!(frame.frame_index, 0);
        assert_eq!(frame.projection_frame_index, 1);
        assert_eq!(frame.particle_sequence_frame_index, 1);
        assert_eq!(frame.sample_time_ticks, 10);
        assert_eq!(frame.animation_pose_readback_sha256, digest('a'));
        assert_eq!(frame.socket_transform_inventory_sha256, digest('i'));
        assert_eq!(frame.socket_transform_readback_sha256, digest('r'));
        assert_eq!(frame.emitter_socket_bindings_sha256, digest('e'));
        assert_eq!(
            frame.trail_socket_bindings_sha256,
            "trail-emitter-binding-one"
        );
        assert_eq!(frame.base_frame_key_sha256, digest('b'));
        assert_eq!(frame.bloom_key_sha256, digest('l'));
        assert_eq!(frame.particle_key_sha256, "particle-one");
        assert_eq!(frame.trail_key_sha256, "trail-one");
        assert_eq!(frame.trail_bloom_key_sha256, "trail-bloom-one");
        assert_eq!(
            frame.projection_frame_canonical_sha256,
            "projection-frame-one"
        );
        assert_eq!(
            frame.particle_sequence_frame_canonical_sha256,
            "particle-frame-one"
        );
        assert_eq!(
            frame.trail_sequence_frame_canonical_sha256,
            "trail-frame-one"
        );
        assert_eq!(
            frame.trail_bloom_sequence_frame_canonical_sha256,
            "trail-bloom-frame-one"
        );
    }

    #[test]
    fn frame_records_fail_closed_for_preroll_index_canonical_and_sample_mismatch() {
        let mut wrong_index = synthetic_dependencies();
        wrong_index.trails.frames[0].current_projection_frame_index = 0;
        assert!(frame_records(&digest('a'), &wrong_index, "created").is_err());

        let mut wrong_canonical = synthetic_dependencies();
        wrong_canonical.trails.frames[0].current_particle_frame_canonical_sha256 =
            "retargeted-particle-frame".to_owned();
        assert!(frame_records(&digest('a'), &wrong_canonical, "created").is_err());

        let mut wrong_sample = synthetic_dependencies();
        wrong_sample.trails.frames[0].sample_time_ticks = 11;
        assert!(frame_records(&digest('a'), &wrong_sample, "created").is_err());
    }

    #[test]
    fn replay_equivalent_normalizes_parent_and_frame_timestamps_and_canonicals() {
        let dependencies = synthetic_dependencies();
        let frames = frame_records(&digest('a'), &dependencies, "created").unwrap();
        let left = synthetic_record(frames.clone());
        let mut right = left.clone();
        right.created_at = "later".to_owned();
        right.canonical_sha256 = digest('y');
        right.frames[0].created_at = "later-frame".to_owned();
        right.frames[0].canonical_sha256 = digest('x');
        assert!(replay_equivalent(&left, &right).unwrap());

        right.frames[0].trail_key_sha256 = "retargeted-trail".to_owned();
        assert!(!replay_equivalent(&left, &right).unwrap());
    }
}
