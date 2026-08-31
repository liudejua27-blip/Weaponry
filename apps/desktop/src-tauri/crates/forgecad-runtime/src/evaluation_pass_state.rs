//! Runtime-owned evaluation of the closed `KnifePassState@1` contract.
//!
//! A pass state is a read/re-derivation seam over already durable Runtime
//! evidence.  This module does not accept geometry, render output, a camera,
//! a quality result, or an object hash as authority.  It resolves the exact
//! SourceBinding anchor, the selected AuthoringMesh revision and the candidate
//! and visual evidence rows from Store/CAS, then compares the caller's Main
//! proposal with that projection.  The only write is the final Main JSON CAS
//! object followed by Store's atomic pass-state transaction.

use super::{
    authoring_mesh_v2_durable, canonical_json_bytes, canonical_json_hash, is_opaque_id, is_sha256,
    sha256_hex, Runtime, RuntimeError,
};
use forgecad_contracts::{AuthoringMeshRevision, CandidateRecord, GeometryCandidateEvidenceRecord};
use forgecad_store::{
    CasObject, KnifePassStateCasBundle, KnifePassStateCommit, KnifePassStateStoreRecord,
    KnifeSourceBindingStoreRecord, KNIFE_PASS_STATE_JSON_MIME, KNIFE_PASS_STATE_MAX_JSON_BYTES,
    KNIFE_PASS_STATE_OBJECT_KIND,
};
use serde_json::{json, Map, Value};
use std::collections::{BTreeSet, HashSet};

pub(crate) const MAIN_SCHEMA: &str = "KnifePassState@1";
pub(crate) const PREPARE_SCHEMA: &str = "KnifePassStatePrepareRequest@1";
pub(crate) const GET_SCHEMA: &str = "KnifePassStateGetRequest@1";
pub(crate) const RESULT_SCHEMA: &str = "KnifePassStateResult@1";
pub(crate) const PREPARE_OPERATION: &str = "knife_pass_state_prepare";
pub(crate) const GET_OPERATION: &str = "knife_pass_state_get";
const WRITER_POLICY: &str = "forgecad-runtime-only-state-writer@1";
const REQUEST_CANONICALIZATION: &str = "canonical-json-sha256-excluding-input-sha256@1";
const MAIN_CANONICALIZATION: &str = "canonical-json-sha256-excluding-canonical-sha256@1";
const MAX_RESPONSE_BYTES: u64 = 1_048_576;
const MAX_LINEAGE_BYTES: u64 = 64 * 1024 * 1024;
const STAGE: &str = "camera-lock";
const FIXED_VIEW_POLICY: &str = "single-runtime-bound-primary-reference-view@1";
const EVIDENCE_BUNDLE_SCHEMA: &str = "KnifeEvidenceBundle@1";

const MAIN_FIELDS: &[&str] = &[
    "schema_version",
    "pass_id",
    "parent_pass_id",
    "parent_pass_sha256",
    "project_id",
    "stage",
    "source_binding_id",
    "source_binding_sha256",
    "source_binding_object_sha256",
    "intent_bundle_id",
    "intent_bundle_sha256",
    "intent_bundle_object_sha256",
    "brief_id",
    "brief_sha256",
    "brief_object_sha256",
    "reference_id",
    "reference_object_sha256",
    "reference_evidence_sha256",
    "source_candidate_id",
    "source_candidate_state_sha256",
    "baseline_candidate_id",
    "baseline_candidate_state_sha256",
    "baseline_artifact_sha256",
    "baseline_geometry_program_sha256",
    "baseline_geometry_program_object_sha256",
    "baseline_artifact_readback_object_sha256",
    "baseline_representation_plan_sha256",
    "attempt_candidate_id",
    "attempt_candidate_state_sha256",
    "attempt_artifact_sha256",
    "attempt_geometry_program_sha256",
    "attempt_geometry_program_object_sha256",
    "attempt_artifact_readback_object_sha256",
    "attempt_representation_plan_sha256",
    "authoring_mesh_id",
    "authoring_mesh_lineage_id",
    "authoring_mesh_revision_id",
    "authoring_mesh_revision_index",
    "authoring_mesh_revision_sha256",
    "authoring_mesh_revision_object_sha256",
    "authoring_mesh_identity_sha256",
    "authoring_mesh_sha256",
    "modifier_graph_id",
    "modifier_graph_sha256",
    "evaluated_mesh_id",
    "evaluated_mesh_sha256",
    "high_artifact_id",
    "high_artifact_sha256",
    "fixed_view",
    "camera_set_sha256",
    "render_set_id",
    "render_set_sha256",
    "render_set_object_sha256",
    "reference_comparison_id",
    "reference_comparison_sha256",
    "reference_comparison_object_sha256",
    "quality_report_id",
    "quality_report_sha256",
    "quality_report_object_sha256",
    "evidence_bundle_sha256",
    "hard_gate_status",
    "visual_gate_status",
    "quality_status",
    "high_status",
    "human_status",
    "engine_status",
    "unknowns",
    "unlocked_successor",
    "high_mesh_created",
    "high_stage_unlocked",
    "production_stage_advanced",
    "candidate_confirmed",
    "version_created",
    "export_performed",
    "canonicalization_policy",
    "canonical_sha256",
    "created_at",
];

const PREPARE_FIELDS: &[&str] = &[
    "schema_version",
    "operation",
    "project_id",
    "pass_state",
    "idempotency_key",
    "max_response_bytes",
    "runtime_write_performed",
    "writer_policy",
    "canonicalization_policy",
    "input_sha256",
];

const GET_FIELDS: &[&str] = &[
    "schema_version",
    "operation",
    "project_id",
    "pass_id",
    "pass_state_sha256",
    "pass_state_object_sha256",
    "source_binding_id",
    "source_binding_sha256",
    "source_binding_object_sha256",
    "intent_bundle_id",
    "intent_bundle_sha256",
    "intent_bundle_object_sha256",
    "brief_id",
    "brief_sha256",
    "brief_object_sha256",
    "reference_id",
    "reference_object_sha256",
    "reference_evidence_sha256",
    "source_candidate_id",
    "source_candidate_state_sha256",
    "baseline_candidate_id",
    "baseline_candidate_state_sha256",
    "baseline_artifact_sha256",
    "baseline_geometry_program_sha256",
    "baseline_geometry_program_object_sha256",
    "baseline_artifact_readback_object_sha256",
    "baseline_representation_plan_sha256",
    "attempt_candidate_id",
    "attempt_candidate_state_sha256",
    "attempt_artifact_sha256",
    "attempt_geometry_program_sha256",
    "attempt_geometry_program_object_sha256",
    "attempt_artifact_readback_object_sha256",
    "attempt_representation_plan_sha256",
    "authoring_mesh_id",
    "authoring_mesh_lineage_id",
    "authoring_mesh_revision_id",
    "authoring_mesh_revision_index",
    "authoring_mesh_revision_sha256",
    "authoring_mesh_revision_object_sha256",
    "authoring_mesh_identity_sha256",
    "authoring_mesh_sha256",
    "fixed_view_id",
    "camera_set_sha256",
    "render_set_id",
    "render_set_sha256",
    "render_set_object_sha256",
    "reference_comparison_id",
    "reference_comparison_sha256",
    "reference_comparison_object_sha256",
    "quality_report_id",
    "quality_report_sha256",
    "quality_report_object_sha256",
    "evidence_bundle_sha256",
    "max_response_bytes",
    "runtime_write_performed",
    "persistent_user_data_touched",
    "writer_policy",
    "canonicalization_policy",
    "input_sha256",
];

fn invalid(message: impl Into<String>) -> RuntimeError {
    RuntimeError::InvalidInput(format!("KNIFE_PASS_STATE_INVALID: {}", message.into()))
}

fn mismatch(message: impl Into<String>) -> RuntimeError {
    RuntimeError::InvalidInput(format!(
        "KNIFE_PASS_STATE_DERIVED_MISMATCH: {}",
        message.into()
    ))
}

fn exact_object<'a>(
    value: &'a Value,
    fields: &[&str],
    context: &str,
) -> Result<&'a Map<String, Value>, RuntimeError> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid(format!("{context} must be an object")))?;
    let expected = fields.iter().copied().collect::<BTreeSet<_>>();
    let actual = object.keys().map(String::as_str).collect::<BTreeSet<_>>();
    if expected != actual {
        return Err(invalid(format!(
            "{context} fields differ from the closed contract"
        )));
    }
    Ok(object)
}

fn text<'a>(
    object: &'a Map<String, Value>,
    key: &str,
    context: &str,
) -> Result<&'a str, RuntimeError> {
    object
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid(format!("{context}.{key} must be a string")))
}

fn id(object: &Map<String, Value>, key: &str, context: &str) -> Result<String, RuntimeError> {
    let value = text(object, key, context)?;
    if !is_opaque_id(value) {
        return Err(invalid(format!(
            "{context}.{key} is not an opaque identifier"
        )));
    }
    Ok(value.to_owned())
}

fn hash(object: &Map<String, Value>, key: &str, context: &str) -> Result<String, RuntimeError> {
    let value = text(object, key, context)?;
    if !is_sha256(value) {
        return Err(invalid(format!("{context}.{key} is not a SHA-256")));
    }
    Ok(value.to_owned())
}

fn request_header(
    object: &Map<String, Value>,
    schema: &str,
    operation: &str,
    context: &str,
) -> Result<(), RuntimeError> {
    if text(object, "schema_version", context)? != schema
        || text(object, "operation", context)? != operation
        || text(object, "writer_policy", context)? != WRITER_POLICY
        || text(object, "canonicalization_policy", context)? != REQUEST_CANONICALIZATION
        || object.get("max_response_bytes").and_then(Value::as_u64) != Some(MAX_RESPONSE_BYTES)
        || object
            .get("runtime_write_performed")
            .and_then(Value::as_bool)
            != Some(false)
    {
        return Err(invalid(format!(
            "{context} policy or response budget differs"
        )));
    }
    Ok(())
}

fn request_hash(
    request: &Value,
    object: &Map<String, Value>,
    context: &str,
) -> Result<(), RuntimeError> {
    let supplied = hash(object, "input_sha256", context)?;
    let mut preimage = request.clone();
    preimage["input_sha256"] = Value::String(String::new());
    if canonical_json_hash(&preimage) != supplied {
        return Err(invalid(format!(
            "{context}.input_sha256 does not bind the closed request"
        )));
    }
    Ok(())
}

fn main_value_is_closed<'a>(
    value: &'a Value,
    context: &str,
) -> Result<&'a Map<String, Value>, RuntimeError> {
    let object = exact_object(value, MAIN_FIELDS, context)?;
    if text(object, "schema_version", context)? != MAIN_SCHEMA
        || text(object, "canonicalization_policy", context)? != MAIN_CANONICALIZATION
    {
        return Err(invalid(format!(
            "{context} schema or canonicalization policy differs"
        )));
    }
    let canonical = hash(object, "canonical_sha256", context)?;
    let mut preimage = value.clone();
    preimage["canonical_sha256"] = Value::String(String::new());
    if canonical_json_hash(&preimage) != canonical {
        return Err(invalid(format!(
            "{context}.canonical_sha256 does not bind Main"
        )));
    }
    Ok(object)
}

