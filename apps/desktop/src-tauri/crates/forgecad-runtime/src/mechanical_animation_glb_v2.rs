//! Appearance-aware, candidate-bound MechanicalAnimationGlb@2.
//!
//! This module is intentionally additive to `rigid_animation_glb` (the V1
//! producer).  It consumes an immutable `MechanicalAnimationClip@2`, whose
//! upstream appearance/quality/lineage replay is already closed, and appends
//! only rigid TRS channels to the exact appearance GLB.  The two output CAS
//! objects (animated GLB and receipt) are reserved and linked atomically by
//! Store; no candidate, version, approval or export state is touched.

use super::{
    artifact_readback_v2_value, canonical_json_bytes, canonical_json_hash, exact_object,
    is_opaque_id, is_sha256, sha256_hex, strict_glb_inspection,
    validate_glb_material_pack_identity, Runtime, RuntimeError, MAX_GEOMETRY_ARTIFACT_BYTES,
};
use forgecad_contracts::{
    MechanicalAnimationGlbV2LinkRecord, MechanicalAnimationGlbV2ReceiptRecord,
};
use forgecad_store::CasObject;
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet};

const PREPARE_SCHEMA: &str = "MechanicalAnimationGlbPrepareRequest@2";
const GET_SCHEMA: &str = "MechanicalAnimationGlbGetRequest@2";
const PREPARE_RESULT_SCHEMA: &str = "MechanicalAnimationGlbPrepareResult@2";
const GET_RESULT_SCHEMA: &str = "MechanicalAnimationGlbGetResult@2";
const RECEIPT_SCHEMA: &str = "MechanicalAnimationGlbReceipt@2";
const POLICY: &str = "appearance-aware-rigid-node-trs-gltf-linear-scheduled-samples@2";
const CLIP_PREVIEW_POLICY: &str =
    "single-tick-transient-geometry-plus-appearance-double-worker-replay@1";
const GLB_KIND: &str = "mechanical-animation-glb-v2";
const RECEIPT_KIND: &str = "mechanical-animation-glb-v2-receipt";
const GLB_MIME: &str = "model/gltf-binary";
const RECEIPT_MIME: &str = "application/json";
const MAX_GLB_BYTES: usize = 64 * 1024 * 1024;
const MAX_JSON_BYTES: usize = 1024 * 1024;
const MAX_SAMPLES: usize = 16;
const TIMEBASE_HZ: u64 = 1000;
const ANIMATION_NAME: &str = "ForgeCAD rigid mechanical clip";
const STATUS: &str = "runtime-owned-cas-appearance-aware-animated-glb";
const VALIDATOR_STATUS: &str = "strict-appearance-aware-rigid-gltf-animation-readback-pass";

const PREPARE_FIELDS: &[&str] = &[
    "schema_version",
    "project_id",
    "appearance_candidate_id",
    "appearance_candidate_state_sha256",
    "clip_id",
    "clip_object_sha256",
    "clip_sha256",
    "materialization_policy",
    "input_sha256",
    "idempotency_key",
];

const GET_FIELDS: &[&str] = &[
    "schema_version",
    "project_id",
    "appearance_candidate_id",
    "clip_id",
];

#[derive(Clone, Copy, Debug, PartialEq)]
struct RigidTransform {
    translation: [f32; 3],
    rotation: [f32; 4],
}

#[derive(Debug, Clone)]
struct Frame {
    frame_sha256: String,
    deltas: BTreeMap<String, RigidTransform>,
}

#[derive(Debug, Clone)]
struct ClipContext {
    request: Value,
    clip: Value,
    quality: Value,
    source_glb: Vec<u8>,
    ticks: Vec<u64>,
    frames: Vec<Frame>,
    part_ids: Vec<String>,
    created_at: String,
    material_pack_id: String,
    material_pack_manifest_sha256: String,
}

fn invalid(detail: impl Into<String>) -> RuntimeError {
    RuntimeError::InvalidInput(format!(
        "MECHANICAL_ANIMATION_GLB_V2_INVALID: {}",
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

fn id<'a>(object: &'a Map<String, Value>, field: &str) -> Result<&'a str, RuntimeError> {
    let value = text(object, field)?;
    if !is_opaque_id(value) {
        return Err(invalid(format!("{field} is not an opaque identifier")));
    }
    Ok(value)
}

fn sha<'a>(object: &'a Map<String, Value>, field: &str) -> Result<&'a str, RuntimeError> {
    let value = text(object, field)?;
    if !is_sha256(value) {
        return Err(invalid(format!("{field} is not a SHA-256")));
    }
    Ok(value)
}

fn verify_canonical(value: &Value, field: &str) -> Result<(), RuntimeError> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid("canonical payload is not an object"))?;
    let expected = object
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .ok_or_else(|| invalid(format!("{field} is not a SHA-256")))?;
    let mut preimage = value.clone();
    preimage
        .as_object_mut()
        .expect("canonical payload is an object")
        .insert(field.to_owned(), Value::String(String::new()));
    if canonical_json_hash(&preimage) != expected {
        return Err(invalid(format!("{field} does not bind the payload")));
    }
    Ok(())
}

fn key_preimage(object: &Map<String, Value>) -> Result<Value, RuntimeError> {
    Ok(json!({
        "project_id": id(object, "project_id")?,
        "appearance_candidate_id": id(object, "appearance_candidate_id")?,
        "appearance_candidate_state_sha256": sha(object, "appearance_candidate_state_sha256")?,
        "clip_id": id(object, "clip_id")?,
        "clip_object_sha256": sha(object, "clip_object_sha256")?,
        "clip_sha256": sha(object, "clip_sha256")?,
        "materialization_policy": text(object, "materialization_policy")?,
    }))
}

fn parse_prepare(value: &Value) -> Result<(Value, String, String), RuntimeError> {
    let object = exact_object(value, PREPARE_FIELDS, PREPARE_SCHEMA)?;
    if text(object, "schema_version")? != PREPARE_SCHEMA
        || text(object, "materialization_policy")? != POLICY
    {
        return Err(invalid("prepare schema or materialization policy differs"));
    }
    let input = sha(object, "input_sha256")?;
    id(object, "idempotency_key")?;
    let mut preimage = object.clone();
    preimage.remove("input_sha256");
    preimage.remove("idempotency_key");
    let computed_input = canonical_json_hash(&Value::Object(preimage));
    if input != computed_input {
        return Err(invalid("input_sha256 does not bind the closed request"));
    }
    let key = canonical_json_hash(&key_preimage(object)?);
    Ok((value.clone(), key, input.to_owned()))
}

fn parse_get(value: &Value) -> Result<(String, String, String), RuntimeError> {
    let object = exact_object(value, GET_FIELDS, GET_SCHEMA)?;
    if text(object, "schema_version")? != GET_SCHEMA {
        return Err(invalid("get schema differs"));
    }
    Ok((
        id(object, "project_id")?.to_owned(),
        id(object, "appearance_candidate_id")?.to_owned(),
        id(object, "clip_id")?.to_owned(),
    ))
}

fn read_json(runtime: &Runtime, hash: &str, kind: &str) -> Result<Value, RuntimeError> {
    let object = runtime
        .store
        .get_object(hash)?
        .ok_or_else(|| invalid(format!("{kind} object is unavailable")))?;
    if object.mime != RECEIPT_MIME
        || object.kind != kind
        || object.size_bytes == 0
        || object.size_bytes > MAX_JSON_BYTES as u64
    {
        return Err(invalid(format!("{kind} object metadata is invalid")));
    }
    let bytes = runtime.cas_read_bounded(hash, MAX_JSON_BYTES as u64)?;
    if sha256_hex(&bytes) != hash {
        return Err(invalid(format!("{kind} object hash differs")));
    }
    let value: Value = serde_json::from_slice(&bytes)
        .map_err(|error| invalid(format!("{kind} object is invalid JSON: {error}")))?;
    if canonical_json_bytes(&value).map_err(|error| invalid(error.to_string()))? != bytes {
        return Err(invalid(format!("{kind} object is not canonical JSON")));
    }
    Ok(value)
}

fn set_canonical(value: &mut Value, field: &str) -> Result<(), RuntimeError> {
    value
        .as_object_mut()
        .ok_or_else(|| invalid("canonical payload is not an object"))?
        .insert(field.to_owned(), Value::String(String::new()));
    let hash = canonical_json_hash(value);
    value
        .as_object_mut()
        .expect("canonical payload is an object")
        .insert(field.to_owned(), Value::String(hash));
    Ok(())
}

fn request_for_clip(project_id: &str, appearance_candidate_id: &str, clip_id: &str) -> Value {
    json!({
        "schema_version":"MechanicalAnimationClipGetRequest@2",
        "project_id":project_id,
        "appearance_candidate_id":appearance_candidate_id,
        "clip_id":clip_id
    })
}

fn clip_get(
    runtime: &Runtime,
    project_id: &str,
    appearance_candidate_id: &str,
    clip_id: &str,
) -> Result<Value, RuntimeError> {
    let result = runtime.mechanical_animation_clip_v2_get(&request_for_clip(
        project_id,
        appearance_candidate_id,
        clip_id,
    ))?;
    if result.get("schema_version").and_then(Value::as_str)
        != Some("MechanicalAnimationClipGetResult@2")
        || result.get("restart_hash_verified").and_then(Value::as_bool) != Some(true)
        || result
            .get("runtime_write_performed")
            .and_then(Value::as_bool)
            != Some(false)
    {
        return Err(invalid(
            "MechanicalAnimationClip@2 get is not a verified read-only result",
        ));
    }
    result
        .get("clip")
        .cloned()
        .ok_or_else(|| invalid("MechanicalAnimationClip@2 payload is missing"))
}

