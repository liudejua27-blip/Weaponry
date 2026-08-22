//! Appearance-aware, candidate-bound MechanicalAnimationClip@2.
//!
//! This is intentionally a new lane beside the historical V1 rigid clip.  It
//! replays the exact material-surface source cohort through the fixed Geometry
//! Worker before reserving one canonical JSON clip object.  The clip is still
//! structural evidence only: it does not advance a production stage, confirm
//! a candidate, create a version or export to a game engine.

use super::{
    artifact_readback_v2_value, canonical_json_bytes, canonical_json_hash,
    compile_geometry_with_runtime_worker, exact_object, is_opaque_id, is_sha256, sha256_hex,
    strict_glb_inspection, validate_glb_material_pack_identity, validate_worker_metadata, Runtime,
    RuntimeError, MAX_DERIVED_JSON_BYTES, MAX_GEOMETRY_ARTIFACT_BYTES,
};
use forgecad_contracts::{
    MechanicalAnimationClipV2LinkRecord, MechanicalAnimationClipV2PrepareRequest,
    MechanicalAnimationClipV2Record,
};
use forgecad_store::CasObject;
use serde_json::{json, Map, Value};

const PREPARE_SCHEMA: &str = "MechanicalAnimationClipPrepareRequest@2";
const GET_SCHEMA: &str = "MechanicalAnimationClipGetRequest@2";
const PREVIEW_SCHEMA: &str = "MechanicalAnimationClipPreviewRequest@2";
const PREPARE_RESULT_SCHEMA: &str = "MechanicalAnimationClipPrepareResult@2";
const GET_RESULT_SCHEMA: &str = "MechanicalAnimationClipGetResult@2";
const PREVIEW_RESULT_SCHEMA: &str = "MechanicalAnimationClipPreview@2";
const CLIP_KIND: &str = "mechanical-animation-clip-v2";
const CLIP_MIME: &str = "application/json";
const MAX_CLIP_BYTES: u64 = 1024 * 1024;
const MAX_SAMPLES: usize = 16;
const REPLAY_POLICY: &str = "geometry-plus-appearance-double-worker-replay@1";
const MATERIALIZATION_STATUS: &str = "runtime-owned-immutable-cas-appearance-aware-clip";
const QUALITY_STATUS: &str = "structural_only";
const VISUAL_STATUS: &str = "NOT_PROVEN";
const COMMERCIAL_STATUS: &str = "NOT_PROVEN";
const HUMAN_STATUS: &str = "NOT_RUN";
const ENGINE_STATUS: &str = "NOT_RUN";

const PREPARE_FIELDS: &[&str] = &[
    "schema_version",
    "clip_id",
    "project_id",
    "appearance_candidate_id",
    "appearance_candidate_state_sha256",
    "appearance_artifact_id",
    "appearance_artifact_sha256",
    "appearance_artifact_readback_sha256",
    "appearance_artifact_readback_object_sha256",
    "source_geometry_candidate_id",
    "source_geometry_candidate_state_sha256",
    "source_geometry_artifact_id",
    "source_geometry_artifact_sha256",
    "source_geometry_candidate_evidence_sha256",
    "material_surface_quality_id",
    "material_surface_quality_report_object_sha256",
    "material_surface_quality_canonical_sha256",
    "appearance_source_lineage_sidecar_object_sha256",
    "appearance_source_lineage_canonical_sha256",
    "appearance_program_object_sha256",
    "appearance_program_sha256",
    "geometry_program_object_sha256",
    "geometry_program_sha256",
    "geometry_preservation_projection_sha256",
    "operator_catalog_sha256",
    "readback_config_sha256",
    "rest_frame",
    "pose_action",
    "sampling_policy",
    "replay_policy",
    "input_sha256",
    "idempotency_key",
];

fn invalid(message: impl Into<String>) -> RuntimeError {
    RuntimeError::InvalidInput(format!(
        "MECHANICAL_ANIMATION_CLIP_V2_INVALID: {}",
        message.into()
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

fn verify_canonical(value: &Value, field: &str) -> Result<(), RuntimeError> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid("canonical payload is not an object"))?;
    let expected = object
        .get(field)
        .and_then(Value::as_str)
        .filter(|hash| is_sha256(hash))
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

fn parse_prepare(
    value: &Value,
) -> Result<(MechanicalAnimationClipV2PrepareRequest, String), RuntimeError> {
    let object = exact_object(value, PREPARE_FIELDS, PREPARE_SCHEMA)?;
    if text(object, "schema_version")? != PREPARE_SCHEMA {
        return Err(invalid("prepare schema differs"));
    }
    let request: MechanicalAnimationClipV2PrepareRequest = serde_json::from_value(value.clone())
        .map_err(|error| invalid(format!("prepare request is malformed: {error}")))?;
    for field in [
        "clip_id",
        "project_id",
        "appearance_candidate_id",
        "appearance_artifact_id",
        "source_geometry_candidate_id",
        "source_geometry_artifact_id",
        "material_surface_quality_id",
        "idempotency_key",
    ] {
        id(object, field)?;
    }
    for field in [
        "appearance_candidate_state_sha256",
        "appearance_artifact_sha256",
        "appearance_artifact_readback_sha256",
        "appearance_artifact_readback_object_sha256",
        "source_geometry_candidate_state_sha256",
        "source_geometry_artifact_sha256",
        "source_geometry_candidate_evidence_sha256",
        "material_surface_quality_report_object_sha256",
        "material_surface_quality_canonical_sha256",
        "appearance_source_lineage_sidecar_object_sha256",
        "appearance_source_lineage_canonical_sha256",
        "appearance_program_object_sha256",
        "appearance_program_sha256",
        "geometry_program_object_sha256",
        "geometry_program_sha256",
        "geometry_preservation_projection_sha256",
        "operator_catalog_sha256",
        "readback_config_sha256",
        "input_sha256",
    ] {
        sha(object, field)?;
    }
    if request.replay_policy != REPLAY_POLICY {
        return Err(invalid("replay_policy differs"));
    }
    if request.appearance_candidate_id == request.source_geometry_candidate_id {
        return Err(invalid("appearance and source candidates must be distinct"));
    }
    verify_canonical(&request.rest_frame, "canonical_sha256")?;
    verify_canonical(&request.pose_action, "canonical_sha256")?;
    let sampling = exact_object(
        &request.sampling_policy,
        &[
            "schema_version",
            "timebase_hz",
            "interpolation",
            "unkeyed",
            "sample_time_ticks",
            "max_samples",
            "frame_preview_batch_size",
        ],
        "MechanicalAnimationSamplingPolicy@1",
    )?;
    if text(sampling, "schema_version")? != "MechanicalAnimationSamplingPolicy@1"
        || sampling.get("timebase_hz").and_then(Value::as_u64) != Some(1000)
        || text(sampling, "interpolation")? != "scalar-linear-integer-ticks-clamped"
        || text(sampling, "unkeyed")? != "rest"
        || sampling.get("max_samples").and_then(Value::as_u64) != Some(MAX_SAMPLES as u64)
        || sampling
            .get("frame_preview_batch_size")
            .and_then(Value::as_u64)
            != Some(1)
    {
        return Err(invalid("sampling policy differs"));
    }
    let ticks = sampling
        .get("sample_time_ticks")
        .and_then(Value::as_array)
        .filter(|values| !values.is_empty() && values.len() <= MAX_SAMPLES)
        .ok_or_else(|| invalid("sample schedule must contain 1..16 ticks"))?;
    let mut prior = None;
    for tick in ticks {
        let tick = tick
            .as_u64()
            .filter(|value| *value <= 1_000_000)
            .ok_or_else(|| invalid("sample schedule contains an invalid tick"))?;
        if prior.is_some_and(|previous| tick <= previous) {
            return Err(invalid("sample schedule must be strictly increasing"));
        }
        prior = Some(tick);
    }
    let mut preimage = Value::Object(object.clone());
    let map = preimage
        .as_object_mut()
        .expect("prepare request is an object");
    map.remove("input_sha256");
    map.remove("idempotency_key");
    let input_sha256 = canonical_json_hash(&preimage);
    if request.input_sha256 != input_sha256 {
        return Err(invalid("input_sha256 does not bind the closed request"));
    }
    Ok((request, input_sha256))
}

fn rest_frame_draft(value: &Value) -> Result<Value, RuntimeError> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid("rest_frame must be an object"))?;
    if object.get("schema_version").and_then(Value::as_str) != Some("MechanicalRestFrame@1") {
        return Err(invalid("rest_frame schema differs"));
    }
    Ok(json!({
        "schema_version":"MechanicalRestFrameDraft@1",
        "rest_frame_id":object.get("rest_frame_id"),
        "coordinate_system":object.get("coordinate_system"),
        "transform_convention":object.get("transform_convention"),
        "root_link_id":object.get("root_link_id"),
        "links":object.get("links"),
        "parent_map":object.get("parent_map")
    }))
}

