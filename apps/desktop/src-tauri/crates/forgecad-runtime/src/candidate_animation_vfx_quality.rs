//! Runtime-owned structural animation/VFX quality composition.
//!
//! This module is deliberately a composition gate.  It does not create a
//! candidate, call the mechanical-animation GLB producer, mutate a render or
//! attach a commercial-engine socket.  Every dependency is re-read through
//! its existing Runtime getter before the one report object is reserved.

use super::{
    canonical_json_bytes, canonical_json_hash, exact_object, is_opaque_id, is_sha256, sha256_hex,
    CasObject, Runtime, RuntimeError,
};
use forgecad_contracts::{
    CandidateAnimationVfxQualityGetRequest, CandidateAnimationVfxQualityHardGate,
    CandidateAnimationVfxQualityPrepareRequest, CandidateAnimationVfxQualityRecord,
    CandidateMaterialSurfaceQualityRecord,
};
use serde_json::{json, Map, Value};
use std::collections::BTreeSet;

const PREPARE_SCHEMA: &str = "CandidateAnimationVfxQualityPrepareRequest@1";
const GET_SCHEMA: &str = "CandidateAnimationVfxQualityGetRequest@1";
const PREPARE_RESULT_SCHEMA: &str = "CandidateAnimationVfxQualityPrepareResult@1";
const GET_RESULT_SCHEMA: &str = "CandidateAnimationVfxQualityGetResult@1";
const REPORT_KIND: &str = "candidate-animation-vfx-quality-report";
const REPORT_MIME: &str = "application/json";
const MAX_REPORT_BYTES: usize = 1024 * 1024;
const SCOPE: &str = "lod0-rigid-animation-full-vfx-stack-single-frame@1";
const POLICY: &str = "candidate-animation-vfx-structural-hard-gate@1";
const STATUS: &str = "runtime-owned-durable-candidate-animation-vfx-quality";
const SAME_CANDIDATE: &str = "same-material-surface-head-candidate-no-geometry-mutation";

const PREPARE_FIELDS: &[&str] = &[
    "schema_version",
    "animation_vfx_quality_id",
    "project_id",
    "source_material_surface_transition_id",
    "source_material_surface_transition_sha256",
    "source_material_surface_head_canonical_sha256",
    "source_material_surface_quality_id",
    "source_material_surface_quality_report_object_sha256",
    "source_material_surface_quality_canonical_sha256",
    "candidate_id",
    "candidate_state_sha256",
    "artifact_id",
    "artifact_sha256",
    "delivery_manifest_object_sha256",
    "anchor_set_object_sha256",
    "anchor_set_canonical_sha256",
    "animation_clip_id",
    "animation_clip_object_sha256",
    "animation_clip_sha256",
    "animated_socket_materialization_key_sha256",
    "animated_artifact_sha256",
    "animation_receipt_object_sha256",
    "animation_receipt_canonical_sha256",
    "derived_animated_socket_artifact_sha256",
    "animated_socket_receipt_object_sha256",
    "vfx_profile_object_sha256",
    "vfx_profile_canonical_sha256",
    "vfx_sequence_key_sha256",
    "vfx_sequence_canonical_sha256",
    "vfx_frame_key_sha256",
    "vfx_frame_canonical_sha256",
    "vfx_bloom_key_sha256",
    "vfx_bloom_canonical_sha256",
    "vfx_particle_key_sha256",
    "vfx_particle_canonical_sha256",
    "vfx_trail_key_sha256",
    "vfx_trail_canonical_sha256",
    "vfx_trail_bloom_key_sha256",
    "vfx_trail_bloom_canonical_sha256",
    "particle_history_key_sha256s",
    "sample_request_sha256",
    "camera_object_sha256",
    "camera_identity_sha256",
    "render_profile_sha256",
    "render_worker_build_cohort_sha256",
    "animation_vfx_scope",
    "animation_vfx_policy",
    "animation_vfx_policy_sha256",
    "from_stage",
    "to_stage",
    "input_sha256",
    "idempotency_key",
];

const GET_FIELDS: &[&str] = &[
    "schema_version",
    "animation_vfx_quality_id",
    "project_id",
    "candidate_id",
];

fn invalid(detail: impl Into<String>) -> RuntimeError {
    RuntimeError::InvalidInput(format!(
        "CANDIDATE_ANIMATION_VFX_QUALITY_INVALID: {}",
        detail.into()
    ))
}

fn required_text<'a>(object: &'a Map<String, Value>, field: &str) -> Result<&'a str, RuntimeError> {
    object
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| invalid(format!("{field} is invalid")))
}