fn validate_clip_scope(
    runtime: &Runtime,
    request: &Value,
    clip: &Value,
) -> Result<Value, RuntimeError> {
    let object = request
        .as_object()
        .ok_or_else(|| invalid("request is not an object"))?;
    let project_id = id(object, "project_id")?;
    let appearance_candidate_id = id(object, "appearance_candidate_id")?;
    let clip_id = id(object, "clip_id")?;
    let clip_object_sha256 = sha(object, "clip_object_sha256")?;
    let clip_sha256 = sha(object, "clip_sha256")?;
    if clip.get("schema_version").and_then(Value::as_str) != Some("MechanicalAnimationClip@2")
        || clip.get("project_id").and_then(Value::as_str) != Some(project_id)
        || clip.get("appearance_candidate_id").and_then(Value::as_str)
            != Some(appearance_candidate_id)
        || clip.get("clip_id").and_then(Value::as_str) != Some(clip_id)
        || clip.get("canonical_sha256").and_then(Value::as_str) != Some(clip_sha256)
    {
        return Err(invalid("clip identity or canonical binding differs"));
    }
    let clip_bytes = runtime.cas_read_bounded(clip_object_sha256, MAX_JSON_BYTES as u64)?;
    if sha256_hex(&clip_bytes) != clip_object_sha256
        || canonical_json_bytes(clip).map_err(|error| invalid(error.to_string()))? != clip_bytes
    {
        return Err(invalid("MechanicalAnimationClip@2 CAS bytes differ"));
    }
    let candidate = runtime
        .candidate(appearance_candidate_id)?
        .ok_or_else(|| invalid("appearance candidate is unavailable"))?;
    if candidate.project_id != project_id
        || candidate.canonical_sha256 != text(object, "appearance_candidate_state_sha256")?
        || candidate.prepared_object_sha256.as_deref()
            != clip
                .get("appearance_artifact_sha256")
                .and_then(Value::as_str)
        || candidate.prepared_object_id.as_deref()
            != clip.get("appearance_artifact_id").and_then(Value::as_str)
    {
        return Err(invalid(
            "appearance candidate/state/artifact binding differs",
        ));
    }

    let quality_request = json!({
        "schema_version":"CandidateMaterialSurfaceQualityGetRequest@1",
        "material_surface_quality_id":clip["material_surface_quality_id"],
        "project_id":project_id,
        "source_candidate_id":clip["source_geometry_candidate_id"],
        "output_candidate_id":appearance_candidate_id
    });
    let quality_result = runtime.candidate_material_surface_quality_get(quality_request)?;
    let quality = quality_result
        .get("material_surface_quality")
        .cloned()
        .ok_or_else(|| invalid("material-surface quality is missing"))?;
    let quality_required = [
        ("project_id", "project_id"),
        ("material_surface_quality_id", "material_surface_quality_id"),
        ("source_candidate_id", "source_geometry_candidate_id"),
        (
            "source_candidate_state_sha256",
            "source_geometry_candidate_state_sha256",
        ),
        ("source_artifact_id", "source_geometry_artifact_id"),
        ("source_artifact_sha256", "source_geometry_artifact_sha256"),
        ("output_candidate_id", "appearance_candidate_id"),
        (
            "output_candidate_state_sha256",
            "appearance_candidate_state_sha256",
        ),
        ("output_artifact_id", "appearance_artifact_id"),
        ("output_artifact_sha256", "appearance_artifact_sha256"),
        (
            "output_artifact_readback_sha256",
            "appearance_artifact_readback_sha256",
        ),
        (
            "output_artifact_readback_object_sha256",
            "appearance_artifact_readback_object_sha256",
        ),
        (
            "source_geometry_candidate_evidence_sha256",
            "source_geometry_candidate_evidence_sha256",
        ),
        (
            "appearance_source_lineage_sidecar_object_sha256",
            "appearance_source_lineage_sidecar_object_sha256",
        ),
        (
            "appearance_source_lineage_canonical_sha256",
            "appearance_source_lineage_canonical_sha256",
        ),
        (
            "appearance_program_object_sha256",
            "appearance_program_object_sha256",
        ),
        ("appearance_program_sha256", "appearance_program_sha256"),
        ("source_geometry_program_sha256", "geometry_program_sha256"),
        ("output_geometry_program_sha256", "geometry_program_sha256"),
        (
            "geometry_preservation_projection_sha256",
            "geometry_preservation_projection_sha256",
        ),
    ];
    for (quality_field, clip_field) in quality_required {
        if quality.get(quality_field) != clip.get(clip_field) {
            return Err(invalid(format!(
                "material-surface quality {quality_field} differs"
            )));
        }
    }
    if quality.get("hard_gate_passed").and_then(Value::as_bool) != Some(true)
        || quality.get("validator_status").and_then(Value::as_str) != Some("passed")
        || quality.get("quality_status").and_then(Value::as_str) != Some("structural_only")
    {
        return Err(invalid("material-surface quality gate is not passed"));
    }
    for field in [
        "material_surface_quality_canonical_sha256",
        "texture_build_receipt_object_sha256",
        "texture_build_receipt_canonical_sha256",
        "candidate_surface_bake_receipt_object_sha256",
        "candidate_surface_bake_receipt_canonical_sha256",
        "material_pack_version",
        "material_pack_license_spdx",
        "material_pack_manifest_object_sha256",
        "material_pack_provenance_sha256",
    ] {
        if let Some(value) = quality.get(field) {
            if value.is_string() && value.as_str().unwrap_or_default().is_empty() {
                return Err(invalid(format!(
                    "material-surface quality {field} is empty"
                )));
            }
        }
    }
    let quality_bytes =
        canonical_json_bytes(&quality).map_err(|error| invalid(error.to_string()))?;
    if sha256_hex(&quality_bytes)
        != clip["material_surface_quality_report_object_sha256"]
            .as_str()
            .unwrap_or_default()
    {
        return Err(invalid("material-surface quality report hash differs"));
    }
    read_json(
        runtime,
        clip["material_surface_quality_report_object_sha256"]
            .as_str()
            .ok_or_else(|| invalid("quality report object hash is missing"))?,
        "candidate-material-surface-quality-report",
    )?;
    let appearance_artifact = clip["appearance_artifact_sha256"]
        .as_str()
        .ok_or_else(|| invalid("appearance artifact hash is missing"))?;
    let appearance_readback = runtime.artifact_readback_bounded(
        appearance_artifact,
        appearance_candidate_id,
        MAX_GEOMETRY_ARTIFACT_BYTES,
    )?;
    if appearance_readback.get("canonical_sha256")
        != clip.get("appearance_artifact_readback_sha256")
        || appearance_readback
            .get("object_sha256")
            .and_then(Value::as_str)
            != Some(appearance_artifact)
    {
        return Err(invalid("appearance ArtifactReadback binding differs"));
    }
    read_json(
        runtime,
        clip["appearance_artifact_readback_object_sha256"]
            .as_str()
            .ok_or_else(|| invalid("appearance readback object hash is missing"))?,
        "appearance-v2-artifact-readback",
    )?;
    let artifact_object = runtime
        .store
        .get_object(appearance_artifact)?
        .ok_or_else(|| invalid("appearance artifact object is unavailable"))?;
    if artifact_object.mime != GLB_MIME
        || !matches!(
            artifact_object.kind.as_str(),
            "appearance-glb" | "appearance-v2-glb"
        )
        || artifact_object.size_bytes == 0
        || artifact_object.size_bytes > MAX_GLB_BYTES as u64
    {
        return Err(invalid("appearance artifact metadata differs"));
    }
    let source_glb = runtime.cas_read_bounded(appearance_artifact, MAX_GLB_BYTES as u64)?;
    if sha256_hex(&source_glb) != appearance_artifact {
        return Err(invalid("appearance artifact bytes differ from its hash"));
    }
    let quality_pack_id = quality
        .get("material_pack_id")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| invalid("material pack id is missing"))?;
    let quality_pack_manifest = quality
        .get("material_pack_manifest_sha256")
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .ok_or_else(|| invalid("material pack manifest hash is missing"))?;
    validate_glb_material_pack_identity(&source_glb, quality_pack_id, quality_pack_manifest)?;
    Ok(quality)
}

fn quality_string(quality: &Value, field: &str) -> Result<String, RuntimeError> {
    quality
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| invalid(format!("material-surface quality {field} is missing")))
}

fn load_context(
    runtime: &Runtime,
    request: &Value,
    clip: &Value,
) -> Result<ClipContext, RuntimeError> {
    let quality = validate_clip_scope(runtime, request, clip)?;
    let object = request
        .as_object()
        .ok_or_else(|| invalid("animation GLB request is not an object"))?;
    let project_id = id(object, "project_id")?.to_owned();
    let appearance_candidate_id = id(object, "appearance_candidate_id")?.to_owned();
    let clip_id = id(object, "clip_id")?.to_owned();
    let clip_object_sha256 = sha(object, "clip_object_sha256")?;
    let clip_sha256 = sha(object, "clip_sha256")?;
    if clip.get("canonical_sha256").and_then(Value::as_str) != Some(clip_sha256) {
        return Err(invalid("Clip@2 canonical hash differs from the request"));
    }
    let source_sha256 = clip
        .get("appearance_artifact_sha256")
        .and_then(Value::as_str)
        .filter(|hash| is_sha256(hash))
        .ok_or_else(|| invalid("appearance artifact hash is missing"))?;
    let source_glb = runtime.cas_read_bounded(source_sha256, MAX_GLB_BYTES as u64)?;
    if sha256_hex(&source_glb) != source_sha256 {
        return Err(invalid("appearance artifact bytes differ from Clip@2"));
    }
    let (ticks, frames, part_ids) = collect_frames(
        runtime,
        &project_id,
        &appearance_candidate_id,
        &clip_id,
        clip_object_sha256,
        clip_sha256,
        clip,
        &quality,
    )?;
    let created_at = clip
        .get("created_at")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| invalid("Clip@2 created_at is missing"))?
        .to_owned();
    if quality.get("created_at").and_then(Value::as_str) != Some(created_at.as_str()) {
        return Err(invalid("Clip@2 and quality created_at differ"));
    }
    Ok(ClipContext {
        request: request.clone(),
        clip: clip.clone(),
        quality: quality.clone(),
        source_glb,
        ticks,
        frames,
        part_ids,
        created_at,
        material_pack_id: quality_string(&quality, "material_pack_id")?,
        material_pack_manifest_sha256: quality_string(&quality, "material_pack_manifest_sha256")?,
    })
}

