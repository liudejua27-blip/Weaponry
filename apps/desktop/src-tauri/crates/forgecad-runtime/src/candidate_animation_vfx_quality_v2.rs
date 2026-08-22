//! Runtime-owned CandidateAnimationVfxQuality@2 composition.
//!
//! This is an additive successor to the V1 sidecar contract.  It accepts no
//! V1 VFX keys or caller-provided gate booleans.  The complete dependency
//! chain is re-read through the durable material-surface head, MaterialSurface
//! Quality@1 and Attachment@3 getters before the one owned report object is
//! reserved.

use super::{
    canonical_json_bytes, canonical_json_hash, exact_object, is_opaque_id, is_sha256, sha256_hex,
    CasObject, Runtime, RuntimeError,
};
use forgecad_contracts::{
    CandidateAnimationVfxQualityV2GetRequest, CandidateAnimationVfxQualityV2HardGate,
    CandidateAnimationVfxQualityV2PrepareRequest, CandidateAnimationVfxQualityV2Record,
    CandidateMaterialSurfaceQualityRecord, FictionalEnergyVfxAnimatedSocketAttachmentV3Record,
    ProductionStageHeadV2Record, ProductionStageTransitionV2Record,
};
use serde_json::{json, Map, Value};

const PREPARE_SCHEMA: &str = "CandidateAnimationVfxQualityPrepareRequest@2";
const GET_SCHEMA: &str = "CandidateAnimationVfxQualityGetRequest@2";
const PREPARE_RESULT_SCHEMA: &str = "CandidateAnimationVfxQualityPrepareResult@2";
const GET_RESULT_SCHEMA: &str = "CandidateAnimationVfxQualityGetResult@2";
const RECORD_SCHEMA: &str = "CandidateAnimationVfxQuality@2";
const REPORT_KIND: &str = "candidate-animation-vfx-quality-v2-report";
const REPORT_MIME: &str = "application/json";
const MAX_REPORT_BYTES: usize = 1024 * 1024;
const FRAME_COUNT: usize = 15;
const FRAME_SET_SCHEMA: &str = "CandidateAnimationVfxQualityAttachmentFrameSet@1";

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
    "geometry_candidate_id",
    "geometry_candidate_state_sha256",
    "geometry_delivery_manifest_object_sha256",
    "geometry_artifact_sha256",
    "appearance_candidate_id",
    "appearance_candidate_state_sha256",
    "appearance_delivery_manifest_object_sha256",
    "appearance_artifact_sha256",
    "geometry_preservation_projection_sha256",
    "geometry_preservation_status",
    "animated_socket_materialization_key_sha256",
    "animated_artifact_sha256",
    "animated_socket_anchor_set_object_sha256",
    "animated_socket_anchor_set_canonical_sha256",
    "appearance_anchor_set_object_sha256",
    "appearance_anchor_set_canonical_sha256",
    "anchor_binding_policy",
    "anchor_binding_sha256",
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
    "attachment_key_sha256",
    "attachment_canonical_sha256",
    "attachment_receipt_object_sha256",
    "attachment_receipt_canonical_sha256",
    "attachment_frame_count",
    "attachment_frame_set_sha256",
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
        "CANDIDATE_ANIMATION_VFX_QUALITY_V2_INVALID: {}",
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

fn check_ids(request: &CandidateAnimationVfxQualityV2PrepareRequest) -> Result<(), RuntimeError> {
    for (name, value) in [
        (
            "animation_vfx_quality_id",
            &request.animation_vfx_quality_id,
        ),
        ("project_id", &request.project_id),
        (
            "source_material_surface_transition_id",
            &request.source_material_surface_transition_id,
        ),
        (
            "source_material_surface_quality_id",
            &request.source_material_surface_quality_id,
        ),
        ("candidate_id", &request.candidate_id),
        ("geometry_candidate_id", &request.geometry_candidate_id),
        ("appearance_candidate_id", &request.appearance_candidate_id),
        ("animation_clip_id", &request.animation_clip_id),
        ("idempotency_key", &request.idempotency_key),
    ] {
        if !is_opaque_id(value) {
            return Err(invalid(format!("{name} is malformed")));
        }
    }
    Ok(())
}

fn check_hashes(
    request: &CandidateAnimationVfxQualityV2PrepareRequest,
) -> Result<(), RuntimeError> {
    for (name, value) in [
        (
            "source_material_surface_transition_sha256",
            &request.source_material_surface_transition_sha256,
        ),
        (
            "source_material_surface_head_canonical_sha256",
            &request.source_material_surface_head_canonical_sha256,
        ),
        (
            "source_material_surface_quality_report_object_sha256",
            &request.source_material_surface_quality_report_object_sha256,
        ),
        (
            "source_material_surface_quality_canonical_sha256",
            &request.source_material_surface_quality_canonical_sha256,
        ),
        (
            "geometry_candidate_state_sha256",
            &request.geometry_candidate_state_sha256,
        ),
        (
            "geometry_delivery_manifest_object_sha256",
            &request.geometry_delivery_manifest_object_sha256,
        ),
        (
            "geometry_artifact_sha256",
            &request.geometry_artifact_sha256,
        ),
        (
            "appearance_candidate_state_sha256",
            &request.appearance_candidate_state_sha256,
        ),
        (
            "appearance_delivery_manifest_object_sha256",
            &request.appearance_delivery_manifest_object_sha256,
        ),
        (
            "appearance_artifact_sha256",
            &request.appearance_artifact_sha256,
        ),
        (
            "geometry_preservation_projection_sha256",
            &request.geometry_preservation_projection_sha256,
        ),
        (
            "animated_socket_materialization_key_sha256",
            &request.animated_socket_materialization_key_sha256,
        ),
        (
            "animated_artifact_sha256",
            &request.animated_artifact_sha256,
        ),
        (
            "animated_socket_anchor_set_object_sha256",
            &request.animated_socket_anchor_set_object_sha256,
        ),
        (
            "animated_socket_anchor_set_canonical_sha256",
            &request.animated_socket_anchor_set_canonical_sha256,
        ),
        (
            "appearance_anchor_set_object_sha256",
            &request.appearance_anchor_set_object_sha256,
        ),
        (
            "appearance_anchor_set_canonical_sha256",
            &request.appearance_anchor_set_canonical_sha256,
        ),
        ("anchor_binding_sha256", &request.anchor_binding_sha256),
        (
            "animation_clip_object_sha256",
            &request.animation_clip_object_sha256,
        ),
        (
            "animation_clip_canonical_sha256",
            &request.animation_clip_canonical_sha256,
        ),
        (
            "animation_receipt_object_sha256",
            &request.animation_receipt_object_sha256,
        ),
        (
            "animation_receipt_canonical_sha256",
            &request.animation_receipt_canonical_sha256,
        ),
        ("projection_key_sha256", &request.projection_key_sha256),
        (
            "projection_object_sha256",
            &request.projection_object_sha256,
        ),
        (
            "projection_canonical_sha256",
            &request.projection_canonical_sha256,
        ),
        (
            "particle_sequence_key_sha256",
            &request.particle_sequence_key_sha256,
        ),
        (
            "particle_sequence_canonical_sha256",
            &request.particle_sequence_canonical_sha256,
        ),
        (
            "trail_sequence_key_sha256",
            &request.trail_sequence_key_sha256,
        ),
        (
            "trail_sequence_canonical_sha256",
            &request.trail_sequence_canonical_sha256,
        ),
        (
            "trail_bloom_sequence_key_sha256",
            &request.trail_bloom_sequence_key_sha256,
        ),
        (
            "trail_bloom_sequence_canonical_sha256",
            &request.trail_bloom_sequence_canonical_sha256,
        ),
        (
            "vfx_profile_object_sha256",
            &request.vfx_profile_object_sha256,
        ),
        (
            "vfx_profile_canonical_sha256",
            &request.vfx_profile_canonical_sha256,
        ),
        (
            "trail_bloom_profile_sha256",
            &request.trail_bloom_profile_sha256,
        ),
        (
            "socket_node_id_encoding_sha256",
            &request.socket_node_id_encoding_sha256,
        ),
        ("socket_roles_sha256", &request.socket_roles_sha256),
        ("camera_object_sha256", &request.camera_object_sha256),
        ("camera_identity_sha256", &request.camera_identity_sha256),
        ("render_profile_sha256", &request.render_profile_sha256),
        (
            "render_worker_build_cohort_sha256",
            &request.render_worker_build_cohort_sha256,
        ),
        ("sample_schedule_sha256", &request.sample_schedule_sha256),
        ("attachment_key_sha256", &request.attachment_key_sha256),
        (
            "attachment_canonical_sha256",
            &request.attachment_canonical_sha256,
        ),
        (
            "attachment_receipt_object_sha256",
            &request.attachment_receipt_object_sha256,
        ),
        (
            "attachment_receipt_canonical_sha256",
            &request.attachment_receipt_canonical_sha256,
        ),
        (
            "attachment_frame_set_sha256",
            &request.attachment_frame_set_sha256,
        ),
        (
            "animation_vfx_policy_sha256",
            &request.animation_vfx_policy_sha256,
        ),
        ("input_sha256", &request.input_sha256),
    ] {
        if !is_sha256(value) {
            return Err(invalid(format!("{name} is malformed")));
        }
    }
    Ok(())
}