fn scalar_identity(main: &Map<String, Value>, key: &str) -> Result<String, RuntimeError> {
    id(main, key, "pass_state")
}

fn scalar_hash(main: &Map<String, Value>, key: &str) -> Result<String, RuntimeError> {
    hash(main, key, "pass_state")
}

fn nullable_identity(main: &Map<String, Value>, key: &str) -> Result<Option<String>, RuntimeError> {
    match main.get(key) {
        Some(Value::Null) => Ok(None),
        Some(Value::String(value)) if is_opaque_id(value) => Ok(Some(value.clone())),
        _ => Err(invalid(format!(
            "pass_state.{key} must be null or an opaque identifier"
        ))),
    }
}

fn nullable_hash(main: &Map<String, Value>, key: &str) -> Result<Option<String>, RuntimeError> {
    match main.get(key) {
        Some(Value::Null) => Ok(None),
        Some(Value::String(value)) if is_sha256(value) => Ok(Some(value.clone())),
        _ => Err(invalid(format!(
            "pass_state.{key} must be null or a SHA-256"
        ))),
    }
}

fn read_canonical_json(
    runtime: &Runtime,
    hash: &str,
    max_bytes: u64,
    context: &str,
) -> Result<Value, RuntimeError> {
    let object = runtime
        .store
        .get_object(hash)?
        .ok_or_else(|| invalid(format!("{context} CAS metadata is missing")))?;
    if object.sha256 != hash || object.size_bytes == 0 || object.size_bytes > max_bytes {
        return Err(invalid(format!(
            "{context} CAS metadata is outside its bound"
        )));
    }
    let bytes = runtime.cas_read_bounded(hash, max_bytes)?;
    if bytes.len() as u64 != object.size_bytes || sha256_hex(&bytes) != hash {
        return Err(invalid(format!("{context} CAS bytes are not hash-bound")));
    }
    let value: Value = serde_json::from_slice(&bytes)
        .map_err(|error| invalid(format!("{context} CAS JSON is invalid: {error}")))?;
    let canonical = canonical_json_bytes(&value).map_err(|error| invalid(error.to_string()))?;
    if canonical != bytes {
        return Err(invalid(format!("{context} CAS JSON is not canonical")));
    }
    Ok(value)
}

fn read_semantic_json(
    runtime: &Runtime,
    hash: &str,
    max_bytes: u64,
    context: &str,
) -> Result<(Value, String), RuntimeError> {
    let value = read_canonical_json(runtime, hash, max_bytes, context)?;
    let semantic = value
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .ok_or_else(|| invalid(format!("{context} semantic hash is missing")))?
        .to_owned();
    let mut preimage = value.clone();
    preimage["canonical_sha256"] = Value::String(String::new());
    if canonical_json_hash(&preimage) != semantic {
        return Err(invalid(format!(
            "{context} semantic hash does not bind its payload"
        )));
    }
    Ok((value, semantic))
}

fn require_string(value: &Value, key: &str, context: &str) -> Result<String, RuntimeError> {
    let value = value
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid(format!("{context}.{key} is missing")))?;
    if !is_opaque_id(value) {
        return Err(invalid(format!(
            "{context}.{key} is not an opaque identifier"
        )));
    }
    Ok(value.to_owned())
}

fn require_sha_value(value: &Value, key: &str, context: &str) -> Result<String, RuntimeError> {
    let value = value
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid(format!("{context}.{key} is missing")))?;
    if !is_sha256(value) {
        return Err(invalid(format!("{context}.{key} is not a SHA-256")));
    }
    Ok(value.to_owned())
}

#[derive(Debug, Clone)]
struct CandidateTruth {
    candidate: CandidateRecord,
    evidence: GeometryCandidateEvidenceRecord,
    artifact_sha256: String,
    program_sha256: String,
    program_object_sha256: String,
    readback_object_sha256: String,
    representation_plan_sha256: String,
    hard_gate_passed: bool,
}

fn candidate_truth(
    runtime: &Runtime,
    candidate_id: &str,
    project_id: &str,
    reference_id: &str,
) -> Result<CandidateTruth, RuntimeError> {
    let candidate = runtime
        .candidate(candidate_id)?
        .ok_or_else(|| invalid(format!("candidate {candidate_id} is not durable")))?;
    if candidate.project_id != project_id || candidate.state != "reviewable" {
        return Err(mismatch(format!(
            "candidate {candidate_id} is outside the project/reviewable scope"
        )));
    }
    let artifact_sha256 = candidate
        .prepared_object_sha256
        .clone()
        .ok_or_else(|| invalid(format!("candidate {candidate_id} has no prepared artifact")))?;
    let evidence = runtime
        .store
        .get_geometry_candidate_evidence(candidate_id)?
        .ok_or_else(|| {
            invalid(format!(
                "candidate {candidate_id} has no GeometryCandidateEvidence"
            ))
        })?;
    if evidence.project_id != project_id
        || evidence.candidate_id != candidate_id
        || evidence.reference_id.as_deref() != Some(reference_id)
        || evidence.artifact_object_sha256 != artifact_sha256
    {
        return Err(mismatch(format!(
            "candidate {candidate_id} evidence binding differs"
        )));
    }
    let artifact_object = runtime.store.get_object(&artifact_sha256)?.ok_or_else(|| {
        invalid(format!(
            "candidate {candidate_id} artifact CAS metadata is missing"
        ))
    })?;
    if artifact_object.kind != "geometry-glb" || artifact_object.mime != "model/gltf-binary" {
        return Err(invalid(format!(
            "candidate {candidate_id} artifact CAS metadata is invalid"
        )));
    }
    let artifact_bytes = runtime.cas_read_bounded(&artifact_sha256, MAX_LINEAGE_BYTES)?;
    if sha256_hex(&artifact_bytes) != artifact_sha256 {
        return Err(invalid(format!(
            "candidate {candidate_id} artifact bytes are not hash-bound"
        )));
    }
    let program_object = runtime
        .store
        .get_object(&evidence.geometry_program_object_sha256)?
        .ok_or_else(|| {
            invalid(format!(
                "candidate {candidate_id} GeometryProgram CAS metadata is missing"
            ))
        })?;
    if program_object.kind != "geometry-program-v2"
        || program_object.mime != KNIFE_PASS_STATE_JSON_MIME
    {
        return Err(invalid(format!(
            "candidate {candidate_id} GeometryProgram CAS metadata is invalid"
        )));
    }
    let program = read_canonical_json(
        runtime,
        &evidence.geometry_program_object_sha256,
        MAX_LINEAGE_BYTES,
        "GeometryProgram",
    )?;
    if program.get("schema_version").and_then(Value::as_str) != Some("GeometryProgram@2")
        || program.get("canonical_sha256").is_some()
        || canonical_json_hash(&program) != evidence.geometry_program_sha256
    {
        return Err(mismatch(format!(
            "candidate {candidate_id} GeometryProgram semantic identity differs"
        )));
    }
    let representation_plan_sha256 =
        require_sha_value(&program, "representation_plan_sha256", "GeometryProgram")?;
    let (readback, _) = read_semantic_json(
        runtime,
        &evidence.artifact_readback_object_sha256,
        8 * 1024 * 1024,
        "ArtifactReadback",
    )?;
    if readback.get("schema_version").and_then(Value::as_str) != Some("ArtifactReadback@2")
        || readback.get("candidate_id").and_then(Value::as_str) != Some(candidate_id)
        || readback.get("object_sha256").and_then(Value::as_str) != Some(artifact_sha256.as_str())
        || readback.get("program_sha256").and_then(Value::as_str)
            != Some(evidence.geometry_program_sha256.as_str())
    {
        return Err(mismatch(format!(
            "candidate {candidate_id} ArtifactReadback differs"
        )));
    }
    let hard_gate_passed = candidate.quality_hard_gate_passed
        && readback.get("hard_gate_passed").and_then(Value::as_bool) == Some(true);
    let program_sha256 = evidence.geometry_program_sha256.clone();
    let program_object_sha256 = evidence.geometry_program_object_sha256.clone();
    let readback_object_sha256 = evidence.artifact_readback_object_sha256.clone();
    Ok(CandidateTruth {
        candidate,
        evidence,
        artifact_sha256,
        program_sha256,
        program_object_sha256,
        readback_object_sha256,
        representation_plan_sha256,
        hard_gate_passed,
    })
}

#[derive(Debug, Clone)]
struct VisualTruth {
    render_set_id: String,
    render_set_sha256: String,
    render_set_object_sha256: String,
    comparison_id: String,
    comparison_sha256: String,
    comparison_object_sha256: String,
    quality_id: String,
    quality_sha256: String,
    quality_object_sha256: String,
    camera_hash: String,
    camera_object_sha256: String,
    camera_id: Option<String>,
    reference_view_sha256: String,
    view_id: String,
    visual_status: String,
}