fn parse_prepare(
    value: &Value,
) -> Result<(CandidateAnimationVfxQualityPrepareRequest, String), RuntimeError> {
    let object = exact_object(value, PREPARE_FIELDS, PREPARE_SCHEMA)?;
    if required_text(object, "schema_version")? != PREPARE_SCHEMA {
        return Err(invalid("prepare schema version differs"));
    }
    let request: CandidateAnimationVfxQualityPrepareRequest = serde_json::from_value(value.clone())
        .map_err(|error| invalid(format!("prepare request is malformed: {error}")))?;
    let ids = [
        &request.animation_vfx_quality_id,
        &request.project_id,
        &request.source_material_surface_transition_id,
        &request.source_material_surface_quality_id,
        &request.candidate_id,
        &request.artifact_id,
        &request.animation_clip_id,
        &request.idempotency_key,
    ];
    if ids.iter().any(|value| !is_opaque_id(value)) {
        return Err(invalid("one or more request identifiers are malformed"));
    }
    let hashes = [
        &request.source_material_surface_transition_sha256,
        &request.source_material_surface_head_canonical_sha256,
        &request.source_material_surface_quality_report_object_sha256,
        &request.source_material_surface_quality_canonical_sha256,
        &request.candidate_state_sha256,
        &request.artifact_sha256,
        &request.delivery_manifest_object_sha256,
        &request.anchor_set_object_sha256,
        &request.anchor_set_canonical_sha256,
        &request.animation_clip_object_sha256,
        &request.animation_clip_sha256,
        &request.animated_socket_materialization_key_sha256,
        &request.animated_artifact_sha256,
        &request.animation_receipt_object_sha256,
        &request.animation_receipt_canonical_sha256,
        &request.derived_animated_socket_artifact_sha256,
        &request.animated_socket_receipt_object_sha256,
        &request.vfx_profile_object_sha256,
        &request.vfx_profile_canonical_sha256,
        &request.vfx_sequence_key_sha256,
        &request.vfx_sequence_canonical_sha256,
        &request.vfx_frame_key_sha256,
        &request.vfx_frame_canonical_sha256,
        &request.vfx_bloom_key_sha256,
        &request.vfx_bloom_canonical_sha256,
        &request.vfx_particle_key_sha256,
        &request.vfx_particle_canonical_sha256,
        &request.vfx_trail_key_sha256,
        &request.vfx_trail_canonical_sha256,
        &request.vfx_trail_bloom_key_sha256,
        &request.vfx_trail_bloom_canonical_sha256,
        &request.sample_request_sha256,
        &request.camera_object_sha256,
        &request.camera_identity_sha256,
        &request.render_profile_sha256,
        &request.render_worker_build_cohort_sha256,
        &request.animation_vfx_policy_sha256,
        &request.input_sha256,
    ];
    if hashes.iter().any(|value| !is_sha256(value)) {
        return Err(invalid("one or more request hashes are malformed"));
    }
    if request.particle_history_key_sha256s.is_empty()
        || request.particle_history_key_sha256s.len() > 4
        || request
            .particle_history_key_sha256s
            .iter()
            .any(|value| !is_sha256(value))
        || request
            .particle_history_key_sha256s
            .iter()
            .collect::<BTreeSet<_>>()
            .len()
            != request.particle_history_key_sha256s.len()
    {
        return Err(invalid(
            "particle history must contain one to four unique SHA-256 keys",
        ));
    }
    if request.animation_vfx_scope != SCOPE
        || request.animation_vfx_policy != POLICY
        || request.animation_vfx_policy_sha256 != sha256_hex(POLICY.as_bytes())
        || request.from_stage != "material-surface"
        || request.to_stage != "animation-vfx"
    {
        return Err(invalid("scope, policy or stage binding differs"));
    }
    let mut preimage = object.clone();
    preimage.remove("input_sha256");
    preimage.remove("idempotency_key");
    let request_sha256 = canonical_json_hash(&Value::Object(preimage));
    if request.input_sha256 != request_sha256 {
        return Err(invalid(format!(
            "input_sha256 differs; expected {request_sha256}"
        )));
    }
    Ok((request, request_sha256))
}

fn parse_get(value: &Value) -> Result<CandidateAnimationVfxQualityGetRequest, RuntimeError> {
    let object = exact_object(value, GET_FIELDS, GET_SCHEMA)?;
    if required_text(object, "schema_version")? != GET_SCHEMA {
        return Err(invalid("get schema version differs"));
    }
    let request: CandidateAnimationVfxQualityGetRequest = serde_json::from_value(value.clone())
        .map_err(|error| invalid(format!("get request is malformed: {error}")))?;
    for (name, value) in [
        (
            "animation_vfx_quality_id",
            &request.animation_vfx_quality_id,
        ),
        ("project_id", &request.project_id),
        ("candidate_id", &request.candidate_id),
    ] {
        if !is_opaque_id(value) {
            return Err(invalid(format!("{name} is malformed")));
        }
    }
    Ok(request)
}

fn require_link<'a>(value: &'a Value, label: &str) -> Result<&'a Map<String, Value>, RuntimeError> {
    value
        .get("link")
        .and_then(Value::as_object)
        .ok_or_else(|| invalid(format!("{label} link is unavailable")))
}

fn link_sha(link: &Map<String, Value>, field: &str, label: &str) -> Result<String, RuntimeError> {
    let hash = link
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid(format!("{label}.{field} is unavailable")))?;
    if !is_sha256(hash) {
        return Err(invalid(format!("{label}.{field} is malformed")));
    }
    Ok(hash.to_owned())
}

fn same_link_field(
    left: &Map<String, Value>,
    right: &Map<String, Value>,
    field: &str,
    label: &str,
) -> Result<(), RuntimeError> {
    if left.get(field) != right.get(field) {
        return Err(invalid(format!(
            "{label}.{field} differs across dependency links"
        )));
    }
    Ok(())
}

fn validate_stage_and_material(
    runtime: &Runtime,
    request: &CandidateAnimationVfxQualityPrepareRequest,
) -> Result<(Value, Value, CandidateMaterialSurfaceQualityRecord), RuntimeError> {
    let transition = runtime
        .store
        .get_production_stage_transition_v2(&request.source_material_surface_transition_id)?
        .ok_or_else(|| invalid("material-surface transition is unavailable"))?;
    if transition.canonical_sha256 != request.source_material_surface_transition_sha256
        || transition.project_id != request.project_id
        || transition.to_stage != "material-surface"
        || transition.material_surface_quality_id != request.source_material_surface_quality_id
        || transition.material_surface_quality_report_object_sha256
            != request.source_material_surface_quality_report_object_sha256
        || transition.material_surface_quality_canonical_sha256
            != request.source_material_surface_quality_canonical_sha256
    {
        return Err(invalid("material-surface transition binding differs"));
    }
    let stage = runtime.production_stage_transition_v2_get(json!({
        "schema_version":"ProductionStageTransitionGetRequest@2",
        "transition_id":transition.transition_id,
        "session_id":transition.session_id,
        "project_id":transition.project_id,
        "root_candidate_id":transition.root_candidate_id,
        "head_candidate_id":transition.head_candidate_id
    }))?;
    let head = stage
        .get("production_stage_head")
        .cloned()
        .ok_or_else(|| invalid("material-surface V2 head is unavailable"))?;
    if head.get("head_candidate_id").and_then(Value::as_str) != Some(request.candidate_id.as_str())
        || head.get("head_artifact_sha256").and_then(Value::as_str)
            != Some(request.artifact_sha256.as_str())
        || head
            .get("head_candidate_state_sha256")
            .and_then(Value::as_str)
            != Some(request.candidate_state_sha256.as_str())
        || head
            .get("material_surface_quality_id")
            .and_then(Value::as_str)
            != Some(request.source_material_surface_quality_id.as_str())
        || head
            .get("material_surface_quality_report_object_sha256")
            .and_then(Value::as_str)
            != Some(
                request
                    .source_material_surface_quality_report_object_sha256
                    .as_str(),
            )
        || head
            .get("material_surface_quality_canonical_sha256")
            .and_then(Value::as_str)
            != Some(
                request
                    .source_material_surface_quality_canonical_sha256
                    .as_str(),
            )
        || head.get("canonical_sha256").and_then(Value::as_str)
            != Some(
                request
                    .source_material_surface_head_canonical_sha256
                    .as_str(),
            )
    {
        return Err(invalid("immutable material-surface head binding differs"));
    }
    let material = runtime
        .store
        .get_candidate_material_surface_quality(&request.source_material_surface_quality_id)?
        .ok_or_else(|| invalid("material-surface quality is unavailable"))?;
    if material.project_id != request.project_id
        || material.output_candidate_id != request.candidate_id
        || material.output_candidate_state_sha256 != request.candidate_state_sha256
        || material.output_artifact_id != request.artifact_id
        || material.output_artifact_sha256 != request.artifact_sha256
        || material.canonical_sha256 != request.source_material_surface_quality_canonical_sha256
        || material.source_output_candidate_binding_status != "distinct-candidates-verified"
        || material.validator_status != "passed"
        || !material.hard_gate_passed
    {
        return Err(invalid("material-surface quality binding or gate differs"));
    }
    let _ = runtime.candidate_material_surface_quality_get(json!({
        "schema_version":"CandidateMaterialSurfaceQualityGetRequest@1",
        "material_surface_quality_id":material.material_surface_quality_id,
        "project_id":request.project_id,
        "source_candidate_id":material.source_candidate_id,
        "output_candidate_id":material.output_candidate_id
    }))?;
    Ok((stage, head, material))
}