fn parse_prepare(
    value: &Value,
) -> Result<(CandidateAnimationVfxQualityV2PrepareRequest, String), RuntimeError> {
    let object = exact_object(value, PREPARE_FIELDS, PREPARE_SCHEMA)?;
    if required_text(object, "schema_version")? != PREPARE_SCHEMA {
        return Err(invalid("prepare schema version differs"));
    }
    let request: CandidateAnimationVfxQualityV2PrepareRequest =
        serde_json::from_value(value.clone())
            .map_err(|error| invalid(format!("prepare request is malformed: {error}")))?;
    check_ids(&request)?;
    check_hashes(&request)?;
    if request.candidate_id != request.appearance_candidate_id
        || request.geometry_candidate_id == request.appearance_candidate_id
        || request.geometry_artifact_sha256 == request.appearance_artifact_sha256
        || request.sample_count != FRAME_COUNT as u64
        || request.sample_time_ticks.len() != FRAME_COUNT
        || request
            .sample_time_ticks
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        || request
            .sample_time_ticks
            .iter()
            .any(|tick| *tick > 1_000_000)
        || request.attachment_frame_count != FRAME_COUNT as u64
    {
        return Err(invalid("candidate or exact fifteen-frame schedule differs"));
    }
    if request.geometry_preservation_status != "source-output-renderable-geometry-byte-exact"
        || request.anchor_binding_policy != "geometry-appearance-anchor-role-owner-trs-equivalent@1"
        || request.attachment_policy
            != forgecad_contracts::FICTIONAL_ENERGY_VFX_ANIMATED_SOCKET_ATTACHMENT_V3_POLICY
        || request.frame_scope
            != forgecad_contracts::FICTIONAL_ENERGY_VFX_ANIMATED_SOCKET_ATTACHMENT_V3_FRAME_SCOPE
        || request.animation_vfx_scope
            != forgecad_contracts::CANDIDATE_ANIMATION_VFX_QUALITY_V2_SCOPE
        || request.animation_vfx_policy
            != forgecad_contracts::CANDIDATE_ANIMATION_VFX_QUALITY_V2_POLICY
        || request.animation_vfx_policy_sha256
            != sha256_hex(forgecad_contracts::CANDIDATE_ANIMATION_VFX_QUALITY_V2_POLICY.as_bytes())
        || request.from_stage != "material-surface"
        || request.to_stage != "animation-vfx"
    {
        return Err(invalid("scope, policy, stage or parent binding differs"));
    }
    let mut preimage = object.clone();
    preimage.remove("input_sha256");
    preimage.remove("idempotency_key");
    let expected = canonical_json_hash(&Value::Object(preimage));
    if request.input_sha256 != expected {
        return Err(invalid(format!(
            "input_sha256 differs; expected {expected}"
        )));
    }
    Ok((request, expected))
}

fn parse_get(value: &Value) -> Result<CandidateAnimationVfxQualityV2GetRequest, RuntimeError> {
    let object = exact_object(value, GET_FIELDS, GET_SCHEMA)?;
    if required_text(object, "schema_version")? != GET_SCHEMA {
        return Err(invalid("get schema version differs"));
    }
    let request: CandidateAnimationVfxQualityV2GetRequest =
        serde_json::from_value(value.clone())
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

fn equal_text(label: &str, actual: &str, expected: &str) -> Result<(), RuntimeError> {
    if actual != expected {
        return Err(invalid(format!("{label} differs")));
    }
    Ok(())
}

struct ArtifactBinding<'a> {
    artifact_id: &'a str,
    artifact_sha256: &'a str,
}

fn validate_artifact_binding(
    label: &str,
    actual: ArtifactBinding<'_>,
    expected: ArtifactBinding<'_>,
) -> Result<(), RuntimeError> {
    equal_text(
        &format!("{label} artifact id"),
        actual.artifact_id,
        expected.artifact_id,
    )?;
    equal_text(
        &format!("{label} artifact sha256"),
        actual.artifact_sha256,
        expected.artifact_sha256,
    )
}

fn validate_stage_and_material(
    runtime: &Runtime,
    request: &CandidateAnimationVfxQualityV2PrepareRequest,
) -> Result<
    (
        ProductionStageTransitionV2Record,
        ProductionStageHeadV2Record,
        CandidateMaterialSurfaceQualityRecord,
    ),
    RuntimeError,
