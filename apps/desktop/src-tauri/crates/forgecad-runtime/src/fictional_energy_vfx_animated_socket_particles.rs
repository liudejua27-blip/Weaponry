//! Projection-driven animated socket particles.
//!
//! This module is deliberately narrower than the older static-particle and
//! trail producers.  It consumes an already durable animated socket
//! transform projection, asks the bounded Render Worker to apply the two
//! selected composed-world transforms, and persists only structural particle
//! evidence.  It does not create trails, attach to an engine, or advance a
//! production stage.

use super::{
    canonical_json_bytes, canonical_json_hash, exact_object, is_opaque_id, is_sha256, now_string,
    render_worker, sha256_hex, CasObject, Runtime, RuntimeError,
};
use crate::game_asset_delivery;
use forgecad_contracts::{
    FictionalEnergyVfxAnimatedSocketParticlesSequence,
    FictionalEnergyVfxAnimatedSocketParticlesSequenceFrame,
    FictionalEnergyVfxAnimatedSocketParticlesSequenceFrameInput,
    FictionalEnergyVfxAnimatedSocketParticlesSequenceGetRequest,
    FictionalEnergyVfxAnimatedSocketParticlesSequencePrepareRequest,
    FictionalEnergyVfxAnimatedSocketParticlesSequenceV2,
    FictionalEnergyVfxAnimatedSocketParticlesSequenceV2Frame,
    FictionalEnergyVfxAnimatedSocketParticlesSequenceV2FrameInput,
    FictionalEnergyVfxAnimatedSocketParticlesSequenceV2GetRequest,
    FictionalEnergyVfxAnimatedSocketParticlesSequenceV2PrepareRequest,
    FictionalEnergyVfxBloomFrameLinkRecord, FictionalEnergyVfxFrameLinkRecord,
};
use serde_json::{json, Map, Value};

const PREPARE_SCHEMA: &str = "FictionalEnergyVfxAnimatedSocketParticlesSequencePrepareRequest@1";
const GET_SCHEMA: &str = "FictionalEnergyVfxAnimatedSocketParticlesSequenceGetRequest@1";
const PREPARE_RESULT_SCHEMA: &str =
    "FictionalEnergyVfxAnimatedSocketParticlesSequencePrepareResult@1";
const GET_RESULT_SCHEMA: &str = "FictionalEnergyVfxAnimatedSocketParticlesSequenceGetResult@1";
const SEQUENCE_SCHEMA: &str = "FictionalEnergyVfxAnimatedSocketParticlesSequence@1";
const FRAME_SCHEMA: &str = "FictionalEnergyVfxAnimatedSocketParticlesSequenceFrame@1";
const FRAME_RECEIPT_SCHEMA: &str = "FictionalEnergyVfxAnimatedSocketParticlesFrameReceipt@1";
const RENDER_SET_SCHEMA: &str = "FictionalEnergyVfxAnimatedSocketParticlesRenderSet@1";
const PARTICLE_INVENTORY_SCHEMA: &str = "RenderWorkerAnimatedSocketParticleWorldInventory@1";
const EMITTER_SCHEMA: &str = "RenderWorkerAnimatedSocketEmitterBindings@1";
const FRAME_SCOPE: &str = "lod0-animation-particles-frame-range-1-16@1";
const PARTICLE_POLICY: &str = "projection-driven-animated-socket-particles@1";
const EMITTER_POLICY: &str = "projection-role-muzzle-vfx-energy-core-vfx-to-particle-emitter@1";
const TRANSFORM_POLICY: &str =
    "glb-animation-linear-nlerp-flat-part-hierarchy-composed-world-trs@1";
const SEQUENCE_STATUS: &str =
    "runtime-owned-durable-fictional-energy-vfx-animated-socket-particles-sequence";
const MAX_JSON_BYTES: u64 = 1024 * 1024;
const MAX_GLB_BYTES: u64 = 64 * 1024 * 1024;
const MAX_FRAMES: usize = 16;

const SEQUENCE_RECEIPT_KIND: &str =
    "fictional-energy-vfx-animated-socket-particles-sequence-receipt";
const RENDER_SET_KIND: &str = "fictional-energy-vfx-animated-socket-particles-render-set";
const FRAME_RECEIPT_KIND: &str = "fictional-energy-vfx-animated-socket-particles-frame-receipt";
const COLOR_KIND: &str = "fictional-energy-vfx-animated-socket-particles-color";
const ID_KIND: &str = "fictional-energy-vfx-animated-socket-particles-id";
const DEPTH_KIND: &str = "fictional-energy-vfx-animated-socket-particles-depth";

// V2 is additive.  It deliberately uses the same bounded Render Worker and
// CAS kinds as V1, while its receipt/RenderSet/frame schemas are versioned so
// a dual-candidate replay can never be mistaken for a V1 single-candidate
// record.
const V2_PREPARE_SCHEMA: &str = "FictionalEnergyVfxAnimatedSocketParticlesSequencePrepareRequest@2";
const V2_GET_SCHEMA: &str = "FictionalEnergyVfxAnimatedSocketParticlesSequenceGetRequest@2";
const V2_PREPARE_RESULT_SCHEMA: &str =
    "FictionalEnergyVfxAnimatedSocketParticlesSequencePrepareResult@2";
const V2_GET_RESULT_SCHEMA: &str = "FictionalEnergyVfxAnimatedSocketParticlesSequenceGetResult@2";
const V2_SEQUENCE_SCHEMA: &str = "FictionalEnergyVfxAnimatedSocketParticlesSequence@2";
const V2_FRAME_SCHEMA: &str = "FictionalEnergyVfxAnimatedSocketParticlesSequenceFrame@2";
const V2_FRAME_RECEIPT_SCHEMA: &str = "FictionalEnergyVfxAnimatedSocketParticlesFrameReceipt@2";
const V2_RENDER_SET_SCHEMA: &str = "FictionalEnergyVfxAnimatedSocketParticlesRenderSet@2";
const V2_FRAME_SCOPE: &str = "lod0-animation-particles-frame-range-1-16@2";
const V2_PARTICLE_POLICY: &str = "projection-v2-driven-animated-socket-particles-dual-candidate@2";
const V2_ANCHOR_BINDING_POLICY: &str = "geometry-appearance-anchor-role-owner-trs-equivalent@1";
const V2_GEOMETRY_PRESERVATION_STATUS: &str = "source-output-renderable-geometry-byte-exact";
const PROJECTION_FRAME_SCOPE: &str = "lod0-animation-frame-range-1-16@2";
const V2_PROJECTION_SCHEMA: &str = "GameWeaponAnimatedGlbSocketTransformProjection@2";
const V2_PROJECTION_FRAME_SCHEMA: &str = "GameWeaponAnimatedGlbSocketTransformProjectionFrame@2";
const V2_PROJECTION_TRANSFORM_POLICY: &str =
    "glb-animation-linear-nlerp-flat-part-hierarchy-composed-world-trs-matrix@2";
const V2_PROJECTION_PART_HIERARCHY_POLICY: &str = "flat-identity-rest-part-hierarchy-only@2";
const V2_PROJECTION_REPRESENTATION_POLICY: &str =
    "trs-quaternion-no-shear-plus-column-major-matrix@2";
const V2_SEQUENCE_STATUS: &str =
    "runtime-owned-durable-fictional-energy-vfx-animated-socket-particles-sequence-v2";

const V2_PREPARE_FIELDS: &[&str] = &[
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
    "particles_sequence_policy",
    "emitter_binding_policy",
    "transform_projection_policy",
    "frames",
    "input_sha256",
    "idempotency_key",
];

const V2_GET_FIELDS: &[&str] = &[
    "schema_version",
    "sequence_key_sha256",
    "project_id",
    "geometry_candidate_id",
    "appearance_candidate_id",
    "geometry_delivery_manifest_object_sha256",
    "appearance_delivery_manifest_object_sha256",
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
    "particles_sequence_policy",
    "emitter_binding_policy",
    "transform_projection_policy",
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
        "FICTIONAL_ENERGY_VFX_ANIMATED_SOCKET_PARTICLES_INVALID: {}",
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
        FictionalEnergyVfxAnimatedSocketParticlesSequencePrepareRequest,
        String,
    ),
    RuntimeError,
> {
    let object = exact_object(value, PREPARE_FIELDS, PREPARE_SCHEMA)?;
    if text(object, "schema_version")? != PREPARE_SCHEMA {
        return Err(invalid("prepare schema differs"));
    }
    let request: FictionalEnergyVfxAnimatedSocketParticlesSequencePrepareRequest =
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
    if request.frame_scope != FRAME_SCOPE
        || request.particles_sequence_policy != PARTICLE_POLICY
        || request.emitter_binding_policy != EMITTER_POLICY
        || request.transform_projection_policy != TRANSFORM_POLICY
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
    {
        return Err(invalid(
            "sequence policy or bounded sample schedule differs",
        ));
    }
    for (ordinal, frame) in request.frames.iter().enumerate() {
        validate_frame_input(frame, ordinal, request.sample_time_ticks[ordinal])?;
    }
    let mut preimage = object.clone();
    preimage.remove("sequence_key_sha256");
    preimage.remove("input_sha256");
    preimage.remove("idempotency_key");
    let input_sha256 = canonical_json_hash(&Value::Object(preimage));
    if request.input_sha256 != input_sha256 || request.sequence_key_sha256 != input_sha256 {
        return Err(invalid("sequence input/key hash differs"));
    }
    Ok((request, input_sha256))
}

fn parse_get(
    value: &Value,
) -> Result<FictionalEnergyVfxAnimatedSocketParticlesSequenceGetRequest, RuntimeError> {
    let object = exact_object(value, GET_FIELDS, GET_SCHEMA)?;
    if text(object, "schema_version")? != GET_SCHEMA {
        return Err(invalid("get schema differs"));
    }
    let request: FictionalEnergyVfxAnimatedSocketParticlesSequenceGetRequest =
        serde_json::from_value(value.clone())
            .map_err(|error| invalid(format!("get request is malformed: {error}")))?;
    sha(object, "sequence_key_sha256")?;
    id(object, "project_id")?;
    id(object, "candidate_id")?;
    Ok(request)
}

fn parse_v2_prepare(
    value: &Value,
) -> Result<
    (
        FictionalEnergyVfxAnimatedSocketParticlesSequenceV2PrepareRequest,
        String,
    ),
    RuntimeError,
> {
    let object = exact_object(value, V2_PREPARE_FIELDS, V2_PREPARE_SCHEMA)?;
    if text(object, "schema_version")? != V2_PREPARE_SCHEMA {
        return Err(invalid("V2 prepare schema differs"));
    }
    let request: FictionalEnergyVfxAnimatedSocketParticlesSequenceV2PrepareRequest =
        serde_json::from_value(value.clone())
            .map_err(|error| invalid(format!("V2 prepare request is malformed: {error}")))?;
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
        || request.frame_scope != V2_FRAME_SCOPE
        || request.particles_sequence_policy != V2_PARTICLE_POLICY
        || request.anchor_binding_policy != V2_ANCHOR_BINDING_POLICY
        || request.emitter_binding_policy != EMITTER_POLICY
        || request.transform_projection_policy != V2_PROJECTION_TRANSFORM_POLICY
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
    {
        return Err(invalid(
            "V2 candidate, policy or bounded sample schedule differs",
        ));
    }
    for (ordinal, frame) in request.frames.iter().enumerate() {
        validate_v2_frame_input(frame, ordinal, request.sample_time_ticks[ordinal])?;
    }
    let mut preimage = object.clone();
    preimage.remove("sequence_key_sha256");
    preimage.remove("input_sha256");
    preimage.remove("idempotency_key");
    let input_sha256 = canonical_json_hash(&Value::Object(preimage));
    if request.input_sha256 != input_sha256 || request.sequence_key_sha256 != input_sha256 {
        return Err(invalid("V2 sequence input/key hash differs"));
    }
    Ok((request, input_sha256))
}

fn parse_v2_get(
    value: &Value,
) -> Result<FictionalEnergyVfxAnimatedSocketParticlesSequenceV2GetRequest, RuntimeError> {
    let object = exact_object(value, V2_GET_FIELDS, V2_GET_SCHEMA)?;
    if text(object, "schema_version")? != V2_GET_SCHEMA {
        return Err(invalid("V2 get schema differs"));
    }
    let request: FictionalEnergyVfxAnimatedSocketParticlesSequenceV2GetRequest =
        serde_json::from_value(value.clone())
            .map_err(|error| invalid(format!("V2 get request is malformed: {error}")))?;
    sha(object, "sequence_key_sha256")?;
    for field in [
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
    if request.geometry_candidate_id == request.appearance_candidate_id {
        return Err(invalid("V2 get candidates must remain distinct"));
    }
    Ok(request)
}

fn validate_v2_frame_input(
    frame: &FictionalEnergyVfxAnimatedSocketParticlesSequenceV2FrameInput,
    ordinal: usize,
    sample_time_ticks: u64,
) -> Result<(), RuntimeError> {
    if frame.frame_index != ordinal as u64
        || frame.sample_time_ticks != sample_time_ticks
        || frame.sample_time_ticks > 1_000_000
        || [
            &frame.projection_frame_canonical_sha256,
            &frame.projection_socket_transform_inventory_sha256,
            &frame.projection_socket_transform_readback_sha256,
            &frame.base_frame_key_sha256,
            &frame.bloom_key_sha256,
        ]
        .iter()
        .any(|value| !is_sha256(value))
    {
        return Err(invalid(
            "V2 sequence frame input is malformed or out of order",
        ));
    }
    Ok(())
}

fn validate_frame_input(
    frame: &FictionalEnergyVfxAnimatedSocketParticlesSequenceFrameInput,
    ordinal: usize,
    sample_time_ticks: u64,
) -> Result<(), RuntimeError> {
    if frame.frame_index != ordinal as u64
        || frame.sample_time_ticks != sample_time_ticks
        || frame.sample_time_ticks > 1_000_000
        || [
            &frame.projection_frame_canonical_sha256,
            &frame.projection_socket_transform_inventory_sha256,
            &frame.projection_socket_transform_readback_sha256,
            &frame.base_frame_key_sha256,
            &frame.bloom_key_sha256,
        ]
        .iter()
        .any(|value| !is_sha256(value))
    {
        return Err(invalid("sequence frame input is malformed or out of order"));
    }
    Ok(())
}

fn read_canonical_json(runtime: &Runtime, hash: &str, schema: &str) -> Result<Value, RuntimeError> {
    let bytes = runtime.cas_read_bounded(hash, MAX_JSON_BYTES)?;
    if bytes.is_empty() || sha256_hex(&bytes) != hash {
        return Err(invalid("dependency JSON CAS hash differs"));
    }
    let value: Value = serde_json::from_slice(&bytes)
        .map_err(|error| invalid(format!("dependency JSON is malformed: {error}")))?;
    if value.get("schema_version").and_then(Value::as_str) != Some(schema) {
        return Err(invalid(format!("dependency schema differs: {schema}")));
    }
    let canonical = canonical_json_bytes(&value).map_err(|error| invalid(error.to_string()))?;
    if canonical != bytes {
        return Err(invalid("dependency JSON is not canonical"));
    }
    Ok(value)
}

fn canonical_values_equal(left: &Value, right: &Value) -> Result<bool, RuntimeError> {
    let normalize = |value: &Value| -> Result<Vec<u8>, RuntimeError> {
        let transport = serde_json::to_vec(value)
            .map_err(|error| invalid(format!("comparison serialization failed: {error}")))?;
        let normalized: Value = serde_json::from_slice(&transport)
            .map_err(|error| invalid(format!("comparison normalization failed: {error}")))?;
        canonical_json_bytes(&normalized).map_err(|error| invalid(error.to_string()))
    };
    let left = normalize(left)?;
    let right = normalize(right)?;
    Ok(left == right)
}

fn expect_field<'a>(object: &'a Value, field: &str) -> Result<&'a str, RuntimeError> {
    object
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid(format!("dependency field {field} is unavailable")))
}

fn expect_same(object: &Value, field: &str, expected: &str) -> Result<(), RuntimeError> {
    if expect_field(object, field)? != expected {
        return Err(invalid(format!("dependency field {field} binding differs")));
    }
    Ok(())
}

#[derive(Debug, Clone)]
struct FrameContext {
    input: FictionalEnergyVfxAnimatedSocketParticlesSequenceFrameInput,
    projection_frame: Value,
    emitter_bindings: Value,
    particles: Value,
    seed_sha256: String,
    worker: render_worker::RenderWorkerAnimatedSocketParticlesFrame,
}

#[derive(Debug, Clone)]
struct DependencyContext {
    frames: Vec<FrameContext>,
    worker_cohort: String,
}

#[derive(Debug, Clone)]
struct V2FrameContext {
    input: FictionalEnergyVfxAnimatedSocketParticlesSequenceV2FrameInput,
    projection_frame: Value,
    emitter_bindings: Value,
    particles: Value,
    seed_sha256: String,
    worker: render_worker::RenderWorkerAnimatedSocketParticlesFrame,
}

#[derive(Debug, Clone)]
struct V2DependencyContext {
    frames: Vec<V2FrameContext>,
    worker_cohort: String,
    anchor_binding_sha256: String,
    geometry_preservation_projection_sha256: String,
    geometry_preservation_status: String,
}

fn validate_projection_parent(
    request: &FictionalEnergyVfxAnimatedSocketParticlesSequencePrepareRequest,
    projection: &Value,
) -> Result<(), RuntimeError> {
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
        ("frame_scope", FRAME_SCOPE),
        ("transform_projection_policy", TRANSFORM_POLICY),
    ] {
        expect_same(projection, field, expected)?;
    }
    if projection.get("sample_count").and_then(Value::as_u64) != Some(request.sample_count)
        || projection.get("sample_time_ticks") != Some(&json!(request.sample_time_ticks))
        || projection
            .get("input_sha256")
            .and_then(Value::as_str)
            .is_none_or(|value| !is_sha256(value))
    {
        return Err(invalid(
            "projection sample schedule or input binding differs",
        ));
    }
    Ok(())
}

fn validate_vfx_profile(
    request: &FictionalEnergyVfxAnimatedSocketParticlesSequencePrepareRequest,
    vfx: &Value,
) -> Result<Value, RuntimeError> {
    let link = vfx
        .get("link")
        .ok_or_else(|| invalid("durable VFX profile link is unavailable"))?;
    expect_same(link, "project_id", request.project_id.as_str())?;
    expect_same(
        link,
        "delivery_manifest_object_sha256",
        request.delivery_manifest_object_sha256.as_str(),
    )?;
    expect_same(
        link,
        "vfx_profile_object_sha256",
        request.vfx_profile_object_sha256.as_str(),
    )?;
    let profile = vfx
        .get("vfx_profile")
        .cloned()
        .ok_or_else(|| invalid("durable VFX profile is unavailable"))?;
    expect_same(
        &profile,
        "canonical_sha256",
        request.vfx_profile_canonical_sha256.as_str(),
    )?;
    Ok(profile)
}

