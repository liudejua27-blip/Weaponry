//! Projection-driven animated socket trail Bloom.
//!
//! The Trails sequence is the immutable source of the first three passes.  A
//! Bloom prepare therefore re-reads and replays that sequence, re-reads the
//! nine-AOV and HDR-Bloom parents, and asks the bounded Worker for the same
//! five-pass typed operation twice.  Only the two additive Bloom PNGs and the
//! two JSON sidecars per frame are owned by this link.

use super::fictional_energy_vfx_animated_socket_trails as trails;
use super::{
    canonical_json_hash, exact_object, is_opaque_id, is_sha256, now_string, render_worker,
    sha256_hex, CasObject, Runtime, RuntimeError,
};
use forgecad_contracts::{
    FictionalEnergyVfxAnimatedSocketTrailsBloomSequence,
    FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceFrame,
    FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceFrameInput,
    FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceGetRequest,
    FictionalEnergyVfxAnimatedSocketTrailsBloomSequencePrepareRequest,
    FictionalEnergyVfxAnimatedSocketTrailsSequence,
    FictionalEnergyVfxAnimatedSocketTrailsSequenceFrame, FictionalEnergyVfxBloomFrameLinkRecord,
    FictionalEnergyVfxFrameLinkRecord,
};
use serde_json::{json, Map, Value};

const PREPARE_SCHEMA: &str = "FictionalEnergyVfxAnimatedSocketTrailsBloomSequencePrepareRequest@1";
const GET_SCHEMA: &str = "FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceGetRequest@1";
const PREPARE_RESULT_SCHEMA: &str =
    "FictionalEnergyVfxAnimatedSocketTrailsBloomSequencePrepareResult@1";
const GET_RESULT_SCHEMA: &str = "FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceGetResult@1";
const SEQUENCE_SCHEMA: &str = "FictionalEnergyVfxAnimatedSocketTrailsBloomSequence@1";
const FRAME_SCHEMA: &str = "FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceFrame@1";
const FRAME_RECEIPT_SCHEMA: &str = "FictionalEnergyVfxAnimatedSocketTrailsBloomFrameReceipt@1";
const RENDER_SET_SCHEMA: &str = "FictionalEnergyVfxAnimatedSocketTrailsBloomRenderSet@1";
const FRAME_SCOPE: &str = "lod0-animation-trails-bloom-source-frames-1-15@1";
const POLICY: &str = "projection-driven-animated-socket-trails-bloom@1";
const TRAIL_KEY_SCOPE: &str = "animated-socket-trails-sequence-frame-binding@1";
const STATUS: &str =
    "runtime-owned-durable-fictional-energy-vfx-animated-socket-trails-bloom-sequence";
const MAX_FRAMES: usize = 15;
const PNG_BYTES: u64 = 4 * 1024 * 1024;

const RENDER_SET_KIND: &str = "fictional-energy-vfx-animated-socket-trails-bloom-render-set";
const FRAME_RECEIPT_KIND: &str = "fictional-energy-vfx-animated-socket-trails-bloom-frame-receipt";
const EMISSIVE_KIND: &str = "fictional-energy-vfx-animated-socket-trails-emissive-source";
const CONTRIBUTION_KIND: &str = "fictional-energy-vfx-animated-socket-trails-bloom-contribution";

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
    "trails_bloom_sequence_policy",
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
    "candidate_id",
];