fn pose_action_draft(value: &Value) -> Result<Value, RuntimeError> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid("pose_action must be an object"))?;
    if object.get("schema_version").and_then(Value::as_str) != Some("MechanicalPoseAction@1") {
        return Err(invalid("pose_action schema differs"));
    }
    Ok(json!({
        "schema_version":"MechanicalPoseActionDraft@1",
        "action_id":object.get("action_id"),
        "timebase_hz":object.get("timebase_hz"),
        "duration_ticks":object.get("duration_ticks"),
        "interpolation":object.get("interpolation"),
        "extrapolation":object.get("extrapolation"),
        "unkeyed_policy":object.get("unkeyed_policy"),
        "channels":object.get("channels")
    }))
}

struct ReplayCohort {
    quality: forgecad_contracts::CandidateMaterialSurfaceQualityRecord,
    appearance_program: Value,
    worker_cohort: String,
}

fn read_json(runtime: &Runtime, hash: &str) -> Result<Value, RuntimeError> {
    let bytes = runtime.cas_read_bounded(hash, MAX_DERIVED_JSON_BYTES)?;
    serde_json::from_slice(&bytes).map_err(|error| invalid(format!("CAS JSON is invalid: {error}")))
}

fn read_verified_json_object(
    runtime: &Runtime,
    hash: &str,
    kind: &str,
    expected: Option<&Value>,
) -> Result<Value, RuntimeError> {
    let object = runtime
        .store
        .get_object(hash)?
        .ok_or_else(|| invalid(format!("required {kind} object is unavailable")))?;
    if object.mime != "application/json"
        || object.kind != kind
        || object.size_bytes == 0
        || object.size_bytes > MAX_DERIVED_JSON_BYTES
    {
        return Err(invalid(format!("{kind} object metadata is invalid")));
    }
    let bytes = runtime.cas_read_bounded(hash, MAX_DERIVED_JSON_BYTES)?;
    if sha256_hex(&bytes) != hash {
        return Err(invalid(format!(
            "{kind} object hash does not match CAS bytes"
        )));
    }
    let value: Value = serde_json::from_slice(&bytes)
        .map_err(|error| invalid(format!("{kind} object is not valid JSON: {error}")))?;
    if canonical_json_bytes(&value).map_err(|error| invalid(error.to_string()))? != bytes {
        return Err(invalid(format!("{kind} object is not canonical JSON")));
    }
    if let Some(expected) = expected {
        if &value != expected {
            return Err(invalid(format!("{kind} object binding differs")));
        }
    }
    Ok(value)
}

