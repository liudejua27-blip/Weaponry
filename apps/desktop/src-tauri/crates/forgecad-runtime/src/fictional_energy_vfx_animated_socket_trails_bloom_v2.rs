//! Additive TrailsBloom@2 producer.
//!
//! This module is deliberately separate from the V1 trail Bloom producer.  It
//! consumes the immutable Trails@2 frame, revalidates the Projection@2 /
//! Particles@2 dual lineage through the Trails@2 read path, and performs the
//! bounded five-pass Worker replay before reserving any CAS object.  Only the
//! two trail-specific Bloom passes and their typed sidecars are owned here.

use super::fictional_energy_vfx_animated_socket_trails_v2 as trails;
use super::{
    canonical_json_bytes, canonical_json_hash, exact_object, is_opaque_id, is_sha256, now_string,
    render_worker, sha256_hex, CasObject, Runtime, RuntimeError,
};
use forgecad_contracts::{
    FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceV2,
    FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceV2Frame,
    FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceV2FrameInput,
    FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceV2GetRequest,
    FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceV2PrepareRequest,
    FictionalEnergyVfxAnimatedSocketTrailsSequenceV2,
    FictionalEnergyVfxAnimatedSocketTrailsSequenceV2Frame,
    FictionalEnergyVfxAnimatedSocketTrailsSequenceV2GetRequest,
    FictionalEnergyVfxBloomFrameLinkRecord, FictionalEnergyVfxFrameLinkRecord,
};
use serde_json::{json, Map, Value};

const PREPARE_SCHEMA: &str = "FictionalEnergyVfxAnimatedSocketTrailsBloomSequencePrepareRequest@2";
const GET_SCHEMA: &str = "FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceGetRequest@2";
const PREPARE_RESULT_SCHEMA: &str =
    "FictionalEnergyVfxAnimatedSocketTrailsBloomSequencePrepareResult@2";
const GET_RESULT_SCHEMA: &str = "FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceGetResult@2";
const SEQUENCE_SCHEMA: &str = "FictionalEnergyVfxAnimatedSocketTrailsBloomSequence@2";
const FRAME_SCHEMA: &str = "FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceFrame@2";
const FRAME_RECEIPT_SCHEMA: &str = "FictionalEnergyVfxAnimatedSocketTrailsBloomV2FrameReceipt@1";
const RENDER_SET_SCHEMA: &str = "FictionalEnergyVfxAnimatedSocketTrailsBloomV2RenderSet@1";
const FRAME_SCOPE: &str =
    "lod0-animation-trails-bloom-v2-source-frames-1-15-with-trails-v2-frame-zero-preroll@2";
const POLICY: &str = "projection-v2-driven-animated-socket-trails-bloom-dual-candidate@2";
const HISTORY_POLICY: &str =
    "particles-v2-history-oldest-to-newest-ordinal-zero-last-immediate-previous-earlier-particle-frames-plus-current@2";
const HISTORY_PREROLL_POLICY: &str =
    "same-parent-particles-v2-frame-zero-is-preroll-output-frames-one-to-fifteen@2";
const TRAIL_KEY_SCOPE: &str = "animated-socket-trails-sequence-v2-frame-binding@2";
const STATUS: &str =
    "runtime-owned-durable-fictional-energy-vfx-animated-socket-trails-bloom-sequence-v2";
const MAX_FRAMES: usize = 15;
const MAX_JSON_BYTES: u64 = 1024 * 1024;
const MAX_PNG_BYTES: u64 = 4 * 1024 * 1024;

const RENDER_SET_KIND: &str = "fictional-energy-vfx-animated-socket-trails-bloom-v2-render-set";
const FRAME_RECEIPT_KIND: &str =
    "fictional-energy-vfx-animated-socket-trails-bloom-v2-frame-receipt";
const SEQUENCE_RECEIPT_KIND: &str =
    "fictional-energy-vfx-animated-socket-trails-bloom-v2-sequence-receipt";
const EMISSIVE_KIND: &str =
    "fictional-energy-vfx-animated-socket-trails-bloom-v2-trail-emissive-source";
const CONTRIBUTION_KIND: &str =
    "fictional-energy-vfx-animated-socket-trails-bloom-v2-trail-bloom-contribution";

const ROLES: [&str; 2] = ["muzzle-vfx", "energy-core-vfx"];
const PASS_NAMES: [&str; 5] = [
    "trail-color",
    "trail-id",
    "trail-depth",
    "trail-emissive-source",
    "trail-bloom-contribution",
];

const PREPARE_FIELDS: &[&str] = &[
    "schema_version",
    "sequence_key_sha256",
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
    "projection_key_sha256",
    "projection_object_sha256",
    "projection_canonical_sha256",
    "particle_sequence_key_sha256",
    "particle_sequence_canonical_sha256",
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
    "trails_bloom_sequence_policy",
    "history_policy",
    "history_pre_roll_policy",
    "trail_sequence_key_sha256",
    "trail_sequence_canonical_sha256",
    "trail_key_scope",
    "trail_count",
    "trail_emitter_roles",
    "trail_bloom_profile_sha256",
    "trail_bloom_profile",
    "frames",
    "input_sha256",
    "idempotency_key",
];
const GET_FIELDS: &[&str] = &[
    "schema_version",
    "sequence_key_sha256",
    "project_id",
    "geometry_candidate_id",
    "appearance_candidate_id",
    "geometry_delivery_manifest_object_sha256",
    "appearance_delivery_manifest_object_sha256",
];

