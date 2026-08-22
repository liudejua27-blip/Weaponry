//! Closed, candidate-bound rigid glTF animation materialization.
//!
//! This module does not implement an armature, skinning, morph targets, IK,
//! constraints, NLA, F-Curves or scripting. It consumes only an immutable
//! Runtime-owned MechanicalAnimationClip and its scheduled, double-Worker
//! verified rigid Part deltas.

use super::{
    canonical_json_bytes, canonical_json_hash, game_asset_delivery, mechanical_pose, now_string,
    sha256_hex, CasObject, Runtime, RuntimeError,
};
use forgecad_contracts::GameWeaponAnimatedGlbSocketMaterializationLinkRecord;
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet};

const ERROR: &str = "MECHANICAL_ANIMATION_GLB_INVALID";
const MAX_GLB_BYTES: usize = 64 * 1024 * 1024;
const ANIMATED_SOCKET_PREPARE_SCHEMA: &str =
    "GameWeaponAnimatedGlbSocketMaterializationPrepareRequest@1";
const ANIMATED_SOCKET_PREPARE_RESULT_SCHEMA: &str =
    "GameWeaponAnimatedGlbSocketMaterializationPrepareResult@1";
const ANIMATED_SOCKET_GET_SCHEMA: &str = "GameWeaponAnimatedGlbSocketMaterializationGetRequest@1";
const ANIMATED_SOCKET_GET_RESULT_SCHEMA: &str =
    "GameWeaponAnimatedGlbSocketMaterializationGetResult@1";
const ANIMATED_SOCKET_LINK_SCHEMA: &str = "GameWeaponAnimatedGlbSocketMaterializationLink@1";
const ANIMATED_SOCKET_RECEIPT_SCHEMA: &str = "GameWeaponAnimatedGlbSocketMaterializationReceipt@1";
const ANIMATED_SOCKET_POLICY: &str =
    "gltf-animated-anchor-node-materialization-preserve-animations-renderable-content@1";
const ANIMATED_SOCKET_LOD_SCOPE: &str = "lod0-animated-source-only@1";
const ANIMATED_SOCKET_STATUS: &str =
    "runtime-owned-durable-game-weapon-animated-glb-socket-materialization";
const ANIMATED_SOCKET_GLB_KIND: &str = "game-weapon-animated-glb-socket-materialized-glb";
const ANIMATED_SOCKET_RECEIPT_KIND: &str =
    "game-weapon-animated-glb-socket-materialization-receipt";
const MECHANICAL_ANIMATION_RECEIPT_SCHEMA: &str = "MechanicalAnimationGlbReceipt@1";
const MECHANICAL_ANIMATION_GLB_KIND: &str = "mechanical-animation-glb";
const MECHANICAL_ANIMATION_RECEIPT_KIND: &str = "mechanical-animation-glb-receipt";
const ANIMATED_SOCKET_SEMANTIC_SCOPE: &str = "fictional-nonfunctional-game-visual-authoring-only@1";

const SOCKET_TRANSFORM_PROJECTION_PREPARE_SCHEMA: &str =
    "GameWeaponAnimatedGlbSocketTransformProjectionPrepareRequest@1";
const SOCKET_TRANSFORM_PROJECTION_GET_SCHEMA: &str =
    "GameWeaponAnimatedGlbSocketTransformProjectionGetRequest@1";
const SOCKET_TRANSFORM_PROJECTION_PREPARE_RESULT_SCHEMA: &str =
    "GameWeaponAnimatedGlbSocketTransformProjectionPrepareResult@1";
const SOCKET_TRANSFORM_PROJECTION_GET_RESULT_SCHEMA: &str =
    "GameWeaponAnimatedGlbSocketTransformProjectionGetResult@1";
const SOCKET_TRANSFORM_PROJECTION_SCHEMA: &str = "GameWeaponAnimatedGlbSocketTransformProjection@1";
const SOCKET_TRANSFORM_PROJECTION_FRAME_SCHEMA: &str =
    "GameWeaponAnimatedGlbSocketTransformProjectionFrame@1";
const SOCKET_TRANSFORM_PROJECTION_KIND: &str =
    "game-weapon-animated-glb-socket-transform-projection";
const SOCKET_TRANSFORM_PROJECTION_POLICY: &str =
    "glb-animation-linear-nlerp-flat-part-hierarchy-composed-world-trs@1";
const SOCKET_TRANSFORM_PART_HIERARCHY_POLICY: &str = "flat-identity-rest-part-hierarchy-only@1";
const SOCKET_TRANSFORM_REPRESENTATION_POLICY: &str = "trs-quaternion-no-matrix-no-shear@1";
const SOCKET_TRANSFORM_FRAME_SCOPE: &str = "lod0-animation-frame-range-1-16@1";
const SOCKET_TRANSFORM_COORDINATE_SYSTEM: &str = "forgecad-rh-y-up-m@1";
const SOCKET_TRANSFORM_CONVENTION: &str = "column-vector-parent-world-times-trs-quaternion-xyzw@1";
const SOCKET_TRANSFORM_FLOAT_POLICY: &str = "f32-round-nearest-canonical-json@1";
const SOCKET_TRANSFORM_STATUS: &str =
    "runtime-owned-durable-game-weapon-animated-glb-socket-transform-projection";
const SOCKET_TRANSFORM_LIMITATIONS: [&str; 8] = [
    "flat-identity-rest-part-hierarchy-only",
    "nested-part-hierarchy-rejected",
    "nonidentity-rest-part-transform-rejected",
    "matrix-and-shear-rejected",
    "structural-transform-readback-only",
    "no-visual-quality-or-likeness-pass",
    "no-commercial-engine-roundtrip",
    "no-functional-weapon-semantics",
];
const SOCKET_TRANSFORM_ROLES: [&str; 6] = [
    "weapon-root",
    "grip-primary",
    "muzzle-vfx",
    "magazine-well",
    "sight-primary",
    "energy-core-vfx",
];

const ANIMATED_SOCKET_LIMITATIONS: [&str; 8] = [
    "no-ballistics",
    "no-damage-or-hitbox",
    "no-physics-binding",
    "no-manufacturing-or-operation",
    "no-commercial-engine-roundtrip",
    "no-runtime-pivot-proof",
    "no-visual-quality-pass",
    "animation-readback-is-structural-only",
];

#[derive(Clone, Copy, Debug, PartialEq)]
struct RigidTransform {
    translation: [f32; 3],
    rotation: [f32; 4],
}

#[derive(Debug, Clone)]
struct AnimatedSourceInspection {
    derived_node_count: usize,
    sampler_count: usize,
    channel_count: usize,
    accessor_count_added: usize,
    buffer_view_count_added: usize,
    sample_time_ticks: Vec<u64>,
    part_ids: Vec<String>,
    source_animation_projection_sha256: String,
    derived_animation_projection_sha256: String,
    source_animation_validation_sha256: String,
    derived_animation_validation_sha256: String,
}