fn parse_ticks(clip: &Value) -> Result<Vec<u64>, RuntimeError> {
    let ticks = clip
        .get("sampling_policy")
        .and_then(|value| value.get("sample_time_ticks"))
        .and_then(Value::as_array)
        .filter(|values| (2..=MAX_SAMPLES).contains(&values.len()))
        .ok_or_else(|| invalid("MechanicalAnimationClip@2 requires 2..16 scheduled ticks"))?;
    let mut result = Vec::with_capacity(ticks.len());
    for value in ticks {
        let tick = value
            .as_u64()
            .filter(|tick| *tick <= 1_000_000)
            .ok_or_else(|| invalid("scheduled tick is invalid"))?;
        if result.last().is_some_and(|prior| *prior >= tick) {
            return Err(invalid("scheduled ticks must be strictly increasing"));
        }
        result.push(tick);
    }
    Ok(result)
}

fn parse_transform(value: &Value) -> Result<RigidTransform, RuntimeError> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid("Part delta pose is not an object"))?;
    let read = |field: &str, count: usize| -> Result<Vec<f32>, RuntimeError> {
        let values = object
            .get(field)
            .and_then(Value::as_array)
            .filter(|values| values.len() == count)
            .ok_or_else(|| invalid(format!("delta pose {field} is invalid")))?;
        values
            .iter()
            .map(|value| {
                value
                    .as_f64()
                    .filter(|value| value.is_finite())
                    .map(|value| value as f32)
                    .ok_or_else(|| invalid(format!("delta pose {field} contains non-finite data")))
            })
            .collect()
    };
    let translation = read("translation_m", 3)?;
    let rotation = read("rotation_quat_xyzw", 4)?;
    let _scale = object
        .get("scale")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("delta pose scale is missing"))?;
    if object.get("scale") != Some(&json!([1.0, 1.0, 1.0]))
        || translation.iter().any(|value| value.abs() > 10.0)
        || rotation.iter().any(|value| !value.is_finite())
    {
        return Err(invalid("delta pose is outside rigid TRS bounds"));
    }
    let norm = rotation
        .iter()
        .map(|value| value * value)
        .sum::<f32>()
        .sqrt();
    if (norm - 1.0).abs() > 1.0e-4 {
        return Err(invalid("delta pose quaternion is not unit length"));
    }
    Ok(RigidTransform {
        translation: [translation[0], translation[1], translation[2]],
        rotation: [rotation[0], rotation[1], rotation[2], rotation[3]],
    })
}