fn invalid(detail: impl Into<String>) -> RuntimeError {
    RuntimeError::InvalidInput(format!(
        "FICTIONAL_ENERGY_VFX_ANIMATED_SOCKET_TRAILS_BLOOM_V2_INVALID: {}",
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

fn profile() -> Value {
    json!({
        "threshold":1.0,
        "source_gain":8.0,
        "radius_px":8,
        "intensity":4.0,
        "hdr_clamp":16.0,
        "blur_passes":2,
        "kernel":"separable-box-two-pass-fixed-radius@1"
    })
}

fn profile_hash() -> String {
    canonical_json_hash(&profile())
}

fn parse_prepare(
    value: &Value,
) -> Result<FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceV2PrepareRequest, RuntimeError> {
    let object = exact_object(value, PREPARE_FIELDS, PREPARE_SCHEMA)?;
    if text(object, "schema_version")? != PREPARE_SCHEMA {
        return Err(invalid("prepare schema differs"));
    }
    let request: FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceV2PrepareRequest =
        serde_json::from_value(value.clone())
            .map_err(|error| invalid(format!("prepare request is malformed: {error}")))?;
    for field in [
        "sequence_key_sha256",
        "geometry_candidate_state_sha256",
        "geometry_delivery_manifest_object_sha256",
        "geometry_artifact_sha256",
        "appearance_candidate_state_sha256",
        "appearance_delivery_manifest_object_sha256",
        "appearance_artifact_sha256",
        "material_surface_quality_report_object_sha256",
        "material_surface_quality_canonical_sha256",
        "projection_key_sha256",
        "projection_object_sha256",
        "projection_canonical_sha256",
        "particle_sequence_key_sha256",
        "particle_sequence_canonical_sha256",
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
        "vfx_profile_object_sha256",
        "vfx_profile_canonical_sha256",
        "socket_node_id_encoding_sha256",
        "socket_roles_sha256",
        "camera_object_sha256",
        "camera_identity_sha256",
        "render_profile_sha256",
        "render_worker_build_cohort_sha256",
        "sample_schedule_sha256",
        "trail_sequence_key_sha256",
        "trail_sequence_canonical_sha256",
        "trail_bloom_profile_sha256",
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
    if request.schema_version != PREPARE_SCHEMA
        || request.geometry_candidate_id == request.appearance_candidate_id
        || request.geometry_artifact_sha256 == request.appearance_artifact_sha256
        || request.frame_scope != FRAME_SCOPE
        || request.trails_bloom_sequence_policy != POLICY
        || request.history_policy != HISTORY_POLICY
        || request.history_pre_roll_policy != HISTORY_PREROLL_POLICY
        || request.trail_key_scope != TRAIL_KEY_SCOPE
        || request.sample_count as usize != MAX_FRAMES
        || request.sample_time_ticks.len() != MAX_FRAMES
        || request.frames.len() != MAX_FRAMES
        || request
            .sample_time_ticks
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        || request
            .sample_time_ticks
            .iter()
            .any(|tick| *tick > 1_000_000)
        || request.trail_count != 2
        || request.trail_emitter_roles != ROLES.map(str::to_owned).to_vec()
        || request.trail_bloom_profile != profile()
        || request.trail_bloom_profile_sha256 != profile_hash()
    {
        return Err(invalid(
            "V2 policy/profile or exact 15-frame schedule differs",
        ));
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
) -> Result<FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceV2GetRequest, RuntimeError> {
    let object = exact_object(value, GET_FIELDS, GET_SCHEMA)?;
    if text(object, "schema_version")? != GET_SCHEMA {
        return Err(invalid("get schema differs"));
    }
    let request: FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceV2GetRequest =
        serde_json::from_value(value.clone())
            .map_err(|error| invalid(format!("get request is malformed: {error}")))?;
    sha(object, "sequence_key_sha256")?;
    sha(object, "geometry_delivery_manifest_object_sha256")?;
    sha(object, "appearance_delivery_manifest_object_sha256")?;
    id(object, "project_id")?;
    id(object, "geometry_candidate_id")?;
    id(object, "appearance_candidate_id")?;
    if request.geometry_candidate_id == request.appearance_candidate_id {
        return Err(invalid("V2 get candidates must remain distinct"));
    }
    Ok(request)
}

fn validate_frame_input(
    frame: &FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceV2FrameInput,
    ordinal: usize,
    tick: u64,
) -> Result<(), RuntimeError> {
    if frame.frame_index != ordinal as u64
        || frame.sample_time_ticks != tick
        || frame.trail_frame_index != ordinal as u64
        || frame.current_projection_frame_index != ordinal as u64 + 1
        || frame.current_particle_frame_index != ordinal as u64 + 1
        || frame.trail_sequence_key_sha256.is_empty()
        || !is_sha256(&frame.trail_sequence_key_sha256)
        || !is_sha256(&frame.trail_sequence_canonical_sha256)
        || !is_sha256(&frame.trail_frame_canonical_sha256)
        || !is_sha256(&frame.trail_key_sha256)
        || !is_sha256(&frame.trail_inventory_sha256)
        || !is_sha256(&frame.trail_id_encoding_sha256)
        || !is_sha256(&frame.emitter_binding_sha256)
        || !is_sha256(&frame.particle_sequence_key_sha256)
        || !is_sha256(&frame.particle_sequence_frame_canonical_sha256)
        || !is_sha256(&frame.current_projection_frame_canonical_sha256)
        || !is_sha256(&frame.current_projection_socket_transform_inventory_sha256)
        || !is_sha256(&frame.current_projection_socket_transform_readback_sha256)
        || !is_sha256(&frame.base_frame_key_sha256)
        || !is_sha256(&frame.bloom_key_sha256)
        || !is_sha256(&frame.camera_object_sha256)
        || !is_sha256(&frame.camera_identity_sha256)
        || !is_sha256(&frame.render_profile_sha256)
        || !is_sha256(&frame.render_worker_build_cohort_sha256)
    {
        return Err(invalid("Bloom V2 frame input binding differs"));
    }
    Ok(())
}

fn read_canonical_json(runtime: &Runtime, hash: &str, role: &str) -> Result<Value, RuntimeError> {
    let bytes = runtime.cas_read_bounded(hash, MAX_JSON_BYTES)?;
    if bytes.is_empty() || sha256_hex(&bytes) != hash {
        return Err(invalid(format!("{role} JSON hash differs")));
    }
    let value: Value = serde_json::from_slice(&bytes)
        .map_err(|error| invalid(format!("{role} JSON is malformed: {error}")))?;
    let canonical = canonical_json_bytes(&value).map_err(|error| invalid(error.to_string()))?;
    if canonical != bytes {
        return Err(invalid(format!("{role} JSON is not canonical")));
    }
    Ok(value)
}

fn read_glb(runtime: &Runtime, hash: &str) -> Result<Vec<u8>, RuntimeError> {
    let object = runtime
        .store
        .get_object(hash)?
        .ok_or_else(|| invalid("appearance GLB is unavailable"))?;
    if object.schema_version != "CasObject@1"
        || object.sha256 != hash
        || object.mime != "model/gltf-binary"
        || object.size_bytes == 0
        || object.size_bytes > 64 * 1024 * 1024
        || !matches!(object.kind.as_str(), "appearance-glb" | "appearance-v2-glb")
    {
        return Err(invalid("appearance GLB metadata differs"));
    }
    let bytes = runtime.cas_read_bounded(hash, 64 * 1024 * 1024)?;
    if bytes.is_empty() || bytes.len() as u64 != object.size_bytes || sha256_hex(&bytes) != hash {
        return Err(invalid("appearance GLB bytes differ"));
    }
    Ok(bytes)
}

fn validate_trails_parent(
    request: &FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceV2PrepareRequest,
    source: &FictionalEnergyVfxAnimatedSocketTrailsSequenceV2,
) -> Result<(), RuntimeError> {
    if source.schema_version != "FictionalEnergyVfxAnimatedSocketTrailsSequence@2"
        || source.sequence_key_sha256 != request.trail_sequence_key_sha256
        || source.canonical_sha256 != request.trail_sequence_canonical_sha256
        || source.project_id != request.project_id
        || source.geometry_candidate_id != request.geometry_candidate_id
        || source.appearance_candidate_id != request.appearance_candidate_id
        || source.geometry_candidate_state_sha256 != request.geometry_candidate_state_sha256
        || source.appearance_candidate_state_sha256 != request.appearance_candidate_state_sha256
        || source.geometry_delivery_manifest_object_sha256
            != request.geometry_delivery_manifest_object_sha256
        || source.appearance_delivery_manifest_object_sha256
            != request.appearance_delivery_manifest_object_sha256
        || source.geometry_artifact_sha256 != request.geometry_artifact_sha256
        || source.appearance_artifact_sha256 != request.appearance_artifact_sha256
        || source.material_surface_quality_id != request.material_surface_quality_id
        || source.material_surface_quality_report_object_sha256
            != request.material_surface_quality_report_object_sha256
        || source.material_surface_quality_canonical_sha256
            != request.material_surface_quality_canonical_sha256
        || source.projection_key_sha256 != request.projection_key_sha256
        || source.projection_object_sha256 != request.projection_object_sha256
        || source.projection_canonical_sha256 != request.projection_canonical_sha256
        || source.particle_sequence_key_sha256 != request.particle_sequence_key_sha256
        || source.particle_sequence_canonical_sha256 != request.particle_sequence_canonical_sha256
        || source.animated_socket_materialization_key_sha256
            != request.animated_socket_materialization_key_sha256
        || source.animated_artifact_sha256 != request.animated_artifact_sha256
        || source.animated_socket_anchor_set_object_sha256
            != request.animated_socket_anchor_set_object_sha256
        || source.animated_socket_anchor_set_canonical_sha256
            != request.animated_socket_anchor_set_canonical_sha256
        || source.appearance_anchor_set_object_sha256 != request.appearance_anchor_set_object_sha256
        || source.appearance_anchor_set_canonical_sha256
            != request.appearance_anchor_set_canonical_sha256
        || source.anchor_binding_policy != request.anchor_binding_policy
        || source.animation_clip_id != request.animation_clip_id
        || source.animation_clip_object_sha256 != request.animation_clip_object_sha256
        || source.animation_clip_canonical_sha256 != request.animation_clip_canonical_sha256
        || source.animation_receipt_object_sha256 != request.animation_receipt_object_sha256
        || source.animation_receipt_canonical_sha256 != request.animation_receipt_canonical_sha256
        || source.vfx_profile_object_sha256 != request.vfx_profile_object_sha256
        || source.vfx_profile_canonical_sha256 != request.vfx_profile_canonical_sha256
        || source.socket_node_id_encoding_sha256 != request.socket_node_id_encoding_sha256
        || source.socket_roles_sha256 != request.socket_roles_sha256
        || source.camera_object_sha256 != request.camera_object_sha256
        || source.camera_identity_sha256 != request.camera_identity_sha256
        || source.render_profile_sha256 != request.render_profile_sha256
        || source.render_worker_build_cohort_sha256 != request.render_worker_build_cohort_sha256
        || source.sample_schedule_sha256 != request.sample_schedule_sha256
        || source.sample_count != request.sample_count
        || source.sample_time_ticks != request.sample_time_ticks
        || source.frame_scope
            != "lod0-animation-trails-v2-source-frames-1-15-with-particles-v2-frame-zero-preroll@2"
        || source.trails_sequence_policy
            != "projection-v2-driven-animated-socket-trails-dual-candidate@2"
        || source.history_policy != HISTORY_POLICY
        || source.history_pre_roll_policy != HISTORY_PREROLL_POLICY
        || source.trail_count != 2
        || source.trail_emitter_roles != ROLES.map(str::to_owned).to_vec()
    {
        return Err(invalid("Trails@2 parent binding differs"));
    }
    if source.frames.len() != MAX_FRAMES || source.sample_time_ticks.len() != MAX_FRAMES {
        return Err(invalid("Trails@2 must contain exactly 15 output frames"));
    }
    Ok(())
}

fn get_trails(
    runtime: &Runtime,
    request: &FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceV2PrepareRequest,
) -> Result<
    (
        FictionalEnergyVfxAnimatedSocketTrailsSequenceV2,
        trails::DependencyContext,
    ),
    RuntimeError,
> {
    let trail_request = FictionalEnergyVfxAnimatedSocketTrailsSequenceV2GetRequest {
        schema_version: "FictionalEnergyVfxAnimatedSocketTrailsSequenceGetRequest@2".to_owned(),
        sequence_key_sha256: request.trail_sequence_key_sha256.clone(),
        project_id: request.project_id.clone(),
        geometry_candidate_id: request.geometry_candidate_id.clone(),
        appearance_candidate_id: request.appearance_candidate_id.clone(),
        geometry_delivery_manifest_object_sha256: request
            .geometry_delivery_manifest_object_sha256
            .clone(),
        appearance_delivery_manifest_object_sha256: request
            .appearance_delivery_manifest_object_sha256
            .clone(),
    };
    let (source, context) = trails::get_with_context(runtime, &trail_request)?;
    validate_trails_parent(request, &source)?;
    Ok((source, context))
}

fn load_base_bloom(
    runtime: &Runtime,
    request: &FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceV2PrepareRequest,
    input: &FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceV2FrameInput,
) -> Result<
    (
        FictionalEnergyVfxFrameLinkRecord,
        FictionalEnergyVfxBloomFrameLinkRecord,
    ),
    RuntimeError,
> {
    let base_value = runtime.fictional_energy_vfx_rendered_frame_get(&json!({
        "schema_version":"FictionalEnergyVfxRenderedFrameGetRequest@1",
        "project_id":request.project_id,
        "frame_key_sha256":input.base_frame_key_sha256
    }))?;
    let base: FictionalEnergyVfxFrameLinkRecord = serde_json::from_value(
        base_value
            .get("link")
            .cloned()
            .ok_or_else(|| invalid("base frame is unavailable"))?,
    )
    .map_err(|error| invalid(format!("base frame is malformed: {error}")))?;
    let bloom_value = runtime.fictional_energy_vfx_hdr_bloom_get(&json!({
        "schema_version":"FictionalEnergyVfxHdrBloomFrameGetRequest@1",
        "project_id":request.project_id,
        "bloom_key_sha256":input.bloom_key_sha256
    }))?;
    let bloom: FictionalEnergyVfxBloomFrameLinkRecord = serde_json::from_value(
        bloom_value
            .get("link")
            .cloned()
            .ok_or_else(|| invalid("base Bloom is unavailable"))?,
    )
    .map_err(|error| invalid(format!("base Bloom is malformed: {error}")))?;
    if base.frame_key_sha256 != input.base_frame_key_sha256
        || bloom.bloom_key_sha256 != input.bloom_key_sha256
        || bloom.base_frame_key_sha256 != base.frame_key_sha256
        || base.project_id != request.project_id
        || bloom.project_id != request.project_id
        || base.delivery_manifest_object_sha256
            != request.appearance_delivery_manifest_object_sha256
        || bloom.delivery_manifest_object_sha256
            != request.appearance_delivery_manifest_object_sha256
        || base.source_candidate_id != request.appearance_candidate_id
        || bloom.source_candidate_id != request.appearance_candidate_id
        || base.source_artifact_sha256 != request.appearance_artifact_sha256
        || bloom.source_artifact_sha256 != request.appearance_artifact_sha256
        || base.camera_object_sha256 != request.camera_object_sha256
        || bloom.camera_object_sha256 != request.camera_object_sha256
        || base.camera_identity_sha256 != request.camera_identity_sha256
        || bloom.camera_identity_sha256 != request.camera_identity_sha256
        || base.render_profile_sha256 != request.render_profile_sha256
        || bloom.render_profile_sha256 != request.render_profile_sha256
        || base.render_worker_build_cohort_sha256 != request.render_worker_build_cohort_sha256
        || bloom.render_worker_build_cohort_sha256 != request.render_worker_build_cohort_sha256
        || base.pass_object_sha256s.len() != 9
    {
        return Err(invalid("base/Bloom frame binding differs"));
    }
    for hash in &base.pass_object_sha256s {
        let bytes = runtime.cas_read_bounded(hash, MAX_PNG_BYTES)?;
        if sha256_hex(&bytes) != *hash {
            return Err(invalid("base AOV bytes differ"));
        }
    }
    for hash in [
        &bloom.source_object_sha256,
        &bloom.contribution_object_sha256,
    ] {
        let bytes = runtime.cas_read_bounded(hash, MAX_PNG_BYTES)?;
        if sha256_hex(&bytes) != *hash {
            return Err(invalid("base Bloom bytes differ"));
        }
    }
    Ok((base, bloom))
}

fn trail_wire_from_inventory(inventory: &Value) -> Result<Value, RuntimeError> {
    let trails = inventory
        .get("trails")
        .and_then(Value::as_array)
        .filter(|values| values.len() == 2)
        .ok_or_else(|| invalid("Trails@2 worker inventory is malformed"))?;
    let mut result = Vec::with_capacity(2);
    for (index, value) in trails.iter().enumerate() {
        let points = value
            .get("points")
            .and_then(Value::as_array)
            .ok_or_else(|| invalid("Trails@2 inventory points are unavailable"))?;
        let local_points = points
            .iter()
            .map(|point| {
                let object = point
                    .as_object()
                    .ok_or_else(|| invalid("Trails@2 inventory point is malformed"))?;
                Ok(json!({
                    "frame_index":object.get("frame_index").cloned().ok_or_else(|| invalid("trail point frame is missing"))?,
                    "sample_time_ticks":object.get("sample_time_ticks").cloned().ok_or_else(|| invalid("trail point tick is missing"))?,
                    "source_particle_key_sha256":object.get("source_particle_key_sha256").cloned().ok_or_else(|| invalid("trail point particle key is missing"))?,
                    "source_particle_id":object.get("source_particle_id").cloned().ok_or_else(|| invalid("trail point particle id is missing"))?,
                    "local_offset_m":object.get("local_offset_m").cloned().ok_or_else(|| invalid("trail point local offset is missing"))?
                }))
            })
            .collect::<Result<Vec<_>, RuntimeError>>()?;
        let emitter = if index == 0 {
            "muzzle-trail"
        } else {
            "energy-core-trail"
        };
        let id = if index == 0 { 30_000_u64 } else { 31_000_u64 };
        result.push(json!({
            "emitter_id":emitter,
            "id":id,
            "local_points":local_points,
            "radius_px":value.get("radius_px").cloned().ok_or_else(|| invalid("trail radius is missing"))?,
            "color_linear_rgb":value.get("color_linear_rgb").cloned().ok_or_else(|| invalid("trail color is missing"))?,
            "alpha":value.get("alpha").cloned().ok_or_else(|| invalid("trail alpha is missing"))?,
            "lifetime_ticks":value.get("lifetime_ticks").cloned().ok_or_else(|| invalid("trail lifetime is missing"))?
        }));
    }
    Ok(Value::Array(result))
}

fn bloom_seed(
    request: &FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceV2PrepareRequest,
    source_frame: &FictionalEnergyVfxAnimatedSocketTrailsSequenceV2Frame,
    base_depth: &str,
) -> String {
    canonical_json_hash(&json!({
        "schema_version":"RenderWorkerAnimatedSocketTrailsBloomV2Seed@1",
        "trail_sequence_key_sha256":request.trail_sequence_key_sha256,
        "trail_frame_canonical_sha256":source_frame.canonical_sha256,
        "frame_index":source_frame.frame_index,
        "trail_seed_sha256":source_frame.trail_seed_sha256,
        "trail_bloom_profile_sha256":request.trail_bloom_profile_sha256,
        "base_opaque_depth_object_sha256":base_depth
    }))
}

fn bloom_key(
    request: &FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceV2PrepareRequest,
    source_frame: &FictionalEnergyVfxAnimatedSocketTrailsSequenceV2Frame,
    computation: &BloomFrameComputation,
    seed: &str,
) -> String {
    canonical_json_hash(&json!({
        "schema_version":"FictionalEnergyVfxAnimatedSocketTrailsBloomV2Key@1",
        "sequence_key_sha256":request.sequence_key_sha256,
        "frame_index":source_frame.frame_index,
        "trail_frame_canonical_sha256":source_frame.canonical_sha256,
        "trail_passes":[source_frame.trail_color_object_sha256,source_frame.trail_id_object_sha256,source_frame.trail_depth_object_sha256],
        "base_frame_key_sha256":source_frame.base_frame_key_sha256,
        "base_opaque_depth_object_sha256":computation.base_depth_sha256,
        "bloom_key_sha256":source_frame.bloom_key_sha256,
        "camera_object_sha256":request.camera_object_sha256,
        "render_profile_sha256":request.render_profile_sha256,
        "render_worker_build_cohort_sha256":request.render_worker_build_cohort_sha256,
        "trail_bloom_profile_sha256":request.trail_bloom_profile_sha256,
        "trail_bloom_seed_sha256":seed,
        "projection_sample_set_sha256":computation.worker.projection_sample_set_sha256,
        "emitter_binding_sha256":computation.worker.emitter_binding_sha256,
        "trail_inventory_sha256":computation.worker.trail_inventory_sha256
    }))
}

#[derive(Debug, Clone)]
struct BloomFrameComputation {
    base_depth_sha256: String,
    worker: render_worker::RenderWorkerAnimatedSocketTrailsBloomFrame,
    seed_sha256: String,
    key_sha256: String,
}

fn replay_worker(
    runtime: &Runtime,
    request: &FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceV2PrepareRequest,
    source_frame: &FictionalEnergyVfxAnimatedSocketTrailsSequenceV2Frame,
    context: &trails::TrailFrameContext,
    source_glb: &[u8],
    base: &FictionalEnergyVfxFrameLinkRecord,
) -> Result<BloomFrameComputation, RuntimeError> {
    let wire = trail_wire_from_inventory(&context.worker.trail_inventory)?;
    let first = render_worker::render_typed_animated_socket_trails_bloom_with_worker_identity(
        source_glb,
        &camera_from_base(runtime, &base.camera_object_sha256)?,
        &request.projection_key_sha256,
        &context.worker.projection_input_sha256,
        context.input.current_projection_frame_index,
        context.input.sample_time_ticks,
        &context.projection_samples,
        &wire,
        &context.worker.seed_sha256,
        render_worker::TypedTrailBloomProfile::FIXED,
    )
    .map_err(|error| invalid(format!("Bloom Worker replay failed: {error}")))?;
    let second = render_worker::render_typed_animated_socket_trails_bloom_with_worker_identity(
        source_glb,
        &camera_from_base(runtime, &base.camera_object_sha256)?,
        &request.projection_key_sha256,
        &context.worker.projection_input_sha256,
        context.input.current_projection_frame_index,
        context.input.sample_time_ticks,
        &context.projection_samples,
        &wire,
        &context.worker.seed_sha256,
        render_worker::TypedTrailBloomProfile::FIXED,
    )
    .map_err(|error| invalid(format!("Bloom Worker second replay failed: {error}")))?;
    if first.trail_bloom_passes.len() != 5
        || second.trail_bloom_passes.len() != 5
        || first
            .trail_bloom_passes
            .iter()
            .zip(&second.trail_bloom_passes)
            .any(|(left, right)| left.pass != right.pass || left.png != right.png)
        || first.trail_bloom_passes[0].png != context.worker.trail_passes[0].png
        || first.trail_bloom_passes[1].png != context.worker.trail_passes[1].png
        || first.trail_bloom_passes[2].png != context.worker.trail_passes[2].png
        || first.build_cohort_sha256.as_deref()
            != Some(request.render_worker_build_cohort_sha256.as_str())
        || second.build_cohort_sha256 != first.build_cohort_sha256
        || first.current_frame_index != context.input.current_projection_frame_index
        || first.current_sample_time_ticks != context.input.sample_time_ticks
        || first.projection_input_sha256 != context.worker.projection_input_sha256
        || first.projection_sample_set_sha256 != context.worker.projection_sample_set_sha256
        || first.emitter_binding_sha256 != context.worker.emitter_binding_sha256
        || first.trail_inventory_sha256 != context.worker.trail_inventory_sha256
        || first.trail_inventory != context.worker.trail_inventory
        || first.trail_bloom_profile != render_worker::TypedTrailBloomProfile::FIXED
    {
        return Err(invalid("Bloom Worker replay is not same-cohort byte exact"));
    }
    for (index, hash) in [
        &source_frame.trail_color_object_sha256,
        &source_frame.trail_id_object_sha256,
        &source_frame.trail_depth_object_sha256,
    ]
    .into_iter()
    .enumerate()
    {
        let bytes = runtime.cas_read_bounded(hash, MAX_PNG_BYTES)?;
        if sha256_hex(&bytes) != *hash || bytes != context.worker.trail_passes[index].png {
            return Err(invalid("Trails@2 source pass bytes differ"));
        }
    }
    let base_depth = base.pass_object_sha256s[2].clone();
    let seed = bloom_seed(request, source_frame, &base_depth);
    let mut computation = BloomFrameComputation {
        base_depth_sha256: base_depth,
        worker: first,
        seed_sha256: seed.clone(),
        key_sha256: String::new(),
    };
    computation.key_sha256 = bloom_key(request, source_frame, &computation, &seed);
    Ok(computation)
}

fn camera_from_base(runtime: &Runtime, hash: &str) -> Result<Value, RuntimeError> {
    let camera = read_canonical_json(runtime, hash, "camera")?;
    if camera.get("transform").is_none() || camera.get("near_m").is_none() {
        return Err(invalid("camera calibration is incomplete"));
    }
    Ok(camera)
}

fn source_context(
    runtime: &Runtime,
    request: &FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceV2PrepareRequest,
) -> Result<
    (
        FictionalEnergyVfxAnimatedSocketTrailsSequenceV2,
        trails::DependencyContext,
        Vec<BloomFrameComputation>,
    ),
    RuntimeError,
> {
    let (source, context) = get_trails(runtime, request)?;
    if context.frames.len() != MAX_FRAMES
        || context.worker_cohort != request.render_worker_build_cohort_sha256
    {
        return Err(invalid("Trails@2 context cohort or frame count differs"));
    }
    let source_glb = read_glb(runtime, &request.appearance_artifact_sha256)?;
    let mut computations = Vec::with_capacity(MAX_FRAMES);
    for (index, input) in request.frames.iter().enumerate() {
        let source_frame = source
            .frames
            .get(index)
            .ok_or_else(|| invalid("Trails@2 source frame is missing"))?;
        let context_frame = context
            .frames
            .get(index)
            .ok_or_else(|| invalid("Trails@2 worker frame is missing"))?;
        if source_frame.frame_index != index as u64
            || source_frame.sample_time_ticks != input.sample_time_ticks
            || source_frame.canonical_sha256 != input.trail_frame_canonical_sha256
            || source_frame.trail_key_sha256 != input.trail_key_sha256
            || source_frame.trail_inventory_sha256 != input.trail_inventory_sha256
            || source_frame.trail_id_encoding_sha256 != input.trail_id_encoding_sha256
            || source_frame.emitter_binding_sha256 != input.emitter_binding_sha256
            || source_frame.current_projection_frame_index != input.current_projection_frame_index
            || source_frame.current_particle_frame_index != input.current_particle_frame_index
            || source_frame.current_projection_frame_canonical_sha256
                != input.current_projection_frame_canonical_sha256
            || context_frame.worker.build_cohort_sha256.as_deref()
                != Some(request.render_worker_build_cohort_sha256.as_str())
        {
            return Err(invalid("Trails@2 frame lineage differs"));
        }
        let (base, _bloom) = load_base_bloom(runtime, request, input)?;
        let computation = replay_worker(
            runtime,
            request,
            source_frame,
            context_frame,
            &source_glb,
            &base,
        )?;
        if computation.key_sha256.is_empty() {
            return Err(invalid("Bloom key derivation is empty"));
        }
        computations.push(computation);
    }
    Ok((source, context, computations))
}

fn pass_metadata(hash: &str, size: usize, pass: &str) -> Value {
    json!({
        "pass":pass,
        "sha256":hash,
        "mime":"image/png",
        "size_bytes":size as u64,
        "width":512,
        "height":512,
        "channels":"rgba8",
        "color_space":"data"
    })
}

fn contribution_values(
    source: &FictionalEnergyVfxAnimatedSocketTrailsSequenceV2Frame,
    output_hash: &str,
) -> Vec<Value> {
    source
        .trails
        .iter()
        .enumerate()
        .map(|(index, trail)| {
            let role = ROLES.get(index).copied().unwrap_or("muzzle-vfx");
            let digest = canonical_json_hash(&json!({
                "schema_version":"FictionalEnergyVfxAnimatedSocketTrailsBloomV2Contribution@1",
                "emitter_role":role,
                "trail_id":trail.trail_id,
                "trail_key_sha256":source.trail_key_sha256,
                "trail_frame_canonical_sha256":source.canonical_sha256,
                "trail_bloom_contribution_object_sha256":output_hash
            }));
            json!({
                "emitter_role":role,
                "trail_id":trail.trail_id,
                "trail_key_sha256":source.trail_key_sha256,
                "trail_frame_canonical_sha256":source.canonical_sha256,
                "trail_bloom_contribution_sha256":digest
            })
        })
        .collect()
}

fn make_frame(
    request: &FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceV2PrepareRequest,
    source: &FictionalEnergyVfxAnimatedSocketTrailsSequenceV2Frame,
    computation: &BloomFrameComputation,
    output_hashes: [&str; 2],
    render_set_hash: &str,
    receipt_hash: &str,
    created_at: &str,
) -> FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceV2Frame {
    let mut frame = FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceV2Frame {
        schema_version: FRAME_SCHEMA.to_owned(),
        frame_index: source.frame_index,
        sample_time_ticks: source.sample_time_ticks,
        trail_frame_index: source.frame_index,
        trail_sequence_key_sha256: request.trail_sequence_key_sha256.clone(),
        trail_sequence_canonical_sha256: request.trail_sequence_canonical_sha256.clone(),
        trail_frame_canonical_sha256: source.canonical_sha256.clone(),
        trail_key_sha256: source.trail_key_sha256.clone(),
        trail_inventory_sha256: source.trail_inventory_sha256.clone(),
        trail_id_encoding_sha256: source.trail_id_encoding_sha256.clone(),
        emitter_binding_sha256: source.emitter_binding_sha256.clone(),
        trail_color_object_sha256: source.trail_color_object_sha256.clone(),
        trail_id_object_sha256: source.trail_id_object_sha256.clone(),
        trail_depth_object_sha256: source.trail_depth_object_sha256.clone(),
        particle_sequence_key_sha256: source.particle_sequence_key_sha256.clone(),
        particle_sequence_frame_canonical_sha256: source
            .current_particle_frame_canonical_sha256
            .clone(),
        current_projection_frame_index: source.current_projection_frame_index,
        current_particle_frame_index: source.current_particle_frame_index,
        current_projection_frame_canonical_sha256: source
            .current_projection_frame_canonical_sha256
            .clone(),
        current_projection_socket_transform_inventory_sha256: source
            .current_projection_socket_transform_inventory_sha256
            .clone(),
        current_projection_socket_transform_readback_sha256: source
            .current_projection_socket_transform_readback_sha256
            .clone(),
        base_frame_key_sha256: source.base_frame_key_sha256.clone(),
        bloom_key_sha256: source.bloom_key_sha256.clone(),
        camera_object_sha256: request.camera_object_sha256.clone(),
        camera_identity_sha256: request.camera_identity_sha256.clone(),
        render_profile_sha256: request.render_profile_sha256.clone(),
        render_worker_build_cohort_sha256: request.render_worker_build_cohort_sha256.clone(),
        trail_bloom_profile_sha256: request.trail_bloom_profile_sha256.clone(),
        base_opaque_depth_object_sha256: computation.base_depth_sha256.clone(),
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
        trail_bloom_key_sha256: computation.key_sha256.clone(),
        trail_bloom_seed_sha256: computation.seed_sha256.clone(),
        trail_bloom_contributions: contribution_values(source, output_hashes[1])
            .into_iter()
            .map(|value| serde_json::from_value(value).expect("contribution is typed"))
            .collect(),
        trail_emissive_source_object_sha256: output_hashes[0].to_owned(),
        trail_bloom_contribution_object_sha256: output_hashes[1].to_owned(),
        render_set_object_sha256: render_set_hash.to_owned(),
        receipt_object_sha256: receipt_hash.to_owned(),
        canonical_sha256: String::new(),
        created_at: created_at.to_owned(),
    };
    frame.canonical_sha256 = canonical_json_hash(
        &serde_json::to_value(&frame).expect("Bloom V2 frame serialization is infallible"),
    );
    frame
}

fn frame_without_receipt(
    frame: &FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceV2Frame,
) -> Value {
    let mut value =
        serde_json::to_value(frame).expect("Bloom V2 frame serialization is infallible");
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

fn make_sequence(
    request: &FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceV2PrepareRequest,
    source: &FictionalEnergyVfxAnimatedSocketTrailsSequenceV2,
    frames: Vec<FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceV2Frame>,
) -> FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceV2 {
    let mut sequence = FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceV2 {
        schema_version: SEQUENCE_SCHEMA.to_owned(),
        sequence_key_sha256: request.sequence_key_sha256.clone(),
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
        geometry_preservation_projection_sha256: source
            .geometry_preservation_projection_sha256
            .clone(),
        geometry_preservation_status: source.geometry_preservation_status.clone(),
        projection_key_sha256: request.projection_key_sha256.clone(),
        projection_object_sha256: request.projection_object_sha256.clone(),
        projection_canonical_sha256: request.projection_canonical_sha256.clone(),
        particle_sequence_key_sha256: request.particle_sequence_key_sha256.clone(),
        particle_sequence_canonical_sha256: request.particle_sequence_canonical_sha256.clone(),
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
        anchor_binding_sha256: source.anchor_binding_sha256.clone(),
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
        render_worker_build_cohort_sha256: request.render_worker_build_cohort_sha256.clone(),
        sample_schedule_sha256: request.sample_schedule_sha256.clone(),
        sample_count: request.sample_count,
        sample_time_ticks: request.sample_time_ticks.clone(),
        frame_scope: FRAME_SCOPE.to_owned(),
        trails_bloom_sequence_policy: POLICY.to_owned(),
        history_policy: HISTORY_POLICY.to_owned(),
        history_pre_roll_policy: HISTORY_PREROLL_POLICY.to_owned(),
        trail_sequence_key_sha256: source.sequence_key_sha256.clone(),
        trail_sequence_canonical_sha256: source.canonical_sha256.clone(),
        trail_key_scope: TRAIL_KEY_SCOPE.to_owned(),
        trail_count: 2,
        trail_emitter_roles: ROLES.map(str::to_owned).to_vec(),
        trail_bloom_profile_sha256: request.trail_bloom_profile_sha256.clone(),
        trail_bloom_profile: profile(),
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
        &serde_json::to_value(&sequence).expect("Bloom V2 sequence serialization is infallible"),
    );
    sequence
}

fn result_value(
    sequence: &FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceV2,
    replayed: bool,
    schema: &str,
    runtime_write: bool,
) -> Value {
    json!({
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
    })
}

fn request_matches_sequence(
    request: &FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceV2PrepareRequest,
    sequence: &FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceV2,
) -> bool {
    request.sequence_key_sha256 == sequence.sequence_key_sha256
        && request.project_id == sequence.project_id
        && request.geometry_candidate_id == sequence.geometry_candidate_id
        && request.appearance_candidate_id == sequence.appearance_candidate_id
        && request.geometry_candidate_state_sha256 == sequence.geometry_candidate_state_sha256
        && request.appearance_candidate_state_sha256 == sequence.appearance_candidate_state_sha256
        && request.geometry_delivery_manifest_object_sha256
            == sequence.geometry_delivery_manifest_object_sha256
        && request.appearance_delivery_manifest_object_sha256
            == sequence.appearance_delivery_manifest_object_sha256
        && request.geometry_artifact_sha256 == sequence.geometry_artifact_sha256
        && request.appearance_artifact_sha256 == sequence.appearance_artifact_sha256
        && request.material_surface_quality_id == sequence.material_surface_quality_id
        && request.material_surface_quality_report_object_sha256
            == sequence.material_surface_quality_report_object_sha256
        && request.material_surface_quality_canonical_sha256
            == sequence.material_surface_quality_canonical_sha256
        && request.projection_key_sha256 == sequence.projection_key_sha256
        && request.projection_object_sha256 == sequence.projection_object_sha256
        && request.projection_canonical_sha256 == sequence.projection_canonical_sha256
        && request.particle_sequence_key_sha256 == sequence.particle_sequence_key_sha256
        && request.particle_sequence_canonical_sha256 == sequence.particle_sequence_canonical_sha256
        && request.trail_sequence_key_sha256 == sequence.trail_sequence_key_sha256
        && request.trail_sequence_canonical_sha256 == sequence.trail_sequence_canonical_sha256
        && request.render_worker_build_cohort_sha256 == sequence.render_worker_build_cohort_sha256
        && request.sample_time_ticks == sequence.sample_time_ticks
        && request.frames.len() == sequence.frames.len()
}

fn render_set_value(
    request: &FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceV2PrepareRequest,
    source: &FictionalEnergyVfxAnimatedSocketTrailsSequenceV2Frame,
    computation: &BloomFrameComputation,
    output_hashes: [&str; 2],
) -> Result<(Value, Vec<u8>), RuntimeError> {
    trails::canonical_object(json!({
        "schema_version":RENDER_SET_SCHEMA,
        "sequence_key_sha256":request.sequence_key_sha256,
        "frame_index":source.frame_index,
        "sample_time_ticks":source.sample_time_ticks,
        "trail_sequence_key_sha256":request.trail_sequence_key_sha256,
        "trail_sequence_canonical_sha256":request.trail_sequence_canonical_sha256,
        "trail_frame_canonical_sha256":source.canonical_sha256,
        "trail_bloom_key_sha256":computation.key_sha256,
        "trail_bloom_seed_sha256":computation.seed_sha256,
        "trail_bloom_profile_sha256":request.trail_bloom_profile_sha256,
        "trail_bloom_profile":profile(),
        "base_frame_key_sha256":source.base_frame_key_sha256,
        "base_opaque_depth_object_sha256":computation.base_depth_sha256,
        "bloom_key_sha256":source.bloom_key_sha256,
        "camera_object_sha256":request.camera_object_sha256,
        "camera_identity_sha256":request.camera_identity_sha256,
        "render_profile_sha256":request.render_profile_sha256,
        "render_worker_build_cohort_sha256":request.render_worker_build_cohort_sha256,
        "passes":PASS_NAMES,
        "pass_artifacts":[
            pass_metadata(&source.trail_color_object_sha256, computation.worker.trail_bloom_passes[0].png.len(), PASS_NAMES[0]),
            pass_metadata(&source.trail_id_object_sha256, computation.worker.trail_bloom_passes[1].png.len(), PASS_NAMES[1]),
            pass_metadata(&source.trail_depth_object_sha256, computation.worker.trail_bloom_passes[2].png.len(), PASS_NAMES[2]),
            pass_metadata(output_hashes[0], computation.worker.trail_bloom_passes[3].png.len(), PASS_NAMES[3]),
            pass_metadata(output_hashes[1], computation.worker.trail_bloom_passes[4].png.len(), PASS_NAMES[4])
        ],
        "first_three_passes_byte_exact":true,
        "base_aov_byte_exact_verified":true,
        "runtime_write_performed":true,
        "quality_status":"structural_only",
        "visual_quality_status":"NOT_PROVEN",
        "canonical_sha256":""
    }))
}

fn receipt_value(
    request: &FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceV2PrepareRequest,
    source: &FictionalEnergyVfxAnimatedSocketTrailsSequenceV2Frame,
    computation: &BloomFrameComputation,
    frame: Value,
    output_hashes: [&str; 2],
) -> Result<(Value, Vec<u8>), RuntimeError> {
    trails::canonical_object(json!({
        "schema_version":FRAME_RECEIPT_SCHEMA,
        "sequence_key_sha256":request.sequence_key_sha256,
        "frame":frame,
        "trail_sequence_key_sha256":request.trail_sequence_key_sha256,
        "trail_sequence_canonical_sha256":request.trail_sequence_canonical_sha256,
        "trail_frame_canonical_sha256":source.canonical_sha256,
        "trail_passes":[source.trail_color_object_sha256,source.trail_id_object_sha256,source.trail_depth_object_sha256],
        "base_frame_key_sha256":source.base_frame_key_sha256,
        "base_opaque_depth_object_sha256":computation.base_depth_sha256,
        "bloom_key_sha256":source.bloom_key_sha256,
        "trail_bloom_key_sha256":computation.key_sha256,
        "trail_bloom_seed_sha256":computation.seed_sha256,
        "trail_bloom_profile_sha256":request.trail_bloom_profile_sha256,
        "trail_bloom_profile":profile(),
        "trail_emissive_source_object_sha256":output_hashes[0],
        "trail_bloom_contribution_object_sha256":output_hashes[1],
        "trail_bloom_passes":[
            pass_metadata(&source.trail_color_object_sha256, computation.worker.trail_bloom_passes[0].png.len(), PASS_NAMES[0]),
            pass_metadata(&source.trail_id_object_sha256, computation.worker.trail_bloom_passes[1].png.len(), PASS_NAMES[1]),
            pass_metadata(&source.trail_depth_object_sha256, computation.worker.trail_bloom_passes[2].png.len(), PASS_NAMES[2]),
            pass_metadata(output_hashes[0], computation.worker.trail_bloom_passes[3].png.len(), PASS_NAMES[3]),
            pass_metadata(output_hashes[1], computation.worker.trail_bloom_passes[4].png.len(), PASS_NAMES[4])
        ],
        "worker_replay_byte_exact":true,
        "first_three_passes_byte_exact":true,
        "runtime_write_performed":true,
        "candidate_confirmed":false,
        "actual_engine_roundtrip":false,
        "quality_status":"structural_only",
        "visual_quality_status":"NOT_PROVEN",
        "human_review_status":"NOT_RUN",
        "commercial_engine_status":"NOT_RUN",
        "canonical_sha256":""
    }))
}

fn stored_outputs_match(
    runtime: &Runtime,
    request: &FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceV2PrepareRequest,
    source: &FictionalEnergyVfxAnimatedSocketTrailsSequenceV2,
    context: &trails::DependencyContext,
    computations: &[BloomFrameComputation],
    stored: &FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceV2,
) -> Result<(), RuntimeError> {
    if !request_matches_sequence(request, stored)
        || stored.sequence_status != STATUS
        || stored.quality_status != "structural_only"
        || stored.visual_quality_status != "NOT_PROVEN"
        || stored.frames.len() != MAX_FRAMES
    {
        return Err(invalid("stored Bloom V2 parent differs"));
    }
    for (index, frame) in stored.frames.iter().enumerate() {
        let source_frame = source
            .frames
            .get(index)
            .ok_or_else(|| invalid("stored Trails@2 frame is missing"))?;
        let context_frame = context
            .frames
            .get(index)
            .ok_or_else(|| invalid("stored Trails@2 context frame is missing"))?;
        let computation = computations
            .get(index)
            .ok_or_else(|| invalid("stored Bloom computation is missing"))?;
        if frame.trail_frame_canonical_sha256 != source_frame.canonical_sha256
            || frame.trail_color_object_sha256 != source_frame.trail_color_object_sha256
            || frame.trail_id_object_sha256 != source_frame.trail_id_object_sha256
            || frame.trail_depth_object_sha256 != source_frame.trail_depth_object_sha256
            || frame.trail_bloom_key_sha256 != computation.key_sha256
            || frame.trail_bloom_seed_sha256 != computation.seed_sha256
            || frame.base_opaque_depth_object_sha256 != computation.base_depth_sha256
            || frame.render_worker_build_cohort_sha256 != request.render_worker_build_cohort_sha256
        {
            return Err(invalid("stored Bloom V2 frame binding differs"));
        }
        let hashes = [
            &frame.trail_emissive_source_object_sha256,
            &frame.trail_bloom_contribution_object_sha256,
        ];
        for (offset, hash) in hashes.into_iter().enumerate() {
            let bytes = runtime.cas_read_bounded(hash, MAX_PNG_BYTES)?;
            if sha256_hex(&bytes) != *hash
                || bytes != computation.worker.trail_bloom_passes[offset + 3].png
            {
                return Err(invalid("stored Bloom V2 output bytes differ"));
            }
        }
        let (render_set, _) = render_set_value(
            request,
            source_frame,
            computation,
            [
                &frame.trail_emissive_source_object_sha256,
                &frame.trail_bloom_contribution_object_sha256,
            ],
        )?;
        if read_canonical_json(
            runtime,
            &frame.render_set_object_sha256,
            "Bloom V2 render set",
        )? != render_set
        {
            return Err(invalid("stored Bloom V2 render set differs"));
        }
        let receipt_frame = frame_without_receipt(frame);
        let (receipt, _) = receipt_value(
            request,
            source_frame,
            computation,
            receipt_frame,
            [
                &frame.trail_emissive_source_object_sha256,
                &frame.trail_bloom_contribution_object_sha256,
            ],
        )?;
        if read_canonical_json(
            runtime,
            &frame.receipt_object_sha256,
            "Bloom V2 frame receipt",
        )? != receipt
        {
            return Err(invalid("stored Bloom V2 frame receipt differs"));
        }
        if context_frame.worker.trail_passes.len() != 3 {
            return Err(invalid("Trails@2 source worker pass inventory differs"));
        }
    }
    Ok(())
}

fn write_sequence(
    runtime: &Runtime,
    request: &FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceV2PrepareRequest,
    source: &FictionalEnergyVfxAnimatedSocketTrailsSequenceV2,
    _context: &trails::DependencyContext,
    computations: &[BloomFrameComputation],
) -> Result<Value, RuntimeError> {
    let reservation = runtime.store.begin_cas_reservation();
    let mut reserved_objects: Vec<CasObject> = Vec::new();
    let operation = (|| -> Result<Value, RuntimeError> {
        let mut frames = Vec::with_capacity(MAX_FRAMES);
        for (index, computation) in computations.iter().enumerate() {
            let source_frame = source
                .frames
                .get(index)
                .ok_or_else(|| invalid("Trails@2 source frame is missing during write"))?;
            let created_at = now_string();
            let mut outputs = [String::new(), String::new()];
            for (output_index, kind) in [EMISSIVE_KIND, CONTRIBUTION_KIND].into_iter().enumerate() {
                let pass = &computation.worker.trail_bloom_passes[output_index + 3];
                let object = runtime.store.put_object_reserved(
                    &reservation,
                    &pass.png,
                    None,
                    "image/png",
                    kind,
                    &created_at,
                )?;
                outputs[output_index] = object.record.sha256.clone();
                reserved_objects.push(object);
            }
            let render_set = render_set_value(
                request,
                source_frame,
                computation,
                [&outputs[0], &outputs[1]],
            )?;
            let render_set_object = runtime.store.put_object_reserved(
                &reservation,
                &render_set.1,
                None,
                "application/json",
                RENDER_SET_KIND,
                &created_at,
            )?;
            let render_set_hash = render_set_object.record.sha256.clone();
            reserved_objects.push(render_set_object);
            let provisional = make_frame(
                request,
                source_frame,
                computation,
                [&outputs[0], &outputs[1]],
                &render_set_hash,
                "",
                &created_at,
            );
            let receipt = receipt_value(
                request,
                source_frame,
                computation,
                frame_without_receipt(&provisional),
                [&outputs[0], &outputs[1]],
            )?;
            let receipt_object = runtime.store.put_object_reserved(
                &reservation,
                &receipt.1,
                None,
                "application/json",
                FRAME_RECEIPT_KIND,
                &created_at,
            )?;
            let receipt_hash = receipt_object.record.sha256.clone();
            reserved_objects.push(receipt_object);
            frames.push(make_frame(
                request,
                source_frame,
                computation,
                [&outputs[0], &outputs[1]],
                &render_set_hash,
                &receipt_hash,
                &created_at,
            ));
        }
        let sequence = make_sequence(request, source, frames);
        let sequence_value = serde_json::to_value(&sequence)
            .map_err(|error| invalid(format!("Bloom V2 sequence serialization failed: {error}")))?;
        let sequence_bytes = canonical_json_bytes(&sequence_value).map_err(|error| {
            invalid(format!(
                "Bloom V2 sequence canonicalization failed: {error}"
            ))
        })?;
        if sequence_bytes.len() as u64 > MAX_JSON_BYTES {
            return Err(invalid("Bloom V2 sequence receipt exceeds one MiB"));
        }
        let sequence_receipt = runtime.store.put_object_reserved(
            &reservation,
            &sequence_bytes,
            None,
            "application/json",
            SEQUENCE_RECEIPT_KIND,
            &sequence.created_at,
        )?;
        reserved_objects.push(sequence_receipt.clone());
        let stored = runtime
            .store
            .record_fictional_energy_vfx_animated_socket_trails_bloom_sequence_v2(
                &sequence,
                &sequence_receipt.record,
            )?;
        for object in &reserved_objects {
            runtime
                .store
                .release_cas_reservation_object(&reservation, object, false)?;
        }
        Ok(result_value(&stored, false, PREPARE_RESULT_SCHEMA, true))
    })();
    match operation {
        Ok(value) => Ok(value),
        Err(error) => {
            for object in reserved_objects.iter().rev() {
                let _ = runtime.store.release_cas_reservation_object(
                    &reservation,
                    object,
                    object.created_new,
                );
            }
            Err(error)
        }
    }
}

pub(super) fn prepare(runtime: &Runtime, value: &Value) -> Result<Value, RuntimeError> {
    let request = parse_prepare(value)?;
    let (source, context, computations) = source_context(runtime, &request)?;
    if let Some(existing) = runtime
        .store
        .get_fictional_energy_vfx_animated_socket_trails_bloom_sequence_v2(
            &request.sequence_key_sha256,
        )?
    {
        if !request_matches_sequence(&request, &existing) {
            return Err(invalid("existing Bloom V2 binding differs"));
        }
        stored_outputs_match(
            runtime,
            &request,
            &source,
            &context,
            &computations,
            &existing,
        )?;
        return Ok(result_value(&existing, true, PREPARE_RESULT_SCHEMA, true));
    }
    write_sequence(runtime, &request, &source, &context, &computations)
}

pub(super) fn get(runtime: &Runtime, value: &Value) -> Result<Value, RuntimeError> {
    let request = parse_get(value)?;
    let stored = runtime
        .store
        .get_fictional_energy_vfx_animated_socket_trails_bloom_sequence_v2(
            &request.sequence_key_sha256,
        )?
        .ok_or_else(|| invalid("Bloom V2 sequence is unavailable"))?;
    if stored.project_id != request.project_id
        || stored.geometry_candidate_id != request.geometry_candidate_id
        || stored.appearance_candidate_id != request.appearance_candidate_id
        || stored.geometry_delivery_manifest_object_sha256
            != request.geometry_delivery_manifest_object_sha256
        || stored.appearance_delivery_manifest_object_sha256
            != request.appearance_delivery_manifest_object_sha256
    {
        return Err(invalid("Bloom V2 sequence scope differs"));
    }
    let mut source_request = FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceV2PrepareRequest {
        schema_version: PREPARE_SCHEMA.to_owned(),
        sequence_key_sha256: stored.sequence_key_sha256.clone(),
        project_id: stored.project_id.clone(),
        geometry_candidate_id: stored.geometry_candidate_id.clone(),
        geometry_candidate_state_sha256: stored.geometry_candidate_state_sha256.clone(),
        geometry_delivery_manifest_object_sha256: stored
            .geometry_delivery_manifest_object_sha256
            .clone(),
        geometry_artifact_sha256: stored.geometry_artifact_sha256.clone(),
        appearance_candidate_id: stored.appearance_candidate_id.clone(),
        appearance_candidate_state_sha256: stored.appearance_candidate_state_sha256.clone(),
        appearance_delivery_manifest_object_sha256: stored
            .appearance_delivery_manifest_object_sha256
            .clone(),
        appearance_artifact_sha256: stored.appearance_artifact_sha256.clone(),
        material_surface_quality_id: stored.material_surface_quality_id.clone(),
        material_surface_quality_report_object_sha256: stored
            .material_surface_quality_report_object_sha256
            .clone(),
        material_surface_quality_canonical_sha256: stored
            .material_surface_quality_canonical_sha256
            .clone(),
        projection_key_sha256: stored.projection_key_sha256.clone(),
        projection_object_sha256: stored.projection_object_sha256.clone(),
        projection_canonical_sha256: stored.projection_canonical_sha256.clone(),
        particle_sequence_key_sha256: stored.particle_sequence_key_sha256.clone(),
        particle_sequence_canonical_sha256: stored.particle_sequence_canonical_sha256.clone(),
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
        appearance_anchor_set_object_sha256: stored.appearance_anchor_set_object_sha256.clone(),
        appearance_anchor_set_canonical_sha256: stored
            .appearance_anchor_set_canonical_sha256
            .clone(),
        anchor_binding_policy: stored.anchor_binding_policy.clone(),
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
        trails_bloom_sequence_policy: stored.trails_bloom_sequence_policy.clone(),
        history_policy: stored.history_policy.clone(),
        history_pre_roll_policy: stored.history_pre_roll_policy.clone(),
        trail_sequence_key_sha256: stored.trail_sequence_key_sha256.clone(),
        trail_sequence_canonical_sha256: stored.trail_sequence_canonical_sha256.clone(),
        trail_key_scope: stored.trail_key_scope.clone(),
        trail_count: stored.trail_count,
        trail_emitter_roles: stored.trail_emitter_roles.clone(),
        trail_bloom_profile_sha256: stored.trail_bloom_profile_sha256.clone(),
        trail_bloom_profile: stored.trail_bloom_profile.clone(),
        frames: Vec::new(),
        input_sha256: stored.input_sha256.clone(),
        idempotency_key: stored.sequence_key_sha256.clone(),
    };
    source_request.frames = stored
        .frames
        .iter()
        .map(
            |frame| FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceV2FrameInput {
                frame_index: frame.frame_index,
                sample_time_ticks: frame.sample_time_ticks,
                trail_frame_index: frame.trail_frame_index,
                trail_sequence_key_sha256: frame.trail_sequence_key_sha256.clone(),
                trail_sequence_canonical_sha256: frame.trail_sequence_canonical_sha256.clone(),
                trail_frame_canonical_sha256: frame.trail_frame_canonical_sha256.clone(),
                trail_key_sha256: frame.trail_key_sha256.clone(),
                trail_inventory_sha256: frame.trail_inventory_sha256.clone(),
                trail_id_encoding_sha256: frame.trail_id_encoding_sha256.clone(),
                emitter_binding_sha256: frame.emitter_binding_sha256.clone(),
                particle_sequence_key_sha256: frame.particle_sequence_key_sha256.clone(),
                particle_sequence_frame_canonical_sha256: frame
                    .particle_sequence_frame_canonical_sha256
                    .clone(),
                current_projection_frame_index: frame.current_projection_frame_index,
                current_particle_frame_index: frame.current_particle_frame_index,
                current_projection_frame_canonical_sha256: frame
                    .current_projection_frame_canonical_sha256
                    .clone(),
                current_projection_socket_transform_inventory_sha256: frame
                    .current_projection_socket_transform_inventory_sha256
                    .clone(),
                current_projection_socket_transform_readback_sha256: frame
                    .current_projection_socket_transform_readback_sha256
                    .clone(),
                base_frame_key_sha256: frame.base_frame_key_sha256.clone(),
                bloom_key_sha256: frame.bloom_key_sha256.clone(),
                camera_object_sha256: frame.camera_object_sha256.clone(),
                camera_identity_sha256: frame.camera_identity_sha256.clone(),
                render_profile_sha256: frame.render_profile_sha256.clone(),
                render_worker_build_cohort_sha256: frame.render_worker_build_cohort_sha256.clone(),
            },
        )
        .collect();
    let (source, context, computations) = source_context(runtime, &source_request)?;
    stored_outputs_match(
        runtime,
        &source_request,
        &source,
        &context,
        &computations,
        &stored,
    )?;
    Ok(result_value(&stored, true, GET_RESULT_SCHEMA, false))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash() -> String {
        "a".repeat(64)
    }

    #[test]
    fn fixed_profile_and_v2_policies_are_closed() {
        assert_eq!(profile_hash().len(), 64);
        assert_eq!(profile()["radius_px"], json!(8));
        assert_eq!(
            POLICY,
            "projection-v2-driven-animated-socket-trails-bloom-dual-candidate@2"
        );
        assert_eq!(
            FRAME_SCOPE,
            "lod0-animation-trails-bloom-v2-source-frames-1-15-with-trails-v2-frame-zero-preroll@2"
        );
        assert!(is_sha256(&hash()));
    }

    #[test]
    fn frame_input_rejects_preroll_retarget() {
        let frame = FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceV2FrameInput {
            frame_index: 0,
            sample_time_ticks: 1,
            trail_frame_index: 0,
            trail_sequence_key_sha256: hash(),
            trail_sequence_canonical_sha256: hash(),
            trail_frame_canonical_sha256: hash(),
            trail_key_sha256: hash(),
            trail_inventory_sha256: hash(),
            trail_id_encoding_sha256: hash(),
            emitter_binding_sha256: hash(),
            particle_sequence_key_sha256: hash(),
            particle_sequence_frame_canonical_sha256: hash(),
            current_projection_frame_index: 1,
            current_particle_frame_index: 1,
            current_projection_frame_canonical_sha256: hash(),
            current_projection_socket_transform_inventory_sha256: hash(),
            current_projection_socket_transform_readback_sha256: hash(),
            base_frame_key_sha256: hash(),
            bloom_key_sha256: hash(),
            camera_object_sha256: hash(),
            camera_identity_sha256: hash(),
            render_profile_sha256: hash(),
            render_worker_build_cohort_sha256: hash(),
        };
        assert!(validate_frame_input(&frame, 0, 1).is_ok());
        let mut retargeted = frame.clone();
        retargeted.trail_frame_index = 1;
        assert!(validate_frame_input(&retargeted, 0, 1).is_err());
    }
}