fn visual_truth(
    runtime: &Runtime,
    candidate_id: &str,
    project_id: &str,
    reference_id: &str,
    primary_view_id: &str,
    artifact_sha256: &str,
    program_sha256: &str,
) -> Result<VisualTruth, RuntimeError> {
    let (render_object_sha256, comparison_object_sha256, quality_object_sha256) =
        if let Some(row) = runtime.store.get_visual_evidence(candidate_id)? {
            if row.project_id != project_id || row.reference_id != reference_id {
                return Err(mismatch("visual evidence project/reference differs"));
            }
            let comparison = row
                .comparison_report_object_sha256
                .ok_or_else(|| invalid("visual evidence has no comparison report"))?;
            (
                row.render_set_object_sha256,
                comparison,
                row.quality_report_object_sha256,
            )
        } else {
            let row = runtime
                .store
                .list_visual_evidence_views(candidate_id)?
                .into_iter()
                .find(|row| {
                    row.project_id == project_id
                        && row.reference_id == reference_id
                        && row.view_id == primary_view_id
                        && row.comparison_report_object_sha256.is_some()
                })
                .ok_or_else(|| invalid("candidate has no exact fixed-view visual evidence"))?;
            (
                row.render_set_object_sha256,
                row.comparison_report_object_sha256.expect("checked above"),
                row.quality_report_object_sha256,
            )
        };
    let render = read_semantic_json(
        runtime,
        &render_object_sha256,
        MAX_LINEAGE_BYTES,
        "RenderSet",
    )?;
    super::validate_render_set_v2_output(&render.0)?;
    let render_set_id = require_string(&render.0, "render_set_id", "RenderSet")?;
    let render_set_sha256 = render.1;
    if render.0.get("candidate_id").and_then(Value::as_str) != Some(candidate_id)
        || render.0.get("artifact_sha256").and_then(Value::as_str) != Some(artifact_sha256)
        || render.0.get("program_sha256").and_then(Value::as_str) != Some(program_sha256)
        || render.0.get("reference_id").and_then(Value::as_str) != Some(reference_id)
        || render.0.get("view_id").and_then(Value::as_str) != Some(primary_view_id)
    {
        return Err(mismatch(
            "RenderSet is not bound to the exact attempt/fixed view",
        ));
    }
    let camera_hash = require_sha_value(&render.0, "camera_hash", "RenderSet")?;
    let camera_object_sha256 = require_sha_value(&render.0, "camera_object_sha256", "RenderSet")?;
    // CameraCalibration@1 is a Runtime contract whose semantic identity is
    // its `camera_hash`; unlike the other reports it need not carry the
    // generic `canonical_sha256` member in older persisted rows.  Read and
    // validate the canonical CAS bytes directly, then bind that identity.
    let camera = read_canonical_json(
        runtime,
        &camera_object_sha256,
        MAX_LINEAGE_BYTES,
        "CameraCalibration",
    )?;
    super::validate_camera_calibration(&camera)?;
    if camera.get("camera_hash").and_then(Value::as_str) != Some(camera_hash.as_str()) {
        return Err(mismatch("camera calibration differs from RenderSet"));
    }
    let comparison = read_semantic_json(
        runtime,
        &comparison_object_sha256,
        MAX_LINEAGE_BYTES,
        "ReferenceComparison",
    )?;
    super::validate_reference_comparison_report(&comparison.0)?;
    let comparison_id = require_string(&comparison.0, "report_id", "ReferenceComparison")?;
    let comparison_sha256 = comparison.1;
    if comparison.0.get("candidate_id").and_then(Value::as_str) != Some(candidate_id)
        || comparison.0.get("artifact_sha256").and_then(Value::as_str) != Some(artifact_sha256)
        || comparison.0.get("reference_id").and_then(Value::as_str) != Some(reference_id)
        || comparison.0.get("render_set_hash").and_then(Value::as_str)
            != Some(render_object_sha256.as_str())
        || comparison.0.get("camera_hash").and_then(Value::as_str) != Some(camera_hash.as_str())
        || comparison.0.get("view_id").and_then(Value::as_str) != Some(primary_view_id)
    {
        return Err(mismatch(
            "ReferenceComparison is not bound to RenderSet/reference/camera",
        ));
    }
    let reference_view_sha256 = comparison
        .0
        .pointer("/mask/sha256")
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .ok_or_else(|| invalid("ReferenceComparison mask identity is missing"))?
        .to_owned();
    let mask = runtime
        .store
        .get_object(&reference_view_sha256)?
        .ok_or_else(|| invalid("reference comparison mask CAS metadata is missing"))?;
    if mask.mime != "image/png"
        || sha256_hex(&runtime.cas_read_bounded(&reference_view_sha256, MAX_LINEAGE_BYTES)?)
            != reference_view_sha256
    {
        return Err(invalid("reference comparison mask is not hash-bound"));
    }
    let quality = read_semantic_json(
        runtime,
        &quality_object_sha256,
        MAX_LINEAGE_BYTES,
        "QualityReport",
    )?;
    super::validate_quality_report_v2_output(&quality.0)?;
    let quality_id = require_string(&quality.0, "quality_report_id", "QualityReport")?;
    let quality_sha256 = quality.1;
    if quality.0.get("candidate_id").and_then(Value::as_str) != Some(candidate_id)
        || quality.0.get("artifact_sha256").and_then(Value::as_str) != Some(artifact_sha256)
        || quality.0.get("program_sha256").and_then(Value::as_str) != Some(program_sha256)
        || quality.0.get("reference_id").and_then(Value::as_str) != Some(reference_id)
        || quality.0.get("render_set_hash").and_then(Value::as_str)
            != Some(render_object_sha256.as_str())
        || quality
            .0
            .get("comparison_report_hash")
            .and_then(Value::as_str)
            != Some(comparison_object_sha256.as_str())
        || quality.0.get("view_id").and_then(Value::as_str) != Some(primary_view_id)
    {
        return Err(mismatch(
            "QualityReport is not bound to the exact candidate/view evidence",
        ));
    }
    let visual_status = match quality.0.get("visual_status").and_then(Value::as_str) {
        Some("QUALITY_TARGET_NOT_MET") => "QUALITY_TARGET_NOT_MET",
        Some("BLOCKED_REFERENCE_COVERAGE") => "BLOCKED_REFERENCE_COVERAGE",
        Some("not-run") => "NOT_RUN",
        Some("PARTIAL_VISIBLE_VIEW_PASS") => {
            return Err(invalid("partial visual view cannot become a PassState"))
        }
        _ => {
            return Err(invalid(
                "QualityReport visual_status is not a conservative PassState status",
            ))
        }
    };
    Ok(VisualTruth {
        render_set_id,
        render_set_sha256,
        render_set_object_sha256: render_object_sha256,
        comparison_id,
        comparison_sha256,
        comparison_object_sha256,
        quality_id,
        quality_sha256,
        quality_object_sha256,
        camera_hash,
        camera_object_sha256,
        camera_id: camera
            .get("camera_id")
            .and_then(Value::as_str)
            .map(str::to_owned),
        reference_view_sha256,
        view_id: primary_view_id.to_owned(),
        visual_status: visual_status.to_owned(),
    })
}

fn fixed_view_from_intent(intent: &Value) -> Result<(String, String), RuntimeError> {
    let fixed_views = intent
        .pointer("/quality_contract/fixed_views")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("intent quality_contract.fixed_views is missing"))?;
    let fixed = fixed_views
        .iter()
        .find(|view| {
            view.get("comparison_role").and_then(Value::as_str) == Some("primary-reference")
                && view.get("reference_required").and_then(Value::as_bool) == Some(true)
        })
        .ok_or_else(|| invalid("intent has no primary-reference fixed view"))?;
    let view_id = require_string(fixed, "view_id", "intent.fixed_view")?;
    let view_kind = fixed
        .get("view")
        .and_then(Value::as_str)
        .filter(|value| {
            matches!(
                *value,
                "front"
                    | "back"
                    | "left"
                    | "right"
                    | "front-three-quarter"
                    | "rear-three-quarter"
                    | "top"
                    | "bottom"
                    | "fps-inspect"
            )
        })
        .ok_or_else(|| invalid("intent primary fixed view kind is invalid"))?;
    Ok((view_id, view_kind.to_owned()))
}

fn unknowns_from_brief(brief: &Value, intent: &Value) -> Result<Vec<Value>, RuntimeError> {
    let missing = brief
        .pointer("/reference_coverage/missing_views")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("brief reference_coverage.missing_views is missing"))?;
    let mut unknowns = Vec::new();
    for view in missing {
        let view = view
            .as_str()
            .ok_or_else(|| invalid("brief missing view is not a string"))?;
        let description = match view {
            "front-three-quarter" => "A front three-quarter authorized reference is required before multi-view silhouette acceptance.",
            "rear-three-quarter" => "A rear three-quarter authorized reference is required before multi-view silhouette acceptance.",
            "top" => "A top authorized reference is required before proportion acceptance.",
            "bottom" => "A bottom authorized reference is required before proportion acceptance.",
            "fps-inspect" => "An FPS inspect authorized reference is required before first-person presentation acceptance.",
            "front" | "back" | "left" | "right" => "An authorized primary reference view is required before silhouette acceptance.",
            _ => return Err(invalid(format!("brief missing view {view} cannot be represented by KnifePassState"))),
        };
        unknowns.push(json!({
            "unknown_id": format!("missing-{view}-reference"),
            "category": "reference-coverage",
            "view_kind": view,
            "description": description,
            "impact": "blocking",
            "status": "open"
        }));
    }
    if unknowns.is_empty() {
        let view_kind = fixed_view_from_intent(intent)?.1;
        unknowns.push(json!({
            "unknown_id": "pass-state-promotion-locked",
            "category": "lineage",
            "view_kind": view_kind,
            "description": "Promotion remains locked until Runtime-owned structural and review evidence is complete.",
            "impact": "blocking",
            "status": "open"
        }));
    }
    Ok(unknowns)
}

fn source_record(
    runtime: &Runtime,
    main: &Map<String, Value>,
) -> Result<KnifeSourceBindingStoreRecord, RuntimeError> {
    let project_id = scalar_identity(main, "project_id")?;
    let source_binding_id = scalar_identity(main, "source_binding_id")?;
    let source_binding_sha256 = scalar_hash(main, "source_binding_sha256")?;
    let record = runtime
        .store
        .get_knife_source_binding(&project_id, &source_binding_id, &source_binding_sha256)?
        .ok_or_else(|| invalid("exact SourceBinding is not durable"))?;
    let expected = [
        (
            "source_binding_object_sha256",
            record.source_binding_object_sha256.as_str(),
        ),
        ("intent_bundle_id", record.intent_bundle_id.as_str()),
        ("intent_bundle_sha256", record.intent_bundle_sha256.as_str()),
        (
            "intent_bundle_object_sha256",
            record.intent_bundle_object_sha256.as_str(),
        ),
        ("brief_id", record.brief_id.as_str()),
        ("brief_sha256", record.brief_sha256.as_str()),
        ("brief_object_sha256", record.brief_object_sha256.as_str()),
        ("reference_id", record.reference_id.as_str()),
        (
            "reference_object_sha256",
            record.reference_object_sha256.as_str(),
        ),
        (
            "reference_evidence_sha256",
            record.reference_evidence_sha256.as_str(),
        ),
        ("source_candidate_id", record.source_candidate_id.as_str()),
        (
            "source_candidate_state_sha256",
            record.source_candidate_state_sha256.as_str(),
        ),
    ];
    if record.project_id != project_id
        || record.source_binding_id != source_binding_id
        || record.source_binding_sha256 != source_binding_sha256
    {
        return Err(mismatch("SourceBinding primary identity differs"));
    }
    for (key, expected) in expected {
        if main.get(key).and_then(Value::as_str) != Some(expected) {
            return Err(mismatch(format!(
                "pass_state.{key} differs from SourceBinding"
            )));
        }
    }
    let source_value = runtime
        .store
        .read_knife_source_binding_json(&project_id, &source_binding_id, &source_binding_sha256)?
        .ok_or_else(|| invalid("SourceBinding CAS readback is missing"))?;
    let source_object = exact_object(
        &source_value,
        &[
            "schema_version",
            "source_binding_id",
            "project_id",
            "binding_status",
            "authoring_eligibility",
            "intent_bundle_id",
            "intent_bundle_sha256",
            "intent_bundle_object_sha256",
            "brief_id",
            "brief_sha256",
            "brief_object_sha256",
            "reference_id",
            "reference_object_sha256",
            "reference_evidence_sha256",
            "quality_contract_id",
            "quality_contract_sha256",
            "quality_contract_object_sha256",
            "source_candidate_id",
            "source_candidate_state_sha256",
            "authoring_mesh_id",
            "authoring_mesh_lineage_id",
            "authoring_mesh_revision_id",
            "authoring_mesh_revision_index",
            "authoring_mesh_revision_sha256",
            "authoring_mesh_revision_object_sha256",
            "authoring_mesh_identity_sha256",
            "downstream_binding_requirements",
            "high_mesh_created",
            "high_stage_unlocked",
            "production_stage_advanced",
            "candidate_confirmed",
            "version_created",
            "export_performed",
            "quality_status",
            "visual_status",
            "human_status",
            "engine_status",
            "binding_policy",
            "canonicalization_policy",
            "canonical_sha256",
            "created_at",
        ],
        "source_binding",
    )?;
    if source_object
        .get("canonical_sha256")
        .and_then(Value::as_str)
        != Some(source_binding_sha256.as_str())
    {
        return Err(mismatch("SourceBinding CAS semantic hash differs"));
    }
    Ok(record)
}