fn camera_for_base(
    runtime: &Runtime,
    request: &FictionalEnergyVfxAnimatedSocketParticlesSequencePrepareRequest,
    base: &FictionalEnergyVfxFrameLinkRecord,
) -> Result<Value, RuntimeError> {
    if base.project_id != request.project_id
        || base.delivery_manifest_object_sha256 != request.delivery_manifest_object_sha256
        || base.source_candidate_id != request.candidate_id
        || base.source_artifact_sha256 != request.source_artifact_sha256
        || base.vfx_profile_object_sha256 != request.vfx_profile_object_sha256
        || base.camera_object_sha256 != request.camera_object_sha256
        || base.camera_identity_sha256 != request.camera_identity_sha256
        || base.render_profile_sha256 != request.render_profile_sha256
        || base.render_worker_build_cohort_sha256 != request.render_worker_build_cohort_sha256
    {
        return Err(invalid("base frame parent/camera/cohort binding differs"));
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

fn frame_projection<'a>(projection: &'a Value, index: usize) -> Result<&'a Value, RuntimeError> {
    projection
        .get("frames")
        .and_then(Value::as_array)
        .and_then(|frames| frames.get(index))
        .ok_or_else(|| invalid("projection frame is unavailable"))
}

fn v2_frame_projection_by_index<'a>(
    projection: &'a Value,
    frame_index: u64,
) -> Result<&'a Value, RuntimeError> {
    let frames = projection
        .get("frames")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("Projection@2 frames are unavailable"))?;
    let mut matches = frames
        .iter()
        .filter(|frame| frame.get("frame_index").and_then(Value::as_u64) == Some(frame_index));
    let frame = matches
        .next()
        .ok_or_else(|| invalid("Projection@2 frame_index is unavailable"))?;
    if matches.next().is_some() {
        return Err(invalid("Projection@2 frame_index is duplicated"));
    }
    Ok(frame)
}

fn validate_projection_frame(
    input: &FictionalEnergyVfxAnimatedSocketParticlesSequenceFrameInput,
    projection_frame: &Value,
) -> Result<(), RuntimeError> {
    if projection_frame.get("frame_index").and_then(Value::as_u64) != Some(input.frame_index)
        || projection_frame
            .get("sample_time_ticks")
            .and_then(Value::as_u64)
            != Some(input.sample_time_ticks)
        || projection_frame
            .get("canonical_sha256")
            .and_then(Value::as_str)
            != Some(input.projection_frame_canonical_sha256.as_str())
        || projection_frame
            .get("socket_transform_inventory_sha256")
            .and_then(Value::as_str)
            != Some(input.projection_socket_transform_inventory_sha256.as_str())
        || projection_frame
            .get("socket_transform_readback_sha256")
            .and_then(Value::as_str)
            != Some(input.projection_socket_transform_readback_sha256.as_str())
    {
        return Err(invalid("projection frame binding differs"));
    }
    Ok(())
}

fn validate_base_bloom(
    request: &FictionalEnergyVfxAnimatedSocketParticlesSequencePrepareRequest,
    input: &FictionalEnergyVfxAnimatedSocketParticlesSequenceFrameInput,
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
        || base.vfx_profile_object_sha256 != request.vfx_profile_object_sha256
        || bloom.vfx_profile_object_sha256 != request.vfx_profile_object_sha256
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

fn projection_socket_transform<'a>(
    frame: &'a Value,
    role: &str,
    socket_node_id: &str,
    anchor_id: &str,
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
    if socket.get("socket_node_id").and_then(Value::as_str) != Some(socket_node_id)
        || socket.get("anchor_id").and_then(Value::as_str) != Some(anchor_id)
        || socket.get("owner_part_id").and_then(Value::as_str) != Some(owner_part_id)
        || socket.get("node_kind").and_then(Value::as_str) != Some("empty")
    {
        return Err(invalid(format!("projection role {role} retargeted")));
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
    let _ = (translation, rotation, scale);
    Ok(socket)
}

fn f32_array(
    object: &Value,
    field: &str,
    length: usize,
    absolute_max: f32,
) -> Result<Vec<f32>, RuntimeError> {
    let values = object
        .get(field)
        .and_then(Value::as_array)
        .filter(|values| values.len() == length)
        .ok_or_else(|| invalid(format!("{field} must contain exactly {length} values")))?;
    values
        .iter()
        .map(|value| {
            let number = value
                .as_f64()
                .filter(|value| value.is_finite() && value.abs() <= f64::from(absolute_max))
                .ok_or_else(|| invalid(format!("{field} contains an invalid number")))?;
            Ok(number as f32)
        })
        .collect()
}

fn f32_value(values: &[f32]) -> Value {
    Value::Array(values.iter().map(|value| json!(value)).collect())
}

fn build_emitter_bindings(frame: &Value) -> Result<Value, RuntimeError> {
    let definitions = [
        (
            "muzzle-burst",
            "socket-muzzle-vfx",
            "muzzle-vfx",
            "barrel-assembly",
        ),
        (
            "energy-core-sparks",
            "socket-energy-core-vfx",
            "energy-core-vfx",
            "energy-core",
        ),
    ];
    let mut emitters = Vec::with_capacity(2);
    for (emitter_id, node_id, role, owner_part_id) in definitions {
        let socket = projection_socket_transform(frame, role, node_id, node_id, owner_part_id)?;
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
    Ok(json!({"schema_version":EMITTER_SCHEMA,"emitters":emitters}))
}

fn emitter_transform(
    bindings: &Value,
    index: usize,
) -> Result<([f32; 3], [f32; 4], [f32; 3]), RuntimeError> {
    let emitter = bindings
        .get("emitters")
        .and_then(Value::as_array)
        .and_then(|values| values.get(index))
        .ok_or_else(|| invalid("emitter binding is unavailable"))?;
    let transform = emitter
        .get("composed_world_transform")
        .ok_or_else(|| invalid("emitter transform is unavailable"))?;
    let t = f32_array(transform, "translation_m", 3, 100.0)?;
    let q = f32_array(transform, "rotation_quat_xyzw", 4, 1.0)?;
    let s = f32_array(transform, "scale_xyz", 3, 2.0)?;
    Ok((
        [t[0], t[1], t[2]],
        [q[0], q[1], q[2], q[3]],
        [s[0], s[1], s[2]],
    ))
}

fn hash_byte(seed: &str, index: usize, axis: usize) -> f32 {
    let digest = sha256_hex(format!("{seed}:{index}:{axis}").as_bytes());
    let offset = axis * 2;
    let byte = u8::from_str_radix(&digest[offset..offset + 2], 16).unwrap_or(0);
    (f32::from(byte) / 255.0) * 2.0 - 1.0
}

fn build_particles(bindings: &Value, frame_seed: &str) -> Result<Value, RuntimeError> {
    let mut values = Vec::with_capacity(56);
    for index in 0..56 {
        let (emitter_id, id, scale, color, alpha, lifetime) = if index < 24 {
            (
                "muzzle-burst",
                10_000_u64 + index as u64,
                [0.10_f32, 0.06, 0.08],
                [0.0_f32, 0.8, 1.0],
                0.8_f32,
                120_u64,
            )
        } else {
            (
                "energy-core-sparks",
                20_000_u64 + (index - 24) as u64,
                [0.08_f32, 0.08, 0.10],
                [1.0_f32, 0.35, 0.05],
                0.75_f32,
                180_u64,
            )
        };
        let local = [
            hash_byte(frame_seed, index, 0) * scale[0],
            hash_byte(frame_seed, index, 1) * scale[1],
            hash_byte(frame_seed, index, 2) * scale[2],
        ];
        values.push(json!({
            "emitter_id":emitter_id,
            "id":id,
            "local_offset_m":f32_value(&local),
            "radius_px":2.0_f32,
            "color_linear_rgb":f32_value(&color),
            "alpha":alpha,
            "lifetime_ticks":lifetime
        }));
    }
    let _ = bindings;
    Ok(Value::Array(values))
}

fn vector_add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
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

#[cfg(test)]
fn transform_point(translation: [f32; 3], rotation: [f32; 4], local: [f32; 3]) -> [f32; 3] {
    transform_point_with_scale(translation, rotation, [1.0, 1.0, 1.0], local)
}

fn transform_point_with_scale(
    translation: [f32; 3],
    rotation: [f32; 4],
    scale: [f32; 3],
    local: [f32; 3],
) -> [f32; 3] {
    let scaled = [
        local[0] * scale[0],
        local[1] * scale[1],
        local[2] * scale[2],
    ];
    let q = [rotation[0], rotation[1], rotation[2]];
    let twice_cross = vector_scale(vector_cross(q, scaled), 2.0);
    let rotated = vector_add(
        vector_add(scaled, vector_scale(twice_cross, rotation[3])),
        vector_cross(q, twice_cross),
    );
    vector_add(translation, rotated)
}

fn normalize(vector: [f32; 3]) -> Result<[f32; 3], RuntimeError> {
    let length = (vector[0] * vector[0] + vector[1] * vector[1] + vector[2] * vector[2]).sqrt();
    if !length.is_finite() || length <= 1e-6 {
        return Err(invalid("camera basis is degenerate"));
    }
    Ok([vector[0] / length, vector[1] / length, vector[2] / length])
}

fn camera_depth(camera: &Value, position: [f32; 3]) -> Result<f32, RuntimeError> {
    let transform = camera
        .get("transform")
        .ok_or_else(|| invalid("camera transform is unavailable"))?;
    let position_camera = f32_array(transform, "position_m", 3, 1_000.0)?;
    let target = f32_array(transform, "target_m", 3, 1_000.0)?;
    let up = f32_array(transform, "up", 3, 1_000.0)?;
    let position_camera = [position_camera[0], position_camera[1], position_camera[2]];
    let target = [target[0], target[1], target[2]];
    let up = [up[0], up[1], up[2]];
    let forward = normalize([
        target[0] - position_camera[0],
        target[1] - position_camera[1],
        target[2] - position_camera[2],
    ])?;
    let right = normalize(vector_cross(forward, up))?;
    let _up = normalize(vector_cross(right, forward))?;
    let near = camera
        .get("near_m")
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite())
        .map(|value| value as f32)
        .ok_or_else(|| invalid("camera near is invalid"))?;
    let far = camera
        .get("far_m")
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite())
        .map(|value| value as f32)
        .ok_or_else(|| invalid("camera far is invalid"))?;
    let relative = [
        position[0] - position_camera[0],
        position[1] - position_camera[1],
        position[2] - position_camera[2],
    ];
    let z = relative[0] * forward[0] + relative[1] * forward[1] + relative[2] * forward[2];
    if !(near > 0.0 && far > near && z > near && z < far) {
        return Err(invalid("particle is outside camera clip range"));
    }
    let depth = (z - near) / (far - near);
    if !depth.is_finite() || !(0.0..=1.0).contains(&depth) {
        return Err(invalid("particle camera depth is invalid"));
    }
    Ok(depth)
}

fn world_values(
    bindings: &Value,
    particles: &Value,
    camera: &Value,
    seed: &str,
) -> Result<Vec<Value>, RuntimeError> {
    let values = particles
        .as_array()
        .filter(|values| values.len() == 56)
        .ok_or_else(|| invalid("particle inventory must contain 56 values"))?;
    let mut result = Vec::with_capacity(values.len());
    for (index, particle) in values.iter().enumerate() {
        let emitter_index = usize::from(index >= 24);
        let (translation, rotation, scale) = emitter_transform(bindings, emitter_index)?;
        let local = f32_array(particle, "local_offset_m", 3, 10.0)?;
        let local = [local[0], local[1], local[2]];
        let position = transform_point_with_scale(translation, rotation, scale, local);
        let depth = camera_depth(camera, position)?;
        result.push(json!({
            "emitter_id":particle.get("emitter_id"),
            "id":particle.get("id"),
            "local_offset_m":f32_value(&local),
            "position":f32_value(&position),
            "radius_px":particle.get("radius_px"),
            "color_linear_rgb":particle.get("color_linear_rgb"),
            "alpha":particle.get("alpha"),
            "lifetime_ticks":particle.get("lifetime_ticks"),
            "depth":depth
        }));
    }
    let _ = seed;
    Ok(result)
}

fn worker_seed(
    projection_key: &str,
    frame_index: u64,
    sample_time_ticks: u64,
    projection_input: &str,
    inventory: &str,
    readback: &str,
    emitter_binding: &str,
    world: &[Value],
) -> String {
    canonical_json_hash(&json!({
        "schema_version":"RenderWorkerAnimatedSocketParticleSeed@1",
        "projection_key_sha256":projection_key,
        "frame_index":frame_index,
        "sample_time_ticks":sample_time_ticks,
        "projection_input_sha256":projection_input,
        "projection_socket_transform_inventory_sha256":inventory,
        "projection_socket_transform_readback_sha256":readback,
        "emitter_binding_sha256":emitter_binding,
        "local_particle_inventory":world
    }))
}

fn frame_local_seed(projection_key: &str, frame: &Value, emitter_binding_sha256: &str) -> String {
    canonical_json_hash(&json!({
        "schema_version":"FictionalEnergyVfxAnimatedSocketParticlesLocalOffsetSeed@1",
        "projection_key_sha256":projection_key,
        "frame_index":frame.get("frame_index"),
        "sample_time_ticks":frame.get("sample_time_ticks"),
        "projection_frame_canonical_sha256":frame.get("canonical_sha256"),
        "projection_socket_transform_inventory_sha256":frame.get("socket_transform_inventory_sha256"),
        "projection_socket_transform_readback_sha256":frame.get("socket_transform_readback_sha256"),
        "emitter_binding_sha256":emitter_binding_sha256
    }))
}

fn frame_local_seed_v2(
    projection_key: &str,
    frame: &Value,
    emitter_binding_sha256: &str,
) -> String {
    canonical_json_hash(&json!({
        "schema_version":"FictionalEnergyVfxAnimatedSocketParticlesLocalOffsetSeed@2",
        "projection_key_sha256":projection_key,
        "frame_index":frame.get("frame_index"),
        "sample_time_ticks":frame.get("sample_time_ticks"),
        "projection_frame_canonical_sha256":frame.get("projection_frame_canonical_sha256"),
        "projection_socket_transform_inventory_sha256":frame.get("socket_transform_inventory_sha256"),
        "projection_socket_transform_readback_sha256":frame.get("socket_transform_readback_sha256"),
        "emitter_binding_sha256":emitter_binding_sha256
    }))
}

fn compare_worker_replay(
    first: &render_worker::RenderWorkerAnimatedSocketParticlesFrame,
    second: &render_worker::RenderWorkerAnimatedSocketParticlesFrame,
    request: &FictionalEnergyVfxAnimatedSocketParticlesSequencePrepareRequest,
    input: &FictionalEnergyVfxAnimatedSocketParticlesSequenceFrameInput,
    projection_input_sha256: &str,
    emitter_binding_sha256: &str,
    seed_sha256: &str,
    expected_world_inventory: &Value,
    expected_world_inventory_sha256: &str,
) -> Result<(), RuntimeError> {
    if first.build_cohort_sha256.is_none()
        || first.build_cohort_sha256 != second.build_cohort_sha256
        || first.render_profile != second.render_profile
        || first.particle_count != 56
        || second.particle_count != first.particle_count
        || first.emitter_counts != [24, 32]
        || second.emitter_counts != first.emitter_counts
        || first.seed_sha256 != seed_sha256
        || second.seed_sha256 != seed_sha256
        || first.projection_key_sha256 != request.projection_key_sha256
        || second.projection_key_sha256 != first.projection_key_sha256
        || first.frame_index != input.frame_index
        || second.frame_index != first.frame_index
        || first.sample_time_ticks != input.sample_time_ticks
        || second.sample_time_ticks != first.sample_time_ticks
        || first.projection_input_sha256 != projection_input_sha256
        || second.projection_input_sha256 != first.projection_input_sha256
        || first.projection_socket_transform_inventory_sha256
            != input.projection_socket_transform_inventory_sha256
        || second.projection_socket_transform_inventory_sha256
            != first.projection_socket_transform_inventory_sha256
        || first.projection_socket_transform_readback_sha256
            != input.projection_socket_transform_readback_sha256
        || second.projection_socket_transform_readback_sha256
            != first.projection_socket_transform_readback_sha256
        || first.emitter_binding_sha256 != emitter_binding_sha256
        || second.emitter_binding_sha256 != first.emitter_binding_sha256
        || first.world_particle_inventory_sha256 != expected_world_inventory_sha256
        || second.world_particle_inventory_sha256 != first.world_particle_inventory_sha256
        || first.world_particle_inventory != *expected_world_inventory
        || second.world_particle_inventory != first.world_particle_inventory
        || first.particle_passes.len() != 3
        || first
            .particle_passes
            .iter()
            .zip(&second.particle_passes)
            .any(|(left, right)| left.pass != right.pass || left.png != right.png)
    {
        return Err(invalid(
            "animated socket particle Worker replay is not byte exact",
        ));
    }
    Ok(())
}

fn expected_world_inventory(
    request: &FictionalEnergyVfxAnimatedSocketParticlesSequencePrepareRequest,
    input: &FictionalEnergyVfxAnimatedSocketParticlesSequenceFrameInput,
    seed_sha256: &str,
    world_particles: Vec<Value>,
) -> (Value, String) {
    let inventory = json!({
        "schema_version":PARTICLE_INVENTORY_SCHEMA,
        "projection_key_sha256":request.projection_key_sha256,
        "frame_index":input.frame_index,
        "sample_time_ticks":input.sample_time_ticks,
        "seed_sha256":seed_sha256,
        "particle_count":world_particles.len(),
        "particles":world_particles,
        "canonical_sha256":""
    });
    let bytes = serde_json::to_vec(&inventory).expect("world inventory serializes");
    let mut inventory: Value =
        serde_json::from_slice(&bytes).expect("world inventory transport normalizes");
    let mut preimage = inventory
        .as_object()
        .expect("world inventory is an object")
        .clone();
    preimage.remove("canonical_sha256");
    let canonical_sha256 = canonical_json_hash(&Value::Object(preimage));
    inventory["canonical_sha256"] = Value::String(canonical_sha256.clone());
    (inventory, canonical_sha256)
}