fn validate_candidate_bindings(
    runtime: &Runtime,
    request: &MechanicalAnimationClipV2PrepareRequest,
) -> Result<ReplayCohort, RuntimeError> {
    let appearance_candidate = runtime
        .candidate(&request.appearance_candidate_id)?
        .ok_or_else(|| invalid("appearance candidate is unavailable"))?;
    if appearance_candidate.project_id != request.project_id
        || appearance_candidate.canonical_sha256 != request.appearance_candidate_state_sha256
        || appearance_candidate.prepared_object_id.as_deref()
            != Some(request.appearance_artifact_id.as_str())
        || appearance_candidate.prepared_object_sha256.as_deref()
            != Some(request.appearance_artifact_sha256.as_str())
    {
        return Err(invalid(
            "appearance candidate/state/artifact binding differs",
        ));
    }
    let source_candidate = runtime
        .candidate(&request.source_geometry_candidate_id)?
        .ok_or_else(|| invalid("source geometry candidate is unavailable"))?;
    if source_candidate.project_id != request.project_id
        || source_candidate.canonical_sha256 != request.source_geometry_candidate_state_sha256
        || source_candidate.prepared_object_id.as_deref()
            != Some(request.source_geometry_artifact_id.as_str())
        || source_candidate.prepared_object_sha256.as_deref()
            != Some(request.source_geometry_artifact_sha256.as_str())
    {
        return Err(invalid(
            "source geometry candidate/state/artifact binding differs",
        ));
    }

    let quality_value = runtime.candidate_material_surface_quality_get(json!({
        "schema_version":"CandidateMaterialSurfaceQualityGetRequest@1",
        "material_surface_quality_id":request.material_surface_quality_id,
        "project_id":request.project_id,
        "source_candidate_id":request.source_geometry_candidate_id,
        "output_candidate_id":request.appearance_candidate_id
    }))?;
    let quality: forgecad_contracts::CandidateMaterialSurfaceQualityRecord =
        serde_json::from_value(
            quality_value
                .get("material_surface_quality")
                .cloned()
                .ok_or_else(|| invalid("material-surface quality result is missing"))?,
        )
        .map_err(|error| invalid(format!("material-surface quality is malformed: {error}")))?;
    let quality_json = serde_json::to_value(&quality).map_err(|error| {
        invalid(format!(
            "material-surface quality cannot be serialized: {error}"
        ))
    })?;
    let quality_report_sha256 =
        sha256_hex(&canonical_json_bytes(&quality_json).map_err(|error| {
            invalid(format!(
                "material-surface quality report is not canonical: {error}"
            ))
        })?);
    read_verified_json_object(
        runtime,
        &request.material_surface_quality_report_object_sha256,
        "candidate-material-surface-quality-report",
        Some(&quality_json),
    )?;
    if quality.project_id != request.project_id
        || quality.material_surface_quality_id != request.material_surface_quality_id
        || quality.source_candidate_id != request.source_geometry_candidate_id
        || quality.source_candidate_state_sha256 != request.source_geometry_candidate_state_sha256
        || quality.source_artifact_id != request.source_geometry_artifact_id
        || quality.source_artifact_sha256 != request.source_geometry_artifact_sha256
        || quality.output_candidate_id != request.appearance_candidate_id
        || quality.output_candidate_state_sha256 != request.appearance_candidate_state_sha256
        || quality.output_artifact_id != request.appearance_artifact_id
        || quality.output_artifact_sha256 != request.appearance_artifact_sha256
        || quality.output_artifact_readback_sha256 != request.appearance_artifact_readback_sha256
        || quality.output_artifact_readback_object_sha256
            != request.appearance_artifact_readback_object_sha256
        || quality.source_geometry_candidate_evidence_sha256
            != request.source_geometry_candidate_evidence_sha256
        || quality_report_sha256 != request.material_surface_quality_report_object_sha256
        || quality.canonical_sha256 != request.material_surface_quality_canonical_sha256
        || quality.appearance_source_lineage_sidecar_object_sha256
            != request.appearance_source_lineage_sidecar_object_sha256
        || quality.appearance_source_lineage_canonical_sha256
            != request.appearance_source_lineage_canonical_sha256
        || quality.appearance_program_object_sha256 != request.appearance_program_object_sha256
        || quality.appearance_program_sha256 != request.appearance_program_sha256
        || quality.source_geometry_program_sha256 != request.geometry_program_sha256
        || quality.output_geometry_program_sha256 != request.geometry_program_sha256
        || quality.geometry_preservation_projection_sha256
            != request.geometry_preservation_projection_sha256
        || !quality.hard_gate_passed
        || quality.validator_status != "passed"
    {
        return Err(invalid("material-surface quality binding or gate differs"));
    }

    let source_evidence = runtime
        .store
        .get_geometry_candidate_evidence(&request.source_geometry_candidate_id)?
        .ok_or_else(|| invalid("source geometry evidence is unavailable"))?;
    if source_evidence.project_id != request.project_id
        || source_evidence.artifact_object_sha256 != request.source_geometry_artifact_sha256
        || source_evidence.canonical_sha256 != request.source_geometry_candidate_evidence_sha256
        || source_evidence.geometry_program_object_sha256 != request.geometry_program_object_sha256
        || source_evidence.geometry_program_sha256 != request.geometry_program_sha256
        || source_evidence.operator_catalog_sha256 != request.operator_catalog_sha256
        || source_evidence.readback_config_sha256 != request.readback_config_sha256
    {
        return Err(invalid("source geometry evidence binding differs"));
    }

    let mut lineage_request = json!({
        "schema_version":"AppearanceSourceLineageGetRequest@1",
        "project_id":request.project_id,
        "candidate_id":request.appearance_candidate_id,
        "appearance_program_sha256":request.appearance_program_sha256,
        "canonical_sha256":""
    });
    set_canonical(&mut lineage_request, "canonical_sha256")?;
    let lineage_value = runtime.appearance_source_lineage_get(&lineage_request)?;
    let lineage: forgecad_contracts::AppearanceSourceLineageLinkRecord = serde_json::from_value(
        lineage_value
            .get("durable_link")
            .cloned()
            .ok_or_else(|| invalid("AppearanceSourceLineage durable link is missing"))?,
    )
    .map_err(|error| {
        invalid(format!(
            "AppearanceSourceLineage link is malformed: {error}"
        ))
    })?;
    if lineage.project_id != request.project_id
        || lineage.candidate_id != request.appearance_candidate_id
        || lineage.candidate_state_sha256 != request.appearance_candidate_state_sha256
        || lineage.sidecar_object_sha256 != request.appearance_source_lineage_sidecar_object_sha256
        || lineage.canonical_sha256 != request.appearance_source_lineage_canonical_sha256
        || lineage.appearance_program_object_sha256 != request.appearance_program_object_sha256
        || lineage.appearance_program_sha256 != request.appearance_program_sha256
        || lineage.geometry_program_object_sha256 != request.geometry_program_object_sha256
        || lineage.geometry_program_sha256 != request.geometry_program_sha256
        || lineage.lod_candidate_ids.first().map(String::as_str)
            != Some(request.appearance_candidate_id.as_str())
        || lineage.lod_artifact_sha256s.first().map(String::as_str)
            != Some(request.appearance_artifact_sha256.as_str())
        || lineage
            .lod_artifact_readback_sha256s
            .first()
            .map(String::as_str)
            != Some(request.appearance_artifact_readback_sha256.as_str())
        || lineage
            .lod_artifact_readback_object_sha256s
            .first()
            .map(String::as_str)
            != Some(request.appearance_artifact_readback_object_sha256.as_str())
    {
        return Err(invalid("AppearanceSourceLineage binding differs"));
    }

    let appearance_readback = runtime.artifact_readback_bounded(
        &request.appearance_artifact_sha256,
        &request.appearance_candidate_id,
        MAX_GEOMETRY_ARTIFACT_BYTES,
    )?;
    if appearance_readback
        .get("canonical_sha256")
        .and_then(Value::as_str)
        != Some(request.appearance_artifact_readback_sha256.as_str())
        || appearance_readback
            .get("object_sha256")
            .and_then(Value::as_str)
            != Some(request.appearance_artifact_sha256.as_str())
    {
        return Err(invalid("appearance ArtifactReadback binding differs"));
    }
    read_verified_json_object(
        runtime,
        &request.appearance_artifact_readback_object_sha256,
        "appearance-v2-artifact-readback",
        Some(&appearance_readback),
    )?;
    let source_readback = runtime.artifact_readback_bounded(
        &request.source_geometry_artifact_sha256,
        &request.source_geometry_candidate_id,
        MAX_GEOMETRY_ARTIFACT_BYTES,
    )?;
    if source_readback
        .get("canonical_sha256")
        .and_then(Value::as_str)
        != Some(quality.source_artifact_readback_sha256.as_str())
    {
        return Err(invalid("source geometry ArtifactReadback binding differs"));
    }
    read_verified_json_object(
        runtime,
        &source_evidence.artifact_readback_object_sha256,
        "geometry-artifact-readback-v2",
        Some(&source_readback),
    )?;

    let geometry_bytes = runtime.cas_read_bounded(
        &request.geometry_program_object_sha256,
        MAX_DERIVED_JSON_BYTES,
    )?;
    let mut geometry_program: Value = serde_json::from_slice(&geometry_bytes)
        .map_err(|error| invalid(format!("GeometryProgram CAS is invalid: {error}")))?;
    let geometry_object = geometry_program
        .as_object_mut()
        .ok_or_else(|| invalid("GeometryProgram CAS is not an object"))?;
    if geometry_object.contains_key("canonical_sha256") {
        return Err(invalid(
            "GeometryProgram source unexpectedly contains canonical_sha256",
        ));
    }
    geometry_object.insert(
        "canonical_sha256".to_owned(),
        Value::String(request.geometry_program_sha256.clone()),
    );
    if geometry_object.get("project_id").and_then(Value::as_str)
        != Some(request.project_id.as_str())
        || geometry_object
            .get("canonical_sha256")
            .and_then(Value::as_str)
            != Some(request.geometry_program_sha256.as_str())
    {
        return Err(invalid("GeometryProgram project/hash binding differs"));
    }

    let appearance_program = read_json(runtime, &request.appearance_program_object_sha256)?;
    if appearance_program.get("project_id").and_then(Value::as_str)
        != Some(request.project_id.as_str())
        || appearance_program
            .get("canonical_sha256")
            .and_then(Value::as_str)
            != Some(request.appearance_program_sha256.as_str())
        || appearance_program
            .get("geometry_program_sha256")
            .and_then(Value::as_str)
            != Some(request.geometry_program_sha256.as_str())
    {
        return Err(invalid(
            "AppearanceProgram project/geometry/hash binding differs",
        ));
    }

    evaluate_pose_sequence(runtime, request, &quality)?;
    let worker_first =
        compile_geometry_with_runtime_worker(&geometry_program, Some(&appearance_program))
            .map_err(|error| {
                invalid(format!("appearance Geometry Worker replay failed: {error}"))
            })?;
    let worker_repeat =
        compile_geometry_with_runtime_worker(&geometry_program, Some(&appearance_program))
            .map_err(|error| {
                invalid(format!(
                    "repeat appearance Geometry Worker replay failed: {error}"
                ))
            })?;
    let appearance_glb = runtime.cas_read_bounded(
        &request.appearance_artifact_sha256,
        MAX_GEOMETRY_ARTIFACT_BYTES,
    )?;
    if worker_first.glb != appearance_glb
        || worker_repeat.glb != appearance_glb
        || worker_first.glb != worker_repeat.glb
        || worker_first.program_sha256 != request.geometry_program_sha256
        || worker_repeat.program_sha256 != request.geometry_program_sha256
    {
        return Err(invalid(
            "appearance Geometry Worker replay is not byte-exact with the appearance artifact",
        ));
    }
    let inspection = strict_glb_inspection(&appearance_glb)?;
    if !inspection.hard_gate_passed
        || inspection.program_sha256 != request.geometry_program_sha256
        || inspection.operator_catalog_sha256.as_deref()
            != Some(request.operator_catalog_sha256.as_str())
        || inspection.readback_config_sha256 != request.readback_config_sha256
    {
        return Err(invalid("appearance strict GLB readback binding differs"));
    }
    validate_worker_metadata(&worker_first, &inspection)
        .map_err(|error| invalid(format!("appearance Worker metadata differs: {error}")))?;
    validate_worker_metadata(&worker_repeat, &inspection)
        .map_err(|error| invalid(format!("repeat Worker metadata differs: {error}")))?;
    let pack_manifest_sha256 = quality.material_pack_manifest_sha256.clone();
    validate_glb_material_pack_identity(
        &appearance_glb,
        &quality.material_pack_id,
        &pack_manifest_sha256,
    )?;
    let worker_cohort = worker_first
        .build_cohort_sha256
        .clone()
        .filter(|value| is_sha256(value))
        .ok_or_else(|| invalid("appearance Worker build cohort is unavailable"))?;
    if worker_repeat.build_cohort_sha256.as_deref() != Some(worker_cohort.as_str())
        || lineage.source_replay_worker_cohort_sha256 != worker_cohort
    {
        return Err(invalid("appearance Worker cohorts differ"));
    }

    Ok(ReplayCohort {
        quality,
        appearance_program,
        worker_cohort,
    })
}