fn load_revision(
    runtime: &Runtime,
    project_id: &str,
    revision_id: &str,
) -> Result<
    (
        forgecad_store::AuthoringMeshV2DurableRecord,
        AuthoringMeshRevision,
    ),
    RuntimeError,
> {
    let durable = runtime
        .store
        .get_authoring_mesh_v2_durable_record_by_revision(project_id, revision_id)?
        .ok_or_else(|| invalid("requested AuthoringMesh revision is not durable"))?;
    let revision = authoring_mesh_v2_durable::revision_from_cas(runtime, &durable)?;
    if revision.schema_version != "AuthoringMeshRevision@2"
        || revision.revision_id.0 != revision_id
        || revision.mesh_id.0 != durable.mesh_id
        || revision.lineage_id.0 != durable.lineage_id
        || revision.canonical_sha256 != durable.revision_sha256
    {
        return Err(mismatch(
            "AuthoringMesh durable row and CAS revision differ",
        ));
    }
    Ok((durable, revision))
}

fn ensure_descendant(
    runtime: &Runtime,
    project_id: &str,
    anchor: &forgecad_store::AuthoringMeshV2DurableRecord,
    selected: &forgecad_store::AuthoringMeshV2DurableRecord,
) -> Result<(), RuntimeError> {
    if anchor.revision_id == selected.revision_id {
        return Ok(());
    }
    if anchor.mesh_id != selected.mesh_id
        || anchor.lineage_id != selected.lineage_id
        || selected.revision_index < anchor.revision_index
    {
        return Err(mismatch(
            "selected AuthoringMesh revision is outside the SourceBinding lineage",
        ));
    }
    let mut pending = vec![selected.clone()];
    let mut visited = HashSet::new();
    for _ in 0..64 {
        let Some(current) = pending.pop() else { break };
        if !visited.insert(current.revision_id.clone()) {
            continue;
        }
        for parent_id in &current.parent_revision_ids {
            let parent = load_revision(runtime, project_id, parent_id.as_str())?.0;
            if parent.mesh_id != anchor.mesh_id
                || parent.lineage_id != anchor.lineage_id
                || parent.revision_index >= current.revision_index
            {
                return Err(mismatch("AuthoringMesh parent DAG is not monotonic"));
            }
            if parent.revision_id == anchor.revision_id {
                return Ok(());
            }
            pending.push(parent);
        }
    }
    Err(mismatch(
        "selected AuthoringMesh revision is not a descendant of SourceBinding anchor",
    ))
}

fn stage_rank(stage: &str) -> Option<u8> {
    match stage {
        "camera-lock" => Some(0),
        "silhouette-blockout" => Some(1),
        "structural-form" => Some(2),
        "secondary-form" => Some(3),
        "high-geometry" => Some(4),
        _ => None,
    }
}

fn validate_parent_successor(
    parent: &KnifePassStateStoreRecord,
    child_stage: &str,
    selected: &forgecad_store::AuthoringMeshV2DurableRecord,
    baseline: &CandidateTruth,
    attempt: &CandidateTruth,
    fixed_view: &Value,
    camera_set_sha256: &str,
) -> Result<(), RuntimeError> {
    let parent_rank =
        stage_rank(parent.stage.as_str()).ok_or_else(|| invalid("parent stage is invalid"))?;
    let child_rank = stage_rank(child_stage).ok_or_else(|| invalid("child stage is invalid"))?;
    if child_rank < parent_rank || child_rank > parent_rank.saturating_add(1) {
        return Err(mismatch(
            "correction stage must preserve or advance the bounded stage sequence by one",
        ));
    }
    if fixed_view != &parent.fixed_view || camera_set_sha256 != parent.camera_set_sha256 {
        return Err(mismatch(
            "correction must preserve the exact fixed view and camera set",
        ));
    }
    if selected.revision_index <= parent.authoring_mesh_revision_index
        || selected.parent_revision_ids.len() != 1
        || selected.parent_revision_ids[0] != parent.authoring_mesh_revision_id
    {
        return Err(mismatch(
            "correction AuthoringMesh revision must directly descend from the parent pass revision",
        ));
    }
    let inherited = [
        (
            "candidate_id",
            parent.attempt_candidate_id.as_str(),
            baseline.candidate.candidate_id.as_str(),
        ),
        (
            "candidate_state_sha256",
            parent.attempt_candidate_state_sha256.as_str(),
            baseline.candidate.canonical_sha256.as_str(),
        ),
        (
            "artifact_sha256",
            parent.attempt_artifact_sha256.as_str(),
            baseline.artifact_sha256.as_str(),
        ),
        (
            "geometry_program_sha256",
            parent.attempt_geometry_program_sha256.as_str(),
            baseline.program_sha256.as_str(),
        ),
        (
            "geometry_program_object_sha256",
            parent.attempt_geometry_program_object_sha256.as_str(),
            baseline.program_object_sha256.as_str(),
        ),
        (
            "artifact_readback_object_sha256",
            parent.attempt_artifact_readback_object_sha256.as_str(),
            baseline.readback_object_sha256.as_str(),
        ),
        (
            "representation_plan_sha256",
            parent.attempt_representation_plan_sha256.as_str(),
            baseline.representation_plan_sha256.as_str(),
        ),
    ];
    if let Some((field, _, _)) = inherited
        .iter()
        .find(|(_, expected, actual)| expected != actual)
    {
        return Err(mismatch(format!(
            "correction baseline differs from parent attempt {field}"
        )));
    }
    if attempt.candidate.candidate_id == baseline.candidate.candidate_id {
        return Err(mismatch("correction attempt candidate must be new"));
    }
    if attempt.program_sha256 == baseline.program_sha256
        && attempt.program_object_sha256 == baseline.program_object_sha256
        && attempt.representation_plan_sha256 == baseline.representation_plan_sha256
    {
        return Err(mismatch(
            "correction attempt must change GeometryProgram or representation plan",
        ));
    }
    Ok(())
}

/// Re-prove that the attempt candidate is the source-bound materialization of
/// the exact AMV2 revision selected by this pass.  Candidate IDs and their
/// state hashes are not sufficient evidence: a caller could otherwise pair a
/// valid candidate with a different revision while retaining a plausible
/// PassState shape.  The materializer plan is reconstructed from the durable
/// source candidate, embedded SourceBinding and selected revision, then the
/// stored GeometryProgram is checked against that plan and replacement node.
fn validate_attempt_materialization_binding(
    runtime: &Runtime,
    project_id: &str,
    source: &KnifeSourceBindingStoreRecord,
    selected: &forgecad_store::AuthoringMeshV2DurableRecord,
    revision: &AuthoringMeshRevision,
    attempt: &CandidateTruth,
) -> Result<(), RuntimeError> {
    let binding = revision
        .source_binding
        .as_ref()
        .ok_or_else(|| mismatch("attempt revision has no embedded SourceBinding"))?;
    super::authoring_mesh_v2::validate_source_binding(binding)?;
    let source_truth = candidate_truth(
        runtime,
        &source.source_candidate_id,
        project_id,
        &source.reference_id,
    )?;
    let (_source_readback, source_readback_sha256) = read_semantic_json(
        runtime,
        &source_truth.readback_object_sha256,
        8 * 1024 * 1024,
        "source ArtifactReadback",
    )?;
    if source_readback_sha256 != binding.artifact_readback_sha256
        || source_truth.artifact_sha256 != binding.artifact_sha256
        || source_truth.program_sha256 != binding.geometry_program_sha256
        || binding.project_id != *project_id
        || binding.candidate_id != source.source_candidate_id
        || binding.candidate_state_sha256 != source.source_candidate_state_sha256
    {
        return Err(mismatch(
            "attempt revision embedded SourceBinding differs from the durable source candidate",
        ));
    }
    let parameters = super::authoring_mesh_v2_geometry::authoring_mesh_v2_geometry_parameters(
        revision,
        binding.position_m,
        binding.rotation_rad,
    )?;
    let projection_sha256 =
        super::authoring_mesh_v2_geometry::authoring_mesh_v2_geometry_projection_sha256(
            revision,
            &parameters,
        );
    let replacement_identity = json!({
        "schema_version":"AuthoringMeshV2CandidateReplacementIdentity@1",
        "project_id":project_id,
        "mesh_id":selected.mesh_id,
        "lineage_id":selected.lineage_id,
        "materialization_mode":"source_binding_part_replacement",
        "revision_id":selected.revision_id,
        "revision_sha256":selected.revision_sha256,
        "revision_object_sha256":selected.revision_object_sha256,
        "projection_sha256":projection_sha256,
        "source_binding_id":source.source_binding_id,
        "source_binding_sha256":source.source_binding_sha256,
        "source_node_id":binding.source_node_id,
        "source_part_id":binding.part_id,
    });
    let replacement_node_id = format!(
        "authoring-mesh-v2-{}",
        &canonical_json_hash(&replacement_identity)[..32]
    );
    let plan = json!({
        "schema_version":"AuthoringMeshV2CandidateMaterializationRepresentationPlan@1",
        "project_id":project_id,
        "mesh_id":selected.mesh_id,
        "lineage_id":selected.lineage_id,
        "materialization_mode":"source_binding_part_replacement",
        "revision_id":selected.revision_id,
        "revision_index":selected.revision_index,
        "revision_sha256":selected.revision_sha256,
        "revision_object_sha256":selected.revision_object_sha256,
        "replacement_revision_id":selected.revision_id,
        "replacement_revision_sha256":selected.revision_sha256,
        "replacement_revision_object_sha256":selected.revision_object_sha256,
        "replacement_projection_sha256":projection_sha256,
        "replacement_node_id":replacement_node_id,
        "source_candidate_id":source.source_candidate_id,
        "source_candidate_state_sha256":source.source_candidate_state_sha256,
        "source_artifact_sha256":binding.artifact_sha256,
        "source_artifact_readback_sha256":source_readback_sha256,
        "source_program_sha256":binding.geometry_program_sha256,
        "source_program_object_sha256":source_truth.program_object_sha256,
        "source_binding_id":source.source_binding_id,
        "source_binding_sha256":source.source_binding_sha256,
        "source_binding_object_sha256":source.source_binding_object_sha256,
        "source_node_id":binding.source_node_id,
        "source_part_id":binding.part_id,
        "source_material_zone_id":binding.material_zone_id,
        "source_solid":binding.solid,
        "source_part_output_sha256":binding.part_output_sha256,
    });
    let expected_plan_sha256 = canonical_json_hash(&plan);
    if attempt.representation_plan_sha256 != expected_plan_sha256 {
        return Err(mismatch(
            "attempt representation plan is not derived from the selected AMV2 revision",
        ));
    }
    let program = read_canonical_json(
        runtime,
        &attempt.program_object_sha256,
        MAX_LINEAGE_BYTES,
        "attempt GeometryProgram",
    )?;
    if program
        .get("representation_plan_sha256")
        .and_then(Value::as_str)
        != Some(expected_plan_sha256.as_str())
    {
        return Err(mismatch(
            "attempt GeometryProgram plan is not bound to the selected AMV2 revision",
        ));
    }
    let node = program
        .get("nodes")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .find(|node| {
            node.get("node_id").and_then(Value::as_str) == Some(replacement_node_id.as_str())
        })
        .ok_or_else(|| {
            mismatch("attempt GeometryProgram replacement node is not the selected revision")
        })?;
    if node.get("operator_id").and_then(Value::as_str) != Some("forgecad.geometry.authoring-mesh@1")
        || node
            .get("inputs")
            .and_then(Value::as_array)
            .is_none_or(|inputs| !inputs.is_empty())
        || node.get("parameters") != Some(&parameters)
    {
        return Err(mismatch(
            "attempt GeometryProgram replacement node parameters drifted",
        ));
    }
    let output = program
        .get("part_outputs")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .find(|output| {
            output.get("part_id").and_then(Value::as_str) == Some(binding.part_id.as_str())
        })
        .ok_or_else(|| mismatch("attempt GeometryProgram source Part output is missing"))?;
    let expected_input_node_ids = json!([replacement_node_id]);
    if output.get("input_node_ids") != Some(&expected_input_node_ids)
        || output.get("material_zone_id").and_then(Value::as_str)
            != Some(binding.material_zone_id.as_str())
        || output.get("solid").and_then(Value::as_bool) != Some(binding.solid)
    {
        return Err(mismatch(
            "attempt GeometryProgram source Part output is not bound to SourceBinding",
        ));
    }
    Ok(())
}