> {
    let transition = runtime
        .store
        .get_production_stage_transition_v2(&request.source_material_surface_transition_id)?
        .ok_or_else(|| invalid("material-surface transition is unavailable"))?;
    if transition.canonical_sha256 != request.source_material_surface_transition_sha256
        || transition.project_id != request.project_id
        || transition.root_candidate_id != request.geometry_candidate_id
        || transition.root_candidate_state_sha256 != request.geometry_candidate_state_sha256
        || transition.root_artifact_sha256 != request.geometry_artifact_sha256
        || transition.head_candidate_id != request.appearance_candidate_id
        || transition.head_candidate_state_sha256 != request.appearance_candidate_state_sha256
        || transition.head_artifact_sha256 != request.appearance_artifact_sha256
        || transition.to_stage != "material-surface"
        || transition.material_surface_quality_id != request.source_material_surface_quality_id
        || transition.material_surface_quality_report_object_sha256
            != request.source_material_surface_quality_report_object_sha256
        || transition.material_surface_quality_canonical_sha256
            != request.source_material_surface_quality_canonical_sha256
    {
        return Err(invalid("material-surface transition binding differs"));
    }
    let stage_value = runtime.production_stage_transition_v2_get(json!({
        "schema_version":"ProductionStageTransitionGetRequest@2",
        "transition_id":transition.transition_id,
        "session_id":transition.session_id,
        "project_id":transition.project_id,
        "root_candidate_id":transition.root_candidate_id,
        "head_candidate_id":transition.head_candidate_id
    }))?;
    let head: ProductionStageHeadV2Record = serde_json::from_value(
        stage_value
            .get("production_stage_head")
            .cloned()
            .ok_or_else(|| invalid("material-surface head is unavailable"))?,
    )
    .map_err(|error| invalid(format!("material-surface head is malformed: {error}")))?;
    equal_text("transition from stage", &transition.from_stage, "topology")?;
    equal_text(
        "transition material quality status",
        &transition.material_surface_quality_status,
        "passed",
    )?;
    equal_text("head project", &head.project_id, &request.project_id)?;
    equal_text(
        "head root candidate",
        &head.root_candidate_id,
        &request.geometry_candidate_id,
    )?;
    equal_text(
        "head root state",
        &head.root_candidate_state_sha256,
        &request.geometry_candidate_state_sha256,
    )?;
    equal_text(
        "head root artifact",
        &head.root_artifact_sha256,
        &request.geometry_artifact_sha256,
    )?;
    equal_text(
        "head candidate",
        &head.head_candidate_id,
        &request.candidate_id,
    )?;
    equal_text(
        "head candidate state",
        &head.head_candidate_state_sha256,
        &request.appearance_candidate_state_sha256,
    )?;
    equal_text(
        "head artifact",
        &head.head_artifact_sha256,
        &request.appearance_artifact_sha256,
    )?;
    equal_text("head stage", &head.head_stage, "material-surface")?;
    equal_text(
        "head canonical",
        &head.canonical_sha256,
        &request.source_material_surface_head_canonical_sha256,
    )?;
    equal_text(
        "head material quality",
        &head.material_surface_quality_id,
        &request.source_material_surface_quality_id,
    )?;
    equal_text(
        "head material quality status",
        &head.material_surface_quality_status,
        "passed",
    )?;
    equal_text(
        "head material report",
        &head.material_surface_quality_report_object_sha256,
        &request.source_material_surface_quality_report_object_sha256,
    )?;
    equal_text(
        "head material canonical",
        &head.material_surface_quality_canonical_sha256,
        &request.source_material_surface_quality_canonical_sha256,
    )?;
    let material_value = runtime.candidate_material_surface_quality_get(json!({
        "schema_version":"CandidateMaterialSurfaceQualityGetRequest@1",
        "material_surface_quality_id":request.source_material_surface_quality_id,
        "project_id":request.project_id,
        "source_candidate_id":request.geometry_candidate_id,
        "output_candidate_id":request.appearance_candidate_id
    }))?;
    let material: CandidateMaterialSurfaceQualityRecord = serde_json::from_value(
        material_value
            .get("material_surface_quality")
            .cloned()
            .ok_or_else(|| invalid("material-surface quality is unavailable"))?,
    )
    .map_err(|error| invalid(format!("material-surface quality is malformed: {error}")))?;
    if material.project_id != request.project_id
        || material.material_surface_quality_id != request.source_material_surface_quality_id
        || material.source_candidate_id != request.geometry_candidate_id
        || material.output_candidate_id != request.appearance_candidate_id
        || material.source_candidate_state_sha256 != request.geometry_candidate_state_sha256
        || material.source_artifact_sha256 != request.geometry_artifact_sha256
        || material.output_candidate_state_sha256 != request.appearance_candidate_state_sha256
        || material.output_artifact_sha256 != request.appearance_artifact_sha256
        || material.canonical_sha256 != request.source_material_surface_quality_canonical_sha256
        || material.geometry_preservation_projection_sha256
            != request.geometry_preservation_projection_sha256
        || material.geometry_preservation_status != request.geometry_preservation_status
        || material.source_output_candidate_binding_status != "distinct-candidates-verified"
        || material.validator_status != "passed"
        || !material.hard_gate_passed
    {
        return Err(invalid(
            "material-surface quality binding or hard gate differs",
        ));
    }
    validate_artifact_binding(
        "transition source",
        ArtifactBinding {
            artifact_id: &transition.source_artifact_id,
            artifact_sha256: &transition.root_artifact_sha256,
        },
        ArtifactBinding {
            artifact_id: &material.source_artifact_id,
            artifact_sha256: &material.source_artifact_sha256,
        },
    )?;
    validate_artifact_binding(
        "transition output",
        ArtifactBinding {
            artifact_id: &transition.output_artifact_id,
            artifact_sha256: &transition.head_artifact_sha256,
        },
        ArtifactBinding {
            artifact_id: &material.output_artifact_id,
            artifact_sha256: &material.output_artifact_sha256,
        },
    )?;
    validate_artifact_binding(
        "head source",
        ArtifactBinding {
            artifact_id: &head.source_artifact_id,
            artifact_sha256: &head.root_artifact_sha256,
        },
        ArtifactBinding {
            artifact_id: &material.source_artifact_id,
            artifact_sha256: &material.source_artifact_sha256,
        },
    )?;
    validate_artifact_binding(
        "head output",
        ArtifactBinding {
            artifact_id: &head.output_artifact_id,
            artifact_sha256: &head.head_artifact_sha256,
        },
        ArtifactBinding {
            artifact_id: &material.output_artifact_id,
            artifact_sha256: &material.output_artifact_sha256,
        },
    )?;
    Ok((transition, head, material))
}