struct DependencyReadback {
    profile: Value,
    sequence: Value,
    frame: Value,
    bloom: Value,
    particles: Value,
    trails: Value,
    trail_bloom: Value,
    animated_socket: Value,
}

fn validate_dependencies(
    runtime: &Runtime,
    request: &CandidateAnimationVfxQualityPrepareRequest,
) -> Result<DependencyReadback, RuntimeError> {
    let candidate = runtime
        .candidate(&request.candidate_id)?
        .ok_or_else(|| invalid("head candidate is unavailable"))?;
    if candidate.project_id != request.project_id
        || candidate.canonical_sha256 != request.candidate_state_sha256
        || candidate.prepared_object_id.as_deref() != Some(request.artifact_id.as_str())
        || candidate.prepared_object_sha256.as_deref() != Some(request.artifact_sha256.as_str())
    {
        return Err(invalid("head candidate or artifact binding differs"));
    }

    let delivery = runtime.game_asset_delivery_get(&json!({
        "schema_version":"GameAssetDeliveryGetRequest@1",
        "project_id":request.project_id,
        "delivery_manifest_object_sha256":request.delivery_manifest_object_sha256
    }))?;
    let delivery_link = require_link(&delivery, "delivery")?;
    if delivery_link.get("project_id").and_then(Value::as_str) != Some(request.project_id.as_str())
        || delivery_link
            .get("delivery_manifest_object_sha256")
            .and_then(Value::as_str)
            != Some(request.delivery_manifest_object_sha256.as_str())
    {
        return Err(invalid("delivery project or key binding differs"));
    }
    let levels = delivery
        .get("lod_receipt")
        .and_then(|value| value.get("levels"))
        .and_then(Value::as_array)
        .filter(|levels| levels.len() == 3)
        .ok_or_else(|| invalid("delivery does not contain exactly three LODs"))?;
    let lod0 = levels
        .first()
        .ok_or_else(|| invalid("delivery LOD0 unavailable"))?;
    if lod0.get("candidate_id").and_then(Value::as_str) != Some(request.candidate_id.as_str())
        || lod0.get("candidate_state_sha256").and_then(Value::as_str)
            != Some(request.candidate_state_sha256.as_str())
        || lod0.get("artifact_sha256").and_then(Value::as_str)
            != Some(request.artifact_sha256.as_str())
    {
        return Err(invalid("delivery LOD0 binding differs"));
    }

    let anchor = runtime.game_weapon_anchor_get(&json!({
        "schema_version":"GameWeaponAnchorGetRequest@1",
        "project_id":request.project_id,
        "delivery_manifest_object_sha256":request.delivery_manifest_object_sha256
    }))?;
    let anchor_link = require_link(&anchor, "AnchorSet")?;
    let anchor_set = anchor
        .get("anchor_set")
        .ok_or_else(|| invalid("AnchorSet is unavailable"))?;
    if anchor_link
        .get("anchor_set_object_sha256")
        .and_then(Value::as_str)
        != Some(request.anchor_set_object_sha256.as_str())
        || anchor_set.get("canonical_sha256").and_then(Value::as_str)
            != Some(request.anchor_set_canonical_sha256.as_str())
        || anchor_set.get("project_id").and_then(Value::as_str) != Some(request.project_id.as_str())
        || anchor_set
            .get("delivery_manifest_object_sha256")
            .and_then(Value::as_str)
            != Some(request.delivery_manifest_object_sha256.as_str())
    {
        return Err(invalid("AnchorSet binding differs"));
    }

    let animation_clip = runtime.mechanical_animation_clip_get(&json!({
        "schema_version":"MechanicalAnimationClipGetRequest@1",
        "project_id":request.project_id,
        "candidate_id":request.candidate_id,
        "clip_id":request.animation_clip_id
    }))?;
    let clip_link = require_link(&animation_clip, "animation clip")?;
    if clip_link.get("project_id").and_then(Value::as_str) != Some(request.project_id.as_str())
        || clip_link.get("candidate_id").and_then(Value::as_str)
            != Some(request.candidate_id.as_str())
        || clip_link.get("artifact_id").and_then(Value::as_str)
            != Some(request.artifact_id.as_str())
        || clip_link.get("clip_object_sha256").and_then(Value::as_str)
            != Some(request.animation_clip_object_sha256.as_str())
        || clip_link.get("clip_sha256").and_then(Value::as_str)
            != Some(request.animation_clip_sha256.as_str())
    {
        return Err(invalid("animation clip binding differs"));
    }

    let animated_socket = runtime.game_weapon_animated_glb_socket_get(&json!({
        "schema_version":"GameWeaponAnimatedGlbSocketMaterializationGetRequest@1",
        "project_id":request.project_id,
        "animated_socket_materialization_key_sha256":request.animated_socket_materialization_key_sha256
    }))?;
    let socket_link = require_link(&animated_socket, "animated socket")?;
    if socket_link.get("project_id").and_then(Value::as_str) != Some(request.project_id.as_str())
        || socket_link.get("candidate_id").and_then(Value::as_str)
            != Some(request.candidate_id.as_str())
        || socket_link
            .get("candidate_state_sha256")
            .and_then(Value::as_str)
            != Some(request.candidate_state_sha256.as_str())
        || socket_link
            .get("delivery_manifest_object_sha256")
            .and_then(Value::as_str)
            != Some(request.delivery_manifest_object_sha256.as_str())
        || socket_link
            .get("anchor_set_object_sha256")
            .and_then(Value::as_str)
            != Some(request.anchor_set_object_sha256.as_str())
        || socket_link
            .get("animated_artifact_sha256")
            .and_then(Value::as_str)
            != Some(request.animated_artifact_sha256.as_str())
        || socket_link
            .get("animation_receipt_object_sha256")
            .and_then(Value::as_str)
            != Some(request.animation_receipt_object_sha256.as_str())
        || socket_link
            .get("animation_receipt_canonical_sha256")
            .and_then(Value::as_str)
            != Some(request.animation_receipt_canonical_sha256.as_str())
        || socket_link
            .get("derived_animated_socket_artifact_sha256")
            .and_then(Value::as_str)
            != Some(request.derived_animated_socket_artifact_sha256.as_str())
        || socket_link
            .get("receipt_object_sha256")
            .and_then(Value::as_str)
            != Some(request.animated_socket_receipt_object_sha256.as_str())
        || animated_socket
            .get("restart_hash_verified")
            .and_then(Value::as_bool)
            != Some(true)
    {
        return Err(invalid("animated socket readback binding differs"));
    }

    let profile = runtime.fictional_energy_vfx_get(&json!({
        "schema_version":"FictionalEnergyVfxGetRequest@1",
        "project_id":request.project_id,
        "delivery_manifest_object_sha256":request.delivery_manifest_object_sha256
    }))?;
    let profile_link = require_link(&profile, "VFX profile")?;
    let profile_value = profile
        .get("vfx_profile")
        .ok_or_else(|| invalid("VFX profile CAS value is unavailable"))?;
    if profile_link
        .get("vfx_profile_object_sha256")
        .and_then(Value::as_str)
        != Some(request.vfx_profile_object_sha256.as_str())
        || profile_value
            .get("canonical_sha256")
            .and_then(Value::as_str)
            != Some(request.vfx_profile_canonical_sha256.as_str())
    {
        return Err(invalid("VFX profile binding differs"));
    }

    let sequence = runtime.fictional_energy_vfx_rendered_sequence_get(&json!({
        "schema_version":"FictionalEnergyVfxRenderedSequenceGetRequest@1",
        "project_id":request.project_id,
        "sequence_key_sha256":request.vfx_sequence_key_sha256
    }))?;
    let sequence_link = require_link(&sequence, "VFX sequence")?;
    if sequence_link
        .get("sequence_key_sha256")
        .and_then(Value::as_str)
        != Some(request.vfx_sequence_key_sha256.as_str())
        || sequence_link
            .get("canonical_sha256")
            .and_then(Value::as_str)
            != Some(request.vfx_sequence_canonical_sha256.as_str())
    {
        return Err(invalid("VFX sequence binding differs"));
    }
    let sequence_frames = sequence
        .get("frames")
        .and_then(Value::as_array)
        .filter(|frames| !frames.is_empty())
        .ok_or_else(|| invalid("VFX sequence must contain at least one frame"))?;
    if !sequence_frames.iter().any(|frame| {
        frame.get("frame_key_sha256").and_then(Value::as_str)
            == Some(request.vfx_frame_key_sha256.as_str())
    }) {
        return Err(invalid("requested VFX frame is not in the sequence"));
    }

    let frame = runtime.fictional_energy_vfx_rendered_frame_get(&json!({
        "schema_version":"FictionalEnergyVfxRenderedFrameGetRequest@1",
        "project_id":request.project_id,
        "frame_key_sha256":request.vfx_frame_key_sha256
    }))?;
    let frame_link = require_link(&frame, "VFX frame")?;
    if frame_link.get("frame_key_sha256").and_then(Value::as_str)
        != Some(request.vfx_frame_key_sha256.as_str())
        || frame_link.get("canonical_sha256").and_then(Value::as_str)
            != Some(request.vfx_frame_canonical_sha256.as_str())
    {
        return Err(invalid("VFX frame binding differs"));
    }

    let bloom = runtime.fictional_energy_vfx_hdr_bloom_get(&json!({
        "schema_version":"FictionalEnergyVfxHdrBloomFrameGetRequest@1",
        "project_id":request.project_id,
        "bloom_key_sha256":request.vfx_bloom_key_sha256
    }))?;
    let bloom_link = require_link(&bloom, "VFX bloom")?;
    if bloom_link.get("bloom_key_sha256").and_then(Value::as_str)
        != Some(request.vfx_bloom_key_sha256.as_str())
        || bloom_link.get("canonical_sha256").and_then(Value::as_str)
            != Some(request.vfx_bloom_canonical_sha256.as_str())
        || bloom_link
            .get("base_frame_key_sha256")
            .and_then(Value::as_str)
            != Some(request.vfx_frame_key_sha256.as_str())
    {
        return Err(invalid("VFX bloom parent binding differs"));
    }

    let particles = runtime.fictional_energy_vfx_particles_get(&json!({
        "schema_version":"FictionalEnergyVfxParticlesFrameGetRequest@1",
        "project_id":request.project_id,
        "particle_key_sha256":request.vfx_particle_key_sha256
    }))?;
    let particle_link = require_link(&particles, "VFX particles")?;
    if particle_link
        .get("particle_key_sha256")
        .and_then(Value::as_str)
        != Some(request.vfx_particle_key_sha256.as_str())
        || particle_link
            .get("canonical_sha256")
            .and_then(Value::as_str)
            != Some(request.vfx_particle_canonical_sha256.as_str())
        || particle_link
            .get("base_frame_key_sha256")
            .and_then(Value::as_str)
            != Some(request.vfx_frame_key_sha256.as_str())
        || particle_link
            .get("bloom_key_sha256")
            .and_then(Value::as_str)
            != Some(request.vfx_bloom_key_sha256.as_str())
        || particle_link
            .get("anchor_set_object_sha256")
            .and_then(Value::as_str)
            != Some(request.anchor_set_object_sha256.as_str())
    {
        return Err(invalid("VFX particle parent binding differs"));
    }

    let trails = runtime.fictional_energy_vfx_trails_get(&json!({
        "schema_version":"FictionalEnergyVfxTrailsFrameGetRequest@1",
        "project_id":request.project_id,
        "trail_key_sha256":request.vfx_trail_key_sha256
    }))?;
    let trail_link = require_link(&trails, "VFX trails")?;
    if trail_link.get("trail_key_sha256").and_then(Value::as_str)
        != Some(request.vfx_trail_key_sha256.as_str())
        || trail_link.get("canonical_sha256").and_then(Value::as_str)
            != Some(request.vfx_trail_canonical_sha256.as_str())
        || trail_link
            .get("base_frame_key_sha256")
            .and_then(Value::as_str)
            != Some(request.vfx_frame_key_sha256.as_str())
        || trail_link.get("bloom_key_sha256").and_then(Value::as_str)
            != Some(request.vfx_bloom_key_sha256.as_str())
        || trail_link
            .get("current_particle_key_sha256")
            .and_then(Value::as_str)
            != Some(request.vfx_particle_key_sha256.as_str())
        || trail_link.get("particle_history_key_sha256s")
            != Some(&Value::Array(
                request
                    .particle_history_key_sha256s
                    .iter()
                    .cloned()
                    .map(Value::String)
                    .collect(),
            ))
    {
        return Err(invalid("VFX trail parent/history binding differs"));
    }

    let trail_bloom = runtime.fictional_energy_vfx_trails_bloom_get(&json!({
        "schema_version":"FictionalEnergyVfxTrailsBloomFrameGetRequest@1",
        "project_id":request.project_id,
        "trail_bloom_key_sha256":request.vfx_trail_bloom_key_sha256
    }))?;
    let trail_bloom_link = require_link(&trail_bloom, "VFX trail bloom")?;
    if trail_bloom_link
        .get("trail_bloom_key_sha256")
        .and_then(Value::as_str)
        != Some(request.vfx_trail_bloom_key_sha256.as_str())
        || trail_bloom_link
            .get("canonical_sha256")
            .and_then(Value::as_str)
            != Some(request.vfx_trail_bloom_canonical_sha256.as_str())
        || trail_bloom_link
            .get("source_trail_key_sha256")
            .and_then(Value::as_str)
            != Some(request.vfx_trail_key_sha256.as_str())
        || trail_bloom_link
            .get("base_frame_key_sha256")
            .and_then(Value::as_str)
            != Some(request.vfx_frame_key_sha256.as_str())
        || trail_bloom_link
            .get("bloom_key_sha256")
            .and_then(Value::as_str)
            != Some(request.vfx_bloom_key_sha256.as_str())
    {
        return Err(invalid("VFX trail bloom parent binding differs"));
    }

    let links = [
        sequence_link,
        frame_link,
        bloom_link,
        particle_link,
        trail_link,
        trail_bloom_link,
    ];
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
        ("source_artifact_sha256", request.artifact_sha256.as_str()),
        (
            "camera_object_sha256",
            request.camera_object_sha256.as_str(),
        ),
        (
            "camera_identity_sha256",
            request.camera_identity_sha256.as_str(),
        ),
        (
            "render_profile_sha256",
            request.render_profile_sha256.as_str(),
        ),
        (
            "render_worker_build_cohort_sha256",
            request.render_worker_build_cohort_sha256.as_str(),
        ),
    ] {
        if links[0].get(field).and_then(Value::as_str) != Some(expected) {
            return Err(invalid(format!(
                "VFX request.{field} differs from dependency links"
            )));
        }
    }
    if frame_link
        .get("sample_request_sha256")
        .and_then(Value::as_str)
        != Some(request.sample_request_sha256.as_str())
        || bloom_link
            .get("sample_request_sha256")
            .and_then(Value::as_str)
            != Some(request.sample_request_sha256.as_str())
        || particle_link
            .get("sample_request_sha256")
            .and_then(Value::as_str)
            != Some(request.sample_request_sha256.as_str())
        || trail_link
            .get("sample_request_sha256")
            .and_then(Value::as_str)
            != Some(request.sample_request_sha256.as_str())
        || trail_bloom_link
            .get("sample_request_sha256")
            .and_then(Value::as_str)
            != Some(request.sample_request_sha256.as_str())
    {
        return Err(invalid("VFX sample request binding differs"));
    }
    for (index, link) in links.iter().enumerate().skip(1) {
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
            same_link_field(links[0], link, field, &format!("VFX layer {index}"))?;
        }
    }

    Ok(DependencyReadback {
        profile,
        sequence,
        frame,
        bloom,
        particles,
        trails,
        trail_bloom,
        animated_socket,
    })
}