fn authoring_mesh_truth(
    runtime: &Runtime,
    main: &Map<String, Value>,
    source: &KnifeSourceBindingStoreRecord,
) -> Result<
    (
        forgecad_store::AuthoringMeshV2DurableRecord,
        AuthoringMeshRevision,
    ),
    RuntimeError,
> {
    let project_id = scalar_identity(main, "project_id")?;
    let revision_id = scalar_identity(main, "authoring_mesh_revision_id")?;
    let (selected, revision) = load_revision(runtime, &project_id, &revision_id)?;
    let (anchor, anchor_revision) =
        load_revision(runtime, &project_id, &source.authoring_mesh_revision_id)?;
    let anchor_binding = anchor_revision
        .source_binding
        .as_ref()
        .ok_or_else(|| mismatch("SourceBinding anchor has no embedded source binding"))?;
    let selected_binding = revision
        .source_binding
        .as_ref()
        .ok_or_else(|| mismatch("selected AuthoringMesh revision has no embedded SourceBinding"))?;
    super::authoring_mesh_v2::validate_source_binding(anchor_binding)?;
    super::authoring_mesh_v2::validate_source_binding(selected_binding)?;
    if anchor.mesh_id != source.authoring_mesh_id
        || anchor.lineage_id != source.authoring_mesh_lineage_id
        || anchor.revision_index != source.authoring_mesh_revision_index
        || anchor.revision_sha256 != source.authoring_mesh_revision_sha256
        || anchor.revision_object_sha256 != source.authoring_mesh_revision_object_sha256
        || anchor_binding != selected_binding
    {
        return Err(mismatch(
            "selected AuthoringMesh revision does not preserve the SourceBinding anchor",
        ));
    }
    ensure_descendant(runtime, &project_id, &anchor, &selected)?;
    let expected = [
        ("authoring_mesh_id", selected.mesh_id.as_str()),
        ("authoring_mesh_lineage_id", selected.lineage_id.as_str()),
        ("authoring_mesh_revision_id", selected.revision_id.as_str()),
        (
            "authoring_mesh_revision_sha256",
            selected.revision_sha256.as_str(),
        ),
        (
            "authoring_mesh_revision_object_sha256",
            selected.revision_object_sha256.as_str(),
        ),
        ("authoring_mesh_sha256", revision.canonical_sha256.as_str()),
    ];
    for (key, expected) in expected {
        if main.get(key).and_then(Value::as_str) != Some(expected) {
            return Err(mismatch(format!(
                "pass_state.{key} differs from the selected AuthoringMesh revision"
            )));
        }
    }
    if main
        .get("authoring_mesh_revision_index")
        .and_then(Value::as_u64)
        != Some(selected.revision_index)
        || main
            .get("authoring_mesh_identity_sha256")
            .and_then(Value::as_str)
            != Some(source.authoring_mesh_identity_sha256.as_str())
    {
        return Err(mismatch(
            "pass_state AuthoringMesh revision index/identity differs",
        ));
    }
    Ok((selected, revision))
}

fn parent_binding(
    runtime: &Runtime,
    main: &Map<String, Value>,
    source: &KnifeSourceBindingStoreRecord,
) -> Result<
    (
        Option<String>,
        Option<String>,
        Option<KnifePassStateStoreRecord>,
    ),
    RuntimeError,
> {
    let parent_id = nullable_identity(main, "parent_pass_id")?;
    let parent_sha = nullable_hash(main, "parent_pass_sha256")?;
    if parent_id.is_none() && parent_sha.is_none() {
        return Ok((None, None, None));
    }
    let (Some(parent_id), Some(parent_sha)) = (parent_id, parent_sha) else {
        return Err(invalid(
            "parent pass identity must be all-null or all-present",
        ));
    };
    let parent = runtime
        .store
        .get_knife_pass_state(&source.project_id, &parent_id, &parent_sha)?
        .ok_or_else(|| invalid("parent pass state is not durable"))?;
    if parent.source_binding_id != source.source_binding_id
        || parent.source_binding_sha256 != source.source_binding_sha256
        || parent.authoring_mesh_lineage_id != source.authoring_mesh_lineage_id
    {
        return Err(mismatch(
            "parent pass state does not share the source lineage",
        ));
    }
    Ok((Some(parent_id), Some(parent_sha), Some(parent)))
}

fn validate_root_selection(
    selected_revision_id: &str,
    source: &KnifeSourceBindingStoreRecord,
    baseline: &CandidateTruth,
) -> Result<(), RuntimeError> {
    if selected_revision_id != source.authoring_mesh_revision_id {
        return Err(mismatch(
            "root pass must select the exact SourceBinding anchor revision",
        ));
    }
    if baseline.candidate.candidate_id != source.source_candidate_id
        || baseline.candidate.canonical_sha256 != source.source_candidate_state_sha256
    {
        return Err(mismatch(
            "root pass baseline must be the exact SourceBinding source candidate",
        ));
    }
    Ok(())
}