fn evaluate_pose_sequence(
    runtime: &Runtime,
    request: &MechanicalAnimationClipV2PrepareRequest,
    quality: &forgecad_contracts::CandidateMaterialSurfaceQualityRecord,
) -> Result<Value, RuntimeError> {
    let rest = rest_frame_draft(&request.rest_frame)?;
    let action = pose_action_draft(&request.pose_action)?;
    let ticks = request
        .sampling_policy
        .get("sample_time_ticks")
        .cloned()
        .ok_or_else(|| invalid("sampling policy omitted sample_time_ticks"))?;
    let mut sequence = json!({
        "schema_version":"MechanicalPoseSequencePreviewRequest@1",
        "project_id":request.project_id,
        "artifact_id":request.source_geometry_artifact_sha256,
        "candidate_id":request.source_geometry_candidate_id,
        "artifact_readback_sha256":quality.source_artifact_readback_sha256,
        "program_sha256":request.geometry_program_sha256,
        "operator_catalog_sha256":request.operator_catalog_sha256,
        "readback_config_sha256":request.readback_config_sha256,
        "rest_frame_draft":rest,
        "pose_action_draft":action,
        "sample_time_ticks":ticks,
        "input_sha256":""
    });
    let mut preimage = sequence.clone();
    preimage
        .as_object_mut()
        .expect("pose sequence is an object")
        .remove("input_sha256");
    sequence["input_sha256"] = Value::String(canonical_json_hash(&preimage));
    let result = runtime.mechanical_pose_evaluate(&sequence)?;
    if result.get("schema_version").and_then(Value::as_str)
        != Some("MechanicalPoseSequencePreview@1")
        || result.get("validator_status").and_then(Value::as_str) != Some("passed")
        || result.get("quality_status").and_then(Value::as_str) != Some(QUALITY_STATUS)
    {
        return Err(invalid(format!(
            "mechanical pose sequence did not pass structural validation (schema={:?}, validator={:?}, quality={:?})",
            result.get("schema_version"),
            result.get("validator_status"),
            result.get("quality_status")
        )));
    }
    let expected_rest = request
        .rest_frame
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("rest_frame canonical hash is missing"))?;
    let expected_action = request
        .pose_action
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("pose_action canonical hash is missing"))?;
    if result.get("rest_frame_sha256").and_then(Value::as_str) != Some(expected_rest)
        || result.get("pose_action_sha256").and_then(Value::as_str) != Some(expected_action)
        || result.get("sample_time_ticks") != request.sampling_policy.get("sample_time_ticks")
    {
        return Err(invalid("mechanical pose sequence binding differs"));
    }
    Ok(result)
}

fn record_value(
    request: &MechanicalAnimationClipV2PrepareRequest,
    input_sha256: &str,
    cohort: &ReplayCohort,
) -> Result<(Value, Value), RuntimeError> {
    let rest_frame_sha256 = request
        .rest_frame
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .filter(|hash| is_sha256(hash))
        .ok_or_else(|| invalid("rest_frame canonical hash is invalid"))?;
    let pose_action_sha256 = request
        .pose_action
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .filter(|hash| is_sha256(hash))
        .ok_or_else(|| invalid("pose_action canonical hash is invalid"))?;
    let sampling_policy_sha256 = canonical_json_hash(&request.sampling_policy);
    let mut typed = MechanicalAnimationClipV2Record {
        schema_version: "MechanicalAnimationClip@2".to_owned(),
        clip_id: request.clip_id.clone(),
        project_id: request.project_id.clone(),
        appearance_candidate_id: request.appearance_candidate_id.clone(),
        appearance_candidate_state_sha256: request.appearance_candidate_state_sha256.clone(),
        appearance_artifact_id: request.appearance_artifact_id.clone(),
        appearance_artifact_sha256: request.appearance_artifact_sha256.clone(),
        appearance_artifact_readback_sha256: request.appearance_artifact_readback_sha256.clone(),
        appearance_artifact_readback_object_sha256: request
            .appearance_artifact_readback_object_sha256
            .clone(),
        source_geometry_candidate_id: request.source_geometry_candidate_id.clone(),
        source_geometry_candidate_state_sha256: request
            .source_geometry_candidate_state_sha256
            .clone(),
        source_geometry_artifact_id: request.source_geometry_artifact_id.clone(),
        source_geometry_artifact_sha256: request.source_geometry_artifact_sha256.clone(),
        source_geometry_candidate_evidence_sha256: request
            .source_geometry_candidate_evidence_sha256
            .clone(),
        material_surface_quality_id: request.material_surface_quality_id.clone(),
        material_surface_quality_report_object_sha256: request
            .material_surface_quality_report_object_sha256
            .clone(),
        material_surface_quality_canonical_sha256: request
            .material_surface_quality_canonical_sha256
            .clone(),
        appearance_source_lineage_sidecar_object_sha256: request
            .appearance_source_lineage_sidecar_object_sha256
            .clone(),
        appearance_source_lineage_canonical_sha256: request
            .appearance_source_lineage_canonical_sha256
            .clone(),
        appearance_program_object_sha256: request.appearance_program_object_sha256.clone(),
        appearance_program_sha256: request.appearance_program_sha256.clone(),
        geometry_program_object_sha256: request.geometry_program_object_sha256.clone(),
        geometry_program_sha256: request.geometry_program_sha256.clone(),
        geometry_preservation_projection_sha256: request
            .geometry_preservation_projection_sha256
            .clone(),
        operator_catalog_sha256: request.operator_catalog_sha256.clone(),
        readback_config_sha256: request.readback_config_sha256.clone(),
        request_sha256: input_sha256.to_owned(),
        rest_frame: request.rest_frame.clone(),
        rest_frame_sha256: rest_frame_sha256.to_owned(),
        pose_action: request.pose_action.clone(),
        pose_action_sha256: pose_action_sha256.to_owned(),
        sampling_policy: request.sampling_policy.clone(),
        sampling_policy_sha256,
        source_replay: json!({
            "worker_build_cohort_sha256":cohort.worker_cohort,
            "first_artifact_sha256":request.appearance_artifact_sha256,
            "repeat_artifact_sha256":request.appearance_artifact_sha256,
            "byte_exact_with_appearance_artifact":true,
            "appearance_materials_replayed":true,
            "strict_readback_passed":true
        }),
        source_replay_worker_cohort_sha256: cohort.worker_cohort.clone(),
        replay_policy: REPLAY_POLICY.to_owned(),
        materialization_status: MATERIALIZATION_STATUS.to_owned(),
        quality_status: QUALITY_STATUS.to_owned(),
        visual_quality_status: VISUAL_STATUS.to_owned(),
        commercial_fps_quality_status: COMMERCIAL_STATUS.to_owned(),
        human_review_status: HUMAN_STATUS.to_owned(),
        commercial_engine_status: ENGINE_STATUS.to_owned(),
        runtime_write_performed: true,
        production_stage_advanced: false,
        candidate_confirmed: false,
        version_created: false,
        export_performed: false,
        canonical_sha256: String::new(),
        // The upstream quality record is immutable for this cohort. Reusing
        // its timestamp keeps the clip CAS bytes stable across an idempotent
        // replay instead of baking wall-clock time into the content hash.
        created_at: cohort.quality.created_at.clone(),
    };
    let mut clip = serde_json::to_value(&typed)
        .map_err(|error| invalid(format!("V2 clip record is malformed: {error}")))?;
    set_canonical(&mut clip, "canonical_sha256")?;
    typed = serde_json::from_value(clip.clone())
        .map_err(|error| invalid(format!("V2 clip record is malformed: {error}")))?;
    let mut link = serde_json::to_value(MechanicalAnimationClipV2LinkRecord {
        schema_version: "MechanicalAnimationClipLink@2".to_owned(),
        project_id: request.project_id.clone(),
        clip_id: request.clip_id.clone(),
        appearance_candidate_id: request.appearance_candidate_id.clone(),
        appearance_candidate_state_sha256: request.appearance_candidate_state_sha256.clone(),
        appearance_artifact_id: request.appearance_artifact_id.clone(),
        appearance_artifact_sha256: request.appearance_artifact_sha256.clone(),
        appearance_artifact_readback_sha256: request.appearance_artifact_readback_sha256.clone(),
        appearance_artifact_readback_object_sha256: request
            .appearance_artifact_readback_object_sha256
            .clone(),
        source_geometry_candidate_id: request.source_geometry_candidate_id.clone(),
        source_geometry_candidate_state_sha256: request
            .source_geometry_candidate_state_sha256
            .clone(),
        source_geometry_artifact_id: request.source_geometry_artifact_id.clone(),
        source_geometry_artifact_sha256: request.source_geometry_artifact_sha256.clone(),
        source_geometry_candidate_evidence_sha256: request
            .source_geometry_candidate_evidence_sha256
            .clone(),
        material_surface_quality_id: request.material_surface_quality_id.clone(),
        material_surface_quality_report_object_sha256: request
            .material_surface_quality_report_object_sha256
            .clone(),
        material_surface_quality_canonical_sha256: request
            .material_surface_quality_canonical_sha256
            .clone(),
        appearance_source_lineage_sidecar_object_sha256: request
            .appearance_source_lineage_sidecar_object_sha256
            .clone(),
        appearance_source_lineage_canonical_sha256: request
            .appearance_source_lineage_canonical_sha256
            .clone(),
        appearance_program_object_sha256: request.appearance_program_object_sha256.clone(),
        appearance_program_sha256: request.appearance_program_sha256.clone(),
        geometry_program_object_sha256: request.geometry_program_object_sha256.clone(),
        geometry_program_sha256: request.geometry_program_sha256.clone(),
        geometry_preservation_projection_sha256: request
            .geometry_preservation_projection_sha256
            .clone(),
        operator_catalog_sha256: request.operator_catalog_sha256.clone(),
        readback_config_sha256: request.readback_config_sha256.clone(),
        clip_object_sha256: String::new(),
        clip_sha256: typed.canonical_sha256.clone(),
        rest_frame_sha256: typed.rest_frame_sha256.clone(),
        pose_action_sha256: typed.pose_action_sha256.clone(),
        request_sha256: typed.request_sha256.clone(),
        source_replay_worker_cohort_sha256: cohort.worker_cohort.clone(),
        replay_policy: REPLAY_POLICY.to_owned(),
        materialization_status: MATERIALIZATION_STATUS.to_owned(),
        quality_status: QUALITY_STATUS.to_owned(),
        visual_quality_status: VISUAL_STATUS.to_owned(),
        commercial_fps_quality_status: COMMERCIAL_STATUS.to_owned(),
        human_review_status: HUMAN_STATUS.to_owned(),
        commercial_engine_status: ENGINE_STATUS.to_owned(),
        runtime_write_performed: true,
        production_stage_advanced: false,
        candidate_confirmed: false,
        version_created: false,
        export_performed: false,
        canonical_sha256: String::new(),
        created_at: typed.created_at.clone(),
    })
    .map_err(|error| invalid(format!("V2 link is malformed: {error}")))?;
    set_canonical(&mut link, "canonical_sha256")?;
    Ok((clip, link))
}