fn attachment_get_request(request: &CandidateAnimationVfxQualityV2PrepareRequest) -> Value {
    json!({
        "schema_version":"FictionalEnergyVfxAnimatedSocketAttachmentGetRequest@3",
        "attachment_key_sha256":request.attachment_key_sha256,
        "project_id":request.project_id,
        "geometry_candidate_id":request.geometry_candidate_id,
        "appearance_candidate_id":request.appearance_candidate_id,
        "geometry_delivery_manifest_object_sha256":request.geometry_delivery_manifest_object_sha256,
        "appearance_delivery_manifest_object_sha256":request.appearance_delivery_manifest_object_sha256
    })
}

fn attachment_frame_set_sha256(
    attachment_key_sha256: &str,
    frames: &[forgecad_contracts::FictionalEnergyVfxAnimatedSocketAttachmentV3FrameRecord],
) -> Result<String, RuntimeError> {
    let frame_values: Vec<Value> = frames
        .iter()
        .map(|frame| {
            json!({
                "frame_index":frame.frame_index,
                "canonical_sha256":frame.canonical_sha256
            })
        })
        .collect();
    let value = json!({
        "schema_version":FRAME_SET_SCHEMA,
        "attachment_key_sha256":attachment_key_sha256,
        "frames":frame_values
    });
    Ok(canonical_json_hash(&value))
}

fn validate_attachment(
    runtime: &Runtime,
    request: &CandidateAnimationVfxQualityV2PrepareRequest,
) -> Result<FictionalEnergyVfxAnimatedSocketAttachmentV3Record, RuntimeError> {
    let attachment_request = attachment_get_request(request);
    let value =
        runtime.fictional_energy_vfx_animated_socket_attachment_v3_get(&attachment_request)?;
    if value.get("runtime_write").and_then(Value::as_bool) != Some(false)
        || value.get("restart_hash_verified").and_then(Value::as_bool) != Some(true)
    {
        return Err(invalid("Attachment@3 exact get is not verified read-only"));
    }
    let attachment: FictionalEnergyVfxAnimatedSocketAttachmentV3Record = serde_json::from_value(
        value
            .get("attachment")
            .cloned()
            .ok_or_else(|| invalid("Attachment@3 payload is unavailable"))?,
    )
    .map_err(|error| invalid(format!("Attachment@3 payload is malformed: {error}")))?;
    for (label, actual, expected) in [
        (
            "Attachment@3 project",
            &attachment.project_id,
            &request.project_id,
        ),
        (
            "Attachment@3 geometry candidate",
            &attachment.geometry_candidate_id,
            &request.geometry_candidate_id,
        ),
        (
            "Attachment@3 geometry state",
            &attachment.geometry_candidate_state_sha256,
            &request.geometry_candidate_state_sha256,
        ),
        (
            "Attachment@3 geometry delivery",
            &attachment.geometry_delivery_manifest_object_sha256,
            &request.geometry_delivery_manifest_object_sha256,
        ),
        (
            "Attachment@3 geometry artifact",
            &attachment.geometry_artifact_sha256,
            &request.geometry_artifact_sha256,
        ),
        (
            "Attachment@3 appearance candidate",
            &attachment.appearance_candidate_id,
            &request.appearance_candidate_id,
        ),
        (
            "Attachment@3 appearance state",
            &attachment.appearance_candidate_state_sha256,
            &request.appearance_candidate_state_sha256,
        ),
        (
            "Attachment@3 appearance delivery",
            &attachment.appearance_delivery_manifest_object_sha256,
            &request.appearance_delivery_manifest_object_sha256,
        ),
        (
            "Attachment@3 appearance artifact",
            &attachment.appearance_artifact_sha256,
            &request.appearance_artifact_sha256,
        ),
        (
            "Attachment@3 material quality",
            &attachment.material_surface_quality_id,
            &request.source_material_surface_quality_id,
        ),
        (
            "Attachment@3 material report",
            &attachment.material_surface_quality_report_object_sha256,
            &request.source_material_surface_quality_report_object_sha256,
        ),
        (
            "Attachment@3 material canonical",
            &attachment.material_surface_quality_canonical_sha256,
            &request.source_material_surface_quality_canonical_sha256,
        ),
        (
            "Attachment@3 geometry preservation",
            &attachment.geometry_preservation_projection_sha256,
            &request.geometry_preservation_projection_sha256,
        ),
        (
            "Attachment@3 animated socket anchor set object",
            &attachment.animated_socket_anchor_set_object_sha256,
            &request.animated_socket_anchor_set_object_sha256,
        ),
        (
            "Attachment@3 animated socket anchor set canonical",
            &attachment.animated_socket_anchor_set_canonical_sha256,
            &request.animated_socket_anchor_set_canonical_sha256,
        ),
        (
            "Attachment@3 appearance anchor set object",
            &attachment.appearance_anchor_set_object_sha256,
            &request.appearance_anchor_set_object_sha256,
        ),
        (
            "Attachment@3 appearance anchor set canonical",
            &attachment.appearance_anchor_set_canonical_sha256,
            &request.appearance_anchor_set_canonical_sha256,
        ),
        (
            "Attachment@3 animated socket key",
            &attachment.animated_socket_materialization_key_sha256,
            &request.animated_socket_materialization_key_sha256,
        ),
        (
            "Attachment@3 animated artifact",
            &attachment.animated_artifact_sha256,
            &request.animated_artifact_sha256,
        ),
        (
            "Attachment@3 animation clip",
            &attachment.animation_clip_id,
            &request.animation_clip_id,
        ),
        (
            "Attachment@3 animation clip object",
            &attachment.animation_clip_object_sha256,
            &request.animation_clip_object_sha256,
        ),
        (
            "Attachment@3 animation clip canonical",
            &attachment.animation_clip_canonical_sha256,
            &request.animation_clip_canonical_sha256,
        ),
        (
            "Attachment@3 animation receipt object",
            &attachment.animation_receipt_object_sha256,
            &request.animation_receipt_object_sha256,
        ),
        (
            "Attachment@3 animation receipt canonical",
            &attachment.animation_receipt_canonical_sha256,
            &request.animation_receipt_canonical_sha256,
        ),
        (
            "Attachment@3 projection key",
            &attachment.projection_key_sha256,
            &request.projection_key_sha256,
        ),
        (
            "Attachment@3 projection object",
            &attachment.projection_object_sha256,
            &request.projection_object_sha256,
        ),
        (
            "Attachment@3 projection canonical",
            &attachment.projection_canonical_sha256,
            &request.projection_canonical_sha256,
        ),
        (
            "Attachment@3 particles key",
            &attachment.particle_sequence_key_sha256,
            &request.particle_sequence_key_sha256,
        ),
        (
            "Attachment@3 particles canonical",
            &attachment.particle_sequence_canonical_sha256,
            &request.particle_sequence_canonical_sha256,
        ),
        (
            "Attachment@3 trails key",
            &attachment.trail_sequence_key_sha256,
            &request.trail_sequence_key_sha256,
        ),
        (
            "Attachment@3 trails canonical",
            &attachment.trail_sequence_canonical_sha256,
            &request.trail_sequence_canonical_sha256,
        ),
        (
            "Attachment@3 trails bloom key",
            &attachment.trail_bloom_sequence_key_sha256,
            &request.trail_bloom_sequence_key_sha256,
        ),
        (
            "Attachment@3 trails bloom canonical",
            &attachment.trail_bloom_sequence_canonical_sha256,
            &request.trail_bloom_sequence_canonical_sha256,
        ),
        (
            "Attachment@3 VFX profile object",
            &attachment.vfx_profile_object_sha256,
            &request.vfx_profile_object_sha256,
        ),
        (
            "Attachment@3 VFX profile canonical",
            &attachment.vfx_profile_canonical_sha256,
            &request.vfx_profile_canonical_sha256,
        ),
        (
            "Attachment@3 TrailBloom profile",
            &attachment.trail_bloom_profile_sha256,
            &request.trail_bloom_profile_sha256,
        ),
        (
            "Attachment@3 socket encoding",
            &attachment.socket_node_id_encoding_sha256,
            &request.socket_node_id_encoding_sha256,
        ),
        (
            "Attachment@3 socket roles",
            &attachment.socket_roles_sha256,
            &request.socket_roles_sha256,
        ),
        (
            "Attachment@3 camera object",
            &attachment.camera_object_sha256,
            &request.camera_object_sha256,
        ),
        (
            "Attachment@3 camera identity",
            &attachment.camera_identity_sha256,
            &request.camera_identity_sha256,
        ),
        (
            "Attachment@3 render profile",
            &attachment.render_profile_sha256,
            &request.render_profile_sha256,
        ),
        (
            "Attachment@3 worker cohort",
            &attachment.render_worker_build_cohort_sha256,
            &request.render_worker_build_cohort_sha256,
        ),
        (
            "Attachment@3 sample schedule",
            &attachment.sample_schedule_sha256,
            &request.sample_schedule_sha256,
        ),
        (
            "Attachment@3 attachment key",
            &attachment.attachment_key_sha256,
            &request.attachment_key_sha256,
        ),
        (
            "Attachment@3 canonical",
            &attachment.canonical_sha256,
            &request.attachment_canonical_sha256,
        ),
        (
            "Attachment@3 receipt object",
            &attachment.attachment_receipt_object_sha256,
            &request.attachment_receipt_object_sha256,
        ),
        (
            "Attachment@3 receipt canonical",
            &attachment.attachment_receipt_canonical_sha256,
            &request.attachment_receipt_canonical_sha256,
        ),
    ] {
        equal_text(label, actual, expected)?;
    }
    if attachment.geometry_preservation_status != request.geometry_preservation_status
        || attachment.anchor_binding_policy != request.anchor_binding_policy
        || attachment.anchor_binding_sha256 != request.anchor_binding_sha256
        || attachment.sample_count != FRAME_COUNT as u64
        || attachment.sample_time_ticks != request.sample_time_ticks
        || attachment.attachment_policy != request.attachment_policy
        || attachment.frame_scope != request.frame_scope
        || attachment.frames.len() != FRAME_COUNT
        || attachment.attachment_status
            != forgecad_contracts::FICTIONAL_ENERGY_VFX_ANIMATED_SOCKET_ATTACHMENT_V3_STATUS
        || attachment.quality_status != "structural_only"
        || attachment.visual_quality_status != "NOT_PROVEN"
        || attachment.commercial_fps_quality_status != "NOT_PROVEN"
        || attachment.human_review_status != "NOT_RUN"
        || attachment.commercial_engine_status != "NOT_RUN"
        || attachment.runtime_write_performed != true
        || !attachment.restart_hash_verified
        || attachment.candidate_confirmed
        || attachment.version_created
        || attachment.export_performed
        || attachment.actual_engine_roundtrip
        || attachment.production_stage_advanced
    {
        return Err(invalid("Attachment@3 status, policy or schedule differs"));
    }
    for (index, frame) in attachment.frames.iter().enumerate() {
        if frame.frame_index != index as u64
            || frame.attachment_key_sha256 != attachment.attachment_key_sha256
            || frame.sample_time_ticks != request.sample_time_ticks[index]
            || !is_sha256(&frame.canonical_sha256)
        {
            return Err(invalid(format!("Attachment@3 frame {index} is invalid")));
        }
    }
    let digest =
        attachment_frame_set_sha256(&attachment.attachment_key_sha256, &attachment.frames)?;
    equal_text(
        "Attachment@3 full frame-set digest",
        &digest,
        &request.attachment_frame_set_sha256,
    )?;
    Ok(attachment)
}