fn invalid(detail: impl Into<String>) -> RuntimeError {
    RuntimeError::InvalidInput(format!(
        "FICTIONAL_ENERGY_VFX_ANIMATED_SOCKET_TRAILS_BLOOM_INVALID: {}",
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
) -> Result<FictionalEnergyVfxAnimatedSocketTrailsBloomSequencePrepareRequest, RuntimeError> {
    let object = exact_object(value, PREPARE_FIELDS, PREPARE_SCHEMA)?;
    if text(object, "schema_version")? != PREPARE_SCHEMA {
        return Err(invalid("prepare schema differs"));
    }
    let request: FictionalEnergyVfxAnimatedSocketTrailsBloomSequencePrepareRequest =
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
        "trail_sequence_key_sha256",
        "trail_sequence_canonical_sha256",
        "trail_bloom_profile_sha256",
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
        || request.trails_bloom_sequence_policy != POLICY
        || request.trail_key_scope != TRAIL_KEY_SCOPE
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
        || request.trail_emitter_roles != ROLES.map(str::to_owned).to_vec()
        || request.trail_bloom_profile != profile()
        || request.trail_bloom_profile_sha256 != profile_hash()
    {
        return Err(invalid(
            "sequence policy/profile or bounded schedule differs",
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
) -> Result<FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceGetRequest, RuntimeError> {
    let object = exact_object(value, GET_FIELDS, GET_SCHEMA)?;
    if text(object, "schema_version")? != GET_SCHEMA {
        return Err(invalid("get schema differs"));
    }
    let request: FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceGetRequest =
        serde_json::from_value(value.clone())
            .map_err(|error| invalid(format!("get request is malformed: {error}")))?;
    sha(object, "sequence_key_sha256")?;
    id(object, "project_id")?;
    id(object, "candidate_id")?;
    Ok(request)
}

fn validate_frame_input(
    frame: &FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceFrameInput,
    ordinal: usize,
    sample_time_ticks: u64,
) -> Result<(), RuntimeError> {
    if frame.frame_index != ordinal as u64
        || frame.sample_time_ticks != sample_time_ticks
        || !is_sha256(&frame.trail_sequence_key_sha256)
        || !is_sha256(&frame.trail_sequence_canonical_sha256)
        || !is_sha256(&frame.trail_frame_canonical_sha256)
        || !is_sha256(&frame.particle_sequence_frame_canonical_sha256)
        || !is_sha256(&frame.base_frame_key_sha256)
        || !is_sha256(&frame.bloom_key_sha256)
        || !is_sha256(&frame.camera_object_sha256)
        || !is_sha256(&frame.camera_identity_sha256)
        || !is_sha256(&frame.render_profile_sha256)
        || !is_sha256(&frame.render_worker_build_cohort_sha256)
    {
        return Err(invalid("Bloom frame input binding differs"));
    }
    Ok(())
}

fn same_string(left: &str, right: &str, field: &str) -> Result<(), RuntimeError> {
    if left != right {
        return Err(invalid(format!("{field} binding differs")));
    }
    Ok(())
}

fn validate_parent_lineage(
    request: &FictionalEnergyVfxAnimatedSocketTrailsBloomSequencePrepareRequest,
    source: &FictionalEnergyVfxAnimatedSocketTrailsSequence,
) -> Result<(), RuntimeError> {
    macro_rules! same {
        ($field:ident) => {
            same_string(&request.$field, &source.$field, stringify!($field))?
        };
    }
    same!(project_id);
    same!(candidate_id);
    same!(candidate_state_sha256);
    same!(delivery_manifest_object_sha256);
    same!(source_artifact_sha256);
    same!(projection_key_sha256);
    same!(projection_object_sha256);
    same!(projection_canonical_sha256);
    same!(animated_socket_materialization_key_sha256);
    same!(animated_artifact_sha256);
    same!(animated_socket_anchor_set_object_sha256);
    same!(animated_socket_anchor_set_canonical_sha256);
    same!(animation_clip_id);
    same!(animation_clip_object_sha256);
    same!(animation_clip_canonical_sha256);
    same!(animation_receipt_object_sha256);
    same!(animation_receipt_canonical_sha256);
    same!(vfx_profile_object_sha256);
    same!(vfx_profile_canonical_sha256);
    same!(socket_node_id_encoding_sha256);
    same!(socket_roles_sha256);
    same!(camera_object_sha256);
    same!(camera_identity_sha256);
    same!(render_profile_sha256);
    same!(render_worker_build_cohort_sha256);
    same!(sample_schedule_sha256);
    if request.sample_count != source.sample_count
        || request.sample_time_ticks != source.sample_time_ticks
        || request.trail_sequence_key_sha256 != source.sequence_key_sha256
        || request.trail_sequence_canonical_sha256 != source.canonical_sha256
        || request.trail_count != source.trail_count
        || request.trail_emitter_roles != source.trail_emitter_roles
    {
        return Err(invalid("Trails parent lineage differs"));
    }
    Ok(())
}

fn validate_frame_lineage(
    input: &FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceFrameInput,
    source: &FictionalEnergyVfxAnimatedSocketTrailsSequenceFrame,
    request: &FictionalEnergyVfxAnimatedSocketTrailsBloomSequencePrepareRequest,
) -> Result<(), RuntimeError> {
    if input.trail_sequence_key_sha256 != request.trail_sequence_key_sha256
        || input.trail_sequence_canonical_sha256 != request.trail_sequence_canonical_sha256
        || input.trail_frame_canonical_sha256 != source.canonical_sha256
        || input.particle_sequence_frame_canonical_sha256
            != source.current_particle_frame_canonical_sha256
        || input.base_frame_key_sha256 != source.base_frame_key_sha256
        || input.bloom_key_sha256 != source.bloom_key_sha256
        || input.camera_object_sha256 != source.camera_object_sha256
        || input.camera_identity_sha256 != source.camera_identity_sha256
        || input.render_profile_sha256 != source.render_profile_sha256
        || input.render_worker_build_cohort_sha256 != source.render_worker_build_cohort_sha256
    {
        return Err(invalid("Bloom frame does not bind exact Trails frame"));
    }
    Ok(())
}

fn load_base_and_bloom(
    runtime: &Runtime,
    request: &FictionalEnergyVfxAnimatedSocketTrailsBloomSequencePrepareRequest,
    input: &FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceFrameInput,
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
            .ok_or_else(|| invalid("base frame link is unavailable"))?,
    )
    .map_err(|error| invalid(format!("base frame link is malformed: {error}")))?;
    let bloom_value = runtime.fictional_energy_vfx_hdr_bloom_get(&json!({
        "schema_version":"FictionalEnergyVfxHdrBloomFrameGetRequest@1",
        "project_id":request.project_id,
        "bloom_key_sha256":input.bloom_key_sha256
    }))?;
    let bloom: FictionalEnergyVfxBloomFrameLinkRecord = serde_json::from_value(
        bloom_value
            .get("link")
            .cloned()
            .ok_or_else(|| invalid("HDR Bloom link is unavailable"))?,
    )
    .map_err(|error| invalid(format!("HDR Bloom link is malformed: {error}")))?;
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
        || bloom.bloom_profile_sha256.is_empty()
    {
        return Err(invalid("base/Bloom frame binding differs"));
    }
    if base.pass_object_sha256s.len() != 9 {
        return Err(invalid("base frame does not contain exactly nine AOVs"));
    }
    for hash in &base.pass_object_sha256s {
        let bytes = runtime.cas_read_bounded(hash, PNG_BYTES)?;
        if sha256_hex(&bytes) != *hash {
            return Err(invalid("base AOV bytes are not hash exact"));
        }
    }
    for hash in [
        &bloom.source_object_sha256,
        &bloom.contribution_object_sha256,
    ] {
        let bytes = runtime.cas_read_bounded(hash, PNG_BYTES)?;
        if sha256_hex(&bytes) != *hash {
            return Err(invalid("HDR Bloom bytes are not hash exact"));
        }
    }
    Ok((base, bloom))
}

#[derive(Debug, Clone)]
struct FrameComputation {
    bloom: FictionalEnergyVfxBloomFrameLinkRecord,
    base_depth_sha256: String,
    bloom_seed_sha256: String,
    bloom_key_sha256: String,
    worker: render_worker::RenderWorkerAnimatedSocketTrailsBloomFrame,
}

fn bloom_seed(
    request: &FictionalEnergyVfxAnimatedSocketTrailsBloomSequencePrepareRequest,
    source: &FictionalEnergyVfxAnimatedSocketTrailsSequenceFrame,
    base_depth_sha256: &str,
) -> String {
    canonical_json_hash(&json!({
        "schema_version":"RenderWorkerAnimatedSocketTrailBloomSeed@1",
        "trail_sequence_key_sha256":request.trail_sequence_key_sha256,
        "trail_frame_canonical_sha256":source.canonical_sha256,
        "frame_index":source.frame_index,
        "trail_seed_sha256":source.trail_seed_sha256,
        "trail_bloom_profile_sha256":request.trail_bloom_profile_sha256,
        "base_opaque_depth_object_sha256":base_depth_sha256
    }))
}

fn bloom_key(
    request: &FictionalEnergyVfxAnimatedSocketTrailsBloomSequencePrepareRequest,
    source: &FictionalEnergyVfxAnimatedSocketTrailsSequenceFrame,
    base: &FictionalEnergyVfxFrameLinkRecord,
    bloom: &FictionalEnergyVfxBloomFrameLinkRecord,
    worker: &render_worker::RenderWorkerAnimatedSocketTrailsBloomFrame,
    seed: &str,
) -> String {
    canonical_json_hash(&json!({
        "schema_version":"FictionalEnergyVfxAnimatedSocketTrailBloomKey@1",
        "sequence_key_sha256":request.sequence_key_sha256,
        "frame_index":source.frame_index,
        "trail_frame_canonical_sha256":source.canonical_sha256,
        "trail_passes":[source.trail_color_object_sha256,source.trail_id_object_sha256,source.trail_depth_object_sha256],
        "base_frame_key_sha256":base.frame_key_sha256,
        "base_opaque_depth_object_sha256":base.pass_object_sha256s[2],
        "bloom_key_sha256":bloom.bloom_key_sha256,
        "camera_object_sha256":request.camera_object_sha256,
        "render_profile_sha256":request.render_profile_sha256,
        "render_worker_build_cohort_sha256":request.render_worker_build_cohort_sha256,
        "trail_bloom_profile_sha256":request.trail_bloom_profile_sha256,
        "trail_bloom_seed_sha256":seed,
        "projection_sample_set_sha256":worker.projection_sample_set_sha256,
        "emitter_binding_sha256":worker.emitter_binding_sha256,
        "trail_inventory_sha256":worker.trail_inventory_sha256
    }))
}

fn replay_bloom_worker(
    runtime: &Runtime,
    request: &FictionalEnergyVfxAnimatedSocketTrailsBloomSequencePrepareRequest,
    source: &FictionalEnergyVfxAnimatedSocketTrailsSequenceFrame,
    context: &trails::TrailFrameContext,
    source_glb: &[u8],
    base: &FictionalEnergyVfxFrameLinkRecord,
    bloom: &FictionalEnergyVfxBloomFrameLinkRecord,
) -> Result<FrameComputation, RuntimeError> {
    let first = render_worker::render_typed_animated_socket_trails_bloom_with_worker_identity(
        source_glb,
        &context.camera,
        &request.projection_key_sha256,
        &context.worker.projection_input_sha256,
        context.input.current_projection_frame_index,
        context.input.sample_time_ticks,
        &context.projection_samples,
        &context.trails_wire,
        &context.worker.seed_sha256,
        render_worker::TypedTrailBloomProfile::FIXED,
    )
    .map_err(|error| {
        invalid(format!(
            "animated trail Bloom Worker render failed: {error}"
        ))
    })?;
    let second = render_worker::render_typed_animated_socket_trails_bloom_with_worker_identity(
        source_glb,
        &context.camera,
        &request.projection_key_sha256,
        &context.worker.projection_input_sha256,
        context.input.current_projection_frame_index,
        context.input.sample_time_ticks,
        &context.projection_samples,
        &context.trails_wire,
        &context.worker.seed_sha256,
        render_worker::TypedTrailBloomProfile::FIXED,
    )
    .map_err(|error| {
        invalid(format!(
            "animated trail Bloom Worker replay failed: {error}"
        ))
    })?;
    if first.trail_bloom_passes.len() != 5
        || second.trail_bloom_passes.len() != 5
        || first
            .trail_bloom_passes
            .iter()
            .zip(&second.trail_bloom_passes)
            .any(|(left, right)| left.pass != right.pass || left.png != right.png)
        || first.trail_bloom_profile != render_worker::TypedTrailBloomProfile::FIXED
        || second.trail_bloom_profile != first.trail_bloom_profile
        || first.trail_count != context.worker.trail_count
        || first.segment_count != context.worker.segment_count
        || first.emitter_counts != context.worker.emitter_counts
        || first.seed_sha256 != context.worker.seed_sha256
        || first.projection_key_sha256 != context.worker.projection_key_sha256
        || first.current_frame_index != context.worker.current_frame_index
        || first.current_sample_time_ticks != context.worker.current_sample_time_ticks
        || first.projection_input_sha256 != context.worker.projection_input_sha256
        || first.projection_sample_set_sha256 != context.worker.projection_sample_set_sha256
        || first.emitter_binding_sha256 != context.worker.emitter_binding_sha256
        || first.trail_inventory_sha256 != context.worker.trail_inventory_sha256
        || first.trail_inventory != context.worker.trail_inventory
        || first.build_cohort_sha256.as_deref()
            != Some(request.render_worker_build_cohort_sha256.as_str())
        || first
            .render_profile
            .get("canonical_sha256")
            .and_then(Value::as_str)
            != Some(request.render_profile_sha256.as_str())
        || first
            .trail_bloom_passes
            .iter()
            .take(3)
            .zip(&context.worker.trail_passes)
            .any(|(bloom_pass, trail_pass)| {
                bloom_pass.pass != trail_pass.pass || bloom_pass.png != trail_pass.png
            })
    {
        return Err(invalid(
            "animated trail Bloom Worker replay is not byte exact",
        ));
    }
    for (index, hash) in [
        &source.trail_color_object_sha256,
        &source.trail_id_object_sha256,
        &source.trail_depth_object_sha256,
    ]
    .into_iter()
    .enumerate()
    {
        let bytes = runtime.cas_read_bounded(hash, PNG_BYTES)?;
        if sha256_hex(&bytes) != *hash || bytes != context.worker.trail_passes[index].png {
            return Err(invalid("source Trails pass is not byte exact"));
        }
    }
    let seed = bloom_seed(request, source, &base.pass_object_sha256s[2]);
    let key = bloom_key(request, source, base, bloom, &first, &seed);
    Ok(FrameComputation {
        bloom: bloom.clone(),
        base_depth_sha256: base.pass_object_sha256s[2].clone(),
        bloom_seed_sha256: seed,
        bloom_key_sha256: key,
        worker: first,
    })
}

fn source_sequence_and_context(
    runtime: &Runtime,
    request: &FictionalEnergyVfxAnimatedSocketTrailsBloomSequencePrepareRequest,
) -> Result<
    (
        FictionalEnergyVfxAnimatedSocketTrailsSequence,
        trails::DependencyContext,
        Vec<FrameComputation>,
    ),
    RuntimeError,
> {
    let source_value =
        runtime.fictional_energy_vfx_animated_socket_trails_sequence_get(&json!({
            "schema_version":"FictionalEnergyVfxAnimatedSocketTrailsSequenceGetRequest@1",
            "sequence_key_sha256":request.trail_sequence_key_sha256,
            "project_id":request.project_id,
            "candidate_id":request.candidate_id
        }))?;
    let source: FictionalEnergyVfxAnimatedSocketTrailsSequence = serde_json::from_value(
        source_value
            .get("sequence")
            .cloned()
            .ok_or_else(|| invalid("Trails source sequence is unavailable"))?,
    )
    .map_err(|error| invalid(format!("Trails source sequence is malformed: {error}")))?;
    validate_parent_lineage(request, &source)?;
    let source_request = trails::replay_request(&source);
    let context = trails::build_context(runtime, &source_request)?;
    if context.frames.len() != request.sample_count as usize
        || context.worker_cohort != request.render_worker_build_cohort_sha256
        || context.projection_input_sha256.is_empty()
        || context.source_glb.is_empty()
    {
        return Err(invalid("Trails source context is incomplete"));
    }
    let mut computations = Vec::with_capacity(request.frames.len());
    for (ordinal, input) in request.frames.iter().enumerate() {
        let source_frame = source
            .frames
            .get(ordinal)
            .ok_or_else(|| invalid("Trails source frame is missing"))?;
        validate_frame_lineage(input, source_frame, request)?;
        let context_frame = context
            .frames
            .get(ordinal)
            .ok_or_else(|| invalid("Trails source worker frame is missing"))?;
        if context_frame.input.frame_index != input.frame_index
            || context_frame.input.sample_time_ticks != input.sample_time_ticks
            || context_frame.input.base_frame_key_sha256 != input.base_frame_key_sha256
            || context_frame.input.bloom_key_sha256 != input.bloom_key_sha256
            || context_frame.worker.current_frame_index
                != source_frame.current_projection_frame_index
        {
            return Err(invalid("Trails source frame input differs"));
        }
        let (base, bloom) = load_base_and_bloom(runtime, request, input)?;
        let computation = replay_bloom_worker(
            runtime,
            request,
            source_frame,
            context_frame,
            &context.source_glb,
            &base,
            &bloom,
        )?;
        if computation.bloom_key_sha256.is_empty()
            || computation.base_depth_sha256 != base.pass_object_sha256s[2]
            || computation.bloom.bloom_key_sha256 != input.bloom_key_sha256
        {
            return Err(invalid("Bloom frame derived binding is incomplete"));
        }
        computations.push(computation);
    }
    Ok((source, context, computations))
}

fn frame_without_receipt(
    frame: &FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceFrame,
) -> Value {
    let mut value = serde_json::to_value(frame).expect("Bloom frame serialization is infallible");
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

fn pass_metadata(hash: &str, size_bytes: u64, pass: &str) -> Value {
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

fn make_frame(
    request: &FictionalEnergyVfxAnimatedSocketTrailsBloomSequencePrepareRequest,
    source: &FictionalEnergyVfxAnimatedSocketTrailsSequenceFrame,
    computation: &FrameComputation,
    new_passes: [&str; 2],
    render_set_hash: &str,
    receipt_hash: &str,
    created_at: &str,
) -> FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceFrame {
    let trail_bloom_key = computation.bloom_key_sha256.clone();
    let mut frame = FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceFrame {
        schema_version: FRAME_SCHEMA.to_owned(),
        frame_index: source.frame_index,
        sample_time_ticks: source.sample_time_ticks,
        trail_sequence_key_sha256: request.trail_sequence_key_sha256.clone(),
        trail_sequence_canonical_sha256: request.trail_sequence_canonical_sha256.clone(),
        trail_frame_canonical_sha256: source.canonical_sha256.clone(),
        trail_color_object_sha256: source.trail_color_object_sha256.clone(),
        trail_id_object_sha256: source.trail_id_object_sha256.clone(),
        trail_depth_object_sha256: source.trail_depth_object_sha256.clone(),
        particle_sequence_frame_canonical_sha256: source
            .current_particle_frame_canonical_sha256
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
        trail_bloom_key_sha256: trail_bloom_key,
        trail_bloom_seed_sha256: computation.bloom_seed_sha256.clone(),
        trail_emissive_source_object_sha256: new_passes[0].to_owned(),
        trail_bloom_contribution_object_sha256: new_passes[1].to_owned(),
        render_set_object_sha256: render_set_hash.to_owned(),
        receipt_object_sha256: receipt_hash.to_owned(),
        canonical_sha256: String::new(),
        created_at: created_at.to_owned(),
    };
    frame.canonical_sha256 = canonical_json_hash(
        &serde_json::to_value(&frame).expect("Bloom frame serialization is infallible"),
    );
    frame
}

fn make_sequence(
    request: &FictionalEnergyVfxAnimatedSocketTrailsBloomSequencePrepareRequest,
    context: &trails::DependencyContext,
    frames: Vec<FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceFrame>,
) -> FictionalEnergyVfxAnimatedSocketTrailsBloomSequence {
    let mut sequence = FictionalEnergyVfxAnimatedSocketTrailsBloomSequence {
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
        trails_bloom_sequence_policy: POLICY.to_owned(),
        trail_sequence_key_sha256: request.trail_sequence_key_sha256.clone(),
        trail_sequence_canonical_sha256: request.trail_sequence_canonical_sha256.clone(),
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
        &serde_json::to_value(&sequence).expect("Bloom sequence serialization is infallible"),
    );
    sequence
}

fn frame_matches(
    input: &FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceFrameInput,
    frame: &FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceFrame,
) -> bool {
    input.frame_index == frame.frame_index
        && input.sample_time_ticks == frame.sample_time_ticks
        && input.trail_sequence_key_sha256 == frame.trail_sequence_key_sha256
        && input.trail_sequence_canonical_sha256 == frame.trail_sequence_canonical_sha256
        && input.trail_frame_canonical_sha256 == frame.trail_frame_canonical_sha256
        && input.particle_sequence_frame_canonical_sha256
            == frame.particle_sequence_frame_canonical_sha256
        && input.base_frame_key_sha256 == frame.base_frame_key_sha256
        && input.bloom_key_sha256 == frame.bloom_key_sha256
        && input.camera_object_sha256 == frame.camera_object_sha256
        && input.camera_identity_sha256 == frame.camera_identity_sha256
        && input.render_profile_sha256 == frame.render_profile_sha256
        && input.render_worker_build_cohort_sha256 == frame.render_worker_build_cohort_sha256
}

fn request_matches_sequence(
    request: &FictionalEnergyVfxAnimatedSocketTrailsBloomSequencePrepareRequest,
    sequence: &FictionalEnergyVfxAnimatedSocketTrailsBloomSequence,
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
        && request.trails_bloom_sequence_policy == sequence.trails_bloom_sequence_policy
        && request.trail_sequence_key_sha256 == sequence.trail_sequence_key_sha256
        && request.trail_sequence_canonical_sha256 == sequence.trail_sequence_canonical_sha256
        && request.trail_key_scope == sequence.trail_key_scope
        && request.trail_count == sequence.trail_count
        && request.trail_emitter_roles == sequence.trail_emitter_roles
        && request.trail_bloom_profile_sha256 == sequence.trail_bloom_profile_sha256
        && request.trail_bloom_profile == sequence.trail_bloom_profile
        && request.input_sha256 == sequence.input_sha256
        && request.frames.len() == sequence.frames.len()
        && request
            .frames
            .iter()
            .zip(&sequence.frames)
            .all(|(input, frame)| frame_matches(input, frame))
}

fn render_set_value(
    request: &FictionalEnergyVfxAnimatedSocketTrailsBloomSequencePrepareRequest,
    source: &FictionalEnergyVfxAnimatedSocketTrailsSequenceFrame,
    computation: &FrameComputation,
    new_passes: [&str; 2],
    context: &trails::TrailFrameContext,
) -> Result<(Value, Vec<u8>), RuntimeError> {
    let pass_artifacts = [
        pass_metadata(
            &source.trail_color_object_sha256,
            context.worker.trail_passes[0].png.len() as u64,
            PASS_NAMES[0],
        ),
        pass_metadata(
            &source.trail_id_object_sha256,
            context.worker.trail_passes[1].png.len() as u64,
            PASS_NAMES[1],
        ),
        pass_metadata(
            &source.trail_depth_object_sha256,
            context.worker.trail_passes[2].png.len() as u64,
            PASS_NAMES[2],
        ),
        pass_metadata(
            new_passes[0],
            computation.worker.trail_bloom_passes[3].png.len() as u64,
            PASS_NAMES[3],
        ),
        pass_metadata(
            new_passes[1],
            computation.worker.trail_bloom_passes[4].png.len() as u64,
            PASS_NAMES[4],
        ),
    ];
    super::fictional_energy_vfx_animated_socket_trails::canonical_object(json!({
        "schema_version":RENDER_SET_SCHEMA,
        "sequence_key_sha256":request.sequence_key_sha256,
        "frame_index":source.frame_index,
        "sample_time_ticks":source.sample_time_ticks,
        "trail_sequence_key_sha256":request.trail_sequence_key_sha256,
        "trail_sequence_canonical_sha256":request.trail_sequence_canonical_sha256,
        "trail_frame_canonical_sha256":source.canonical_sha256,
        "trail_bloom_key_sha256":computation.bloom_key_sha256,
        "trail_bloom_seed_sha256":computation.bloom_seed_sha256,
        "trail_bloom_profile_sha256":request.trail_bloom_profile_sha256,
        "trail_bloom_profile":profile(),
        "base_frame_key_sha256":source.base_frame_key_sha256,
        "base_opaque_depth_object_sha256":computation.base_depth_sha256,
        "bloom_key_sha256":source.bloom_key_sha256,
        "camera_object_sha256":request.camera_object_sha256,
        "camera_identity_sha256":request.camera_identity_sha256,
        "render_profile_sha256":request.render_profile_sha256,
        "render_worker_build_cohort_sha256":request.render_worker_build_cohort_sha256,
        "projection_sample_set_sha256":computation.worker.projection_sample_set_sha256,
        "emitter_binding_sha256":computation.worker.emitter_binding_sha256,
        "trail_inventory_sha256":computation.worker.trail_inventory_sha256,
        "trail_inventory":computation.worker.trail_inventory,
        "pass_artifacts":pass_artifacts,
        "first_three_passes_byte_exact":true,
        "base_aov_byte_exact_verified":true,
        "runtime_write_performed":true,
        "quality_status":"structural_only",
        "visual_quality_status":"NOT_PROVEN",
        "canonical_sha256":""
    }))
}

fn receipt_value(
    request: &FictionalEnergyVfxAnimatedSocketTrailsBloomSequencePrepareRequest,
    source: &FictionalEnergyVfxAnimatedSocketTrailsSequenceFrame,
    computation: &FrameComputation,
    frame_without_receipt: Value,
    new_passes: [&str; 2],
) -> Result<(Value, Vec<u8>), RuntimeError> {
    super::fictional_energy_vfx_animated_socket_trails::canonical_object(json!({
        "schema_version":FRAME_RECEIPT_SCHEMA,
        "sequence_key_sha256":request.sequence_key_sha256,
        "frame":frame_without_receipt,
        "trail_sequence_key_sha256":request.trail_sequence_key_sha256,
        "trail_sequence_canonical_sha256":request.trail_sequence_canonical_sha256,
        "trail_frame_canonical_sha256":source.canonical_sha256,
        "trail_passes":[source.trail_color_object_sha256,source.trail_id_object_sha256,source.trail_depth_object_sha256],
        "base_frame_key_sha256":source.base_frame_key_sha256,
        "base_opaque_depth_object_sha256":computation.base_depth_sha256,
        "bloom_key_sha256":source.bloom_key_sha256,
        "trail_bloom_key_sha256":computation.bloom_key_sha256,
        "trail_bloom_seed_sha256":computation.bloom_seed_sha256,
        "trail_bloom_profile_sha256":request.trail_bloom_profile_sha256,
        "trail_bloom_profile":profile(),
        "trail_emissive_source_object_sha256":new_passes[0],
        "trail_bloom_contribution_object_sha256":new_passes[1],
        "projection_sample_set_sha256":computation.worker.projection_sample_set_sha256,
        "emitter_binding_sha256":computation.worker.emitter_binding_sha256,
        "trail_inventory_sha256":computation.worker.trail_inventory_sha256,
        "trail_inventory":computation.worker.trail_inventory,
        "trail_bloom_passes":computation.worker.trail_bloom_passes.iter().enumerate().map(|(index, pass)| pass_metadata(&if index < 3 { [source.trail_color_object_sha256.as_str(),source.trail_id_object_sha256.as_str(),source.trail_depth_object_sha256.as_str()][index] } else { new_passes[index-3] }, pass.png.len() as u64, pass.pass.as_str())).collect::<Vec<_>>(),
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

fn validate_stored_outputs(
    runtime: &Runtime,
    request: &FictionalEnergyVfxAnimatedSocketTrailsBloomSequencePrepareRequest,
    source: &FictionalEnergyVfxAnimatedSocketTrailsSequence,
    context: &trails::DependencyContext,
    computations: &[FrameComputation],
    stored: &FictionalEnergyVfxAnimatedSocketTrailsBloomSequence,
) -> Result<(), RuntimeError> {
    if !request_matches_sequence(request, stored)
        || stored.trail_sequence_key_sha256 != source.sequence_key_sha256
        || stored.trail_sequence_canonical_sha256 != source.canonical_sha256
    {
        return Err(invalid("existing animated Trails Bloom binding differs"));
    }
    for (ordinal, (frame, computation)) in stored.frames.iter().zip(computations).enumerate() {
        let source_frame = source
            .frames
            .get(ordinal)
            .ok_or_else(|| invalid("stored Trails source frame is missing"))?;
        let context_frame = context
            .frames
            .get(ordinal)
            .ok_or_else(|| invalid("stored Trails worker frame is missing"))?;
        if frame.frame_index != source_frame.frame_index
            || frame.sample_time_ticks != source_frame.sample_time_ticks
            || frame.trail_frame_canonical_sha256 != source_frame.canonical_sha256
            || frame.trail_color_object_sha256 != source_frame.trail_color_object_sha256
            || frame.trail_id_object_sha256 != source_frame.trail_id_object_sha256
            || frame.trail_depth_object_sha256 != source_frame.trail_depth_object_sha256
            || frame.base_opaque_depth_object_sha256 != computation.base_depth_sha256
            || frame.trail_bloom_profile_sha256 != request.trail_bloom_profile_sha256
            || frame.trail_bloom_seed_sha256 != computation.bloom_seed_sha256
            || frame.trail_bloom_key_sha256 != computation.bloom_key_sha256
            || frame.bloom_key_sha256 != source_frame.bloom_key_sha256
            || frame.render_worker_build_cohort_sha256 != request.render_worker_build_cohort_sha256
        {
            return Err(invalid("stored Bloom frame binding differs"));
        }
        let new_hashes = [
            &frame.trail_emissive_source_object_sha256,
            &frame.trail_bloom_contribution_object_sha256,
        ];
        for (index, hash) in new_hashes.into_iter().enumerate() {
            let bytes = runtime.cas_read_bounded(hash, PNG_BYTES)?;
            if sha256_hex(&bytes) != *hash
                || bytes != computation.worker.trail_bloom_passes[index + 3].png
            {
                return Err(invalid("stored Bloom pass bytes differ after restart"));
            }
        }
        let render_set = trails::read_owned_canonical_json(
            runtime,
            &frame.render_set_object_sha256,
            RENDER_SET_SCHEMA,
        )?;
        let expected_render_set = render_set_value(
            request,
            source_frame,
            computation,
            [
                &frame.trail_emissive_source_object_sha256,
                &frame.trail_bloom_contribution_object_sha256,
            ],
            context_frame,
        )?
        .0;
        if render_set != expected_render_set {
            return Err(invalid("stored Bloom render-set differs"));
        }
        let receipt = trails::read_owned_canonical_json(
            runtime,
            &frame.receipt_object_sha256,
            FRAME_RECEIPT_SCHEMA,
        )?;
        let expected_receipt = receipt_value(
            request,
            source_frame,
            computation,
            frame_without_receipt(frame),
            [
                &frame.trail_emissive_source_object_sha256,
                &frame.trail_bloom_contribution_object_sha256,
            ],
        )?
        .0;
        if receipt != expected_receipt || context_frame.worker.trail_passes.len() != 3 {
            return Err(invalid("stored Bloom receipt differs"));
        }
    }
    Ok(())
}

fn result_value(
    sequence: &FictionalEnergyVfxAnimatedSocketTrailsBloomSequence,
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

fn write_sequence(
    runtime: &Runtime,
    request: &FictionalEnergyVfxAnimatedSocketTrailsBloomSequencePrepareRequest,
    source: &FictionalEnergyVfxAnimatedSocketTrailsSequence,
    context: &trails::DependencyContext,
    computations: &[FrameComputation],
) -> Result<Value, RuntimeError> {
    let reservation = runtime.store.begin_cas_reservation();
    let mut reserved_objects: Vec<CasObject> = Vec::new();
    let operation = (|| -> Result<Value, RuntimeError> {
        let mut frames = Vec::with_capacity(computations.len());
        for (ordinal, computation) in computations.iter().enumerate() {
            let source_frame = source
                .frames
                .get(ordinal)
                .ok_or_else(|| invalid("source Trails frame is missing during write"))?;
            let context_frame = context
                .frames
                .get(ordinal)
                .ok_or_else(|| invalid("source worker frame is missing during write"))?;
            let created_at = now_string();
            let mut new_hashes = [String::new(), String::new()];
            for (index, kind) in [EMISSIVE_KIND, CONTRIBUTION_KIND].into_iter().enumerate() {
                let pass = &computation.worker.trail_bloom_passes[index + 3];
                let object = runtime.store.put_object_reserved(
                    &reservation,
                    &pass.png,
                    None,
                    "image/png",
                    kind,
                    &created_at,
                )?;
                new_hashes[index] = object.record.sha256.clone();
                reserved_objects.push(object);
            }
            let render_set_value = render_set_value(
                request,
                source_frame,
                computation,
                [&new_hashes[0], &new_hashes[1]],
                context_frame,
            )?;
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
            let provisional = make_frame(
                request,
                source_frame,
                computation,
                [&new_hashes[0], &new_hashes[1]],
                &render_set_hash,
                "",
                &created_at,
            );
            let receipt_value = receipt_value(
                request,
                source_frame,
                computation,
                frame_without_receipt(&provisional),
                [&new_hashes[0], &new_hashes[1]],
            )?;
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
            frames.push(make_frame(
                request,
                source_frame,
                computation,
                [&new_hashes[0], &new_hashes[1]],
                &render_set_hash,
                &receipt_hash,
                &created_at,
            ));
        }
        let sequence = make_sequence(request, context, frames);
        let stored = runtime
            .store
            .record_fictional_energy_vfx_animated_socket_trails_bloom_sequence(&sequence)?;
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

pub(super) fn prepare(runtime: &Runtime, value: &Value) -> Result<Value, RuntimeError> {
    let request = parse_prepare(value)?;
    let (source, context, computations) = source_sequence_and_context(runtime, &request)?;
    if let Some(existing) = runtime
        .store
        .get_fictional_energy_vfx_animated_socket_trails_bloom_sequence(
            &request.sequence_key_sha256,
        )?
    {
        validate_stored_outputs(
            runtime,
            &request,
            &source,
            &context,
            &computations,
            &existing,
        )?;
        return result_value(&existing, true, PREPARE_RESULT_SCHEMA, true);
    }
    write_sequence(runtime, &request, &source, &context, &computations)
}

pub(super) fn get(runtime: &Runtime, value: &Value) -> Result<Value, RuntimeError> {
    let request = parse_get(value)?;
    let stored = runtime
        .store
        .get_fictional_energy_vfx_animated_socket_trails_bloom_sequence(
            &request.sequence_key_sha256,
        )?
        .ok_or_else(|| invalid("animated Trails Bloom sequence is unavailable"))?;
    if stored.project_id != request.project_id || stored.candidate_id != request.candidate_id {
        return Err(invalid("animated Trails Bloom sequence scope differs"));
    }
    let replay_request = FictionalEnergyVfxAnimatedSocketTrailsBloomSequencePrepareRequest {
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
        trails_bloom_sequence_policy: stored.trails_bloom_sequence_policy.clone(),
        trail_sequence_key_sha256: stored.trail_sequence_key_sha256.clone(),
        trail_sequence_canonical_sha256: stored.trail_sequence_canonical_sha256.clone(),
        trail_key_scope: stored.trail_key_scope.clone(),
        trail_count: stored.trail_count,
        trail_emitter_roles: stored.trail_emitter_roles.clone(),
        trail_bloom_profile_sha256: stored.trail_bloom_profile_sha256.clone(),
        trail_bloom_profile: stored.trail_bloom_profile.clone(),
        frames: stored
            .frames
            .iter()
            .map(
                |frame| FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceFrameInput {
                    frame_index: frame.frame_index,
                    sample_time_ticks: frame.sample_time_ticks,
                    trail_sequence_key_sha256: frame.trail_sequence_key_sha256.clone(),
                    trail_sequence_canonical_sha256: frame.trail_sequence_canonical_sha256.clone(),
                    trail_frame_canonical_sha256: frame.trail_frame_canonical_sha256.clone(),
                    particle_sequence_frame_canonical_sha256: frame
                        .particle_sequence_frame_canonical_sha256
                        .clone(),
                    base_frame_key_sha256: frame.base_frame_key_sha256.clone(),
                    bloom_key_sha256: frame.bloom_key_sha256.clone(),
                    camera_object_sha256: frame.camera_object_sha256.clone(),
                    camera_identity_sha256: frame.camera_identity_sha256.clone(),
                    render_profile_sha256: frame.render_profile_sha256.clone(),
                    render_worker_build_cohort_sha256: frame
                        .render_worker_build_cohort_sha256
                        .clone(),
                },
            )
            .collect(),
        input_sha256: stored.input_sha256.clone(),
        idempotency_key: stored.sequence_key_sha256.clone(),
    };
    let (source, context, computations) = source_sequence_and_context(runtime, &replay_request)?;
    validate_stored_outputs(
        runtime,
        &replay_request,
        &source,
        &context,
        &computations,
        &stored,
    )?;
    result_value(&stored, true, GET_RESULT_SCHEMA, false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash() -> String {
        "a".repeat(64)
    }

    fn frame_pair() -> (
        FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceFrameInput,
        FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceFrame,
    ) {
        let input = FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceFrameInput {
            frame_index: 0,
            sample_time_ticks: 1,
            trail_sequence_key_sha256: hash(),
            trail_sequence_canonical_sha256: hash(),
            trail_frame_canonical_sha256: hash(),
            particle_sequence_frame_canonical_sha256: hash(),
            base_frame_key_sha256: hash(),
            bloom_key_sha256: hash(),
            camera_object_sha256: hash(),
            camera_identity_sha256: hash(),
            render_profile_sha256: hash(),
            render_worker_build_cohort_sha256: hash(),
        };
        let frame = FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceFrame {
            schema_version: FRAME_SCHEMA.to_owned(),
            frame_index: 0,
            sample_time_ticks: 1,
            trail_sequence_key_sha256: hash(),
            trail_sequence_canonical_sha256: hash(),
            trail_frame_canonical_sha256: hash(),
            trail_color_object_sha256: hash(),
            trail_id_object_sha256: hash(),
            trail_depth_object_sha256: hash(),
            particle_sequence_frame_canonical_sha256: hash(),
            base_frame_key_sha256: hash(),
            bloom_key_sha256: hash(),
            camera_object_sha256: hash(),
            camera_identity_sha256: hash(),
            render_profile_sha256: hash(),
            render_worker_build_cohort_sha256: hash(),
            trail_bloom_profile_sha256: profile_hash(),
            base_opaque_depth_object_sha256: hash(),
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
            trail_bloom_key_sha256: hash(),
            trail_bloom_seed_sha256: hash(),
            trail_emissive_source_object_sha256: hash(),
            trail_bloom_contribution_object_sha256: hash(),
            render_set_object_sha256: hash(),
            receipt_object_sha256: hash(),
            canonical_sha256: hash(),
            created_at: "1".to_owned(),
        };
        (input, frame)
    }

    #[test]
    fn fixed_profile_is_closed_and_hashed() {
        assert_eq!(
            profile().get("kernel").and_then(Value::as_str),
            Some("separable-box-two-pass-fixed-radius@1")
        );
        assert_eq!(profile_hash().len(), 64);
        let mut altered = profile();
        altered["radius_px"] = json!(9);
        assert_ne!(canonical_json_hash(&altered), profile_hash());
    }

    #[test]
    fn frame_replay_rejects_camera_or_particle_retarget() {
        let (input, mut frame) = frame_pair();
        assert!(frame_matches(&input, &frame));
        frame.camera_identity_sha256 = "b".repeat(64);
        assert!(!frame_matches(&input, &frame));
        frame.camera_identity_sha256 = input.camera_identity_sha256.clone();
        frame.particle_sequence_frame_canonical_sha256 = "c".repeat(64);
        assert!(!frame_matches(&input, &frame));
    }

    #[test]
    fn owned_sidecar_canonical_hash_is_not_just_cas_hash() {
        let (value, _) = crate::fictional_energy_vfx_animated_socket_trails::canonical_object(
            json!({"schema_version":RENDER_SET_SCHEMA,"value":1,"canonical_sha256":""}),
        )
        .expect("canonical sidecar");
        let stored = value
            .get("canonical_sha256")
            .and_then(Value::as_str)
            .expect("canonical field");
        let mut tampered = value.as_object().expect("object").clone();
        tampered.insert("value".to_owned(), json!(2));
        let mut preimage = tampered.clone();
        preimage.insert("canonical_sha256".to_owned(), Value::String(String::new()));
        assert_ne!(canonical_json_hash(&Value::Object(preimage)), stored);
    }
}