fn expected_world_inventory_v2(
    request: &FictionalEnergyVfxAnimatedSocketParticlesSequenceV2PrepareRequest,
    input: &FictionalEnergyVfxAnimatedSocketParticlesSequenceV2FrameInput,
    seed_sha256: &str,
    world_particles: Vec<Value>,
) -> (Value, String) {
    let inventory = json!({
        "schema_version":PARTICLE_INVENTORY_SCHEMA,
        "projection_key_sha256":request.projection_key_sha256,
        "frame_index":input.frame_index,
        "sample_time_ticks":input.sample_time_ticks,
        "seed_sha256":seed_sha256,
        "particle_count":world_particles.len(),
        "particles":world_particles,
        "canonical_sha256":""
    });
    let bytes = serde_json::to_vec(&inventory).expect("V2 world inventory serializes");
    let mut inventory: Value =
        serde_json::from_slice(&bytes).expect("V2 world inventory transport normalizes");
    let mut preimage = inventory
        .as_object()
        .expect("V2 world inventory is an object")
        .clone();
    preimage.remove("canonical_sha256");
    let canonical_sha256 = canonical_json_hash(&Value::Object(preimage));
    inventory["canonical_sha256"] = Value::String(canonical_sha256.clone());
    (inventory, canonical_sha256)
}

fn compare_worker_replay_v2(
    first: &render_worker::RenderWorkerAnimatedSocketParticlesFrame,
    second: &render_worker::RenderWorkerAnimatedSocketParticlesFrame,
    request: &FictionalEnergyVfxAnimatedSocketParticlesSequenceV2PrepareRequest,
    input: &FictionalEnergyVfxAnimatedSocketParticlesSequenceV2FrameInput,
    projection_input_sha256: &str,
    emitter_binding_sha256: &str,
    seed_sha256: &str,
    expected_world_inventory: &Value,
    expected_world_inventory_sha256: &str,
) -> Result<(), RuntimeError> {
    macro_rules! require_replay {
        ($condition:expr, $field:literal) => {
            if !$condition {
                return Err(invalid(concat!(
                    "V2 animated socket particle Worker replay differs at ",
                    $field
                )));
            }
        };
    }
    require_replay!(
        first.build_cohort_sha256.is_some()
            && first.build_cohort_sha256 == second.build_cohort_sha256,
        "build cohort"
    );
    require_replay!(
        first.render_profile == second.render_profile,
        "render profile"
    );
    require_replay!(
        first.particle_count == 56 && second.particle_count == first.particle_count,
        "particle count"
    );
    require_replay!(
        first.emitter_counts == [24, 32] && second.emitter_counts == first.emitter_counts,
        "emitter counts"
    );
    require_replay!(
        first.seed_sha256 == seed_sha256 && second.seed_sha256 == seed_sha256,
        "seed"
    );
    require_replay!(
        first.projection_key_sha256 == request.projection_key_sha256
            && second.projection_key_sha256 == first.projection_key_sha256,
        "projection key"
    );
    require_replay!(
        first.frame_index == input.frame_index && second.frame_index == first.frame_index,
        "frame index"
    );
    require_replay!(
        first.sample_time_ticks == input.sample_time_ticks
            && second.sample_time_ticks == first.sample_time_ticks,
        "sample time"
    );
    require_replay!(
        first.projection_input_sha256 == projection_input_sha256
            && second.projection_input_sha256 == first.projection_input_sha256,
        "projection input"
    );
    require_replay!(
        first.projection_socket_transform_inventory_sha256
            == input.projection_socket_transform_inventory_sha256
            && second.projection_socket_transform_inventory_sha256
                == first.projection_socket_transform_inventory_sha256,
        "transform inventory"
    );
    require_replay!(
        first.projection_socket_transform_readback_sha256
            == input.projection_socket_transform_readback_sha256
            && second.projection_socket_transform_readback_sha256
                == first.projection_socket_transform_readback_sha256,
        "transform readback"
    );
    require_replay!(
        first.emitter_binding_sha256 == emitter_binding_sha256
            && second.emitter_binding_sha256 == first.emitter_binding_sha256,
        "emitter binding"
    );
    require_replay!(
        first.world_particle_inventory_sha256 == expected_world_inventory_sha256
            && second.world_particle_inventory_sha256 == first.world_particle_inventory_sha256,
        "world inventory hash"
    );
    let first_world_inventory_bytes = canonical_json_bytes(&first.world_particle_inventory)
        .map_err(|error| {
            invalid(format!(
                "V2 first world inventory is not canonical: {error}"
            ))
        })?;
    let second_world_inventory_bytes = canonical_json_bytes(&second.world_particle_inventory)
        .map_err(|error| {
            invalid(format!(
                "V2 second world inventory is not canonical: {error}"
            ))
        })?;
    let expected_world_inventory_bytes =
        canonical_json_bytes(expected_world_inventory).map_err(|error| {
            invalid(format!(
                "V2 expected world inventory is not canonical: {error}"
            ))
        })?;
    require_replay!(
        first_world_inventory_bytes == expected_world_inventory_bytes
            && second_world_inventory_bytes == first_world_inventory_bytes,
        "world inventory canonical bytes"
    );
    require_replay!(first.particle_passes.len() == 3, "particle pass count");
    require_replay!(
        first
            .particle_passes
            .iter()
            .zip(&second.particle_passes)
            .all(|(left, right)| left.pass == right.pass && left.png == right.png),
        "particle pass bytes"
    );
    Ok(())
}

fn read_v2_glb(
    runtime: &Runtime,
    sha256: &str,
    role: &str,
    allowed_kinds: &[&str],
) -> Result<Vec<u8>, RuntimeError> {
    let object = runtime
        .store
        .get_object(sha256)?
        .ok_or_else(|| invalid(format!("{role} CAS object is unavailable")))?;
    if object.schema_version != "CasObject@1"
        || object.sha256 != sha256
        || object.mime != "model/gltf-binary"
        || object.size_bytes == 0
        || object.size_bytes > MAX_GLB_BYTES
        || !allowed_kinds.contains(&object.kind.as_str())
    {
        return Err(invalid(format!("{role} GLB metadata differs")));
    }
    let bytes = runtime.cas_read_bounded(sha256, MAX_GLB_BYTES)?;
    if bytes.is_empty() || bytes.len() as u64 != object.size_bytes || sha256_hex(&bytes) != sha256 {
        return Err(invalid(format!("{role} GLB hash/readback differs")));
    }
    Ok(bytes)
}

fn read_v2_json_object(
    runtime: &Runtime,
    sha256: &str,
    role: &str,
    kind: &str,
    schema: &str,
) -> Result<Value, RuntimeError> {
    let object = runtime
        .store
        .get_object(sha256)?
        .ok_or_else(|| invalid(format!("{role} CAS object is unavailable")))?;
    if object.schema_version != "CasObject@1"
        || object.sha256 != sha256
        || object.mime != "application/json"
        || object.kind != kind
        || object.size_bytes == 0
        || object.size_bytes > MAX_JSON_BYTES
    {
        return Err(invalid(format!("{role} CAS metadata differs")));
    }
    read_canonical_json(runtime, sha256, schema)
}

fn delivery_lod0<'a>(delivery: &'a Value, role: &str) -> Result<&'a Value, RuntimeError> {
    delivery
        .get("lod_receipt")
        .and_then(|value| value.get("levels"))
        .and_then(Value::as_array)
        .filter(|levels| levels.len() == 3)
        .and_then(|levels| levels.first())
        .ok_or_else(|| invalid(format!("{role} delivery has no exact LOD0 receipt")))
}

fn validate_v2_candidate_delivery(
    runtime: &Runtime,
    project_id: &str,
    candidate_id: &str,
    candidate_state_sha256: &str,
    artifact_sha256: &str,
    delivery_manifest_sha256: &str,
    role: &str,
) -> Result<Value, RuntimeError> {
    let candidate = runtime
        .candidate(candidate_id)?
        .ok_or_else(|| invalid(format!("{role} candidate is unavailable")))?;
    if candidate.project_id != project_id
        || candidate.canonical_sha256 != candidate_state_sha256
        || candidate.prepared_object_sha256.as_deref() != Some(artifact_sha256)
    {
        return Err(invalid(format!(
            "{role} candidate/state/artifact binding differs"
        )));
    }
    let delivery = runtime.game_asset_delivery_get(&json!({
        "schema_version":"GameAssetDeliveryGetRequest@1",
        "project_id":project_id,
        "delivery_manifest_object_sha256":delivery_manifest_sha256
    }))?;
    let link = delivery
        .get("link")
        .ok_or_else(|| invalid(format!("{role} delivery link is unavailable")))?;
    if link.get("project_id").and_then(Value::as_str) != Some(project_id)
        || link
            .get("delivery_manifest_object_sha256")
            .and_then(Value::as_str)
            != Some(delivery_manifest_sha256)
    {
        return Err(invalid(format!(
            "{role} delivery project/key binding differs"
        )));
    }
    let lod0 = delivery_lod0(&delivery, role)?;
    if lod0.get("candidate_id").and_then(Value::as_str) != Some(candidate_id)
        || lod0.get("candidate_state_sha256").and_then(Value::as_str)
            != Some(candidate_state_sha256)
        || lod0.get("artifact_sha256").and_then(Value::as_str) != Some(artifact_sha256)
    {
        return Err(invalid(format!("{role} delivery LOD0 binding differs")));
    }
    Ok(delivery)
}

fn v2_anchor_projection(anchor_set: &Value) -> Result<Value, RuntimeError> {
    let anchor_ids = game_asset_delivery::socket_anchor_ids(anchor_set)?;
    let anchors = anchor_set
        .get("anchors")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("AnchorSet anchors are unavailable"))?;
    let mut normalized = Vec::with_capacity(anchor_ids.len());
    for anchor_id in anchor_ids {
        let anchor = anchors
            .iter()
            .find(|value| {
                value.get("anchor_id").and_then(Value::as_str) == Some(anchor_id.as_str())
            })
            .ok_or_else(|| invalid("AnchorSet anchor ID is missing"))?;
        let object = anchor
            .as_object()
            .ok_or_else(|| invalid("AnchorSet anchor is not an object"))?;
        let role = object
            .get("role")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid("AnchorSet anchor role is unavailable"))?;
        let parent_kind = object
            .get("parent_kind")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid("AnchorSet anchor parent kind is unavailable"))?;
        let owner_part_id = object
            .get("owner_part_id")
            .cloned()
            .ok_or_else(|| invalid("AnchorSet anchor owner Part is unavailable"))?;
        let translation = object
            .get("local_translation_m")
            .ok_or_else(|| invalid("AnchorSet local translation is unavailable"))?;
        let rotation = object
            .get("local_rotation_quat_xyzw")
            .ok_or_else(|| invalid("AnchorSet local rotation is unavailable"))?;
        let scale = object
            .get("local_scale_xyz")
            .ok_or_else(|| invalid("AnchorSet local scale is unavailable"))?;
        let _ = f32_array(&json!({"value":translation}), "value", 3, 100.0)?;
        let quat = f32_array(&json!({"value":rotation}), "value", 4, 1.0)?;
        let norm = quat.iter().map(|value| value * value).sum::<f32>().sqrt();
        if (norm - 1.0).abs() > 1e-3 {
            return Err(invalid("AnchorSet local rotation is not unit length"));
        }
        let scale_values = f32_array(&json!({"value":scale}), "value", 3, 2.0)?;
        if scale_values.iter().any(|value| (*value - 1.0).abs() > 1e-6) {
            return Err(invalid("AnchorSet local scale is not identity"));
        }
        normalized.push(json!({
            "anchor_id":anchor_id,
            "role":role,
            "parent_kind":parent_kind,
            "owner_part_id":owner_part_id,
            "local_translation_m":translation,
            "local_rotation_quat_xyzw":rotation,
            "local_scale_xyz":scale
        }));
    }
    Ok(json!({
        "schema_version":"GameWeaponAnchorProjection@1",
        "anchors":normalized
    }))
}

fn validate_v2_anchor(
    result: &Value,
    project_id: &str,
    delivery_manifest_sha256: &str,
    anchor_object_sha256: &str,
    anchor_canonical_sha256: &str,
    role: &str,
) -> Result<(Value, Value), RuntimeError> {
    let link = result
        .get("link")
        .ok_or_else(|| invalid(format!("{role} AnchorSet link is unavailable")))?;
    let anchor_set = result
        .get("anchor_set")
        .cloned()
        .ok_or_else(|| invalid(format!("{role} AnchorSet is unavailable")))?;
    if link.get("project_id").and_then(Value::as_str) != Some(project_id)
        || link
            .get("delivery_manifest_object_sha256")
            .and_then(Value::as_str)
            != Some(delivery_manifest_sha256)
        || link.get("anchor_set_object_sha256").and_then(Value::as_str)
            != Some(anchor_object_sha256)
        || anchor_set.get("project_id").and_then(Value::as_str) != Some(project_id)
        || anchor_set
            .get("delivery_manifest_object_sha256")
            .and_then(Value::as_str)
            != Some(delivery_manifest_sha256)
        || anchor_set.get("canonical_sha256").and_then(Value::as_str)
            != Some(anchor_canonical_sha256)
        || result.get("restart_hash_verified").and_then(Value::as_bool) != Some(true)
    {
        return Err(invalid(format!("{role} AnchorSet binding differs")));
    }
    let projection = v2_anchor_projection(&anchor_set)?;
    Ok((anchor_set, projection))
}

fn validate_v2_material_quality(
    runtime: &Runtime,
    request: &FictionalEnergyVfxAnimatedSocketParticlesSequenceV2PrepareRequest,
) -> Result<Value, RuntimeError> {
    let result = runtime.candidate_material_surface_quality_get(json!({
        "schema_version":"CandidateMaterialSurfaceQualityGetRequest@1",
        "material_surface_quality_id":request.material_surface_quality_id,
        "project_id":request.project_id,
        "source_candidate_id":request.geometry_candidate_id,
        "output_candidate_id":request.appearance_candidate_id
    }))?;
    if result.get("runtime_write").and_then(Value::as_bool) != Some(false) {
        return Err(invalid("material-surface quality get unexpectedly writes"));
    }
    let record = result
        .get("material_surface_quality")
        .cloned()
        .ok_or_else(|| invalid("material-surface quality record is unavailable"))?;
    for (field, expected) in [
        ("project_id", request.project_id.as_str()),
        (
            "source_candidate_id",
            request.geometry_candidate_id.as_str(),
        ),
        (
            "source_candidate_state_sha256",
            request.geometry_candidate_state_sha256.as_str(),
        ),
        (
            "source_artifact_sha256",
            request.geometry_artifact_sha256.as_str(),
        ),
        (
            "output_candidate_id",
            request.appearance_candidate_id.as_str(),
        ),
        (
            "output_candidate_state_sha256",
            request.appearance_candidate_state_sha256.as_str(),
        ),
        (
            "output_artifact_sha256",
            request.appearance_artifact_sha256.as_str(),
        ),
        (
            "geometry_preservation_status",
            V2_GEOMETRY_PRESERVATION_STATUS,
        ),
        (
            "canonical_sha256",
            request.material_surface_quality_canonical_sha256.as_str(),
        ),
    ] {
        expect_same(&record, field, expected)?;
    }
    if record.get("hard_gate_passed").and_then(Value::as_bool) != Some(true)
        || record.get("validator_status").and_then(Value::as_str) != Some("passed")
        || record
            .get("source_output_candidate_binding_status")
            .and_then(Value::as_str)
            != Some("distinct-candidates-verified")
    {
        return Err(invalid("material-surface quality hard gate is not passed"));
    }
    let report = read_v2_json_object(
        runtime,
        &request.material_surface_quality_report_object_sha256,
        "material-surface quality report",
        "candidate-material-surface-quality-report",
        "CandidateMaterialSurfaceQuality@1",
    )?;
    if report != record
        || report.get("canonical_sha256").and_then(Value::as_str)
            != Some(request.material_surface_quality_canonical_sha256.as_str())
    {
        return Err(invalid("material-surface quality report readback differs"));
    }
    Ok(record)
}

fn validate_v2_vfx_profile(
    request: &FictionalEnergyVfxAnimatedSocketParticlesSequenceV2PrepareRequest,
    vfx: &Value,
) -> Result<Value, RuntimeError> {
    let link = vfx
        .get("link")
        .ok_or_else(|| invalid("appearance VFX profile link is unavailable"))?;
    if link.get("project_id").and_then(Value::as_str) != Some(request.project_id.as_str())
        || link
            .get("delivery_manifest_object_sha256")
            .and_then(Value::as_str)
            != Some(request.appearance_delivery_manifest_object_sha256.as_str())
        || link
            .get("vfx_profile_object_sha256")
            .and_then(Value::as_str)
            != Some(request.vfx_profile_object_sha256.as_str())
        || link.get("anchor_set_object_sha256").and_then(Value::as_str)
            != Some(request.appearance_anchor_set_object_sha256.as_str())
    {
        return Err(invalid("appearance VFX profile binding differs"));
    }
    let profile = vfx
        .get("vfx_profile")
        .cloned()
        .ok_or_else(|| invalid("appearance VFX profile is unavailable"))?;
    if profile.get("project_id").and_then(Value::as_str) != Some(request.project_id.as_str())
        || profile
            .get("delivery_manifest_object_sha256")
            .and_then(Value::as_str)
            != Some(request.appearance_delivery_manifest_object_sha256.as_str())
        || profile.get("canonical_sha256").and_then(Value::as_str)
            != Some(request.vfx_profile_canonical_sha256.as_str())
    {
        return Err(invalid("appearance VFX profile canonical binding differs"));
    }
    Ok(profile)
}

fn validate_v2_projection_parent(
    request: &FictionalEnergyVfxAnimatedSocketParticlesSequenceV2PrepareRequest,
    projection: &Value,
) -> Result<String, RuntimeError> {
    for (field, expected) in [
        ("schema_version", V2_PROJECTION_SCHEMA),
        (
            "projection_key_sha256",
            request.projection_key_sha256.as_str(),
        ),
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
            "animated_socket_materialization_key_sha256",
            request.animated_socket_materialization_key_sha256.as_str(),
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
            "anchor_set_object_sha256",
            request.appearance_anchor_set_object_sha256.as_str(),
        ),
        (
            "anchor_set_canonical_sha256",
            request.appearance_anchor_set_canonical_sha256.as_str(),
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
        ("frame_scope", PROJECTION_FRAME_SCOPE),
        ("part_hierarchy_policy", V2_PROJECTION_PART_HIERARCHY_POLICY),
        (
            "transform_representation_policy",
            V2_PROJECTION_REPRESENTATION_POLICY,
        ),
        (
            "transform_projection_policy",
            V2_PROJECTION_TRANSFORM_POLICY,
        ),
    ] {
        expect_same(projection, field, expected)?;
    }
    if projection.get("canonical_sha256").and_then(Value::as_str)
        != Some(request.projection_canonical_sha256.as_str())
        || projection
            .get("socket_node_id_encoding_sha256")
            .and_then(Value::as_str)
            != Some(request.socket_node_id_encoding_sha256.as_str())
        || projection
            .get("socket_roles_sha256")
            .and_then(Value::as_str)
            != Some(request.socket_roles_sha256.as_str())
        || projection.get("socket_roles")
            != Some(&json!([
                "weapon-root",
                "grip-primary",
                "muzzle-vfx",
                "magazine-well",
                "sight-primary",
                "energy-core-vfx"
            ]))
        || projection
            .get("sampling_policy_sha256")
            .and_then(Value::as_str)
            .is_none()
        || projection
            .get("input_sha256")
            .and_then(Value::as_str)
            .is_none_or(|value| !is_sha256(value))
        || projection.get("sample_count").and_then(Value::as_u64) != Some(request.sample_count)
        || projection.get("sample_time_ticks") != Some(&json!(request.sample_time_ticks))
        || projection
            .get("frames")
            .and_then(Value::as_array)
            .map_or(true, |frames| frames.len() != request.sample_count as usize)
    {
        return Err(invalid(
            "Projection@2 appearance binding, schedule or canonical binding differs",
        ));
    }
    projection
        .get("input_sha256")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| invalid("geometry projection input hash is unavailable"))
}