fn derive_main(runtime: &Runtime, proposal: &Value) -> Result<Value, RuntimeError> {
    let input = main_value_is_closed(proposal, "pass_state")?;
    let project_id = scalar_identity(input, "project_id")?;
    let pass_id = scalar_identity(input, "pass_id")?;
    let created_at = text(input, "created_at", "pass_state")?.to_owned();
    if created_at.len() != 20
        || !created_at.ends_with('Z')
        || created_at.contains('/')
        || created_at.chars().any(|c| c.is_control())
    {
        return Err(invalid(
            "pass_state.created_at must be a bounded RFC3339 UTC timestamp",
        ));
    }
    let source = source_record(runtime, input)?;
    let intent_record = runtime
        .store
        .get_knife_reference_intent_bundle_exact(
            &project_id,
            &source.brief_id,
            &source.brief_sha256,
            &source.brief_object_sha256,
            &source.reference_id,
            &source.reference_object_sha256,
            &source.reference_evidence_sha256,
            &source.intent_bundle_id,
            &source.intent_bundle_sha256,
            &source.intent_bundle_object_sha256,
        )?
        .ok_or_else(|| invalid("exact ReferenceIntent bundle is not durable"))?;
    let intent = runtime
        .store
        .read_knife_reference_intent_bundle_json(
            &project_id,
            &source.brief_id,
            &source.intent_bundle_id,
            &source.intent_bundle_sha256,
        )?
        .ok_or_else(|| invalid("ReferenceIntent CAS readback is missing"))?;
    if intent_record.intent_bundle_object_sha256 != source.intent_bundle_object_sha256 {
        return Err(mismatch("ReferenceIntent object binding differs"));
    }
    let brief_record = runtime
        .store
        .get_weaponry_knife_production_brief_exact(
            &project_id,
            &source.reference_id,
            &source.reference_object_sha256,
            &source.reference_evidence_sha256,
            &source.brief_id,
            &source.brief_sha256,
            &source.brief_object_sha256,
        )?
        .ok_or_else(|| invalid("exact ProductionBrief is not durable"))?;
    let brief = runtime
        .store
        .read_weaponry_knife_production_brief_json(
            &project_id,
            &source.brief_id,
            &source.brief_sha256,
        )?
        .ok_or_else(|| invalid("ProductionBrief CAS readback is missing"))?;
    if brief_record.brief_object_sha256 != source.brief_object_sha256 {
        return Err(mismatch("ProductionBrief object binding differs"));
    }
    let reference = runtime
        .reference(&source.reference_id)?
        .ok_or_else(|| invalid("reference evidence is not durable"))?;
    if reference.project_id != project_id
        || reference.object_sha256 != source.reference_object_sha256
        || reference.canonical_sha256 != source.reference_evidence_sha256
    {
        return Err(mismatch("ReferenceEvidence differs from SourceBinding"));
    }
    let source_candidate = runtime
        .candidate(&source.source_candidate_id)?
        .ok_or_else(|| invalid("source candidate is not durable"))?;
    if source_candidate.project_id != project_id
        || source_candidate.canonical_sha256 != source.source_candidate_state_sha256
        || source_candidate.quality_hard_gate_passed == false
    {
        return Err(mismatch("source candidate differs from SourceBinding"));
    }
    let (primary_view_id, view_kind) = fixed_view_from_intent(&intent)?;
    let (selected_revision, revision) = authoring_mesh_truth(runtime, input, &source)?;
    let baseline_id = scalar_identity(input, "baseline_candidate_id")?;
    let attempt_id = scalar_identity(input, "attempt_candidate_id")?;
    let baseline = candidate_truth(runtime, &baseline_id, &project_id, &source.reference_id)?;
    let attempt = candidate_truth(runtime, &attempt_id, &project_id, &source.reference_id)?;
    let visual = visual_truth(
        runtime,
        &attempt_id,
        &project_id,
        &source.reference_id,
        &primary_view_id,
        &attempt.artifact_sha256,
        &attempt.program_sha256,
    )?;
    let (parent_id, parent_sha, parent) = parent_binding(runtime, input, &source)?;
    if parent.is_none() {
        validate_root_selection(selected_revision.revision_id.as_str(), &source, &baseline)?;
    }
    validate_attempt_materialization_binding(
        runtime,
        &project_id,
        &source,
        &selected_revision,
        &revision,
        &attempt,
    )?;
    let stage = match parent.as_ref() {
        None => {
            if input.get("stage").and_then(Value::as_str) != Some(STAGE) {
                return Err(mismatch("root pass must start at camera-lock"));
            }
            STAGE.to_owned()
        }
        Some(_parent) => input
            .get("stage")
            .and_then(Value::as_str)
            .filter(|stage| stage_rank(stage).is_some())
            .ok_or_else(|| invalid("correction stage is invalid"))?
            .to_owned(),
    };
    let camera_id = visual
        .camera_id
        .clone()
        .unwrap_or_else(|| format!("knife-camera-{view_kind}-{}", &visual.camera_hash[..16]));
    let fixed_view = json!({
        "view_id": visual.view_id,
        "view_kind": view_kind,
        "comparison_role": "primary-reference",
        "reference_required": true,
        "camera_id": camera_id,
        "camera_sha256": visual.camera_hash,
        "reference_view_id": visual.view_id,
        "reference_view_sha256": visual.reference_view_sha256,
        "fixed_view_policy": FIXED_VIEW_POLICY
    });
    let camera_set = json!({"schema_version":"KnifeCameraSet@1","fixed_views":[fixed_view.clone()],"fixed_view_count":1});
    let camera_set_sha256 = canonical_json_hash(&camera_set);
    let unknowns = unknowns_from_brief(&brief, &intent)?;
    let hard_gate_status = if attempt.hard_gate_passed {
        "PASS_SOURCE_STRUCTURAL"
    } else {
        "BLOCKED"
    };
    let mut main = json!({
        "schema_version": MAIN_SCHEMA,
        "pass_id": pass_id,
        "parent_pass_id": parent_id,
        "parent_pass_sha256": parent_sha,
        "project_id": project_id,
        "stage": stage,
        "source_binding_id": source.source_binding_id,
        "source_binding_sha256": source.source_binding_sha256,
        "source_binding_object_sha256": source.source_binding_object_sha256,
        "intent_bundle_id": source.intent_bundle_id,
        "intent_bundle_sha256": source.intent_bundle_sha256,
        "intent_bundle_object_sha256": source.intent_bundle_object_sha256,
        "brief_id": source.brief_id,
        "brief_sha256": source.brief_sha256,
        "brief_object_sha256": source.brief_object_sha256,
        "reference_id": source.reference_id,
        "reference_object_sha256": source.reference_object_sha256,
        "reference_evidence_sha256": source.reference_evidence_sha256,
        "source_candidate_id": source.source_candidate_id,
        "source_candidate_state_sha256": source.source_candidate_state_sha256,
        "baseline_candidate_id": baseline.candidate.candidate_id,
        "baseline_candidate_state_sha256": baseline.candidate.canonical_sha256,
        "baseline_artifact_sha256": baseline.artifact_sha256,
        "baseline_geometry_program_sha256": baseline.program_sha256,
        "baseline_geometry_program_object_sha256": baseline.program_object_sha256,
        "baseline_artifact_readback_object_sha256": baseline.readback_object_sha256,
        "baseline_representation_plan_sha256": baseline.representation_plan_sha256,
        "attempt_candidate_id": attempt.candidate.candidate_id,
        "attempt_candidate_state_sha256": attempt.candidate.canonical_sha256,
        "attempt_artifact_sha256": attempt.artifact_sha256,
        "attempt_geometry_program_sha256": attempt.program_sha256,
        "attempt_geometry_program_object_sha256": attempt.program_object_sha256,
        "attempt_artifact_readback_object_sha256": attempt.readback_object_sha256,
        "attempt_representation_plan_sha256": attempt.representation_plan_sha256,
        "authoring_mesh_id": selected_revision.mesh_id,
        "authoring_mesh_lineage_id": selected_revision.lineage_id,
        "authoring_mesh_revision_id": selected_revision.revision_id,
        "authoring_mesh_revision_index": selected_revision.revision_index,
        "authoring_mesh_revision_sha256": selected_revision.revision_sha256,
        "authoring_mesh_revision_object_sha256": selected_revision.revision_object_sha256,
        "authoring_mesh_identity_sha256": source.authoring_mesh_identity_sha256,
        "authoring_mesh_sha256": revision.canonical_sha256,
        "modifier_graph_id": null,
        "modifier_graph_sha256": null,
        "evaluated_mesh_id": null,
        "evaluated_mesh_sha256": null,
        "high_artifact_id": null,
        "high_artifact_sha256": null,
        "fixed_view": fixed_view,
        "camera_set_sha256": camera_set_sha256,
        "render_set_id": visual.render_set_id,
        "render_set_sha256": visual.render_set_sha256,
        "render_set_object_sha256": visual.render_set_object_sha256,
        "reference_comparison_id": visual.comparison_id,
        "reference_comparison_sha256": visual.comparison_sha256,
        "reference_comparison_object_sha256": visual.comparison_object_sha256,
        "quality_report_id": visual.quality_id,
        "quality_report_sha256": visual.quality_sha256,
        "quality_report_object_sha256": visual.quality_object_sha256,
        "evidence_bundle_sha256": "",
        "hard_gate_status": hard_gate_status,
        "visual_gate_status": visual.visual_status,
        "quality_status": visual.visual_status,
        "high_status": "NOT_RUN",
        "human_status": "NOT_RUN",
        "engine_status": "NOT_RUN",
        "unknowns": unknowns,
        "unlocked_successor": "none",
        "high_mesh_created": false,
        "high_stage_unlocked": false,
        "production_stage_advanced": false,
        "candidate_confirmed": false,
        "version_created": false,
        "export_performed": false,
        "canonicalization_policy": MAIN_CANONICALIZATION,
        "canonical_sha256": "",
        "created_at": created_at,
    });
    let evidence_bundle = json!({
        "schema_version": EVIDENCE_BUNDLE_SCHEMA,
        "render_set_sha256": visual.render_set_sha256,
        "reference_comparison_sha256": visual.comparison_sha256,
        "quality_report_sha256": visual.quality_sha256,
        "camera_set_sha256": camera_set_sha256,
    });
    main["evidence_bundle_sha256"] = Value::String(canonical_json_hash(&evidence_bundle));
    main["canonical_sha256"] = Value::String(canonical_json_hash(&main));
    if let Some(parent) = parent.as_ref() {
        validate_parent_successor(
            parent,
            main["stage"].as_str().unwrap_or_default(),
            &selected_revision,
            &baseline,
            &attempt,
            &main["fixed_view"],
            main["camera_set_sha256"].as_str().unwrap_or_default(),
        )?;
    }
    Ok(main)
}

fn validate_derived(proposal: &Value, derived: &Value) -> Result<(), RuntimeError> {
    main_value_is_closed(proposal, "pass_state")?;
    main_value_is_closed(derived, "derived pass_state")?;
    if proposal != derived {
        return Err(mismatch(
            "caller pass_state is not the Runtime-derived truth",
        ));
    }
    Ok(())
}

fn result_from_record(
    runtime: &Runtime,
    record: &KnifePassStateStoreRecord,
    operation: &str,
    request_kind: &str,
    status: &str,
    idempotency_key: Option<&str>,
    replayed: bool,
    effects: bool,
) -> Result<Value, RuntimeError> {
    let main = forgecad_store::knife_pass_state_main_value(record)?;
    // Own the projection before constructing the result: `pass_state` moves
    // the Main value into the closed result, so borrowing its map here would
    // make the construction order dependent on Rust's move analysis.
    let main_object = main
        .as_object()
        .cloned()
        .ok_or_else(|| invalid("Store Main readback is not an object"))?;
    let mut result = json!({
        "schema_version": RESULT_SCHEMA,
        "operation": operation,
        "request_kind": request_kind,
        "status": status,
        "project_id": main_object["project_id"],
        "pass_id": main_object["pass_id"],
        "pass_state_sha256": main_object["canonical_sha256"],
        "pass_state_object_sha256": record.pass_state_object_sha256,
        "pass_state": main,
        "source_binding_id": main_object["source_binding_id"],
        "source_binding_sha256": main_object["source_binding_sha256"],
        "source_binding_object_sha256": main_object["source_binding_object_sha256"],
        "intent_bundle_id": main_object["intent_bundle_id"],
        "intent_bundle_sha256": main_object["intent_bundle_sha256"],
        "intent_bundle_object_sha256": main_object["intent_bundle_object_sha256"],
        "brief_id": main_object["brief_id"],
        "brief_sha256": main_object["brief_sha256"],
        "brief_object_sha256": main_object["brief_object_sha256"],
        "reference_id": main_object["reference_id"],
        "reference_object_sha256": main_object["reference_object_sha256"],
        "reference_evidence_sha256": main_object["reference_evidence_sha256"],
        "source_candidate_id": main_object["source_candidate_id"],
        "source_candidate_state_sha256": main_object["source_candidate_state_sha256"],
        "baseline_candidate_id": main_object["baseline_candidate_id"],
        "baseline_candidate_state_sha256": main_object["baseline_candidate_state_sha256"],
        "baseline_artifact_sha256": main_object["baseline_artifact_sha256"],
        "baseline_geometry_program_sha256": main_object["baseline_geometry_program_sha256"],
        "baseline_geometry_program_object_sha256": main_object["baseline_geometry_program_object_sha256"],
        "baseline_artifact_readback_object_sha256": main_object["baseline_artifact_readback_object_sha256"],
        "baseline_representation_plan_sha256": main_object["baseline_representation_plan_sha256"],
        "attempt_candidate_id": main_object["attempt_candidate_id"],
        "attempt_candidate_state_sha256": main_object["attempt_candidate_state_sha256"],
        "attempt_artifact_sha256": main_object["attempt_artifact_sha256"],
        "attempt_geometry_program_sha256": main_object["attempt_geometry_program_sha256"],
        "attempt_geometry_program_object_sha256": main_object["attempt_geometry_program_object_sha256"],
        "attempt_artifact_readback_object_sha256": main_object["attempt_artifact_readback_object_sha256"],
        "attempt_representation_plan_sha256": main_object["attempt_representation_plan_sha256"],
        "authoring_mesh_id": main_object["authoring_mesh_id"],
        "authoring_mesh_lineage_id": main_object["authoring_mesh_lineage_id"],
        "authoring_mesh_revision_id": main_object["authoring_mesh_revision_id"],
        "authoring_mesh_revision_index": main_object["authoring_mesh_revision_index"],
        "authoring_mesh_revision_sha256": main_object["authoring_mesh_revision_sha256"],
        "authoring_mesh_revision_object_sha256": main_object["authoring_mesh_revision_object_sha256"],
        "authoring_mesh_identity_sha256": main_object["authoring_mesh_identity_sha256"],
        "authoring_mesh_sha256": main_object["authoring_mesh_sha256"],
        "fixed_view_id": main_object["fixed_view"]["view_id"],
        "camera_set_sha256": main_object["camera_set_sha256"],
        "render_set_id": main_object["render_set_id"],
        "render_set_sha256": main_object["render_set_sha256"],
        "render_set_object_sha256": main_object["render_set_object_sha256"],
        "reference_comparison_id": main_object["reference_comparison_id"],
        "reference_comparison_sha256": main_object["reference_comparison_sha256"],
        "reference_comparison_object_sha256": main_object["reference_comparison_object_sha256"],
        "quality_report_id": main_object["quality_report_id"],
        "quality_report_sha256": main_object["quality_report_sha256"],
        "quality_report_object_sha256": main_object["quality_report_object_sha256"],
        "evidence_bundle_sha256": main_object["evidence_bundle_sha256"],
        "hard_gate_status": main_object["hard_gate_status"],
        "visual_gate_status": main_object["visual_gate_status"],
        "quality_status": main_object["quality_status"],
        "high_status": main_object["high_status"],
        "human_status": main_object["human_status"],
        "engine_status": main_object["engine_status"],
        "high_mesh_created": false,
        "high_stage_unlocked": false,
        "production_stage_advanced": false,
        "candidate_confirmed": false,
        "version_created": false,
        "export_performed": false,
        "idempotency_key": idempotency_key.map(Value::from).unwrap_or(Value::Null),
        "replayed": replayed,
        "store_effect": if effects { "inserted" } else { "not-touched" },
        "cas_effect": if effects { "inserted" } else { "not-touched" },
        "atomicity_status": if effects { "committed" } else { "not-touched" },
        "store_commit_status": if effects { "committed" } else { "not-touched" },
        "cas_commit_status": if effects { "committed" } else { "not-touched" },
        "runtime_write_performed": effects,
        "persistent_user_data_touched": effects,
        "partial_result_exposed": false,
        "writer_policy": WRITER_POLICY,
        "canonicalization_policy": MAIN_CANONICALIZATION,
        "canonical_sha256": "",
    });
    result["canonical_sha256"] = Value::String(canonical_json_hash(&result));
    let bytes = canonical_json_bytes(&result).map_err(|error| invalid(error.to_string()))?;
    if bytes.len() as u64 > MAX_RESPONSE_BYTES {
        return Err(invalid("result exceeds the one MiB response budget"));
    }
    let _ = runtime;
    Ok(result)
}