fn collect_frames(
    runtime: &Runtime,
    project_id: &str,
    appearance_candidate_id: &str,
    clip_id: &str,
    clip_object_sha256: &str,
    clip_sha256: &str,
    clip: &Value,
    quality: &Value,
) -> Result<(Vec<u64>, Vec<Frame>, Vec<String>), RuntimeError> {
    let ticks = parse_ticks(clip)?;
    let appearance_artifact_sha256 = clip
        .get("appearance_artifact_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("appearance artifact hash is missing"))?;
    let mut frames = Vec::with_capacity(ticks.len());
    let mut expected_parts: Option<Vec<String>> = None;
    for tick in &ticks {
        let mut request = json!({
            "schema_version":"MechanicalAnimationClipPreviewRequest@2",
            "project_id":project_id,
            "appearance_candidate_id":appearance_candidate_id,
            "clip_id":clip_id,
            "sample_time_ticks":tick,
            "preview_policy":CLIP_PREVIEW_POLICY,
            "canonical_sha256":""
        });
        set_canonical(&mut request, "canonical_sha256")?;
        let preview = runtime.mechanical_animation_clip_v2_preview_get(&request)?;
        if preview
            .get("runtime_write_performed")
            .and_then(Value::as_bool)
            != Some(false)
            || preview
                .get("persistent_user_data_touched")
                .and_then(Value::as_bool)
                != Some(false)
            || preview.get("quality_status").and_then(Value::as_str) != Some("structural_only")
            || preview.get("clip_id").and_then(Value::as_str) != Some(clip_id)
            || preview.get("clip_object_sha256").and_then(Value::as_str) != Some(clip_object_sha256)
            || preview.get("clip_sha256").and_then(Value::as_str) != Some(clip_sha256)
            || preview
                .get("appearance_artifact_sha256")
                .and_then(Value::as_str)
                != Some(appearance_artifact_sha256)
            || preview
                .get("material_pack_manifest_sha256")
                .and_then(Value::as_str)
                != quality
                    .get("material_pack_manifest_sha256")
                    .and_then(Value::as_str)
        {
            return Err(invalid("V2 preview binding or safe flags differ"));
        }
        if preview.get("sample_time_ticks").and_then(Value::as_u64) != Some(*tick)
            || preview
                .get("source_replay_worker_cohort_sha256")
                .and_then(Value::as_str)
                != clip
                    .get("source_replay_worker_cohort_sha256")
                    .and_then(Value::as_str)
            || preview
                .get("appearance_replay_worker_cohort_sha256")
                .and_then(Value::as_str)
                != clip
                    .get("source_replay_worker_cohort_sha256")
                    .and_then(Value::as_str)
        {
            return Err(invalid("V2 preview schedule or worker cohort differs"));
        }
        let frame_sha256 = preview
            .get("frame_sha256")
            .and_then(Value::as_str)
            .filter(|value| is_sha256(value))
            .ok_or_else(|| invalid("V2 preview frame hash is missing"))?
            .to_owned();
        let deltas = preview
            .get("pose_geometry_preview")
            .and_then(|value| value.get("part_deltas"))
            .and_then(Value::as_array)
            .ok_or_else(|| invalid("V2 preview omitted Part deltas"))?;
        let mut frame = BTreeMap::new();
        for delta in deltas {
            let part_id = delta
                .get("part_id")
                .and_then(Value::as_str)
                .filter(|value| is_opaque_id(value))
                .ok_or_else(|| invalid("V2 preview Part ID is invalid"))?;
            let pose = delta
                .get("delta_pose")
                .ok_or_else(|| invalid("V2 preview delta pose is missing"))?;
            if frame
                .insert(part_id.to_owned(), parse_transform(pose)?)
                .is_some()
            {
                return Err(invalid("V2 preview duplicates a Part"));
            }
        }
        let parts = frame.keys().cloned().collect::<Vec<_>>();
        if expected_parts
            .as_ref()
            .is_some_and(|expected| expected != &parts)
        {
            return Err(invalid("V2 preview frames differ in Part coverage"));
        }
        expected_parts.get_or_insert(parts);
        frames.push(Frame {
            frame_sha256,
            deltas: frame,
        });
    }
    let parts = expected_parts.ok_or_else(|| invalid("V2 preview has no animated Parts"))?;
    Ok((ticks, frames, parts))
}

fn ensure_static_source(root: &Value) -> Result<(), RuntimeError> {
    if root.get("animations").is_some() {
        return Err(invalid("appearance source GLB already contains animations"));
    }
    if root.get("skins").is_some() {
        return Err(invalid("appearance source GLB contains skins"));
    }
    if let Some(meshes) = root.get("meshes").and_then(Value::as_array) {
        for mesh in meshes {
            if let Some(primitives) = mesh.get("primitives").and_then(Value::as_array) {
                for primitive in primitives {
                    if primitive.get("targets").is_some() {
                        return Err(invalid("appearance source GLB contains morph targets"));
                    }
                }
            }
        }
    }
    Ok(())
}

fn materialize(
    source_glb: &[u8],
    source_sha256: &str,
    key_sha256: &str,
    clip_sha256: &str,
    part_ids: &[String],
    ticks: &[u64],
    frames: &[Frame],
) -> Result<Vec<u8>, RuntimeError> {
    if part_ids.is_empty()
        || part_ids.len() > 64
        || ticks.len() < 2
        || ticks.len() > MAX_SAMPLES
        || frames.len() != ticks.len()
    {
        return Err(invalid(
            "animation schedule or Part coverage is outside its bound",
        ));
    }
    let (mut root, mut binary) = parse_glb(source_glb)?;
    ensure_static_source(&root)?;
    let nodes = root
        .get("nodes")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("appearance source GLB nodes are missing"))?;
    let mut node_by_part = BTreeMap::new();
    for (index, node) in nodes.iter().enumerate() {
        let Some(name) = node.get("name").and_then(Value::as_str) else {
            continue;
        };
        if part_ids.iter().any(|part| part == name)
            && node_by_part.insert(name.to_owned(), index).is_some()
        {
            return Err(invalid(
                "appearance source GLB has duplicate Part node owners",
            ));
        }
    }
    if node_by_part.keys().cloned().collect::<Vec<_>>() != part_ids {
        return Err(invalid(
            "appearance source GLB does not have exact-one node owner for every animated Part",
        ));
    }
    for frame in frames {
        if frame.deltas.keys().cloned().collect::<Vec<_>>() != part_ids {
            return Err(invalid("animation frame Part coverage differs"));
        }
    }
    let source_accessor_count = root
        .get("accessors")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("appearance source GLB accessors are missing"))?
        .len();
    let source_view_count = root
        .get("bufferViews")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("appearance source GLB bufferViews are missing"))?
        .len();
    let times = ticks
        .iter()
        .map(|tick| *tick as f32 / TIMEBASE_HZ as f32)
        .collect::<Vec<_>>();
    let time_accessor = append_accessor(
        &mut root,
        &mut binary,
        &times,
        ticks.len(),
        "SCALAR",
        Some(json!([times[0]])),
        Some(json!([times[times.len() - 1]])),
    )?;
    let mut samplers = Vec::with_capacity(part_ids.len() * 2);
    let mut channels = Vec::with_capacity(part_ids.len() * 2);
    for part_id in part_ids {
        let translations = frames
            .iter()
            .flat_map(|frame| frame.deltas[part_id].translation)
            .collect::<Vec<_>>();
        let rotations = frames
            .iter()
            .flat_map(|frame| frame.deltas[part_id].rotation)
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
        samplers.push(json!({
            "input":time_accessor,
            "output":translation_accessor,
            "interpolation":"LINEAR"
        }));
        channels.push(json!({
            "sampler":translation_sampler,
            "target":{"node":node,"path":"translation"}
        }));
        let rotation_sampler = samplers.len();
        samplers.push(json!({
            "input":time_accessor,
            "output":rotation_accessor,
            "interpolation":"LINEAR"
        }));
        channels.push(json!({
            "sampler":rotation_sampler,
            "target":{"node":node,"path":"rotation"}
        }));
    }
    root["animations"] = json!([{
        "name":ANIMATION_NAME,
        "samplers":samplers,
        "channels":channels
    }]);
    let buffers = root
        .get_mut("buffers")
        .and_then(Value::as_array_mut)
        .filter(|buffers| !buffers.is_empty())
        .ok_or_else(|| invalid("appearance source GLB buffers are missing"))?;
    buffers[0]["byteLength"] = Value::from(binary.len() as u64);
    let mut metadata = json!({
        "schema_version":RECEIPT_SCHEMA,
        "animation_glb_key_sha256":key_sha256,
        "source_artifact_sha256":source_sha256,
        "clip_sha256":clip_sha256,
        "sample_time_ticks":ticks,
        "timebase_hz":TIMEBASE_HZ,
        "interpolation":"LINEAR",
        "part_ids":part_ids,
        "source_accessor_count":source_accessor_count,
        "source_buffer_view_count":source_view_count,
        "canonical_sha256":""
    });
    set_canonical(&mut metadata, "canonical_sha256")?;
    root["extras"]["forgecad"]["mechanical_animation_v2"] = metadata;
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
            return Err(invalid("animation accessor contains a non-finite value"));
        }
        binary.extend_from_slice(&value.to_le_bytes());
    }
    let views = root
        .get_mut("bufferViews")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| invalid("appearance source GLB bufferViews are invalid"))?;
    let view_index = views.len();
    views.push(json!({
        "buffer":0,
        "byteOffset":offset,
        "byteLength":values.len() * 4
    }));
    let accessors = root
        .get_mut("accessors")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| invalid("appearance source GLB accessors are invalid"))?;
    let accessor_index = accessors.len();
    let mut accessor = json!({
        "bufferView":view_index,
        "componentType":5126,
        "count":count,
        "type":kind
    });
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
    key_sha256: &str,
    clip_sha256: &str,
    material_pack_id: &str,
    material_pack_manifest_sha256: &str,
    part_ids: &[String],
    ticks: &[u64],
    frames: &[Frame],
) -> Result<Value, RuntimeError> {
    if animated_glb.is_empty()
        || animated_glb.len() > MAX_GLB_BYTES
        || sha256_hex(source_glb) != source_sha256
        || sha256_hex(animated_glb) == source_sha256
    {
        return Err(invalid("animated GLB size or source binding is invalid"));
    }
    let (source_root, source_binary) = parse_glb(source_glb)?;
    let (root, binary) = parse_glb(animated_glb)?;
    ensure_static_source(&source_root)?;
    if root.get("skins").is_some() {
        return Err(invalid("animated GLB contains skins"));
    }
    if let Some(meshes) = root.get("meshes").and_then(Value::as_array) {
        for mesh in meshes {
            for primitive in mesh
                .get("primitives")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                if primitive.get("targets").is_some() {
                    return Err(invalid("animated GLB contains morph targets"));
                }
            }
        }
    }
    validate_glb_material_pack_identity(
        source_glb,
        material_pack_id,
        material_pack_manifest_sha256,
    )?;
    validate_glb_material_pack_identity(
        animated_glb,
        material_pack_id,
        material_pack_manifest_sha256,
    )?;
    let metadata = root
        .get("extras")
        .and_then(|value| value.get("forgecad"))
        .and_then(|value| value.get("mechanical_animation_v2"))
        .and_then(Value::as_object)
        .ok_or_else(|| invalid("animated GLB V2 metadata is missing"))?;
    verify_canonical(&Value::Object(metadata.clone()), "canonical_sha256")?;
    if metadata.get("schema_version").and_then(Value::as_str) != Some(RECEIPT_SCHEMA)
        || metadata
            .get("animation_glb_key_sha256")
            .and_then(Value::as_str)
            != Some(key_sha256)
        || metadata
            .get("source_artifact_sha256")
            .and_then(Value::as_str)
            != Some(source_sha256)
        || metadata.get("clip_sha256").and_then(Value::as_str) != Some(clip_sha256)
        || metadata.get("sample_time_ticks") != Some(&json!(ticks))
        || metadata.get("timebase_hz").and_then(Value::as_u64) != Some(TIMEBASE_HZ)
        || metadata.get("interpolation").and_then(Value::as_str) != Some("LINEAR")
        || metadata.get("part_ids") != Some(&json!(part_ids))
    {
        return Err(invalid("animated GLB V2 metadata binding differs"));
    }
    if binary.get(..source_binary.len()) != Some(source_binary.as_slice()) {
        return Err(invalid(
            "animated GLB BIN does not preserve the complete source prefix",
        ));
    }
    let source_accessors = source_root
        .get("accessors")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("source accessors are missing"))?
        .len();
    let source_views = source_root
        .get("bufferViews")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("source bufferViews are missing"))?
        .len();
    let mut projected = root.clone();
    projected
        .as_object_mut()
        .ok_or_else(|| invalid("animated GLB root is not an object"))?
        .remove("animations");
    projected["accessors"]
        .as_array_mut()
        .ok_or_else(|| invalid("animated accessors are missing"))?
        .truncate(source_accessors);
    projected["bufferViews"]
        .as_array_mut()
        .ok_or_else(|| invalid("animated bufferViews are missing"))?
        .truncate(source_views);
    projected["buffers"][0]["byteLength"] = source_root["buffers"][0]["byteLength"].clone();
    if let Some(forgecad) = projected
        .get_mut("extras")
        .and_then(|value| value.get_mut("forgecad"))
        .and_then(Value::as_object_mut)
    {
        forgecad.remove("mechanical_animation_v2");
    }
    if projected != source_root {
        return Err(invalid(
            "animated GLB cannot reconstruct the exact source static projection",
        ));
    }
    let materials_projection = |value: &Value| {
        json!({
            "materials":value.get("materials").cloned().unwrap_or(Value::Null),
            "textures":value.get("textures").cloned().unwrap_or(Value::Null),
            "images":value.get("images").cloned().unwrap_or(Value::Null),
            "samplers":value.get("samplers").cloned().unwrap_or(Value::Null),
            "meshes":value.get("meshes").cloned().unwrap_or(Value::Null)
        })
    };
    let source_materials = materials_projection(&source_root);
    let animated_materials = materials_projection(&root);
    if source_materials != animated_materials {
        return Err(invalid(
            "animated GLB material/textures/images/samplers/meshes projection differs",
        ));
    }
    let animation = root
        .get("animations")
        .and_then(Value::as_array)
        .filter(|animations| animations.len() == 1)
        .and_then(|animations| animations.first())
        .ok_or_else(|| invalid("animated GLB must contain exactly one animation"))?;
    let animation_object = exact_object(
        animation,
        &["name", "samplers", "channels"],
        "appearance-aware glTF animation",
    )?;
    if animation_object.get("name").and_then(Value::as_str) != Some(ANIMATION_NAME) {
        return Err(invalid("animation name differs"));
    }
    let samplers = animation
        .get("samplers")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("animation samplers are invalid"))?;
    let channels = animation
        .get("channels")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("animation channels are invalid"))?;
    if samplers.len() != part_ids.len() * 2 || channels.len() != part_ids.len() * 2 {
        return Err(invalid(
            "animation sampler/channel count differs from Part coverage",
        ));
    }
    let accessors = root
        .get("accessors")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("animated accessors are invalid"))?;
    let views = root
        .get("bufferViews")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("animated bufferViews are invalid"))?;
    let expected_times = ticks
        .iter()
        .map(|tick| *tick as f32 / TIMEBASE_HZ as f32)
        .collect::<Vec<_>>();
    let mut seen_channels = BTreeSet::new();
    let mut seen_samplers = BTreeSet::new();
    let mut seen_accessors = BTreeSet::new();
    for channel in channels {
        exact_object(
            channel,
            &["sampler", "target"],
            "appearance animation channel",
        )?;
        exact_object(
            channel
                .get("target")
                .ok_or_else(|| invalid("animation target is missing"))?,
            &["node", "path"],
            "appearance animation target",
        )?;
        let sampler_index = channel
            .get("sampler")
            .and_then(Value::as_u64)
            .ok_or_else(|| invalid("channel sampler is invalid"))?
            as usize;
        let sampler = samplers
            .get(sampler_index)
            .ok_or_else(|| invalid("channel sampler index overflowed"))?;
        exact_object(
            sampler,
            &["input", "output", "interpolation"],
            "appearance animation sampler",
        )?;
        if !seen_samplers.insert(sampler_index)
            || sampler.get("interpolation").and_then(Value::as_str) != Some("LINEAR")
        {
            return Err(invalid("animation sampler is reused or not LINEAR"));
        }
        let node_index = channel["target"]["node"]
            .as_u64()
            .ok_or_else(|| invalid("channel target node is invalid"))?
            as usize;
        let part_id = root
            .get("nodes")
            .and_then(Value::as_array)
            .and_then(|nodes| nodes.get(node_index))
            .and_then(|node| node.get("name"))
            .and_then(Value::as_str)
            .ok_or_else(|| invalid("animated Part node name is missing"))?;
        let path = channel["target"]["path"]
            .as_str()
            .ok_or_else(|| invalid("channel target path is invalid"))?;
        if !part_ids.iter().any(|part| part == part_id)
            || !matches!(path, "translation" | "rotation")
            || !seen_channels.insert((part_id.to_owned(), path.to_owned()))
        {
            return Err(invalid("animation channel target coverage differs"));
        }
        let input_index = sampler["input"]
            .as_u64()
            .ok_or_else(|| invalid("animation input accessor is invalid"))?
            as usize;
        let output_index = sampler["output"]
            .as_u64()
            .ok_or_else(|| invalid("animation output accessor is invalid"))?
            as usize;
        if input_index != source_accessors || output_index <= source_accessors {
            return Err(invalid("animation accessor ownership differs"));
        }
        seen_accessors.insert(input_index);
        if !seen_accessors.insert(output_index) {
            return Err(invalid("animation output accessor is reused"));
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
            return Err(invalid(
                "animation time accessor differs from scheduled ticks",
            ));
        }
        let output_kind = if path == "translation" {
            "VEC3"
        } else {
            "VEC4"
        };
        let output = read_f32_accessor(
            accessors,
            views,
            &binary,
            output_index,
            output_kind,
            ticks.len(),
        )?;
        let expected = if path == "translation" {
            frames
                .iter()
                .flat_map(|frame| frame.deltas[part_id].translation)
                .collect::<Vec<_>>()
        } else {
            frames
                .iter()
                .flat_map(|frame| frame.deltas[part_id].rotation)
                .collect::<Vec<_>>()
        };
        if output != expected {
            return Err(invalid(
                "animation output accessor differs from verified Part delta",
            ));
        }
    }
    if seen_channels.len() != part_ids.len() * 2
        || seen_samplers.len() != samplers.len()
        || seen_accessors != (source_accessors..accessors.len()).collect::<BTreeSet<_>>()
    {
        return Err(invalid(
            "animation has unconsumed channels, samplers or accessors",
        ));
    }
    validate_added_buffer_layout(views, source_views, &binary, source_binary.len())?;
    Ok(json!({
        "schema_version":"MechanicalAnimationGlbValidation@2",
        "source_artifact_sha256":source_sha256,
        "animated_artifact_sha256":sha256_hex(animated_glb),
        "animation_glb_key_sha256":key_sha256,
        "clip_sha256":clip_sha256,
        "node_count":part_ids.len(),
        "sampler_count":samplers.len(),
        "channel_count":channels.len(),
        "accessor_count_added":accessors.len() - source_accessors,
        "buffer_view_count_added":views.len() - source_views,
        "frame_count":ticks.len(),
        "source_static_projection_sha256":canonical_json_hash(&source_root),
        "appearance_material_projection_sha256":canonical_json_hash(&source_materials),
        "source_static_projection_exact":true,
        "binary_prefix_exact":true,
        "appearance_material_projection_exact":true,
        "material_pack_identity_exact":true,
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
        .ok_or_else(|| invalid("animation accessor is invalid"))?;
    if accessor.get("componentType").and_then(Value::as_u64) != Some(5126)
        || accessor.get("count").and_then(Value::as_u64) != Some(expected_count as u64)
        || accessor.get("type").and_then(Value::as_str) != Some(expected_kind)
        || accessor.get("byteOffset").is_some()
        || accessor.get("sparse").is_some()
    {
        return Err(invalid(
            "animation accessor shape or component type differs",
        ));
    }
    let view_index = accessor
        .get("bufferView")
        .and_then(Value::as_u64)
        .ok_or_else(|| invalid("animation accessor view is missing"))?
        as usize;
    let view = views
        .get(view_index)
        .and_then(Value::as_object)
        .ok_or_else(|| invalid("animation buffer view is invalid"))?;
    let offset = view.get("byteOffset").and_then(Value::as_u64).unwrap_or(0) as usize;
    let length =
        view.get("byteLength")
            .and_then(Value::as_u64)
            .ok_or_else(|| invalid("animation buffer view length is missing"))? as usize;
    let component_count = match expected_kind {
        "SCALAR" => 1,
        "VEC3" => 3,
        "VEC4" => 4,
        _ => return Err(invalid("animation accessor kind is unsupported")),
    };
    if view.get("buffer").and_then(Value::as_u64) != Some(0)
        || view.get("byteStride").is_some()
        || length != expected_count * component_count * 4
    {
        return Err(invalid("animation buffer view layout differs"));
    }
    let end = offset
        .checked_add(length)
        .ok_or_else(|| invalid("animation buffer view overflowed"))?;
    let bytes = binary
        .get(offset..end)
        .ok_or_else(|| invalid("animation buffer view exceeds BIN"))?;
    if bytes.len() % 4 != 0 {
        return Err(invalid("animation float buffer is misaligned"));
    }
    let values = bytes
        .chunks_exact(4)
        .map(|chunk| f32::from_le_bytes(chunk.try_into().unwrap()))
        .collect::<Vec<_>>();
    if values.iter().any(|value| !value.is_finite()) {
        return Err(invalid("animation accessor contains a non-finite value"));
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
            .ok_or_else(|| invalid("animation buffer view offset is invalid"))?
            as usize;
        let length = object
            .get("byteLength")
            .and_then(Value::as_u64)
            .ok_or_else(|| invalid("animation buffer view length is invalid"))?
            as usize;
        while cursor % 4 != 0 {
            if binary.get(cursor) != Some(&0) {
                return Err(invalid("animation alignment padding is non-zero"));
            }
            cursor += 1;
        }
        if object.get("buffer").and_then(Value::as_u64) != Some(0) || offset != cursor {
            return Err(invalid("animation buffer views are not contiguous"));
        }
        cursor = cursor
            .checked_add(length)
            .filter(|end| *end <= binary.len())
            .ok_or_else(|| invalid("animation buffer view exceeds BIN"))?;
    }
    if cursor != binary.len() {
        return Err(invalid("animated GLB has hidden BIN tail bytes"));
    }
    Ok(())
}

fn parse_glb(bytes: &[u8]) -> Result<(Value, Vec<u8>), RuntimeError> {
    if bytes.len() < 28
        || &bytes[..4] != b"glTF"
        || u32::from_le_bytes(bytes[4..8].try_into().unwrap()) != 2
        || u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize != bytes.len()
    {
        return Err(invalid("GLB header is invalid"));
    }
    let json_length = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    let json_end = 20usize
        .checked_add(json_length)
        .ok_or_else(|| invalid("GLB JSON length overflowed"))?;
    if json_end + 8 > bytes.len()
        || &bytes[16..20] != b"JSON"
        || &bytes[json_end + 4..json_end + 8] != b"BIN\0"
    {
        return Err(invalid("GLB chunks are invalid"));
    }
    let binary_length =
        u32::from_le_bytes(bytes[json_end..json_end + 4].try_into().unwrap()) as usize;
    let binary_start = json_end + 8;
    let binary_end = binary_start
        .checked_add(binary_length)
        .filter(|end| *end == bytes.len())
        .ok_or_else(|| invalid("GLB BIN length differs"))?;
    let root = serde_json::from_slice(&bytes[20..json_end])
        .map_err(|error| invalid(format!("GLB JSON is invalid: {error}")))?;
    Ok((root, bytes[binary_start..binary_end].to_vec()))
}

fn encode_glb(root: &Value, binary: &[u8]) -> Result<Vec<u8>, RuntimeError> {
    let mut json_bytes = serde_json::to_vec(root).map_err(|error| invalid(error.to_string()))?;
    while json_bytes.len() % 4 != 0 {
        json_bytes.push(b' ');
    }
    let total = 12usize
        .checked_add(8)
        .and_then(|value| value.checked_add(json_bytes.len()))
        .and_then(|value| value.checked_add(8))
        .and_then(|value| value.checked_add(binary.len()))
        .ok_or_else(|| invalid("animated GLB size overflowed"))?;
    if total > MAX_GLB_BYTES || total > u32::MAX as usize {
        return Err(invalid("animated GLB exceeds its size budget"));
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

fn frame_preview_hashes_sha256(frames: &[Frame]) -> String {
    canonical_json_hash(&Value::Array(
        frames
            .iter()
            .map(|frame| Value::String(frame.frame_sha256.clone()))
            .collect(),
    ))
}

fn receipt_value(
    context: &ClipContext,
    key_sha256: &str,
    animated_artifact_sha256: &str,
    animated_artifact_readback_sha256: &str,
    validation: &Value,
) -> Result<MechanicalAnimationGlbV2ReceiptRecord, RuntimeError> {
    let clip = context
        .clip
        .as_object()
        .ok_or_else(|| invalid("Clip@2 is not an object"))?;
    let quality = context
        .quality
        .as_object()
        .ok_or_else(|| invalid("material-surface quality is not an object"))?;
    let source_static_projection_sha256 = validation
        .get("source_static_projection_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("source static projection hash is missing"))?;
    let appearance_material_projection_sha256 = validation
        .get("appearance_material_projection_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("appearance material projection hash is missing"))?;
    let frame_preview_hashes_sha256 = frame_preview_hashes_sha256(&context.frames);
    let mut receipt = json!({
        "schema_version":RECEIPT_SCHEMA,
        "animation_glb_key_sha256":key_sha256,
        "project_id":clip["project_id"],
        "appearance_candidate_id":clip["appearance_candidate_id"],
        "appearance_candidate_state_sha256":clip["appearance_candidate_state_sha256"],
        "appearance_artifact_id":clip["appearance_artifact_id"],
        "appearance_artifact_sha256":clip["appearance_artifact_sha256"],
        "appearance_artifact_readback_sha256":clip["appearance_artifact_readback_sha256"],
        "appearance_artifact_readback_object_sha256":clip["appearance_artifact_readback_object_sha256"],
        "source_geometry_candidate_id":clip["source_geometry_candidate_id"],
        "source_geometry_candidate_state_sha256":clip["source_geometry_candidate_state_sha256"],
        "source_geometry_artifact_id":clip["source_geometry_artifact_id"],
        "source_geometry_artifact_sha256":clip["source_geometry_artifact_sha256"],
        "source_geometry_candidate_evidence_sha256":clip["source_geometry_candidate_evidence_sha256"],
        "material_surface_quality_id":clip["material_surface_quality_id"],
        "material_surface_quality_report_object_sha256":clip["material_surface_quality_report_object_sha256"],
        "material_surface_quality_canonical_sha256":clip["material_surface_quality_canonical_sha256"],
        "appearance_source_lineage_sidecar_object_sha256":clip["appearance_source_lineage_sidecar_object_sha256"],
        "appearance_source_lineage_canonical_sha256":clip["appearance_source_lineage_canonical_sha256"],
        "appearance_program_object_sha256":clip["appearance_program_object_sha256"],
        "appearance_program_sha256":clip["appearance_program_sha256"],
        "geometry_program_object_sha256":clip["geometry_program_object_sha256"],
        "geometry_program_sha256":clip["geometry_program_sha256"],
        "geometry_preservation_projection_sha256":clip["geometry_preservation_projection_sha256"],
        "operator_catalog_sha256":clip["operator_catalog_sha256"],
        "readback_config_sha256":clip["readback_config_sha256"],
        "material_pack_id":quality["material_pack_id"],
        "material_pack_version":quality["material_pack_version"],
        "material_pack_license_spdx":quality["material_pack_license_spdx"],
        "material_pack_manifest_object_sha256":quality["material_pack_manifest_object_sha256"],
        "material_pack_manifest_sha256":quality["material_pack_manifest_sha256"],
        "material_pack_provenance_sha256":quality["material_pack_provenance_sha256"],
        "texture_build_receipt_object_sha256":quality["texture_build_receipt_object_sha256"],
        "texture_build_receipt_canonical_sha256":quality["texture_build_receipt_canonical_sha256"],
        "candidate_surface_bake_receipt_object_sha256":quality["candidate_surface_bake_receipt_object_sha256"],
        "candidate_surface_bake_receipt_canonical_sha256":quality["candidate_surface_bake_receipt_canonical_sha256"],
        "clip_id":clip["clip_id"],
        "clip_object_sha256":context.request["clip_object_sha256"],
        "clip_sha256":clip["canonical_sha256"],
        "rest_frame_sha256":clip["rest_frame_sha256"],
        "pose_action_sha256":clip["pose_action_sha256"],
        "sampling_policy_sha256":clip["sampling_policy_sha256"],
        "source_replay_worker_cohort_sha256":clip["source_replay_worker_cohort_sha256"],
        "frame_preview_hashes_sha256":frame_preview_hashes_sha256,
        "frame_preview_worker_cohort_sha256":clip["source_replay_worker_cohort_sha256"],
        "sample_time_ticks":context.ticks,
        "timebase_hz":TIMEBASE_HZ,
        "interpolation":"LINEAR",
        "part_ids":context.part_ids,
        "node_count":validation["node_count"],
        "sampler_count":validation["sampler_count"],
        "channel_count":validation["channel_count"],
        "accessor_count_added":validation["accessor_count_added"],
        "buffer_view_count_added":validation["buffer_view_count_added"],
        "animated_artifact_sha256":animated_artifact_sha256,
        "animated_artifact_readback_sha256":animated_artifact_readback_sha256,
        "animation_validation_sha256":canonical_json_hash(validation),
        "source_static_projection_sha256":source_static_projection_sha256,
        "appearance_material_projection_sha256":appearance_material_projection_sha256,
        "source_static_projection_exact":true,
        "binary_prefix_exact":true,
        "appearance_material_projection_exact":true,
        "material_pack_identity_exact":true,
        "no_skinning":true,
        "no_morph_targets":true,
        "validator_status":VALIDATOR_STATUS,
        "hard_gate_passed":true,
        "materialization_status":STATUS,
        "runtime_write_performed":true,
        "production_stage_advanced":false,
        "candidate_confirmed":false,
        "version_created":false,
        "export_performed":false,
        "quality_status":"structural_only",
        "visual_quality_status":"NOT_PROVEN",
        "commercial_fps_quality_status":"NOT_PROVEN",
        "human_review_status":"NOT_RUN",
        "commercial_engine_status":"NOT_RUN",
        "limitations":[
            "appearance-candidate-bound-rigid-Part-TRS-only",
            "scheduled-integer-ticks-and-LINEAR-interpolation-only",
            "no-skinning-morph-targets-armature-IK-constraints-NLA-or-drivers",
            "source-BIN-and-appearance-material-projection-must-remain-exact",
            "structural-readback-does-not-prove-visual-quality-or-engine-roundtrip"
        ],
        "canonical_sha256":"",
        "created_at":context.created_at
    });
    set_canonical(&mut receipt, "canonical_sha256")?;
    serde_json::from_value(receipt).map_err(|error| {
        invalid(format!(
            "MechanicalAnimationGlbReceipt@2 is malformed: {error}"
        ))
    })
}

fn link_value(
    context: &ClipContext,
    key_sha256: &str,
    input_sha256: &str,
    animated_artifact_sha256: &str,
    animated_artifact_readback_sha256: &str,
    receipt_object_sha256: &str,
    receipt_canonical_sha256: &str,
) -> Result<MechanicalAnimationGlbV2LinkRecord, RuntimeError> {
    let clip = context
        .clip
        .as_object()
        .ok_or_else(|| invalid("Clip@2 is not an object"))?;
    let quality = context
        .quality
        .as_object()
        .ok_or_else(|| invalid("material-surface quality is not an object"))?;
    let mut link = json!({
        "schema_version":"MechanicalAnimationGlbLink@2",
        "animation_glb_key_sha256":key_sha256,
        "project_id":clip["project_id"],
        "appearance_candidate_id":clip["appearance_candidate_id"],
        "appearance_candidate_state_sha256":clip["appearance_candidate_state_sha256"],
        "appearance_artifact_id":clip["appearance_artifact_id"],
        "appearance_artifact_sha256":clip["appearance_artifact_sha256"],
        "appearance_artifact_readback_sha256":clip["appearance_artifact_readback_sha256"],
        "appearance_artifact_readback_object_sha256":clip["appearance_artifact_readback_object_sha256"],
        "source_geometry_candidate_id":clip["source_geometry_candidate_id"],
        "source_geometry_candidate_state_sha256":clip["source_geometry_candidate_state_sha256"],
        "source_geometry_artifact_id":clip["source_geometry_artifact_id"],
        "source_geometry_artifact_sha256":clip["source_geometry_artifact_sha256"],
        "source_geometry_candidate_evidence_sha256":clip["source_geometry_candidate_evidence_sha256"],
        "material_surface_quality_id":clip["material_surface_quality_id"],
        "material_surface_quality_report_object_sha256":clip["material_surface_quality_report_object_sha256"],
        "material_surface_quality_canonical_sha256":clip["material_surface_quality_canonical_sha256"],
        "appearance_source_lineage_sidecar_object_sha256":clip["appearance_source_lineage_sidecar_object_sha256"],
        "appearance_source_lineage_canonical_sha256":clip["appearance_source_lineage_canonical_sha256"],
        "appearance_program_object_sha256":clip["appearance_program_object_sha256"],
        "appearance_program_sha256":clip["appearance_program_sha256"],
        "geometry_program_object_sha256":clip["geometry_program_object_sha256"],
        "geometry_program_sha256":clip["geometry_program_sha256"],
        "geometry_preservation_projection_sha256":clip["geometry_preservation_projection_sha256"],
        "operator_catalog_sha256":clip["operator_catalog_sha256"],
        "readback_config_sha256":clip["readback_config_sha256"],
        "material_pack_id":quality["material_pack_id"],
        "material_pack_version":quality["material_pack_version"],
        "material_pack_license_spdx":quality["material_pack_license_spdx"],
        "material_pack_manifest_object_sha256":quality["material_pack_manifest_object_sha256"],
        "material_pack_manifest_sha256":quality["material_pack_manifest_sha256"],
        "material_pack_provenance_sha256":quality["material_pack_provenance_sha256"],
        "texture_build_receipt_object_sha256":quality["texture_build_receipt_object_sha256"],
        "texture_build_receipt_canonical_sha256":quality["texture_build_receipt_canonical_sha256"],
        "candidate_surface_bake_receipt_object_sha256":quality["candidate_surface_bake_receipt_object_sha256"],
        "candidate_surface_bake_receipt_canonical_sha256":quality["candidate_surface_bake_receipt_canonical_sha256"],
        "clip_id":clip["clip_id"],
        "clip_object_sha256":context.request["clip_object_sha256"],
        "clip_sha256":clip["canonical_sha256"],
        "rest_frame_sha256":clip["rest_frame_sha256"],
        "pose_action_sha256":clip["pose_action_sha256"],
        "sampling_policy_sha256":clip["sampling_policy_sha256"],
        "source_replay_worker_cohort_sha256":clip["source_replay_worker_cohort_sha256"],
        "animated_artifact_sha256":animated_artifact_sha256,
        "animated_artifact_readback_sha256":animated_artifact_readback_sha256,
        "receipt_object_sha256":receipt_object_sha256,
        "receipt_canonical_sha256":receipt_canonical_sha256,
        "request_sha256":input_sha256,
        "materialization_policy":POLICY,
        "validator_status":VALIDATOR_STATUS,
        "hard_gate_passed":true,
        "materialization_status":STATUS,
        "runtime_write_performed":true,
        "production_stage_advanced":false,
        "candidate_confirmed":false,
        "version_created":false,
        "export_performed":false,
        "quality_status":"structural_only",
        "visual_quality_status":"NOT_PROVEN",
        "commercial_fps_quality_status":"NOT_PROVEN",
        "human_review_status":"NOT_RUN",
        "commercial_engine_status":"NOT_RUN",
        "canonical_sha256":"",
        "created_at":context.created_at
    });
    set_canonical(&mut link, "canonical_sha256")?;
    serde_json::from_value(link).map_err(|error| {
        invalid(format!(
            "MechanicalAnimationGlbLink@2 is malformed: {error}"
        ))
    })
}

fn result_value(
    schema: &str,
    link: &MechanicalAnimationGlbV2LinkRecord,
    receipt: &MechanicalAnimationGlbV2ReceiptRecord,
    animated_artifact_size_bytes: u64,
    replayed: bool,
    runtime_write_performed: bool,
) -> Result<Value, RuntimeError> {
    Ok(json!({
        "schema_version":schema,
        "animation_glb_key_sha256":link.animation_glb_key_sha256,
        "animated_artifact_sha256":link.animated_artifact_sha256,
        "animated_artifact_size_bytes":animated_artifact_size_bytes,
        "receipt_object_sha256":link.receipt_object_sha256,
        "receipt":serde_json::to_value(receipt).map_err(|error| invalid(error.to_string()))?,
        "durable_link":serde_json::to_value(link).map_err(|error| invalid(error.to_string()))?,
        "replayed":replayed,
        "restart_hash_verified":true,
        "runtime_write_performed":runtime_write_performed,
        "production_stage_advanced":false,
        "candidate_confirmed":false,
        "version_created":false,
        "export_performed":false,
        "quality_status":"structural_only"
    }))
}

fn clean_reservation(
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

fn read_receipt(
    runtime: &Runtime,
    link: &MechanicalAnimationGlbV2LinkRecord,
) -> Result<MechanicalAnimationGlbV2ReceiptRecord, RuntimeError> {
    let value = read_json(runtime, &link.receipt_object_sha256, RECEIPT_KIND)?;
    let receipt: MechanicalAnimationGlbV2ReceiptRecord = serde_json::from_value(value)
        .map_err(|error| invalid(format!("durable V2 receipt is malformed: {error}")))?;
    if receipt.canonical_sha256 != link.receipt_canonical_sha256
        || receipt.animation_glb_key_sha256 != link.animation_glb_key_sha256
        || receipt.animated_artifact_sha256 != link.animated_artifact_sha256
        || receipt.animated_artifact_readback_sha256 != link.animated_artifact_readback_sha256
        || receipt.project_id != link.project_id
        || receipt.appearance_candidate_id != link.appearance_candidate_id
        || receipt.clip_id != link.clip_id
    {
        return Err(invalid("durable V2 receipt differs from its Store link"));
    }
    Ok(receipt)
}

fn read_animated_object(runtime: &Runtime, hash: &str) -> Result<Vec<u8>, RuntimeError> {
    let object = runtime
        .store
        .get_object(hash)?
        .ok_or_else(|| invalid("animated GLB CAS object is unavailable"))?;
    if object.kind != GLB_KIND
        || object.mime != GLB_MIME
        || object.size_bytes == 0
        || object.size_bytes > MAX_GLB_BYTES as u64
    {
        return Err(invalid("animated GLB CAS object metadata differs"));
    }
    let bytes = runtime.cas_read_bounded(hash, MAX_GLB_BYTES as u64)?;
    if sha256_hex(&bytes) != hash {
        return Err(invalid("animated GLB CAS bytes differ from its hash"));
    }
    Ok(bytes)
}

fn validate_stored_pair(
    runtime: &Runtime,
    link: &MechanicalAnimationGlbV2LinkRecord,
    context: &ClipContext,
    key_sha256: &str,
) -> Result<(MechanicalAnimationGlbV2ReceiptRecord, Vec<u8>), RuntimeError> {
    if link.animation_glb_key_sha256 != key_sha256
        || link.project_id
            != context
                .request
                .get("project_id")
                .and_then(Value::as_str)
                .unwrap_or_default()
        || link.appearance_candidate_id
            != context
                .request
                .get("appearance_candidate_id")
                .and_then(Value::as_str)
                .unwrap_or_default()
        || link.clip_id
            != context
                .request
                .get("clip_id")
                .and_then(Value::as_str)
                .unwrap_or_default()
        || link.clip_object_sha256
            != context
                .request
                .get("clip_object_sha256")
                .and_then(Value::as_str)
                .unwrap_or_default()
        || link.clip_sha256
            != context
                .request
                .get("clip_sha256")
                .and_then(Value::as_str)
                .unwrap_or_default()
    {
        return Err(invalid("stored V2 GLB link is not bound to the request"));
    }
    let receipt = read_receipt(runtime, link)?;
    let animated_glb = read_animated_object(runtime, &link.animated_artifact_sha256)?;
    let inspection = strict_glb_inspection(&animated_glb)?;
    let readback = artifact_readback_v2_value(
        &link.animated_artifact_sha256,
        &link.appearance_candidate_id,
        &inspection,
        animated_glb.len() as u64,
    );
    if readback.get("canonical_sha256").and_then(Value::as_str)
        != Some(link.animated_artifact_readback_sha256.as_str())
    {
        return Err(invalid("stored animated ArtifactReadback digest differs"));
    }
    Ok((receipt, animated_glb))
}

pub(super) fn prepare(runtime: &Runtime, request_value: &Value) -> Result<Value, RuntimeError> {
    let (request, key_sha256, input_sha256) = parse_prepare(request_value)?;
    let object = request
        .as_object()
        .ok_or_else(|| invalid("animation GLB request is not an object"))?;
    let project_id = id(object, "project_id")?.to_owned();
    let appearance_candidate_id = id(object, "appearance_candidate_id")?.to_owned();
    let clip_id = id(object, "clip_id")?.to_owned();

    if let Some(existing) = runtime
        .store
        .get_mechanical_animation_glb_v2_link_for_clip(&appearance_candidate_id, &clip_id)?
    {
        if existing.animation_glb_key_sha256 != key_sha256
            || existing.project_id != project_id
            || existing.appearance_candidate_id != appearance_candidate_id
            || existing.clip_id != clip_id
            || existing.clip_object_sha256 != sha(object, "clip_object_sha256")?
            || existing.clip_sha256 != sha(object, "clip_sha256")?
        {
            return Err(invalid(
                "animation GLB key is bound to different frozen inputs",
            ));
        }
        // Existing-link replay must run the same read-only Get path, including
        // Clip@2/quality/lineage/readback and double materialization checks.
        let replay = get(
            runtime,
            &json!({
                "schema_version":GET_SCHEMA,
                "project_id":project_id,
                "appearance_candidate_id":appearance_candidate_id,
                "clip_id":clip_id
            }),
        )?;
        replay
            .as_object()
            .ok_or_else(|| invalid("replayed get result is not an object"))?;
        let mut replay = replay;
        replay["schema_version"] = Value::String(PREPARE_RESULT_SCHEMA.to_owned());
        replay["replayed"] = Value::Bool(true);
        replay["runtime_write_performed"] = Value::Bool(true);
        return Ok(replay);
    }

    let clip = clip_get(runtime, &project_id, &appearance_candidate_id, &clip_id)?;
    let clip_sha256 = sha(object, "clip_sha256")?;
    if clip.get("canonical_sha256").and_then(Value::as_str) != Some(clip_sha256)
        || clip.get("project_id").and_then(Value::as_str) != Some(project_id.as_str())
        || clip.get("appearance_candidate_id").and_then(Value::as_str)
            != Some(appearance_candidate_id.as_str())
        || clip.get("clip_id").and_then(Value::as_str) != Some(clip_id.as_str())
    {
        return Err(invalid("Clip@2 does not match the frozen prepare request"));
    }
    let context = load_context(runtime, &request, &clip)?;
    let source_sha256 = clip["appearance_artifact_sha256"]
        .as_str()
        .ok_or_else(|| invalid("appearance artifact hash is missing"))?;
    let animated_glb = materialize(
        &context.source_glb,
        source_sha256,
        &key_sha256,
        clip_sha256,
        &context.part_ids,
        &context.ticks,
        &context.frames,
    )?;
    let validation = validate_animated_glb(
        &context.source_glb,
        &animated_glb,
        source_sha256,
        &key_sha256,
        clip_sha256,
        &context.material_pack_id,
        &context.material_pack_manifest_sha256,
        &context.part_ids,
        &context.ticks,
        &context.frames,
    )?;
    let animated_artifact_sha256 = sha256_hex(&animated_glb);
    let inspection = strict_glb_inspection(&animated_glb)?;
    let readback = artifact_readback_v2_value(
        &animated_artifact_sha256,
        &appearance_candidate_id,
        &inspection,
        animated_glb.len() as u64,
    );
    let animated_artifact_readback_sha256 = readback
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .filter(|hash| is_sha256(hash))
        .ok_or_else(|| invalid("animated ArtifactReadback digest is unavailable"))?
        .to_owned();
    let receipt = receipt_value(
        &context,
        &key_sha256,
        &animated_artifact_sha256,
        &animated_artifact_readback_sha256,
        &validation,
    )?;
    let receipt_value = serde_json::to_value(&receipt)
        .map_err(|error| invalid(format!("receipt cannot be serialized: {error}")))?;
    let receipt_bytes =
        canonical_json_bytes(&receipt_value).map_err(|error| invalid(error.to_string()))?;
    if receipt_bytes.len() > MAX_JSON_BYTES {
        return Err(invalid(
            "MechanicalAnimationGlbReceipt@2 exceeds its JSON budget",
        ));
    }
    let receipt_object_sha256 = sha256_hex(&receipt_bytes);
    let link = link_value(
        &context,
        &key_sha256,
        &input_sha256,
        &animated_artifact_sha256,
        &animated_artifact_readback_sha256,
        &receipt_object_sha256,
        &receipt.canonical_sha256,
    )?;
    let reservation = runtime.store.begin_cas_reservation();
    let animated_object = match runtime.store.put_object_reserved(
        &reservation,
        &animated_glb,
        Some(&animated_artifact_sha256),
        GLB_MIME,
        GLB_KIND,
        &context.created_at,
    ) {
        Ok(object) => object,
        Err(error) => return Err(error.into()),
    };
    let receipt_object = match runtime.store.put_object_reserved(
        &reservation,
        &receipt_bytes,
        Some(&receipt_object_sha256),
        RECEIPT_MIME,
        RECEIPT_KIND,
        &context.created_at,
    ) {
        Ok(object) => object,
        Err(error) => {
            clean_reservation(runtime, &reservation, &animated_object, true);
            return Err(error.into());
        }
    };
    let stored = match runtime.store.record_mechanical_animation_glb_v2_link(&link) {
        Ok(link) => link,
        Err(error) => {
            clean_reservation(runtime, &reservation, &animated_object, true);
            clean_reservation(runtime, &reservation, &receipt_object, true);
            return Err(error.into());
        }
    };
    clean_reservation(runtime, &reservation, &animated_object, false);
    clean_reservation(runtime, &reservation, &receipt_object, false);
    result_value(
        PREPARE_RESULT_SCHEMA,
        &stored,
        &receipt,
        animated_glb.len() as u64,
        false,
        true,
    )
}

pub(super) fn get(runtime: &Runtime, request_value: &Value) -> Result<Value, RuntimeError> {
    let (project_id, appearance_candidate_id, clip_id) = parse_get(request_value)?;
    let clip_link = runtime
        .store
        .get_mechanical_animation_clip_v2_link(&appearance_candidate_id, &clip_id)?
        .ok_or_else(|| invalid("durable MechanicalAnimationClip@2 is unavailable"))?;
    if clip_link.project_id != project_id {
        return Err(invalid(
            "durable MechanicalAnimationClip@2 belongs to another project",
        ));
    }
    let clip = clip_get(runtime, &project_id, &appearance_candidate_id, &clip_id)?;
    let request = json!({
        "schema_version":PREPARE_SCHEMA,
        "project_id":clip_link.project_id,
        "appearance_candidate_id":clip_link.appearance_candidate_id,
        "appearance_candidate_state_sha256":clip_link.appearance_candidate_state_sha256,
        "clip_id":clip_link.clip_id,
        "clip_object_sha256":clip_link.clip_object_sha256,
        "clip_sha256":clip_link.clip_sha256,
        "materialization_policy":POLICY,
        "input_sha256":clip_link.request_sha256,
        "idempotency_key":"replay"
    });
    let context = load_context(runtime, &request, &clip)?;
    let key_sha256 = canonical_json_hash(&key_preimage(
        request
            .as_object()
            .ok_or_else(|| invalid("derived animation request is not an object"))?,
    )?);
    let link = runtime
        .store
        .get_mechanical_animation_glb_v2_link(&key_sha256)?
        .ok_or_else(|| invalid("durable MechanicalAnimationGlb@2 is unavailable"))?;
    if link.project_id != project_id
        || link.appearance_candidate_id != appearance_candidate_id
        || link.clip_id != clip_id
    {
        return Err(invalid("durable MechanicalAnimationGlb@2 identity differs"));
    }
    let (receipt, stored_glb) = validate_stored_pair(runtime, &link, &context, &key_sha256)?;
    let source_sha256 = clip["appearance_artifact_sha256"]
        .as_str()
        .ok_or_else(|| invalid("appearance artifact hash is missing"))?;
    let clip_sha256 = clip
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("Clip@2 canonical hash is missing"))?;
    let materialized_glb = materialize(
        &context.source_glb,
        source_sha256,
        &key_sha256,
        clip_sha256,
        &context.part_ids,
        &context.ticks,
        &context.frames,
    )?;
    let validation = validate_animated_glb(
        &context.source_glb,
        &materialized_glb,
        source_sha256,
        &key_sha256,
        clip_sha256,
        &context.material_pack_id,
        &context.material_pack_manifest_sha256,
        &context.part_ids,
        &context.ticks,
        &context.frames,
    )?;
    if materialized_glb != stored_glb
        || sha256_hex(&materialized_glb) != link.animated_artifact_sha256
        || receipt.animation_validation_sha256 != canonical_json_hash(&validation)
    {
        return Err(invalid(
            "durable animated GLB is not byte-identical to fresh materialization",
        ));
    }
    result_value(
        GET_RESULT_SCHEMA,
        &link,
        &receipt,
        stored_glb.len() as u64,
        false,
        false,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(ch: char) -> String {
        std::iter::repeat_n(ch, 64).collect()
    }

    fn source_glb() -> (Vec<u8>, String, String) {
        let manifest = hash('m');
        let root = json!({
            "asset":{"version":"2.0"},
            "scene":0,
            "scenes":[{"nodes":[0,1]}],
            "nodes":[{"name":"part-a","mesh":0},{"name":"part-b","mesh":0}],
            "meshes":[{"primitives":[{"attributes":{}}]}],
            "materials":[{"name":"paint"}],
            "textures":[{"sampler":0,"source":0}],
            "images":[{"bufferView":0,"mimeType":"image/png"}],
            "samplers":[{"magFilter":9729,"minFilter":9729}],
            "buffers":[{"byteLength":8}],
            "bufferViews":[{"buffer":0,"byteOffset":0,"byteLength":8}],
            "accessors":[{"bufferView":0,"componentType":5126,"count":2,"type":"SCALAR"}],
            "extras":{"forgecad":{"material_pack_id":"pack-1","material_pack_manifest_sha256":manifest}}
        });
        let bytes = encode_glb(&root, &[1, 2, 3, 4, 5, 6, 7, 8]).expect("fixture GLB");
        (bytes, manifest, hash('s'))
    }

    fn frames() -> Vec<Frame> {
        vec![
            Frame {
                frame_sha256: hash('a'),
                deltas: BTreeMap::from([
                    (
                        "part-a".to_owned(),
                        RigidTransform {
                            translation: [0.0, 0.0, 0.0],
                            rotation: [0.0, 0.0, 0.0, 1.0],
                        },
                    ),
                    (
                        "part-b".to_owned(),
                        RigidTransform {
                            translation: [0.0, 1.0, 0.0],
                            rotation: [0.0, 0.0, 0.0, 1.0],
                        },
                    ),
                ]),
            },
            Frame {
                frame_sha256: hash('b'),
                deltas: BTreeMap::from([
                    (
                        "part-a".to_owned(),
                        RigidTransform {
                            translation: [1.0, 0.0, 0.0],
                            rotation: [0.0, 0.0, 0.0, 1.0],
                        },
                    ),
                    (
                        "part-b".to_owned(),
                        RigidTransform {
                            translation: [0.0, 1.0, 0.0],
                            rotation: [0.0, 0.0, 0.0, 1.0],
                        },
                    ),
                ]),
            },
        ]
    }

    fn request() -> Value {
        let mut value = json!({
            "schema_version":PREPARE_SCHEMA,
            "project_id":"project-1",
            "appearance_candidate_id":"appearance-1",
            "appearance_candidate_state_sha256":hash('c'),
            "clip_id":"clip-1",
            "clip_object_sha256":hash('d'),
            "clip_sha256":hash('e'),
            "materialization_policy":POLICY,
            "input_sha256":"",
            "idempotency_key":"request-1"
        });
        let mut preimage = value.clone();
        preimage
            .as_object_mut()
            .expect("request object")
            .remove("input_sha256");
        preimage
            .as_object_mut()
            .expect("request object")
            .remove("idempotency_key");
        value["input_sha256"] = Value::String(canonical_json_hash(&preimage));
        value
    }

    #[test]
    fn closed_request_derives_non_cas_key_and_is_idempotent() {
        let request = request();
        let (first, key, input) = parse_prepare(&request).expect("closed request");
        let (second, same_key, same_input) = parse_prepare(&request).expect("replay request");
        assert_eq!(first, second);
        assert_eq!(key, same_key);
        assert_eq!(input, same_input);
        let expected = canonical_json_hash(&key_preimage(request.as_object().unwrap()).unwrap());
        assert_eq!(key, expected);
        let mut changed = request.clone();
        changed["clip_sha256"] = Value::String(hash('f'));
        let mut preimage = changed.clone();
        preimage.as_object_mut().unwrap().remove("input_sha256");
        preimage.as_object_mut().unwrap().remove("idempotency_key");
        changed["input_sha256"] = Value::String(canonical_json_hash(&preimage));
        let (_, changed_key, _) = parse_prepare(&changed).expect("changed closed request");
        assert_ne!(key, changed_key);
    }

    #[test]
    fn materialization_is_byte_exact_and_preserves_static_material_projection_and_bin_prefix() {
        let (source, manifest, _) = source_glb();
        let source_hash = sha256_hex(&source);
        let part_ids = vec!["part-a".to_owned(), "part-b".to_owned()];
        let ticks = vec![0, 10];
        let frames = frames();
        let first = materialize(
            &source,
            &source_hash,
            &hash('k'),
            &hash('e'),
            &part_ids,
            &ticks,
            &frames,
        )
        .expect("materialize");
        let second = materialize(
            &source,
            &source_hash,
            &hash('k'),
            &hash('e'),
            &part_ids,
            &ticks,
            &frames,
        )
        .expect("repeat materialize");
        assert_eq!(first, second);
        let (_, source_bin) = parse_glb(&source).expect("source parse");
        let (_, animated_bin) = parse_glb(&first).expect("animated parse");
        assert_eq!(&animated_bin[..source_bin.len()], source_bin.as_slice());
        let validation = validate_animated_glb(
            &source,
            &first,
            &source_hash,
            &hash('k'),
            &hash('e'),
            "pack-1",
            &manifest,
            &part_ids,
            &ticks,
            &frames,
        )
        .expect("structural readback");
        assert_eq!(validation["source_static_projection_exact"], true);
        assert_eq!(validation["binary_prefix_exact"], true);
        assert_eq!(validation["appearance_material_projection_exact"], true);
        assert_eq!(validation["no_skinning"], true);
        assert_eq!(validation["no_morph_targets"], true);
    }

    #[test]
    fn materializer_rejects_existing_animation_skin_morph_and_tick_drift() {
        let (source, manifest, _) = source_glb();
        let parts = vec!["part-a".to_owned(), "part-b".to_owned()];
        let ticks = vec![0, 10];
        let frames = frames();
        let mut root = parse_glb(&source).expect("source parse").0;
        for field in ["animations", "skins"] {
            let mut with_forbidden = root.clone();
            with_forbidden[field] = json!([]);
            let bytes =
                encode_glb(&with_forbidden, &[1, 2, 3, 4, 5, 6, 7, 8]).expect("forbidden GLB");
            assert!(materialize(
                &bytes,
                &sha256_hex(&bytes),
                &hash('k'),
                &hash('e'),
                &parts,
                &ticks,
                &frames
            )
            .is_err());
        }
        root["meshes"][0]["primitives"][0]["targets"] = json!([{"POSITION":0}]);
        let morph = encode_glb(&root, &[1, 2, 3, 4, 5, 6, 7, 8]).expect("morph GLB");
        assert!(materialize(
            &morph,
            &sha256_hex(&morph),
            &hash('k'),
            &hash('e'),
            &parts,
            &ticks,
            &frames
        )
        .is_err());
        let (clean, _, _) = source_glb();
        let animated = materialize(
            &clean,
            &sha256_hex(&clean),
            &hash('k'),
            &hash('e'),
            &parts,
            &ticks,
            &frames,
        )
        .expect("materialize clean GLB");
        assert!(validate_animated_glb(
            &clean,
            &animated,
            &sha256_hex(&clean),
            &hash('k'),
            &hash('e'),
            "pack-1",
            &manifest,
            &parts,
            &[0, 20],
            &frames,
        )
        .is_err());
    }
}