fn receipt_socket_attachment(value: &Value) -> bool {
    let receipt = value.get("receipt").unwrap_or(value);
    receipt
        .get("glb_socket_transform_executed")
        .and_then(Value::as_bool)
        == Some(true)
        && receipt
            .get("anchor_is_runtime_sidecar_not_glb_socket")
            .and_then(Value::as_bool)
            != Some(true)
        && receipt.get("node_materialization").and_then(Value::as_str)
            != Some("sidecar-only-not-glb-nodes")
}

fn nonfunctional(value: &Value) -> bool {
    let candidates = [value.get("receipt"), value.get("vfx_profile"), Some(value)];
    candidates.into_iter().flatten().all(|item| {
        item.get("functional_semantics").and_then(Value::as_bool) != Some(true)
            && item.get("actual_engine_roundtrip").and_then(Value::as_bool) != Some(true)
            && item.get("candidate_confirmed").and_then(Value::as_bool) != Some(true)
            && item.get("export_performed").and_then(Value::as_bool) != Some(true)
    })
}

fn compute_gate(readback: &DependencyReadback) -> CandidateAnimationVfxQualityHardGate {
    let attachment = [
        &readback.profile,
        &readback.sequence,
        &readback.frame,
        &readback.bloom,
        &readback.particles,
        &readback.trails,
        &readback.trail_bloom,
    ]
    .iter()
    .all(|value| receipt_socket_attachment(value));
    let nonfunctional_scope = [
        &readback.profile,
        &readback.sequence,
        &readback.frame,
        &readback.bloom,
        &readback.particles,
        &readback.trails,
        &readback.trail_bloom,
        &readback.animated_socket,
    ]
    .iter()
    .all(|value| nonfunctional(value));
    CandidateAnimationVfxQualityHardGate {
        material_surface_head_binding: true,
        material_surface_quality: true,
        delivery_lod0_binding: true,
        anchor_set_binding: true,
        animation_clip_binding: true,
        animation_glb_readback: true,
        animated_socket_readback: true,
        vfx_profile_binding: true,
        base_frame_stack: true,
        bloom_stack: true,
        particle_stack: true,
        trail_stack: true,
        trail_bloom_stack: true,
        cross_layer_parent_binding: true,
        sample_camera_binding: true,
        worker_cohort_binding: true,
        render_pass_byte_exact: true,
        bounded_resource_policy: true,
        vfx_glb_socket_attachment: attachment,
        nonfunctional_scope,
    }
}