#[derive(Debug, Clone)]
struct ProjectionRequest {
    projection_key_sha256: String,
    project_id: String,
    candidate_id: String,
    candidate_state_sha256: String,
    delivery_manifest_object_sha256: String,
    source_artifact_sha256: String,
    source_artifact_readback_sha256: String,
    animated_artifact_sha256: String,
    animated_artifact_readback_sha256: String,
    animation_receipt_object_sha256: String,
    animation_receipt_canonical_sha256: String,
    animated_socket_materialization_key_sha256: String,
    derived_animated_socket_artifact_sha256: String,
    derived_animated_socket_artifact_readback_sha256: String,
    derived_animated_socket_receipt_object_sha256: String,
    derived_animated_socket_receipt_canonical_sha256: String,
    anchor_set_object_sha256: String,
    anchor_set_canonical_sha256: String,
    animation_clip_id: String,
    animation_clip_object_sha256: String,
    animation_clip_canonical_sha256: String,
    socket_node_id_encoding_sha256: String,
    socket_node_inventory_sha256: String,
    socket_roles_sha256: String,
    part_hierarchy_sha256: String,
    part_hierarchy_policy: String,
    transform_representation_policy: String,
    sample_schedule_sha256: String,
    sample_time_ticks: Vec<u64>,
    frame_scope: String,
    timebase_hz: u64,
    transform_projection_policy: String,
    coordinate_system: String,
    transform_convention: String,
    float_quantization_policy: String,
    input_sha256: String,
    _idempotency_key: String,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct ProjectionPose {
    pub(super) translation: [f32; 3],
    pub(super) rotation: [f32; 4],
}

#[derive(Debug, Clone)]
pub(super) struct ProjectionChannel {
    pub(super) node_index: usize,
    pub(super) path: String,
    pub(super) times_seconds: Vec<f32>,
    pub(super) values: Vec<Vec<f32>>,
}

#[derive(Debug, Clone)]
pub(super) struct ProjectionAnimation {
    pub(super) channels: Vec<ProjectionChannel>,
    pub(super) source_animation_projection_sha256: String,
    pub(super) derived_animation_projection_sha256: String,
}

#[derive(Debug, Clone)]
pub(super) struct ProjectionSocketNode {
    pub(super) socket_node_id: String,
    pub(super) anchor_id: String,
    pub(super) role: String,
    pub(super) node_index: usize,
    pub(super) parent_node_index: isize,
    pub(super) node_name: String,
    pub(super) parent_node_name: Option<String>,
    pub(super) parent_kind: String,
    pub(super) owner_part_id: Option<String>,
    pub(super) local: ProjectionPose,
}

const SOCKET_TRANSFORM_PROJECTION_PREPARE_FIELDS: &[&str] = &[
    "schema_version",
    "projection_key_sha256",
    "project_id",
    "candidate_id",
    "candidate_state_sha256",
    "delivery_manifest_object_sha256",
    "source_artifact_sha256",
    "source_artifact_readback_sha256",
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
    "animation_clip_id",
    "animation_clip_object_sha256",
    "animation_clip_canonical_sha256",
    "socket_node_id_encoding_sha256",
    "socket_node_inventory_sha256",
    "socket_roles_sha256",
    "socket_roles",
    "part_hierarchy_sha256",
    "part_hierarchy_policy",
    "transform_representation_policy",
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

const SOCKET_TRANSFORM_PROJECTION_GET_FIELDS: &[&str] = &[
    "schema_version",
    "projection_key_sha256",
    "project_id",
    "candidate_id",
];

pub(super) fn prepare(runtime: &Runtime, request: &Value) -> Result<Value, RuntimeError> {
    let object = exact_object(
        request,
        &[
            "schema_version",
            "project_id",
            "candidate_id",
            "candidate_state_sha256",
            "clip_id",
            "materialization_policy",
            "canonical_sha256",
        ],
        "MechanicalAnimationGlbPrepareRequest@1",
    )?;
    if text(object, "schema_version")? != "MechanicalAnimationGlbPrepareRequest@1"
        || text(object, "materialization_policy")?
            != "rigid-node-trs-gltf-linear-scheduled-samples@1"
    {
        return invalid("rigid animation materialization policy differs");
    }
    verify_request_canonical(request)?;
    let project_id = identifier(object, "project_id")?.to_owned();
    let candidate_id = identifier(object, "candidate_id")?.to_owned();
    let candidate_state_sha256 = sha(object, "candidate_state_sha256")?.to_owned();
    let clip_id = identifier(object, "clip_id")?.to_owned();
    let candidate = runtime
        .store
        .get_candidate(&candidate_id)?
        .ok_or_else(|| error("candidate is unavailable"))?;
    if candidate.project_id != project_id || candidate.canonical_sha256 != candidate_state_sha256 {
        return invalid("candidate state binding differs");
    }
    let record = runtime
        .store
        .get_mechanical_animation_clip_link(&candidate_id, &clip_id)?
        .ok_or_else(|| error("durable mechanical animation clip is unavailable"))?;
    if record.project_id != project_id {
        return invalid("mechanical animation clip belongs to another project");
    }
    let link = mechanical_pose::load_animation_clip_link(runtime, &record)?;
    let clip = link
        .get("clip")
        .and_then(Value::as_object)
        .ok_or_else(|| error("durable mechanical animation clip is invalid"))?;
    let ticks = clip["sampling_policy"]["sample_time_ticks"]
        .as_array()
        .filter(|ticks| (2..=16).contains(&ticks.len()))
        .ok_or_else(|| error("rigid glTF animation requires 2..16 scheduled ticks"))?
        .iter()
        .map(|value| {
            value
                .as_u64()
                .filter(|tick| *tick <= 1_000_000)
                .ok_or_else(|| error("scheduled tick is invalid"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if ticks.windows(2).any(|pair| pair[0] >= pair[1]) {
        return invalid("scheduled ticks must be strictly increasing");
    }
    let mut frames = Vec::with_capacity(ticks.len());
    let mut frame_hashes = Vec::with_capacity(ticks.len());
    let mut expected_parts: Option<Vec<String>> = None;
    for tick in &ticks {
        let mut preview_request = json!({
            "schema_version":"MechanicalAnimationClipPreviewRequest@1",
            "project_id":project_id,
            "candidate_id":candidate_id,
            "clip_id":clip_id,
            "sample_time_ticks":tick,
            "preview_policy":"single-tick-transient-double-worker-replay@1",
            "canonical_sha256":""
        });
        let mut preview_preimage = preview_request.clone();
        preview_preimage
            .as_object_mut()
            .expect("preview request is an object")
            .remove("canonical_sha256");
        preview_request["canonical_sha256"] = Value::String(canonical_json_hash(&preview_preimage));
        let preview = mechanical_pose::animation_clip_preview_get(runtime, &preview_request)?;
        let deltas = preview["pose_geometry_preview"]["part_deltas"]
            .as_array()
            .ok_or_else(|| error("verified frame omitted Part deltas"))?;
        let mut frame = BTreeMap::new();
        for delta in deltas {
            let part_id = delta
                .get("part_id")
                .and_then(Value::as_str)
                .filter(|value| valid_identifier(value))
                .ok_or_else(|| error("verified frame Part ID is invalid"))?;
            let pose = delta
                .get("delta_pose")
                .and_then(Value::as_object)
                .ok_or_else(|| error("verified frame delta pose is invalid"))?;
            if frame
                .insert(part_id.to_owned(), parse_transform(pose)?)
                .is_some()
            {
                return invalid("verified frame duplicates a Part");
            }
        }
        let parts = frame.keys().cloned().collect::<Vec<_>>();
        if expected_parts
            .as_ref()
            .is_some_and(|expected| expected != &parts)
        {
            return invalid("verified animation frames differ in Part coverage");
        }
        expected_parts.get_or_insert(parts);
        frame_hashes.push(
            preview["frame_sha256"]
                .as_str()
                .ok_or_else(|| error("verified frame hash is missing"))?
                .to_owned(),
        );
        frames.push(frame);
    }
    let part_ids = expected_parts.ok_or_else(|| error("animation has no Parts"))?;
    let source_glb = runtime.cas_read(&record.artifact_id)?;
    if source_glb.len() > MAX_GLB_BYTES || sha256_hex(&source_glb) != record.artifact_id {
        return invalid("source artifact bytes differ from the durable clip binding");
    }
    let animated_glb = materialize(
        &source_glb,
        &record.artifact_id,
        &record.clip_sha256,
        &part_ids,
        &ticks,
        &frames,
    )?;
    let validation = validate_animated_glb(
        &source_glb,
        &animated_glb,
        &record.artifact_id,
        &record.clip_sha256,
        &part_ids,
        &ticks,
        &frames,
    )?;
    let artifact_object = runtime.put_object(
        &animated_glb,
        None,
        "model/gltf-binary",
        "mechanical-animation-glb",
    )?;
    let mut receipt = json!({
        "schema_version":"MechanicalAnimationGlbReceipt@1",
        "project_id":project_id,
        "candidate_id":candidate_id,
        "candidate_state_sha256":candidate_state_sha256,
        "source_artifact_sha256":record.artifact_id,
        "artifact_readback_sha256":record.artifact_readback_sha256,
        "geometry_candidate_evidence_sha256":record.geometry_candidate_evidence_sha256,
        "program_sha256":record.program_sha256,
        "operator_catalog_sha256":record.operator_catalog_sha256,
        "readback_config_sha256":record.readback_config_sha256,
        "animated_artifact_sha256":artifact_object.record.sha256,
        "clip_id":record.clip_id,
        "clip_object_sha256":record.clip_object_sha256,
        "clip_sha256":record.clip_sha256,
        "rest_frame_sha256":record.rest_frame_sha256,
        "pose_action_sha256":record.pose_action_sha256,
        "source_replay_worker_cohort_sha256":record.source_replay_worker_cohort_sha256,
        "sampling_policy_sha256":clip["sampling_policy_sha256"],
        "sample_time_ticks":ticks,
        "timebase_hz":1000,
        "interpolation":"LINEAR",
        "part_ids":part_ids,
        "node_count":validation["node_count"],
        "sampler_count":validation["sampler_count"],
        "channel_count":validation["channel_count"],
        "accessor_count_added":validation["accessor_count_added"],
        "buffer_view_count_added":validation["buffer_view_count_added"],
        "animation_validation_sha256":canonical_json_hash(&validation),
        "validator_status":"strict-rigid-gltf-animation-readback-pass",
        "hard_gate_passed":true,
        "source_static_projection_exact":true,
        "no_skinning":true,
        "no_morph_targets":true,
        "materialization_status":"runtime-owned-cas-animated-glb",
        "runtime_write_performed":true,
        "quality_status":"structural_only",
        "limitations":[
            "caller-authored-rigid-Part-rest-and-action-only",
            "linear-scheduled-samples-only-no-editable-timeline",
            "no-armature-bones-skinning-morph-targets-ik-constraints-nla-fcurves-or-drivers",
            "CAS-materialization-is-not-user-confirmation-or-external-export",
            "structural-animation-readback-does-not-prove-visual-quality-or-engine-roundtrip"
        ],
        "canonical_sha256":""
    });
    receipt["canonical_sha256"] = Value::String(canonical_json_hash(&receipt));
    validate_receipt(&receipt)?;
    let receipt_object = runtime.put_object(
        &canonical_json_bytes(&receipt).map_err(|source| error(source.to_string()))?,
        None,
        "application/json",
        "mechanical-animation-glb-receipt",
    )?;
    Ok(json!({
        "schema_version":"MechanicalAnimationGlbPrepareResult@1",
        "animated_artifact_sha256":artifact_object.record.sha256,
        "animated_artifact_size_bytes":artifact_object.record.size_bytes,
        "receipt_object_sha256":receipt_object.record.sha256,
        "receipt":receipt,
        "candidate_confirmed":false,
        "export_performed":false
    }))
}

/// Materialize the fixed six AnchorSet sockets into the LOD0 animated GLB.
///
/// This is deliberately kept beside the rigid-animation producer because the
/// animated GLB is the source of truth for channels/samplers/keyframes.  The
/// socket operation only appends empty transform nodes and attachment edges;
/// it never rewrites meshes, materials, accessors, bufferViews, animations or
/// BIN bytes.  All source validation is completed before the reservation is
/// opened, so malformed delivery, receipt, AnchorSet or GLB input is a
/// zero-write failure.
pub(super) fn animated_socket_prepare(
    runtime: &Runtime,
    request: &Value,
) -> Result<Value, RuntimeError> {
    let object = exact_object(
        request,
        &[
            "schema_version",
            "project_id",
            "delivery_manifest_object_sha256",
            "anchor_set_object_sha256",
            "source_candidate_id",
            "source_candidate_state_sha256",
            "source_animated_artifact_sha256",
            "source_animation_receipt_object_sha256",
            "materialization_policy",
            "canonical_sha256",
        ],
        ANIMATED_SOCKET_PREPARE_SCHEMA,
    )?;
    if text(object, "schema_version")? != ANIMATED_SOCKET_PREPARE_SCHEMA
        || text(object, "materialization_policy")? != ANIMATED_SOCKET_POLICY
    {
        return invalid("animated GLB socket materialization policy or LOD scope differs");
    }
    verify_request_canonical(request)?;
    let project_id = identifier(object, "project_id")?.to_owned();
    let candidate_id = identifier(object, "source_candidate_id")?.to_owned();
    let candidate_state_sha256 = sha(object, "source_candidate_state_sha256")?.to_owned();
    let delivery_sha256 = sha(object, "delivery_manifest_object_sha256")?.to_owned();
    let animated_sha256 = sha(object, "source_animated_artifact_sha256")?.to_owned();
    let animation_receipt_sha256 =
        sha(object, "source_animation_receipt_object_sha256")?.to_owned();
    let anchor_sha256 = sha(object, "anchor_set_object_sha256")?.to_owned();
    let socket_key_sha256 = sha(object, "canonical_sha256")?.to_owned();

    let node_encoding = game_asset_delivery::socket_node_id_encoding_value()?;
    let node_encoding_sha256 = node_encoding
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .filter(|value| forgecad_contracts::is_sha256(value))
        .ok_or_else(|| error("animated GLB socket node ID encoding hash is unavailable"))?;
    if let Some(existing) = runtime
        .store
        .get_game_weapon_animated_glb_socket_materialization_link(&socket_key_sha256)?
    {
        if existing.project_id != project_id
            || existing.candidate_id != candidate_id
            || existing.candidate_state_sha256 != candidate_state_sha256
            || existing.delivery_manifest_object_sha256 != delivery_sha256
            || existing.animated_artifact_sha256 != animated_sha256
            || existing.animation_receipt_object_sha256 != animation_receipt_sha256
            || existing.anchor_set_object_sha256 != anchor_sha256
            || existing.socket_materialization_policy != ANIMATED_SOCKET_POLICY
            || existing.lod_scope != ANIMATED_SOCKET_LOD_SCOPE
            || existing.socket_node_id_encoding_sha256 != node_encoding_sha256
            || existing.request_sha256 != socket_key_sha256
        {
            return invalid("animated GLB socket materialization key is bound to another request");
        }
        let value = animated_socket_get_by_key(runtime, &project_id, &socket_key_sha256)?;
        return Ok(json!({
            "schema_version":ANIMATED_SOCKET_PREPARE_RESULT_SCHEMA,
            "animated_socket_materialization_key_sha256":socket_key_sha256,
            "derived_animated_socket_artifact_sha256":value["derived_animated_socket_artifact_sha256"],
            "receipt_object_sha256":value["receipt_object_sha256"],
            "receipt":value["receipt"],
            "durable_link":value["link"],
            "runtime_write_performed":true,
            "candidate_confirmed":false,
            "export_performed":false,
            "actual_engine_roundtrip":false,
            "quality_status":"structural_only"
        }));
    }

    let delivery = game_asset_delivery::get(
        runtime,
        &json!({
            "schema_version":"GameAssetDeliveryGetRequest@1",
            "project_id":project_id,
            "delivery_manifest_object_sha256":delivery_sha256
        }),
    )?;
    let delivery_link = delivery
        .get("link")
        .and_then(Value::as_object)
        .ok_or_else(|| error("animated GLB socket delivery link is unavailable"))?;
    if delivery_link.get("project_id").and_then(Value::as_str) != Some(project_id.as_str())
        || delivery_link
            .get("delivery_manifest_object_sha256")
            .and_then(Value::as_str)
            != Some(delivery_sha256.as_str())
    {
        return invalid("animated GLB socket delivery project or manifest differs");
    }
    let levels = delivery
        .get("lod_receipt")
        .and_then(|value| value.get("levels"))
        .and_then(Value::as_array)
        .filter(|values| values.len() == 3)
        .ok_or_else(|| error("animated GLB socket delivery LOD receipt is incomplete"))?;
    let lod0 = levels
        .first()
        .ok_or_else(|| error("animated GLB socket delivery LOD0 is unavailable"))?;
    let lod0_sha256 = lod0
        .get("artifact_sha256")
        .and_then(Value::as_str)
        .filter(|value| forgecad_contracts::is_sha256(value))
        .ok_or_else(|| error("animated GLB socket delivery LOD0 artifact is invalid"))?
        .to_owned();
    let candidate = runtime
        .store
        .get_candidate(&candidate_id)?
        .ok_or_else(|| error("animated GLB socket candidate is unavailable"))?;
    if candidate.project_id != project_id
        || candidate.canonical_sha256 != candidate_state_sha256
        || candidate.prepared_object_sha256.as_deref() != Some(lod0_sha256.as_str())
        || candidate.manifest_hash.as_deref() != Some(lod0_sha256.as_str())
    {
        return invalid("animated GLB socket candidate/LOD0 binding differs");
    }
    if lod0.get("level").and_then(Value::as_u64) != Some(0)
        || lod0.get("candidate_id").and_then(Value::as_str) != Some(candidate_id.as_str())
        || lod0.get("candidate_state_sha256").and_then(Value::as_str)
            != Some(candidate_state_sha256.as_str())
        || lod0.get("artifact_sha256").and_then(Value::as_str) != Some(lod0_sha256.as_str())
    {
        return invalid("animated GLB socket delivery LOD0 binding differs");
    }
    if delivery_link
        .get("animation_artifact_sha256")
        .filter(|value| !value.is_null())
        .and_then(Value::as_str)
        .is_some_and(|value| value != animated_sha256)
    {
        return invalid("animated GLB socket delivery animation artifact differs");
    }

    let anchor_result = game_asset_delivery::weapon_anchor_get(
        runtime,
        &json!({
            "schema_version":"GameWeaponAnchorGetRequest@1",
            "project_id":project_id,
            "delivery_manifest_object_sha256":delivery_sha256
        }),
    )?;
    let anchor_link = anchor_result
        .get("link")
        .and_then(Value::as_object)
        .ok_or_else(|| error("animated GLB socket AnchorSet link is unavailable"))?;
    if anchor_link
        .get("anchor_set_object_sha256")
        .and_then(Value::as_str)
        != Some(anchor_sha256.as_str())
    {
        return invalid("animated GLB socket AnchorSet differs from the durable AnchorSet");
    }
    let anchor_set = anchor_result
        .get("anchor_set")
        .cloned()
        .ok_or_else(|| error("animated GLB socket AnchorSet is unavailable"))?;
    let anchor_set_canonical_sha256 = anchor_set
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .filter(|value| forgecad_contracts::is_sha256(value))
        .ok_or_else(|| error("animated GLB socket AnchorSet canonical hash is invalid"))?
        .to_owned();
    let anchor_ids = game_asset_delivery::socket_anchor_ids(&anchor_set)?;
    let part_ids = anchor_set
        .get("part_ids")
        .and_then(Value::as_array)
        .ok_or_else(|| error("animated GLB socket AnchorSet Part inventory is unavailable"))?
        .iter()
        .map(|value| {
            value
                .as_str()
                .filter(|value| valid_identifier(value))
                .map(str::to_owned)
                .ok_or_else(|| error("animated GLB socket AnchorSet Part ID is invalid"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if part_ids.is_empty() || part_ids.len() > 64 {
        return invalid("animated GLB socket AnchorSet Part inventory is outside the bound");
    }

    ensure_source_object(runtime, &lod0_sha256, false)?;
    ensure_source_object(runtime, &animated_sha256, true)?;
    ensure_json_object(
        runtime,
        &animation_receipt_sha256,
        MECHANICAL_ANIMATION_RECEIPT_SCHEMA,
        "mechanical-animation-glb-receipt",
    )?;
    ensure_json_object(
        runtime,
        &anchor_sha256,
        "GameWeaponAnchorSet@1",
        "game-weapon-anchor-set",
    )?;

    let animation_receipt = read_canonical_json(
        runtime,
        &animation_receipt_sha256,
        MECHANICAL_ANIMATION_RECEIPT_SCHEMA,
    )?;
    validate_mechanical_animation_source_receipt(
        &animation_receipt,
        &project_id,
        &candidate_id,
        &candidate_state_sha256,
        &lod0_sha256,
        &animated_sha256,
        &part_ids,
    )?;
    if animation_receipt
        .get("source_artifact_sha256")
        .and_then(Value::as_str)
        != Some(lod0_sha256.as_str())
    {
        return invalid("animated GLB socket animation receipt source artifact differs");
    }

    let source_glb = runtime.cas_read_bounded(&lod0_sha256, MAX_GLB_BYTES as u64)?;
    let animated_glb = runtime.cas_read_bounded(&animated_sha256, MAX_GLB_BYTES as u64)?;
    let mut inspection = inspect_animated_source(
        &source_glb,
        &animated_glb,
        &lod0_sha256,
        &animated_sha256,
        &animation_receipt,
        &part_ids,
    )?;
    let materialized = game_asset_delivery::materialize_socket_glb(
        &animated_glb,
        &animated_sha256,
        &anchor_sha256,
        &anchor_set,
        &part_ids,
        &anchor_ids,
    )?;
    if materialized.source_node_count != inspection.derived_node_count
        || materialized.derived_node_count != inspection.derived_node_count + anchor_ids.len()
    {
        return invalid("animated GLB socket source node count differs from animation readback");
    }
    let derived_sha256 = sha256_hex(&materialized.glb);
    validate_socket_animation_preservation(
        &animated_glb,
        &materialized.glb,
        &animated_sha256,
        &derived_sha256,
        &mut inspection,
    )?;
    let derived_readback_sha256 = animated_socket_readback_sha256(
        &socket_key_sha256,
        &project_id,
        &candidate_id,
        &candidate_state_sha256,
        &lod0_sha256,
        &animated_sha256,
        &anchor_sha256,
        &anchor_set_canonical_sha256,
        &materialized,
        &inspection,
        &derived_sha256,
    )?;
    let source_animation_validation_sha256 = animation_receipt
        .get("animation_validation_sha256")
        .and_then(Value::as_str)
        .filter(|value| forgecad_contracts::is_sha256(value))
        .ok_or_else(|| error("animated GLB socket source animation validation hash is invalid"))?
        .to_owned();
    let source_artifact_readback_sha256 = animation_receipt
        .get("artifact_readback_sha256")
        .and_then(Value::as_str)
        .filter(|value| forgecad_contracts::is_sha256(value))
        .ok_or_else(|| error("animated GLB socket source artifact readback hash is invalid"))?
        .to_owned();
    let receipt = animated_socket_receipt(
        &socket_key_sha256,
        &project_id,
        &candidate_id,
        &candidate_state_sha256,
        &delivery_sha256,
        &lod0_sha256,
        &animated_sha256,
        &animation_receipt_sha256,
        animation_receipt["canonical_sha256"]
            .as_str()
            .ok_or_else(|| error("animated GLB socket source receipt canonical hash is invalid"))?,
        &anchor_sha256,
        &anchor_set_canonical_sha256,
        &node_encoding_sha256,
        &materialized,
        &inspection,
        &derived_sha256,
        &derived_readback_sha256,
        &source_animation_validation_sha256,
        &source_artifact_readback_sha256,
        &source_animation_validation_sha256,
    )?;
    verify_animated_socket_receipt(&receipt)?;
    let receipt_bytes =
        canonical_json_bytes(&receipt).map_err(|source| error(source.to_string()))?;

    let reservation = runtime.store.begin_cas_reservation();
    let mut reserved_objects = Vec::<CasObject>::new();
    let operation = (|| -> Result<Value, RuntimeError> {
        let derived_object = runtime.store.put_object_reserved(
            &reservation,
            &materialized.glb,
            Some(&derived_sha256),
            "model/gltf-binary",
            ANIMATED_SOCKET_GLB_KIND,
            &now_string(),
        )?;
        reserved_objects.push(derived_object.clone());
        let receipt_object = runtime.store.put_object_reserved(
            &reservation,
            &receipt_bytes,
            None,
            "application/json",
            ANIMATED_SOCKET_RECEIPT_KIND,
            &now_string(),
        )?;
        reserved_objects.push(receipt_object.clone());

        let mut link = GameWeaponAnimatedGlbSocketMaterializationLinkRecord {
            schema_version: ANIMATED_SOCKET_LINK_SCHEMA.to_owned(),
            animated_socket_materialization_key_sha256: socket_key_sha256.clone(),
            project_id: project_id.clone(),
            candidate_id: candidate_id.clone(),
            candidate_state_sha256: candidate_state_sha256.clone(),
            delivery_manifest_object_sha256: delivery_sha256.clone(),
            lod0_artifact_sha256: lod0_sha256.clone(),
            source_artifact_sha256: lod0_sha256.clone(),
            source_artifact_readback_sha256: animation_receipt["artifact_readback_sha256"]
                .as_str()
                .ok_or_else(|| error("animated GLB socket source artifact readback is invalid"))?
                .to_owned(),
            animated_artifact_sha256: animated_sha256.clone(),
            animated_artifact_readback_sha256: source_animation_validation_sha256.clone(),
            animation_receipt_object_sha256: animation_receipt_sha256.clone(),
            animation_receipt_canonical_sha256: animation_receipt["canonical_sha256"]
                .as_str()
                .ok_or_else(|| {
                    error("animated GLB socket source receipt canonical hash is invalid")
                })?
                .to_owned(),
            anchor_set_object_sha256: anchor_sha256.clone(),
            anchor_set_canonical_sha256: anchor_set_canonical_sha256.clone(),
            request_sha256: socket_key_sha256.clone(),
            socket_materialization_policy: ANIMATED_SOCKET_POLICY.to_owned(),
            lod_scope: ANIMATED_SOCKET_LOD_SCOPE.to_owned(),
            socket_node_id_encoding_sha256: node_encoding_sha256.to_owned(),
            derived_animated_socket_artifact_sha256: derived_object.record.sha256.clone(),
            derived_animated_socket_artifact_readback_sha256: derived_readback_sha256.clone(),
            receipt_object_sha256: receipt_object.record.sha256.clone(),
            materialization_status: ANIMATED_SOCKET_STATUS.to_owned(),
            canonical_sha256: String::new(),
            created_at: now_string(),
        };
        link.canonical_sha256 = canonical_json_hash(
            &serde_json::to_value(&link).map_err(|source| error(source.to_string()))?,
        );
        let durable_link = runtime
            .store
            .record_game_weapon_animated_glb_socket_materialization_link(&link)?;
        Ok(json!({
            "schema_version":ANIMATED_SOCKET_PREPARE_RESULT_SCHEMA,
            "animated_socket_materialization_key_sha256":socket_key_sha256,
            "derived_animated_socket_artifact_sha256":derived_object.record.sha256,
            "receipt_object_sha256":receipt_object.record.sha256,
            "receipt":receipt,
            "durable_link":durable_link,
            "runtime_write_performed":true,
            "candidate_confirmed":false,
            "export_performed":false,
            "actual_engine_roundtrip":false,
            "quality_status":"structural_only"
        }))
    })();
    match operation {
        Ok(value) => {
            for object in &reserved_objects {
                runtime
                    .store
                    .release_cas_reservation_object(&reservation, object, false)?;
            }
            Ok(value)
        }
        Err(operation_error) => {
            let mut rollback_error = None;
            for object in reserved_objects.iter().rev() {
                if let Err(error) =
                    runtime
                        .store
                        .release_cas_reservation_object(&reservation, object, true)
                {
                    rollback_error = Some(error.to_string());
                }
            }
            if let Some(rollback_error) = rollback_error {
                return Err(error(format!(
                    "animated GLB socket materialization failed ({operation_error}); reservation rollback also failed ({rollback_error})"
                )));
            }
            Err(operation_error)
        }
    }
}

/// Compatibility aliases keep the Runtime wrapper naming independent from
/// this module's implementation name while exposing one closed operation.
pub(super) fn weapon_animated_glb_socket_prepare(
    runtime: &Runtime,
    request: &Value,
) -> Result<Value, RuntimeError> {
    animated_socket_prepare(runtime, request)
}

pub(super) fn animated_socket_get(
    runtime: &Runtime,
    request: &Value,
) -> Result<Value, RuntimeError> {
    let object = exact_object(
        request,
        &[
            "schema_version",
            "project_id",
            "animated_socket_materialization_key_sha256",
        ],
        ANIMATED_SOCKET_GET_SCHEMA,
    )?;
    if text(object, "schema_version")? != ANIMATED_SOCKET_GET_SCHEMA {
        return invalid("animated GLB socket get schema differs");
    }
    let project_id = identifier(object, "project_id")?.to_owned();
    let key = sha(object, "animated_socket_materialization_key_sha256")?.to_owned();
    animated_socket_get_by_key(runtime, &project_id, &key)
}

pub(super) fn weapon_animated_glb_socket_get(
    runtime: &Runtime,
    request: &Value,
) -> Result<Value, RuntimeError> {
    animated_socket_get(runtime, request)
}

/// Independently sample the already materialized animated socket GLB.  This
/// producer deliberately does not call `mechanical_animation_glb_prepare` or
/// `game_weapon_animated_glb_socket_prepare`: all source bytes and receipts
/// must exist before this operation can open its single projection reservation.
pub(super) fn game_weapon_animated_glb_socket_transform_projection_prepare(
    runtime: &Runtime,
    request: &Value,
) -> Result<Value, RuntimeError> {
    let request = parse_projection_prepare_request(request)?;
    let (projection, existing_object_sha256) =
        build_socket_transform_projection(runtime, &request)?;
    if let Some(existing_object_sha256) = existing_object_sha256 {
        return projection_result(
            &projection,
            &existing_object_sha256,
            true,
            SOCKET_TRANSFORM_PROJECTION_PREPARE_RESULT_SCHEMA,
            true,
        );
    }

    let projection_value = serde_json::to_value(&projection).map_err(|source| {
        error(format!(
            "socket transform projection serialization failed: {source}"
        ))
    })?;
    let projection_bytes = projection_canonical_object_bytes(&projection_value)?;
    if projection_bytes.len() > 1024 * 1024 {
        return invalid("socket transform projection exceeds one MiB");
    }
    let reservation = runtime.store.begin_cas_reservation();
    let projection_object = runtime.store.put_object_reserved(
        &reservation,
        &projection_bytes,
        None,
        "application/json",
        SOCKET_TRANSFORM_PROJECTION_KIND,
        &projection.created_at,
    )?;
    let commit = runtime
        .store
        .record_game_weapon_animated_glb_socket_transform_projection(
            &projection,
            &projection_object.record,
        );
    match commit {
        Ok(stored) => {
            runtime.store.release_cas_reservation_object(
                &reservation,
                &projection_object,
                false,
            )?;
            projection_result(
                &stored,
                &projection_object.record.sha256,
                false,
                SOCKET_TRANSFORM_PROJECTION_PREPARE_RESULT_SCHEMA,
                true,
            )
        }
        Err(commit_error) => {
            let rollback = runtime.store.release_cas_reservation_object(
                &reservation,
                &projection_object,
                true,
            );
            if let Err(rollback_error) = rollback {
                return Err(error(format!(
                    "socket transform projection commit failed ({commit_error}); reservation rollback failed ({rollback_error})"
                )));
            }
            Err(commit_error.into())
        }
    }
}

/// Read one durable projection and independently replay every source byte,
/// animation accessor and six-socket transform.  No reservation or Runtime
/// write is performed on this path.
pub(super) fn game_weapon_animated_glb_socket_transform_projection_get(
    runtime: &Runtime,
    request: &Value,
) -> Result<Value, RuntimeError> {
    let object = exact_object(
        request,
        SOCKET_TRANSFORM_PROJECTION_GET_FIELDS,
        SOCKET_TRANSFORM_PROJECTION_GET_SCHEMA,
    )?;
    if text(object, "schema_version")? != SOCKET_TRANSFORM_PROJECTION_GET_SCHEMA {
        return invalid("socket transform projection get schema differs");
    }
    let projection_key_sha256 = sha(object, "projection_key_sha256")?.to_owned();
    let project_id = identifier(object, "project_id")?.to_owned();
    let candidate_id = identifier(object, "candidate_id")?.to_owned();
    let stored = runtime
        .store
        .get_game_weapon_animated_glb_socket_transform_projection(&projection_key_sha256)?
        .ok_or_else(|| error("socket transform projection is unavailable"))?;
    let stored_value = serde_json::to_value(&stored).map_err(|source| {
        error(format!(
            "stored socket transform projection is invalid: {source}"
        ))
    })?;
    if stored.project_id != project_id
        || stored.candidate_id != candidate_id
        || stored.projection_key_sha256 != projection_key_sha256
    {
        return invalid("socket transform projection get scope differs");
    }
    let replay_request = projection_request_from_record(&stored_value)?;
    let (replayed, object_sha256) = build_socket_transform_projection(runtime, &replay_request)?;
    if !projection_replay_equivalent(
        &stored_value,
        &serde_json::to_value(&replayed).map_err(|source| error(source.to_string()))?,
    ) {
        return invalid("socket transform projection receipt is tampered");
    }
    let projection_object_sha256 = sha256_hex(&projection_canonical_object_bytes(&stored_value)?);
    if object_sha256.as_deref() != Some(projection_object_sha256.as_str()) {
        return invalid("socket transform projection CAS hash differs after replay");
    }
    projection_result(
        &replayed,
        &projection_object_sha256,
        true,
        SOCKET_TRANSFORM_PROJECTION_GET_RESULT_SCHEMA,
        false,
    )
}

fn parse_projection_prepare_request(value: &Value) -> Result<ProjectionRequest, RuntimeError> {
    let object = exact_object(
        value,
        SOCKET_TRANSFORM_PROJECTION_PREPARE_FIELDS,
        SOCKET_TRANSFORM_PROJECTION_PREPARE_SCHEMA,
    )?;
    if text(object, "schema_version")? != SOCKET_TRANSFORM_PROJECTION_PREPARE_SCHEMA {
        return invalid("socket transform projection prepare schema differs");
    }
    let request = ProjectionRequest {
        projection_key_sha256: sha(object, "projection_key_sha256")?.to_owned(),
        project_id: identifier(object, "project_id")?.to_owned(),
        candidate_id: identifier(object, "candidate_id")?.to_owned(),
        candidate_state_sha256: sha(object, "candidate_state_sha256")?.to_owned(),
        delivery_manifest_object_sha256: sha(object, "delivery_manifest_object_sha256")?.to_owned(),
        source_artifact_sha256: sha(object, "source_artifact_sha256")?.to_owned(),
        source_artifact_readback_sha256: sha(object, "source_artifact_readback_sha256")?.to_owned(),
        animated_artifact_sha256: sha(object, "animated_artifact_sha256")?.to_owned(),
        animated_artifact_readback_sha256: sha(object, "animated_artifact_readback_sha256")?
            .to_owned(),
        animation_receipt_object_sha256: sha(object, "animation_receipt_object_sha256")?.to_owned(),
        animation_receipt_canonical_sha256: sha(object, "animation_receipt_canonical_sha256")?
            .to_owned(),
        animated_socket_materialization_key_sha256: sha(
            object,
            "animated_socket_materialization_key_sha256",
        )?
        .to_owned(),
        derived_animated_socket_artifact_sha256: sha(
            object,
            "derived_animated_socket_artifact_sha256",
        )?
        .to_owned(),
        derived_animated_socket_artifact_readback_sha256: sha(
            object,
            "derived_animated_socket_artifact_readback_sha256",
        )?
        .to_owned(),
        derived_animated_socket_receipt_object_sha256: sha(
            object,
            "derived_animated_socket_receipt_object_sha256",
        )?
        .to_owned(),
        derived_animated_socket_receipt_canonical_sha256: sha(
            object,
            "derived_animated_socket_receipt_canonical_sha256",
        )?
        .to_owned(),
        anchor_set_object_sha256: sha(object, "anchor_set_object_sha256")?.to_owned(),
        anchor_set_canonical_sha256: sha(object, "anchor_set_canonical_sha256")?.to_owned(),
        animation_clip_id: identifier(object, "animation_clip_id")?.to_owned(),
        animation_clip_object_sha256: sha(object, "animation_clip_object_sha256")?.to_owned(),
        animation_clip_canonical_sha256: sha(object, "animation_clip_canonical_sha256")?.to_owned(),
        socket_node_id_encoding_sha256: sha(object, "socket_node_id_encoding_sha256")?.to_owned(),
        socket_node_inventory_sha256: sha(object, "socket_node_inventory_sha256")?.to_owned(),
        socket_roles_sha256: sha(object, "socket_roles_sha256")?.to_owned(),
        part_hierarchy_sha256: sha(object, "part_hierarchy_sha256")?.to_owned(),
        part_hierarchy_policy: text(object, "part_hierarchy_policy")?.to_owned(),
        transform_representation_policy: text(object, "transform_representation_policy")?
            .to_owned(),
        sample_schedule_sha256: sha(object, "sample_schedule_sha256")?.to_owned(),
        sample_time_ticks: parse_projection_ticks(object.get("sample_time_ticks"))?,
        frame_scope: text(object, "frame_scope")?.to_owned(),
        timebase_hz: object
            .get("timebase_hz")
            .and_then(Value::as_u64)
            .ok_or_else(|| error("timebase_hz is invalid"))?,
        transform_projection_policy: text(object, "transform_projection_policy")?.to_owned(),
        coordinate_system: text(object, "coordinate_system")?.to_owned(),
        transform_convention: text(object, "transform_convention")?.to_owned(),
        float_quantization_policy: text(object, "float_quantization_policy")?.to_owned(),
        input_sha256: sha(object, "input_sha256")?.to_owned(),
        _idempotency_key: identifier(object, "idempotency_key")?.to_owned(),
    };
    if request.sample_time_ticks.len()
        != object
            .get("sample_count")
            .and_then(Value::as_u64)
            .ok_or_else(|| error("sample_count is invalid"))? as usize
        || !(1..=16).contains(&request.sample_time_ticks.len())
        || request
            .sample_time_ticks
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        || request.timebase_hz != 1000
        || request.frame_scope != SOCKET_TRANSFORM_FRAME_SCOPE
        || request.part_hierarchy_policy != SOCKET_TRANSFORM_PART_HIERARCHY_POLICY
        || request.transform_representation_policy != SOCKET_TRANSFORM_REPRESENTATION_POLICY
        || request.transform_projection_policy != SOCKET_TRANSFORM_PROJECTION_POLICY
        || request.coordinate_system != SOCKET_TRANSFORM_COORDINATE_SYSTEM
        || request.transform_convention != SOCKET_TRANSFORM_CONVENTION
        || request.float_quantization_policy != SOCKET_TRANSFORM_FLOAT_POLICY
    {
        return invalid("socket transform projection policy or schedule differs");
    }
    let roles = object
        .get("socket_roles")
        .and_then(Value::as_array)
        .ok_or_else(|| error("socket_roles is invalid"))?;
    if roles.len() != SOCKET_TRANSFORM_ROLES.len()
        || roles
            .iter()
            .zip(SOCKET_TRANSFORM_ROLES)
            .any(|(value, expected)| value.as_str() != Some(expected))
    {
        return invalid("socket transform projection roles differ");
    }
    let mut preimage = Value::Object(object.clone());
    let preimage_object = preimage
        .as_object_mut()
        .ok_or_else(|| error("socket transform projection request is not an object"))?;
    preimage_object.remove("projection_key_sha256");
    preimage_object.remove("input_sha256");
    preimage_object.remove("idempotency_key");
    let expected_input_sha256 = canonical_json_hash(&preimage);
    if request.input_sha256 != expected_input_sha256
        || request.projection_key_sha256 != expected_input_sha256
    {
        return invalid("socket transform projection input/key hash differs");
    }
    let roles_hash = canonical_json_hash(object.get("socket_roles").unwrap());
    if request.socket_roles_sha256 != roles_hash {
        return invalid("socket transform projection role hash differs");
    }
    let schedule_hash = canonical_json_hash(&json!({
        "frame_scope": request.frame_scope,
        "sample_time_ticks": request.sample_time_ticks,
        "timebase_hz": request.timebase_hz,
    }));
    if request.sample_schedule_sha256 != schedule_hash {
        return invalid("socket transform projection sample schedule hash differs");
    }
    Ok(request)
}

fn parse_projection_ticks(value: Option<&Value>) -> Result<Vec<u64>, RuntimeError> {
    let values = value
        .and_then(Value::as_array)
        .filter(|values| (1..=16).contains(&values.len()))
        .ok_or_else(|| error("sample_time_ticks is invalid"))?;
    values
        .iter()
        .map(|value| {
            value
                .as_u64()
                .filter(|tick| *tick <= 1_000_000)
                .ok_or_else(|| error("sample_time_ticks contains an invalid tick"))
        })
        .collect()
}

fn projection_request_from_record(value: &Value) -> Result<ProjectionRequest, RuntimeError> {
    let object = value
        .as_object()
        .ok_or_else(|| error("stored socket transform projection is not an object"))?;
    let mut request = Map::new();
    for field in SOCKET_TRANSFORM_PROJECTION_PREPARE_FIELDS {
        if *field == "schema_version" {
            request.insert(
                (*field).to_owned(),
                Value::String(SOCKET_TRANSFORM_PROJECTION_PREPARE_SCHEMA.to_owned()),
            );
        } else if *field == "idempotency_key" {
            request.insert(
                (*field).to_owned(),
                Value::String(
                    object
                        .get("projection_key_sha256")
                        .and_then(Value::as_str)
                        .ok_or_else(|| error("stored projection key is missing"))?
                        .to_owned(),
                ),
            );
        } else if let Some(field_value) = object.get(*field) {
            request.insert((*field).to_owned(), field_value.clone());
        } else {
            return invalid(format!("stored projection field {field} is missing"));
        }
    }
    parse_projection_prepare_request(&Value::Object(request))
}

fn projection_result(
    projection: &forgecad_contracts::GameWeaponAnimatedGlbSocketTransformProjection,
    projection_object_sha256: &str,
    replayed: bool,
    schema_version: &str,
    runtime_write: bool,
) -> Result<Value, RuntimeError> {
    if !forgecad_contracts::is_sha256(projection_object_sha256) {
        return invalid("socket transform projection object hash is invalid");
    }
    let projection_value = serde_json::to_value(projection).map_err(|source| {
        error(format!(
            "socket transform projection serialization failed: {source}"
        ))
    })?;
    Ok(json!({
        "schema_version":schema_version,
        "projection_key_sha256":projection.projection_key_sha256,
        "projection_object_sha256":projection_object_sha256,
        "projection":projection_value,
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

fn projection_replay_equivalent(left: &Value, right: &Value) -> bool {
    fn normalize(value: &mut Value) {
        if let Some(object) = value.as_object_mut() {
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
    }
    let mut left = left.clone();
    let mut right = right.clone();
    normalize(&mut left);
    normalize(&mut right);
    left == right
}

/// Widened f32 samples can initially serialize with a longer f64 decimal
/// spelling. Reparse once so the CAS form uses serde_json's shortest stable
/// f64 spelling, then prove a second replay is byte-exact for Store/restart.
fn projection_canonical_object_bytes(value: &Value) -> Result<Vec<u8>, RuntimeError> {
    let first = canonical_json_bytes(value).map_err(|source| {
        error(format!(
            "socket transform projection JSON is invalid: {source}"
        ))
    })?;
    let reparsed: Value = serde_json::from_slice(&first).map_err(|source| {
        error(format!(
            "socket transform projection canonical JSON cannot be reparsed: {source}"
        ))
    })?;
    let stable = canonical_json_bytes(&reparsed).map_err(|source| {
        error(format!(
            "socket transform projection canonical replay failed: {source}"
        ))
    })?;
    let replayed: Value = serde_json::from_slice(&stable).map_err(|source| {
        error(format!(
            "socket transform projection stable JSON cannot be reparsed: {source}"
        ))
    })?;
    if canonical_json_bytes(&replayed).map_err(|source| error(source.to_string()))? != stable {
        return invalid("socket transform projection canonical JSON is not round-trip stable");
    }
    Ok(stable)
}

fn build_socket_transform_projection(
    runtime: &Runtime,
    request: &ProjectionRequest,
) -> Result<
    (
        forgecad_contracts::GameWeaponAnimatedGlbSocketTransformProjection,
        Option<String>,
    ),
    RuntimeError,
> {
    let existing = runtime
        .store
        .get_game_weapon_animated_glb_socket_transform_projection(&request.projection_key_sha256)?;

    let candidate = runtime
        .store
        .get_candidate(&request.candidate_id)?
        .ok_or_else(|| error("socket transform projection candidate is unavailable"))?;
    if candidate.project_id != request.project_id
        || candidate.canonical_sha256 != request.candidate_state_sha256
        || candidate.prepared_object_sha256.as_deref()
            != Some(request.source_artifact_sha256.as_str())
        || candidate.manifest_hash.as_deref() != Some(request.source_artifact_sha256.as_str())
    {
        return invalid("socket transform projection candidate/source binding differs");
    }

    let delivery = game_asset_delivery::get(
        runtime,
        &json!({
            "schema_version":"GameAssetDeliveryGetRequest@1",
            "project_id":request.project_id,
            "delivery_manifest_object_sha256":request.delivery_manifest_object_sha256
        }),
    )?;
    let delivery_link = delivery
        .get("link")
        .and_then(Value::as_object)
        .ok_or_else(|| error("socket transform projection delivery link is unavailable"))?;
    if delivery_link.get("project_id").and_then(Value::as_str) != Some(request.project_id.as_str())
        || delivery_link
            .get("delivery_manifest_object_sha256")
            .and_then(Value::as_str)
            != Some(request.delivery_manifest_object_sha256.as_str())
    {
        return invalid("socket transform projection delivery scope differs");
    }
    let levels = delivery
        .get("lod_receipt")
        .and_then(|value| value.get("levels"))
        .and_then(Value::as_array)
        .filter(|levels| levels.len() == 3)
        .ok_or_else(|| error("socket transform projection delivery LOD receipt is incomplete"))?;
    let lod0 = levels
        .first()
        .ok_or_else(|| error("socket transform projection delivery LOD0 is unavailable"))?;
    for (field, expected) in [
        ("candidate_id", request.candidate_id.as_str()),
        (
            "candidate_state_sha256",
            request.candidate_state_sha256.as_str(),
        ),
        ("artifact_sha256", request.source_artifact_sha256.as_str()),
        (
            "artifact_readback_sha256",
            request.source_artifact_readback_sha256.as_str(),
        ),
    ] {
        if lod0.get(field).and_then(Value::as_str) != Some(expected) {
            return invalid(format!(
                "socket transform projection delivery LOD0 {field} differs"
            ));
        }
    }
    if lod0.get("level").and_then(Value::as_u64) != Some(0) {
        return invalid("socket transform projection delivery LOD0 level differs");
    }

    let anchor_result = game_asset_delivery::weapon_anchor_get(
        runtime,
        &json!({
            "schema_version":"GameWeaponAnchorGetRequest@1",
            "project_id":request.project_id,
            "delivery_manifest_object_sha256":request.delivery_manifest_object_sha256
        }),
    )?;
    let anchor_link = anchor_result
        .get("link")
        .and_then(Value::as_object)
        .ok_or_else(|| error("socket transform projection AnchorSet link is unavailable"))?;
    if anchor_link
        .get("anchor_set_object_sha256")
        .and_then(Value::as_str)
        != Some(request.anchor_set_object_sha256.as_str())
    {
        return invalid("socket transform projection AnchorSet object differs");
    }
    let anchor_set = anchor_result
        .get("anchor_set")
        .cloned()
        .ok_or_else(|| error("socket transform projection AnchorSet is unavailable"))?;
    if anchor_set.get("canonical_sha256").and_then(Value::as_str)
        != Some(request.anchor_set_canonical_sha256.as_str())
    {
        return invalid("socket transform projection AnchorSet canonical hash differs");
    }
    let anchor_ids = game_asset_delivery::socket_anchor_ids(&anchor_set)?;
    if anchor_ids.len() != SOCKET_TRANSFORM_ROLES.len() {
        return invalid("socket transform projection AnchorSet role count differs");
    }
    let part_ids = anchor_set
        .get("part_ids")
        .and_then(Value::as_array)
        .filter(|part_ids| !part_ids.is_empty() && part_ids.len() <= 64)
        .ok_or_else(|| {
            error("socket transform projection AnchorSet Part inventory is unavailable")
        })?
        .iter()
        .map(|value| {
            value
                .as_str()
                .filter(|value| valid_identifier(value))
                .map(str::to_owned)
                .ok_or_else(|| error("socket transform projection AnchorSet Part ID is invalid"))
        })
        .collect::<Result<Vec<_>, _>>()?;

    let socket_link = runtime
        .store
        .get_game_weapon_animated_glb_socket_materialization_link(
            &request.animated_socket_materialization_key_sha256,
        )?
        .ok_or_else(|| error("socket transform projection animated socket link is unavailable"))?;
    if socket_link.project_id != request.project_id
        || socket_link.candidate_id != request.candidate_id
        || socket_link.candidate_state_sha256 != request.candidate_state_sha256
        || socket_link.delivery_manifest_object_sha256 != request.delivery_manifest_object_sha256
        || socket_link.lod0_artifact_sha256 != request.source_artifact_sha256
        || socket_link.source_artifact_sha256 != request.source_artifact_sha256
        || socket_link.animated_artifact_sha256 != request.animated_artifact_sha256
        || socket_link.animation_receipt_object_sha256 != request.animation_receipt_object_sha256
        || socket_link.anchor_set_object_sha256 != request.anchor_set_object_sha256
        || socket_link.anchor_set_canonical_sha256 != request.anchor_set_canonical_sha256
        || socket_link.derived_animated_socket_artifact_sha256
            != request.derived_animated_socket_artifact_sha256
        || socket_link.derived_animated_socket_artifact_readback_sha256
            != request.derived_animated_socket_artifact_readback_sha256
        || socket_link.receipt_object_sha256
            != request.derived_animated_socket_receipt_object_sha256
    {
        return invalid("socket transform projection animated socket link binding differs");
    }
    let animated_socket_result = weapon_animated_glb_socket_get(
        runtime,
        &json!({
            "schema_version":ANIMATED_SOCKET_GET_SCHEMA,
            "project_id":request.project_id,
            "animated_socket_materialization_key_sha256":request.animated_socket_materialization_key_sha256
        }),
    )?;
    let socket_receipt = animated_socket_result.get("receipt").ok_or_else(|| {
        error("socket transform projection animated socket receipt is unavailable")
    })?;
    let socket_receipt_object = socket_receipt
        .as_object()
        .ok_or_else(|| error("socket transform projection animated socket receipt is invalid"))?;
    if socket_receipt_object
        .get("canonical_sha256")
        .and_then(Value::as_str)
        != Some(
            request
                .derived_animated_socket_receipt_canonical_sha256
                .as_str(),
        )
        || socket_receipt_object
            .get("socket_node_inventory_sha256")
            .and_then(Value::as_str)
            != Some(request.socket_node_inventory_sha256.as_str())
        || socket_receipt_object
            .get("socket_node_id_encoding_sha256")
            .and_then(Value::as_str)
            != Some(request.socket_node_id_encoding_sha256.as_str())
        || socket_receipt_object
            .get("animation_receipt_canonical_sha256")
            .and_then(Value::as_str)
            != Some(request.animation_receipt_canonical_sha256.as_str())
    {
        return invalid("socket transform projection animated socket receipt binding differs");
    }
    let derived_socket_receipt = read_canonical_json(
        runtime,
        &request.derived_animated_socket_receipt_object_sha256,
        ANIMATED_SOCKET_RECEIPT_SCHEMA,
    )?;
    if derived_socket_receipt != *socket_receipt {
        return invalid("socket transform projection animated socket receipt replay differs");
    }

    let animation_receipt = read_canonical_json(
        runtime,
        &request.animation_receipt_object_sha256,
        MECHANICAL_ANIMATION_RECEIPT_SCHEMA,
    )?;
    validate_mechanical_animation_source_receipt(
        &animation_receipt,
        &request.project_id,
        &request.candidate_id,
        &request.candidate_state_sha256,
        &request.source_artifact_sha256,
        &request.animated_artifact_sha256,
        &part_ids,
    )?;
    if animation_receipt
        .get("canonical_sha256")
        .and_then(Value::as_str)
        != Some(request.animation_receipt_canonical_sha256.as_str())
        || animation_receipt
            .get("artifact_readback_sha256")
            .and_then(Value::as_str)
            != Some(request.source_artifact_readback_sha256.as_str())
        || animation_receipt
            .get("animated_artifact_sha256")
            .and_then(Value::as_str)
            != Some(request.animated_artifact_sha256.as_str())
        || animation_receipt
            .get("animation_validation_sha256")
            .and_then(Value::as_str)
            != Some(request.animated_artifact_readback_sha256.as_str())
        || animation_receipt.get("clip_id").and_then(Value::as_str)
            != Some(request.animation_clip_id.as_str())
        || animation_receipt
            .get("clip_object_sha256")
            .and_then(Value::as_str)
            != Some(request.animation_clip_object_sha256.as_str())
        || animation_receipt.get("clip_sha256").and_then(Value::as_str)
            != Some(request.animation_clip_canonical_sha256.as_str())
    {
        return invalid("socket transform projection animation receipt binding differs");
    }
    let animation_receipt_ticks = animation_receipt
        .get("sample_time_ticks")
        .and_then(Value::as_array)
        .ok_or_else(|| error("socket transform projection animation receipt schedule is missing"))?
        .iter()
        .map(|value| {
            value.as_u64().ok_or_else(|| {
                error("socket transform projection animation receipt tick is invalid")
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    let clip_request = {
        let mut value = json!({
            "schema_version":"MechanicalAnimationClipGetRequest@1",
            "project_id":request.project_id,
            "candidate_id":request.candidate_id,
            "clip_id":request.animation_clip_id,
            "canonical_sha256":""
        });
        let mut preimage = value.clone();
        preimage
            .as_object_mut()
            .expect("clip lookup request is an object")
            .remove("canonical_sha256");
        let canonical = canonical_json_hash(&preimage);
        value["canonical_sha256"] = Value::String(canonical);
        value
    };
    let clip_result = mechanical_pose::animation_clip_get(runtime, &clip_request)?;
    let clip = clip_result
        .get("clip")
        .cloned()
        .ok_or_else(|| error("socket transform projection clip is unavailable"))?;
    let clip_link = clip_result
        .as_object()
        .ok_or_else(|| error("socket transform projection clip link is unavailable"))?;
    if clip.get("clip_id").and_then(Value::as_str) != Some(request.animation_clip_id.as_str())
        || clip.get("canonical_sha256").and_then(Value::as_str)
            != Some(request.animation_clip_canonical_sha256.as_str())
        || clip_link.get("clip_object_sha256").and_then(Value::as_str)
            != Some(request.animation_clip_object_sha256.as_str())
        || clip.get("artifact_id").and_then(Value::as_str)
            != Some(request.source_artifact_sha256.as_str())
    {
        return invalid("socket transform projection clip binding differs");
    }
    let clip_ticks = clip
        .get("sampling_policy")
        .and_then(|value| value.get("sample_time_ticks"))
        .and_then(Value::as_array)
        .ok_or_else(|| error("socket transform projection clip schedule is missing"))?;
    let clip_ticks = clip_ticks
        .iter()
        .map(|value| {
            value
                .as_u64()
                .ok_or_else(|| error("socket transform projection clip tick is invalid"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if clip_ticks != animation_receipt_ticks {
        return invalid("socket transform projection clip and GLB schedule differ");
    }

    ensure_source_object(runtime, &request.source_artifact_sha256, false)?;
    ensure_source_object(runtime, &request.animated_artifact_sha256, true)?;
    let derived_object = runtime
        .store
        .get_object(&request.derived_animated_socket_artifact_sha256)?
        .ok_or_else(|| {
            error("socket transform projection derived socket CAS object is unavailable")
        })?;
    if derived_object.size_bytes == 0
        || derived_object.size_bytes > MAX_GLB_BYTES as u64
        || derived_object.mime != "model/gltf-binary"
        || derived_object.kind != ANIMATED_SOCKET_GLB_KIND
    {
        return invalid("socket transform projection derived socket CAS metadata differs");
    }
    ensure_json_object(
        runtime,
        &request.animation_receipt_object_sha256,
        MECHANICAL_ANIMATION_RECEIPT_SCHEMA,
        MECHANICAL_ANIMATION_RECEIPT_KIND,
    )?;
    ensure_json_object(
        runtime,
        &request.derived_animated_socket_receipt_object_sha256,
        ANIMATED_SOCKET_RECEIPT_SCHEMA,
        ANIMATED_SOCKET_RECEIPT_KIND,
    )?;

    let source_glb =
        runtime.cas_read_bounded(&request.source_artifact_sha256, MAX_GLB_BYTES as u64)?;
    let animated_glb =
        runtime.cas_read_bounded(&request.animated_artifact_sha256, MAX_GLB_BYTES as u64)?;
    let derived_glb = runtime.cas_read_bounded(
        &request.derived_animated_socket_artifact_sha256,
        MAX_GLB_BYTES as u64,
    )?;
    if sha256_hex(&source_glb) != request.source_artifact_sha256
        || sha256_hex(&animated_glb) != request.animated_artifact_sha256
        || sha256_hex(&derived_glb) != request.derived_animated_socket_artifact_sha256
    {
        return invalid("socket transform projection source GLB bytes differ");
    }
    let mut inspection = inspect_animated_source(
        &source_glb,
        &animated_glb,
        &request.source_artifact_sha256,
        &request.animated_artifact_sha256,
        &animation_receipt,
        &part_ids,
    )?;
    validate_socket_animation_preservation(
        &animated_glb,
        &derived_glb,
        &request.animated_artifact_sha256,
        &request.derived_animated_socket_artifact_sha256,
        &mut inspection,
    )?;
    if request
        .sample_time_ticks
        .iter()
        .any(|tick| *tick > 1_000_000)
    {
        return invalid("socket transform projection sample tick exceeds timebase bound");
    }
    let (source_root, source_binary) = parse_glb(&source_glb)?;
    let (animated_root, animated_binary) = parse_glb(&animated_glb)?;
    let (derived_root, derived_binary) = parse_glb(&derived_glb)?;
    let source_node_part_ids = projection_part_ids_in_node_order(&source_root, &part_ids)?;
    validate_projection_flat_source_root(&source_root, &source_node_part_ids)?;
    validate_projection_flat_source_root(&animated_root, &source_node_part_ids)?;
    let socket_nodes = validate_projection_socket_nodes(
        &derived_root,
        &source_node_part_ids,
        &anchor_set,
        &anchor_ids,
        source_root
            .get("nodes")
            .and_then(Value::as_array)
            .ok_or_else(|| error("socket transform projection source nodes are missing"))?
            .len(),
    )?;
    if animated_binary != derived_binary
        || animated_binary.get(..source_binary.len()) != Some(source_binary.as_slice())
        || request.socket_node_inventory_sha256
            != projection_socket_inventory_hash(
                &request.anchor_set_object_sha256,
                &anchor_set,
                &anchor_ids,
                part_ids.len(),
            )?
    {
        return invalid("socket transform projection renderable/BIN or socket inventory differs");
    }
    let animation = parse_projection_animation(
        &animated_root,
        &derived_root,
        &animated_binary,
        &inspection.sample_time_ticks,
        &source_node_part_ids,
    )?;
    let part_hierarchy_sha256 =
        projection_part_hierarchy_hash(&source_root, &source_node_part_ids)?;
    if request.part_hierarchy_sha256 != part_hierarchy_sha256 {
        return invalid("socket transform projection Part hierarchy hash differs");
    }
    let mut frames = Vec::with_capacity(request.sample_time_ticks.len());
    for (frame_index, sample_time_ticks) in request.sample_time_ticks.iter().enumerate() {
        frames.push(build_projection_frame(
            &request.projection_key_sha256,
            frame_index as u64,
            *sample_time_ticks,
            &animation,
            &socket_nodes,
            &source_node_part_ids,
            &request.animated_artifact_sha256,
            &request.derived_animated_socket_artifact_sha256,
        )?);
    }
    let mut projection_value = build_projection_value(
        request,
        &animation_receipt,
        &inspection,
        &animation,
        &part_hierarchy_sha256,
        &frames,
    )?;
    projection_value["canonical_sha256"] = Value::String(String::new());
    let projection_canonical_sha256 = canonical_json_hash(&projection_value);
    projection_value["canonical_sha256"] = Value::String(projection_canonical_sha256);
    let projection: forgecad_contracts::GameWeaponAnimatedGlbSocketTransformProjection =
        serde_json::from_value(projection_value).map_err(|source| {
            error(format!(
                "socket transform projection contract is invalid: {source}"
            ))
        })?;

    if let Some(existing) = existing {
        let existing_value = serde_json::to_value(&existing).map_err(|source| {
            error(format!(
                "stored socket transform projection is invalid: {source}"
            ))
        })?;
        if !projection_replay_equivalent(
            &existing_value,
            &serde_json::to_value(&projection).map_err(|source| error(source.to_string()))?,
        ) {
            return Err(RuntimeError::InvalidInput(
                "GAME_WEAPON_ANIMATED_GLB_SOCKET_TRANSFORM_PROJECTION_CONFLICT".to_owned(),
            ));
        }
        let existing_bytes = projection_canonical_object_bytes(&existing_value)?;
        let object_sha256 = sha256_hex(&existing_bytes);
        let bytes = runtime.cas_read_bounded(&object_sha256, 1024 * 1024)?;
        if sha256_hex(&bytes) != object_sha256 || existing_bytes != bytes {
            return invalid("socket transform projection stored CAS bytes differ");
        }
        return Ok((existing, Some(object_sha256)));
    }
    Ok((projection, None))
}

pub(super) fn projection_part_ids_in_node_order(
    source_root: &Value,
    anchor_part_ids: &[String],
) -> Result<Vec<String>, RuntimeError> {
    let nodes = source_root
        .get("nodes")
        .and_then(Value::as_array)
        .filter(|nodes| nodes.len() == anchor_part_ids.len() && !nodes.is_empty())
        .ok_or_else(|| error("socket transform projection source node count differs"))?;
    let node_part_ids = nodes
        .iter()
        .map(|node| {
            node.get("name")
                .and_then(Value::as_str)
                .filter(|name| valid_identifier(name))
                .map(str::to_owned)
                .ok_or_else(|| error("socket transform projection source Part name is invalid"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let node_part_set = node_part_ids.iter().cloned().collect::<BTreeSet<_>>();
    let anchor_part_set = anchor_part_ids.iter().cloned().collect::<BTreeSet<_>>();
    if node_part_set.len() != node_part_ids.len()
        || anchor_part_set.len() != anchor_part_ids.len()
        || node_part_set != anchor_part_set
    {
        return invalid("socket transform projection source/AnchorSet Part inventory differs");
    }
    Ok(node_part_ids)
}

pub(super) fn validate_projection_flat_source_root(
    root: &Value,
    part_ids: &[String],
) -> Result<(), RuntimeError> {
    let scenes = root
        .get("scenes")
        .and_then(Value::as_array)
        .filter(|scenes| scenes.len() == 1)
        .ok_or_else(|| error("socket transform projection source scenes are invalid"))?;
    if root.get("scene").and_then(Value::as_u64) != Some(0) {
        return invalid("socket transform projection source scene is not scene zero");
    }
    let nodes = root
        .get("nodes")
        .and_then(Value::as_array)
        .filter(|nodes| nodes.len() == part_ids.len() && !nodes.is_empty())
        .ok_or_else(|| error("socket transform projection source node count differs"))?;
    let scene_nodes = scenes[0]
        .get("nodes")
        .and_then(Value::as_array)
        .ok_or_else(|| error("socket transform projection source scene nodes are unavailable"))?;
    if scene_nodes.len() != nodes.len()
        || scene_nodes
            .iter()
            .enumerate()
            .any(|(index, value)| value.as_u64() != Some(index as u64))
    {
        return invalid("socket transform projection requires flat scene-root Parts");
    }
    for (index, (node, part_id)) in nodes.iter().zip(part_ids).enumerate() {
        let object = node
            .as_object()
            .ok_or_else(|| error("socket transform projection source node is invalid"))?;
        if object.get("name").and_then(Value::as_str) != Some(part_id.as_str())
            || object.get("children").is_some()
        {
            return invalid("socket transform projection Part hierarchy is not flat");
        }
        validate_projection_identity_node_transform(object, index)?;
    }
    Ok(())
}

fn validate_projection_identity_node_transform(
    object: &Map<String, Value>,
    node_index: usize,
) -> Result<(), RuntimeError> {
    if object.contains_key("matrix") {
        return invalid(format!(
            "socket transform projection node {node_index} contains matrix transform"
        ));
    }
    if let Some(value) = object.get("translation") {
        let translation = projection_f32_vector(value, 3, 1000.0, "translation")?;
        if translation != [0.0, 0.0, 0.0] {
            return invalid(format!(
                "socket transform projection node {node_index} has non-identity rest translation"
            ));
        }
    }
    if let Some(value) = object.get("rotation") {
        let rotation = projection_f32_vector(value, 4, 1.0, "rotation")?;
        let rotation = projection_normalize_quaternion(rotation)?;
        if rotation != [0.0, 0.0, 0.0, 1.0] {
            return invalid(format!(
                "socket transform projection node {node_index} has non-identity rest rotation"
            ));
        }
    }
    if let Some(value) = object.get("scale") {
        let scale = projection_f32_vector(value, 3, 1.0, "scale")?;
        if scale != [1.0, 1.0, 1.0] {
            return invalid(format!(
                "socket transform projection node {node_index} has non-identity rest scale"
            ));
        }
    }
    Ok(())
}

pub(super) fn validate_projection_socket_nodes(
    root: &Value,
    part_ids: &[String],
    anchor_set: &Value,
    anchor_ids: &[String],
    source_node_count: usize,
) -> Result<Vec<ProjectionSocketNode>, RuntimeError> {
    let nodes = root
        .get("nodes")
        .and_then(Value::as_array)
        .filter(|nodes| nodes.len() == source_node_count + anchor_ids.len())
        .ok_or_else(|| error("socket transform projection derived node count differs"))?;
    let source_nodes = nodes
        .get(..source_node_count)
        .ok_or_else(|| error("socket transform projection derived source nodes are unavailable"))?;
    for (index, (node, part_id)) in source_nodes.iter().zip(part_ids).enumerate() {
        let object = node
            .as_object()
            .ok_or_else(|| error("socket transform projection derived Part node is invalid"))?;
        if object.get("name").and_then(Value::as_str) != Some(part_id.as_str()) {
            return invalid("socket transform projection derived Part node order differs");
        }
        if object
            .get("children")
            .and_then(Value::as_array)
            .is_some_and(|children| {
                children.iter().any(|value| {
                    value
                        .as_u64()
                        .is_some_and(|child| child < source_node_count as u64)
                })
            })
        {
            return invalid("socket transform projection derived Part hierarchy is nested");
        }
        validate_projection_identity_node_transform(object, index)?;
    }
    let anchors = anchor_set
        .get("anchors")
        .and_then(Value::as_array)
        .filter(|anchors| anchors.len() == anchor_ids.len())
        .ok_or_else(|| error("socket transform projection AnchorSet anchors are unavailable"))?;
    let by_id = anchors
        .iter()
        .map(|anchor| {
            let id = anchor
                .get("anchor_id")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    error("socket transform projection AnchorSet anchor ID is invalid")
                })?;
            Ok((id.to_owned(), anchor))
        })
        .collect::<Result<BTreeMap<_, _>, RuntimeError>>()?;
    let mut result = Vec::with_capacity(anchor_ids.len());
    for (offset, anchor_id) in anchor_ids.iter().enumerate() {
        let anchor = by_id
            .get(anchor_id)
            .copied()
            .ok_or_else(|| error("socket transform projection AnchorSet anchor is missing"))?;
        let anchor_object = anchor
            .as_object()
            .ok_or_else(|| error("socket transform projection AnchorSet anchor is invalid"))?;
        let role = anchor_object
            .get("role")
            .and_then(Value::as_str)
            .ok_or_else(|| error("socket transform projection AnchorSet role is invalid"))?;
        if role != SOCKET_TRANSFORM_ROLES[offset] {
            return invalid("socket transform projection AnchorSet role order differs");
        }
        let node_index = source_node_count + offset;
        let node = nodes
            .get(node_index)
            .and_then(Value::as_object)
            .ok_or_else(|| {
                error("socket transform projection derived socket node is unavailable")
            })?;
        let expected_name = format!("forgecad-anchor-{anchor_id}");
        if node.get("name").and_then(Value::as_str) != Some(expected_name.as_str()) {
            return invalid("socket transform projection socket node name differs");
        }
        if node.contains_key("mesh")
            || node.contains_key("matrix")
            || node
                .get("children")
                .and_then(Value::as_array)
                .is_some_and(|children| !children.is_empty())
        {
            return invalid(
                "socket transform projection socket node is renderable or hierarchical",
            );
        }
        let local = projection_pose_from_node(node)?;
        let expected_local = projection_pose_from_anchor(anchor_object)?;
        if local != expected_local {
            return invalid("socket transform projection socket local TRS differs from AnchorSet");
        }
        let parent_kind = anchor_object
            .get("parent_kind")
            .and_then(Value::as_str)
            .ok_or_else(|| error("socket transform projection socket parent kind is invalid"))?;
        let owner_part_id = anchor_object
            .get("owner_part_id")
            .filter(|value| !value.is_null())
            .and_then(Value::as_str)
            .map(str::to_owned);
        let (parent_node_index, parent_node_name) = match parent_kind {
            "synthetic-scene-root" => {
                if owner_part_id.is_some()
                    || anchor_id != "weapon-root"
                    || !root
                        .get("scenes")
                        .and_then(Value::as_array)
                        .and_then(|scenes| scenes.first())
                        .and_then(|scene| scene.get("nodes"))
                        .and_then(Value::as_array)
                        .is_some_and(|roots| {
                            roots
                                .iter()
                                .any(|value| value.as_u64() == Some(node_index as u64))
                        })
                {
                    return invalid("socket transform projection synthetic root binding differs");
                }
                (-1, None)
            }
            "part-node" => {
                let owner_part_id = owner_part_id
                    .as_deref()
                    .filter(|owner| part_ids.iter().any(|part| part == owner))
                    .ok_or_else(|| {
                        error("socket transform projection socket owner Part is invalid")
                    })?;
                let owner_index = part_ids
                    .iter()
                    .position(|part| part == owner_part_id)
                    .ok_or_else(|| {
                        error("socket transform projection socket owner Part index is missing")
                    })?;
                let owner_children = nodes
                    .get(owner_index)
                    .and_then(Value::as_object)
                    .and_then(|node| node.get("children"))
                    .and_then(Value::as_array)
                    .ok_or_else(|| {
                        error("socket transform projection socket owner children are missing")
                    })?;
                if !owner_children
                    .iter()
                    .any(|value| value.as_u64() == Some(node_index as u64))
                {
                    return invalid(
                        "socket transform projection socket owner child binding differs",
                    );
                }
                (owner_index as isize, Some(owner_part_id.to_owned()))
            }
            _ => {
                return invalid(
                    "socket transform projection socket parent kind is outside the closed set",
                )
            }
        };
        result.push(ProjectionSocketNode {
            socket_node_id: anchor_id.clone(),
            anchor_id: anchor_id.clone(),
            role: role.to_owned(),
            node_index,
            parent_node_index,
            node_name: expected_name,
            parent_node_name,
            parent_kind: parent_kind.to_owned(),
            owner_part_id,
            local,
        });
    }
    Ok(result)
}

fn projection_pose_from_node(object: &Map<String, Value>) -> Result<ProjectionPose, RuntimeError> {
    let translation = projection_f32_array::<3>(
        object.get("translation").unwrap_or(&Value::Array(vec![
            Value::from(0.0),
            Value::from(0.0),
            Value::from(0.0),
        ])),
        1000.0,
        "socket node translation",
    )?;
    let rotation = projection_f32_vector(
        object.get("rotation").unwrap_or(&Value::Array(vec![
            Value::from(0.0),
            Value::from(0.0),
            Value::from(0.0),
            Value::from(1.0),
        ])),
        4,
        1.0,
        "socket node rotation",
    )?;
    let rotation = projection_normalize_quaternion(rotation)?;
    let default_scale = Value::Array(vec![Value::from(1.0), Value::from(1.0), Value::from(1.0)]);
    let scale = object.get("scale").unwrap_or(&default_scale);
    if projection_f32_array::<3>(scale, 1.0, "socket node scale")? != [1.0, 1.0, 1.0] {
        return invalid("socket transform projection socket scale is not identity");
    }
    Ok(ProjectionPose {
        translation,
        rotation,
    })
}

fn projection_pose_from_anchor(
    object: &Map<String, Value>,
) -> Result<ProjectionPose, RuntimeError> {
    let translation = projection_f32_array::<3>(
        object
            .get("local_translation_m")
            .ok_or_else(|| error("AnchorSet local translation is missing"))?,
        1000.0,
        "AnchorSet local translation",
    )?;
    let rotation = projection_f32_vector(
        object
            .get("local_rotation_quat_xyzw")
            .ok_or_else(|| error("AnchorSet local rotation is missing"))?,
        4,
        1.0,
        "AnchorSet local rotation",
    )?;
    let rotation = projection_normalize_quaternion(rotation)?;
    if projection_f32_array::<3>(
        object
            .get("local_scale_xyz")
            .ok_or_else(|| error("AnchorSet local scale is missing"))?,
        1.0,
        "AnchorSet local scale",
    )? != [1.0, 1.0, 1.0]
    {
        return invalid("socket transform projection AnchorSet scale is not identity");
    }
    Ok(ProjectionPose {
        translation,
        rotation,
    })
}

fn projection_f32_vector(
    value: &Value,
    length: usize,
    limit: f32,
    field: &str,
) -> Result<Vec<f32>, RuntimeError> {
    let values = value
        .as_array()
        .filter(|values| values.len() == length)
        .ok_or_else(|| error(format!("{field} has an invalid length")))?;
    let mut result = Vec::with_capacity(length);
    for value in values {
        let number = value
            .as_f64()
            .filter(|number| number.is_finite() && number.abs() <= limit as f64)
            .ok_or_else(|| {
                error(format!(
                    "{field} contains a non-finite or out-of-bound value"
                ))
            })? as f32;
        if !number.is_finite() || number.abs() > limit {
            return invalid(format!("{field} is not f32-stable"));
        }
        result.push(canonical_f32(number));
    }
    Ok(result)
}

pub(super) fn projection_f32_array<const N: usize>(
    value: &Value,
    limit: f32,
    field: &str,
) -> Result<[f32; N], RuntimeError> {
    let values = projection_f32_vector(value, N, limit, field)?;
    let mut result = [0.0_f32; N];
    result.copy_from_slice(&values);
    Ok(result)
}

pub(super) fn canonical_f32(value: f32) -> f32 {
    if value == 0.0 {
        0.0
    } else {
        value
    }
}

pub(super) fn projection_normalize_quaternion(
    mut value: Vec<f32>,
) -> Result<[f32; 4], RuntimeError> {
    if value.len() != 4 {
        return invalid("socket transform projection quaternion length differs");
    }
    let mut result = [
        value.remove(0),
        value.remove(0),
        value.remove(0),
        value.remove(0),
    ];
    let norm = result
        .iter()
        .map(|component| component * component)
        .sum::<f32>()
        .sqrt();
    if !norm.is_finite() || norm < 1.0e-7 {
        return invalid("socket transform projection quaternion is degenerate");
    }
    for component in &mut result {
        *component = canonical_f32(*component / norm);
    }
    let flip = result[3] < 0.0
        || (result[3] == 0.0
            && (result[0] < 0.0
                || (result[0] == 0.0 && result[1] < 0.0)
                || (result[0] == 0.0 && result[1] == 0.0 && result[2] < 0.0)));
    if flip {
        for component in &mut result {
            *component = canonical_f32(-*component);
        }
    }
    Ok(result)
}

pub(super) fn parse_projection_animation(
    animated_root: &Value,
    derived_root: &Value,
    binary: &[u8],
    source_sample_time_ticks: &[u64],
    part_ids: &[String],
) -> Result<ProjectionAnimation, RuntimeError> {
    let animated_animations = animated_root
        .get("animations")
        .and_then(Value::as_array)
        .filter(|animations| animations.len() == 1)
        .ok_or_else(|| error("socket transform projection animation is unavailable"))?;
    let derived_animations = derived_root
        .get("animations")
        .and_then(Value::as_array)
        .filter(|animations| animations.len() == 1)
        .ok_or_else(|| error("socket transform projection derived animation is unavailable"))?;
    if animated_animations != derived_animations {
        return invalid("socket transform projection derived animation changed");
    }
    let animation = exact_object(
        &animated_animations[0],
        &["name", "samplers", "channels"],
        "socket transform projection animation",
    )?;
    if text(animation, "name")? != "ForgeCAD rigid mechanical clip" {
        return invalid("socket transform projection animation name differs");
    }
    let samplers = animation
        .get("samplers")
        .and_then(Value::as_array)
        .filter(|samplers| !samplers.is_empty() && samplers.len() <= 128)
        .ok_or_else(|| error("socket transform projection samplers are unavailable"))?;
    let channels = animation
        .get("channels")
        .and_then(Value::as_array)
        .filter(|channels| !channels.is_empty() && channels.len() <= 128)
        .ok_or_else(|| error("socket transform projection channels are unavailable"))?;
    if samplers.len() != channels.len() {
        return invalid("socket transform projection sampler/channel count differs");
    }
    let mut result = Vec::with_capacity(channels.len());
    let mut seen = BTreeSet::new();
    for channel in channels {
        let channel_object = exact_object(
            channel,
            &["sampler", "target"],
            "socket transform projection animation channel",
        )?;
        let sampler_index = channel_object
            .get("sampler")
            .and_then(Value::as_u64)
            .ok_or_else(|| error("socket transform projection channel sampler is invalid"))?
            as usize;
        let sampler = samplers
            .get(sampler_index)
            .ok_or_else(|| error("socket transform projection channel sampler overflows"))?;
        let sampler_object = exact_object(
            sampler,
            &["input", "output", "interpolation"],
            "socket transform projection sampler",
        )?;
        if text(sampler_object, "interpolation")? != "LINEAR" {
            return invalid("socket transform projection interpolation differs");
        }
        let target = exact_object(
            channel_object
                .get("target")
                .ok_or_else(|| error("socket transform projection target is missing"))?,
            &["node", "path"],
            "socket transform projection target",
        )?;
        let node_index = target
            .get("node")
            .and_then(Value::as_u64)
            .ok_or_else(|| error("socket transform projection target node is invalid"))?
            as usize;
        if node_index >= part_ids.len() {
            return invalid("socket transform projection target leaves flat Part domain");
        }
        let path = text(target, "path")?.to_owned();
        if path != "translation" && path != "rotation" {
            return invalid("socket transform projection target path differs");
        }
        if !seen.insert((node_index, path.clone())) {
            return invalid("socket transform projection channel target is duplicated");
        }
        let input_index = sampler_object
            .get("input")
            .and_then(Value::as_u64)
            .ok_or_else(|| error("socket transform projection input accessor is invalid"))?
            as usize;
        let output_index = sampler_object
            .get("output")
            .and_then(Value::as_u64)
            .ok_or_else(|| error("socket transform projection output accessor is invalid"))?
            as usize;
        let input = decode_projection_accessor(
            animated_root,
            binary,
            input_index,
            "SCALAR",
            source_sample_time_ticks.len(),
        )?;
        let output = decode_projection_accessor(
            animated_root,
            binary,
            output_index,
            if path == "translation" {
                "VEC3"
            } else {
                "VEC4"
            },
            source_sample_time_ticks.len(),
        )?;
        let times_seconds = input
            .into_iter()
            .map(|value| {
                if value.len() != 1 {
                    return invalid("socket transform projection animation input is not scalar");
                }
                Ok(value[0])
            })
            .collect::<Result<Vec<_>, RuntimeError>>()?;
        for (time, tick) in times_seconds.iter().zip(source_sample_time_ticks) {
            let expected = *tick as f32 / 1000.0;
            if !time.is_finite() || (*time - expected).abs() > 1.0e-6 {
                return invalid("socket transform projection animation sample schedule differs");
            }
        }
        if times_seconds.windows(2).any(|pair| pair[0] >= pair[1]) {
            return invalid(
                "socket transform projection animation sample times are not increasing",
            );
        }
        for value in &output {
            if value.iter().any(|component| !component.is_finite()) {
                return invalid("socket transform projection animation output is non-finite");
            }
            if path == "translation" && value.iter().any(|component| component.abs() > 1000.0) {
                return invalid(
                    "socket transform projection animation translation is out of bounds",
                );
            }
            if path == "rotation" {
                projection_normalize_quaternion(value.clone())?;
            }
        }
        result.push(ProjectionChannel {
            node_index,
            path,
            times_seconds,
            values: output,
        });
    }
    Ok(ProjectionAnimation {
        channels: result,
        source_animation_projection_sha256: canonical_json_hash(&Value::Array(
            animated_animations.to_vec(),
        )),
        derived_animation_projection_sha256: canonical_json_hash(&Value::Array(
            derived_animations.to_vec(),
        )),
    })
}

fn decode_projection_accessor(
    root: &Value,
    binary: &[u8],
    accessor_index: usize,
    expected_type: &str,
    expected_count: usize,
) -> Result<Vec<Vec<f32>>, RuntimeError> {
    let accessors = root
        .get("accessors")
        .and_then(Value::as_array)
        .ok_or_else(|| error("socket transform projection accessors are unavailable"))?;
    let accessor = accessors
        .get(accessor_index)
        .ok_or_else(|| error("socket transform projection accessor index overflows"))?;
    let accessor_object = accessor
        .as_object()
        .ok_or_else(|| error("socket transform projection accessor is not an object"))?;
    if accessor_object.keys().any(|field| {
        !matches!(
            field.as_str(),
            "bufferView" | "componentType" | "count" | "type" | "min" | "max"
        )
    }) || !["bufferView", "componentType", "count", "type"]
        .iter()
        .all(|field| accessor_object.contains_key(*field))
    {
        return invalid("socket transform projection accessor field set differs");
    }
    if accessor_object.get("componentType").and_then(Value::as_u64) != Some(5126)
        || accessor_object.get("type").and_then(Value::as_str) != Some(expected_type)
        || accessor_object.get("count").and_then(Value::as_u64) != Some(expected_count as u64)
    {
        return invalid("socket transform projection accessor shape differs");
    }
    if accessor_object.contains_key("normalized") || accessor_object.contains_key("sparse") {
        return invalid("socket transform projection accessor uses normalized or sparse data");
    }
    let view_index = accessor_object
        .get("bufferView")
        .and_then(Value::as_u64)
        .ok_or_else(|| error("socket transform projection accessor bufferView is invalid"))?
        as usize;
    let views = root
        .get("bufferViews")
        .and_then(Value::as_array)
        .ok_or_else(|| error("socket transform projection bufferViews are unavailable"))?;
    let view = views
        .get(view_index)
        .ok_or_else(|| error("socket transform projection bufferView index overflows"))?;
    let view_object = exact_object(
        view,
        &["buffer", "byteOffset", "byteLength"],
        "socket transform projection bufferView",
    )?;
    if view_object.get("buffer").and_then(Value::as_u64) != Some(0)
        || view_object.contains_key("byteStride")
    {
        return invalid("socket transform projection bufferView layout differs");
    }
    let byte_offset = view_object
        .get("byteOffset")
        .and_then(Value::as_u64)
        .ok_or_else(|| error("socket transform projection bufferView offset is invalid"))?
        as usize;
    let byte_length = view_object
        .get("byteLength")
        .and_then(Value::as_u64)
        .ok_or_else(|| error("socket transform projection bufferView length is invalid"))?
        as usize;
    let components = match expected_type {
        "SCALAR" => 1,
        "VEC3" => 3,
        "VEC4" => 4,
        _ => return invalid("socket transform projection accessor type is outside the closed set"),
    };
    let expected_bytes = expected_count
        .checked_mul(components)
        .and_then(|count| count.checked_mul(4))
        .ok_or_else(|| error("socket transform projection accessor byte count overflowed"))?;
    if byte_offset
        .checked_add(byte_length)
        .is_none_or(|end| end > binary.len())
        || byte_length != expected_bytes
    {
        return invalid("socket transform projection accessor bytes are outside the bound");
    }
    let mut result = Vec::with_capacity(expected_count);
    for index in 0..expected_count {
        let mut row = Vec::with_capacity(components);
        for component in 0..components {
            let start = byte_offset + (index * components + component) * 4;
            let value =
                f32::from_le_bytes(binary[start..start + 4].try_into().map_err(|_| {
                    error("socket transform projection accessor bytes are truncated")
                })?);
            if !value.is_finite() {
                return invalid("socket transform projection accessor value is non-finite");
            }
            row.push(canonical_f32(value));
        }
        result.push(row);
    }
    Ok(result)
}

pub(super) fn sample_projection_channel(
    channel: &ProjectionChannel,
    sample_time_ticks: u64,
) -> Result<Vec<f32>, RuntimeError> {
    let sample_time_seconds = sample_time_ticks as f32 / 1000.0;
    if !sample_time_seconds.is_finite() {
        return invalid("socket transform projection sample time is non-finite");
    }
    let last = channel
        .times_seconds
        .len()
        .checked_sub(1)
        .ok_or_else(|| error("socket transform projection channel has no samples"))?;
    if sample_time_seconds <= channel.times_seconds[0] {
        return Ok(channel.values[0].clone());
    }
    if sample_time_seconds >= channel.times_seconds[last] {
        return Ok(channel.values[last].clone());
    }
    let upper = channel
        .times_seconds
        .windows(2)
        .position(|pair| sample_time_seconds >= pair[0] && sample_time_seconds <= pair[1])
        .ok_or_else(|| error("socket transform projection sample interval is unavailable"))?;
    let t0 = channel.times_seconds[upper];
    let t1 = channel.times_seconds[upper + 1];
    let alpha = ((sample_time_seconds - t0) / (t1 - t0)).clamp(0.0, 1.0);
    let left = &channel.values[upper];
    let right = &channel.values[upper + 1];
    if channel.path == "rotation" {
        let left = projection_normalize_quaternion(left.clone())?;
        let mut right = projection_normalize_quaternion(right.clone())?;
        let dot = left
            .iter()
            .zip(right.iter())
            .map(|(left, right)| left * right)
            .sum::<f32>();
        if dot < 0.0 {
            for value in &mut right {
                *value = -*value;
            }
        }
        let mut result = [0.0_f32; 4];
        for index in 0..4 {
            result[index] = left[index] + (right[index] - left[index]) * alpha;
        }
        return Ok(projection_normalize_quaternion(result.to_vec())?.to_vec());
    }
    Ok(left
        .iter()
        .zip(right.iter())
        .map(|(left, right)| canonical_f32(left + (right - left) * alpha))
        .collect())
}

pub(super) fn projection_identity_pose() -> ProjectionPose {
    ProjectionPose {
        translation: [0.0, 0.0, 0.0],
        rotation: [0.0, 0.0, 0.0, 1.0],
    }
}

pub(super) fn projection_compose(parent: ProjectionPose, local: ProjectionPose) -> ProjectionPose {
    let rotated_local = projection_rotate_vector(parent.rotation, local.translation);
    let translation = [
        canonical_f32(parent.translation[0] + rotated_local[0]),
        canonical_f32(parent.translation[1] + rotated_local[1]),
        canonical_f32(parent.translation[2] + rotated_local[2]),
    ];
    let rotation = projection_quaternion_multiply(parent.rotation, local.rotation);
    ProjectionPose {
        translation,
        rotation,
    }
}

fn projection_rotate_vector(rotation: [f32; 4], vector: [f32; 3]) -> [f32; 3] {
    let q_vector = [vector[0], vector[1], vector[2], 0.0];
    let inverse = [-rotation[0], -rotation[1], -rotation[2], rotation[3]];
    // The vector quaternion is not a unit quaternion: its norm is the
    // translation magnitude.  Normalizing either intermediate Hamilton
    // product would therefore collapse every non-unit local translation to
    // unit length before the parent rotation is applied.  Keep this path
    // raw, and normalize only the pose-rotation product in
    // `projection_compose`.
    let result = projection_quaternion_multiply_raw(
        projection_quaternion_multiply_raw(rotation, q_vector),
        inverse,
    );
    [
        canonical_f32(result[0]),
        canonical_f32(result[1]),
        canonical_f32(result[2]),
    ]
}

fn projection_quaternion_multiply_raw(left: [f32; 4], right: [f32; 4]) -> [f32; 4] {
    [
        left[3] * right[0] + left[0] * right[3] + left[1] * right[2] - left[2] * right[1],
        left[3] * right[1] - left[0] * right[2] + left[1] * right[3] + left[2] * right[0],
        left[3] * right[2] + left[0] * right[1] - left[1] * right[0] + left[2] * right[3],
        left[3] * right[3] - left[0] * right[0] - left[1] * right[1] - left[2] * right[2],
    ]
}

fn projection_quaternion_multiply(left: [f32; 4], right: [f32; 4]) -> [f32; 4] {
    projection_normalize_quaternion(projection_quaternion_multiply_raw(left, right).to_vec())
        .unwrap_or([0.0, 0.0, 0.0, 1.0])
}

fn pose_value(pose: ProjectionPose) -> Value {
    let translation = pose.translation.map(projection_f32_json_number);
    let rotation = pose.rotation.map(projection_f32_json_number);
    json!({
        "translation_m":translation,
        "rotation_quat_xyzw":rotation,
        "scale_xyz":[1.0,1.0,1.0]
    })
}

pub(super) fn projection_f32_json_number(value: f32) -> f64 {
    let value = canonical_f32(value);
    value
        .to_string()
        .parse::<f64>()
        .expect("finite f32 has a finite shortest decimal representation")
}

fn build_projection_frame(
    projection_key_sha256: &str,
    frame_index: u64,
    sample_time_ticks: u64,
    animation: &ProjectionAnimation,
    socket_nodes: &[ProjectionSocketNode],
    part_ids: &[String],
    animated_artifact_sha256: &str,
    derived_animated_socket_artifact_sha256: &str,
) -> Result<Value, RuntimeError> {
    let mut part_poses = vec![projection_identity_pose(); part_ids.len()];
    for (node_index, pose) in part_poses.iter_mut().enumerate() {
        let translation = animation
            .channels
            .iter()
            .find(|channel| channel.node_index == node_index && channel.path == "translation")
            .map(|channel| sample_projection_channel(channel, sample_time_ticks))
            .transpose()?
            .unwrap_or_else(|| vec![0.0, 0.0, 0.0]);
        let rotation = animation
            .channels
            .iter()
            .find(|channel| channel.node_index == node_index && channel.path == "rotation")
            .map(|channel| sample_projection_channel(channel, sample_time_ticks))
            .transpose()?
            .unwrap_or_else(|| vec![0.0, 0.0, 0.0, 1.0]);
        *pose = ProjectionPose {
            translation: projection_f32_array::<3>(
                &Value::Array(translation.into_iter().map(Value::from).collect()),
                1000.0,
                "sampled Part translation",
            )?,
            rotation: projection_normalize_quaternion(rotation)?,
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
        "schema_version":"GameWeaponAnimatedGlbSocketTransformProjectionSourceSample@1",
        "animated_artifact_sha256":animated_artifact_sha256,
        "sample_time_ticks":sample_time_ticks,
        "animation_projection_sha256":animation.source_animation_projection_sha256,
        "parts":part_sample
    }));

    let socket_transforms = socket_nodes
        .iter()
        .map(|socket| {
            let parent_world = if socket.parent_node_index < 0 {
                projection_identity_pose()
            } else {
                *part_poses
                    .get(socket.parent_node_index as usize)
                    .ok_or_else(|| {
                        error("socket transform projection owner Part index overflows")
                    })?
            };
            let composed = projection_compose(parent_world, socket.local);
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
                "composed_world_transform":pose_value(composed)
            }))
        })
        .collect::<Result<Vec<_>, RuntimeError>>()?;
    let inventory = socket_transforms
        .iter()
        .map(|value| {
            let object = value
                .as_object()
                .ok_or_else(|| error("socket transform projection frame is invalid"))?;
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
        "schema_version":"GameWeaponAnimatedGlbSocketTransformProjectionDerivedSample@1",
        "derived_animated_socket_artifact_sha256":derived_animated_socket_artifact_sha256,
        "sample_time_ticks":sample_time_ticks,
        "animation_projection_sha256":animation.derived_animation_projection_sha256,
        "socket_transforms":socket_transforms
    }));
    let mut frame = json!({
        "schema_version":SOCKET_TRANSFORM_PROJECTION_FRAME_SCHEMA,
        "projection_key_sha256":projection_key_sha256,
        "frame_index":frame_index,
        "sample_time_ticks":sample_time_ticks,
        "source_animation_sample_sha256":source_animation_sample_sha256,
        "derived_socket_sample_sha256":derived_socket_sample_sha256,
        "socket_transform_inventory_sha256":socket_transform_inventory_sha256,
        "socket_transform_readback_sha256":"",
        "socket_transforms":socket_transforms,
        "canonical_sha256":"",
        "created_at":now_string()
    });
    let mut readback = frame.clone();
    readback["created_at"] = Value::String(String::new());
    frame["socket_transform_readback_sha256"] = Value::String(canonical_json_hash(&readback));
    frame["canonical_sha256"] = Value::String(canonical_json_hash(&frame));
    Ok(frame)
}

pub(super) fn projection_socket_inventory_hash(
    anchor_set_object_sha256: &str,
    anchor_set: &Value,
    anchor_ids: &[String],
    source_node_count: usize,
) -> Result<String, RuntimeError> {
    let anchors = anchor_set
        .get("anchors")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            error("socket transform projection AnchorSet inventory anchors are unavailable")
        })?;
    let by_id = anchors
        .iter()
        .filter_map(|anchor| Some((anchor.get("anchor_id")?.as_str()?, anchor)))
        .collect::<BTreeMap<_, _>>();
    let mut nodes = Vec::with_capacity(anchor_ids.len());
    for anchor_id in anchor_ids {
        let anchor = by_id
            .get(anchor_id.as_str())
            .copied()
            .ok_or_else(|| error("socket transform projection inventory anchor is unavailable"))?;
        nodes.push(json!({
            "socket_node_id":anchor_id,
            "anchor_id":anchor_id,
            "role":anchor["role"],
            "node_name":format!("forgecad-anchor-{anchor_id}"),
            "node_kind":"empty",
            "parent_kind":anchor["parent_kind"],
            "parent_node_name":if anchor["parent_kind"] == "synthetic-scene-root" {
                Value::Null
            } else {
                anchor["owner_part_id"].clone()
            },
            "owner_part_id":anchor["owner_part_id"],
            "local_translation_m":anchor["local_translation_m"],
            "local_rotation_quat_xyzw":anchor["local_rotation_quat_xyzw"],
            "local_scale_xyz":anchor["local_scale_xyz"]
        }));
    }
    let mut inventory = json!({
        "schema_version":"GameWeaponGlbSocketNodeInventory@1",
        "anchor_set_object_sha256":anchor_set_object_sha256,
        "anchor_set_canonical_sha256":anchor_set["canonical_sha256"],
        "source_node_count":source_node_count,
        "nodes":nodes,
        "canonical_sha256":""
    });
    inventory["canonical_sha256"] = Value::String(canonical_json_hash(&inventory));
    Ok(inventory["canonical_sha256"]
        .as_str()
        .expect("socket inventory canonical hash is a string")
        .to_owned())
}

pub(super) fn projection_part_hierarchy_hash(
    source_root: &Value,
    part_ids: &[String],
) -> Result<String, RuntimeError> {
    let nodes = source_root
        .get("nodes")
        .and_then(Value::as_array)
        .filter(|nodes| nodes.len() == part_ids.len())
        .ok_or_else(|| error("socket transform projection Part hierarchy nodes are unavailable"))?;
    let hierarchy = nodes
        .iter()
        .enumerate()
        .map(|(node_index, _node)| {
            json!({
                "node_index":node_index,
                "part_id":part_ids[node_index],
                "parent_node_index":-1,
                "parent_node_name":Value::Null,
                "children":[]
            })
        })
        .collect::<Vec<_>>();
    let _ = nodes;
    Ok(canonical_json_hash(&json!({
        "schema_version":"GameWeaponAnimatedGlbSocketPartHierarchy@1",
        "policy":SOCKET_TRANSFORM_PART_HIERARCHY_POLICY,
        "nodes":hierarchy
    })))
}

fn build_projection_value(
    request: &ProjectionRequest,
    animation_receipt: &Value,
    inspection: &AnimatedSourceInspection,
    animation: &ProjectionAnimation,
    part_hierarchy_sha256: &str,
    frames: &[Value],
) -> Result<Value, RuntimeError> {
    let mut map = Map::new();
    map.insert(
        "schema_version".to_owned(),
        json!(SOCKET_TRANSFORM_PROJECTION_SCHEMA),
    );
    map.insert(
        "projection_key_sha256".to_owned(),
        json!(request.projection_key_sha256),
    );
    map.insert("project_id".to_owned(), json!(request.project_id));
    map.insert("candidate_id".to_owned(), json!(request.candidate_id));
    map.insert(
        "candidate_state_sha256".to_owned(),
        json!(request.candidate_state_sha256),
    );
    for (field, value) in [
        (
            "delivery_manifest_object_sha256",
            request.delivery_manifest_object_sha256.clone(),
        ),
        (
            "source_artifact_sha256",
            request.source_artifact_sha256.clone(),
        ),
        (
            "source_artifact_readback_sha256",
            request.source_artifact_readback_sha256.clone(),
        ),
        (
            "animated_artifact_sha256",
            request.animated_artifact_sha256.clone(),
        ),
        (
            "animated_artifact_readback_sha256",
            request.animated_artifact_readback_sha256.clone(),
        ),
        (
            "animation_receipt_object_sha256",
            request.animation_receipt_object_sha256.clone(),
        ),
        (
            "animation_receipt_canonical_sha256",
            request.animation_receipt_canonical_sha256.clone(),
        ),
        (
            "animated_socket_materialization_key_sha256",
            request.animated_socket_materialization_key_sha256.clone(),
        ),
        (
            "derived_animated_socket_artifact_sha256",
            request.derived_animated_socket_artifact_sha256.clone(),
        ),
        (
            "derived_animated_socket_artifact_readback_sha256",
            request
                .derived_animated_socket_artifact_readback_sha256
                .clone(),
        ),
        (
            "derived_animated_socket_receipt_object_sha256",
            request
                .derived_animated_socket_receipt_object_sha256
                .clone(),
        ),
        (
            "derived_animated_socket_receipt_canonical_sha256",
            request
                .derived_animated_socket_receipt_canonical_sha256
                .clone(),
        ),
        (
            "anchor_set_object_sha256",
            request.anchor_set_object_sha256.clone(),
        ),
        (
            "anchor_set_canonical_sha256",
            request.anchor_set_canonical_sha256.clone(),
        ),
        ("animation_clip_id", request.animation_clip_id.clone()),
        (
            "animation_clip_object_sha256",
            request.animation_clip_object_sha256.clone(),
        ),
        (
            "animation_clip_canonical_sha256",
            request.animation_clip_canonical_sha256.clone(),
        ),
        (
            "socket_node_id_encoding_sha256",
            request.socket_node_id_encoding_sha256.clone(),
        ),
        (
            "socket_node_inventory_sha256",
            request.socket_node_inventory_sha256.clone(),
        ),
        ("socket_roles_sha256", request.socket_roles_sha256.clone()),
        ("part_hierarchy_sha256", part_hierarchy_sha256.to_owned()),
        (
            "part_hierarchy_policy",
            request.part_hierarchy_policy.clone(),
        ),
        (
            "transform_representation_policy",
            request.transform_representation_policy.clone(),
        ),
        (
            "sample_schedule_sha256",
            request.sample_schedule_sha256.clone(),
        ),
        ("frame_scope", request.frame_scope.clone()),
        (
            "transform_projection_policy",
            request.transform_projection_policy.clone(),
        ),
        ("coordinate_system", request.coordinate_system.clone()),
        ("transform_convention", request.transform_convention.clone()),
        (
            "float_quantization_policy",
            request.float_quantization_policy.clone(),
        ),
        ("input_sha256", request.input_sha256.clone()),
    ] {
        map.insert(field.to_owned(), Value::String(value));
    }
    map.insert(
        "socket_roles".to_owned(),
        Value::Array(
            SOCKET_TRANSFORM_ROLES
                .iter()
                .map(|role| Value::String((*role).to_owned()))
                .collect(),
        ),
    );
    map.insert(
        "sample_count".to_owned(),
        json!(request.sample_time_ticks.len()),
    );
    map.insert(
        "sample_time_ticks".to_owned(),
        json!(request.sample_time_ticks),
    );
    map.insert("timebase_hz".to_owned(), json!(request.timebase_hz));
    map.insert("frames".to_owned(), Value::Array(frames.to_vec()));
    map.insert(
        "projection_status".to_owned(),
        json!(SOCKET_TRANSFORM_STATUS),
    );
    map.insert("quality_status".to_owned(), json!("structural_only"));
    map.insert("visual_quality_status".to_owned(), json!("NOT_PROVEN"));
    map.insert(
        "commercial_fps_quality_status".to_owned(),
        json!("NOT_PROVEN"),
    );
    map.insert("human_review_status".to_owned(), json!("NOT_RUN"));
    map.insert("commercial_engine_status".to_owned(), json!("NOT_RUN"));
    map.insert("runtime_write_performed".to_owned(), json!(true));
    map.insert("restart_hash_verified".to_owned(), json!(true));
    map.insert("candidate_confirmed".to_owned(), json!(false));
    map.insert("version_created".to_owned(), json!(false));
    map.insert("export_performed".to_owned(), json!(false));
    map.insert("actual_engine_roundtrip".to_owned(), json!(false));
    map.insert("production_stage_advanced".to_owned(), json!(false));
    map.insert(
        "limitations".to_owned(),
        Value::Array(
            SOCKET_TRANSFORM_LIMITATIONS
                .iter()
                .map(|value| Value::String((*value).to_owned()))
                .collect(),
        ),
    );
    map.insert("canonical_sha256".to_owned(), Value::String(String::new()));
    map.insert("created_at".to_owned(), Value::String(now_string()));
    if animation_receipt
        .get("sample_time_ticks")
        .and_then(Value::as_array)
        .is_none()
        || inspection.sample_time_ticks.is_empty()
        || animation.source_animation_projection_sha256.is_empty()
    {
        return invalid("socket transform projection source animation evidence is incomplete");
    }
    Ok(Value::Object(map))
}

fn animated_socket_get_by_key(
    runtime: &Runtime,
    project_id: &str,
    key: &str,
) -> Result<Value, RuntimeError> {
    let link = runtime
        .store
        .get_game_weapon_animated_glb_socket_materialization_link(key)?
        .ok_or_else(|| error("durable animated GLB socket materialization is unavailable"))?;
    if link.project_id != project_id
        || link.animated_socket_materialization_key_sha256 != key
        || link.request_sha256 != key
        || link.socket_materialization_policy != ANIMATED_SOCKET_POLICY
        || link.lod_scope != ANIMATED_SOCKET_LOD_SCOPE
    {
        return invalid("durable animated GLB socket materialization binding differs");
    }
    let delivery = game_asset_delivery::get(
        runtime,
        &json!({
            "schema_version":"GameAssetDeliveryGetRequest@1",
            "project_id":project_id,
            "delivery_manifest_object_sha256":link.delivery_manifest_object_sha256
        }),
    )?;
    let levels = delivery
        .get("lod_receipt")
        .and_then(|value| value.get("levels"))
        .and_then(Value::as_array)
        .filter(|values| values.len() == 3)
        .ok_or_else(|| error("durable animated GLB socket delivery LOD receipt is incomplete"))?;
    let lod0 = levels
        .first()
        .ok_or_else(|| error("durable animated GLB socket delivery LOD0 is unavailable"))?;
    if lod0.get("candidate_id").and_then(Value::as_str) != Some(link.candidate_id.as_str())
        || lod0.get("candidate_state_sha256").and_then(Value::as_str)
            != Some(link.candidate_state_sha256.as_str())
        || lod0.get("artifact_sha256").and_then(Value::as_str)
            != Some(link.lod0_artifact_sha256.as_str())
    {
        return invalid("durable animated GLB socket delivery LOD0 binding differs");
    }
    if delivery
        .get("link")
        .and_then(|value| value.get("animation_artifact_sha256"))
        .filter(|value| !value.is_null())
        .and_then(Value::as_str)
        .is_some_and(|value| value != link.animated_artifact_sha256)
    {
        return invalid("durable animated GLB socket delivery animation differs");
    }
    let anchor_result = game_asset_delivery::weapon_anchor_get(
        runtime,
        &json!({
            "schema_version":"GameWeaponAnchorGetRequest@1",
            "project_id":project_id,
            "delivery_manifest_object_sha256":link.delivery_manifest_object_sha256
        }),
    )?;
    let anchor_link = anchor_result
        .get("link")
        .and_then(Value::as_object)
        .ok_or_else(|| error("durable animated GLB socket AnchorSet link is unavailable"))?;
    if anchor_link
        .get("anchor_set_object_sha256")
        .and_then(Value::as_str)
        != Some(link.anchor_set_object_sha256.as_str())
    {
        return invalid("durable animated GLB socket AnchorSet binding differs");
    }
    let anchor_set = anchor_result
        .get("anchor_set")
        .cloned()
        .ok_or_else(|| error("durable animated GLB socket AnchorSet is unavailable"))?;
    let anchor_canonical = anchor_set
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| error("durable animated GLB socket AnchorSet canonical hash is missing"))?;
    if anchor_canonical != link.anchor_set_canonical_sha256 {
        return invalid("durable animated GLB socket AnchorSet canonical hash differs");
    }
    let anchor_ids = game_asset_delivery::socket_anchor_ids(&anchor_set)?;
    let part_ids = anchor_set
        .get("part_ids")
        .and_then(Value::as_array)
        .ok_or_else(|| error("durable animated GLB socket Part inventory is unavailable"))?
        .iter()
        .map(|value| {
            value
                .as_str()
                .filter(|value| valid_identifier(value))
                .map(str::to_owned)
                .ok_or_else(|| error("durable animated GLB socket Part ID is invalid"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let animation_receipt = read_canonical_json(
        runtime,
        &link.animation_receipt_object_sha256,
        MECHANICAL_ANIMATION_RECEIPT_SCHEMA,
    )?;
    validate_mechanical_animation_source_receipt(
        &animation_receipt,
        project_id,
        &link.candidate_id,
        &link.candidate_state_sha256,
        &link.lod0_artifact_sha256,
        &link.animated_artifact_sha256,
        &part_ids,
    )?;
    if animation_receipt
        .get("canonical_sha256")
        .and_then(Value::as_str)
        != Some(link.animation_receipt_canonical_sha256.as_str())
    {
        return invalid("durable animated GLB socket source receipt canonical hash differs");
    }
    let source_glb = runtime.cas_read_bounded(&link.lod0_artifact_sha256, MAX_GLB_BYTES as u64)?;
    let animated_glb =
        runtime.cas_read_bounded(&link.animated_artifact_sha256, MAX_GLB_BYTES as u64)?;
    let mut inspection = inspect_animated_source(
        &source_glb,
        &animated_glb,
        &link.lod0_artifact_sha256,
        &link.animated_artifact_sha256,
        &animation_receipt,
        &part_ids,
    )?;
    let materialized = game_asset_delivery::materialize_socket_glb(
        &animated_glb,
        &link.animated_artifact_sha256,
        &link.anchor_set_object_sha256,
        &anchor_set,
        &part_ids,
        &anchor_ids,
    )?;
    let expected_derived = sha256_hex(&materialized.glb);
    validate_socket_animation_preservation(
        &animated_glb,
        &materialized.glb,
        &link.animated_artifact_sha256,
        &expected_derived,
        &mut inspection,
    )?;
    if expected_derived != link.derived_animated_socket_artifact_sha256 {
        return invalid("durable animated GLB socket derived artifact hash differs");
    }
    let expected_readback = animated_socket_readback_sha256(
        key,
        project_id,
        &link.candidate_id,
        &link.candidate_state_sha256,
        &link.lod0_artifact_sha256,
        &link.animated_artifact_sha256,
        &link.anchor_set_object_sha256,
        &link.anchor_set_canonical_sha256,
        &materialized,
        &inspection,
        &expected_derived,
    )?;
    if expected_readback != link.derived_animated_socket_artifact_readback_sha256 {
        return invalid("durable animated GLB socket inline readback hash differs");
    }
    let derived_glb = runtime.cas_read_bounded(
        &link.derived_animated_socket_artifact_sha256,
        MAX_GLB_BYTES as u64,
    )?;
    if sha256_hex(&derived_glb) != link.derived_animated_socket_artifact_sha256
        || derived_glb != materialized.glb
    {
        return invalid("durable animated GLB socket derived bytes differ on restart");
    }
    let receipt = read_canonical_json(
        runtime,
        &link.receipt_object_sha256,
        ANIMATED_SOCKET_RECEIPT_SCHEMA,
    )?;
    verify_animated_socket_receipt(&receipt)?;
    validate_animated_socket_receipt_binding(&receipt, &link, &materialized, &inspection)?;
    Ok(json!({
        "schema_version":ANIMATED_SOCKET_GET_RESULT_SCHEMA,
        "animated_socket_materialization_key_sha256":key,
        "derived_animated_socket_artifact_sha256":link.derived_animated_socket_artifact_sha256,
        "receipt_object_sha256":link.receipt_object_sha256,
        "receipt":receipt,
        "link":link,
        "restart_hash_verified":true,
        "runtime_write_performed":false,
        "candidate_confirmed":false,
        "export_performed":false,
        "actual_engine_roundtrip":false,
        "quality_status":"structural_only"
    }))
}

fn ensure_source_object(
    runtime: &Runtime,
    sha256: &str,
    animated: bool,
) -> Result<(), RuntimeError> {
    let object = runtime
        .store
        .get_object(sha256)?
        .ok_or_else(|| error("animated GLB socket source CAS object is unavailable"))?;
    let allowed_static = matches!(
        object.kind.as_str(),
        "geometry-glb" | "appearance-glb" | "appearance-v2-glb"
    );
    let valid = object.size_bytes > 0
        && object.size_bytes <= MAX_GLB_BYTES as u64
        && object.mime == "model/gltf-binary"
        && if animated {
            object.kind == MECHANICAL_ANIMATION_GLB_KIND
        } else {
            allowed_static
        };
    if !valid {
        return invalid("animated GLB socket source CAS metadata differs");
    }
    Ok(())
}

fn ensure_json_object(
    runtime: &Runtime,
    sha256: &str,
    schema_version: &str,
    kind: &str,
) -> Result<(), RuntimeError> {
    let object = runtime
        .store
        .get_object(sha256)?
        .ok_or_else(|| error("animated GLB socket JSON CAS object is unavailable"))?;
    if object.size_bytes == 0
        || object.size_bytes > 1024 * 1024
        || object.mime != "application/json"
        || object.kind != kind
    {
        return invalid(format!(
            "animated GLB socket JSON metadata differs for {schema_version}"
        ));
    }
    Ok(())
}

fn read_canonical_json(
    runtime: &Runtime,
    sha256: &str,
    schema_version: &str,
) -> Result<Value, RuntimeError> {
    let bytes = runtime.cas_read_bounded(sha256, 1024 * 1024)?;
    if bytes.is_empty() || sha256_hex(&bytes) != sha256 {
        return invalid("animated GLB socket JSON bytes or hash differs");
    }
    let value: Value =
        serde_json::from_slice(&bytes).map_err(|_| error("animated GLB socket JSON is invalid"))?;
    if value.get("schema_version").and_then(Value::as_str) != Some(schema_version) {
        return invalid("animated GLB socket JSON schema differs");
    }
    verify_canonical(&value)?;
    Ok(value)
}

fn validate_mechanical_animation_source_receipt(
    receipt: &Value,
    project_id: &str,
    candidate_id: &str,
    candidate_state_sha256: &str,
    source_artifact_sha256: &str,
    animated_artifact_sha256: &str,
    part_ids: &[String],
) -> Result<(), RuntimeError> {
    let object = exact_object(
        receipt,
        &[
            "schema_version",
            "project_id",
            "candidate_id",
            "candidate_state_sha256",
            "source_artifact_sha256",
            "artifact_readback_sha256",
            "geometry_candidate_evidence_sha256",
            "program_sha256",
            "operator_catalog_sha256",
            "readback_config_sha256",
            "animated_artifact_sha256",
            "clip_id",
            "clip_object_sha256",
            "clip_sha256",
            "rest_frame_sha256",
            "pose_action_sha256",
            "source_replay_worker_cohort_sha256",
            "sampling_policy_sha256",
            "sample_time_ticks",
            "timebase_hz",
            "interpolation",
            "part_ids",
            "node_count",
            "sampler_count",
            "channel_count",
            "accessor_count_added",
            "buffer_view_count_added",
            "animation_validation_sha256",
            "validator_status",
            "hard_gate_passed",
            "source_static_projection_exact",
            "no_skinning",
            "no_morph_targets",
            "materialization_status",
            "runtime_write_performed",
            "quality_status",
            "limitations",
            "canonical_sha256",
        ],
        MECHANICAL_ANIMATION_RECEIPT_SCHEMA,
    )?;
    if text(object, "schema_version")? != MECHANICAL_ANIMATION_RECEIPT_SCHEMA
        || text(object, "project_id")? != project_id
        || text(object, "candidate_id")? != candidate_id
        || text(object, "candidate_state_sha256")? != candidate_state_sha256
        || text(object, "source_artifact_sha256")? != source_artifact_sha256
        || text(object, "animated_artifact_sha256")? != animated_artifact_sha256
        || text(object, "validator_status")? != "strict-rigid-gltf-animation-readback-pass"
        || object.get("hard_gate_passed").and_then(Value::as_bool) != Some(true)
        || object
            .get("source_static_projection_exact")
            .and_then(Value::as_bool)
            != Some(true)
        || object.get("no_skinning").and_then(Value::as_bool) != Some(true)
        || object.get("no_morph_targets").and_then(Value::as_bool) != Some(true)
        || text(object, "materialization_status")? != "runtime-owned-cas-animated-glb"
        || object
            .get("runtime_write_performed")
            .and_then(Value::as_bool)
            != Some(true)
        || text(object, "quality_status")? != "structural_only"
        || object.get("part_ids")
            != Some(&Value::Array(
                part_ids.iter().cloned().map(Value::String).collect(),
            ))
    {
        return invalid("mechanical animation source receipt binding or semantics differ");
    }
    for field in [
        "candidate_state_sha256",
        "source_artifact_sha256",
        "artifact_readback_sha256",
        "geometry_candidate_evidence_sha256",
        "program_sha256",
        "operator_catalog_sha256",
        "readback_config_sha256",
        "animated_artifact_sha256",
        "clip_object_sha256",
        "clip_sha256",
        "rest_frame_sha256",
        "pose_action_sha256",
        "source_replay_worker_cohort_sha256",
        "sampling_policy_sha256",
        "animation_validation_sha256",
    ] {
        if !object
            .get(field)
            .and_then(Value::as_str)
            .is_some_and(forgecad_contracts::is_sha256)
        {
            return invalid(format!(
                "mechanical animation receipt field {field} is invalid"
            ));
        }
    }
    if object
        .get("sample_time_ticks")
        .and_then(Value::as_array)
        .is_none()
        || object.get("node_count").and_then(Value::as_u64).is_none()
        || object
            .get("sampler_count")
            .and_then(Value::as_u64)
            .is_none()
        || object
            .get("channel_count")
            .and_then(Value::as_u64)
            .is_none()
        || object
            .get("accessor_count_added")
            .and_then(Value::as_u64)
            .is_none()
        || object
            .get("buffer_view_count_added")
            .and_then(Value::as_u64)
            .is_none()
    {
        return invalid("mechanical animation receipt readback counts are invalid");
    }
    verify_canonical(receipt)
}

fn inspect_animated_source(
    source_glb: &[u8],
    animated_glb: &[u8],
    source_artifact_sha256: &str,
    animated_artifact_sha256: &str,
    source_receipt: &Value,
    part_ids: &[String],
) -> Result<AnimatedSourceInspection, RuntimeError> {
    if source_glb.is_empty()
        || animated_glb.is_empty()
        || source_glb.len() > MAX_GLB_BYTES
        || animated_glb.len() > MAX_GLB_BYTES
        || sha256_hex(source_glb) != source_artifact_sha256
        || sha256_hex(animated_glb) != animated_artifact_sha256
    {
        return invalid("animated GLB socket source or animated artifact bytes differ");
    }
    let (source_root, source_binary) = parse_glb(source_glb)?;
    let (animated_root, animated_binary) = parse_glb(animated_glb)?;
    let source_nodes = source_root
        .get("nodes")
        .and_then(Value::as_array)
        .filter(|nodes| !nodes.is_empty() && nodes.len() <= 64)
        .ok_or_else(|| error("animated GLB socket source nodes are outside the bound"))?;
    let animated_nodes = animated_root
        .get("nodes")
        .and_then(Value::as_array)
        .filter(|nodes| !nodes.is_empty() && nodes.len() <= 64)
        .ok_or_else(|| error("animated GLB socket animated nodes are outside the bound"))?;
    if source_nodes != animated_nodes {
        return invalid("animated GLB source node projection differs before socket append");
    }
    if source_root.get("animations").is_some()
        || source_root.get("skins").is_some()
        || animated_root.get("skins").is_some()
    {
        return invalid("animated GLB socket source must be unskinned static geometry");
    }
    if glb_has_morph_targets(&source_root) || glb_has_morph_targets(&animated_root) {
        return invalid("animated GLB socket source contains morph targets");
    }
    validate_buffer_declaration(&source_root, source_binary.len())?;
    validate_buffer_declaration(&animated_root, animated_binary.len())?;
    if animated_binary.get(..source_binary.len()) != Some(source_binary.as_slice()) {
        return invalid("animated GLB socket BIN does not preserve the source prefix");
    }

    let source_accessors = source_root
        .get("accessors")
        .and_then(Value::as_array)
        .ok_or_else(|| error("animated GLB socket source accessors are unavailable"))?;
    let source_views = source_root
        .get("bufferViews")
        .and_then(Value::as_array)
        .ok_or_else(|| error("animated GLB socket source bufferViews are unavailable"))?;
    let animated_accessors = animated_root
        .get("accessors")
        .and_then(Value::as_array)
        .ok_or_else(|| error("animated GLB socket animated accessors are unavailable"))?;
    let animated_views = animated_root
        .get("bufferViews")
        .and_then(Value::as_array)
        .ok_or_else(|| error("animated GLB socket animated bufferViews are unavailable"))?;
    if animated_accessors.len() < source_accessors.len() + 3
        || animated_views.len() < source_views.len() + 3
    {
        return invalid("animated GLB socket animation did not append bounded accessors/views");
    }

    let animations = animated_root
        .get("animations")
        .and_then(Value::as_array)
        .filter(|animations| animations.len() == 1)
        .ok_or_else(|| error("animated GLB socket requires one rigid animation"))?;
    let animation = exact_object(
        &animations[0],
        &["name", "samplers", "channels"],
        "animated GLB socket rigid animation",
    )?;
    if text(animation, "name")? != "ForgeCAD rigid mechanical clip" {
        return invalid("animated GLB socket animation name differs");
    }
    let samplers = animation
        .get("samplers")
        .and_then(Value::as_array)
        .filter(|values| (2..=128).contains(&values.len()))
        .ok_or_else(|| error("animated GLB socket animation samplers are invalid"))?;
    let channels = animation
        .get("channels")
        .and_then(Value::as_array)
        .filter(|values| (2..=128).contains(&values.len()))
        .ok_or_else(|| error("animated GLB socket animation channels are invalid"))?;
    if samplers.len() != channels.len() {
        return invalid("animated GLB socket sampler/channel coverage differs");
    }
    let mut seen_samplers = BTreeSet::new();
    for channel in channels {
        let channel_object = exact_object(
            channel,
            &["sampler", "target"],
            "animated GLB socket channel",
        )?;
        let sampler_index = channel_object
            .get("sampler")
            .and_then(Value::as_u64)
            .ok_or_else(|| error("animated GLB socket channel sampler is invalid"))?
            as usize;
        if !seen_samplers.insert(sampler_index) {
            return invalid("animated GLB socket sampler is referenced more than once");
        }
        let sampler = samplers
            .get(sampler_index)
            .ok_or_else(|| error("animated GLB socket channel sampler overflows"))?;
        let sampler_object = exact_object(
            sampler,
            &["input", "output", "interpolation"],
            "animated GLB socket sampler",
        )?;
        if text(sampler_object, "interpolation")? != "LINEAR" {
            return invalid("animated GLB socket interpolation is not LINEAR");
        }
        let target = channel_object
            .get("target")
            .ok_or_else(|| error("animated GLB socket channel target is unavailable"))?;
        let target_object = exact_object(target, &["node", "path"], "animated GLB socket target")?;
        let node_index = target_object
            .get("node")
            .and_then(Value::as_u64)
            .ok_or_else(|| error("animated GLB socket target node is invalid"))?
            as usize;
        if node_index >= source_nodes.len()
            || !matches!(
                target_object.get("path").and_then(Value::as_str),
                Some("translation") | Some("rotation")
            )
        {
            return invalid("animated GLB socket target leaves the source rigid Part domain");
        }
        for field in ["input", "output"] {
            let index = sampler_object
                .get(field)
                .and_then(Value::as_u64)
                .ok_or_else(|| error(format!("animated GLB socket sampler {field} is invalid")))?
                as usize;
            if index < source_accessors.len() || index >= animated_accessors.len() {
                return invalid("animated GLB socket sampler accessor ownership differs");
            }
        }
    }
    if seen_samplers.len() != samplers.len() {
        return invalid("animated GLB socket has unconsumed samplers");
    }
    validate_added_buffer_layout(
        animated_views,
        source_views.len(),
        &animated_binary,
        source_binary.len(),
    )?;

    let mut projected = animated_root.clone();
    projected
        .as_object_mut()
        .ok_or_else(|| error("animated GLB socket projection is not an object"))?
        .remove("animations");
    projected["accessors"] = Value::Array(animated_accessors[..source_accessors.len()].to_vec());
    projected["bufferViews"] = Value::Array(animated_views[..source_views.len()].to_vec());
    projected["buffers"][0]["byteLength"] = source_root["buffers"][0]["byteLength"].clone();
    if let Some(forgecad) = projected
        .get_mut("extras")
        .and_then(Value::as_object_mut)
        .and_then(|extras| extras.get_mut("forgecad"))
        .and_then(Value::as_object_mut)
    {
        forgecad.remove("rigid_animation");
    }
    if projected != source_root {
        return invalid("animated GLB socket cannot reconstruct the source static projection");
    }

    let source_receipt_object = source_receipt
        .as_object()
        .ok_or_else(|| error("mechanical animation receipt is not an object"))?;
    if source_receipt_object
        .get("node_count")
        .and_then(Value::as_u64)
        != Some(source_nodes.len() as u64)
        || source_receipt_object
            .get("sampler_count")
            .and_then(Value::as_u64)
            != Some(samplers.len() as u64)
        || source_receipt_object
            .get("channel_count")
            .and_then(Value::as_u64)
            != Some(channels.len() as u64)
        || source_receipt_object
            .get("accessor_count_added")
            .and_then(Value::as_u64)
            != Some((animated_accessors.len() - source_accessors.len()) as u64)
        || source_receipt_object
            .get("buffer_view_count_added")
            .and_then(Value::as_u64)
            != Some((animated_views.len() - source_views.len()) as u64)
    {
        return invalid("mechanical animation receipt counts differ from the animated GLB");
    }
    let sample_time_ticks = source_receipt_object
        .get("sample_time_ticks")
        .and_then(Value::as_array)
        .ok_or_else(|| error("mechanical animation sample ticks are unavailable"))?
        .iter()
        .map(|value| {
            value
                .as_u64()
                .filter(|tick| *tick <= 1_000_000)
                .ok_or_else(|| error("mechanical animation sample tick is invalid"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if sample_time_ticks.len() < 2 || sample_time_ticks.windows(2).any(|pair| pair[0] >= pair[1]) {
        return invalid("mechanical animation sample ticks are not strictly increasing");
    }
    if source_receipt_object.get("part_ids")
        != Some(&Value::Array(
            part_ids.iter().cloned().map(Value::String).collect(),
        ))
    {
        return invalid("mechanical animation Part inventory differs from the AnchorSet");
    }
    let source_animation_projection_sha256 =
        canonical_json_hash(animated_root.get("animations").ok_or_else(|| {
            error("animated GLB socket source animation projection is unavailable")
        })?);
    let derived_animation = animated_root
        .get("animations")
        .ok_or_else(|| error("animated GLB socket animation projection is unavailable"))?;
    let derived_animation_projection_sha256 = canonical_json_hash(derived_animation);
    let source_animation_validation_sha256 = source_receipt_object
        .get("animation_validation_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| error("mechanical animation validation hash is unavailable"))?
        .to_owned();
    let derived_animation_validation_sha256 = canonical_json_hash(&json!({
        "schema_version":"GameWeaponAnimatedGlbSocketAnimationValidation@1",
        "source_artifact_sha256":source_artifact_sha256,
        "animated_artifact_sha256":animated_artifact_sha256,
        "animation_projection_sha256":derived_animation_projection_sha256,
        "sample_time_ticks":sample_time_ticks,
        "part_ids":part_ids,
        "sampler_count":samplers.len(),
        "channel_count":channels.len(),
        "accessor_count_added":animated_accessors.len() - source_accessors.len(),
        "buffer_view_count_added":animated_views.len() - source_views.len()
    }));
    Ok(AnimatedSourceInspection {
        derived_node_count: animated_nodes.len(),
        sampler_count: samplers.len(),
        channel_count: channels.len(),
        accessor_count_added: animated_accessors.len() - source_accessors.len(),
        buffer_view_count_added: animated_views.len() - source_views.len(),
        sample_time_ticks,
        part_ids: part_ids.to_vec(),
        source_animation_projection_sha256,
        derived_animation_projection_sha256,
        source_animation_validation_sha256,
        derived_animation_validation_sha256,
    })
}

fn validate_buffer_declaration(root: &Value, binary_len: usize) -> Result<(), RuntimeError> {
    if root
        .get("buffers")
        .and_then(Value::as_array)
        .and_then(|buffers| buffers.first())
        .and_then(|buffer| buffer.get("byteLength"))
        .and_then(Value::as_u64)
        != Some(binary_len as u64)
    {
        return invalid("animated GLB socket buffer declaration differs from BIN length");
    }
    Ok(())
}

fn validate_socket_animation_preservation(
    source_animated_glb: &[u8],
    derived_socket_glb: &[u8],
    source_animated_artifact_sha256: &str,
    derived_socket_artifact_sha256: &str,
    inspection: &mut AnimatedSourceInspection,
) -> Result<(), RuntimeError> {
    if sha256_hex(source_animated_glb) != source_animated_artifact_sha256
        || sha256_hex(derived_socket_glb) != derived_socket_artifact_sha256
    {
        return invalid("animated GLB socket animation replay artifact hashes differ");
    }
    let (source_root, _) = parse_glb(source_animated_glb)?;
    let (derived_root, _) = parse_glb(derived_socket_glb)?;
    let source_animation = source_root
        .get("animations")
        .ok_or_else(|| error("animated GLB socket source animations are unavailable"))?;
    let derived_animation = derived_root
        .get("animations")
        .ok_or_else(|| error("animated GLB socket derived animations are unavailable"))?;
    if source_animation != derived_animation {
        return invalid("animated GLB socket materialization changed animation channels");
    }
    let source_object = source_animation
        .as_array()
        .and_then(|animations| animations.first())
        .and_then(Value::as_object)
        .ok_or_else(|| error("animated GLB socket source animation is invalid"))?;
    let derived_object = derived_animation
        .as_array()
        .and_then(|animations| animations.first())
        .and_then(Value::as_object)
        .ok_or_else(|| error("animated GLB socket derived animation is invalid"))?;
    if source_object.get("channels") != derived_object.get("channels")
        || source_object.get("samplers") != derived_object.get("samplers")
    {
        return invalid("animated GLB socket materialization changed samplers or channels");
    }
    inspection.source_animation_projection_sha256 = canonical_json_hash(source_animation);
    inspection.derived_animation_projection_sha256 = canonical_json_hash(derived_animation);
    inspection.derived_animation_validation_sha256 = canonical_json_hash(&json!({
        "schema_version":"GameWeaponAnimatedGlbSocketDerivedAnimationValidation@1",
        "source_animated_artifact_sha256":source_animated_artifact_sha256,
        "derived_socket_artifact_sha256":derived_socket_artifact_sha256,
        "source_animation_projection_sha256":inspection.source_animation_projection_sha256,
        "derived_animation_projection_sha256":inspection.derived_animation_projection_sha256,
        "sample_time_ticks":inspection.sample_time_ticks,
        "part_ids":inspection.part_ids,
        "sampler_count":inspection.sampler_count,
        "channel_count":inspection.channel_count,
        "accessor_count_added":inspection.accessor_count_added,
        "buffer_view_count_added":inspection.buffer_view_count_added,
        "animations_preserved":true,
        "channels_preserved":true,
        "samplers_preserved":true
    }));
    Ok(())
}

fn glb_has_morph_targets(root: &Value) -> bool {
    root.get("meshes")
        .and_then(Value::as_array)
        .is_some_and(|meshes| {
            meshes.iter().any(|mesh| {
                mesh.get("weights").is_some()
                    || mesh
                        .get("primitives")
                        .and_then(Value::as_array)
                        .is_some_and(|primitives| {
                            primitives
                                .iter()
                                .any(|primitive| primitive.get("targets").is_some())
                        })
            })
        })
        || root
            .get("nodes")
            .and_then(Value::as_array)
            .is_some_and(|nodes| nodes.iter().any(|node| node.get("weights").is_some()))
}

fn animated_socket_readback_sha256(
    socket_key_sha256: &str,
    project_id: &str,
    candidate_id: &str,
    candidate_state_sha256: &str,
    source_artifact_sha256: &str,
    animated_artifact_sha256: &str,
    anchor_set_object_sha256: &str,
    anchor_set_canonical_sha256: &str,
    materialized: &game_asset_delivery::MaterializedSocketGlb,
    inspection: &AnimatedSourceInspection,
    derived_artifact_sha256: &str,
) -> Result<String, RuntimeError> {
    let readback = json!({
        "schema_version":"GameWeaponAnimatedGlbSocketMaterializationReadback@1",
        "animated_socket_materialization_key_sha256":socket_key_sha256,
        "project_id":project_id,
        "candidate_id":candidate_id,
        "candidate_state_sha256":candidate_state_sha256,
        "source_artifact_sha256":source_artifact_sha256,
        "animated_artifact_sha256":animated_artifact_sha256,
        "anchor_set_object_sha256":anchor_set_object_sha256,
        "anchor_set_canonical_sha256":anchor_set_canonical_sha256,
        "source_renderable_inventory_sha256":materialized.source_renderable_inventory_sha256,
        "derived_renderable_inventory_sha256":materialized.derived_renderable_inventory_sha256,
        "socket_node_inventory_sha256":materialized.socket_node_inventory_sha256,
        "source_bin_sha256":materialized.source_bin_sha256,
        "derived_bin_sha256":materialized.derived_bin_sha256,
        "source_animation_projection_sha256":inspection.source_animation_projection_sha256,
        "derived_animation_projection_sha256":inspection.derived_animation_projection_sha256,
        "source_animation_validation_sha256":inspection.source_animation_validation_sha256,
        "derived_animation_validation_sha256":inspection.derived_animation_validation_sha256,
        "sample_time_ticks":inspection.sample_time_ticks,
        "part_ids":inspection.part_ids,
        "sampler_count":inspection.sampler_count,
        "channel_count":inspection.channel_count,
        "source_node_count":materialized.source_node_count,
        "derived_node_count":materialized.derived_node_count,
        "accessor_count_added":inspection.accessor_count_added,
        "buffer_view_count_added":inspection.buffer_view_count_added,
        "socket_node_count":materialized.socket_nodes.len(),
        "socket_nodes":materialized.socket_nodes,
        "derived_animated_socket_artifact_sha256":derived_artifact_sha256,
        "derived_animated_socket_artifact_readback_sha256":"",
        "canonical_sha256":""
    });
    Ok(canonical_json_hash(&readback))
}

fn animated_socket_receipt(
    socket_key_sha256: &str,
    project_id: &str,
    candidate_id: &str,
    candidate_state_sha256: &str,
    delivery_manifest_object_sha256: &str,
    lod0_artifact_sha256: &str,
    animated_artifact_sha256: &str,
    animation_receipt_object_sha256: &str,
    animation_receipt_canonical_sha256: &str,
    anchor_set_object_sha256: &str,
    anchor_set_canonical_sha256: &str,
    node_encoding_sha256: &str,
    materialized: &game_asset_delivery::MaterializedSocketGlb,
    inspection: &AnimatedSourceInspection,
    derived_artifact_sha256: &str,
    derived_readback_sha256: &str,
    source_animation_validation_sha256: &str,
    source_artifact_readback_sha256: &str,
    animated_artifact_readback_sha256: &str,
) -> Result<Value, RuntimeError> {
    let mut map = Map::new();
    map.insert(
        "schema_version".to_owned(),
        json!(ANIMATED_SOCKET_RECEIPT_SCHEMA),
    );
    map.insert(
        "animated_socket_materialization_key_sha256".to_owned(),
        json!(socket_key_sha256),
    );
    map.insert("project_id".to_owned(), json!(project_id));
    map.insert("candidate_id".to_owned(), json!(candidate_id));
    map.insert(
        "candidate_state_sha256".to_owned(),
        json!(candidate_state_sha256),
    );
    map.insert(
        "delivery_manifest_object_sha256".to_owned(),
        json!(delivery_manifest_object_sha256),
    );
    map.insert(
        "lod0_artifact_sha256".to_owned(),
        json!(lod0_artifact_sha256),
    );
    map.insert(
        "source_artifact_sha256".to_owned(),
        json!(lod0_artifact_sha256),
    );
    map.insert(
        "source_artifact_readback_sha256".to_owned(),
        json!(source_artifact_readback_sha256),
    );
    map.insert(
        "animated_artifact_sha256".to_owned(),
        json!(animated_artifact_sha256),
    );
    map.insert(
        "animated_artifact_readback_sha256".to_owned(),
        json!(animated_artifact_readback_sha256),
    );
    map.insert(
        "animation_receipt_object_sha256".to_owned(),
        json!(animation_receipt_object_sha256),
    );
    map.insert(
        "animation_receipt_canonical_sha256".to_owned(),
        json!(animation_receipt_canonical_sha256),
    );
    map.insert(
        "anchor_set_object_sha256".to_owned(),
        json!(anchor_set_object_sha256),
    );
    map.insert(
        "anchor_set_canonical_sha256".to_owned(),
        json!(anchor_set_canonical_sha256),
    );
    map.insert(
        "derived_animated_socket_artifact_sha256".to_owned(),
        json!(derived_artifact_sha256),
    );
    map.insert("request_sha256".to_owned(), json!(socket_key_sha256));
    map.insert(
        "socket_materialization_policy".to_owned(),
        json!(ANIMATED_SOCKET_POLICY),
    );
    map.insert("lod_scope".to_owned(), json!(ANIMATED_SOCKET_LOD_SCOPE));
    map.insert(
        "socket_node_id_encoding_sha256".to_owned(),
        json!(node_encoding_sha256),
    );
    map.insert(
        "source_animation_projection_sha256".to_owned(),
        json!(inspection.source_animation_projection_sha256),
    );
    map.insert(
        "derived_animation_projection_sha256".to_owned(),
        json!(inspection.derived_animation_projection_sha256),
    );
    map.insert(
        "source_animation_validation_sha256".to_owned(),
        json!(source_animation_validation_sha256),
    );
    map.insert(
        "derived_animation_validation_sha256".to_owned(),
        json!(inspection.derived_animation_validation_sha256),
    );
    map.insert(
        "source_renderable_inventory_sha256".to_owned(),
        json!(materialized.source_renderable_inventory_sha256),
    );
    map.insert(
        "derived_renderable_inventory_sha256".to_owned(),
        json!(materialized.derived_renderable_inventory_sha256),
    );
    map.insert(
        "source_bin_sha256".to_owned(),
        json!(materialized.source_bin_sha256),
    );
    map.insert(
        "derived_bin_sha256".to_owned(),
        json!(materialized.derived_bin_sha256),
    );
    map.insert(
        "sample_time_ticks".to_owned(),
        json!(inspection.sample_time_ticks),
    );
    map.insert("part_ids".to_owned(), json!(inspection.part_ids));
    map.insert("sampler_count".to_owned(), json!(inspection.sampler_count));
    map.insert("channel_count".to_owned(), json!(inspection.channel_count));
    map.insert(
        "node_count".to_owned(),
        json!(inspection.derived_node_count),
    );
    map.insert(
        "source_node_count".to_owned(),
        json!(materialized.source_node_count),
    );
    map.insert(
        "derived_node_count".to_owned(),
        json!(materialized.derived_node_count),
    );
    map.insert(
        "accessor_count_added".to_owned(),
        json!(inspection.accessor_count_added),
    );
    map.insert(
        "buffer_view_count_added".to_owned(),
        json!(inspection.buffer_view_count_added),
    );
    map.insert(
        "derived_animated_socket_artifact_readback_sha256".to_owned(),
        json!(derived_readback_sha256),
    );
    map.insert(
        "socket_node_inventory_sha256".to_owned(),
        json!(materialized.socket_node_inventory_sha256),
    );
    map.insert(
        "socket_node_count".to_owned(),
        json!(materialized.socket_nodes.len()),
    );
    map.insert("socket_nodes".to_owned(), json!(materialized.socket_nodes));
    map.insert(
        "owned_cas_kinds".to_owned(),
        json!([ANIMATED_SOCKET_GLB_KIND, ANIMATED_SOCKET_RECEIPT_KIND]),
    );
    for field in [
        "animations_preserved",
        "channels_preserved",
        "samplers_preserved",
        "renderable_projection_exact",
        "bin_byte_exact",
        "source_static_projection_exact",
        "no_skinning",
        "no_morph_targets",
        "socket_nodes_materialized",
        "runtime_write_performed",
        "restart_hash_verified",
    ] {
        map.insert(field.to_owned(), Value::Bool(true));
    }
    map.insert("candidate_confirmed".to_owned(), Value::Bool(false));
    map.insert("export_performed".to_owned(), Value::Bool(false));
    map.insert("actual_engine_roundtrip".to_owned(), Value::Bool(false));
    map.insert(
        "semantic_scope".to_owned(),
        json!(ANIMATED_SOCKET_SEMANTIC_SCOPE),
    );
    map.insert("functional_semantics".to_owned(), Value::Bool(false));
    map.insert(
        "materialization_status".to_owned(),
        json!(ANIMATED_SOCKET_STATUS),
    );
    map.insert("quality_status".to_owned(), json!("structural_only"));
    map.insert("limitations".to_owned(), json!(ANIMATED_SOCKET_LIMITATIONS));
    map.insert("canonical_sha256".to_owned(), Value::String(String::new()));
    map.insert("created_at".to_owned(), json!(now_string()));
    let mut receipt = Value::Object(map);
    receipt["canonical_sha256"] = Value::String(canonical_json_hash(&receipt));
    Ok(receipt)
}

fn verify_animated_socket_receipt(receipt: &Value) -> Result<(), RuntimeError> {
    let object = exact_object(
        receipt,
        &[
            "schema_version",
            "animated_socket_materialization_key_sha256",
            "project_id",
            "candidate_id",
            "candidate_state_sha256",
            "delivery_manifest_object_sha256",
            "lod0_artifact_sha256",
            "source_artifact_sha256",
            "source_artifact_readback_sha256",
            "animated_artifact_sha256",
            "animated_artifact_readback_sha256",
            "animation_receipt_object_sha256",
            "animation_receipt_canonical_sha256",
            "anchor_set_object_sha256",
            "anchor_set_canonical_sha256",
            "derived_animated_socket_artifact_sha256",
            "request_sha256",
            "socket_materialization_policy",
            "lod_scope",
            "socket_node_id_encoding_sha256",
            "source_animation_projection_sha256",
            "derived_animation_projection_sha256",
            "source_animation_validation_sha256",
            "derived_animation_validation_sha256",
            "source_renderable_inventory_sha256",
            "derived_renderable_inventory_sha256",
            "source_bin_sha256",
            "derived_bin_sha256",
            "sample_time_ticks",
            "part_ids",
            "sampler_count",
            "channel_count",
            "node_count",
            "source_node_count",
            "derived_node_count",
            "accessor_count_added",
            "buffer_view_count_added",
            "derived_animated_socket_artifact_readback_sha256",
            "socket_node_inventory_sha256",
            "socket_node_count",
            "socket_nodes",
            "owned_cas_kinds",
            "animations_preserved",
            "channels_preserved",
            "samplers_preserved",
            "renderable_projection_exact",
            "bin_byte_exact",
            "source_static_projection_exact",
            "no_skinning",
            "no_morph_targets",
            "socket_nodes_materialized",
            "runtime_write_performed",
            "restart_hash_verified",
            "candidate_confirmed",
            "export_performed",
            "actual_engine_roundtrip",
            "semantic_scope",
            "functional_semantics",
            "materialization_status",
            "quality_status",
            "limitations",
            "canonical_sha256",
            "created_at",
        ],
        ANIMATED_SOCKET_RECEIPT_SCHEMA,
    )?;
    if text(object, "schema_version")? != ANIMATED_SOCKET_RECEIPT_SCHEMA
        || text(object, "socket_materialization_policy")? != ANIMATED_SOCKET_POLICY
        || text(object, "lod_scope")? != ANIMATED_SOCKET_LOD_SCOPE
        || text(object, "semantic_scope")? != ANIMATED_SOCKET_SEMANTIC_SCOPE
        || text(object, "materialization_status")? != ANIMATED_SOCKET_STATUS
        || text(object, "quality_status")? != "structural_only"
        || object.get("functional_semantics").and_then(Value::as_bool) != Some(false)
        || object.get("animations_preserved").and_then(Value::as_bool) != Some(true)
        || object.get("channels_preserved").and_then(Value::as_bool) != Some(true)
        || object.get("samplers_preserved").and_then(Value::as_bool) != Some(true)
        || object
            .get("renderable_projection_exact")
            .and_then(Value::as_bool)
            != Some(true)
        || object.get("bin_byte_exact").and_then(Value::as_bool) != Some(true)
        || object
            .get("source_static_projection_exact")
            .and_then(Value::as_bool)
            != Some(true)
        || object.get("no_skinning").and_then(Value::as_bool) != Some(true)
        || object.get("no_morph_targets").and_then(Value::as_bool) != Some(true)
        || object
            .get("socket_nodes_materialized")
            .and_then(Value::as_bool)
            != Some(true)
        || object
            .get("runtime_write_performed")
            .and_then(Value::as_bool)
            != Some(true)
        || object.get("restart_hash_verified").and_then(Value::as_bool) != Some(true)
        || object.get("candidate_confirmed").and_then(Value::as_bool) != Some(false)
        || object.get("export_performed").and_then(Value::as_bool) != Some(false)
        || object
            .get("actual_engine_roundtrip")
            .and_then(Value::as_bool)
            != Some(false)
        || object.get("limitations") != Some(&json!(ANIMATED_SOCKET_LIMITATIONS))
        || object.get("owned_cas_kinds")
            != Some(&json!([
                ANIMATED_SOCKET_GLB_KIND,
                ANIMATED_SOCKET_RECEIPT_KIND
            ]))
        || object.get("socket_node_count").and_then(Value::as_u64) != Some(6)
    {
        return invalid("animated GLB socket receipt semantics differ");
    }
    for field in [
        "animated_socket_materialization_key_sha256",
        "candidate_state_sha256",
        "delivery_manifest_object_sha256",
        "lod0_artifact_sha256",
        "source_artifact_sha256",
        "source_artifact_readback_sha256",
        "animated_artifact_sha256",
        "animated_artifact_readback_sha256",
        "animation_receipt_object_sha256",
        "animation_receipt_canonical_sha256",
        "anchor_set_object_sha256",
        "anchor_set_canonical_sha256",
        "derived_animated_socket_artifact_sha256",
        "request_sha256",
        "socket_node_id_encoding_sha256",
        "source_animation_projection_sha256",
        "derived_animation_projection_sha256",
        "source_animation_validation_sha256",
        "derived_animation_validation_sha256",
        "source_renderable_inventory_sha256",
        "derived_renderable_inventory_sha256",
        "source_bin_sha256",
        "derived_bin_sha256",
        "derived_animated_socket_artifact_readback_sha256",
        "socket_node_inventory_sha256",
    ] {
        if !object
            .get(field)
            .and_then(Value::as_str)
            .is_some_and(forgecad_contracts::is_sha256)
        {
            return invalid(format!(
                "animated GLB socket receipt field {field} is invalid"
            ));
        }
    }
    verify_canonical(receipt)
}

fn validate_animated_socket_receipt_binding(
    receipt: &Value,
    link: &GameWeaponAnimatedGlbSocketMaterializationLinkRecord,
    materialized: &game_asset_delivery::MaterializedSocketGlb,
    inspection: &AnimatedSourceInspection,
) -> Result<(), RuntimeError> {
    verify_animated_socket_receipt(receipt)?;
    let object = receipt
        .as_object()
        .ok_or_else(|| error("animated GLB socket receipt is not an object"))?;
    let pairs = [
        (
            "animated_socket_materialization_key_sha256",
            link.animated_socket_materialization_key_sha256.as_str(),
        ),
        ("project_id", link.project_id.as_str()),
        ("candidate_id", link.candidate_id.as_str()),
        (
            "candidate_state_sha256",
            link.candidate_state_sha256.as_str(),
        ),
        (
            "delivery_manifest_object_sha256",
            link.delivery_manifest_object_sha256.as_str(),
        ),
        ("lod0_artifact_sha256", link.lod0_artifact_sha256.as_str()),
        (
            "source_artifact_sha256",
            link.source_artifact_sha256.as_str(),
        ),
        (
            "source_artifact_readback_sha256",
            link.source_artifact_readback_sha256.as_str(),
        ),
        (
            "animated_artifact_sha256",
            link.animated_artifact_sha256.as_str(),
        ),
        (
            "animated_artifact_readback_sha256",
            link.animated_artifact_readback_sha256.as_str(),
        ),
        (
            "animation_receipt_object_sha256",
            link.animation_receipt_object_sha256.as_str(),
        ),
        (
            "animation_receipt_canonical_sha256",
            link.animation_receipt_canonical_sha256.as_str(),
        ),
        (
            "anchor_set_object_sha256",
            link.anchor_set_object_sha256.as_str(),
        ),
        (
            "anchor_set_canonical_sha256",
            link.anchor_set_canonical_sha256.as_str(),
        ),
        (
            "derived_animated_socket_artifact_sha256",
            link.derived_animated_socket_artifact_sha256.as_str(),
        ),
        ("request_sha256", link.request_sha256.as_str()),
        (
            "socket_node_id_encoding_sha256",
            link.socket_node_id_encoding_sha256.as_str(),
        ),
    ];
    if pairs
        .iter()
        .any(|(field, expected)| object.get(*field).and_then(Value::as_str) != Some(*expected))
    {
        return invalid("animated GLB socket receipt binding differs from durable Link");
    }
    if object
        .get("source_renderable_inventory_sha256")
        .and_then(Value::as_str)
        != Some(materialized.source_renderable_inventory_sha256.as_str())
        || object
            .get("derived_renderable_inventory_sha256")
            .and_then(Value::as_str)
            != Some(materialized.derived_renderable_inventory_sha256.as_str())
        || object
            .get("socket_node_inventory_sha256")
            .and_then(Value::as_str)
            != Some(materialized.socket_node_inventory_sha256.as_str())
        || object.get("source_bin_sha256").and_then(Value::as_str)
            != Some(materialized.source_bin_sha256.as_str())
        || object.get("derived_bin_sha256").and_then(Value::as_str)
            != Some(materialized.derived_bin_sha256.as_str())
        || object.get("source_node_count").and_then(Value::as_u64)
            != Some(materialized.source_node_count as u64)
        || object.get("derived_node_count").and_then(Value::as_u64)
            != Some(materialized.derived_node_count as u64)
        || object.get("sampler_count").and_then(Value::as_u64)
            != Some(inspection.sampler_count as u64)
        || object.get("channel_count").and_then(Value::as_u64)
            != Some(inspection.channel_count as u64)
        || object.get("accessor_count_added").and_then(Value::as_u64)
            != Some(inspection.accessor_count_added as u64)
        || object
            .get("buffer_view_count_added")
            .and_then(Value::as_u64)
            != Some(inspection.buffer_view_count_added as u64)
        || object.get("socket_nodes") != Some(&Value::Array(materialized.socket_nodes.clone()))
    {
        return invalid("animated GLB socket receipt readback differs from derived bytes");
    }
    Ok(())
}

fn materialize(
    source_glb: &[u8],
    source_sha256: &str,
    clip_sha256: &str,
    part_ids: &[String],
    ticks: &[u64],
    frames: &[BTreeMap<String, RigidTransform>],
) -> Result<Vec<u8>, RuntimeError> {
    let (mut root, mut binary) = parse_glb(source_glb)?;
    if root.get("animations").is_some() || root.get("skins").is_some() {
        return invalid("source GLB must be static and unskinned");
    }
    let nodes = root
        .get("nodes")
        .and_then(Value::as_array)
        .ok_or_else(|| error("source GLB nodes are missing"))?;
    let mut node_by_part = BTreeMap::new();
    for (index, node) in nodes.iter().enumerate() {
        let Some(name) = node.get("name").and_then(Value::as_str) else {
            continue;
        };
        if part_ids.iter().any(|part| part == name)
            && node_by_part.insert(name.to_owned(), index).is_some()
        {
            return invalid("source GLB has duplicate Part node owners");
        }
    }
    if node_by_part.keys().cloned().collect::<Vec<_>>() != part_ids {
        return invalid("source GLB does not have exact-one node owner for every animated Part");
    }
    let source_accessor_count = root["accessors"]
        .as_array()
        .ok_or_else(|| error("source GLB accessors are missing"))?
        .len();
    let source_view_count = root["bufferViews"]
        .as_array()
        .ok_or_else(|| error("source GLB bufferViews are missing"))?
        .len();
    let times = ticks
        .iter()
        .map(|tick| *tick as f32 / 1000.0)
        .collect::<Vec<_>>();
    let time_accessor = append_accessor(
        &mut root,
        &mut binary,
        &times.iter().flat_map(|value| [*value]).collect::<Vec<_>>(),
        ticks.len(),
        "SCALAR",
        Some(json!([times[0]])),
        Some(json!([times[times.len() - 1]])),
    )?;
    let mut samplers = Vec::new();
    let mut channels = Vec::new();
    for part_id in part_ids {
        let translations = frames
            .iter()
            .flat_map(|frame| frame[part_id].translation)
            .collect::<Vec<_>>();
        let rotations = frames
            .iter()
            .flat_map(|frame| frame[part_id].rotation)
            .collect::<Vec<_>>();
        let translation_accessor = append_accessor(
            &mut root,
            &mut binary,
            &translations,
            ticks.len(),
            "VEC3",
            None,
            None,
        )?;
        let rotation_accessor = append_accessor(
            &mut root,
            &mut binary,
            &rotations,
            ticks.len(),
            "VEC4",
            None,
            None,
        )?;
        let node = node_by_part[part_id];
        let translation_sampler = samplers.len();
        samplers.push(
            json!({"input":time_accessor,"output":translation_accessor,"interpolation":"LINEAR"}),
        );
        channels.push(
            json!({"sampler":translation_sampler,"target":{"node":node,"path":"translation"}}),
        );
        let rotation_sampler = samplers.len();
        samplers.push(
            json!({"input":time_accessor,"output":rotation_accessor,"interpolation":"LINEAR"}),
        );
        channels.push(json!({"sampler":rotation_sampler,"target":{"node":node,"path":"rotation"}}));
    }
    root["animations"] =
        json!([{"name":"ForgeCAD rigid mechanical clip","samplers":samplers,"channels":channels}]);
    root["buffers"][0]["byteLength"] = Value::from(binary.len() as u64);
    let mut metadata = json!({
        "schema_version":"RigidAnimationGlb@1",
        "source_artifact_sha256":source_sha256,
        "clip_sha256":clip_sha256,
        "part_ids":part_ids,
        "sample_time_ticks":ticks,
        "timebase_hz":1000,
        "interpolation":"LINEAR",
        "source_accessor_count":source_accessor_count,
        "source_buffer_view_count":source_view_count,
        "canonical_sha256":""
    });
    metadata["canonical_sha256"] = Value::String(canonical_json_hash(&metadata));
    root["extras"]["forgecad"]["rigid_animation"] = metadata;
    encode_glb(&root, &binary)
}

fn append_accessor(
    root: &mut Value,
    binary: &mut Vec<u8>,
    values: &[f32],
    count: usize,
    kind: &str,
    min: Option<Value>,
    max: Option<Value>,
) -> Result<usize, RuntimeError> {
    while binary.len() % 4 != 0 {
        binary.push(0);
    }
    let offset = binary.len();
    for value in values {
        if !value.is_finite() {
            return invalid("animation accessor contains a non-finite value");
        }
        binary.extend_from_slice(&value.to_le_bytes());
    }
    let views = root["bufferViews"]
        .as_array_mut()
        .ok_or_else(|| error("source bufferViews are invalid"))?;
    let view_index = views.len();
    views.push(json!({"buffer":0,"byteOffset":offset,"byteLength":values.len() * 4}));
    let accessors = root["accessors"]
        .as_array_mut()
        .ok_or_else(|| error("source accessors are invalid"))?;
    let accessor_index = accessors.len();
    let mut accessor =
        json!({"bufferView":view_index,"componentType":5126,"count":count,"type":kind});
    if let Some(min) = min {
        accessor["min"] = min;
    }
    if let Some(max) = max {
        accessor["max"] = max;
    }
    accessors.push(accessor);
    Ok(accessor_index)
}

fn validate_animated_glb(
    source_glb: &[u8],
    animated_glb: &[u8],
    source_sha256: &str,
    clip_sha256: &str,
    part_ids: &[String],
    ticks: &[u64],
    frames: &[BTreeMap<String, RigidTransform>],
) -> Result<Value, RuntimeError> {
    if animated_glb.len() > MAX_GLB_BYTES || sha256_hex(source_glb) != source_sha256 {
        return invalid("animated GLB size or source binding is invalid");
    }
    let (source_root, source_binary) = parse_glb(source_glb)?;
    let (root, binary) = parse_glb(animated_glb)?;
    let metadata = root["extras"]["forgecad"]["rigid_animation"]
        .as_object()
        .ok_or_else(|| error("animated GLB metadata is missing"))?;
    verify_canonical(&Value::Object(metadata.clone()))?;
    if metadata
        .get("source_artifact_sha256")
        .and_then(Value::as_str)
        != Some(source_sha256)
        || metadata.get("clip_sha256").and_then(Value::as_str) != Some(clip_sha256)
        || root.get("skins").is_some()
        || root
            .get("animations")
            .and_then(Value::as_array)
            .is_none_or(|items| items.len() != 1)
        || binary.get(..source_binary.len()) != Some(source_binary.as_slice())
    {
        return invalid("animated GLB source projection or closed animation identity differs");
    }
    let source_accessors = source_root["accessors"].as_array().unwrap().len();
    let source_views = source_root["bufferViews"].as_array().unwrap().len();
    let mut projected = root.clone();
    projected.as_object_mut().unwrap().remove("animations");
    projected["accessors"]
        .as_array_mut()
        .unwrap()
        .truncate(source_accessors);
    projected["bufferViews"]
        .as_array_mut()
        .unwrap()
        .truncate(source_views);
    projected["buffers"][0]["byteLength"] = source_root["buffers"][0]["byteLength"].clone();
    projected["extras"]["forgecad"]
        .as_object_mut()
        .unwrap()
        .remove("rigid_animation");
    if projected != source_root {
        return invalid("animated GLB cannot reconstruct the exact source static projection");
    }
    let animation = &root["animations"][0];
    let animation_object = exact_object(
        animation,
        &["name", "samplers", "channels"],
        "rigid glTF animation",
    )?;
    if animation_object.get("name").and_then(Value::as_str)
        != Some("ForgeCAD rigid mechanical clip")
    {
        return invalid("animation name differs");
    }
    let samplers = animation["samplers"]
        .as_array()
        .ok_or_else(|| error("animation samplers are invalid"))?;
    let channels = animation["channels"]
        .as_array()
        .ok_or_else(|| error("animation channels are invalid"))?;
    if samplers.len() != part_ids.len() * 2 || channels.len() != part_ids.len() * 2 {
        return invalid("animation sampler/channel count differs from Part coverage");
    }
    let accessors = root["accessors"].as_array().unwrap();
    let views = root["bufferViews"].as_array().unwrap();
    let expected_times = ticks
        .iter()
        .map(|tick| *tick as f32 / 1000.0)
        .collect::<Vec<_>>();
    let mut seen = BTreeSet::new();
    let mut seen_samplers = BTreeSet::new();
    let mut seen_accessors = BTreeSet::new();
    for channel in channels {
        exact_object(channel, &["sampler", "target"], "animation channel")?;
        exact_object(
            channel.get("target").unwrap_or(&Value::Null),
            &["node", "path"],
            "animation target",
        )?;
        let sampler_index = channel["sampler"]
            .as_u64()
            .ok_or_else(|| error("channel sampler is invalid"))?
            as usize;
        let sampler = samplers
            .get(sampler_index)
            .ok_or_else(|| error("channel sampler index overflowed"))?;
        exact_object(
            sampler,
            &["input", "output", "interpolation"],
            "animation sampler",
        )?;
        if !seen_samplers.insert(sampler_index) {
            return invalid("animation sampler is referenced more than once");
        }
        if sampler["interpolation"] != "LINEAR" {
            return invalid("animation interpolation is not LINEAR");
        }
        let node_index = channel["target"]["node"]
            .as_u64()
            .ok_or_else(|| error("channel target node is invalid"))?
            as usize;
        let part_id = root["nodes"][node_index]["name"]
            .as_str()
            .ok_or_else(|| error("animated Part node name is missing"))?;
        let path = channel["target"]["path"]
            .as_str()
            .ok_or_else(|| error("channel target path is invalid"))?;
        if !part_ids.iter().any(|part| part == part_id)
            || !matches!(path, "translation" | "rotation")
            || !seen.insert((part_id.to_owned(), path.to_owned()))
        {
            return invalid("animation channel target coverage differs");
        }
        let input_index = sampler["input"]
            .as_u64()
            .ok_or_else(|| error("animation input accessor is invalid"))?
            as usize;
        let output_index = sampler["output"]
            .as_u64()
            .ok_or_else(|| error("animation output accessor is invalid"))?
            as usize;
        if input_index != source_accessors || output_index <= source_accessors {
            return invalid("animation accessor ownership differs");
        }
        seen_accessors.insert(input_index);
        if !seen_accessors.insert(output_index) {
            return invalid("animation output accessor is reused");
        }
        let input = read_f32_accessor(
            accessors,
            views,
            &binary,
            input_index,
            "SCALAR",
            ticks.len(),
        )?;
        if input != expected_times {
            return invalid("animation time accessor differs from scheduled ticks");
        }
        let output = read_f32_accessor(
            accessors,
            views,
            &binary,
            output_index,
            if path == "translation" {
                "VEC3"
            } else {
                "VEC4"
            },
            ticks.len(),
        )?;
        let expected = if path == "translation" {
            frames
                .iter()
                .flat_map(|frame| frame[part_id].translation)
                .collect::<Vec<_>>()
        } else {
            frames
                .iter()
                .flat_map(|frame| frame[part_id].rotation)
                .collect::<Vec<_>>()
        };
        if output != expected {
            return invalid("animation output accessor differs from verified frame deltas");
        }
    }
    if seen.len() != part_ids.len() * 2 {
        return invalid("animation does not cover translation and rotation for every Part");
    }
    if seen_samplers.len() != samplers.len()
        || seen_accessors != (source_accessors..accessors.len()).collect::<BTreeSet<_>>()
    {
        return invalid("animation has unconsumed samplers or accessors");
    }
    validate_added_buffer_layout(views, source_views, &binary, source_binary.len())?;
    Ok(json!({
        "schema_version":"RigidAnimationGlbValidation@1",
        "source_artifact_sha256":source_sha256,
        "animated_artifact_sha256":sha256_hex(animated_glb),
        "clip_sha256":clip_sha256,
        "node_count":part_ids.len(),
        "sampler_count":samplers.len(),
        "channel_count":channels.len(),
        "accessor_count_added":accessors.len() - source_accessors,
        "buffer_view_count_added":views.len() - source_views,
        "frame_count":ticks.len(),
        "source_static_projection_exact":true,
        "binary_prefix_exact":true,
        "no_skinning":true,
        "no_morph_targets":true
    }))
}

fn read_f32_accessor(
    accessors: &[Value],
    views: &[Value],
    binary: &[u8],
    index: usize,
    expected_kind: &str,
    expected_count: usize,
) -> Result<Vec<f32>, RuntimeError> {
    let accessor = accessors
        .get(index)
        .and_then(Value::as_object)
        .ok_or_else(|| error("animation accessor is invalid"))?;
    if accessor.get("componentType").and_then(Value::as_u64) != Some(5126)
        || accessor.get("count").and_then(Value::as_u64) != Some(expected_count as u64)
        || accessor.get("type").and_then(Value::as_str) != Some(expected_kind)
        || accessor.get("byteOffset").is_some()
        || accessor.get("sparse").is_some()
    {
        return invalid("animation accessor shape or component type differs");
    }
    let view = views
        .get(
            accessor["bufferView"]
                .as_u64()
                .ok_or_else(|| error("animation accessor view is missing"))? as usize,
        )
        .and_then(Value::as_object)
        .ok_or_else(|| error("animation buffer view is invalid"))?;
    let offset = view.get("byteOffset").and_then(Value::as_u64).unwrap_or(0) as usize;
    let length = view
        .get("byteLength")
        .and_then(Value::as_u64)
        .ok_or_else(|| error("animation buffer view length is missing"))? as usize;
    let component_count = match expected_kind {
        "SCALAR" => 1,
        "VEC3" => 3,
        "VEC4" => 4,
        _ => return invalid("animation accessor kind is unsupported"),
    };
    if view.get("buffer").and_then(Value::as_u64) != Some(0)
        || view.get("byteStride").is_some()
        || length != expected_count * component_count * 4
    {
        return invalid("animation buffer view layout differs");
    }
    let end = offset
        .checked_add(length)
        .ok_or_else(|| error("animation buffer view overflowed"))?;
    let bytes = binary
        .get(offset..end)
        .ok_or_else(|| error("animation buffer view exceeds BIN"))?;
    if bytes.len() % 4 != 0 {
        return invalid("animation float buffer is misaligned");
    }
    let values = bytes
        .chunks_exact(4)
        .map(|chunk| f32::from_le_bytes(chunk.try_into().unwrap()))
        .collect::<Vec<_>>();
    if values.iter().any(|value| !value.is_finite()) {
        return invalid("animation accessor contains a non-finite value");
    }
    Ok(values)
}

fn validate_added_buffer_layout(
    views: &[Value],
    source_view_count: usize,
    binary: &[u8],
    source_binary_len: usize,
) -> Result<(), RuntimeError> {
    let mut cursor = source_binary_len;
    for view in views.iter().skip(source_view_count) {
        let object = exact_object(
            view,
            &["buffer", "byteOffset", "byteLength"],
            "animation buffer view",
        )?;
        let offset = object
            .get("byteOffset")
            .and_then(Value::as_u64)
            .ok_or_else(|| error("animation buffer view offset is invalid"))?
            as usize;
        let length = object
            .get("byteLength")
            .and_then(Value::as_u64)
            .ok_or_else(|| error("animation buffer view length is invalid"))?
            as usize;
        while cursor % 4 != 0 {
            if binary.get(cursor) != Some(&0) {
                return invalid("animation alignment padding is non-zero");
            }
            cursor += 1;
        }
        if object.get("buffer").and_then(Value::as_u64) != Some(0) || offset != cursor {
            return invalid("animation buffer views are not contiguous");
        }
        cursor = cursor
            .checked_add(length)
            .filter(|end| *end <= binary.len())
            .ok_or_else(|| error("animation buffer view exceeds BIN"))?;
    }
    if cursor != binary.len() {
        return invalid("animated GLB has hidden BIN tail bytes");
    }
    Ok(())
}

pub(super) fn parse_glb(bytes: &[u8]) -> Result<(Value, Vec<u8>), RuntimeError> {
    if bytes.len() < 28
        || &bytes[..4] != b"glTF"
        || u32::from_le_bytes(bytes[4..8].try_into().unwrap()) != 2
    {
        return invalid("GLB header is invalid");
    }
    let json_length = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    let json_end = 20usize
        .checked_add(json_length)
        .ok_or_else(|| error("GLB JSON length overflowed"))?;
    if json_end + 8 > bytes.len()
        || &bytes[16..20] != b"JSON"
        || &bytes[json_end + 4..json_end + 8] != b"BIN\0"
    {
        return invalid("GLB chunks are invalid");
    }
    let binary_length =
        u32::from_le_bytes(bytes[json_end..json_end + 4].try_into().unwrap()) as usize;
    let binary_start = json_end + 8;
    let binary_end = binary_start
        .checked_add(binary_length)
        .filter(|end| *end == bytes.len())
        .ok_or_else(|| error("GLB BIN length differs"))?;
    let root =
        serde_json::from_slice(&bytes[20..json_end]).map_err(|_| error("GLB JSON is invalid"))?;
    Ok((root, bytes[binary_start..binary_end].to_vec()))
}

fn encode_glb(root: &Value, binary: &[u8]) -> Result<Vec<u8>, RuntimeError> {
    let mut json_bytes = serde_json::to_vec(root).map_err(|source| error(source.to_string()))?;
    while json_bytes.len() % 4 != 0 {
        json_bytes.push(b' ');
    }
    let total = 12usize + 8 + json_bytes.len() + 8 + binary.len();
    if total > MAX_GLB_BYTES || total > u32::MAX as usize {
        return invalid("animated GLB exceeds its size budget");
    }
    let mut result = Vec::with_capacity(total);
    result.extend_from_slice(b"glTF");
    result.extend_from_slice(&2u32.to_le_bytes());
    result.extend_from_slice(&(total as u32).to_le_bytes());
    result.extend_from_slice(&(json_bytes.len() as u32).to_le_bytes());
    result.extend_from_slice(b"JSON");
    result.extend_from_slice(&json_bytes);
    result.extend_from_slice(&(binary.len() as u32).to_le_bytes());
    result.extend_from_slice(b"BIN\0");
    result.extend_from_slice(binary);
    Ok(result)
}

#[cfg(test)]
pub(super) fn inspect_for_test(bytes: &[u8]) -> Result<Value, RuntimeError> {
    parse_glb(bytes).map(|(root, _)| root)
}

fn parse_transform(object: &Map<String, Value>) -> Result<RigidTransform, RuntimeError> {
    let translation = array::<3>(object.get("translation_m"), "translation_m")?;
    let rotation = array::<4>(object.get("rotation_quat_xyzw"), "rotation_quat_xyzw")?;
    if translation.iter().any(|value| value.abs() > 10.0)
        || (rotation
            .iter()
            .map(|value| value * value)
            .sum::<f32>()
            .sqrt()
            - 1.0)
            .abs()
            > 1.0e-4
        || object.get("scale") != Some(&json!([1.0, 1.0, 1.0]))
    {
        return invalid("rigid transform is out of bounds, non-unit or scaled");
    }
    Ok(RigidTransform {
        translation,
        rotation,
    })
}

fn array<const N: usize>(value: Option<&Value>, field: &str) -> Result<[f32; N], RuntimeError> {
    let values = value
        .and_then(Value::as_array)
        .filter(|values| values.len() == N)
        .ok_or_else(|| error(format!("{field} is invalid")))?;
    let mut result = [0.0; N];
    for (index, value) in values.iter().enumerate() {
        result[index] = value
            .as_f64()
            .filter(|value| value.is_finite())
            .ok_or_else(|| error(format!("{field} contains a non-finite value")))?
            as f32;
    }
    Ok(result)
}

fn validate_receipt(value: &Value) -> Result<(), RuntimeError> {
    let object = value
        .as_object()
        .ok_or_else(|| error("animation receipt is invalid"))?;
    if text(object, "schema_version")? != "MechanicalAnimationGlbReceipt@1"
        || text(object, "validator_status")? != "strict-rigid-gltf-animation-readback-pass"
        || object.get("hard_gate_passed").and_then(Value::as_bool) != Some(true)
        || object
            .get("source_static_projection_exact")
            .and_then(Value::as_bool)
            != Some(true)
        || object.get("no_skinning").and_then(Value::as_bool) != Some(true)
        || object.get("no_morph_targets").and_then(Value::as_bool) != Some(true)
        || text(object, "materialization_status")? != "runtime-owned-cas-animated-glb"
        || object
            .get("runtime_write_performed")
            .and_then(Value::as_bool)
            != Some(true)
        || text(object, "quality_status")? != "structural_only"
    {
        return invalid("animation receipt policy differs");
    }
    verify_canonical(value)
}

fn verify_canonical(value: &Value) -> Result<(), RuntimeError> {
    let declared = value
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .filter(|value| forgecad_contracts::is_sha256(value))
        .ok_or_else(|| error("canonical_sha256 is invalid"))?;
    let mut preimage = value.clone();
    preimage["canonical_sha256"] = Value::String(String::new());
    if canonical_json_hash(&preimage) != declared {
        return invalid("canonical_sha256 differs");
    }
    Ok(())
}

fn verify_request_canonical(value: &Value) -> Result<(), RuntimeError> {
    let declared = value
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .filter(|value| forgecad_contracts::is_sha256(value))
        .ok_or_else(|| error("canonical_sha256 is invalid"))?;
    let mut preimage = value.clone();
    preimage
        .as_object_mut()
        .ok_or_else(|| error("request must be an object"))?
        .remove("canonical_sha256");
    if canonical_json_hash(&preimage) != declared {
        return invalid("canonical_sha256 differs");
    }
    Ok(())
}

fn exact_object<'a>(
    value: &'a Value,
    fields: &[&str],
    context: &str,
) -> Result<&'a Map<String, Value>, RuntimeError> {
    let object = value
        .as_object()
        .ok_or_else(|| error(format!("{context} must be an object")))?;
    if object.len() != fields.len()
        || fields.iter().any(|field| !object.contains_key(*field))
        || object.keys().any(|field| !fields.contains(&field.as_str()))
    {
        return invalid(format!("{context} field set differs"));
    }
    Ok(object)
}
fn text<'a>(object: &'a Map<String, Value>, field: &str) -> Result<&'a str, RuntimeError> {
    object
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| error(format!("{field} is invalid")))
}
fn identifier<'a>(object: &'a Map<String, Value>, field: &str) -> Result<&'a str, RuntimeError> {
    text(object, field).and_then(|value| {
        if valid_identifier(value) {
            Ok(value)
        } else {
            Err(error(format!("{field} is invalid")))
        }
    })
}
fn sha<'a>(object: &'a Map<String, Value>, field: &str) -> Result<&'a str, RuntimeError> {
    text(object, field).and_then(|value| {
        if forgecad_contracts::is_sha256(value) {
            Ok(value)
        } else {
            Err(error(format!("{field} is invalid")))
        }
    })
}
fn valid_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'-'))
        && value.as_bytes()[0].is_ascii_alphanumeric()
}
fn invalid<T>(detail: impl Into<String>) -> Result<T, RuntimeError> {
    Err(error(detail))
}
fn error(detail: impl Into<String>) -> RuntimeError {
    RuntimeError::InvalidInput(format!("{ERROR}: {}", detail.into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_anchor_set() -> Value {
        let anchors = [
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
        ]
        .into_iter()
        .map(|(anchor_id, role, parent_kind, owner_part_id)| {
            json!({
                "anchor_id":anchor_id,
                "role":role,
                "parent_kind":parent_kind,
                "owner_part_id":owner_part_id,
                "local_translation_m":[0.0,0.0,0.0],
                "local_rotation_quat_xyzw":[0.0,0.0,0.0,1.0],
                "local_scale_xyz":[1.0,1.0,1.0]
            })
        })
        .collect::<Vec<_>>();
        json!({
            "schema_version":"GameWeaponAnchorSet@1",
            "node_materialization":"sidecar-only-not-glb-nodes",
            "canonical_sha256":"a".repeat(64),
            "anchors":anchors
        })
    }

    fn fixture_glb(animation_target: usize) -> Vec<u8> {
        let root = json!({
            "asset":{"version":"2.0"},
            "scene":0,
            "scenes":[{"nodes":[0]}],
            "nodes":[{"name":"part-1","mesh":0}],
            "meshes":[{"primitives":[]}],
            "materials":[],
            "buffers":[{"byteLength":4}],
            "bufferViews":[],
            "accessors":[],
            "extras":{"forgecad":{}},
            "animations":[{
                "name":"fixture-rigid",
                "samplers":[{"input":0,"output":1,"interpolation":"LINEAR"}],
                "channels":[{"sampler":0,"target":{"node":animation_target,"path":"translation"}}]
            }]
        });
        encode_glb(&root, &[7, 11, 13, 17]).expect("fixture GLB encodes")
    }

    #[test]
    fn animated_socket_materializer_preserves_animation_and_bin_exactly() {
        let source = fixture_glb(0);
        let anchor_set = fixture_anchor_set();
        let anchor_ids = game_asset_delivery::socket_anchor_ids(&anchor_set)
            .expect("fixture AnchorSet is closed");
        let materialized = game_asset_delivery::materialize_socket_glb(
            &source,
            &sha256_hex(&source),
            &"b".repeat(64),
            &anchor_set,
            &["part-1".to_owned()],
            &anchor_ids,
        )
        .expect("fixture animated GLB materializes");
        let (source_root, source_bin) = parse_glb(&source).expect("source parses");
        let (derived_root, derived_bin) = parse_glb(&materialized.glb).expect("derived parses");
        assert_eq!(source_bin, derived_bin);
        assert_eq!(source_root["animations"], derived_root["animations"]);
        assert_eq!(source_root["meshes"], derived_root["meshes"]);
        assert_eq!(source_root["materials"], derived_root["materials"]);
        assert_eq!(source_root["accessors"], derived_root["accessors"]);
        assert_eq!(source_root["bufferViews"], derived_root["bufferViews"]);
        assert_eq!(source_root["buffers"], derived_root["buffers"]);
        assert_eq!(derived_root["nodes"].as_array().unwrap().len(), 7);
        assert_eq!(
            materialized.source_bin_sha256,
            materialized.derived_bin_sha256
        );

        let mut inspection = AnimatedSourceInspection {
            derived_node_count: 1,
            sampler_count: 1,
            channel_count: 1,
            accessor_count_added: 3,
            buffer_view_count_added: 3,
            sample_time_ticks: vec![0, 1000],
            part_ids: vec!["part-1".to_owned()],
            source_animation_projection_sha256: String::new(),
            derived_animation_projection_sha256: String::new(),
            source_animation_validation_sha256: "c".repeat(64),
            derived_animation_validation_sha256: String::new(),
        };
        validate_socket_animation_preservation(
            &source,
            &materialized.glb,
            &sha256_hex(&source),
            &sha256_hex(&materialized.glb),
            &mut inspection,
        )
        .expect("socket animation preservation validates");
        assert_eq!(
            inspection.source_animation_projection_sha256,
            inspection.derived_animation_projection_sha256
        );
        assert_ne!(
            inspection.derived_animation_validation_sha256,
            "c".repeat(64)
        );
    }

    #[test]
    fn animated_socket_materializer_rejects_future_animation_target() {
        let source = fixture_glb(1);
        let anchor_set = fixture_anchor_set();
        let anchor_ids = game_asset_delivery::socket_anchor_ids(&anchor_set)
            .expect("fixture AnchorSet is closed");
        let result = game_asset_delivery::materialize_socket_glb(
            &source,
            &sha256_hex(&source),
            &"b".repeat(64),
            &anchor_set,
            &["part-1".to_owned()],
            &anchor_ids,
        );
        assert!(result.is_err());
    }

    #[test]
    fn animated_socket_animation_channel_tamper_is_rejected() {
        let source = fixture_glb(0);
        let anchor_set = fixture_anchor_set();
        let anchor_ids = game_asset_delivery::socket_anchor_ids(&anchor_set)
            .expect("fixture AnchorSet is closed");
        let materialized = game_asset_delivery::materialize_socket_glb(
            &source,
            &sha256_hex(&source),
            &"b".repeat(64),
            &anchor_set,
            &["part-1".to_owned()],
            &anchor_ids,
        )
        .expect("fixture animated GLB materializes");
        let (mut tampered_root, binary) = parse_glb(&materialized.glb).expect("derived parses");
        tampered_root["animations"][0]["channels"][0]["target"]["path"] =
            Value::String("rotation".to_owned());
        let tampered = encode_glb(&tampered_root, &binary).expect("tampered GLB encodes");
        let mut inspection = AnimatedSourceInspection {
            derived_node_count: 1,
            sampler_count: 1,
            channel_count: 1,
            accessor_count_added: 3,
            buffer_view_count_added: 3,
            sample_time_ticks: vec![0, 1000],
            part_ids: vec!["part-1".to_owned()],
            source_animation_projection_sha256: String::new(),
            derived_animation_projection_sha256: String::new(),
            source_animation_validation_sha256: "c".repeat(64),
            derived_animation_validation_sha256: String::new(),
        };
        assert!(validate_socket_animation_preservation(
            &source,
            &tampered,
            &sha256_hex(&source),
            &sha256_hex(&tampered),
            &mut inspection,
        )
        .is_err());
    }

    #[test]
    fn animated_socket_receipt_rejects_owned_cas_kind_tamper() {
        let source = fixture_glb(0);
        let anchor_set = fixture_anchor_set();
        let anchor_ids = game_asset_delivery::socket_anchor_ids(&anchor_set)
            .expect("fixture AnchorSet is closed");
        let materialized = game_asset_delivery::materialize_socket_glb(
            &source,
            &sha256_hex(&source),
            &"b".repeat(64),
            &anchor_set,
            &["part-1".to_owned()],
            &anchor_ids,
        )
        .expect("fixture animated GLB materializes");
        let mut inspection = AnimatedSourceInspection {
            derived_node_count: 1,
            sampler_count: 1,
            channel_count: 1,
            accessor_count_added: 3,
            buffer_view_count_added: 3,
            sample_time_ticks: vec![0, 1000],
            part_ids: vec!["part-1".to_owned()],
            source_animation_projection_sha256: "1".repeat(64),
            derived_animation_projection_sha256: "2".repeat(64),
            source_animation_validation_sha256: "3".repeat(64),
            derived_animation_validation_sha256: "4".repeat(64),
        };
        validate_socket_animation_preservation(
            &source,
            &materialized.glb,
            &sha256_hex(&source),
            &sha256_hex(&materialized.glb),
            &mut inspection,
        )
        .expect("socket animation preservation validates");
        let readback = animated_socket_readback_sha256(
            &"5".repeat(64),
            "project",
            "candidate",
            &"6".repeat(64),
            &"7".repeat(64),
            &sha256_hex(&source),
            &"8".repeat(64),
            &"9".repeat(64),
            &materialized,
            &inspection,
            &sha256_hex(&materialized.glb),
        )
        .expect("readback hashes");
        let receipt = animated_socket_receipt(
            &"5".repeat(64),
            "project",
            "candidate",
            &"6".repeat(64),
            &"a".repeat(64),
            &"7".repeat(64),
            &sha256_hex(&source),
            &"b".repeat(64),
            &"f".repeat(64),
            &"8".repeat(64),
            &"9".repeat(64),
            &"a".repeat(64),
            &materialized,
            &inspection,
            &sha256_hex(&materialized.glb),
            &readback,
            &"c".repeat(64),
            &"d".repeat(64),
            &"e".repeat(64),
        )
        .expect("receipt builds");
        verify_animated_socket_receipt(&receipt).expect("receipt validates");
        let mut tampered = receipt;
        tampered["owned_cas_kinds"][0] = Value::String("geometry-glb".to_owned());
        tampered["canonical_sha256"] = Value::String(canonical_json_hash(&{
            let mut preimage = tampered.clone();
            preimage["canonical_sha256"] = Value::String(String::new());
            preimage
        }));
        assert!(verify_animated_socket_receipt(&tampered).is_err());
    }

    #[test]
    fn socket_transform_projection_sampler_is_exact_and_uses_quaternion_nlerp() {
        let translation = ProjectionChannel {
            node_index: 0,
            path: "translation".to_owned(),
            times_seconds: vec![0.0, 1.0],
            values: vec![vec![0.0, 0.0, 0.0], vec![1.0, 2.0, 3.0]],
        };
        assert_eq!(
            sample_projection_channel(&translation, 0).expect("exact first sample"),
            vec![0.0, 0.0, 0.0]
        );
        assert_eq!(
            sample_projection_channel(&translation, 1000).expect("exact last sample"),
            vec![1.0, 2.0, 3.0]
        );
        let midpoint = sample_projection_channel(&translation, 500).expect("linear midpoint");
        assert_eq!(midpoint, vec![0.5, 1.0, 1.5]);

        let rotation = ProjectionChannel {
            node_index: 0,
            path: "rotation".to_owned(),
            times_seconds: vec![0.0, 1.0],
            values: vec![vec![0.0, 0.0, 0.0, 1.0], vec![0.0, 0.0, 1.0, 0.0]],
        };
        let midpoint = sample_projection_channel(&rotation, 500).expect("quaternion midpoint");
        let expected = 2.0_f32.sqrt() / 2.0;
        assert!((midpoint[2] - expected).abs() < 1.0e-6);
        assert!((midpoint[3] - expected).abs() < 1.0e-6);
        let norm = midpoint
            .iter()
            .map(|component| component * component)
            .sum::<f32>();
        assert!((norm - 1.0).abs() < 1.0e-6);
    }

    #[test]
    fn socket_transform_projection_rejects_non_identity_or_nested_part_rest_graph() {
        let mut root = json!({
            "scene":0,
            "scenes":[{"nodes":[0]}],
            "nodes":[{"name":"part-1","translation":[0.1,0.0,0.0]}]
        });
        assert!(validate_projection_flat_source_root(&root, &["part-1".to_owned()]).is_err());
        root["nodes"][0]["translation"] = json!([0.0, 0.0, 0.0]);
        root["nodes"][0]["children"] = json!([]);
        assert!(validate_projection_flat_source_root(&root, &["part-1".to_owned()]).is_err());
        root["nodes"][0].as_object_mut().unwrap().remove("children");
        root["nodes"][0]["matrix"] =
            json!([1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0]);
        assert!(validate_projection_flat_source_root(&root, &["part-1".to_owned()]).is_err());
    }

    #[test]
    fn socket_transform_projection_inventory_matches_materializer_and_replay_normalizes_only_metadata(
    ) {
        let source = fixture_glb(0);
        let anchor_set = fixture_anchor_set();
        let anchor_ids = game_asset_delivery::socket_anchor_ids(&anchor_set)
            .expect("fixture AnchorSet is closed");
        let materialized = game_asset_delivery::materialize_socket_glb(
            &source,
            &sha256_hex(&source),
            &"b".repeat(64),
            &anchor_set,
            &["part-1".to_owned()],
            &anchor_ids,
        )
        .expect("fixture socket materializes");
        assert_eq!(
            projection_socket_inventory_hash(&"b".repeat(64), &anchor_set, &anchor_ids, 1,)
                .expect("inventory hash"),
            materialized.socket_node_inventory_sha256
        );

        let mut left = json!({
            "canonical_sha256":"a",
            "created_at":"old",
            "projection_key_sha256":"b",
            "frames":[{"canonical_sha256":"c","created_at":"old-frame","sample_time_ticks":0}]
        });
        let mut right = left.clone();
        right["canonical_sha256"] = json!("different");
        right["created_at"] = json!("new");
        right["frames"][0]["canonical_sha256"] = json!("different-frame");
        right["frames"][0]["created_at"] = json!("new-frame");
        assert!(projection_replay_equivalent(&left, &right));
        right["projection_key_sha256"] = json!("retargeted");
        assert!(!projection_replay_equivalent(&left, &right));
        left["frames"][0]["sample_time_ticks"] = json!(1);
        assert!(!projection_replay_equivalent(&left, &right));
    }
}