fn derive_hard_gate(
    material: &CandidateMaterialSurfaceQualityRecord,
    attachment: &FictionalEnergyVfxAnimatedSocketAttachmentV3Record,
) -> CandidateAnimationVfxQualityV2HardGate {
    let structural = material.validator_status == "passed"
        && material.hard_gate_passed
        && attachment.frames.len() == FRAME_COUNT
        && attachment.quality_status == "structural_only"
        && attachment.visual_quality_status == "NOT_PROVEN"
        && attachment.commercial_fps_quality_status == "NOT_PROVEN"
        && attachment.human_review_status == "NOT_RUN"
        && attachment.commercial_engine_status == "NOT_RUN"
        && attachment.restart_hash_verified
        && !attachment.actual_engine_roundtrip
        && !attachment.candidate_confirmed
        && !attachment.version_created
        && !attachment.export_performed
        && !attachment.production_stage_advanced;
    CandidateAnimationVfxQualityV2HardGate {
        material_surface_head_binding: structural,
        material_surface_quality: structural,
        delivery_lod0_binding: structural,
        anchor_set_binding: structural,
        animation_clip_binding: structural,
        animation_glb_readback: structural,
        animated_socket_readback: structural,
        vfx_profile_binding: structural,
        base_frame_stack: structural,
        bloom_stack: structural,
        particle_stack: structural,
        trail_stack: structural,
        trail_bloom_stack: structural,
        cross_layer_parent_binding: structural,
        sample_camera_binding: structural,
        worker_cohort_binding: structural,
        render_pass_byte_exact: structural,
        bounded_resource_policy: structural,
        vfx_glb_socket_attachment: structural && attachment.frames.len() == FRAME_COUNT,
        nonfunctional_scope: structural,
    }
}