fn gate_passed(gate: &CandidateAnimationVfxQualityHardGate) -> bool {
    [
        gate.material_surface_head_binding,
        gate.material_surface_quality,
        gate.delivery_lod0_binding,
        gate.anchor_set_binding,
        gate.animation_clip_binding,
        gate.animation_glb_readback,
        gate.animated_socket_readback,
        gate.vfx_profile_binding,
        gate.base_frame_stack,
        gate.bloom_stack,
        gate.particle_stack,
        gate.trail_stack,
        gate.trail_bloom_stack,
        gate.cross_layer_parent_binding,
        gate.sample_camera_binding,
        gate.worker_cohort_binding,
        gate.render_pass_byte_exact,
        gate.bounded_resource_policy,
        gate.vfx_glb_socket_attachment,
        gate.nonfunctional_scope,
    ]
    .into_iter()
    .all(|value| value)
}

fn record_from_request(
    request: &CandidateAnimationVfxQualityPrepareRequest,
    request_sha256: &str,
    material: &forgecad_contracts::CandidateMaterialSurfaceQualityRecord,
    head: &Value,
    readback: &DependencyReadback,
) -> Result<CandidateAnimationVfxQualityRecord, RuntimeError> {
    let sequence_link = require_link(&readback.sequence, "VFX sequence")?;
    let frame_link = require_link(&readback.frame, "VFX frame")?;
    let bloom_link = require_link(&readback.bloom, "VFX bloom")?;
    let particle_link = require_link(&readback.particles, "VFX particles")?;
    let trail_link = require_link(&readback.trails, "VFX trails")?;
    let trail_bloom_link = require_link(&readback.trail_bloom, "VFX trail bloom")?;
    let _profile_link = require_link(&readback.profile, "VFX profile")?;
    let profile = readback
        .profile
        .get("vfx_profile")
        .ok_or_else(|| invalid("VFX profile is unavailable"))?;
    let gate = compute_gate(readback);
    let passed = gate_passed(&gate);
    let mut record = CandidateAnimationVfxQualityRecord {
        schema_version: "CandidateAnimationVfxQuality@1".to_owned(),
        animation_vfx_quality_id: request.animation_vfx_quality_id.clone(),
        project_id: request.project_id.clone(),
        source_material_surface_transition_id: request
            .source_material_surface_transition_id
            .clone(),
        source_material_surface_transition_sha256: request
            .source_material_surface_transition_sha256
            .clone(),
        source_material_surface_head_canonical_sha256: request
            .source_material_surface_head_canonical_sha256
            .clone(),
        source_material_surface_quality_id: request.source_material_surface_quality_id.clone(),
        source_material_surface_quality_report_object_sha256: request
            .source_material_surface_quality_report_object_sha256
            .clone(),
        source_material_surface_quality_canonical_sha256: request
            .source_material_surface_quality_canonical_sha256
            .clone(),
        candidate_id: request.candidate_id.clone(),
        candidate_state_sha256: request.candidate_state_sha256.clone(),
        artifact_id: request.artifact_id.clone(),
        artifact_sha256: request.artifact_sha256.clone(),
        delivery_manifest_object_sha256: request.delivery_manifest_object_sha256.clone(),
        anchor_set_object_sha256: request.anchor_set_object_sha256.clone(),
        anchor_set_canonical_sha256: request.anchor_set_canonical_sha256.clone(),
        animation_clip_id: request.animation_clip_id.clone(),
        animation_clip_object_sha256: request.animation_clip_object_sha256.clone(),
        animation_clip_sha256: request.animation_clip_sha256.clone(),
        animated_socket_materialization_key_sha256: request
            .animated_socket_materialization_key_sha256
            .clone(),
        animated_artifact_sha256: request.animated_artifact_sha256.clone(),
        animation_receipt_object_sha256: request.animation_receipt_object_sha256.clone(),
        animation_receipt_canonical_sha256: request.animation_receipt_canonical_sha256.clone(),
        derived_animated_socket_artifact_sha256: request
            .derived_animated_socket_artifact_sha256
            .clone(),
        animated_socket_receipt_object_sha256: request
            .animated_socket_receipt_object_sha256
            .clone(),
        vfx_profile_object_sha256: request.vfx_profile_object_sha256.clone(),
        vfx_profile_canonical_sha256: profile
            .get("canonical_sha256")
            .and_then(Value::as_str)
            .unwrap_or(&request.vfx_profile_canonical_sha256)
            .to_owned(),
        vfx_sequence_key_sha256: request.vfx_sequence_key_sha256.clone(),
        vfx_sequence_canonical_sha256: link_sha(sequence_link, "canonical_sha256", "sequence")?,
        vfx_frame_key_sha256: request.vfx_frame_key_sha256.clone(),
        vfx_frame_canonical_sha256: link_sha(frame_link, "canonical_sha256", "frame")?,
        vfx_bloom_key_sha256: request.vfx_bloom_key_sha256.clone(),
        vfx_bloom_canonical_sha256: link_sha(bloom_link, "canonical_sha256", "bloom")?,
        vfx_particle_key_sha256: request.vfx_particle_key_sha256.clone(),
        vfx_particle_canonical_sha256: link_sha(particle_link, "canonical_sha256", "particle")?,
        vfx_trail_key_sha256: request.vfx_trail_key_sha256.clone(),
        vfx_trail_canonical_sha256: link_sha(trail_link, "canonical_sha256", "trail")?,
        vfx_trail_bloom_key_sha256: request.vfx_trail_bloom_key_sha256.clone(),
        vfx_trail_bloom_canonical_sha256: link_sha(
            trail_bloom_link,
            "canonical_sha256",
            "trail bloom",
        )?,
        particle_history_key_sha256s: request.particle_history_key_sha256s.clone(),
        sample_request_sha256: request.sample_request_sha256.clone(),
        camera_object_sha256: request.camera_object_sha256.clone(),
        camera_identity_sha256: request.camera_identity_sha256.clone(),
        render_profile_sha256: request.render_profile_sha256.clone(),
        render_worker_build_cohort_sha256: request.render_worker_build_cohort_sha256.clone(),
        animation_vfx_scope: SCOPE.to_owned(),
        animation_vfx_policy: POLICY.to_owned(),
        animation_vfx_policy_sha256: request.animation_vfx_policy_sha256.clone(),
        from_stage: "material-surface".to_owned(),
        to_stage: "animation-vfx".to_owned(),
        input_sha256: request.input_sha256.clone(),
        candidate_binding_status: SAME_CANDIDATE.to_owned(),
        hard_gate: gate,
        validator_status: if passed { "passed" } else { "failed" }.to_owned(),
        hard_gate_passed: passed,
        animation_status: "structural_only".to_owned(),
        vfx_status: "structural_only".to_owned(),
        visual_quality_status: "NOT_PROVEN".to_owned(),
        artistic_quality_status: "NOT_PROVEN".to_owned(),
        human_review_status: "NOT_RUN".to_owned(),
        commercial_fps_quality_status: "NOT_PROVEN".to_owned(),
        commercial_engine_status: "NOT_RUN".to_owned(),
        actual_engine_roundtrip: false,
        functional_semantics: false,
        materialization_status: STATUS.to_owned(),
        quality_status: "structural_only".to_owned(),
        runtime_write_performed: true,
        production_stage_advanced: false,
        candidate_confirmed: false,
        version_created: false,
        export_performed: false,
        request_sha256: request_sha256.to_owned(),
        canonical_sha256: String::new(),
        created_at: material.created_at.clone(),
    };
    if head.get("canonical_sha256").and_then(Value::as_str)
        != Some(
            record
                .source_material_surface_head_canonical_sha256
                .as_str(),
        )
    {
        return Err(invalid("material-surface head canonical hash drifted"));
    }
    let mut preimage = serde_json::to_value(&record)
        .map_err(|error| invalid(format!("quality record cannot be serialized: {error}")))?;
    preimage["canonical_sha256"] = Value::String(String::new());
    record.canonical_sha256 = canonical_json_hash(&preimage);
    Ok(record)
}