fn validate_v2_animation_clip(
    runtime: &Runtime,
    request: &FictionalEnergyVfxAnimatedSocketParticlesSequenceV2PrepareRequest,
) -> Result<Value, RuntimeError> {
    let result = runtime.mechanical_animation_clip_v2_get(&json!({
        "schema_version":"MechanicalAnimationClipGetRequest@2",
        "project_id":request.project_id,
        "appearance_candidate_id":request.appearance_candidate_id,
        "clip_id":request.animation_clip_id
    }))?;
    if result.get("schema_version").and_then(Value::as_str)
        != Some("MechanicalAnimationClipGetResult@2")
        || result
            .get("runtime_write_performed")
            .and_then(Value::as_bool)
            != Some(false)
        || result.get("restart_hash_verified").and_then(Value::as_bool) != Some(true)
    {
        return Err(invalid(
            "MechanicalAnimationClip@2 get is not read-only verified",
        ));
    }
    let link = result
        .get("durable_link")
        .ok_or_else(|| invalid("MechanicalAnimationClip@2 durable link is unavailable"))?;
    if link.get("schema_version").and_then(Value::as_str) != Some("MechanicalAnimationClipLink@2")
        || result
            .get("clip")
            .and_then(|clip| clip.get("schema_version"))
            .and_then(Value::as_str)
            != Some("MechanicalAnimationClip@2")
        || link.get("project_id").and_then(Value::as_str) != Some(request.project_id.as_str())
        || link.get("appearance_candidate_id").and_then(Value::as_str)
            != Some(request.appearance_candidate_id.as_str())
        || link
            .get("source_geometry_candidate_id")
            .and_then(Value::as_str)
            != Some(request.geometry_candidate_id.as_str())
        || link
            .get("source_geometry_candidate_state_sha256")
            .and_then(Value::as_str)
            != Some(request.geometry_candidate_state_sha256.as_str())
        || link
            .get("source_geometry_artifact_sha256")
            .and_then(Value::as_str)
            != Some(request.geometry_artifact_sha256.as_str())
        || link.get("clip_id").and_then(Value::as_str) != Some(request.animation_clip_id.as_str())
        || link.get("clip_object_sha256").and_then(Value::as_str)
            != Some(request.animation_clip_object_sha256.as_str())
        || link.get("clip_sha256").and_then(Value::as_str)
            != Some(request.animation_clip_canonical_sha256.as_str())
        || result
            .get("clip")
            .and_then(|clip| clip.get("canonical_sha256"))
            .and_then(Value::as_str)
            != Some(request.animation_clip_canonical_sha256.as_str())
    {
        return Err(invalid("MechanicalAnimationClip@2 binding differs"));
    }
    Ok(result)
}

fn validate_v2_animated_socket(
    runtime: &Runtime,
    request: &FictionalEnergyVfxAnimatedSocketParticlesSequenceV2PrepareRequest,
) -> Result<Value, RuntimeError> {
    let result = runtime.game_weapon_animated_glb_socket_v2_get(&json!({
        "schema_version":"GameWeaponAnimatedGlbSocketMaterializationGetRequest@2",
        "project_id":request.project_id,
        "appearance_candidate_id":request.appearance_candidate_id,
        "clip_id":request.animation_clip_id,
        "animated_socket_materialization_key_sha256":request.animated_socket_materialization_key_sha256
    }))?;
    if result.get("schema_version").and_then(Value::as_str)
        != Some("GameWeaponAnimatedGlbSocketMaterializationGetResult@2")
        || result
            .get("runtime_write_performed")
            .and_then(Value::as_bool)
            != Some(false)
        || result.get("restart_hash_verified").and_then(Value::as_bool) != Some(true)
        || result
            .get("actual_engine_roundtrip")
            .and_then(Value::as_bool)
            != Some(false)
    {
        return Err(invalid(
            "AnimatedSocket@2 get is not a read-only verified result",
        ));
    }
    let link = result
        .get("durable_link")
        .ok_or_else(|| invalid("AnimatedSocket@2 durable link is unavailable"))?;
    for (field, expected) in [
        (
            "schema_version",
            "GameWeaponAnimatedGlbSocketMaterializationLink@2",
        ),
        (
            "animated_socket_materialization_key_sha256",
            request.animated_socket_materialization_key_sha256.as_str(),
        ),
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
        (
            "animation_receipt_canonical_sha256",
            request.animation_receipt_canonical_sha256.as_str(),
        ),
        ("clip_id", request.animation_clip_id.as_str()),
        (
            "clip_object_sha256",
            request.animation_clip_object_sha256.as_str(),
        ),
        ("clip_sha256", request.animation_clip_canonical_sha256.as_str()),
        (
            "socket_materialization_policy",
            "appearance-aware-animation-v2-socket-node-materialization-preserve-renderable-content@2",
        ),
        ("lod_scope", "lod0-appearance-animated-source-only@2"),
    ] {
        expect_same(link, field, expected)?;
    }
    if link.get("validator_status").and_then(Value::as_str)
        != Some("strict-appearance-aware-animated-glb-socket-materialization-readback-pass")
        || link.get("hard_gate_passed").and_then(Value::as_bool) != Some(true)
        || link.get("quality_status").and_then(Value::as_str) != Some("structural_only")
        || result
            .get("derived_animated_socket_artifact_sha256")
            .and_then(Value::as_str)
            .is_none_or(|value| !is_sha256(value))
    {
        return Err(invalid("AnimatedSocket@2 durable link status differs"));
    }
    let receipt = result
        .get("receipt")
        .ok_or_else(|| invalid("AnimatedSocket@2 receipt is unavailable"))?;
    if receipt.get("schema_version").and_then(Value::as_str)
        != Some("GameWeaponAnimatedGlbSocketMaterializationReceipt@2")
        || receipt
            .get("animated_socket_materialization_key_sha256")
            .and_then(Value::as_str)
            != Some(request.animated_socket_materialization_key_sha256.as_str())
        || receipt.get("project_id").and_then(Value::as_str) != Some(request.project_id.as_str())
        || receipt
            .get("appearance_candidate_id")
            .and_then(Value::as_str)
            != Some(request.appearance_candidate_id.as_str())
        || receipt
            .get("appearance_candidate_state_sha256")
            .and_then(Value::as_str)
            != Some(request.appearance_candidate_state_sha256.as_str())
        || receipt
            .get("appearance_delivery_manifest_object_sha256")
            .and_then(Value::as_str)
            != Some(request.appearance_delivery_manifest_object_sha256.as_str())
        || receipt
            .get("appearance_artifact_sha256")
            .and_then(Value::as_str)
            != Some(request.appearance_artifact_sha256.as_str())
        || receipt
            .get("anchor_set_object_sha256")
            .and_then(Value::as_str)
            != Some(request.appearance_anchor_set_object_sha256.as_str())
        || receipt
            .get("anchor_set_canonical_sha256")
            .and_then(Value::as_str)
            != Some(request.appearance_anchor_set_canonical_sha256.as_str())
        || receipt.get("socket_node_count").and_then(Value::as_u64) != Some(6)
        || receipt
            .get("socket_nodes")
            .and_then(Value::as_array)
            .map_or(true, |nodes| nodes.len() != 6)
    {
        return Err(invalid("AnimatedSocket@2 receipt binding differs"));
    }
    let nodes = receipt
        .get("socket_nodes")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("AnimatedSocket@2 socket node inventory is unavailable"))?;
    let mut roles = nodes
        .iter()
        .map(|node| {
            if node.get("node_kind").and_then(Value::as_str) != Some("empty")
                || node
                    .get("socket_node_id")
                    .and_then(Value::as_str)
                    .is_none_or(|value| !is_opaque_id(value))
                || node
                    .get("anchor_id")
                    .and_then(Value::as_str)
                    .is_none_or(|value| !is_opaque_id(value))
            {
                return Err(invalid("AnimatedSocket@2 socket node identity differs"));
            }
            node.get("role")
                .and_then(Value::as_str)
                .ok_or_else(|| invalid("AnimatedSocket@2 socket role is unavailable"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    roles.sort_unstable();
    if roles
        != [
            "energy-core-vfx",
            "grip-primary",
            "magazine-well",
            "muzzle-vfx",
            "sight-primary",
            "weapon-root",
        ]
    {
        return Err(invalid(
            "AnimatedSocket@2 must contain the exact six socket roles",
        ));
    }
    Ok(result)
}

fn validate_v2_projection_frame(
    input: &FictionalEnergyVfxAnimatedSocketParticlesSequenceV2FrameInput,
    projection_frame: &Value,
) -> Result<(), RuntimeError> {
    if projection_frame
        .get("schema_version")
        .and_then(Value::as_str)
        != Some(V2_PROJECTION_FRAME_SCHEMA)
        || projection_frame.get("frame_index").and_then(Value::as_u64) != Some(input.frame_index)
        || projection_frame
            .get("sample_time_ticks")
            .and_then(Value::as_u64)
            != Some(input.sample_time_ticks)
        || projection_frame
            .get("socket_transform_inventory_sha256")
            .and_then(Value::as_str)
            != Some(input.projection_socket_transform_inventory_sha256.as_str())
        || projection_frame
            .get("socket_transform_readback_sha256")
            .and_then(Value::as_str)
            != Some(input.projection_socket_transform_readback_sha256.as_str())
        || projection_frame
            .get("projection_frame_canonical_sha256")
            .and_then(Value::as_str)
            != Some(input.projection_frame_canonical_sha256.as_str())
        || projection_frame
            .get("canonical_sha256")
            .and_then(Value::as_str)
            .is_none_or(|value| !is_sha256(value))
        || projection_frame
            .get("socket_transform_inventory_sha256")
            .and_then(Value::as_str)
            .is_none_or(|value| !is_sha256(value))
        || projection_frame
            .get("socket_transform_readback_sha256")
            .and_then(Value::as_str)
            .is_none_or(|value| !is_sha256(value))
        || projection_frame
            .get("projection_frame_canonical_sha256")
            .and_then(Value::as_str)
            .is_none_or(|value| !is_sha256(value))
        || projection_frame
            .get("source_animation_sample_sha256")
            .and_then(Value::as_str)
            .is_none_or(|value| !is_sha256(value))
        || projection_frame
            .get("derived_socket_sample_sha256")
            .and_then(Value::as_str)
            .is_none_or(|value| !is_sha256(value))
        || projection_frame
            .get("socket_transforms")
            .and_then(Value::as_array)
            .map_or(true, |sockets| sockets.len() != 6)
    {
        return Err(invalid("V2 geometry projection frame binding differs"));
    }
    let sockets = projection_frame
        .get("socket_transforms")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("V2 projection socket transforms are unavailable"))?;
    let expected_roles = [
        "weapon-root",
        "grip-primary",
        "muzzle-vfx",
        "magazine-well",
        "sight-primary",
        "energy-core-vfx",
    ];
    let mut inventory = Vec::with_capacity(sockets.len());
    for socket in sockets {
        let object = socket
            .as_object()
            .ok_or_else(|| invalid("V2 projection socket is not an object"))?;
        let role = object
            .get("role")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid("V2 projection socket role is unavailable"))?;
        if !expected_roles.contains(&role)
            || object.get("node_kind").and_then(Value::as_str) != Some("empty")
            || object
                .get("socket_node_id")
                .and_then(Value::as_str)
                .is_none_or(|value| !is_opaque_id(value))
            || object
                .get("anchor_id")
                .and_then(Value::as_str)
                .is_none_or(|value| !is_opaque_id(value))
        {
            return Err(invalid("V2 projection socket identity differs"));
        }
        inventory.push(json!({
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
        }));
    }
    let mut sorted_roles = sockets
        .iter()
        .filter_map(|socket| socket.get("role").and_then(Value::as_str))
        .collect::<Vec<_>>();
    sorted_roles.sort_unstable();
    let mut expected_sorted_roles = expected_roles.to_vec();
    expected_sorted_roles.sort_unstable();
    if sorted_roles != expected_sorted_roles {
        return Err(invalid(
            "V2 projection must contain the exact six socket roles",
        ));
    }
    let inventory_sha256 = canonical_json_hash(&Value::Array(inventory));
    if inventory_sha256
        != projection_frame
            .get("socket_transform_inventory_sha256")
            .and_then(Value::as_str)
            .unwrap_or_default()
    {
        return Err(invalid("V2 projection socket inventory readback differs"));
    }
    let mut readback = projection_frame.clone();
    let readback_object = readback
        .as_object_mut()
        .ok_or_else(|| invalid("V2 projection frame is not an object"))?;
    readback_object.insert("created_at".to_owned(), Value::String(String::new()));
    readback_object.insert("canonical_sha256".to_owned(), Value::String(String::new()));
    readback_object.insert(
        "projection_frame_canonical_sha256".to_owned(),
        Value::String(String::new()),
    );
    readback_object.insert(
        "socket_transform_readback_sha256".to_owned(),
        Value::String(String::new()),
    );
    let readback_sha256 = canonical_json_hash(&readback);
    if readback_sha256
        != projection_frame
            .get("socket_transform_readback_sha256")
            .and_then(Value::as_str)
            .unwrap_or_default()
    {
        return Err(invalid("V2 projection socket transform readback differs"));
    }
    let projection_frame_sha256 = readback_sha256_from_projection_frame(projection_frame)?;
    if projection_frame_sha256
        != projection_frame
            .get("projection_frame_canonical_sha256")
            .and_then(Value::as_str)
            .unwrap_or_default()
    {
        return Err(invalid("V2 projection frame canonical readback differs"));
    }
    let mut canonical = projection_frame.clone();
    canonical
        .as_object_mut()
        .ok_or_else(|| invalid("V2 projection frame is not an object"))?
        .insert("canonical_sha256".to_owned(), Value::String(String::new()));
    if canonical_json_hash(&canonical)
        != projection_frame
            .get("canonical_sha256")
            .and_then(Value::as_str)
            .unwrap_or_default()
    {
        return Err(invalid("V2 projection frame canonical hash differs"));
    }
    Ok(())
}

fn readback_sha256_from_projection_frame(frame: &Value) -> Result<String, RuntimeError> {
    let mut projection_frame = frame.clone();
    let object = projection_frame
        .as_object_mut()
        .ok_or_else(|| invalid("V2 projection frame is not an object"))?;
    object.insert("created_at".to_owned(), Value::String(String::new()));
    object.insert("canonical_sha256".to_owned(), Value::String(String::new()));
    object.insert(
        "projection_frame_canonical_sha256".to_owned(),
        Value::String(String::new()),
    );
    Ok(canonical_json_hash(&projection_frame))
}

fn camera_for_v2_base(
    runtime: &Runtime,
    request: &FictionalEnergyVfxAnimatedSocketParticlesSequenceV2PrepareRequest,
    base: &FictionalEnergyVfxFrameLinkRecord,
) -> Result<Value, RuntimeError> {
    if base.project_id != request.project_id
        || base.delivery_manifest_object_sha256
            != request.appearance_delivery_manifest_object_sha256
        || base.source_candidate_id != request.appearance_candidate_id
        || base.source_artifact_sha256 != request.appearance_artifact_sha256
        || base.vfx_profile_object_sha256 != request.vfx_profile_object_sha256
        || base.camera_object_sha256 != request.camera_object_sha256
        || base.camera_identity_sha256 != request.camera_identity_sha256
        || base.render_profile_sha256 != request.render_profile_sha256
        || base.render_worker_build_cohort_sha256 != request.render_worker_build_cohort_sha256
    {
        return Err(invalid(
            "appearance base frame camera/cohort binding differs",
        ));
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
        return Err(invalid("appearance camera identity differs"));
    }
    Ok(camera)
}

fn validate_v2_base_bloom(
    request: &FictionalEnergyVfxAnimatedSocketParticlesSequenceV2PrepareRequest,
    input: &FictionalEnergyVfxAnimatedSocketParticlesSequenceV2FrameInput,
    base: &FictionalEnergyVfxFrameLinkRecord,
    bloom: &FictionalEnergyVfxBloomFrameLinkRecord,
) -> Result<(), RuntimeError> {
    if base.frame_key_sha256 != input.base_frame_key_sha256
        || bloom.bloom_key_sha256 != input.bloom_key_sha256
        || bloom.base_frame_key_sha256 != base.frame_key_sha256
        || base.project_id != request.project_id
        || bloom.project_id != request.project_id
        || base.delivery_manifest_object_sha256
            != request.appearance_delivery_manifest_object_sha256
        || bloom.delivery_manifest_object_sha256
            != request.appearance_delivery_manifest_object_sha256
        || base.vfx_profile_object_sha256 != request.vfx_profile_object_sha256
        || bloom.vfx_profile_object_sha256 != request.vfx_profile_object_sha256
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
    {
        return Err(invalid("appearance base/Bloom binding differs"));
    }
    Ok(())
}

fn build_context_v2(
    runtime: &Runtime,
    request: &FictionalEnergyVfxAnimatedSocketParticlesSequenceV2PrepareRequest,
) -> Result<V2DependencyContext, RuntimeError> {
    // Everything above this line is read-only.  In particular, candidate,
    // quality, delivery, AnchorSet, animation and VFX links are all replayed
    // before a CAS reservation can be opened by prepare().
    let quality = validate_v2_material_quality(runtime, request)?;
    let geometry_delivery = validate_v2_candidate_delivery(
        runtime,
        &request.project_id,
        &request.geometry_candidate_id,
        &request.geometry_candidate_state_sha256,
        &request.geometry_artifact_sha256,
        &request.geometry_delivery_manifest_object_sha256,
        "geometry",
    )?;
    let appearance_delivery = validate_v2_candidate_delivery(
        runtime,
        &request.project_id,
        &request.appearance_candidate_id,
        &request.appearance_candidate_state_sha256,
        &request.appearance_artifact_sha256,
        &request.appearance_delivery_manifest_object_sha256,
        "appearance",
    )?;
    let geometry_anchor_result = runtime.game_weapon_anchor_get(&json!({
        "schema_version":"GameWeaponAnchorGetRequest@1",
        "project_id":request.project_id,
        "delivery_manifest_object_sha256":request.geometry_delivery_manifest_object_sha256
    }))?;
    let (geometry_anchor, geometry_anchor_projection) = validate_v2_anchor(
        &geometry_anchor_result,
        &request.project_id,
        &request.geometry_delivery_manifest_object_sha256,
        &request.animated_socket_anchor_set_object_sha256,
        &request.animated_socket_anchor_set_canonical_sha256,
        "geometry",
    )?;
    let appearance_anchor_result = runtime.game_weapon_anchor_get(&json!({
        "schema_version":"GameWeaponAnchorGetRequest@1",
        "project_id":request.project_id,
        "delivery_manifest_object_sha256":request.appearance_delivery_manifest_object_sha256
    }))?;
    let (appearance_anchor, appearance_anchor_projection) = validate_v2_anchor(
        &appearance_anchor_result,
        &request.project_id,
        &request.appearance_delivery_manifest_object_sha256,
        &request.appearance_anchor_set_object_sha256,
        &request.appearance_anchor_set_canonical_sha256,
        "appearance",
    )?;
    if geometry_anchor_projection != appearance_anchor_projection {
        return Err(invalid(
            "geometry and appearance AnchorSet owner/TRS projections differ",
        ));
    }
    let anchor_binding_sha256 = canonical_json_hash(&json!({
        "schema_version":"FictionalEnergyVfxAnimatedSocketParticlesAnchorBinding@1",
        "policy":V2_ANCHOR_BINDING_POLICY,
        "geometry_anchor_set_object_sha256":request.animated_socket_anchor_set_object_sha256,
        "geometry_anchor_set_canonical_sha256":request.animated_socket_anchor_set_canonical_sha256,
        "appearance_anchor_set_object_sha256":request.appearance_anchor_set_object_sha256,
        "appearance_anchor_set_canonical_sha256":request.appearance_anchor_set_canonical_sha256,
        "projection":geometry_anchor_projection
    }));
    let _ = (
        &geometry_anchor,
        &appearance_anchor,
        &geometry_delivery,
        &appearance_delivery,
    );

    let _clip = validate_v2_animation_clip(runtime, request)?;
    let _animated_socket = validate_v2_animated_socket(runtime, request)?;
    let projection_result =
        runtime.game_weapon_animated_glb_socket_transform_projection_v2_get(&json!({
            "schema_version":"GameWeaponAnimatedGlbSocketTransformProjectionGetRequest@2",
            "projection_key_sha256":request.projection_key_sha256,
            "project_id":request.project_id,
            "appearance_candidate_id":request.appearance_candidate_id,
            "animation_clip_id":request.animation_clip_id
        }))?;
    if projection_result
        .get("schema_version")
        .and_then(Value::as_str)
        != Some("GameWeaponAnimatedGlbSocketTransformProjectionGetResult@2")
        || projection_result
            .get("runtime_write_performed")
            .and_then(Value::as_bool)
            != Some(false)
        || projection_result
            .get("restart_hash_verified")
            .and_then(Value::as_bool)
            != Some(true)
    {
        return Err(invalid(
            "Projection@2 get is not a read-only verified result",
        ));
    }
    if projection_result
        .get("projection_object_sha256")
        .and_then(Value::as_str)
        != Some(request.projection_object_sha256.as_str())
    {
        return Err(invalid("geometry projection object binding differs"));
    }
    let projection = projection_result
        .get("projection")
        .cloned()
        .ok_or_else(|| invalid("Projection@2 payload is unavailable"))?;
    let projection_object = read_v2_json_object(
        runtime,
        &request.projection_object_sha256,
        "Projection@2 object",
        "game-weapon-animated-glb-v2-socket-transform-projection",
        V2_PROJECTION_SCHEMA,
    )?;
    if !canonical_values_equal(&projection_object, &projection)? {
        return Err(invalid(
            "Projection@2 CAS payload differs from get readback",
        ));
    }
    let projection_input_sha256 = validate_v2_projection_parent(request, &projection)?;
    let vfx = runtime.fictional_energy_vfx_get(&json!({
        "schema_version":"FictionalEnergyVfxGetRequest@1",
        "project_id":request.project_id,
        "delivery_manifest_object_sha256":request.appearance_delivery_manifest_object_sha256
    }))?;
    let _profile = validate_v2_vfx_profile(request, &vfx)?;

    let _geometry_glb = read_v2_glb(
        runtime,
        &request.geometry_artifact_sha256,
        "geometry source",
        &["geometry-glb"],
    )?;
    let appearance_glb = read_v2_glb(
        runtime,
        &request.appearance_artifact_sha256,
        "appearance source",
        &["appearance-glb", "appearance-v2-glb"],
    )?;
    let _animated_glb = read_v2_glb(
        runtime,
        &request.animated_artifact_sha256,
        "animated source",
        &["mechanical-animation-glb-v2"],
    )?;

    let mut frames = Vec::with_capacity(request.frames.len());
    let mut cohort = None::<String>;
    for input in &request.frames {
        let projection_frame =
            v2_frame_projection_by_index(&projection, input.frame_index)?.clone();
        if projection_frame
            .get("projection_key_sha256")
            .and_then(Value::as_str)
            != Some(request.projection_key_sha256.as_str())
        {
            return Err(invalid("Projection@2 frame projection key differs"));
        }
        validate_v2_projection_frame(input, &projection_frame)?;
        let base_value = runtime.fictional_energy_vfx_rendered_frame_get(&json!({
            "schema_version":"FictionalEnergyVfxRenderedFrameGetRequest@1",
            "project_id":request.project_id,
            "frame_key_sha256":input.base_frame_key_sha256
        }))?;
        let base: FictionalEnergyVfxFrameLinkRecord = serde_json::from_value(
            base_value
                .get("link")
                .cloned()
                .ok_or_else(|| invalid("appearance base frame link is unavailable"))?,
        )
        .map_err(|error| invalid(format!("appearance base frame link is malformed: {error}")))?;
        let bloom_value = runtime.fictional_energy_vfx_hdr_bloom_get(&json!({
            "schema_version":"FictionalEnergyVfxHdrBloomFrameGetRequest@1",
            "project_id":request.project_id,
            "bloom_key_sha256":input.bloom_key_sha256
        }))?;
        let bloom: FictionalEnergyVfxBloomFrameLinkRecord = serde_json::from_value(
            bloom_value
                .get("link")
                .cloned()
                .ok_or_else(|| invalid("appearance Bloom frame link is unavailable"))?,
        )
        .map_err(|error| invalid(format!("appearance Bloom frame link is malformed: {error}")))?;
        validate_v2_base_bloom(request, input, &base, &bloom)?;
        let camera = camera_for_v2_base(runtime, request, &base)?;
        let bindings = build_emitter_bindings(&projection_frame)?;
        let binding_hash = canonical_json_hash(&bindings);
        let local_seed = frame_local_seed_v2(
            &request.projection_key_sha256,
            &projection_frame,
            &binding_hash,
        );
        let particles = build_particles(&bindings, &local_seed)?;
        let world = world_values(&bindings, &particles, &camera, &local_seed)?;
        let seed = worker_seed(
            &request.projection_key_sha256,
            input.frame_index,
            input.sample_time_ticks,
            &projection_input_sha256,
            &input.projection_socket_transform_inventory_sha256,
            &input.projection_socket_transform_readback_sha256,
            &binding_hash,
            &world,
        );
        let (expected_inventory, expected_inventory_sha256) =
            expected_world_inventory_v2(request, input, &seed, world);
        // V2 intentionally feeds the appearance GLB to the Worker.  The
        // material-surface quality gate above proves its renderable geometry
        // projection is byte-exact with geometry, while its materials remain
        // the appearance candidate's responsibility.
        let first = render_worker::render_typed_animated_socket_particles_with_worker_identity(
            &appearance_glb,
            &camera,
            &request.projection_key_sha256,
            input.frame_index,
            input.sample_time_ticks,
            &projection_input_sha256,
            &input.projection_socket_transform_inventory_sha256,
            &input.projection_socket_transform_readback_sha256,
            &bindings,
            &particles,
            &seed,
        )
        .map_err(|error| {
            invalid(format!(
                "V2 animated socket particle render failed: {error}"
            ))
        })?;
        let second = render_worker::render_typed_animated_socket_particles_with_worker_identity(
            &appearance_glb,
            &camera,
            &request.projection_key_sha256,
            input.frame_index,
            input.sample_time_ticks,
            &projection_input_sha256,
            &input.projection_socket_transform_inventory_sha256,
            &input.projection_socket_transform_readback_sha256,
            &bindings,
            &particles,
            &seed,
        )
        .map_err(|error| {
            invalid(format!(
                "V2 animated socket particle replay failed: {error}"
            ))
        })?;
        compare_worker_replay_v2(
            &first,
            &second,
            request,
            input,
            &projection_input_sha256,
            &binding_hash,
            &seed,
            &expected_inventory,
            &expected_inventory_sha256,
        )?;
        let this_cohort = first
            .build_cohort_sha256
            .clone()
            .ok_or_else(|| invalid("V2 animated socket Worker cohort is unavailable"))?;
        if this_cohort != request.render_worker_build_cohort_sha256
            || first
                .render_profile
                .get("canonical_sha256")
                .and_then(Value::as_str)
                != Some(request.render_profile_sha256.as_str())
        {
            return Err(invalid(
                "V2 animated socket Worker cohort or profile differs",
            ));
        }
        if cohort
            .as_deref()
            .is_some_and(|value| value != this_cohort.as_str())
        {
            return Err(invalid(
                "V2 animated socket Worker cohort changes across frames",
            ));
        }
        cohort = Some(this_cohort);
        frames.push(V2FrameContext {
            input: input.clone(),
            projection_frame,
            emitter_bindings: bindings,
            particles,
            seed_sha256: seed,
            worker: first,
        });
    }
    Ok(V2DependencyContext {
        frames,
        worker_cohort: cohort.ok_or_else(|| invalid("V2 Worker cohort is missing"))?,
        anchor_binding_sha256,
        geometry_preservation_projection_sha256: quality
            .get("geometry_preservation_projection_sha256")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid("geometry preservation projection is unavailable"))?
            .to_owned(),
        geometry_preservation_status: quality
            .get("geometry_preservation_status")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid("geometry preservation status is unavailable"))?
            .to_owned(),
    })
}