fn hard_gate_is_passed(gate: &CandidateAnimationVfxQualityV2HardGate) -> bool {
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
    request: &CandidateAnimationVfxQualityV2PrepareRequest,
    request_sha256: &str,
    material: &CandidateMaterialSurfaceQualityRecord,
    head: &ProductionStageHeadV2Record,
    attachment: &FictionalEnergyVfxAnimatedSocketAttachmentV3Record,
) -> Result<CandidateAnimationVfxQualityV2Record, RuntimeError> {
    let hard_gate = derive_hard_gate(material, attachment);
    let hard_gate_passed = hard_gate_is_passed(&hard_gate);
    if !hard_gate_passed {
        return Err(invalid("Attachment@3 dependency hard gate is not passed"));
    }
    let mut record = CandidateAnimationVfxQualityV2Record {
        schema_version: RECORD_SCHEMA.to_owned(),
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
        geometry_candidate_id: attachment.geometry_candidate_id.clone(),
        geometry_candidate_state_sha256: attachment.geometry_candidate_state_sha256.clone(),
        geometry_delivery_manifest_object_sha256: attachment
            .geometry_delivery_manifest_object_sha256
            .clone(),
        geometry_artifact_sha256: attachment.geometry_artifact_sha256.clone(),
        appearance_candidate_id: attachment.appearance_candidate_id.clone(),
        appearance_candidate_state_sha256: attachment.appearance_candidate_state_sha256.clone(),
        appearance_delivery_manifest_object_sha256: attachment
            .appearance_delivery_manifest_object_sha256
            .clone(),
        appearance_artifact_sha256: attachment.appearance_artifact_sha256.clone(),
        geometry_preservation_projection_sha256: attachment
            .geometry_preservation_projection_sha256
            .clone(),
        geometry_preservation_status: attachment.geometry_preservation_status.clone(),
        animated_socket_materialization_key_sha256: attachment
            .animated_socket_materialization_key_sha256
            .clone(),
        animated_artifact_sha256: attachment.animated_artifact_sha256.clone(),
        animated_socket_anchor_set_object_sha256: attachment
            .animated_socket_anchor_set_object_sha256
            .clone(),
        animated_socket_anchor_set_canonical_sha256: attachment
            .animated_socket_anchor_set_canonical_sha256
            .clone(),
        appearance_anchor_set_object_sha256: attachment.appearance_anchor_set_object_sha256.clone(),
        appearance_anchor_set_canonical_sha256: attachment
            .appearance_anchor_set_canonical_sha256
            .clone(),
        anchor_binding_policy: attachment.anchor_binding_policy.clone(),
        anchor_binding_sha256: attachment.anchor_binding_sha256.clone(),
        animation_clip_id: attachment.animation_clip_id.clone(),
        animation_clip_object_sha256: attachment.animation_clip_object_sha256.clone(),
        animation_clip_canonical_sha256: attachment.animation_clip_canonical_sha256.clone(),
        animation_receipt_object_sha256: attachment.animation_receipt_object_sha256.clone(),
        animation_receipt_canonical_sha256: attachment.animation_receipt_canonical_sha256.clone(),
        projection_key_sha256: attachment.projection_key_sha256.clone(),
        projection_object_sha256: attachment.projection_object_sha256.clone(),
        projection_canonical_sha256: attachment.projection_canonical_sha256.clone(),
        particle_sequence_key_sha256: attachment.particle_sequence_key_sha256.clone(),
        particle_sequence_canonical_sha256: attachment.particle_sequence_canonical_sha256.clone(),
        trail_sequence_key_sha256: attachment.trail_sequence_key_sha256.clone(),
        trail_sequence_canonical_sha256: attachment.trail_sequence_canonical_sha256.clone(),
        trail_bloom_sequence_key_sha256: attachment.trail_bloom_sequence_key_sha256.clone(),
        trail_bloom_sequence_canonical_sha256: attachment
            .trail_bloom_sequence_canonical_sha256
            .clone(),
        attachment_key_sha256: attachment.attachment_key_sha256.clone(),
        attachment_canonical_sha256: attachment.canonical_sha256.clone(),
        attachment_receipt_object_sha256: attachment.attachment_receipt_object_sha256.clone(),
        attachment_receipt_canonical_sha256: attachment.attachment_receipt_canonical_sha256.clone(),
        attachment_frame_count: attachment.frames.len() as u64,
        attachment_frame_set_sha256: attachment_frame_set_sha256(
            &attachment.attachment_key_sha256,
            &attachment.frames,
        )?,
        vfx_profile_object_sha256: attachment.vfx_profile_object_sha256.clone(),
        vfx_profile_canonical_sha256: attachment.vfx_profile_canonical_sha256.clone(),
        trail_bloom_profile_sha256: attachment.trail_bloom_profile_sha256.clone(),
        socket_node_id_encoding_sha256: attachment.socket_node_id_encoding_sha256.clone(),
        socket_roles_sha256: attachment.socket_roles_sha256.clone(),
        camera_object_sha256: attachment.camera_object_sha256.clone(),
        camera_identity_sha256: attachment.camera_identity_sha256.clone(),
        render_profile_sha256: attachment.render_profile_sha256.clone(),
        render_worker_build_cohort_sha256: attachment.render_worker_build_cohort_sha256.clone(),
        sample_schedule_sha256: attachment.sample_schedule_sha256.clone(),
        sample_count: attachment.sample_count,
        sample_time_ticks: attachment.sample_time_ticks.clone(),
        attachment_policy: attachment.attachment_policy.clone(),
        frame_scope: attachment.frame_scope.clone(),
        animation_vfx_scope: forgecad_contracts::CANDIDATE_ANIMATION_VFX_QUALITY_V2_SCOPE
            .to_owned(),
        animation_vfx_policy: forgecad_contracts::CANDIDATE_ANIMATION_VFX_QUALITY_V2_POLICY
            .to_owned(),
        animation_vfx_policy_sha256: sha256_hex(
            forgecad_contracts::CANDIDATE_ANIMATION_VFX_QUALITY_V2_POLICY.as_bytes(),
        ),
        from_stage: "material-surface".to_owned(),
        to_stage: "animation-vfx".to_owned(),
        input_sha256: request.input_sha256.clone(),
        candidate_binding_status:
            forgecad_contracts::CANDIDATE_ANIMATION_VFX_QUALITY_V2_BINDING_STATUS.to_owned(),
        hard_gate,
        validator_status: "passed".to_owned(),
        hard_gate_passed,
        animation_status: "structural_only".to_owned(),
        vfx_status: "structural_only".to_owned(),
        visual_quality_status: "NOT_PROVEN".to_owned(),
        artistic_quality_status: "NOT_PROVEN".to_owned(),
        human_review_status: "NOT_RUN".to_owned(),
        commercial_fps_quality_status: "NOT_PROVEN".to_owned(),
        commercial_engine_status: "NOT_RUN".to_owned(),
        actual_engine_roundtrip: false,
        functional_semantics: false,
        materialization_status: "runtime-owned-durable-candidate-animation-vfx-quality-v2"
            .to_owned(),
        quality_status: "structural_only".to_owned(),
        runtime_write_performed: true,
        production_stage_advanced: false,
        candidate_confirmed: false,
        version_created: false,
        export_performed: false,
        request_sha256: request_sha256.to_owned(),
        canonical_sha256: String::new(),
        created_at: attachment.created_at.clone(),
    };
    equal_text(
        "head canonical",
        &head.canonical_sha256,
        &record.source_material_surface_head_canonical_sha256,
    )?;
    let mut preimage = serde_json::to_value(&record)
        .map_err(|error| invalid(format!("quality record cannot be serialized: {error}")))?;
    preimage["canonical_sha256"] = Value::String(String::new());
    record.canonical_sha256 = canonical_json_hash(&preimage);
    Ok(record)
}