pub(crate) fn prepare(runtime: &Runtime, request: &Value) -> Result<Value, RuntimeError> {
    let object = exact_object(request, PREPARE_FIELDS, "prepare")?;
    request_header(object, PREPARE_SCHEMA, PREPARE_OPERATION, "prepare")?;
    request_hash(request, object, "prepare")?;
    let project_id = id(object, "project_id", "prepare")?;
    let idempotency_key = id(object, "idempotency_key", "prepare")?;
    let proposal = object
        .get("pass_state")
        .ok_or_else(|| invalid("prepare.pass_state is missing"))?;
    if proposal.get("project_id").and_then(Value::as_str) != Some(project_id.as_str()) {
        return Err(mismatch("request project and pass_state project differ"));
    }
    let derived = derive_main(runtime, proposal)?;
    validate_derived(proposal, &derived)?;
    if let Some(existing) = runtime
        .store
        .get_knife_pass_state_by_idempotency(&project_id, &idempotency_key)?
    {
        let existing_main = forgecad_store::knife_pass_state_main_value(&existing)?;
        if existing_main != derived {
            return Err(invalid(
                "idempotency key is bound to a different immutable pass state",
            ));
        }
        // The replay branch deliberately omits the idempotency key from the
        // public result.  The closed result schema reserves that field for a
        // newly committed prepare; replay is a readback of immutable state.
        return result_from_record(
            runtime,
            &existing,
            PREPARE_OPERATION,
            "prepare",
            "replayed",
            None,
            true,
            false,
        );
    }
    let bytes = canonical_json_bytes(&derived).map_err(|error| invalid(error.to_string()))?;
    if bytes.is_empty() || bytes.len() as u64 > KNIFE_PASS_STATE_MAX_JSON_BYTES {
        return Err(invalid("derived Main exceeds the pass-state CAS budget"));
    }
    let reservation = runtime.store.begin_cas_reservation();
    let mut staged: Option<CasObject> = None;
    let result = (|| {
        let object = runtime.store.put_object_reserved(
            &reservation,
            &bytes,
            None,
            KNIFE_PASS_STATE_JSON_MIME,
            KNIFE_PASS_STATE_OBJECT_KIND,
            derived["created_at"].as_str().unwrap_or_default(),
        )?;
        staged = Some(object.clone());
        let record = forgecad_store::knife_pass_state_record_from_main_value(
            derived.clone(),
            object.record.sha256.clone(),
            idempotency_key.clone(),
        )?;
        let commit = KnifePassStateCommit {
            record,
            cas: KnifePassStateCasBundle {
                pass_state: object.record.clone(),
            },
        };
        let (stored, replayed) = runtime.store.record_knife_pass_state_with_replay(&commit)?;
        let _ = runtime
            .store
            .release_cas_reservation_object(&reservation, &object, false);
        staged = None;
        result_from_record(
            runtime,
            &stored,
            PREPARE_OPERATION,
            "prepare",
            if replayed { "replayed" } else { "prepared" },
            if replayed {
                None
            } else {
                Some(&stored.idempotency_key)
            },
            replayed,
            !replayed,
        )
    })();
    if result.is_err() {
        if let Some(object) = staged.as_ref() {
            let _ = runtime
                .store
                .release_cas_reservation_object(&reservation, object, true);
        }
    }
    result
}

pub(crate) fn get(runtime: &Runtime, request: &Value) -> Result<Value, RuntimeError> {
    let object = exact_object(request, GET_FIELDS, "get")?;
    request_header(object, GET_SCHEMA, GET_OPERATION, "get")?;
    request_hash(request, object, "get")?;
    if object
        .get("persistent_user_data_touched")
        .and_then(Value::as_bool)
        != Some(false)
    {
        return Err(invalid("get.persistent_user_data_touched must be false"));
    }
    let project_id = id(object, "project_id", "get")?;
    let pass_id = id(object, "pass_id", "get")?;
    let pass_state_sha256 = hash(object, "pass_state_sha256", "get")?;
    let record = runtime
        .store
        .get_knife_pass_state(&project_id, &pass_id, &pass_state_sha256)?
        .ok_or_else(|| invalid("exact pass state is not durable"))?;
    if object
        .get("pass_state_object_sha256")
        .and_then(Value::as_str)
        != Some(record.pass_state_object_sha256.as_str())
    {
        return Err(mismatch(
            "get pass state object identity differs from the immutable Store record",
        ));
    }
    let main = forgecad_store::knife_pass_state_main_value(&record)?;
    main_value_is_closed(&main, "stored pass_state")?;
    for key in [
        "project_id",
        "pass_id",
        "source_binding_id",
        "source_binding_sha256",
        "source_binding_object_sha256",
        "intent_bundle_id",
        "intent_bundle_sha256",
        "intent_bundle_object_sha256",
        "brief_id",
        "brief_sha256",
        "brief_object_sha256",
        "reference_id",
        "reference_object_sha256",
        "reference_evidence_sha256",
        "source_candidate_id",
        "source_candidate_state_sha256",
        "baseline_candidate_id",
        "baseline_candidate_state_sha256",
        "baseline_artifact_sha256",
        "baseline_geometry_program_sha256",
        "baseline_geometry_program_object_sha256",
        "baseline_artifact_readback_object_sha256",
        "baseline_representation_plan_sha256",
        "attempt_candidate_id",
        "attempt_candidate_state_sha256",
        "attempt_artifact_sha256",
        "attempt_geometry_program_sha256",
        "attempt_geometry_program_object_sha256",
        "attempt_artifact_readback_object_sha256",
        "attempt_representation_plan_sha256",
        "authoring_mesh_id",
        "authoring_mesh_lineage_id",
        "authoring_mesh_revision_id",
        "authoring_mesh_revision_sha256",
        "authoring_mesh_revision_object_sha256",
        "authoring_mesh_identity_sha256",
        "authoring_mesh_sha256",
        "camera_set_sha256",
        "render_set_id",
        "render_set_sha256",
        "render_set_object_sha256",
        "reference_comparison_id",
        "reference_comparison_sha256",
        "reference_comparison_object_sha256",
        "quality_report_id",
        "quality_report_sha256",
        "quality_report_object_sha256",
        "evidence_bundle_sha256",
    ] {
        if object.get(key) != main.get(key) {
            return Err(mismatch(format!(
                "get identity field {key} differs from the immutable Main"
            )));
        }
    }
    if object.get("authoring_mesh_revision_index") != main.get("authoring_mesh_revision_index")
        || object.get("fixed_view_id") != main.pointer("/fixed_view/view_id")
    {
        return Err(mismatch(
            "get revision index or fixed view identity differs",
        ));
    }
    let derived = derive_main(runtime, &main)?;
    validate_derived(&main, &derived)?;
    result_from_record(
        runtime,
        &record,
        GET_OPERATION,
        "get",
        "found",
        None,
        false,
        false,
    )
}

impl Runtime {
    pub fn knife_pass_state_prepare(&self, request: &Value) -> Result<Value, RuntimeError> {
        prepare(self, request)
    }