fn build_context(
    runtime: &Runtime,
    request: &FictionalEnergyVfxAnimatedSocketParticlesSequencePrepareRequest,
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
    validate_projection_parent(request, &projection)?;
    let projection_input_sha256 = projection
        .get("input_sha256")
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .ok_or_else(|| invalid("projection input hash is unavailable"))?
        .to_owned();
    let vfx = runtime.fictional_energy_vfx_get(&json!({
        "schema_version":"FictionalEnergyVfxGetRequest@1",
        "project_id":request.project_id,
        "delivery_manifest_object_sha256":request.delivery_manifest_object_sha256
    }))?;
    let _vfx_profile = validate_vfx_profile(request, &vfx)?;
    let source_glb = runtime.cas_read_bounded(&request.source_artifact_sha256, MAX_GLB_BYTES)?;
    let mut frames = Vec::with_capacity(request.frames.len());
    let mut cohort = None::<String>;
    for (ordinal, input) in request.frames.iter().enumerate() {
        let projection_frame = frame_projection(&projection, ordinal)?.clone();
        validate_projection_frame(input, &projection_frame)?;
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
                .ok_or_else(|| invalid("Bloom frame link is unavailable"))?,
        )
        .map_err(|error| invalid(format!("Bloom frame link is malformed: {error}")))?;
        validate_base_bloom(request, input, &base, &bloom)?;
        let camera = camera_for_base(runtime, request, &base)?;
        let bindings = build_emitter_bindings(&projection_frame)?;
        let binding_hash = canonical_json_hash(&bindings);
        let local_seed = frame_local_seed(
            &request.projection_key_sha256,
            &projection_frame,
            &binding_hash,
        );
        let particles = build_particles(&bindings, &local_seed)?;
        let world = world_values(&bindings, &particles, &camera, &local_seed)?;
        let seed = worker_seed(
            &request.projection_key_sha256,
            input.frame_index,
            input.sample_time_ticks,
            &projection_input_sha256,
            &input.projection_socket_transform_inventory_sha256,
            &input.projection_socket_transform_readback_sha256,
            &binding_hash,
            &world,
        );
        let (expected_inventory, expected_inventory_sha256) =
            expected_world_inventory(request, input, &seed, world);
        let first = render_worker::render_typed_animated_socket_particles_with_worker_identity(
            &source_glb,
            &camera,
            &request.projection_key_sha256,
            input.frame_index,
            input.sample_time_ticks,
            &projection_input_sha256,
            &input.projection_socket_transform_inventory_sha256,
            &input.projection_socket_transform_readback_sha256,
            &bindings,
            &particles,
            &seed,
        )
        .map_err(|error| invalid(format!("animated socket particle render failed: {error}")))?;
        let second = render_worker::render_typed_animated_socket_particles_with_worker_identity(
            &source_glb,
            &camera,
            &request.projection_key_sha256,
            input.frame_index,
            input.sample_time_ticks,
            &projection_input_sha256,
            &input.projection_socket_transform_inventory_sha256,
            &input.projection_socket_transform_readback_sha256,
            &bindings,
            &particles,
            &seed,
        )
        .map_err(|error| invalid(format!("animated socket particle replay failed: {error}")))?;
        compare_worker_replay(
            &first,
            &second,
            request,
            input,
            &projection_input_sha256,
            &binding_hash,
            &seed,
            &expected_inventory,
            &expected_inventory_sha256,
        )?;
        let this_cohort = first
            .build_cohort_sha256
            .clone()
            .ok_or_else(|| invalid("animated socket Worker cohort is unavailable"))?;
        if this_cohort != request.render_worker_build_cohort_sha256
            || first
                .render_profile
                .get("canonical_sha256")
                .and_then(Value::as_str)
                != Some(request.render_profile_sha256.as_str())
        {
            return Err(invalid("animated socket Worker cohort or profile differs"));
        }
        if cohort
            .as_deref()
            .is_some_and(|value| value != this_cohort.as_str())
        {
            return Err(invalid(
                "animated socket Worker cohort changes across frames",
            ));
        }
        cohort = Some(this_cohort);
        frames.push(FrameContext {
            input: input.clone(),
            projection_frame,
            emitter_bindings: bindings,
            particles,
            seed_sha256: seed,
            worker: first,
        });
    }
    Ok(DependencyContext {
        frames,
        worker_cohort: cohort.ok_or_else(|| invalid("animated socket Worker cohort is missing"))?,
    })
}

fn make_frame_record(
    request: &FictionalEnergyVfxAnimatedSocketParticlesSequencePrepareRequest,
    context: &FrameContext,
    pass_hashes: (&str, &str, &str),
    render_set_hash: &str,
    receipt_hash: &str,
    created_at: &str,
) -> FictionalEnergyVfxAnimatedSocketParticlesSequenceFrame {
    let input_sha256 = canonical_json_hash(
        &serde_json::to_value(&context.input).expect("frame input serialization is infallible"),
    );
    let particle_key_sha256 = canonical_json_hash(&json!({
        "schema_version":"FictionalEnergyVfxAnimatedSocketParticlesKey@1",
        "sequence_key_sha256":request.sequence_key_sha256,
        "frame_index":context.input.frame_index,
        "input_sha256":input_sha256,
        "emitter_socket_bindings_sha256":context.worker.emitter_binding_sha256,
        "particle_seed_sha256":context.seed_sha256,
        "world_particle_inventory_sha256":context.worker.world_particle_inventory_sha256
    }));
    let mut frame = FictionalEnergyVfxAnimatedSocketParticlesSequenceFrame {
        schema_version: FRAME_SCHEMA.to_owned(),
        frame_index: context.input.frame_index,
        sample_time_ticks: context.input.sample_time_ticks,
        projection_frame_canonical_sha256: context.input.projection_frame_canonical_sha256.clone(),
        projection_socket_transform_inventory_sha256: context
            .input
            .projection_socket_transform_inventory_sha256
            .clone(),
        projection_socket_transform_readback_sha256: context
            .input
            .projection_socket_transform_readback_sha256
            .clone(),
        base_frame_key_sha256: context.input.base_frame_key_sha256.clone(),
        bloom_key_sha256: context.input.bloom_key_sha256.clone(),
        emitter_socket_bindings_sha256: context.worker.emitter_binding_sha256.clone(),
        input_sha256,
        particle_key_sha256,
        particle_seed_sha256: context.seed_sha256.clone(),
        render_set_object_sha256: render_set_hash.to_owned(),
        receipt_object_sha256: receipt_hash.to_owned(),
        particle_color_object_sha256: pass_hashes.0.to_owned(),
        particle_id_object_sha256: pass_hashes.1.to_owned(),
        particle_depth_object_sha256: pass_hashes.2.to_owned(),
        canonical_sha256: String::new(),
        created_at: created_at.to_owned(),
    };
    frame.canonical_sha256 = canonical_json_hash(
        &serde_json::to_value(&frame).expect("frame serialization is infallible"),
    );
    frame
}

fn make_frame_record_v2(
    request: &FictionalEnergyVfxAnimatedSocketParticlesSequenceV2PrepareRequest,
    context: &V2FrameContext,
    pass_hashes: (&str, &str, &str),
    render_set_hash: &str,
    receipt_hash: &str,
    created_at: &str,
) -> FictionalEnergyVfxAnimatedSocketParticlesSequenceV2Frame {
    let input_sha256 = canonical_json_hash(
        &serde_json::to_value(&context.input).expect("V2 frame input serialization is infallible"),
    );
    let particle_key_sha256 = canonical_json_hash(&json!({
        "schema_version":"FictionalEnergyVfxAnimatedSocketParticlesKey@2",
        "sequence_key_sha256":request.sequence_key_sha256,
        "frame_index":context.input.frame_index,
        "input_sha256":input_sha256,
        "emitter_socket_bindings_sha256":context.worker.emitter_binding_sha256,
        "particle_seed_sha256":context.seed_sha256,
        "world_particle_inventory_sha256":context.worker.world_particle_inventory_sha256
    }));
    let mut frame = FictionalEnergyVfxAnimatedSocketParticlesSequenceV2Frame {
        schema_version: V2_FRAME_SCHEMA.to_owned(),
        frame_index: context.input.frame_index,
        sample_time_ticks: context.input.sample_time_ticks,
        projection_frame_canonical_sha256: context.input.projection_frame_canonical_sha256.clone(),
        projection_socket_transform_inventory_sha256: context
            .input
            .projection_socket_transform_inventory_sha256
            .clone(),
        projection_socket_transform_readback_sha256: context
            .input
            .projection_socket_transform_readback_sha256
            .clone(),
        base_frame_key_sha256: context.input.base_frame_key_sha256.clone(),
        bloom_key_sha256: context.input.bloom_key_sha256.clone(),
        emitter_socket_bindings_sha256: context.worker.emitter_binding_sha256.clone(),
        input_sha256,
        particle_key_sha256,
        particle_seed_sha256: context.seed_sha256.clone(),
        render_set_object_sha256: render_set_hash.to_owned(),
        receipt_object_sha256: receipt_hash.to_owned(),
        particle_color_object_sha256: pass_hashes.0.to_owned(),
        particle_id_object_sha256: pass_hashes.1.to_owned(),
        particle_depth_object_sha256: pass_hashes.2.to_owned(),
        canonical_sha256: String::new(),
        created_at: created_at.to_owned(),
    };
    frame.canonical_sha256 = canonical_json_hash(
        &serde_json::to_value(&frame).expect("V2 frame serialization is infallible"),
    );
    frame
}