fn load_clip(
    runtime: &Runtime,
    link: &MechanicalAnimationClipV2LinkRecord,
) -> Result<MechanicalAnimationClipV2Record, RuntimeError> {
    let bytes = runtime.cas_read_bounded(&link.clip_object_sha256, MAX_CLIP_BYTES)?;
    let clip: MechanicalAnimationClipV2Record = serde_json::from_slice(&bytes)
        .map_err(|error| invalid(format!("durable V2 clip is malformed: {error}")))?;
    let clip_value = serde_json::to_value(&clip)
        .map_err(|error| invalid(format!("durable V2 clip cannot be serialized: {error}")))?;
    if canonical_json_bytes(&clip_value).map_err(|error| invalid(error.to_string()))? != bytes {
        return Err(invalid("durable V2 clip is not canonical JSON"));
    }
    if clip.canonical_sha256 != link.clip_sha256
        || clip.project_id != link.project_id
        || clip.clip_id != link.clip_id
        || clip.appearance_candidate_id != link.appearance_candidate_id
        || clip.appearance_artifact_sha256 != link.appearance_artifact_sha256
        || clip.source_geometry_artifact_sha256 != link.source_geometry_artifact_sha256
        || clip.request_sha256 != link.request_sha256
        || clip.rest_frame_sha256 != link.rest_frame_sha256
        || clip.pose_action_sha256 != link.pose_action_sha256
        || clip.source_replay_worker_cohort_sha256 != link.source_replay_worker_cohort_sha256
    {
        return Err(invalid("durable V2 clip differs from its link"));
    }
    Ok(clip)
}

fn link_from_value(value: &Value) -> Result<MechanicalAnimationClipV2LinkRecord, RuntimeError> {
    serde_json::from_value(value.clone())
        .map_err(|error| invalid(format!("V2 link is malformed: {error}")))
}