fn request_from_record(
    record: &CandidateAnimationVfxQualityV2Record,
) -> Result<CandidateAnimationVfxQualityV2PrepareRequest, RuntimeError> {
    let mut value = serde_json::to_value(record).map_err(|error| invalid(error.to_string()))?;
    let object = value
        .as_object_mut()
        .ok_or_else(|| invalid("quality record is not an object"))?;
    for field in [
        "candidate_binding_status",
        "hard_gate",
        "validator_status",
        "hard_gate_passed",
        "animation_status",
        "vfx_status",
        "visual_quality_status",
        "artistic_quality_status",
        "human_review_status",
        "commercial_fps_quality_status",
        "commercial_engine_status",
        "actual_engine_roundtrip",
        "functional_semantics",
        "materialization_status",
        "quality_status",
        "runtime_write_performed",
        "production_stage_advanced",
        "candidate_confirmed",
        "version_created",
        "export_performed",
        "request_sha256",
        "canonical_sha256",
        "created_at",
    ] {
        object.remove(field);
    }
    object.insert(
        "schema_version".to_owned(),
        Value::String(PREPARE_SCHEMA.to_owned()),
    );
    object.insert(
        "idempotency_key".to_owned(),
        Value::String(record.animation_vfx_quality_id.clone()),
    );
    serde_json::from_value(value).map_err(|error| {
        invalid(format!(
            "stored quality replay request is malformed: {error}"
        ))
    })
}

fn revalidate_stored_request(
    record: &CandidateAnimationVfxQualityV2Record,
) -> Result<(CandidateAnimationVfxQualityV2PrepareRequest, String), RuntimeError> {
    let request = request_from_record(record)?;
    let value = serde_json::to_value(&request).map_err(|error| {
        invalid(format!(
            "stored quality replay request cannot serialize: {error}"
        ))
    })?;
    let (request, request_sha256) = parse_prepare(&value)?;
    if request_sha256 != record.request_sha256 {
        return Err(invalid(
            "stored quality request_sha256 differs from its closed request preimage",
        ));
    }
    Ok((request, request_sha256))
}