fn canonical_object(mut value: Value) -> Result<(Value, Vec<u8>), RuntimeError> {
    value
        .as_object_mut()
        .ok_or_else(|| invalid("CAS sidecar must be an object"))?
        .insert("canonical_sha256".to_owned(), Value::String(String::new()));
    let transport_bytes = serde_json::to_vec(&value)
        .map_err(|error| invalid(format!("CAS sidecar serialization failed: {error}")))?;
    value = serde_json::from_slice(&transport_bytes)
        .map_err(|error| invalid(format!("CAS sidecar normalization failed: {error}")))?;
    let canonical = canonical_json_hash(&value);
    value["canonical_sha256"] = Value::String(canonical);
    let bytes = canonical_json_bytes(&value).map_err(|error| invalid(error.to_string()))?;
    Ok((value, bytes))
}

fn frame_receipt_projection(
    frame: &FictionalEnergyVfxAnimatedSocketParticlesSequenceFrame,
) -> FictionalEnergyVfxAnimatedSocketParticlesSequenceFrame {
    let mut projected = frame.clone();
    projected.receipt_object_sha256.clear();
    projected.canonical_sha256.clear();
    projected.canonical_sha256 = canonical_json_hash(
        &serde_json::to_value(&projected).expect("frame serialization is infallible"),
    );
    projected
}

fn frame_receipt_projection_v2(
    frame: &FictionalEnergyVfxAnimatedSocketParticlesSequenceV2Frame,
) -> FictionalEnergyVfxAnimatedSocketParticlesSequenceV2Frame {
    let mut projected = frame.clone();
    projected.receipt_object_sha256 = "0".repeat(64);
    projected.canonical_sha256.clear();
    projected.canonical_sha256 = canonical_json_hash(
        &serde_json::to_value(&projected).expect("V2 frame serialization is infallible"),
    );
    projected
}

fn pass_metadata(sha256: &str, size_bytes: u64, pass: &str) -> Value {
    json!({
        "pass":pass,
        "sha256":sha256,
        "mime":"image/png",
        "size_bytes":size_bytes,
        "width":512,
        "height":512,
        "channels":"rgba8",
        "color_space":"data"
    })
}