    pub fn knife_pass_state_get(&self, request: &Value) -> Result<Value, RuntimeError> {
        get(self, request)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn main_fixture() -> Value {
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../../../packages/forgecad-contracts/fixtures/knife-pass-state/positive/dragonfang-pass-state.json"
        )))
        .expect("KnifePassState Main fixture")
    }

    fn source_fixture() -> KnifeSourceBindingStoreRecord {
        let mut value: Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../../../packages/forgecad-contracts/fixtures/knife-source-binding/positive/dragonfang-source-binding.json"
        )))
        .expect("KnifeSourceBinding fixture");
        let object = value.as_object_mut().expect("source object");
        let semantic = object
            .remove("canonical_sha256")
            .expect("source semantic hash");
        object.insert(
            "schema_version".to_owned(),
            Value::String("KnifeSourceBindingStoreRecord@1".to_owned()),
        );
        object.insert("source_binding_sha256".to_owned(), semantic);
        object.insert(
            "source_binding_object_sha256".to_owned(),
            Value::String("a".repeat(64)),
        );
        object.insert(
            "idempotency_key".to_owned(),
            Value::String("knife-source-binding-fixture-key".to_owned()),
        );
        serde_json::from_value(value).expect("Store source record fixture")
    }

    fn candidate_truth_fixture(
        candidate_id: &str,
        state_sha256: &str,
        artifact_sha256: &str,
        program_sha256: &str,
        program_object_sha256: &str,
        readback_object_sha256: &str,
        representation_plan_sha256: &str,
    ) -> CandidateTruth {
        CandidateTruth {
            candidate: CandidateRecord {
                schema_version: "Candidate@1".to_owned(),
                candidate_id: candidate_id.to_owned(),
                project_id: "project-fixture".to_owned(),
                base_version_id: None,
                source_version_id: None,
                prepared_object_id: Some(artifact_sha256.to_owned()),
                prepared_object_sha256: Some(artifact_sha256.to_owned()),
                state: "reviewable".to_owned(),
                request_sha256: "b".repeat(64),
                manifest_hash: None,
                quality_report_id: Some("quality-fixture".to_owned()),
                quality_hard_gate_passed: true,
                canonical_sha256: state_sha256.to_owned(),
                error_code: None,
                created_at: "2026-08-31T00:00:00Z".to_owned(),
                updated_at: "2026-08-31T00:00:00Z".to_owned(),
            },
            evidence: GeometryCandidateEvidenceRecord {
                schema_version: "GeometryCandidateEvidence@1".to_owned(),
                candidate_id: candidate_id.to_owned(),
                project_id: "project-fixture".to_owned(),
                reference_id: Some("reference-fixture".to_owned()),
                reference_sha256: Some("c".repeat(64)),
                geometry_program_sha256: program_sha256.to_owned(),
                geometry_program_object_sha256: program_object_sha256.to_owned(),
                operator_catalog_sha256: "d".repeat(64),
                readback_config_sha256: "e".repeat(64),
                artifact_object_sha256: artifact_sha256.to_owned(),
                artifact_readback_object_sha256: readback_object_sha256.to_owned(),
                quality_report_object_sha256: "f".repeat(64),
                quality_report_id: "quality-fixture".to_owned(),
                canonical_sha256: "0".repeat(64),
                created_at: "2026-08-31T00:00:00Z".to_owned(),
            },
            artifact_sha256: artifact_sha256.to_owned(),
            program_sha256: program_sha256.to_owned(),
            program_object_sha256: program_object_sha256.to_owned(),
            readback_object_sha256: readback_object_sha256.to_owned(),
            representation_plan_sha256: representation_plan_sha256.to_owned(),
            hard_gate_passed: true,
        }
    }

    fn durable_revision_fixture(
        parent_revision_id: &str,
        revision_id: &str,
        revision_index: u64,
    ) -> forgecad_store::AuthoringMeshV2DurableRecord {
        forgecad_store::AuthoringMeshV2DurableRecord {
            schema_version: "AuthoringMeshV2DurableRecord@1".to_owned(),
            project_id: "project-fixture".to_owned(),
            mesh_id: "mesh-fixture".to_owned(),
            lineage_id: "lineage-fixture".to_owned(),
            revision_id: revision_id.to_owned(),
            parent_revision_ids: if parent_revision_id.is_empty() {
                Vec::new()
            } else {
                vec![parent_revision_id.to_owned()]
            },
            revision_index,
            revision_object_sha256: "1".repeat(64),
            revision_sha256: "2".repeat(64),
            operation_id: Some("operation-fixture".to_owned()),
            operation_kind: Some("move_vertices".to_owned()),
            operation_lineage_sha256: Some("3".repeat(64)),
            request_input_sha256: "4".repeat(64),
            idempotency_key: "revision-fixture-key".to_owned(),
            materialization_status: "runtime-owned-store-authoring-mesh-v2-durable-record@1"
                .to_owned(),
            canonical_sha256: "5".repeat(64),
            created_at: "2026-08-31T00:00:00Z".to_owned(),
        }
    }

    #[test]
    fn knife_pass_state_closed_main_rejects_unknown_and_stale_fields() {
        let fixture = main_fixture();
        assert!(main_value_is_closed(&fixture, "fixture").is_ok());

        let mut unknown = fixture.clone();
        unknown["caller_claim"] = json!("ignored");
        assert!(main_value_is_closed(&unknown, "fixture").is_err());

        let mut stale = fixture;
        stale["stage"] = json!("silhouette-blockout");
        assert!(main_value_is_closed(&stale, "fixture").is_err());
    }

    #[test]
    fn knife_pass_state_root_requires_anchor_and_source_baseline() {
        let source = source_fixture();
        let baseline = candidate_truth_fixture(
            &source.source_candidate_id,
            &source.source_candidate_state_sha256,
            &"b".repeat(64),
            &"c".repeat(64),
            &"d".repeat(64),
            &"e".repeat(64),
            &"f".repeat(64),
        );
        assert!(
            validate_root_selection(&source.authoring_mesh_revision_id, &source, &baseline).is_ok()
        );

        let mut wrong_baseline = baseline.clone();
        wrong_baseline.candidate.candidate_id = "old-candidate".to_owned();
        assert!(validate_root_selection(
            &source.authoring_mesh_revision_id,
            &source,
            &wrong_baseline
        )
        .is_err());
        assert!(validate_root_selection("descendant-revision", &source, &baseline).is_err());
    }

    #[test]
    fn knife_pass_state_child_requires_direct_descendant_and_real_program_plan_change() {
        let fixture = main_fixture();
        let parent_record = forgecad_store::knife_pass_state_record_from_main_value(
            fixture.clone(),
            "a".repeat(64),
            "parent-idempotency",
        )
        .expect("parent Store projection");
        let baseline = candidate_truth_fixture(
            &parent_record.attempt_candidate_id,
            &parent_record.attempt_candidate_state_sha256,
            &parent_record.attempt_artifact_sha256,
            &parent_record.attempt_geometry_program_sha256,
            &parent_record.attempt_geometry_program_object_sha256,
            &parent_record.attempt_artifact_readback_object_sha256,
            &parent_record.attempt_representation_plan_sha256,
        );
        let attempt = candidate_truth_fixture(
            "child-candidate",
            &"6".repeat(64),
            &"7".repeat(64),
            &"8".repeat(64),
            &"9".repeat(64),
            &"a".repeat(64),
            &"b".repeat(64),
        );
        let selected = durable_revision_fixture(
            &parent_record.authoring_mesh_revision_id,
            "child-revision",
            parent_record.authoring_mesh_revision_index + 1,
        );
        assert!(validate_parent_successor(
            &parent_record,
            "silhouette-blockout",
            &selected,
            &baseline,
            &attempt,
            &parent_record.fixed_view,
            &parent_record.camera_set_sha256,
        )
        .is_ok());

        let mut fake_successor = selected.clone();
        fake_successor.parent_revision_ids = vec!["unrelated-parent".to_owned()];
        assert!(validate_parent_successor(
            &parent_record,
            "silhouette-blockout",
            &fake_successor,
            &baseline,
            &attempt,
            &parent_record.fixed_view,
            &parent_record.camera_set_sha256,
        )
        .is_err());

        let mut unchanged_attempt = attempt;
        unchanged_attempt.candidate = baseline.candidate.clone();
        unchanged_attempt.program_sha256 = baseline.program_sha256.clone();
        unchanged_attempt.program_object_sha256 = baseline.program_object_sha256.clone();
        unchanged_attempt.representation_plan_sha256 = baseline.representation_plan_sha256.clone();
        assert!(validate_parent_successor(
            &parent_record,
            "silhouette-blockout",
            &selected,
            &baseline,
            &unchanged_attempt,
            &parent_record.fixed_view,
            &parent_record.camera_set_sha256,
        )
        .is_err());
    }

    #[test]
    fn knife_pass_state_result_projection_marks_replay_and_get_read_only() {
        let main = main_fixture();
        let record = forgecad_store::knife_pass_state_record_from_main_value(
            main,
            "a".repeat(64),
            "prepare-idempotency",
        )
        .expect("Store record");
        let runtime = Runtime::ephemeral().expect("ephemeral Runtime");

        let prepared = result_from_record(
            &runtime,
            &record,
            PREPARE_OPERATION,
            "prepare",
            "prepared",
            Some("prepare-idempotency"),
            false,
            true,
        )
        .expect("prepared result");
        assert_eq!(prepared["idempotency_key"], json!("prepare-idempotency"));
        assert_eq!(prepared["runtime_write_performed"], json!(true));
        assert_eq!(prepared["persistent_user_data_touched"], json!(true));

        let replayed = result_from_record(
            &runtime,
            &record,
            PREPARE_OPERATION,
            "prepare",
            "replayed",
            None,
            true,
            false,
        )
        .expect("replayed result");
        assert_eq!(replayed["idempotency_key"], Value::Null);
        assert_eq!(replayed["replayed"], json!(true));
        assert_eq!(replayed["runtime_write_performed"], json!(false));
        assert_eq!(replayed["persistent_user_data_touched"], json!(false));

        let found = result_from_record(
            &runtime,
            &record,
            GET_OPERATION,
            "get",
            "found",
            None,
            false,
            false,
        )
        .expect("get result");
        assert_eq!(found["idempotency_key"], Value::Null);
        assert_eq!(found["runtime_write_performed"], json!(false));
        assert_eq!(found["persistent_user_data_touched"], json!(false));
        let mut found_preimage = found.clone();
        found_preimage["canonical_sha256"] = Value::String(String::new());
        assert_eq!(
            found["canonical_sha256"],
            json!(canonical_json_hash(&found_preimage))
        );
        assert!(!found
            .as_object()
            .expect("result object")
            .contains_key("source_ref"));
        assert!(!found
            .as_object()
            .expect("result object")
            .contains_key("restart_hash_verified"));
    }

    #[test]
    fn knife_pass_state_malformed_prepare_never_writes() {
        let runtime = Runtime::ephemeral().expect("ephemeral Runtime");
        let mut request = json!({
            "schema_version": PREPARE_SCHEMA,
            "operation": PREPARE_OPERATION,
            "project_id": "project-fixture",
            "pass_state": main_fixture(),
            "idempotency_key": "malformed-request",
            "max_response_bytes": MAX_RESPONSE_BYTES,
            "runtime_write_performed": false,
            "writer_policy": WRITER_POLICY,
            "canonicalization_policy": REQUEST_CANONICALIZATION,
            "input_sha256": ""
        });
        request["input_sha256"] = json!(canonical_json_hash(&request));
        request["pass_state"]["quality_status"] = json!("QUALITY_TARGET_NOT_MET");
        let error = runtime
            .knife_pass_state_prepare(&request)
            .expect_err("tampered Main must fail before Store/CAS write");
        assert!(error.to_string().contains("KNIFE_PASS_STATE"));
        assert!(runtime
            .store
            .get_knife_pass_state_by_idempotency("project-fixture", "malformed-request")
            .expect("lookup")
            .is_none());
    }
}