fn result_value(
    record: &CandidateAnimationVfxQualityV2Record,
    replayed: bool,
    schema: &str,
    runtime_write: bool,
) -> Result<Value, RuntimeError> {
    Ok(json!({
        "schema_version":schema,
        "animation_vfx_quality":serde_json::to_value(record).map_err(|error| invalid(error.to_string()))?,
        "replayed":replayed,
        "runtime_write":runtime_write,
        "production_stage_advanced":false,
        "candidate_confirmed":false,
        "version_created":false,
        "export_performed":false
    }))
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

pub(super) fn prepare(runtime: &Runtime, value: &Value) -> Result<Value, RuntimeError> {
    let (request, request_sha256) = parse_prepare(value)?;
    let (_transition, head, material) = validate_stage_and_material(runtime, &request)?;
    let attachment = validate_attachment(runtime, &request)?;
    let record = record_from_request(&request, &request_sha256, &material, &head, &attachment)?;
    let bytes = canonical_json_bytes(
        &serde_json::to_value(&record).map_err(|error| invalid(error.to_string()))?,
    )
    .map_err(|error| invalid(error.to_string()))?;
    if bytes.is_empty() || bytes.len() > MAX_REPORT_BYTES {
        return Err(invalid("V2 report exceeds one MiB"));
    }
    let reservation = runtime.store.begin_cas_reservation();
    let report = match runtime.store.put_object_reserved(
        &reservation,
        &bytes,
        None,
        REPORT_MIME,
        REPORT_KIND,
        &record.created_at,
    ) {
        Ok(object) => object,
        Err(error) => return Err(error.into()),
    };
    match runtime
        .store
        .record_candidate_animation_vfx_quality_v2_with_replay(&record, &report.record)
    {
        Ok((stored, replayed)) => {
            release_report(runtime, &reservation, &report, false);
            result_value(&stored, replayed, PREPARE_RESULT_SCHEMA, true)
        }
        Err(error) => {
            release_report(runtime, &reservation, &report, true);
            Err(error.into())
        }
    }
}

pub(super) fn get(runtime: &Runtime, value: &Value) -> Result<Value, RuntimeError> {
    let request = parse_get(value)?;
    let record = runtime
        .store
        .get_candidate_animation_vfx_quality_v2(&request.animation_vfx_quality_id)?
        .ok_or_else(|| invalid("CandidateAnimationVfxQuality@2 is unavailable"))?;
    if record.project_id != request.project_id || record.candidate_id != request.candidate_id {
        return Err(invalid("V2 quality get scope differs"));
    }
    let (replay_request, replay_request_sha256) = revalidate_stored_request(&record)?;
    let (_transition, head, material) = validate_stage_and_material(runtime, &replay_request)?;
    let attachment = validate_attachment(runtime, &replay_request)?;
    let recomputed = record_from_request(
        &replay_request,
        &replay_request_sha256,
        &material,
        &head,
        &attachment,
    )?;
    if recomputed.canonical_sha256 != record.canonical_sha256
        || recomputed.hard_gate != record.hard_gate
        || recomputed.attachment_frame_set_sha256 != record.attachment_frame_set_sha256
        || recomputed.attachment_canonical_sha256 != record.attachment_canonical_sha256
        || recomputed.attachment_receipt_object_sha256 != record.attachment_receipt_object_sha256
    {
        return Err(invalid("V2 quality receipt is tampered or stale"));
    }
    result_value(&record, true, GET_RESULT_SCHEMA, false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request_fixture() -> Value {
        let hash = "a".repeat(64);
        let mut object = Map::new();
        for field in PREPARE_FIELDS {
            object.insert((*field).to_owned(), Value::String(hash.clone()));
        }
        object.insert(
            "schema_version".to_owned(),
            Value::String(PREPARE_SCHEMA.to_owned()),
        );
        for field in [
            "animation_vfx_quality_id",
            "project_id",
            "source_material_surface_transition_id",
            "source_material_surface_quality_id",
            "candidate_id",
            "appearance_candidate_id",
            "idempotency_key",
            "animation_clip_id",
        ] {
            object.insert((*field).to_owned(), Value::String(field.to_owned()));
        }
        object.insert(
            "geometry_candidate_id".to_owned(),
            Value::String("geometry-candidate".to_owned()),
        );
        object.insert(
            "candidate_id".to_owned(),
            Value::String("appearance_candidate_id".to_owned()),
        );
        object.insert(
            "appearance_candidate_id".to_owned(),
            Value::String("appearance_candidate_id".to_owned()),
        );
        object.insert("sample_count".to_owned(), Value::from(FRAME_COUNT as u64));
        object.insert(
            "sample_time_ticks".to_owned(),
            Value::Array((0..FRAME_COUNT as u64).map(Value::from).collect()),
        );
        object.insert(
            "attachment_frame_count".to_owned(),
            Value::from(FRAME_COUNT as u64),
        );
        object.insert(
            "geometry_preservation_status".to_owned(),
            Value::String("source-output-renderable-geometry-byte-exact".to_owned()),
        );
        object.insert(
            "anchor_binding_policy".to_owned(),
            Value::String("geometry-appearance-anchor-role-owner-trs-equivalent@1".to_owned()),
        );
        object.insert(
            "attachment_policy".to_owned(),
            Value::String(
                forgecad_contracts::FICTIONAL_ENERGY_VFX_ANIMATED_SOCKET_ATTACHMENT_V3_POLICY
                    .to_owned(),
            ),
        );
        object.insert(
            "frame_scope".to_owned(),
            Value::String(
                forgecad_contracts::FICTIONAL_ENERGY_VFX_ANIMATED_SOCKET_ATTACHMENT_V3_FRAME_SCOPE
                    .to_owned(),
            ),
        );
        object.insert(
            "animation_vfx_scope".to_owned(),
            Value::String(forgecad_contracts::CANDIDATE_ANIMATION_VFX_QUALITY_V2_SCOPE.to_owned()),
        );
        object.insert(
            "animation_vfx_policy".to_owned(),
            Value::String(forgecad_contracts::CANDIDATE_ANIMATION_VFX_QUALITY_V2_POLICY.to_owned()),
        );
        object.insert(
            "animation_vfx_policy_sha256".to_owned(),
            Value::String(sha256_hex(
                forgecad_contracts::CANDIDATE_ANIMATION_VFX_QUALITY_V2_POLICY.as_bytes(),
            )),
        );
        object.insert(
            "from_stage".to_owned(),
            Value::String("material-surface".to_owned()),
        );
        object.insert(
            "to_stage".to_owned(),
            Value::String("animation-vfx".to_owned()),
        );
        object.insert(
            "geometry_artifact_sha256".to_owned(),
            Value::String("b".repeat(64)),
        );
        object.insert(
            "appearance_artifact_sha256".to_owned(),
            Value::String("c".repeat(64)),
        );
        let mut preimage = object.clone();
        preimage.remove("input_sha256");
        preimage.remove("idempotency_key");
        object.insert(
            "input_sha256".to_owned(),
            Value::String(canonical_json_hash(&Value::Object(preimage))),
        );
        Value::Object(object)
    }

    #[test]
    fn prepare_is_closed_and_full_frame_bound() {
        let value = request_fixture();
        let (request, request_sha256) = parse_prepare(&value).expect("closed V2 request");
        assert_eq!(request.sample_count, 15);
        assert_eq!(request.attachment_frame_count, 15);
        assert_eq!(request_sha256, request.input_sha256);
    }

    #[test]
    fn replay_request_rejects_tampered_input_digest() {
        let mut value = request_fixture();
        value["input_sha256"] = Value::String("b".repeat(64));
        let error = parse_prepare(&value).expect_err("replay must revalidate input preimage");
        assert!(error.to_string().contains("input_sha256 differs"));
    }

    #[test]
    fn v1_sidecar_fields_are_rejected() {
        let mut value = request_fixture();
        value.as_object_mut().unwrap().insert(
            "vfx_sequence_key_sha256".to_owned(),
            Value::String("a".repeat(64)),
        );
        let error = parse_prepare(&value).expect_err("V1 sidecar field must be rejected");
        assert!(error.to_string().contains("unexpected field set"));
    }

    #[test]
    fn retargeted_head_candidate_is_rejected() {
        let mut value = request_fixture();
        value.as_object_mut().unwrap().insert(
            "candidate_id".to_owned(),
            Value::String("geometry-candidate".to_owned()),
        );
        let error = parse_prepare(&value).expect_err("quality cannot retarget geometry head");
        assert!(error
            .to_string()
            .contains("candidate or exact fifteen-frame schedule differs"));
    }

    #[test]
    fn frame_set_digest_is_ordered_and_attachment_bound() {
        let frames = (0..FRAME_COUNT)
            .map(|index| {
                json!({
                    "frame_index":index,
                    "canonical_sha256":"a".repeat(64)
                })
            })
            .collect::<Vec<_>>();
        let first = canonical_json_hash(&json!({
            "schema_version":FRAME_SET_SCHEMA,
            "attachment_key_sha256":"b".repeat(64),
            "frames":frames.clone()
        }));
        let mut reordered = frames.clone();
        reordered.swap(0, 1);
        let reordered_digest = canonical_json_hash(&json!({
            "schema_version":FRAME_SET_SCHEMA,
            "attachment_key_sha256":"b".repeat(64),
            "frames":reordered
        }));
        let second = canonical_json_hash(&json!({
            "schema_version":FRAME_SET_SCHEMA,
            "attachment_key_sha256":"c".repeat(64),
            "frames":(0..FRAME_COUNT).map(|index| json!({"frame_index":index,"canonical_sha256":"a".repeat(64)})).collect::<Vec<_>>()
        }));
        assert_ne!(first, reordered_digest);
        assert_ne!(first, second);
    }

    #[test]
    fn artifact_ids_and_sha256_bindings_are_never_interchangeable() {
        let source_id = "prepared-source-artifact-id";
        let source_sha256 = "a".repeat(64);
        let expected = ArtifactBinding {
            artifact_id: source_id,
            artifact_sha256: source_sha256.as_str(),
        };
        let actual = ArtifactBinding {
            artifact_id: source_id,
            artifact_sha256: source_sha256.as_str(),
        };
        assert!(validate_artifact_binding("source", actual, expected).is_ok());

        let id_from_sha = ArtifactBinding {
            artifact_id: source_sha256.as_str(),
            artifact_sha256: source_sha256.as_str(),
        };
        let expected = ArtifactBinding {
            artifact_id: source_id,
            artifact_sha256: source_sha256.as_str(),
        };
        assert!(validate_artifact_binding("source", id_from_sha, expected).is_err());

        let sha_from_id = ArtifactBinding {
            artifact_id: source_id,
            artifact_sha256: source_id,
        };
        let expected = ArtifactBinding {
            artifact_id: source_id,
            artifact_sha256: source_sha256.as_str(),
        };
        assert!(validate_artifact_binding("source", sha_from_id, expected).is_err());
    }
}