fn release_report(
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

fn result_value(
    record: &CandidateAnimationVfxQualityRecord,
    replayed: bool,
    schema_version: &str,
    runtime_write: bool,
) -> Result<Value, RuntimeError> {
    Ok(json!({
        "schema_version":schema_version,
        "animation_vfx_quality":serde_json::to_value(record).map_err(|error| invalid(error.to_string()))?,
        "replayed":replayed,
        "runtime_write":runtime_write,
        "production_stage_advanced":false,
        "candidate_confirmed":false,
        "version_created":false,
        "export_performed":false
    }))
}

fn request_from_record(
    record: &CandidateAnimationVfxQualityRecord,
) -> CandidateAnimationVfxQualityPrepareRequest {
    CandidateAnimationVfxQualityPrepareRequest {
        schema_version: PREPARE_SCHEMA.to_owned(),
        animation_vfx_quality_id: record.animation_vfx_quality_id.clone(),
        project_id: record.project_id.clone(),
        source_material_surface_transition_id: record.source_material_surface_transition_id.clone(),
        source_material_surface_transition_sha256: record
            .source_material_surface_transition_sha256
            .clone(),
        source_material_surface_head_canonical_sha256: record
            .source_material_surface_head_canonical_sha256
            .clone(),
        source_material_surface_quality_id: record.source_material_surface_quality_id.clone(),
        source_material_surface_quality_report_object_sha256: record
            .source_material_surface_quality_report_object_sha256
            .clone(),
        source_material_surface_quality_canonical_sha256: record
            .source_material_surface_quality_canonical_sha256
            .clone(),
        candidate_id: record.candidate_id.clone(),
        candidate_state_sha256: record.candidate_state_sha256.clone(),
        artifact_id: record.artifact_id.clone(),
        artifact_sha256: record.artifact_sha256.clone(),
        delivery_manifest_object_sha256: record.delivery_manifest_object_sha256.clone(),
        anchor_set_object_sha256: record.anchor_set_object_sha256.clone(),
        anchor_set_canonical_sha256: record.anchor_set_canonical_sha256.clone(),
        animation_clip_id: record.animation_clip_id.clone(),
        animation_clip_object_sha256: record.animation_clip_object_sha256.clone(),
        animation_clip_sha256: record.animation_clip_sha256.clone(),
        animated_socket_materialization_key_sha256: record
            .animated_socket_materialization_key_sha256
            .clone(),
        animated_artifact_sha256: record.animated_artifact_sha256.clone(),
        animation_receipt_object_sha256: record.animation_receipt_object_sha256.clone(),
        animation_receipt_canonical_sha256: record.animation_receipt_canonical_sha256.clone(),
        derived_animated_socket_artifact_sha256: record
            .derived_animated_socket_artifact_sha256
            .clone(),
        animated_socket_receipt_object_sha256: record.animated_socket_receipt_object_sha256.clone(),
        vfx_profile_object_sha256: record.vfx_profile_object_sha256.clone(),
        vfx_profile_canonical_sha256: record.vfx_profile_canonical_sha256.clone(),
        vfx_sequence_key_sha256: record.vfx_sequence_key_sha256.clone(),
        vfx_sequence_canonical_sha256: record.vfx_sequence_canonical_sha256.clone(),
        vfx_frame_key_sha256: record.vfx_frame_key_sha256.clone(),
        vfx_frame_canonical_sha256: record.vfx_frame_canonical_sha256.clone(),
        vfx_bloom_key_sha256: record.vfx_bloom_key_sha256.clone(),
        vfx_bloom_canonical_sha256: record.vfx_bloom_canonical_sha256.clone(),
        vfx_particle_key_sha256: record.vfx_particle_key_sha256.clone(),
        vfx_particle_canonical_sha256: record.vfx_particle_canonical_sha256.clone(),
        vfx_trail_key_sha256: record.vfx_trail_key_sha256.clone(),
        vfx_trail_canonical_sha256: record.vfx_trail_canonical_sha256.clone(),
        vfx_trail_bloom_key_sha256: record.vfx_trail_bloom_key_sha256.clone(),
        vfx_trail_bloom_canonical_sha256: record.vfx_trail_bloom_canonical_sha256.clone(),
        particle_history_key_sha256s: record.particle_history_key_sha256s.clone(),
        sample_request_sha256: record.sample_request_sha256.clone(),
        camera_object_sha256: record.camera_object_sha256.clone(),
        camera_identity_sha256: record.camera_identity_sha256.clone(),
        render_profile_sha256: record.render_profile_sha256.clone(),
        render_worker_build_cohort_sha256: record.render_worker_build_cohort_sha256.clone(),
        animation_vfx_scope: record.animation_vfx_scope.clone(),
        animation_vfx_policy: record.animation_vfx_policy.clone(),
        animation_vfx_policy_sha256: record.animation_vfx_policy_sha256.clone(),
        from_stage: record.from_stage.clone(),
        to_stage: record.to_stage.clone(),
        input_sha256: record.input_sha256.clone(),
        idempotency_key: record.animation_vfx_quality_id.clone(),
    }
}

impl Runtime {
    pub fn candidate_animation_vfx_quality_prepare(
        &self,
        request: Value,
    ) -> Result<Value, RuntimeError> {
        let (request, request_sha256) = parse_prepare(&request)?;
        let (_stage, head, material) = validate_stage_and_material(self, &request)?;
        let readback = validate_dependencies(self, &request)?;
        let record = record_from_request(&request, &request_sha256, &material, &head, &readback)?;
        let bytes = canonical_json_bytes(
            &serde_json::to_value(&record).map_err(|error| invalid(error.to_string()))?,
        )
        .map_err(|error| invalid(error.to_string()))?;
        if bytes.len() > MAX_REPORT_BYTES {
            return Err(invalid("animation/VFX report exceeds one MiB"));
        }
        let reservation = self.store.begin_cas_reservation();
        let report = self.store.put_object_reserved(
            &reservation,
            &bytes,
            None,
            REPORT_MIME,
            REPORT_KIND,
            &record.created_at,
        )?;
        match self
            .store
            .record_candidate_animation_vfx_quality_with_replay(&record, &report.record)
        {
            Ok((stored, replayed)) => {
                release_report(self, &reservation, &report, false);
                result_value(&stored, replayed, PREPARE_RESULT_SCHEMA, true)
            }
            Err(error) => {
                release_report(self, &reservation, &report, true);
                Err(error.into())
            }
        }
    }

    pub fn candidate_animation_vfx_quality_get(
        &self,
        request: Value,
    ) -> Result<Value, RuntimeError> {
        let request = parse_get(&request)?;
        let record = self
            .store
            .get_candidate_animation_vfx_quality(&request.animation_vfx_quality_id)?
            .ok_or_else(|| invalid("animation/VFX quality is unavailable"))?;
        if record.project_id != request.project_id || record.candidate_id != request.candidate_id {
            return Err(invalid("animation/VFX quality scope differs"));
        }
        let replay_request = request_from_record(&record);
        let (_stage, head, material) = validate_stage_and_material(self, &replay_request)?;
        let readback = validate_dependencies(self, &replay_request)?;
        let recomputed = record_from_request(
            &replay_request,
            &record.request_sha256,
            &material,
            &head,
            &readback,
        )?;
        if recomputed.canonical_sha256 != record.canonical_sha256
            || recomputed.hard_gate != record.hard_gate
            || recomputed.validator_status != record.validator_status
            || recomputed.hard_gate_passed != record.hard_gate_passed
        {
            return Err(invalid("animation/VFX quality receipt is tampered"));
        }
        result_value(&record, true, GET_RESULT_SCHEMA, false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all_socket_truth_value() -> Value {
        json!({
            "receipt": {
                "glb_socket_transform_executed": true,
                "anchor_is_runtime_sidecar_not_glb_socket": false,
                "node_materialization": "glb-nodes",
                "functional_semantics": false,
                "actual_engine_roundtrip": false,
                "candidate_confirmed": false,
                "export_performed": false
            }
        })
    }

    fn dependency_fixture() -> DependencyReadback {
        let value = all_socket_truth_value();
        DependencyReadback {
            profile: value.clone(),
            sequence: value.clone(),
            frame: value.clone(),
            bloom: value.clone(),
            particles: value.clone(),
            trails: value.clone(),
            trail_bloom: value.clone(),
            animated_socket: value,
        }
    }

    #[test]
    fn socket_attachment_gate_is_fail_closed_for_sidecar_receipts() {
        let value = json!({
            "receipt": {
                "anchor_is_runtime_sidecar_not_glb_socket": true,
                "functional_semantics": false,
                "actual_engine_roundtrip": false,
                "candidate_confirmed": false,
                "export_performed": false
            }
        });
        assert!(!receipt_socket_attachment(&value));
    }

    #[test]
    fn socket_attachment_gate_requires_explicit_transform_execution() {
        let value = json!({
            "receipt": {
                "glb_socket_transform_executed": false,
                "functional_semantics": false,
                "actual_engine_roundtrip": false,
                "candidate_confirmed": false,
                "export_performed": false
            }
        });
        assert!(!receipt_socket_attachment(&value));
    }

    #[test]
    fn nonfunctional_scope_rejects_engine_or_functional_claims() {
        assert!(!nonfunctional(&json!({"functional_semantics":true})));
        assert!(!nonfunctional(&json!({"actual_engine_roundtrip":true})));
        assert!(nonfunctional(
            &json!({"functional_semantics":false,"actual_engine_roundtrip":false})
        ));
    }

    #[test]
    fn full_dependency_fixture_passes_only_when_socket_truth_is_explicit() {
        let gate = compute_gate(&dependency_fixture());
        assert!(gate.vfx_glb_socket_attachment);
        assert!(gate_passed(&gate));
    }

    #[test]
    fn blocked_dependency_fixture_is_structural_and_cannot_forge_attachment() {
        let mut dependency = dependency_fixture();
        dependency.trails = json!({
            "receipt": {
                "anchor_is_runtime_sidecar_not_glb_socket": true,
                "functional_semantics": false,
                "actual_engine_roundtrip": false,
                "candidate_confirmed": false,
                "export_performed": false
            }
        });
        let gate = compute_gate(&dependency);
        assert!(!gate.vfx_glb_socket_attachment);
        assert!(!gate_passed(&gate));
        assert!(
            gate.material_surface_head_binding
                && gate.render_pass_byte_exact
                && gate.nonfunctional_scope
        );
    }

    #[test]
    fn get_request_is_closed_and_rejects_unknown_fields() {
        let value = json!({
            "schema_version": GET_SCHEMA,
            "animation_vfx_quality_id": "quality-1",
            "project_id": "project-1",
            "candidate_id": "candidate-1",
            "raw_glb": "forbidden"
        });
        assert!(parse_get(&value).is_err());
    }

    #[test]
    fn attachment_gate_rejects_missing_transform_execution_after_restart() {
        let value = json!({
            "receipt": {
                "anchor_is_runtime_sidecar_not_glb_socket": false,
                "node_materialization": "glb-nodes",
                "functional_semantics": false,
                "actual_engine_roundtrip": false,
                "candidate_confirmed": false,
                "export_performed": false
            }
        });
        assert!(!receipt_socket_attachment(&value));
    }
}