fn result_value(
    schema: &str,
    clip: &MechanicalAnimationClipV2Record,
    link: &MechanicalAnimationClipV2LinkRecord,
    replayed: bool,
    restart_hash_verified: bool,
) -> Result<Value, RuntimeError> {
    // The compact Store link is intentionally kept free of the clip payload,
    // but the public Link@2 projection is a self-describing durable envelope
    // and the contract requires its nested immutable clip.  Add the payload
    // only to this readback projection; SQLite/CAS ownership remains the
    // compact link plus the separately-addressed clip object.
    let mut durable_link = serde_json::to_value(link)
        .map_err(|error| invalid(format!("V2 link cannot be serialized: {error}")))?;
    durable_link
        .as_object_mut()
        .ok_or_else(|| invalid("V2 link serialization is not an object"))?
        .insert(
            "clip".to_owned(),
            serde_json::to_value(clip)
                .map_err(|error| invalid(format!("V2 clip cannot be nested in link: {error}")))?,
        );
    Ok(json!({
        "schema_version":schema,
        "clip":clip,
        "durable_link":durable_link,
        "replayed":replayed,
        "restart_hash_verified":restart_hash_verified,
        "runtime_write_performed":schema == PREPARE_RESULT_SCHEMA,
        "production_stage_advanced":false,
        "candidate_confirmed":false,
        "version_created":false,
        "export_performed":false,
        "quality_status":QUALITY_STATUS
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

pub(super) fn prepare(runtime: &Runtime, request_value: &Value) -> Result<Value, RuntimeError> {
    let (request, input_sha256) = parse_prepare(request_value)?;
    let cohort = validate_candidate_bindings(runtime, &request)?;
    let (clip_value, mut link_value) = record_value(&request, &input_sha256, &cohort)?;
    let clip_bytes =
        canonical_json_bytes(&clip_value).map_err(|error| invalid(error.to_string()))?;
    if clip_bytes.is_empty() || clip_bytes.len() as u64 > MAX_CLIP_BYTES {
        return Err(invalid("V2 clip exceeds the 1 MiB JSON budget"));
    }
    let clip_sha256 = sha256_hex(&clip_bytes);
    link_value["clip_object_sha256"] = Value::String(clip_sha256.clone());
    set_canonical(&mut link_value, "canonical_sha256")?;
    let expected_link = link_from_value(&link_value)?;

    if let Some(existing) = runtime
        .store
        .get_mechanical_animation_clip_v2_link(&request.appearance_candidate_id, &request.clip_id)?
    {
        if !same_link_for_replay(&existing, &expected_link) {
            return Err(invalid("V2 clip key is already bound to different content"));
        }
        let clip = load_clip(runtime, &existing)?;
        return result_value(PREPARE_RESULT_SCHEMA, &clip, &existing, true, true);
    }

    let reservation = runtime.store.begin_cas_reservation();
    let object = match runtime.store.put_object_reserved(
        &reservation,
        &clip_bytes,
        Some(&clip_sha256),
        CLIP_MIME,
        CLIP_KIND,
        &cohort.quality.created_at,
    ) {
        Ok(object) => object,
        Err(error) => return Err(error.into()),
    };
    link_value["clip_object_sha256"] = Value::String(object.record.sha256.clone());
    set_canonical(&mut link_value, "canonical_sha256")?;
    let link = link_from_value(&link_value)?;
    match runtime
        .store
        .record_mechanical_animation_clip_v2_link(&link)
    {
        Ok(stored) => {
            clean_reservation(runtime, &reservation, &object, false);
            let clip = load_clip(runtime, &stored)?;
            result_value(PREPARE_RESULT_SCHEMA, &clip, &stored, false, true)
        }
        Err(error) => {
            clean_reservation(runtime, &reservation, &object, true);
            Err(error.into())
        }
    }
}

pub(super) fn get(runtime: &Runtime, request_value: &Value) -> Result<Value, RuntimeError> {
    let object = exact_object(
        request_value,
        &[
            "schema_version",
            "project_id",
            "appearance_candidate_id",
            "clip_id",
        ],
        GET_SCHEMA,
    )?;
    if text(object, "schema_version")? != GET_SCHEMA {
        return Err(invalid("get schema differs"));
    }
    let project_id = id(object, "project_id")?.to_owned();
    let appearance_candidate_id = id(object, "appearance_candidate_id")?.to_owned();
    let clip_id = id(object, "clip_id")?.to_owned();
    let link = runtime
        .store
        .get_mechanical_animation_clip_v2_link(&appearance_candidate_id, &clip_id)?
        .ok_or_else(|| invalid("durable V2 animation clip is unavailable"))?;
    if link.project_id != project_id {
        return Err(invalid(
            "durable V2 animation clip belongs to another project",
        ));
    }
    let clip = load_clip(runtime, &link)?;
    let request = request_from_link(&link, &clip)?;
    let _cohort = validate_candidate_bindings(runtime, &request)?;
    result_value(GET_RESULT_SCHEMA, &clip, &link, false, true)
}

fn request_from_link(
    link: &MechanicalAnimationClipV2LinkRecord,
    clip: &MechanicalAnimationClipV2Record,
) -> Result<MechanicalAnimationClipV2PrepareRequest, RuntimeError> {
    Ok(MechanicalAnimationClipV2PrepareRequest {
        schema_version: PREPARE_SCHEMA.to_owned(),
        clip_id: link.clip_id.clone(),
        project_id: link.project_id.clone(),
        appearance_candidate_id: link.appearance_candidate_id.clone(),
        appearance_candidate_state_sha256: link.appearance_candidate_state_sha256.clone(),
        appearance_artifact_id: link.appearance_artifact_id.clone(),
        appearance_artifact_sha256: link.appearance_artifact_sha256.clone(),
        appearance_artifact_readback_sha256: link.appearance_artifact_readback_sha256.clone(),
        appearance_artifact_readback_object_sha256: link
            .appearance_artifact_readback_object_sha256
            .clone(),
        source_geometry_candidate_id: link.source_geometry_candidate_id.clone(),
        source_geometry_candidate_state_sha256: link.source_geometry_candidate_state_sha256.clone(),
        source_geometry_artifact_id: link.source_geometry_artifact_id.clone(),
        source_geometry_artifact_sha256: link.source_geometry_artifact_sha256.clone(),
        source_geometry_candidate_evidence_sha256: link
            .source_geometry_candidate_evidence_sha256
            .clone(),
        material_surface_quality_id: link.material_surface_quality_id.clone(),
        material_surface_quality_report_object_sha256: link
            .material_surface_quality_report_object_sha256
            .clone(),
        material_surface_quality_canonical_sha256: link
            .material_surface_quality_canonical_sha256
            .clone(),
        appearance_source_lineage_sidecar_object_sha256: link
            .appearance_source_lineage_sidecar_object_sha256
            .clone(),
        appearance_source_lineage_canonical_sha256: link
            .appearance_source_lineage_canonical_sha256
            .clone(),
        appearance_program_object_sha256: link.appearance_program_object_sha256.clone(),
        appearance_program_sha256: link.appearance_program_sha256.clone(),
        geometry_program_object_sha256: link.geometry_program_object_sha256.clone(),
        geometry_program_sha256: link.geometry_program_sha256.clone(),
        geometry_preservation_projection_sha256: link
            .geometry_preservation_projection_sha256
            .clone(),
        operator_catalog_sha256: link.operator_catalog_sha256.clone(),
        readback_config_sha256: link.readback_config_sha256.clone(),
        rest_frame: clip.rest_frame.clone(),
        pose_action: clip.pose_action.clone(),
        sampling_policy: clip.sampling_policy.clone(),
        replay_policy: link.replay_policy.clone(),
        input_sha256: link.request_sha256.clone(),
        idempotency_key: format!("replay-{}", link.clip_id),
    })
}

fn same_link_for_replay(
    left: &MechanicalAnimationClipV2LinkRecord,
    right: &MechanicalAnimationClipV2LinkRecord,
) -> bool {
    let mut left = left.clone();
    let mut right = right.clone();
    left.created_at.clear();
    right.created_at.clear();
    left.canonical_sha256.clear();
    right.canonical_sha256.clear();
    left == right
}

pub(super) fn preview(runtime: &Runtime, request_value: &Value) -> Result<Value, RuntimeError> {
    let object = exact_object(
        request_value,
        &[
            "schema_version",
            "project_id",
            "appearance_candidate_id",
            "clip_id",
            "sample_time_ticks",
            "preview_policy",
            "canonical_sha256",
        ],
        "MechanicalAnimationClipPreviewRequest@2",
    )?;
    if text(object, "schema_version")? != PREVIEW_SCHEMA
        || text(object, "preview_policy")?
            != "single-tick-transient-geometry-plus-appearance-double-worker-replay@1"
    {
        return Err(invalid("preview policy differs"));
    }
    verify_canonical(request_value, "canonical_sha256")?;
    let project_id = id(object, "project_id")?.to_owned();
    let appearance_candidate_id = id(object, "appearance_candidate_id")?.to_owned();
    let clip_id = id(object, "clip_id")?.to_owned();
    let sample_time_ticks = object
        .get("sample_time_ticks")
        .and_then(Value::as_u64)
        .filter(|tick| *tick <= 1_000_000)
        .ok_or_else(|| invalid("sample_time_ticks is outside the bounded range"))?;
    let link = runtime
        .store
        .get_mechanical_animation_clip_v2_link(&appearance_candidate_id, &clip_id)?
        .ok_or_else(|| invalid("durable V2 animation clip is unavailable"))?;
    if link.project_id != project_id {
        return Err(invalid(
            "durable V2 animation clip belongs to another project",
        ));
    }
    let clip = load_clip(runtime, &link)?;
    let request = request_from_link(&link, &clip)?;
    let cohort = validate_candidate_bindings(runtime, &request)?;
    let ticks = clip
        .sampling_policy
        .get("sample_time_ticks")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("durable V2 sample schedule is invalid"))?;
    if !ticks
        .iter()
        .any(|tick| tick.as_u64() == Some(sample_time_ticks))
    {
        return Err(invalid("sample_time_ticks is not scheduled in the clip"));
    }
    let pose_request = single_pose_request(&request, &cohort.quality, sample_time_ticks)?;
    let mut geometry_preview = json!({
        "schema_version":"MechanicalPoseGeometryPreviewRequest@1",
        "pose_evaluation_request":pose_request,
        "preview_policy":"transient-derived-program-worker-readback@1",
        "input_sha256":""
    });
    let mut preimage = geometry_preview.clone();
    preimage
        .as_object_mut()
        .expect("geometry preview is an object")
        .remove("input_sha256");
    geometry_preview["input_sha256"] = Value::String(canonical_json_hash(&preimage));
    let pose_geometry_preview = runtime.mechanical_pose_geometry_preview(&geometry_preview)?;
    let (
        appearance_transient_artifact_sha256,
        appearance_transient_artifact_readback_sha256,
        appearance_replay_worker_cohort_sha256,
        appearance_transient_program_sha256,
    ) = preview_appearance_replay(runtime, &request, &cohort, &pose_geometry_preview)?;
    if appearance_replay_worker_cohort_sha256 != link.source_replay_worker_cohort_sha256 {
        return Err(invalid(
            "preview source and appearance worker cohorts differ",
        ));
    }
    let mut frame_identity = json!({
        "schema_version":"MechanicalAnimationFrameIdentity@2",
        "clip_sha256":link.clip_sha256,
        "sample_time_ticks":sample_time_ticks,
        "evaluated_pose_sha256":pose_geometry_preview["evaluated_pose_sha256"],
        "posed_program_sha256":pose_geometry_preview["posed_program_sha256"],
        "appearance_artifact_sha256":link.appearance_artifact_sha256,
        "appearance_transient_artifact_sha256":appearance_transient_artifact_sha256,
        "appearance_transient_artifact_readback_sha256":appearance_transient_artifact_readback_sha256,
        "worker_build_cohort_sha256":link.source_replay_worker_cohort_sha256,
        "appearance_transient_program_sha256":appearance_transient_program_sha256
    });
    set_canonical(&mut frame_identity, "canonical_sha256")?;
    let mut result = json!({
        "schema_version":PREVIEW_RESULT_SCHEMA,
        "project_id":link.project_id,
        "appearance_candidate_id":link.appearance_candidate_id,
        "appearance_candidate_state_sha256":link.appearance_candidate_state_sha256,
        "appearance_artifact_sha256":link.appearance_artifact_sha256,
        "appearance_artifact_readback_sha256":link.appearance_artifact_readback_sha256,
        "appearance_artifact_readback_object_sha256":link.appearance_artifact_readback_object_sha256,
        "source_geometry_candidate_id":link.source_geometry_candidate_id,
        "source_geometry_candidate_state_sha256":link.source_geometry_candidate_state_sha256,
        "source_geometry_artifact_sha256":link.source_geometry_artifact_sha256,
        "source_geometry_candidate_evidence_sha256":link.source_geometry_candidate_evidence_sha256,
        "clip_id":link.clip_id,
        "clip_object_sha256":link.clip_object_sha256,
        "clip_sha256":link.clip_sha256,
        "rest_frame_sha256":link.rest_frame_sha256,
        "pose_action_sha256":link.pose_action_sha256,
        "sample_time_ticks":sample_time_ticks,
        "frame_sha256":frame_identity["canonical_sha256"],
        "source_replay_worker_cohort_sha256":link.source_replay_worker_cohort_sha256,
        "appearance_transient_artifact_sha256":appearance_transient_artifact_sha256,
        "appearance_transient_artifact_readback_sha256":appearance_transient_artifact_readback_sha256,
        "appearance_replay_worker_cohort_sha256":appearance_replay_worker_cohort_sha256,
        "appearance_program_sha256":link.appearance_program_sha256,
        "appearance_transient_program_sha256":appearance_transient_program_sha256,
        "material_pack_manifest_sha256":cohort.quality.material_pack_manifest_sha256,
        "geometry_preservation_projection_sha256":link.geometry_preservation_projection_sha256,
        "pose_geometry_preview":pose_geometry_preview,
        "geometry_materialization":"transient-double-worker-glb-not-persisted",
        "appearance_materialization":"transient-double-worker-appearance-not-persisted",
        "runtime_write_performed":false,
        "persistent_user_data_touched":false,
        "quality_status":QUALITY_STATUS,
        "visual_quality_status":VISUAL_STATUS,
        "commercial_fps_quality_status":COMMERCIAL_STATUS,
        "human_review_status":HUMAN_STATUS,
        "commercial_engine_status":ENGINE_STATUS,
        "limitations":[
            "rigid-parts-only-no-skinning-or-deformation",
            "single-scheduled-tick-per-preview-call",
            "transient-geometry-and-appearance-not-persisted",
            "no-ik-constraints-nla-fcurves-drivers-or-timeline",
            "not-blender-armature-animation-or-python-parity",
            "structural-replay-does-not-prove-visual-quality"
        ],
        "canonical_sha256":""
    });
    set_canonical(&mut result, "canonical_sha256")?;
    if canonical_json_bytes(&result)
        .map_err(|error| invalid(error.to_string()))?
        .len()
        > MAX_CLIP_BYTES as usize
    {
        return Err(invalid("V2 preview exceeds the 1 MiB response budget"));
    }
    Ok(result)
}

fn single_pose_request(
    request: &MechanicalAnimationClipV2PrepareRequest,
    quality: &forgecad_contracts::CandidateMaterialSurfaceQualityRecord,
    tick: u64,
) -> Result<Value, RuntimeError> {
    let mut value = json!({
        "schema_version":"MechanicalPoseEvaluationRequest@1",
        "project_id":request.project_id,
        "artifact_id":request.source_geometry_artifact_sha256,
        "candidate_id":request.source_geometry_candidate_id,
        "artifact_readback_sha256":quality.source_artifact_readback_sha256,
        "program_sha256":request.geometry_program_sha256,
        "operator_catalog_sha256":request.operator_catalog_sha256,
        "readback_config_sha256":request.readback_config_sha256,
        "rest_frame_draft":rest_frame_draft(&request.rest_frame)?,
        "pose_action_draft":pose_action_draft(&request.pose_action)?,
        "sample_time_ticks":tick,
        "input_sha256":""
    });
    let mut preimage = value.clone();
    preimage
        .as_object_mut()
        .expect("pose request is an object")
        .remove("input_sha256");
    value["input_sha256"] = Value::String(canonical_json_hash(&preimage));
    Ok(value)
}

fn preview_appearance_replay(
    runtime: &Runtime,
    request: &MechanicalAnimationClipV2PrepareRequest,
    cohort: &ReplayCohort,
    pose_geometry_preview: &Value,
) -> Result<(String, String, String, String), RuntimeError> {
    let posed_program = pose_geometry_preview
        .get("posed_geometry_program")
        .cloned()
        .ok_or_else(|| invalid("pose geometry preview omitted posed GeometryProgram"))?;
    let posed_program_sha256 = pose_geometry_preview
        .get("posed_program_sha256")
        .and_then(Value::as_str)
        .filter(|hash| is_sha256(hash))
        .ok_or_else(|| invalid("pose geometry preview omitted posed program hash"))?;
    if posed_program
        .get("canonical_sha256")
        .and_then(Value::as_str)
        != Some(posed_program_sha256)
    {
        return Err(invalid("posed GeometryProgram canonical hash differs"));
    }

    // The worker binds an AppearanceProgram to the exact GeometryProgram hash.
    // Pose lowering creates a new transient program hash, so derive an
    // in-memory appearance binding for this preview while preserving every
    // source material zone and pack/provenance field. Nothing is persisted.
    let mut transient_appearance = cohort.appearance_program.clone();
    let source_zones = transient_appearance.get("material_zones").cloned();
    transient_appearance["geometry_program_sha256"] =
        Value::String(posed_program_sha256.to_owned());
    let mut transient_preimage = transient_appearance
        .as_object()
        .ok_or_else(|| invalid("transient appearance program is not an object"))?
        .clone();
    transient_preimage.remove("canonical_sha256");
    transient_appearance["canonical_sha256"] =
        Value::String(canonical_json_hash(&Value::Object(transient_preimage)));
    if transient_appearance.get("material_zones") != source_zones.as_ref() {
        return Err(invalid("transient appearance material zones changed"));
    }

    let first = compile_geometry_with_runtime_worker(&posed_program, Some(&transient_appearance))
        .map_err(|error| invalid(format!("preview appearance replay failed: {error}")))?;
    let repeat = compile_geometry_with_runtime_worker(&posed_program, Some(&transient_appearance))
        .map_err(|error| invalid(format!("preview appearance repeat replay failed: {error}")))?;
    if first.glb != repeat.glb
        || first.program_sha256 != posed_program_sha256
        || repeat.program_sha256 != posed_program_sha256
        || first.part_ids != repeat.part_ids
        || first.triangle_count != repeat.triangle_count
        || first.material_zone_ids != repeat.material_zone_ids
        || first.uv_status != repeat.uv_status
        || first.tangent_status != repeat.tangent_status
    {
        return Err(invalid("preview appearance worker replay differs"));
    }
    let inspection = strict_glb_inspection(&first.glb)?;
    if !inspection.hard_gate_passed
        || inspection.program_sha256 != posed_program_sha256
        || inspection.operator_catalog_sha256.as_deref()
            != Some(request.operator_catalog_sha256.as_str())
        || inspection.readback_config_sha256 != request.readback_config_sha256
    {
        return Err(invalid("preview appearance strict readback differs"));
    }
    validate_worker_metadata(&first, &inspection).map_err(|error| {
        invalid(format!(
            "preview appearance worker metadata differs: {error}"
        ))
    })?;
    validate_worker_metadata(&repeat, &inspection).map_err(|error| {
        invalid(format!(
            "preview appearance repeat metadata differs: {error}"
        ))
    })?;
    validate_glb_material_pack_identity(
        &first.glb,
        &cohort.quality.material_pack_id,
        &cohort.quality.material_pack_manifest_sha256,
    )?;

    let source_appearance_glb = runtime.cas_read_bounded(
        &request.appearance_artifact_sha256,
        MAX_GEOMETRY_ARTIFACT_BYTES,
    )?;
    let source_inspection = strict_glb_inspection(&source_appearance_glb)?;
    if inspection.part_ids != source_inspection.part_ids
        || inspection.triangle_count != source_inspection.triangle_count
        || inspection.material_zone_ids != source_inspection.material_zone_ids
    {
        return Err(invalid(
            "preview appearance material/geometry inventory differs",
        ));
    }
    let artifact_sha256 = sha256_hex(&first.glb);
    let readback = artifact_readback_v2_value(
        &artifact_sha256,
        &request.appearance_candidate_id,
        &inspection,
        first.glb.len() as u64,
    );
    let readback_sha256 = readback
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .filter(|hash| is_sha256(hash))
        .ok_or_else(|| invalid("preview appearance readback hash is unavailable"))?
        .to_owned();
    let worker_cohort = first
        .build_cohort_sha256
        .clone()
        .filter(|hash| is_sha256(hash))
        .ok_or_else(|| invalid("preview appearance worker cohort is unavailable"))?;
    if repeat.build_cohort_sha256.as_deref() != Some(worker_cohort.as_str()) {
        return Err(invalid("preview appearance worker cohorts differ"));
    }
    let transient_program_sha256 = transient_appearance
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .filter(|hash| is_sha256(hash))
        .ok_or_else(|| invalid("preview transient appearance program hash is unavailable"))?
        .to_owned();
    Ok((
        artifact_sha256,
        readback_sha256,
        worker_cohort,
        transient_program_sha256,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(ch: char) -> String {
        std::iter::repeat_n(ch, 64).collect()
    }

    fn canonical_value(mut value: Value) -> Value {
        set_canonical(&mut value, "canonical_sha256").expect("test value is an object");
        value
    }

    fn prepare_request() -> Value {
        let mut value = Map::new();
        value.insert("schema_version".to_owned(), json!(PREPARE_SCHEMA));
        value.insert("clip_id".to_owned(), json!("clip-v2"));
        value.insert("project_id".to_owned(), json!("project-1"));
        value.insert(
            "appearance_candidate_id".to_owned(),
            json!("appearance-candidate"),
        );
        value.insert(
            "appearance_candidate_state_sha256".to_owned(),
            json!(hash('a')),
        );
        value.insert(
            "appearance_artifact_id".to_owned(),
            json!("appearance-artifact"),
        );
        value.insert("appearance_artifact_sha256".to_owned(), json!(hash('b')));
        value.insert(
            "appearance_artifact_readback_sha256".to_owned(),
            json!(hash('c')),
        );
        value.insert(
            "appearance_artifact_readback_object_sha256".to_owned(),
            json!(hash('d')),
        );
        value.insert(
            "source_geometry_candidate_id".to_owned(),
            json!("geometry-candidate"),
        );
        value.insert(
            "source_geometry_candidate_state_sha256".to_owned(),
            json!(hash('e')),
        );
        value.insert(
            "source_geometry_artifact_id".to_owned(),
            json!("geometry-artifact"),
        );
        value.insert(
            "source_geometry_artifact_sha256".to_owned(),
            json!(hash('f')),
        );
        value.insert(
            "source_geometry_candidate_evidence_sha256".to_owned(),
            json!(hash('0')),
        );
        value.insert("material_surface_quality_id".to_owned(), json!("quality-1"));
        value.insert(
            "material_surface_quality_report_object_sha256".to_owned(),
            json!(hash('1')),
        );
        value.insert(
            "material_surface_quality_canonical_sha256".to_owned(),
            json!(hash('2')),
        );
        value.insert(
            "appearance_source_lineage_sidecar_object_sha256".to_owned(),
            json!(hash('3')),
        );
        value.insert(
            "appearance_source_lineage_canonical_sha256".to_owned(),
            json!(hash('4')),
        );
        value.insert(
            "appearance_program_object_sha256".to_owned(),
            json!(hash('5')),
        );
        value.insert("appearance_program_sha256".to_owned(), json!(hash('6')));
        value.insert(
            "geometry_program_object_sha256".to_owned(),
            json!(hash('7')),
        );
        value.insert("geometry_program_sha256".to_owned(), json!(hash('8')));
        value.insert(
            "geometry_preservation_projection_sha256".to_owned(),
            json!(hash('9')),
        );
        value.insert("operator_catalog_sha256".to_owned(), json!(hash('a')));
        value.insert("readback_config_sha256".to_owned(), json!(hash('b')));
        value.insert(
            "rest_frame".to_owned(),
            canonical_value(json!({
                "schema_version":"MechanicalRestFrame@1"
            })),
        );
        value.insert(
            "pose_action".to_owned(),
            canonical_value(json!({
                "schema_version":"MechanicalPoseAction@1"
            })),
        );
        value.insert(
            "sampling_policy".to_owned(),
            json!({
                "schema_version":"MechanicalAnimationSamplingPolicy@1",
                "timebase_hz":1000,
                "interpolation":"scalar-linear-integer-ticks-clamped",
                "unkeyed":"rest",
                "sample_time_ticks":[0,10],
                "max_samples":16,
                "frame_preview_batch_size":1
            }),
        );
        value.insert("replay_policy".to_owned(), json!(REPLAY_POLICY));
        value.insert("input_sha256".to_owned(), json!(hash('c')));
        value.insert("idempotency_key".to_owned(), json!("request-1"));
        let mut request = Value::Object(value);
        let mut preimage = request.clone();
        let object = preimage.as_object_mut().expect("request object");
        object.remove("input_sha256");
        object.remove("idempotency_key");
        request["input_sha256"] = Value::String(canonical_json_hash(&preimage));
        request
    }

    #[test]
    fn parse_prepare_keeps_artifact_ids_distinct_from_cas_hashes() {
        let request = prepare_request();
        let (parsed, input_sha256) = parse_prepare(&request).expect("closed V2 request");
        assert_eq!(parsed.appearance_artifact_id, "appearance-artifact");
        assert_eq!(parsed.source_geometry_artifact_id, "geometry-artifact");
        assert_eq!(parsed.input_sha256, input_sha256);
    }

    #[test]
    fn parse_prepare_rejects_non_monotonic_sampling() {
        let mut request = prepare_request();
        request["sampling_policy"]["sample_time_ticks"] = json!([10, 0]);
        let mut preimage = request.clone();
        let object = preimage.as_object_mut().expect("request object");
        object.remove("input_sha256");
        object.remove("idempotency_key");
        request["input_sha256"] = Value::String(canonical_json_hash(&preimage));
        let error = parse_prepare(&request).expect_err("non-monotonic schedule must fail");
        assert!(error.to_string().contains("strictly increasing"));
    }
}