fn result_value(
    sequence: &FictionalEnergyVfxAnimatedSocketParticlesSequence,
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

fn build_sequence(
    request: &FictionalEnergyVfxAnimatedSocketParticlesSequencePrepareRequest,
    context: &DependencyContext,
    frames: Vec<FictionalEnergyVfxAnimatedSocketParticlesSequenceFrame>,
) -> FictionalEnergyVfxAnimatedSocketParticlesSequence {
    let mut sequence = FictionalEnergyVfxAnimatedSocketParticlesSequence {
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
        particles_sequence_policy: PARTICLE_POLICY.to_owned(),
        emitter_binding_policy: EMITTER_POLICY.to_owned(),
        transform_projection_policy: TRANSFORM_POLICY.to_owned(),
        frames,
        sequence_status: SEQUENCE_STATUS.to_owned(),
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

fn build_sequence_v2(
    request: &FictionalEnergyVfxAnimatedSocketParticlesSequenceV2PrepareRequest,
    context: &V2DependencyContext,
    frames: Vec<FictionalEnergyVfxAnimatedSocketParticlesSequenceV2Frame>,
) -> FictionalEnergyVfxAnimatedSocketParticlesSequenceV2 {
    let mut sequence = FictionalEnergyVfxAnimatedSocketParticlesSequenceV2 {
        schema_version: V2_SEQUENCE_SCHEMA.to_owned(),
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
        geometry_preservation_projection_sha256: context
            .geometry_preservation_projection_sha256
            .clone(),
        geometry_preservation_status: context.geometry_preservation_status.clone(),
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
        appearance_anchor_set_object_sha256: request.appearance_anchor_set_object_sha256.clone(),
        appearance_anchor_set_canonical_sha256: request
            .appearance_anchor_set_canonical_sha256
            .clone(),
        anchor_binding_policy: request.anchor_binding_policy.clone(),
        anchor_binding_sha256: context.anchor_binding_sha256.clone(),
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
        frame_scope: V2_FRAME_SCOPE.to_owned(),
        particles_sequence_policy: V2_PARTICLE_POLICY.to_owned(),
        emitter_binding_policy: EMITTER_POLICY.to_owned(),
        transform_projection_policy: V2_PROJECTION_TRANSFORM_POLICY.to_owned(),
        frames,
        sequence_status: V2_SEQUENCE_STATUS.to_owned(),
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
    let mut canonical_preimage =
        serde_json::to_value(&sequence).expect("V2 sequence serialization is infallible");
    if let Some(frames) = canonical_preimage
        .get_mut("frames")
        .and_then(Value::as_array_mut)
    {
        for frame in frames {
            if let Some(frame) = frame.as_object_mut() {
                frame.insert("canonical_sha256".to_owned(), Value::String(String::new()));
            }
        }
    }
    sequence.canonical_sha256 = canonical_json_hash(&canonical_preimage);
    sequence
}

fn request_matches_sequence(
    request: &FictionalEnergyVfxAnimatedSocketParticlesSequencePrepareRequest,
    sequence: &FictionalEnergyVfxAnimatedSocketParticlesSequence,
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
        && request.particles_sequence_policy == sequence.particles_sequence_policy
        && request.emitter_binding_policy == sequence.emitter_binding_policy
        && request.transform_projection_policy == sequence.transform_projection_policy
        && request.input_sha256 == sequence.input_sha256
        && request.frames.len() == sequence.frames.len()
        && request
            .frames
            .iter()
            .zip(&sequence.frames)
            .all(|(input, frame)| {
                input.frame_index == frame.frame_index
                    && input.sample_time_ticks == frame.sample_time_ticks
                    && input.projection_frame_canonical_sha256
                        == frame.projection_frame_canonical_sha256
                    && input.projection_socket_transform_inventory_sha256
                        == frame.projection_socket_transform_inventory_sha256
                    && input.projection_socket_transform_readback_sha256
                        == frame.projection_socket_transform_readback_sha256
                    && input.base_frame_key_sha256 == frame.base_frame_key_sha256
                    && input.bloom_key_sha256 == frame.bloom_key_sha256
            })
}

fn request_matches_sequence_v2(
    request: &FictionalEnergyVfxAnimatedSocketParticlesSequenceV2PrepareRequest,
    sequence: &FictionalEnergyVfxAnimatedSocketParticlesSequenceV2,
) -> bool {
    request.sequence_key_sha256 == sequence.sequence_key_sha256
        && request.project_id == sequence.project_id
        && request.geometry_candidate_id == sequence.geometry_candidate_id
        && request.geometry_candidate_state_sha256 == sequence.geometry_candidate_state_sha256
        && request.geometry_delivery_manifest_object_sha256
            == sequence.geometry_delivery_manifest_object_sha256
        && request.geometry_artifact_sha256 == sequence.geometry_artifact_sha256
        && request.appearance_candidate_id == sequence.appearance_candidate_id
        && request.appearance_candidate_state_sha256 == sequence.appearance_candidate_state_sha256
        && request.appearance_delivery_manifest_object_sha256
            == sequence.appearance_delivery_manifest_object_sha256
        && request.appearance_artifact_sha256 == sequence.appearance_artifact_sha256
        && request.material_surface_quality_id == sequence.material_surface_quality_id
        && request.material_surface_quality_report_object_sha256
            == sequence.material_surface_quality_report_object_sha256
        && request.material_surface_quality_canonical_sha256
            == sequence.material_surface_quality_canonical_sha256
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
        && request.appearance_anchor_set_object_sha256
            == sequence.appearance_anchor_set_object_sha256
        && request.appearance_anchor_set_canonical_sha256
            == sequence.appearance_anchor_set_canonical_sha256
        && request.anchor_binding_policy == sequence.anchor_binding_policy
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
        && request.particles_sequence_policy == sequence.particles_sequence_policy
        && request.emitter_binding_policy == sequence.emitter_binding_policy
        && request.transform_projection_policy == sequence.transform_projection_policy
        && request.input_sha256 == sequence.input_sha256
        && request.frames.len() == sequence.frames.len()
        && request
            .frames
            .iter()
            .zip(&sequence.frames)
            .all(|(input, frame)| {
                input.frame_index == frame.frame_index
                    && input.sample_time_ticks == frame.sample_time_ticks
                    && input.projection_frame_canonical_sha256
                        == frame.projection_frame_canonical_sha256
                    && input.projection_socket_transform_inventory_sha256
                        == frame.projection_socket_transform_inventory_sha256
                    && input.projection_socket_transform_readback_sha256
                        == frame.projection_socket_transform_readback_sha256
                    && input.base_frame_key_sha256 == frame.base_frame_key_sha256
                    && input.bloom_key_sha256 == frame.bloom_key_sha256
            })
}

fn result_value_v2(
    sequence: &FictionalEnergyVfxAnimatedSocketParticlesSequenceV2,
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

pub(super) fn prepare_v2(runtime: &Runtime, value: &Value) -> Result<Value, RuntimeError> {
    let (request, _input_sha256) = parse_v2_prepare(value)?;
    let context = build_context_v2(runtime, &request)?;
    if let Some(existing) = runtime
        .store
        .get_fictional_energy_vfx_animated_socket_particles_sequence_v2(
            &request.sequence_key_sha256,
        )?
    {
        if !request_matches_sequence_v2(&request, &existing)
            || existing.anchor_binding_sha256 != context.anchor_binding_sha256
            || existing.geometry_preservation_projection_sha256
                != context.geometry_preservation_projection_sha256
            || existing.geometry_preservation_status != context.geometry_preservation_status
        {
            return Err(invalid("existing V2 sequence binding differs"));
        }
        return result_value_v2(&existing, true, V2_PREPARE_RESULT_SCHEMA, true);
    }

    let reservation = runtime.store.begin_cas_reservation();
    let mut reserved_objects: Vec<CasObject> = Vec::new();
    let operation = (|| -> Result<Value, RuntimeError> {
        let mut frame_records = Vec::with_capacity(context.frames.len());
        for frame in &context.frames {
            let frame_created_at = now_string();
            let mut pass_hashes = [String::new(), String::new(), String::new()];
            for (index, pass) in frame.worker.particle_passes.iter().enumerate() {
                let kind = match pass.pass.as_str() {
                    "particle-color" => COLOR_KIND,
                    "particle-id" => ID_KIND,
                    "particle-depth" => DEPTH_KIND,
                    _ => return Err(invalid("V2 Worker particle pass inventory differs")),
                };
                let object = runtime.store.put_object_reserved(
                    &reservation,
                    &pass.png,
                    None,
                    "image/png",
                    kind,
                    &now_string(),
                )?;
                pass_hashes[index] = object.record.sha256.clone();
                reserved_objects.push(object);
            }
            let temporary_frame = make_frame_record_v2(
                &request,
                frame,
                (&pass_hashes[0], &pass_hashes[1], &pass_hashes[2]),
                &"0".repeat(64),
                &"0".repeat(64),
                &frame_created_at,
            );
            let render_set_value = canonical_object(json!({
                "schema_version":V2_RENDER_SET_SCHEMA,
                "sequence_key_sha256":request.sequence_key_sha256,
                "frame_index":frame.input.frame_index,
                "sample_time_ticks":frame.input.sample_time_ticks,
                "projection_key_sha256":request.projection_key_sha256,
                "projection_frame_canonical_sha256":frame.input.projection_frame_canonical_sha256,
                "base_frame_key_sha256":frame.input.base_frame_key_sha256,
                "bloom_key_sha256":frame.input.bloom_key_sha256,
                "emitter_socket_bindings_sha256":frame.worker.emitter_binding_sha256,
                "particle_key_sha256":temporary_frame.particle_key_sha256,
                "particle_seed_sha256":frame.seed_sha256,
                "world_particle_inventory_sha256":frame.worker.world_particle_inventory_sha256,
                "camera_object_sha256":request.camera_object_sha256,
                "camera_identity_sha256":request.camera_identity_sha256,
                "render_profile_sha256":request.render_profile_sha256,
                "render_worker_build_cohort_sha256":request.render_worker_build_cohort_sha256,
                "passes":["particle-color","particle-id","particle-depth"],
                "pass_artifacts":[
                    pass_metadata(&pass_hashes[0], frame.worker.particle_passes[0].png.len() as u64, "particle-color"),
                    pass_metadata(&pass_hashes[1], frame.worker.particle_passes[1].png.len() as u64, "particle-id"),
                    pass_metadata(&pass_hashes[2], frame.worker.particle_passes[2].png.len() as u64, "particle-depth")
                ],
                "canonical_sha256":""
            }))?;
            let render_set_object = runtime.store.put_object_reserved(
                &reservation,
                &render_set_value.1,
                None,
                "application/json",
                RENDER_SET_KIND,
                &now_string(),
            )?;
            let render_set_hash = render_set_object.record.sha256.clone();
            reserved_objects.push(render_set_object);
            let frame_record_without_receipt = make_frame_record_v2(
                &request,
                frame,
                (&pass_hashes[0], &pass_hashes[1], &pass_hashes[2]),
                &render_set_hash,
                &"0".repeat(64),
                &frame_created_at,
            );
            let receipt_value = canonical_object(json!({
                "schema_version":V2_FRAME_RECEIPT_SCHEMA,
                "sequence_key_sha256":request.sequence_key_sha256,
                "frame":frame_record_without_receipt,
                "projection_frame":frame.projection_frame,
                "emitter_bindings":frame.emitter_bindings,
                "particles":frame.particles,
                "world_particle_inventory":frame.worker.world_particle_inventory,
                "base_frame_key_sha256":frame.input.base_frame_key_sha256,
                "bloom_key_sha256":frame.input.bloom_key_sha256,
                "camera_object_sha256":request.camera_object_sha256,
                "camera_identity_sha256":request.camera_identity_sha256,
                "render_profile_sha256":request.render_profile_sha256,
                "render_worker_build_cohort_sha256":request.render_worker_build_cohort_sha256,
                "worker_replay_byte_exact":true,
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
                &now_string(),
            )?;
            let receipt_hash = receipt_object.record.sha256.clone();
            reserved_objects.push(receipt_object);
            frame_records.push(make_frame_record_v2(
                &request,
                frame,
                (&pass_hashes[0], &pass_hashes[1], &pass_hashes[2]),
                &render_set_hash,
                &receipt_hash,
                &frame_created_at,
            ));
        }
        let sequence = build_sequence_v2(&request, &context, frame_records);
        let sequence_value = serde_json::to_value(&sequence)
            .map_err(|error| invalid(format!("V2 sequence serialization failed: {error}")))?;
        let sequence_bytes = canonical_json_bytes(&sequence_value).map_err(|error| {
            invalid(format!("V2 sequence receipt serialization failed: {error}"))
        })?;
        if sequence_bytes.len() > MAX_JSON_BYTES as usize {
            return Err(invalid("V2 sequence receipt exceeds one MiB"));
        }
        let sequence_receipt_object = runtime.store.put_object_reserved(
            &reservation,
            &sequence_bytes,
            None,
            "application/json",
            SEQUENCE_RECEIPT_KIND,
            &sequence.created_at,
        )?;
        reserved_objects.push(sequence_receipt_object.clone());
        let stored = runtime
            .store
            .record_fictional_energy_vfx_animated_socket_particles_sequence_v2(
                &sequence,
                &sequence_receipt_object.record,
            )?;
        for object in &reserved_objects {
            runtime
                .store
                .release_cas_reservation_object(&reservation, object, false)?;
        }
        result_value_v2(&stored, false, V2_PREPARE_RESULT_SCHEMA, true)
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
                "{error}; V2 CAS reservation rollback failed: {}",
                rollback_errors.join(" | ")
            )));
        }
        return Err(error);
    }
    operation
}

pub(super) fn get_v2(runtime: &Runtime, value: &Value) -> Result<Value, RuntimeError> {
    let request = parse_v2_get(value)?;
    let stored = runtime
        .store
        .get_fictional_energy_vfx_animated_socket_particles_sequence_v2(
            &request.sequence_key_sha256,
        )?
        .ok_or_else(|| invalid("V2 animated socket particle sequence is unavailable"))?;
    if stored.project_id != request.project_id
        || stored.geometry_candidate_id != request.geometry_candidate_id
        || stored.appearance_candidate_id != request.appearance_candidate_id
        || stored.geometry_delivery_manifest_object_sha256
            != request.geometry_delivery_manifest_object_sha256
        || stored.appearance_delivery_manifest_object_sha256
            != request.appearance_delivery_manifest_object_sha256
    {
        return Err(invalid(
            "V2 animated socket particle sequence scope differs",
        ));
    }
    let replay_request = FictionalEnergyVfxAnimatedSocketParticlesSequenceV2PrepareRequest {
        schema_version: V2_PREPARE_SCHEMA.to_owned(),
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
        particles_sequence_policy: stored.particles_sequence_policy.clone(),
        emitter_binding_policy: stored.emitter_binding_policy.clone(),
        transform_projection_policy: stored.transform_projection_policy.clone(),
        frames: stored
            .frames
            .iter()
            .map(
                |frame| FictionalEnergyVfxAnimatedSocketParticlesSequenceV2FrameInput {
                    frame_index: frame.frame_index,
                    sample_time_ticks: frame.sample_time_ticks,
                    projection_frame_canonical_sha256: frame
                        .projection_frame_canonical_sha256
                        .clone(),
                    projection_socket_transform_inventory_sha256: frame
                        .projection_socket_transform_inventory_sha256
                        .clone(),
                    projection_socket_transform_readback_sha256: frame
                        .projection_socket_transform_readback_sha256
                        .clone(),
                    base_frame_key_sha256: frame.base_frame_key_sha256.clone(),
                    bloom_key_sha256: frame.bloom_key_sha256.clone(),
                },
            )
            .collect(),
        input_sha256: stored.input_sha256.clone(),
        idempotency_key: stored.sequence_key_sha256.clone(),
    };
    let context = build_context_v2(runtime, &replay_request)?;
    if context.anchor_binding_sha256 != stored.anchor_binding_sha256
        || context.geometry_preservation_projection_sha256
            != stored.geometry_preservation_projection_sha256
        || context.geometry_preservation_status != stored.geometry_preservation_status
    {
        return Err(invalid(
            "V2 geometry/appearance derived binding differs after restart",
        ));
    }
    for (frame, expected) in context.frames.iter().zip(&stored.frames) {
        if frame.worker.emitter_binding_sha256 != expected.emitter_socket_bindings_sha256
            || frame.worker.seed_sha256 != expected.particle_seed_sha256
        {
            return Err(invalid("V2 particle replay binding differs after restart"));
        }
        for (pass, hash) in frame.worker.particle_passes.iter().zip([
            &expected.particle_color_object_sha256,
            &expected.particle_id_object_sha256,
            &expected.particle_depth_object_sha256,
        ]) {
            let bytes = runtime.cas_read_bounded(hash, 4 * 1024 * 1024)?;
            if sha256_hex(&bytes) != *hash || bytes != pass.png {
                return Err(invalid("V2 particle pass bytes differ after restart"));
            }
        }
        let render_set = read_canonical_json(
            runtime,
            &expected.render_set_object_sha256,
            V2_RENDER_SET_SCHEMA,
        )?;
        let render_set_pass_hashes = render_set
            .get("pass_artifacts")
            .and_then(Value::as_array)
            .filter(|values| values.len() == 3)
            .map(|values| {
                values
                    .iter()
                    .filter_map(|value| value.get("sha256").and_then(Value::as_str))
                    .collect::<Vec<_>>()
            })
            .ok_or_else(|| invalid("V2 particle render set is malformed"))?;
        if render_set
            .get("sequence_key_sha256")
            .and_then(Value::as_str)
            != Some(stored.sequence_key_sha256.as_str())
            || render_set.get("frame_index").and_then(Value::as_u64) != Some(expected.frame_index)
            || render_set
                .get("particle_key_sha256")
                .and_then(Value::as_str)
                != Some(expected.particle_key_sha256.as_str())
            || render_set_pass_hashes
                != vec![
                    expected.particle_color_object_sha256.as_str(),
                    expected.particle_id_object_sha256.as_str(),
                    expected.particle_depth_object_sha256.as_str(),
                ]
        {
            return Err(invalid("V2 particle render set differs after restart"));
        }
        let receipt = read_canonical_json(
            runtime,
            &expected.receipt_object_sha256,
            V2_FRAME_RECEIPT_SCHEMA,
        )?;
        let expected_receipt_frame = serde_json::to_value(frame_receipt_projection_v2(expected))
            .map_err(|error| invalid(format!("V2 frame receipt projection failed: {error}")))?;
        let receipt_frame = receipt
            .get("frame")
            .ok_or_else(|| invalid("V2 particle frame receipt projection is unavailable"))?;
        let receipt_world = receipt
            .get("world_particle_inventory")
            .ok_or_else(|| invalid("V2 particle world inventory is unavailable"))?;
        let receipt_emitters = receipt
            .get("emitter_bindings")
            .ok_or_else(|| invalid("V2 particle emitter bindings are unavailable"))?;
        let receipt_particles = receipt
            .get("particles")
            .ok_or_else(|| invalid("V2 local particle inventory is unavailable"))?;
        if !canonical_values_equal(receipt_frame, &expected_receipt_frame)? {
            return Err(invalid(
                "V2 particle frame receipt record projection differs after restart",
            ));
        }
        if !canonical_values_equal(receipt_world, &frame.worker.world_particle_inventory)? {
            return Err(invalid(
                "V2 particle frame receipt world inventory differs after restart",
            ));
        }
        if !canonical_values_equal(receipt_emitters, &frame.emitter_bindings)? {
            return Err(invalid(
                "V2 particle frame receipt emitter bindings differ after restart",
            ));
        }
        if !canonical_values_equal(receipt_particles, &frame.particles)? {
            return Err(invalid(
                "V2 particle frame receipt local inventory differs after restart",
            ));
        }
    }
    result_value_v2(&stored, true, V2_GET_RESULT_SCHEMA, false)
}

pub(super) fn prepare(runtime: &Runtime, value: &Value) -> Result<Value, RuntimeError> {
    let (request, _input_sha256) = parse_prepare(value)?;
    let context = build_context(runtime, &request)?;
    if let Some(existing) = runtime
        .store
        .get_fictional_energy_vfx_animated_socket_particles_sequence(&request.sequence_key_sha256)?
    {
        if !request_matches_sequence(&request, &existing) {
            return Err(invalid("existing sequence binding differs"));
        }
        return result_value(&existing, true, PREPARE_RESULT_SCHEMA, true);
    }

    let reservation = runtime.store.begin_cas_reservation();
    let mut reserved_objects: Vec<CasObject> = Vec::new();
    let operation = (|| -> Result<Value, RuntimeError> {
        let mut frame_records = Vec::with_capacity(context.frames.len());
        for frame in &context.frames {
            let frame_created_at = now_string();
            let mut pass_hashes = [String::new(), String::new(), String::new()];
            for (index, pass) in frame.worker.particle_passes.iter().enumerate() {
                let kind = match pass.pass.as_str() {
                    "particle-color" => COLOR_KIND,
                    "particle-id" => ID_KIND,
                    "particle-depth" => DEPTH_KIND,
                    _ => return Err(invalid("Worker particle pass inventory differs")),
                };
                let object = runtime.store.put_object_reserved(
                    &reservation,
                    &pass.png,
                    None,
                    "image/png",
                    kind,
                    &now_string(),
                )?;
                pass_hashes[index] = object.record.sha256.clone();
                reserved_objects.push(object);
            }
            let temporary_frame = make_frame_record(
                &request,
                frame,
                (&pass_hashes[0], &pass_hashes[1], &pass_hashes[2]),
                "",
                "",
                &frame_created_at,
            );
            let render_set_value = canonical_object(json!({
                "schema_version":RENDER_SET_SCHEMA,
                "sequence_key_sha256":request.sequence_key_sha256,
                "frame_index":frame.input.frame_index,
                "sample_time_ticks":frame.input.sample_time_ticks,
                "projection_key_sha256":request.projection_key_sha256,
                "projection_frame_canonical_sha256":frame.input.projection_frame_canonical_sha256,
                "base_frame_key_sha256":frame.input.base_frame_key_sha256,
                "bloom_key_sha256":frame.input.bloom_key_sha256,
                "emitter_socket_bindings_sha256":frame.worker.emitter_binding_sha256,
                "particle_key_sha256":temporary_frame.particle_key_sha256,
                "particle_seed_sha256":frame.seed_sha256,
                "world_particle_inventory_sha256":frame.worker.world_particle_inventory_sha256,
                "camera_object_sha256":request.camera_object_sha256,
                "camera_identity_sha256":request.camera_identity_sha256,
                "render_profile_sha256":request.render_profile_sha256,
                "render_worker_build_cohort_sha256":request.render_worker_build_cohort_sha256,
                "passes":["particle-color","particle-id","particle-depth"],
                "pass_artifacts":[
                    pass_metadata(&pass_hashes[0], frame.worker.particle_passes[0].png.len() as u64, "particle-color"),
                    pass_metadata(&pass_hashes[1], frame.worker.particle_passes[1].png.len() as u64, "particle-id"),
                    pass_metadata(&pass_hashes[2], frame.worker.particle_passes[2].png.len() as u64, "particle-depth")
                ],
                "canonical_sha256":""
            }))?;
            let render_set_object = runtime.store.put_object_reserved(
                &reservation,
                &render_set_value.1,
                None,
                "application/json",
                RENDER_SET_KIND,
                &now_string(),
            )?;
            let render_set_hash = render_set_object.record.sha256.clone();
            reserved_objects.push(render_set_object);
            let frame_record_without_receipt = make_frame_record(
                &request,
                frame,
                (&pass_hashes[0], &pass_hashes[1], &pass_hashes[2]),
                &render_set_hash,
                "",
                &frame_created_at,
            );
            let receipt_value = canonical_object(json!({
                "schema_version":FRAME_RECEIPT_SCHEMA,
                "sequence_key_sha256":request.sequence_key_sha256,
                "frame":frame_record_without_receipt,
                "projection_frame":frame.projection_frame,
                "emitter_bindings":frame.emitter_bindings,
                "particles":frame.particles,
                "world_particle_inventory":frame.worker.world_particle_inventory,
                "base_frame_key_sha256":frame.input.base_frame_key_sha256,
                "bloom_key_sha256":frame.input.bloom_key_sha256,
                "camera_object_sha256":request.camera_object_sha256,
                "camera_identity_sha256":request.camera_identity_sha256,
                "render_profile_sha256":request.render_profile_sha256,
                "render_worker_build_cohort_sha256":request.render_worker_build_cohort_sha256,
                "worker_replay_byte_exact":true,
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
                &now_string(),
            )?;
            let receipt_hash = receipt_object.record.sha256.clone();
            reserved_objects.push(receipt_object);
            frame_records.push(make_frame_record(
                &request,
                frame,
                (&pass_hashes[0], &pass_hashes[1], &pass_hashes[2]),
                &render_set_hash,
                &receipt_hash,
                &frame_created_at,
            ));
        }
        let sequence = build_sequence(&request, &context, frame_records);
        let sequence_value = serde_json::to_value(&sequence)
            .map_err(|error| invalid(format!("sequence serialization failed: {error}")))?;
        let sequence_bytes = canonical_json_bytes(&sequence_value)
            .map_err(|error| invalid(format!("sequence receipt serialization failed: {error}")))?;
        if sequence_bytes.len() > MAX_JSON_BYTES as usize {
            return Err(invalid("sequence receipt exceeds one MiB"));
        }
        let sequence_receipt_object = runtime.store.put_object_reserved(
            &reservation,
            &sequence_bytes,
            None,
            "application/json",
            SEQUENCE_RECEIPT_KIND,
            &sequence.created_at,
        )?;
        reserved_objects.push(sequence_receipt_object.clone());
        let stored = runtime
            .store
            .record_fictional_energy_vfx_animated_socket_particles_sequence(
                &sequence,
                &sequence_receipt_object.record,
            )?;
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

pub(super) fn get(runtime: &Runtime, value: &Value) -> Result<Value, RuntimeError> {
    let request = parse_get(value)?;
    let stored = runtime
        .store
        .get_fictional_energy_vfx_animated_socket_particles_sequence(&request.sequence_key_sha256)?
        .ok_or_else(|| invalid("animated socket particle sequence is unavailable"))?;
    if stored.project_id != request.project_id || stored.candidate_id != request.candidate_id {
        return Err(invalid("animated socket particle sequence scope differs"));
    }
    let frame_inputs = stored
        .frames
        .iter()
        .map(
            |frame| FictionalEnergyVfxAnimatedSocketParticlesSequenceFrameInput {
                frame_index: frame.frame_index,
                sample_time_ticks: frame.sample_time_ticks,
                projection_frame_canonical_sha256: frame.projection_frame_canonical_sha256.clone(),
                projection_socket_transform_inventory_sha256: frame
                    .projection_socket_transform_inventory_sha256
                    .clone(),
                projection_socket_transform_readback_sha256: frame
                    .projection_socket_transform_readback_sha256
                    .clone(),
                base_frame_key_sha256: frame.base_frame_key_sha256.clone(),
                bloom_key_sha256: frame.bloom_key_sha256.clone(),
            },
        )
        .collect::<Vec<_>>();
    let replay_request = FictionalEnergyVfxAnimatedSocketParticlesSequencePrepareRequest {
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
        particles_sequence_policy: stored.particles_sequence_policy.clone(),
        emitter_binding_policy: stored.emitter_binding_policy.clone(),
        transform_projection_policy: stored.transform_projection_policy.clone(),
        frames: frame_inputs,
        input_sha256: stored.input_sha256.clone(),
        idempotency_key: stored.sequence_key_sha256.clone(),
    };
    let context = build_context(runtime, &replay_request)?;
    for (frame, expected) in context.frames.iter().zip(&stored.frames) {
        if frame.worker.emitter_binding_sha256 != expected.emitter_socket_bindings_sha256
            || frame.worker.seed_sha256 != expected.particle_seed_sha256
        {
            return Err(invalid("animated socket particle replay binding differs"));
        }
        for (pass, hash) in frame.worker.particle_passes.iter().zip([
            &expected.particle_color_object_sha256,
            &expected.particle_id_object_sha256,
            &expected.particle_depth_object_sha256,
        ]) {
            let bytes = runtime.cas_read_bounded(hash, 4 * 1024 * 1024)?;
            if sha256_hex(&bytes) != *hash || bytes != pass.png {
                return Err(invalid(
                    "animated socket particle pass bytes differ after restart",
                ));
            }
        }
        let render_set = read_canonical_json(
            runtime,
            &expected.render_set_object_sha256,
            RENDER_SET_SCHEMA,
        )?;
        let render_set_pass_hashes = render_set
            .get("pass_artifacts")
            .and_then(Value::as_array)
            .filter(|values| values.len() == 3)
            .map(|values| {
                values
                    .iter()
                    .filter_map(|value| value.get("sha256").and_then(Value::as_str))
                    .collect::<Vec<_>>()
            })
            .ok_or_else(|| invalid("animated socket particle render set is malformed"))?;
        if render_set
            .get("sequence_key_sha256")
            .and_then(Value::as_str)
            != Some(stored.sequence_key_sha256.as_str())
            || render_set.get("frame_index").and_then(Value::as_u64) != Some(expected.frame_index)
            || render_set
                .get("particle_key_sha256")
                .and_then(Value::as_str)
                != Some(expected.particle_key_sha256.as_str())
            || render_set_pass_hashes
                != vec![
                    expected.particle_color_object_sha256.as_str(),
                    expected.particle_id_object_sha256.as_str(),
                    expected.particle_depth_object_sha256.as_str(),
                ]
        {
            return Err(invalid("animated socket particle render set differs"));
        }
        let receipt = read_canonical_json(
            runtime,
            &expected.receipt_object_sha256,
            FRAME_RECEIPT_SCHEMA,
        )?;
        let expected_receipt_frame = serde_json::to_value(frame_receipt_projection(expected))
            .map_err(|error| invalid(format!("frame receipt projection failed: {error}")))?;
        let receipt_frame = receipt
            .get("frame")
            .ok_or_else(|| invalid("particle frame receipt projection is unavailable"))?;
        let receipt_world = receipt
            .get("world_particle_inventory")
            .ok_or_else(|| invalid("particle world inventory is unavailable"))?;
        let receipt_emitters = receipt
            .get("emitter_bindings")
            .ok_or_else(|| invalid("particle emitter bindings are unavailable"))?;
        let receipt_particles = receipt
            .get("particles")
            .ok_or_else(|| invalid("local particle inventory is unavailable"))?;
        if !canonical_values_equal(receipt_frame, &expected_receipt_frame)?
            || !canonical_values_equal(receipt_world, &frame.worker.world_particle_inventory)?
            || !canonical_values_equal(receipt_emitters, &frame.emitter_bindings)?
            || !canonical_values_equal(receipt_particles, &frame.particles)?
            || receipt
                .get("render_worker_build_cohort_sha256")
                .and_then(Value::as_str)
                != Some(stored.render_worker_build_cohort_sha256.as_str())
        {
            return Err(invalid(
                "animated socket particle frame receipt canonical projection differs",
            ));
        }
    }
    result_value(&stored, true, GET_RESULT_SCHEMA, false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn transform_frame() -> Value {
        json!({
            "frame_index":0,
            "sample_time_ticks":10,
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
    fn derive_particles_is_bounded_and_trs_sensitive() {
        let bindings = build_emitter_bindings(&transform_frame()).unwrap();
        let first = build_particles(&bindings, "a".repeat(64).as_str()).unwrap();
        let mut changed = transform_frame();
        changed["socket_transforms"][0]["composed_world_transform"]["translation_m"] =
            json!([0.2, 0.0, 2.0]);
        let changed_bindings = build_emitter_bindings(&changed).unwrap();
        assert_ne!(bindings, changed_bindings);
        assert_eq!(first.as_array().unwrap().len(), 56);
    }

    #[test]
    fn transform_point_preserves_local_offset_and_applies_quaternion_rotation() {
        assert_eq!(
            transform_point([1.0, 2.0, 3.0], [0.0, 0.0, 0.0, 1.0], [0.5, -0.25, 1.0]),
            [1.5, 1.75, 4.0]
        );
        let rotated = transform_point(
            [0.0, 0.0, 0.0],
            [
                0.0,
                0.0,
                std::f32::consts::FRAC_1_SQRT_2,
                std::f32::consts::FRAC_1_SQRT_2,
            ],
            [1.0, 0.0, 0.0],
        );
        assert!(rotated[0].abs() <= 1.0e-6);
        assert!((rotated[1] - 1.0).abs() <= 1.0e-6);
        assert!(rotated[2].abs() <= 1.0e-6);
    }

    #[test]
    fn transform_point_matches_worker_f32_order_for_nontrivial_world_seed_input() {
        let translation = [0.12345679_f32, -0.9876543, 0.456789];
        let rotation = [0.1234567_f32, -0.2345678, 0.3456789, 0.9001347];
        let scale = [1.0000001_f32, 0.99999994, 1.0];
        let local = [0.03765432_f32, -0.08123457, 0.12987654];
        let expected = [
            f32::from_bits(0x3e234b6a),
            f32::from_bits(0xbf89afb3),
            f32::from_bits(0x3f15286d),
        ];
        assert_eq!(
            transform_point_with_scale(translation, rotation, scale, local),
            expected
        );

        let bindings = json!({
            "schema_version":EMITTER_SCHEMA,
            "emitters":[
                {
                    "emitter_id":"muzzle-burst",
                    "socket_node_id":"socket-muzzle-vfx",
                    "anchor_id":"socket-muzzle-vfx",
                    "role":"muzzle-vfx",
                    "owner_part_id":"barrel-assembly",
                    "composed_world_transform":{
                        "translation_m":f32_value(&translation),
                        "rotation_quat_xyzw":f32_value(&rotation),
                        "scale_xyz":f32_value(&scale)
                    }
                },
                {
                    "emitter_id":"energy-core-sparks",
                    "socket_node_id":"socket-energy-core-vfx",
                    "anchor_id":"socket-energy-core-vfx",
                    "role":"energy-core-vfx",
                    "owner_part_id":"energy-core",
                    "composed_world_transform":{
                        "translation_m":[0.0,0.0,2.0],
                        "rotation_quat_xyzw":[0.0,0.0,0.0,1.0],
                        "scale_xyz":[1.0,1.0,1.0]
                    }
                }
            ]
        });
        let mut particles = build_particles(&bindings, "worker-f32-order").unwrap();
        particles[0]["local_offset_m"] = f32_value(&local);
        let camera = json!({
            "transform":{"position_m":[0.0,0.0,0.0],"target_m":[0.0,0.0,1.0],"up":[0.0,1.0,0.0]},
            "near_m":0.1,
            "far_m":100.0
        });
        let world = world_values(&bindings, &particles, &camera, "worker-f32-order").unwrap();
        let world_position = world[0]["position"]
            .as_array()
            .expect("world position array")
            .iter()
            .map(|value| value.as_f64().expect("world position number") as f32)
            .collect::<Vec<_>>();
        assert_eq!(
            world_position
                .iter()
                .map(|value| value.to_bits())
                .collect::<Vec<_>>(),
            expected
                .iter()
                .map(|value| value.to_bits())
                .collect::<Vec<_>>()
        );

        let seed = worker_seed(
            &"a".repeat(64),
            3,
            240,
            &"b".repeat(64),
            &"c".repeat(64),
            &"d".repeat(64),
            &canonical_json_hash(&bindings),
            &world,
        );
        assert_eq!(
            seed,
            "a210755135197ef232ee063648bccc6ce91dccb10db6a866ebb509d6215721c9"
        );
    }

    #[test]
    fn canonical_sidecar_is_stable_after_json_transport_roundtrip() {
        let (_, bytes) = canonical_object(json!({
            "schema_version":"AnimatedParticleSidecarTest@1",
            "particle":{"position":f32_value(&[0.1_f32, -0.2_f32, 0.3_f32])},
            "canonical_sha256":""
        }))
        .unwrap();
        let readback: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(canonical_json_bytes(&readback).unwrap(), bytes);
        let local_f32 = json!({"position":f32_value(&[0.1_f32, -0.2_f32, 0.3_f32])});
        let transported: Value =
            serde_json::from_slice(&serde_json::to_vec(&local_f32).unwrap()).unwrap();
        assert!(canonical_values_equal(&local_f32, &transported).unwrap());
    }

    #[test]
    fn emitter_role_retarget_is_rejected() {
        let mut frame = transform_frame();
        frame["socket_transforms"][1]["role"] = json!("muzzle-vfx");
        assert!(build_emitter_bindings(&frame).is_err());
    }

    #[test]
    fn worker_seed_changes_when_local_inventory_changes() {
        let bindings = build_emitter_bindings(&transform_frame()).unwrap();
        let particles = build_particles(&bindings, "b".repeat(64).as_str()).unwrap();
        let camera = json!({
            "transform":{"position_m":[0.0,0.0,0.0],"target_m":[0.0,0.0,1.0],"up":[0.0,1.0,0.0]},
            "near_m":0.1,"far_m":100.0
        });
        let world = world_values(&bindings, &particles, &camera, "seed").unwrap();
        let first = worker_seed(
            "a".repeat(64).as_str(),
            0,
            10,
            "c".repeat(64).as_str(),
            "d".repeat(64).as_str(),
            "e".repeat(64).as_str(),
            &canonical_json_hash(&bindings),
            &world,
        );
        let mut changed = world.clone();
        changed[0]["id"] = json!(10001);
        let second = worker_seed(
            "a".repeat(64).as_str(),
            0,
            10,
            "c".repeat(64).as_str(),
            "d".repeat(64).as_str(),
            "e".repeat(64).as_str(),
            &canonical_json_hash(&bindings),
            &changed,
        );
        assert_ne!(first, second);
        let retargeted_projection_input = worker_seed(
            "a".repeat(64).as_str(),
            0,
            10,
            "f".repeat(64).as_str(),
            "d".repeat(64).as_str(),
            "e".repeat(64).as_str(),
            &canonical_json_hash(&bindings),
            &world,
        );
        assert_ne!(first, retargeted_projection_input);
    }

    #[test]
    fn frame_receipt_projection_breaks_the_receipt_hash_cycle_deterministically() {
        let frame = FictionalEnergyVfxAnimatedSocketParticlesSequenceFrame {
            schema_version: FRAME_SCHEMA.to_owned(),
            frame_index: 0,
            sample_time_ticks: 10,
            projection_frame_canonical_sha256: "a".repeat(64),
            projection_socket_transform_inventory_sha256: "b".repeat(64),
            projection_socket_transform_readback_sha256: "c".repeat(64),
            base_frame_key_sha256: "d".repeat(64),
            bloom_key_sha256: "e".repeat(64),
            emitter_socket_bindings_sha256: "f".repeat(64),
            input_sha256: "1".repeat(64),
            particle_key_sha256: "2".repeat(64),
            particle_seed_sha256: "3".repeat(64),
            render_set_object_sha256: "4".repeat(64),
            receipt_object_sha256: "5".repeat(64),
            particle_color_object_sha256: "6".repeat(64),
            particle_id_object_sha256: "7".repeat(64),
            particle_depth_object_sha256: "8".repeat(64),
            canonical_sha256: "9".repeat(64),
            created_at: "2026-08-21T00:00:00Z".to_owned(),
        };
        let first = frame_receipt_projection(&frame);
        let second = frame_receipt_projection(&frame);
        assert!(first.receipt_object_sha256.is_empty());
        assert!(is_sha256(&first.canonical_sha256));
        assert_eq!(first, second);
        assert_ne!(first.canonical_sha256, frame.canonical_sha256);
    }

    fn v2_prepare_fixture() -> Value {
        let hash = |byte: char| byte.to_string().repeat(64);
        let frame = json!({
            "frame_index":0,
            "sample_time_ticks":10,
            "projection_frame_canonical_sha256":hash('a'),
            "projection_socket_transform_inventory_sha256":hash('b'),
            "projection_socket_transform_readback_sha256":hash('c'),
            "base_frame_key_sha256":hash('d'),
            "bloom_key_sha256":hash('e')
        });
        let mut object = Map::new();
        for (field, value) in [
            (
                "schema_version",
                Value::String(V2_PREPARE_SCHEMA.to_owned()),
            ),
            ("sequence_key_sha256", Value::String(hash('f'))),
            ("project_id", json!("project-1")),
            ("geometry_candidate_id", json!("candidate-geometry")),
            ("geometry_candidate_state_sha256", Value::String(hash('1'))),
            (
                "geometry_delivery_manifest_object_sha256",
                Value::String(hash('2')),
            ),
            ("geometry_artifact_sha256", Value::String(hash('3'))),
            ("appearance_candidate_id", json!("candidate-appearance")),
            (
                "appearance_candidate_state_sha256",
                Value::String(hash('4')),
            ),
            (
                "appearance_delivery_manifest_object_sha256",
                Value::String(hash('5')),
            ),
            ("appearance_artifact_sha256", Value::String(hash('6'))),
            ("material_surface_quality_id", json!("quality-1")),
            (
                "material_surface_quality_report_object_sha256",
                Value::String(hash('7')),
            ),
            (
                "material_surface_quality_canonical_sha256",
                Value::String(hash('8')),
            ),
            ("projection_key_sha256", Value::String(hash('9'))),
            ("projection_object_sha256", Value::String(hash('a'))),
            ("projection_canonical_sha256", Value::String(hash('b'))),
            (
                "animated_socket_materialization_key_sha256",
                Value::String(hash('c')),
            ),
            ("animated_artifact_sha256", Value::String(hash('d'))),
            (
                "animated_socket_anchor_set_object_sha256",
                Value::String(hash('e')),
            ),
            (
                "animated_socket_anchor_set_canonical_sha256",
                Value::String(hash('f')),
            ),
            (
                "appearance_anchor_set_object_sha256",
                Value::String(hash('0')),
            ),
            (
                "appearance_anchor_set_canonical_sha256",
                Value::String(hash('1')),
            ),
            (
                "anchor_binding_policy",
                Value::String(V2_ANCHOR_BINDING_POLICY.to_owned()),
            ),
            ("animation_clip_id", json!("clip-1")),
            ("animation_clip_object_sha256", Value::String(hash('2'))),
            ("animation_clip_canonical_sha256", Value::String(hash('3'))),
            ("animation_receipt_object_sha256", Value::String(hash('4'))),
            (
                "animation_receipt_canonical_sha256",
                Value::String(hash('5')),
            ),
            ("vfx_profile_object_sha256", Value::String(hash('6'))),
            ("vfx_profile_canonical_sha256", Value::String(hash('7'))),
            ("socket_node_id_encoding_sha256", Value::String(hash('8'))),
            ("socket_roles_sha256", Value::String(hash('9'))),
            ("camera_object_sha256", Value::String(hash('a'))),
            ("camera_identity_sha256", Value::String(hash('b'))),
            ("render_profile_sha256", Value::String(hash('c'))),
            (
                "render_worker_build_cohort_sha256",
                Value::String(hash('d')),
            ),
            ("sample_schedule_sha256", Value::String(hash('e'))),
            ("sample_count", json!(1)),
            ("sample_time_ticks", json!([10])),
            ("frame_scope", Value::String(V2_FRAME_SCOPE.to_owned())),
            (
                "particles_sequence_policy",
                Value::String(V2_PARTICLE_POLICY.to_owned()),
            ),
            (
                "emitter_binding_policy",
                Value::String(EMITTER_POLICY.to_owned()),
            ),
            (
                "transform_projection_policy",
                Value::String(V2_PROJECTION_TRANSFORM_POLICY.to_owned()),
            ),
            ("frames", json!([frame])),
            ("input_sha256", Value::String(hash('f'))),
            ("idempotency_key", json!("idempotency-1")),
        ] {
            object.insert(field.to_owned(), value);
        }
        let mut value = Value::Object(object);
        let object = value.as_object_mut().expect("fixture object");
        object.remove("sequence_key_sha256");
        object.remove("input_sha256");
        object.remove("idempotency_key");
        let input_sha256 = canonical_json_hash(&Value::Object(object.clone()));
        value["sequence_key_sha256"] = Value::String(input_sha256.clone());
        value["input_sha256"] = Value::String(input_sha256);
        value["idempotency_key"] = Value::String("idempotency-1".to_owned());
        value
    }

    fn anchor_fixture() -> Value {
        let definitions = [
            (
                "weapon-root",
                "weapon-root",
                "synthetic-scene-root",
                Value::Null,
            ),
            ("grip-primary", "grip-primary", "part-node", json!("part-1")),
            (
                "socket-muzzle-vfx",
                "muzzle-vfx",
                "part-node",
                json!("part-1"),
            ),
            (
                "socket-magazine-well",
                "magazine-well",
                "part-node",
                json!("part-1"),
            ),
            (
                "socket-sight-primary",
                "sight-primary",
                "part-node",
                json!("part-1"),
            ),
            (
                "socket-energy-core-vfx",
                "energy-core-vfx",
                "part-node",
                json!("part-1"),
            ),
        ];
        json!({
            "schema_version":"GameWeaponAnchorSet@1",
            "node_materialization":"sidecar-only-not-glb-nodes",
            "anchors":definitions.into_iter().map(|(anchor_id,role,parent_kind,owner_part_id)| json!({
                "anchor_id":anchor_id,
                "role":role,
                "parent_kind":parent_kind,
                "owner_part_id":owner_part_id,
                "local_translation_m":[0.0,0.0,0.0],
                "local_rotation_quat_xyzw":[0.0,0.0,0.0,1.0],
                "local_scale_xyz":[1.0,1.0,1.0]
            })).collect::<Vec<_>>()
        })
    }

    #[test]
    fn v2_anchor_projection_requires_exact_equivalent_six_role_trs() {
        let first = anchor_fixture();
        let mut second = anchor_fixture();
        assert_eq!(
            v2_anchor_projection(&first).unwrap(),
            v2_anchor_projection(&second).unwrap()
        );
        second["anchors"][2]["local_translation_m"] = json!([0.01, 0.0, 0.0]);
        assert_ne!(
            v2_anchor_projection(&first).unwrap(),
            v2_anchor_projection(&second).unwrap()
        );
        second["anchors"][2]["local_rotation_quat_xyzw"] = json!([0.0, 0.0, 0.0, 2.0]);
        assert!(v2_anchor_projection(&second).is_err());
    }

    #[test]
    fn v2_prepare_rejects_same_candidate_and_unknown_fields() {
        let mut same_candidate = v2_prepare_fixture();
        same_candidate["appearance_candidate_id"] = same_candidate["geometry_candidate_id"].clone();
        assert!(parse_v2_prepare(&same_candidate).is_err());
        let mut unknown = v2_prepare_fixture();
        unknown["unexpected"] = json!(true);
        assert!(parse_v2_prepare(&unknown).is_err());
    }

    #[test]
    fn v2_prepare_accepts_projection2_transform_policy_and_rejects_v1_policy() {
        let valid = v2_prepare_fixture();
        parse_v2_prepare(&valid).expect("Projection@2 transform policy should parse");

        let mut legacy = valid.clone();
        legacy["transform_projection_policy"] = Value::String(TRANSFORM_POLICY.to_owned());
        let mut preimage = legacy.clone();
        preimage
            .as_object_mut()
            .expect("V2 parser policy fixture object")
            .remove("sequence_key_sha256");
        preimage
            .as_object_mut()
            .expect("V2 parser policy fixture object")
            .remove("input_sha256");
        preimage
            .as_object_mut()
            .expect("V2 parser policy fixture object")
            .remove("idempotency_key");
        let hash = canonical_json_hash(&preimage);
        legacy["sequence_key_sha256"] = Value::String(hash.clone());
        legacy["input_sha256"] = Value::String(hash);
        assert!(parse_v2_prepare(&legacy).is_err());
    }

    #[test]
    fn v2_projection_frame_requires_exact_frame_inventory_and_readbacks() {
        let roles = [
            ("weapon-root", "weapon-root"),
            ("grip-primary", "grip-primary"),
            ("muzzle-vfx", "socket-muzzle-vfx"),
            ("magazine-well", "socket-magazine-well"),
            ("sight-primary", "socket-sight-primary"),
            ("energy-core-vfx", "socket-energy-core-vfx"),
        ];
        let sockets = roles
            .iter()
            .enumerate()
            .map(|(index, (role, node_id))| {
                json!({
                    "socket_node_id":node_id,
                    "anchor_id":node_id,
                    "role":role,
                    "node_index":index,
                    "parent_node_index":-1,
                    "node_name":node_id,
                    "parent_node_name":Value::Null,
                    "node_kind":"empty",
                    "parent_kind":"synthetic-scene-root",
                    "owner_part_id":Value::Null,
                    "local_transform":{"translation_m":[0.0,0.0,1.0],"rotation_quat_xyzw":[0.0,0.0,0.0,1.0],"scale_xyz":[1.0,1.0,1.0]},
                    "parent_world_transform":{"translation_m":[0.0,0.0,0.0],"rotation_quat_xyzw":[0.0,0.0,0.0,1.0],"scale_xyz":[1.0,1.0,1.0]},
                    "composed_world_transform":{"translation_m":[0.0,0.0,1.0],"rotation_quat_xyzw":[0.0,0.0,0.0,1.0],"scale_xyz":[1.0,1.0,1.0]},
                    "local_matrix_4x4":[1.0,0.0,0.0,0.0,0.0,1.0,0.0,0.0,0.0,0.0,1.0,0.0,0.0,0.0,1.0,1.0],
                    "parent_world_matrix_4x4":[1.0,0.0,0.0,0.0,0.0,1.0,0.0,0.0,0.0,0.0,1.0,0.0,0.0,0.0,0.0,1.0],
                    "composed_world_matrix_4x4":[1.0,0.0,0.0,0.0,0.0,1.0,0.0,0.0,0.0,0.0,1.0,0.0,0.0,0.0,1.0,1.0]
                })
            })
            .collect::<Vec<_>>();
        let inventory = sockets
            .iter()
            .map(|socket| {
                json!({
                    "socket_node_id":socket["socket_node_id"],
                    "anchor_id":socket["anchor_id"],
                    "role":socket["role"],
                    "node_index":socket["node_index"],
                    "parent_node_index":socket["parent_node_index"],
                    "node_name":socket["node_name"],
                    "parent_node_name":socket["parent_node_name"],
                    "node_kind":socket["node_kind"],
                    "parent_kind":socket["parent_kind"],
                    "owner_part_id":socket["owner_part_id"]
                })
            })
            .collect::<Vec<_>>();
        let inventory_hash = canonical_json_hash(&Value::Array(inventory));
        let mut frame = json!({
            "schema_version":V2_PROJECTION_FRAME_SCHEMA,
            "projection_key_sha256":"a".repeat(64),
            "frame_index":0,
            "sample_time_ticks":10,
            "source_animation_sample_sha256":"b".repeat(64),
            "derived_socket_sample_sha256":"c".repeat(64),
            "socket_transform_inventory_sha256":inventory_hash,
            "socket_transform_readback_sha256":"",
            "projection_frame_canonical_sha256":"",
            "socket_transforms":sockets,
            "canonical_sha256":"",
            "created_at":"2026-08-22T00:00:00Z"
        });
        let mut readback_preimage = frame.clone();
        readback_preimage["created_at"] = Value::String(String::new());
        readback_preimage["socket_transform_readback_sha256"] = Value::String(String::new());
        readback_preimage["projection_frame_canonical_sha256"] = Value::String(String::new());
        readback_preimage["canonical_sha256"] = Value::String(String::new());
        let readback = canonical_json_hash(&readback_preimage);
        frame["socket_transform_readback_sha256"] = Value::String(readback);
        frame["projection_frame_canonical_sha256"] =
            Value::String(readback_sha256_from_projection_frame(&frame).unwrap());
        let mut canonical = frame.clone();
        canonical["canonical_sha256"] = Value::String(String::new());
        frame["canonical_sha256"] = Value::String(canonical_json_hash(&canonical));
        let input = FictionalEnergyVfxAnimatedSocketParticlesSequenceV2FrameInput {
            frame_index: 0,
            sample_time_ticks: 10,
            projection_frame_canonical_sha256: frame["projection_frame_canonical_sha256"]
                .as_str()
                .unwrap()
                .to_owned(),
            projection_socket_transform_inventory_sha256: frame
                ["socket_transform_inventory_sha256"]
                .as_str()
                .unwrap()
                .to_owned(),
            projection_socket_transform_readback_sha256: frame["socket_transform_readback_sha256"]
                .as_str()
                .unwrap()
                .to_owned(),
            base_frame_key_sha256: "d".repeat(64),
            bloom_key_sha256: "e".repeat(64),
        };
        validate_v2_projection_frame(&input, &frame).unwrap();
        let mut tampered = frame.clone();
        tampered["socket_transform_inventory_sha256"] = Value::String("f".repeat(64));
        assert!(validate_v2_projection_frame(&input, &tampered).is_err());
        let mut tampered = frame.clone();
        tampered["projection_frame_canonical_sha256"] = Value::String("f".repeat(64));
        assert!(validate_v2_projection_frame(&input, &tampered).is_err());
        let mut tampered = frame;
        tampered["socket_transform_readback_sha256"] = Value::String("f".repeat(64));
        assert!(validate_v2_projection_frame(&input, &tampered).is_err());
    }
}
