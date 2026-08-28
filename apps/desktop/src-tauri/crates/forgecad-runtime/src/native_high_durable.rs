//! Runtime-owned durable Native High source materialization.
//!
//! The two fixed sibling operations remain pure projections. Runtime admits
//! their byte-exact replay only after the request is bound to an already
//! durable AuthoringMesh. It then writes five derived CAS objects and one
//! Store link without retargeting a candidate or advancing ProductionStage.

use super::{
    canonical_json_bytes, canonical_json_hash, is_opaque_id, is_sha256, now_string, sha256_hex,
    Runtime, RuntimeError,
};
use base64::Engine;
use forgecad_store::{
    CasObject, CasReservation, NativeHighDurableRecord, AUTHORING_MESH_CANONICAL_OBJECT_KIND,
    NATIVE_HIGH_DETAIL_GRAPH_OBJECT_KIND, NATIVE_HIGH_DURABLE_LINK_OBJECT_KIND,
    NATIVE_HIGH_DURABLE_RECORD_SCHEMA_VERSION, NATIVE_HIGH_GLB_MATERIALIZE_RESULT_OBJECT_KIND,
    NATIVE_HIGH_MESH_ARTIFACT_OBJECT_KIND,
};
use forgecad_worker_protocol::{
    validate_native_high_glb_materialize_payload, validate_native_high_glb_materialize_result,
    NATIVE_HIGH_GLB_REQUEST_SCHEMA_VERSION,
};
use serde_json::{json, Map, Value};
use std::collections::BTreeSet;

const PREPARE_SCHEMA: &str = "NativeHighDurablePrepareRequest@1";
const GET_SCHEMA: &str = "NativeHighDurableGetRequest@1";
const PREPARE_RESULT_SCHEMA: &str = "NativeHighDurablePrepareResult@1";
const GET_RESULT_SCHEMA: &str = "NativeHighDurableGetResult@1";
const LINK_SCHEMA: &str = "NativeHighDurableLink@1";
const PREPARE_OPERATION: &str = "forgecad.production.native-high-durable-prepare@1";
const GET_OPERATION: &str = "forgecad.production.native-high-durable-get@1";
const WRITER_POLICY: &str = "forgecad-runtime-only-state-writer@1";
const CANONICALIZATION_POLICY: &str = "canonical-json-sha256-excluding-canonical-sha256@1";
const LINK_POLICY: &str = "native-high-authoring-mesh-detail-graph-artifact-glb-readback@1";
const IDEMPOTENCY_POLICY: &str = "same-input-hash-replays-without-new-record@1";
const LINK_STATUS: &str = "runtime-owned-durable-native-high-source-only@1";
const STORE_STATUS: &str = "runtime-owned-native-high-durable@1";
const JSON_MIME: &str = "application/json";
const GLB_MIME: &str = "model/gltf-binary";
const GLB_KIND: &str = "production-weapon-high-artifact-glb";
const MAX_RESPONSE_BYTES: u64 = 1_048_576;
const MAX_JSON_BYTES: u64 = 8 * 1024 * 1024;
const MAX_GLB_BYTES: u64 = 64 * 1024 * 1024;
// Runtime durable admission is deliberately stricter than the sibling
// transport envelope: every persisted JSON object must remain readable through
// Runtime's global bounded CAS gate.
const MAX_GLB_RESULT_BYTES: u64 = 64 * 1024 * 1024;
const LIMITATIONS: &[&str] = &[
    "RUNTIME_SOLE_WRITER",
    "NO_STAGE_ADVANCEMENT",
    "NO_CANDIDATE_CONFIRM",
    "NO_VERSION_CREATED",
    "NO_EXPORT",
    "SOURCE_ONLY_NOT_PRODUCTION_WEAPON_HIGH",
    "STRUCTURAL_ONLY_NOT_COMMERCIAL_QUALITY",
];

const PREPARE_FIELDS: &[&str] = &[
    "schema_version",
    "project_id",
    "candidate_id",
    "candidate_state_sha256",
    "base_version_id",
    "source_authoring_mesh_id",
    "source_authoring_mesh_object_sha256",
    "source_authoring_mesh_sha256",
    "high_mesh_request",
    "high_mesh_request_sha256",
    "idempotency_key",
    "max_response_bytes",
    "source_only",
    "runtime_write_performed",
    "writer_policy",
    "canonicalization_policy",
    "input_sha256",
];

const GET_FIELDS: &[&str] = &[
    "schema_version",
    "operation",
    "project_id",
    "candidate_id",
    "candidate_state_sha256",
    "base_version_id",
    "link_id",
    "link_object_sha256",
    "source_authoring_mesh_id",
    "source_authoring_mesh_sha256",
    "detail_graph_canonical_sha256",
    "artifact_id",
    "artifact_sha256",
    "glb_sha256",
    "idempotency_key",
    "source_only",
    "writer_policy",
    "runtime_write_performed",
    "persistent_user_data_touched",
    "input_sha256",
];

#[derive(Debug, Clone)]
struct PrepareRequest {
    project_id: String,
    candidate_id: String,
    candidate_state_sha256: String,
    base_version_id: Option<String>,
    source_mesh_id: String,
    source_mesh_object_sha256: String,
    source_mesh_sha256: String,
    high_mesh_request: Value,
    high_mesh_request_sha256: String,
    idempotency_key: String,
    input_sha256: String,
}

fn invalid(message: impl Into<String>) -> RuntimeError {
    RuntimeError::InvalidInput(format!("NATIVE_HIGH_DURABLE_INVALID: {}", message.into()))
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
    if actual != expected {
        return Err(invalid(format!(
            "{context} contains an unknown or missing field"
        )));
    }
    Ok(object)
}

fn text(object: &Map<String, Value>, field: &str) -> Result<String, RuntimeError> {
    object
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| is_opaque_id(value))
        .map(str::to_owned)
        .ok_or_else(|| invalid(format!("{field} is not an opaque id")))
}

fn hash(object: &Map<String, Value>, field: &str) -> Result<String, RuntimeError> {
    object
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .map(str::to_owned)
        .ok_or_else(|| invalid(format!("{field} is not a SHA-256")))
}

fn nullable_id(object: &Map<String, Value>, field: &str) -> Result<Option<String>, RuntimeError> {
    match object.get(field) {
        Some(Value::Null) => Ok(None),
        Some(Value::String(value)) if is_opaque_id(value) => Ok(Some(value.clone())),
        _ => Err(invalid(format!("{field} is not a nullable opaque id"))),
    }
}

fn request_input_hash(value: &Value) -> Result<String, RuntimeError> {
    let mut preimage = value.clone();
    let object = preimage
        .as_object_mut()
        .ok_or_else(|| invalid("request must be an object"))?;
    object.remove("input_sha256");
    object.remove("idempotency_key");
    Ok(canonical_json_hash(&preimage))
}

fn parse_prepare(value: &Value) -> Result<PrepareRequest, RuntimeError> {
    let object = exact_object(value, PREPARE_FIELDS, PREPARE_SCHEMA)?;
    if object.get("schema_version").and_then(Value::as_str) != Some(PREPARE_SCHEMA)
        || object.get("max_response_bytes").and_then(Value::as_u64) != Some(MAX_RESPONSE_BYTES)
        || object.get("source_only") != Some(&Value::Bool(true))
        || object.get("runtime_write_performed") != Some(&Value::Bool(false))
        || object.get("writer_policy").and_then(Value::as_str) != Some(WRITER_POLICY)
        || object
            .get("canonicalization_policy")
            .and_then(Value::as_str)
            != Some(CANONICALIZATION_POLICY)
    {
        return Err(invalid("prepare policy fields differ"));
    }
    let high_mesh_request = object
        .get("high_mesh_request")
        .filter(|value| value.is_object())
        .cloned()
        .ok_or_else(|| invalid("high_mesh_request is not an object"))?;
    let high_mesh_request_sha256 = hash(object, "high_mesh_request_sha256")?;
    let mut high_request_preimage = high_mesh_request.clone();
    high_request_preimage["canonical_sha256"] = Value::String(String::new());
    if high_mesh_request
        .get("canonical_sha256")
        .and_then(Value::as_str)
        != Some(high_mesh_request_sha256.as_str())
        || canonical_json_hash(&high_request_preimage) != high_mesh_request_sha256
    {
        return Err(invalid("high_mesh_request canonical binding differs"));
    }
    let input_sha256 = hash(object, "input_sha256")?;
    if request_input_hash(value)? != input_sha256 {
        return Err(invalid("input_sha256 does not bind the prepare request"));
    }
    Ok(PrepareRequest {
        project_id: text(object, "project_id")?,
        candidate_id: text(object, "candidate_id")?,
        candidate_state_sha256: hash(object, "candidate_state_sha256")?,
        base_version_id: nullable_id(object, "base_version_id")?,
        source_mesh_id: text(object, "source_authoring_mesh_id")?,
        source_mesh_object_sha256: hash(object, "source_authoring_mesh_object_sha256")?,
        source_mesh_sha256: hash(object, "source_authoring_mesh_sha256")?,
        high_mesh_request,
        high_mesh_request_sha256,
        idempotency_key: text(object, "idempotency_key")?,
        input_sha256,
    })
}

fn parse_get(value: &Value) -> Result<&Map<String, Value>, RuntimeError> {
    let object = exact_object(value, GET_FIELDS, GET_SCHEMA)?;
    if object.get("schema_version").and_then(Value::as_str) != Some(GET_SCHEMA)
        || object.get("operation").and_then(Value::as_str) != Some(GET_OPERATION)
        || object.get("source_only") != Some(&Value::Bool(true))
        || object.get("runtime_write_performed") != Some(&Value::Bool(false))
        || object.get("persistent_user_data_touched") != Some(&Value::Bool(false))
        || object.get("writer_policy").and_then(Value::as_str) != Some(WRITER_POLICY)
    {
        return Err(invalid("get policy fields differ"));
    }
    let input = hash(object, "input_sha256")?;
    let mut preimage = value.clone();
    preimage["input_sha256"] = Value::String(String::new());
    if canonical_json_hash(&preimage) != input {
        return Err(invalid("input_sha256 does not bind the get request"));
    }
    Ok(object)
}

fn source_preflight(runtime: &Runtime, request: &PrepareRequest) -> Result<Value, RuntimeError> {
    let record = runtime
        .store
        .get_authoring_mesh_durable_record_by_mesh(&request.candidate_id, &request.source_mesh_id)?
        .ok_or_else(|| invalid("durable source AuthoringMesh is unavailable"))?;
    if record.project_id != request.project_id
        || record.candidate_state_sha256 != request.candidate_state_sha256
        || record.base_version_id != request.base_version_id
        || record.canonical_mesh_object_sha256 != request.source_mesh_object_sha256
        || record.canonical_mesh_sha256 != request.source_mesh_sha256
    {
        return Err(invalid("durable source AuthoringMesh binding differs"));
    }
    let bytes = runtime.cas_read_bounded(&request.source_mesh_object_sha256, MAX_JSON_BYTES)?;
    let source_mesh: Value = serde_json::from_slice(&bytes)
        .map_err(|_| invalid("source AuthoringMesh JSON is invalid"))?;
    if source_mesh.get("schema_version").and_then(Value::as_str) != Some("AuthoringMeshCanonical@1")
        || source_mesh.get("canonical_mesh_id").and_then(Value::as_str)
            != Some(request.source_mesh_id.as_str())
        || source_mesh.get("project_id").and_then(Value::as_str)
            != Some(request.project_id.as_str())
        || source_mesh.get("candidate_id").and_then(Value::as_str)
            != Some(request.candidate_id.as_str())
        || source_mesh
            .get("candidate_state_sha256")
            .and_then(Value::as_str)
            != Some(request.candidate_state_sha256.as_str())
        || source_mesh.get("canonical_sha256").and_then(Value::as_str)
            != Some(request.source_mesh_sha256.as_str())
        || request.high_mesh_request["source_authoring_mesh"]["canonical_mesh"] != source_mesh
        || request.high_mesh_request["source_authoring_mesh"]["candidate_id"]
            != request.candidate_id
        || request.high_mesh_request["source_authoring_mesh"]["candidate_state_sha256"]
            != request.candidate_state_sha256
        || request.high_mesh_request["source_authoring_mesh"]["head_candidate_id"]
            != request.candidate_id
        || request.high_mesh_request["source_authoring_mesh"]["head_candidate_state_sha256"]
            != request.candidate_state_sha256
        || request.high_mesh_request["source_authoring_mesh"]["source_mesh_sha256"]
            != request.source_mesh_sha256
        || request
            .high_mesh_request
            .get("source_authoring_mesh_sha256")
            .and_then(Value::as_str)
            != Some(
                canonical_json_hash(&request.high_mesh_request["source_authoring_mesh"]).as_str(),
            )
    {
        return Err(invalid(
            "High request source AuthoringMesh differs from durable CAS truth",
        ));
    }
    Ok(source_mesh)
}

fn execute_workers(
    request: &PrepareRequest,
) -> Result<(Value, Value, Vec<u8>, String), RuntimeError> {
    let high_first =
        super::geometry_worker::production_weapon_native_high(&request.high_mesh_request)
            .map_err(|error| invalid(error.to_string()))?;
    let high_second =
        super::geometry_worker::production_weapon_native_high(&request.high_mesh_request)
            .map_err(|error| invalid(error.to_string()))?;
    if high_first.result != high_second.result
        || high_first.build_cohort_sha256 != high_second.build_cohort_sha256
    {
        return Err(invalid("Native High Worker replay or cohort differs"));
    }
    let artifact = high_first.result;
    // The Worker records the digest of the complete typed request, while the
    // outer durable envelope separately carries the canonical preimage hash
    // (with `canonical_sha256` blank). Keep both bindings strict and distinct.
    let high_request_sha256 = canonical_json_hash(&request.high_mesh_request);
    if artifact.get("schema_version").and_then(Value::as_str) != Some("HighMeshArtifact@1")
        || artifact
            .get("source_authoring_mesh_sha256")
            .and_then(Value::as_str)
            != request
                .high_mesh_request
                .get("source_authoring_mesh_sha256")
                .and_then(Value::as_str)
        || artifact.get("request_sha256").and_then(Value::as_str)
            != Some(high_request_sha256.as_str())
        || artifact.get("replay_count").and_then(Value::as_u64) != Some(2)
        || artifact.get("replay_byte_exact") != Some(&Value::Bool(true))
        || artifact.get("runtime_write_performed") != Some(&Value::Bool(false))
        || artifact.get("production_stage_advanced") != Some(&Value::Bool(false))
        || artifact.get("candidate_confirmed") != Some(&Value::Bool(false))
        || artifact.get("version_created") != Some(&Value::Bool(false))
        || artifact.get("export_performed") != Some(&Value::Bool(false))
        || artifact.get("quality_status").and_then(Value::as_str) != Some("structural_only")
        || artifact.get("hard_gate_passed") != Some(&Value::Bool(false))
    {
        return Err(invalid(
            "Native High artifact binding or source-only truth differs",
        ));
    }
    for field in [
        "artifact_sha256",
        "canonical_sha256",
        "detail_graph_canonical_sha256",
        "high_worker_algorithm_sha256",
        "high_worker_build_cohort_sha256",
    ] {
        if artifact
            .get(field)
            .and_then(Value::as_str)
            .is_none_or(|value| !is_sha256(value))
        {
            return Err(invalid(format!("Native High artifact {field} is invalid")));
        }
    }
    let mut glb_payload = json!({
        "schema_version":NATIVE_HIGH_GLB_REQUEST_SCHEMA_VERSION,
        "artifact":artifact,
        "input_canonical_sha256":artifact["canonical_sha256"],
        "canonical_sha256":""
    });
    glb_payload["canonical_sha256"] = Value::String(canonical_json_hash(&glb_payload));
    validate_native_high_glb_materialize_payload(&glb_payload).map_err(|error| invalid(error))?;
    let glb_first =
        super::geometry_worker::production_weapon_native_high_glb_materialize(&glb_payload)
            .map_err(|error| invalid(error.to_string()))?;
    let glb_second =
        super::geometry_worker::production_weapon_native_high_glb_materialize(&glb_payload)
            .map_err(|error| invalid(error.to_string()))?;
    if glb_first.result != glb_second.result
        || glb_first.build_cohort_sha256 != glb_second.build_cohort_sha256
        || high_first.build_cohort_sha256 != glb_first.build_cohort_sha256
    {
        return Err(invalid("Native High GLB replay or sibling cohort differs"));
    }
    validate_native_high_glb_materialize_result(&glb_first.result)
        .map_err(|error| invalid(error))?;
    let glb = base64::engine::general_purpose::STANDARD
        .decode(
            glb_first
                .result
                .get("glb_base64")
                .and_then(Value::as_str)
                .ok_or_else(|| invalid("Native High GLB base64 is missing"))?,
        )
        .map_err(|_| invalid("Native High GLB base64 is invalid"))?;
    if glb.is_empty()
        || glb.len() as u64 > MAX_GLB_BYTES
        || glb_first.result.get("glb_sha256").and_then(Value::as_str)
            != Some(sha256_hex(&glb).as_str())
    {
        return Err(invalid("Native High GLB bytes or hash differ"));
    }
    super::native_high_glb_readback::inspect_and_validate_against_worker_readback(
        &glb,
        &glb_first.result["strict_readback"],
    )
    .map_err(|error| invalid(error.to_string()))?;
    let glb_transport_cohort = glb_first
        .build_cohort_sha256
        .filter(|value| is_sha256(value))
        .ok_or_else(|| invalid("Native High GLB sibling cohort is unavailable"))?;
    Ok((artifact, glb_first.result, glb, glb_transport_cohort))
}

fn canonical_object(value: &Value) -> Result<(Vec<u8>, String), RuntimeError> {
    let bytes = canonical_json_bytes(value).map_err(|error| invalid(error.to_string()))?;
    Ok((bytes.clone(), sha256_hex(&bytes)))
}

fn build_link(
    request: &PrepareRequest,
    artifact: &Value,
    detail_graph_object_sha256: &str,
    artifact_object_sha256: &str,
    glb_sha256: &str,
    glb_result_object_sha256: &str,
    glb_result_canonical_sha256: &str,
) -> Result<Value, RuntimeError> {
    let artifact_id = artifact
        .get("artifact_id")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("artifact_id is missing"))?;
    let artifact_sha256 = artifact
        .get("artifact_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("artifact_sha256 is missing"))?;
    let detail_graph_sha256 = artifact
        .get("detail_graph_canonical_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("detail graph hash is missing"))?;
    let worker_algorithm = artifact
        .get("high_worker_algorithm_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("High Worker algorithm hash is missing"))?;
    let worker_cohort = artifact
        .get("high_worker_build_cohort_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("High Worker build cohort is missing"))?;
    let request_sha256 = artifact
        .get("request_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("High artifact request hash is missing"))?;
    let link_seed = canonical_json_hash(&json!({
        "project_id":request.project_id,
        "candidate_id":request.candidate_id,
        "candidate_state_sha256":request.candidate_state_sha256,
        "source_authoring_mesh_sha256":request.source_mesh_sha256,
        "artifact_sha256":artifact_sha256,
        "glb_sha256":glb_sha256,
        "request_input_sha256":request.input_sha256
    }));
    let mut link = json!({
        "schema_version":LINK_SCHEMA,
        "link_id":format!("native-high-link-{}", &link_seed[..24]),
        "project_id":request.project_id,
        "candidate_id":request.candidate_id,
        "candidate_state_sha256":request.candidate_state_sha256,
        "base_version_id":request.base_version_id,
        "source_authoring_mesh_id":request.source_mesh_id,
        "source_authoring_mesh_object_sha256":request.source_mesh_object_sha256,
        "source_authoring_mesh_sha256":request.source_mesh_sha256,
        "detail_graph_object_sha256":detail_graph_object_sha256,
        "detail_graph_canonical_sha256":detail_graph_sha256,
        "artifact_id":artifact_id,
        "artifact_object_sha256":artifact_object_sha256,
        "artifact_sha256":artifact_sha256,
        "artifact_readback_object_sha256":glb_result_object_sha256,
        "artifact_readback_sha256":glb_result_canonical_sha256,
        "glb_object_sha256":glb_sha256,
        "glb_sha256":glb_sha256,
        "glb_readback_object_sha256":glb_result_object_sha256,
        "glb_readback_sha256":glb_result_canonical_sha256,
        "high_worker_algorithm_sha256":worker_algorithm,
        "high_worker_build_cohort_sha256":worker_cohort,
        "replay_count":2,
        "replay_byte_exact":true,
        "request_sha256":request_sha256,
        "idempotency_key":request.idempotency_key,
        "link_policy":LINK_POLICY,
        "writer_policy":WRITER_POLICY,
        "materialization_status":LINK_STATUS,
        "idempotency_policy":IDEMPOTENCY_POLICY,
        "source_only":true,
        "runtime_write_performed":true,
        "persistent_user_data_touched":true,
        "production_stage_advanced":false,
        "candidate_confirmed":false,
        "version_created":false,
        "export_performed":false,
        "quality_status":"structural_only",
        "limitations":LIMITATIONS,
        "canonicalization_policy":CANONICALIZATION_POLICY,
        "canonical_sha256":""
    });
    link["canonical_sha256"] = Value::String(canonical_json_hash(&link));
    Ok(link)
}

fn release(runtime: &Runtime, reservation: &CasReservation, objects: &[CasObject], rollback: bool) {
    for object in objects.iter().rev() {
        let _ = runtime
            .store
            .release_cas_reservation_object(reservation, object, rollback);
    }
}

fn put_reserved(
    runtime: &Runtime,
    reservation: &CasReservation,
    bytes: &[u8],
    expected_sha256: &str,
    mime: &str,
    kind: &str,
    created_at: &str,
    objects: &mut Vec<CasObject>,
) -> Result<CasObject, RuntimeError> {
    let object = runtime.store.put_object_reserved(
        reservation,
        bytes,
        Some(expected_sha256),
        mime,
        kind,
        created_at,
    )?;
    objects.push(object.clone());
    Ok(object)
}

fn read_value(runtime: &Runtime, sha256: &str, max_bytes: u64) -> Result<Value, RuntimeError> {
    let bytes = runtime.cas_read_bounded(sha256, max_bytes)?;
    serde_json::from_slice(&bytes).map_err(|_| invalid("durable CAS JSON is invalid"))
}

fn build_output(
    runtime: &Runtime,
    record: &NativeHighDurableRecord,
    replayed: bool,
    schema: &str,
    operation: &str,
) -> Result<Value, RuntimeError> {
    let source_mesh = read_value(
        runtime,
        &record.source_canonical_mesh_object_sha256,
        MAX_JSON_BYTES,
    )?;
    let detail_graph = read_value(runtime, &record.detail_graph_object_sha256, MAX_JSON_BYTES)?;
    let artifact = read_value(
        runtime,
        &record.high_mesh_artifact_object_sha256,
        MAX_JSON_BYTES,
    )?;
    let glb_result = read_value(
        runtime,
        &record.high_artifact_readback_object_sha256,
        MAX_GLB_RESULT_BYTES,
    )?;
    validate_native_high_glb_materialize_result(&glb_result).map_err(|error| invalid(error))?;
    let glb = runtime.cas_read_bounded(&record.high_artifact_object_sha256, MAX_GLB_BYTES)?;
    let embedded_glb = base64::engine::general_purpose::STANDARD
        .decode(
            glb_result
                .get("glb_base64")
                .and_then(Value::as_str)
                .ok_or_else(|| invalid("durable GLB readback base64 is missing"))?,
        )
        .map_err(|_| invalid("durable GLB readback base64 is invalid"))?;
    if embedded_glb != glb
        || sha256_hex(&glb) != record.high_artifact_sha256
        || glb_result.get("glb_sha256").and_then(Value::as_str)
            != Some(record.high_artifact_sha256.as_str())
    {
        return Err(invalid("durable GLB and strict readback bytes differ"));
    }
    let link = read_value(runtime, &record.link_object_sha256, MAX_JSON_BYTES)?;
    if link
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .is_none_or(|value| !is_sha256(value))
        || link.get("link_id").and_then(Value::as_str) != Some(record.link_id.as_str())
        || link.get("artifact_object_sha256").and_then(Value::as_str)
            != Some(record.high_mesh_artifact_object_sha256.as_str())
        || link.get("glb_object_sha256").and_then(Value::as_str)
            != Some(record.high_artifact_object_sha256.as_str())
    {
        return Err(invalid("durable Native High link binding differs"));
    }
    let mut output = json!({
        "schema_version":schema,
        "operation":operation,
        "project_id":record.project_id,
        "candidate_id":record.candidate_id,
        "candidate_state_sha256":record.candidate_state_sha256,
        "base_version_id":record.base_version_id,
        "source_authoring_mesh":source_mesh,
        "source_authoring_mesh_object_sha256":record.source_canonical_mesh_object_sha256,
        "source_authoring_mesh_sha256":record.source_canonical_mesh_sha256,
        "detail_graph":detail_graph,
        "detail_graph_object_sha256":record.detail_graph_object_sha256,
        "detail_graph_canonical_sha256":record.detail_graph_canonical_sha256,
        "artifact":artifact,
        "artifact_id":record.high_artifact_id,
        "artifact_object_sha256":record.high_mesh_artifact_object_sha256,
        "artifact_sha256":record.high_mesh_artifact_sha256,
        "artifact_readback_object_sha256":record.high_artifact_readback_object_sha256,
        "artifact_readback_sha256":artifact["canonical_sha256"],
        "glb_object_sha256":record.high_artifact_object_sha256,
        "glb_sha256":record.high_artifact_sha256,
        "glb_readback_object_sha256":record.high_artifact_readback_object_sha256,
        "glb_readback_sha256":glb_result["canonical_sha256"],
        "high_worker_algorithm_sha256":artifact["high_worker_algorithm_sha256"],
        "high_worker_build_cohort_sha256":artifact["high_worker_build_cohort_sha256"],
        "replay_count":2,
        "replay_byte_exact":true,
        "request_sha256":record.request_sha256,
        "request_input_sha256":record.input_sha256,
        "idempotency_key":record.idempotency_key,
        "replayed":replayed,
        "restart_hash_verified":true,
        "link_id":record.link_id,
        "link_object_sha256":record.link_object_sha256,
        "durable_link":link,
        "source_only":true,
        "writer_policy":WRITER_POLICY,
        "runtime_write_performed":schema == PREPARE_RESULT_SCHEMA,
        "persistent_user_data_touched":schema == PREPARE_RESULT_SCHEMA,
        "production_stage_advanced":false,
        "candidate_confirmed":false,
        "version_created":false,
        "export_performed":false,
        "quality_status":"structural_only",
        "limitations":LIMITATIONS,
        "canonicalization_policy":CANONICALIZATION_POLICY,
        "canonical_sha256":""
    });
    output["canonical_sha256"] = Value::String(canonical_json_hash(&output));
    if canonical_json_bytes(&output)
        .map_err(|error| invalid(error.to_string()))?
        .len() as u64
        > MAX_RESPONSE_BYTES
    {
        return Err(invalid("durable Native High response exceeds its bound"));
    }
    Ok(output)
}

impl Runtime {
    pub fn native_high_durable_prepare(&self, value: Value) -> Result<Value, RuntimeError> {
        let request = parse_prepare(&value)?;
        if let Some(existing) = self
            .store
            .get_native_high_durable(&request.project_id, &request.idempotency_key)?
        {
            if existing.input_sha256 != request.input_sha256 {
                return Err(invalid("idempotency key is bound to another input"));
            }
            return build_output(
                self,
                &existing,
                true,
                PREPARE_RESULT_SCHEMA,
                PREPARE_OPERATION,
            );
        }
        let source_mesh = source_preflight(self, &request)?;
        let detail_graph = request.high_mesh_request["detail_graph"].clone();
        let detail_graph_sha = request.high_mesh_request["detail_graph_canonical_sha256"]
            .as_str()
            .filter(|value| is_sha256(value))
            .ok_or_else(|| invalid("detail graph canonical hash is invalid"))?
            .to_owned();
        if canonical_json_hash(&detail_graph) != detail_graph_sha {
            return Err(invalid("detail graph canonical hash differs"));
        }
        let (artifact, glb_result, glb, glb_transport_cohort) = execute_workers(&request)?;
        let (detail_bytes, detail_object_sha) = canonical_object(&detail_graph)?;
        let (artifact_bytes, artifact_object_sha) = canonical_object(&artifact)?;
        let (glb_result_bytes, glb_result_object_sha) = canonical_object(&glb_result)?;
        let glb_sha = sha256_hex(&glb);
        let link = build_link(
            &request,
            &artifact,
            &detail_object_sha,
            &artifact_object_sha,
            &glb_sha,
            &glb_result_object_sha,
            glb_result["canonical_sha256"].as_str().unwrap_or_default(),
        )?;
        let (link_bytes, link_object_sha) = canonical_object(&link)?;
        let created_at = now_string();
        let source_object = self
            .store
            .get_object(&request.source_mesh_object_sha256)?
            .ok_or_else(|| invalid("source AuthoringMesh CAS object is unavailable"))?;
        if source_object.kind != AUTHORING_MESH_CANONICAL_OBJECT_KIND
            || source_object.mime != JSON_MIME
        {
            return Err(invalid("source AuthoringMesh CAS metadata differs"));
        }
        let reservation = self.store.begin_cas_reservation();
        let mut objects = Vec::new();
        let write = (|| -> Result<_, RuntimeError> {
            let detail_object = put_reserved(
                self,
                &reservation,
                &detail_bytes,
                &detail_object_sha,
                JSON_MIME,
                NATIVE_HIGH_DETAIL_GRAPH_OBJECT_KIND,
                &created_at,
                &mut objects,
            )?;
            let artifact_object = put_reserved(
                self,
                &reservation,
                &artifact_bytes,
                &artifact_object_sha,
                JSON_MIME,
                NATIVE_HIGH_MESH_ARTIFACT_OBJECT_KIND,
                &created_at,
                &mut objects,
            )?;
            let glb_object = put_reserved(
                self,
                &reservation,
                &glb,
                &glb_sha,
                GLB_MIME,
                GLB_KIND,
                &created_at,
                &mut objects,
            )?;
            let glb_result_object = put_reserved(
                self,
                &reservation,
                &glb_result_bytes,
                &glb_result_object_sha,
                JSON_MIME,
                NATIVE_HIGH_GLB_MATERIALIZE_RESULT_OBJECT_KIND,
                &created_at,
                &mut objects,
            )?;
            let link_object = put_reserved(
                self,
                &reservation,
                &link_bytes,
                &link_object_sha,
                JSON_MIME,
                NATIVE_HIGH_DURABLE_LINK_OBJECT_KIND,
                &created_at,
                &mut objects,
            )?;
            let record = NativeHighDurableRecord {
                schema_version: NATIVE_HIGH_DURABLE_RECORD_SCHEMA_VERSION.to_owned(),
                project_id: request.project_id.clone(),
                candidate_id: request.candidate_id.clone(),
                candidate_state_sha256: request.candidate_state_sha256.clone(),
                base_version_id: request.base_version_id.clone(),
                source_canonical_mesh_id: request.source_mesh_id.clone(),
                source_canonical_mesh_object_sha256: request.source_mesh_object_sha256.clone(),
                source_canonical_mesh_sha256: request.source_mesh_sha256.clone(),
                detail_graph_object_sha256: detail_object.record.sha256.clone(),
                detail_graph_canonical_sha256: detail_graph_sha,
                high_mesh_artifact_object_sha256: artifact_object.record.sha256.clone(),
                high_mesh_artifact_sha256: artifact["artifact_sha256"]
                    .as_str()
                    .unwrap_or_default()
                    .to_owned(),
                high_artifact_id: artifact["artifact_id"]
                    .as_str()
                    .unwrap_or_default()
                    .to_owned(),
                high_artifact_object_sha256: glb_object.record.sha256.clone(),
                high_artifact_sha256: glb_sha,
                high_artifact_size_bytes: glb_object.record.size_bytes,
                high_artifact_readback_object_sha256: glb_result_object.record.sha256.clone(),
                high_artifact_readback_sha256: glb_result["canonical_sha256"]
                    .as_str()
                    .unwrap_or_default()
                    .to_owned(),
                link_id: link["link_id"].as_str().unwrap_or_default().to_owned(),
                link_object_sha256: link_object.record.sha256.clone(),
                request_sha256: artifact["request_sha256"]
                    .as_str()
                    .unwrap_or_default()
                    .to_owned(),
                input_sha256: request.input_sha256.clone(),
                idempotency_key: request.idempotency_key.clone(),
                high_worker_build_cohort_sha256: artifact["high_worker_build_cohort_sha256"]
                    .as_str()
                    .unwrap_or_default()
                    .to_owned(),
                glb_worker_build_cohort_sha256: glb_transport_cohort,
                materialization_status: STORE_STATUS.to_owned(),
                canonical_sha256: String::new(),
                created_at,
            };
            self.store
                .record_native_high_durable_with_replay(
                    &record,
                    &source_object,
                    &detail_object.record,
                    &artifact_object.record,
                    &glb_object.record,
                    &glb_result_object.record,
                    &link_object.record,
                )
                .map_err(RuntimeError::from)
        })();
        let (stored, replayed) = match write {
            Ok(value) => {
                release(self, &reservation, &objects, false);
                value
            }
            Err(error) => {
                release(self, &reservation, &objects, true);
                return Err(error);
            }
        };
        let _ = source_mesh;
        build_output(
            self,
            &stored,
            replayed,
            PREPARE_RESULT_SCHEMA,
            PREPARE_OPERATION,
        )
    }

    pub fn native_high_durable_get(&self, value: Value) -> Result<Value, RuntimeError> {
        let request = parse_get(&value)?;
        let project_id = text(request, "project_id")?;
        let idempotency_key = text(request, "idempotency_key")?;
        let record = self
            .store
            .get_native_high_durable(&project_id, &idempotency_key)?
            .ok_or_else(|| invalid("Native High durable record is unavailable"))?;
        let base_version_id = nullable_id(request, "base_version_id")?;
        let expected = [
            ("candidate_id", record.candidate_id.as_str()),
            (
                "candidate_state_sha256",
                record.candidate_state_sha256.as_str(),
            ),
            ("link_id", record.link_id.as_str()),
            ("link_object_sha256", record.link_object_sha256.as_str()),
            (
                "source_authoring_mesh_id",
                record.source_canonical_mesh_id.as_str(),
            ),
            (
                "source_authoring_mesh_sha256",
                record.source_canonical_mesh_sha256.as_str(),
            ),
            (
                "detail_graph_canonical_sha256",
                record.detail_graph_canonical_sha256.as_str(),
            ),
            ("artifact_id", record.high_artifact_id.as_str()),
            ("artifact_sha256", record.high_mesh_artifact_sha256.as_str()),
            ("glb_sha256", record.high_artifact_sha256.as_str()),
        ];
        if base_version_id != record.base_version_id
            || expected.iter().any(|(field, expected)| {
                request.get(*field).and_then(Value::as_str) != Some(*expected)
            })
        {
            return Err(invalid("Native High get binding differs"));
        }
        build_output(self, &record, true, GET_RESULT_SCHEMA, GET_OPERATION)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use uuid::Uuid;

    fn authoring_program(project_id: &str) -> Value {
        let mut program = json!({
            "schema_version":"GeometryProgram@2",
            "project_id":project_id,
            "representation_plan_sha256":"b".repeat(64),
            "operator_catalog_sha256":crate::operator_catalog_sha256(),
            "units":{"length":"meter","angle":"radian","coordinate_system":"right-handed-y-up"},
            "budgets":{
                "max_nodes":1,
                "max_triangles":32,
                "max_glb_bytes":67108864,
                "max_worker_memory_bytes":536870912,
                "max_runtime_ms":10000
            },
            "nodes":[{
                "node_id":"native-high-restart-panel",
                "operator_id":"forgecad.geometry.authoring-mesh@1",
                "inputs":[],
                "parameters":{
                    "shape":"authoring-mesh",
                    "topology_policy":"triangle-quad-manifold-with-boundary@1",
                    "vertices":[
                        {"element_id":"v0","position_m":[-1.0,-1.0,0.0]},
                        {"element_id":"v1","position_m":[1.0,-1.0,0.0]},
                        {"element_id":"v2","position_m":[1.0,1.0,0.0]},
                        {"element_id":"v3","position_m":[-1.0,1.0,0.0]}
                    ],
                    "edges":[
                        {"element_id":"e01","vertex_ids":["v0","v1"]},
                        {"element_id":"e03","vertex_ids":["v0","v3"]},
                        {"element_id":"e12","vertex_ids":["v1","v2"]},
                        {"element_id":"e23","vertex_ids":["v2","v3"]}
                    ],
                    "loops":[
                        {"element_id":"l0","face_id":"f0","ordinal":0,"vertex_id":"v0","edge_id":"e01","edge_forward":true},
                        {"element_id":"l1","face_id":"f0","ordinal":1,"vertex_id":"v1","edge_id":"e12","edge_forward":true},
                        {"element_id":"l2","face_id":"f0","ordinal":2,"vertex_id":"v2","edge_id":"e23","edge_forward":true},
                        {"element_id":"l3","face_id":"f0","ordinal":3,"vertex_id":"v3","edge_id":"e03","edge_forward":false}
                    ],
                    "faces":[{"element_id":"f0","loop_ids":["l0","l1","l2","l3"]}],
                    "position_m":[0.0,0.0,0.0],
                    "rotation_rad":[0.0,0.0,0.0]
                }
            }],
            "part_outputs":[{
                "part_id":"native-high-restart-panel",
                "input_node_ids":["native-high-restart-panel"],
                "material_zone_id":"zone-native-high-shell",
                "solid":false
            }]
        });
        let hash = crate::hash_geometry_program_with_runtime_worker(&program)
            .expect("GeometryProgram hash");
        program["canonical_sha256"] = hash["canonical_sha256"].clone();
        program
    }

    fn expected_canonical_mesh(
        projection: &Value,
        project_id: &str,
        candidate_id: &str,
        candidate_state_sha256: &str,
        base_version_id: Value,
        source_program_object_sha256: &str,
        source_program_sha256: &str,
        source_artifact_object_sha256: &str,
        source_artifact_sha256: &str,
        source_artifact_readback_object_sha256: &str,
        source_artifact_readback_sha256: &str,
        source_lineage_sha256: &str,
    ) -> Value {
        let canonical_mesh_id = projection["mesh_id"].as_str().expect("projection mesh id");
        let mesh_sha256 = projection["mesh_sha256"]
            .as_str()
            .expect("projection mesh sha");
        let original_id = projection["original_identity"]["identity_id"]
            .as_str()
            .expect("projection original identity");
        let evaluated_id = projection["evaluated_identity"]["identity_id"]
            .as_str()
            .expect("projection evaluated identity");
        let mut canonical = json!({
            "schema_version":"AuthoringMeshCanonical@1",
            "canonical_mesh_id":canonical_mesh_id,
            "project_id":project_id,
            "candidate_id":candidate_id,
            "candidate_state_sha256":candidate_state_sha256,
            "base_version_id":base_version_id,
            "authoring_node_id":"native-high-restart-panel",
            "part_id":"native-high-restart-panel",
            "source_program_object_sha256":source_program_object_sha256,
            "source_program_sha256":source_program_sha256,
            "source_artifact_object_sha256":source_artifact_object_sha256,
            "source_artifact_sha256":source_artifact_sha256,
            "source_artifact_readback_object_sha256":source_artifact_readback_object_sha256,
            "source_artifact_readback_sha256":source_artifact_readback_sha256,
            "source_lineage_sha256":source_lineage_sha256,
            "representation":"runtime-owned-original-half-edge@1",
            "storage_policy":"runtime-owned-sqlite-cas-canonical-authoring-mesh@1",
            "writer_policy":"forgecad-runtime-only-state-writer@1",
            "original_identity":{
                "identity_id":original_id,
                "namespace":"original",
                "identity_kind":"runtime-owned-original-authoring@1",
                "element_id_policy":"lineage-scoped-opaque-not-cross-version-stable@1",
                "topology_sha256":mesh_sha256,
                "source_lineage_sha256":source_lineage_sha256,
                "stability_scope":"same-canonical-mesh-lineage-only@1"
            },
            "evaluated_identity":{
                "identity_id":evaluated_id,
                "namespace":"evaluated",
                "identity_kind":"runtime-derived-evaluated-artifact-readback@1",
                "element_id_policy":"artifact-local-no-authoring-bijection@1",
                "correspondence_policy":"non-bijective-derived-only@1",
                "artifact_object_sha256":source_artifact_object_sha256,
                "artifact_readback_sha256":source_artifact_readback_sha256,
                "source_lineage_sha256":source_lineage_sha256,
                "cross_version_stable":false
            },
            "cross_version_stable":false,
            "cross_version_stability":{
                "status":"not-proven@1",
                "scope":"same-canonical-mesh-lineage-only@1",
                "stable_id_claim":"none-across-revisions@1",
                "deleted_id_reuse_policy":"not-proven-and-not-a-contract@1",
                "new_id_policy":"lineage-operation-parent-derived-draft-only@1",
                "evaluated_id_policy":"artifact-local-unstable-derived-only@1"
            },
            "counts":projection["counts"],
            "vertices":projection["vertices"],
            "edges":projection["edges"],
            "half_edges":projection["half_edges"],
            "corners":projection["corners"],
            "faces":projection["faces"],
            "loops":projection["loops"],
            "rings":projection["rings"],
            "topology":projection["topology"],
            "canonicalization_policy":"canonical-json-sha256-excluding-canonical-sha256@1",
            "runtime_write_performed":true,
            "persistent_user_data_touched":true,
            "stage_advanced":false,
            "candidate_confirmed":false,
            "version_created":false,
            "export_performed":false,
            "quality_status":"structural_only",
            "canonical_sha256":""
        });
        canonical["canonical_sha256"] = Value::String(canonical_json_hash(&canonical));
        canonical
    }

    fn high_request(
        canonical_mesh: Value,
        candidate_id: &str,
        candidate_state_sha256: &str,
    ) -> Value {
        let source_mesh_sha256 = canonical_mesh["canonical_sha256"]
            .as_str()
            .expect("canonical mesh hash")
            .to_owned();
        let source_authoring_mesh = json!({
            "schema_version":"HighWorkerAuthoringMeshAdapter@1",
            "canonical_mesh":canonical_mesh,
            "candidate_id":candidate_id,
            "candidate_state_sha256":candidate_state_sha256,
            "head_candidate_id":candidate_id,
            "head_candidate_state_sha256":candidate_state_sha256,
            "source_mesh_sha256":source_mesh_sha256
        });
        let detail_graph = json!({
            "schema_version":"DetailGraph@1",
            "nodes":[{
                "node_id":"native-high-floating-detail",
                "kind":"floating_detail",
                "parent_part_id":"native-high-restart-panel",
                "parent_node_id":null,
                "source_edge":null,
                "width_m":null,
                "count":null,
                "sharpness":null,
                "center_m":[0.0,0.0,2.0],
                "size_m":[1.0,1.0,1.0]
            }]
        });
        let mut request = json!({
            "schema_version":"HighMeshWorkerRequest@1",
            "operation":"forgecad.production.high-mesh-prepare@1",
            "source_authoring_mesh":source_authoring_mesh,
            "source_authoring_mesh_sha256":"",
            "detail_graph":detail_graph,
            "detail_graph_canonical_sha256":"",
            "budgets":{
                "max_detail_nodes":16,
                "max_output_vertices":1024,
                "max_output_triangles":2048
            },
            "canonical_sha256":""
        });
        request["source_authoring_mesh_sha256"] =
            Value::String(canonical_json_hash(&request["source_authoring_mesh"]));
        request["detail_graph_canonical_sha256"] =
            Value::String(canonical_json_hash(&request["detail_graph"]));
        request["canonical_sha256"] = Value::String(canonical_json_hash(&request));
        request
    }

    fn native_prepare_request(
        project_id: &str,
        candidate_id: &str,
        candidate_state_sha256: &str,
        base_version_id: Value,
        source_mesh_id: &str,
        source_mesh_object_sha256: &str,
        source_mesh_sha256: &str,
        high_mesh_request: Value,
        idempotency_key: &str,
    ) -> Value {
        let mut request = json!({
            "schema_version":PREPARE_SCHEMA,
            "project_id":project_id,
            "candidate_id":candidate_id,
            "candidate_state_sha256":candidate_state_sha256,
            "base_version_id":base_version_id,
            "source_authoring_mesh_id":source_mesh_id,
            "source_authoring_mesh_object_sha256":source_mesh_object_sha256,
            "source_authoring_mesh_sha256":source_mesh_sha256,
            "high_mesh_request":high_mesh_request,
            "high_mesh_request_sha256":"",
            "idempotency_key":idempotency_key,
            "max_response_bytes":MAX_RESPONSE_BYTES,
            "source_only":true,
            "runtime_write_performed":false,
            "writer_policy":WRITER_POLICY,
            "canonicalization_policy":CANONICALIZATION_POLICY,
            "input_sha256":""
        });
        request["high_mesh_request_sha256"] =
            request["high_mesh_request"]["canonical_sha256"].clone();
        request["input_sha256"] = Value::String(request_input_hash(&request).expect("input hash"));
        request
    }

    #[test]
    fn prepare_parser_rejects_output_injection() {
        let value = json!({"schema_version":PREPARE_SCHEMA,"artifact":{}});
        assert!(parse_prepare(&value).is_err());
    }

    #[test]
    fn request_hash_excludes_only_transport_idempotency_fields() {
        let value = json!({"a":1,"idempotency_key":"key","input_sha256":"0".repeat(64)});
        assert_eq!(
            request_input_hash(&value).unwrap(),
            canonical_json_hash(&json!({"a":1}))
        );
    }

    #[test]
    fn native_high_durable_prepare_replays_after_runtime_restart_and_rejects_conflict() {
        let root = std::env::temp_dir().join(format!(
            "forgecad-native-high-durable-restart-{}",
            Uuid::new_v4()
        ));
        fs::create_dir_all(&root).expect("restart root");
        let database = root.join("runtime.sqlite");
        let cas = root.join("cas");

        let (request, first, cohort, durable_hashes) = {
            let runtime = Runtime::open_with_cas(&database, &cas).expect("initial Runtime");
            let project = runtime
                .create_project("Native High durable restart", json!({"profile":"test"}))
                .expect("project");
            let prepared = runtime
                .prepare_geometry_candidate(
                    &project.project_id,
                    None,
                    json!({
                        "typed":"geometry",
                        "geometry_program":authoring_program(&project.project_id)
                    }),
                )
                .expect("source GeometryProgram candidate");
            let candidate_id = prepared["candidate"]["candidate_id"]
                .as_str()
                .expect("candidate id")
                .to_owned();
            let candidate = runtime
                .candidate(&candidate_id)
                .expect("candidate query")
                .expect("candidate");
            let evidence = runtime
                .store
                .get_geometry_candidate_evidence(&candidate_id)
                .expect("evidence query")
                .expect("geometry evidence");
            let source_artifact_id = candidate
                .prepared_object_id
                .clone()
                .expect("source artifact id");
            let source_artifact_object_sha256 = candidate
                .prepared_object_sha256
                .clone()
                .expect("source artifact object SHA");
            let readback = runtime
                .artifact_readback(&source_artifact_object_sha256, &candidate_id)
                .expect("source ArtifactReadback");
            let source_artifact_readback_sha256 = readback["canonical_sha256"]
                .as_str()
                .expect("source ArtifactReadback SHA")
                .to_owned();
            let projection_request = json!({
                "schema_version":"AuthoringMeshRequest@1",
                "project_id":project.project_id,
                "candidate_id":candidate_id,
                "artifact_id":source_artifact_object_sha256,
                "artifact_readback_sha256":source_artifact_readback_sha256,
                "program_sha256":evidence.geometry_program_sha256,
                "operator_catalog_sha256":evidence.operator_catalog_sha256,
                "readback_config_sha256":evidence.readback_config_sha256,
                "authoring_node_id":"native-high-restart-panel",
                "part_id":"native-high-restart-panel",
                "authoring_mesh_policy_sha256":"aa72cadabba90ddb43dd0014cfa434ab9b13f4e072b09258072f37334c72e709",
                "max_response_bytes":1048576
            });
            let projection = crate::authoring_mesh::get(&runtime, &projection_request)
                .expect("source AuthoringMesh projection");
            let source_lineage_sha256 = projection["lineage"]["lineage_sha256"]
                .as_str()
                .expect("source lineage SHA")
                .to_owned();
            let base_version_id = candidate
                .base_version_id
                .clone()
                .map(Value::String)
                .unwrap_or(Value::Null);
            let expected_canonical = expected_canonical_mesh(
                &projection,
                &project.project_id,
                &candidate_id,
                &candidate.canonical_sha256,
                base_version_id.clone(),
                &evidence.geometry_program_object_sha256,
                &evidence.geometry_program_sha256,
                &source_artifact_object_sha256,
                &source_artifact_object_sha256,
                &evidence.artifact_readback_object_sha256,
                &source_artifact_readback_sha256,
                &source_lineage_sha256,
            );
            let mut source_request = json!({
                "schema_version":"AuthoringMeshPrepareRequest@1",
                "project_id":project.project_id,
                "source_candidate_id":candidate_id,
                "source_candidate_state_sha256":candidate.canonical_sha256,
                "base_version_id":base_version_id,
                "authoring_node_id":"native-high-restart-panel",
                "part_id":"native-high-restart-panel",
                "source_program_object_sha256":evidence.geometry_program_object_sha256,
                "source_program_sha256":evidence.geometry_program_sha256,
                "source_artifact_id":source_artifact_id,
                "source_artifact_object_sha256":source_artifact_object_sha256,
                "source_artifact_sha256":source_artifact_object_sha256,
                "source_artifact_readback_object_sha256":evidence.artifact_readback_object_sha256,
                "source_artifact_readback_sha256":source_artifact_readback_sha256,
                "source_lineage_sha256":source_lineage_sha256,
                "expected_canonical_mesh_sha256":expected_canonical["canonical_sha256"],
                "idempotency_key":"native-high-source-once",
                "max_response_bytes":1048576,
                "runtime_write_performed":false,
                "writer_policy":"forgecad-runtime-only-state-writer@1",
                "canonicalization_policy":CANONICALIZATION_POLICY,
                "input_sha256":""
            });
            source_request["input_sha256"] = Value::String(canonical_json_hash(&source_request));
            let source = runtime
                .authoring_mesh_durable_prepare(&source_request)
                .expect("durable AuthoringMesh source");
            assert_eq!(source["canonical_mesh"], expected_canonical);

            let high_request = high_request(
                source["canonical_mesh"].clone(),
                &candidate_id,
                &candidate.canonical_sha256,
            );
            let request = native_prepare_request(
                &project.project_id,
                &candidate_id,
                &candidate.canonical_sha256,
                source["base_version_id"].clone(),
                source["canonical_mesh_id"]
                    .as_str()
                    .expect("source mesh id"),
                source["canonical_mesh_object_sha256"]
                    .as_str()
                    .expect("source mesh object SHA"),
                source["canonical_mesh_sha256"]
                    .as_str()
                    .expect("source mesh SHA"),
                high_request,
                "native-high-durable-once",
            );
            let first = runtime
                .native_high_durable_prepare(request.clone())
                .expect("Native High durable prepare");
            assert_eq!(first["replayed"], false);
            assert_eq!(first["restart_hash_verified"], true);
            assert_eq!(first["replay_count"], 2);
            assert_eq!(first["replay_byte_exact"], true);
            assert_eq!(first["source_only"], true);
            assert_eq!(first["runtime_write_performed"], true);
            assert_eq!(first["production_stage_advanced"], false);
            assert_eq!(first["candidate_confirmed"], false);
            assert_eq!(first["version_created"], false);
            assert_eq!(first["export_performed"], false);
            assert_eq!(first["quality_status"], "structural_only");
            assert!(first["glb_sha256"].as_str().is_some_and(is_sha256));
            assert!(first["glb_readback_sha256"].as_str().is_some_and(is_sha256));
            let cohort = first["high_worker_build_cohort_sha256"]
                .as_str()
                .expect("same High Worker cohort")
                .to_owned();
            assert!(is_sha256(&cohort));
            let record = runtime
                .store
                .get_native_high_durable(&project.project_id, "native-high-durable-once")
                .expect("Native High durable record query")
                .expect("Native High durable record");
            let durable_hashes = vec![
                record.source_canonical_mesh_object_sha256.clone(),
                record.detail_graph_object_sha256.clone(),
                record.high_mesh_artifact_object_sha256.clone(),
                record.high_artifact_object_sha256.clone(),
                record.high_artifact_readback_object_sha256.clone(),
                record.link_object_sha256.clone(),
                record.canonical_sha256.clone(),
            ];
            let replay = runtime
                .native_high_durable_prepare(request.clone())
                .expect("same-key Native High replay");
            assert_eq!(replay["replayed"], true);
            assert_eq!(replay["glb_sha256"], first["glb_sha256"]);
            assert_eq!(
                replay["high_worker_build_cohort_sha256"],
                first["high_worker_build_cohort_sha256"]
            );
            drop(runtime);
            (request, first, cohort, durable_hashes)
        };

        let reopened = Runtime::open_with_cas(&database, &cas).expect("reopened Runtime");
        let get_request = {
            let mut value = json!({
                "schema_version":GET_SCHEMA,
                "operation":GET_OPERATION,
                "project_id":request["project_id"],
                "candidate_id":request["candidate_id"],
                "candidate_state_sha256":request["candidate_state_sha256"],
                "base_version_id":request["base_version_id"],
                "link_id":first["link_id"],
                "link_object_sha256":first["link_object_sha256"],
                "source_authoring_mesh_id":request["source_authoring_mesh_id"],
                "source_authoring_mesh_sha256":request["source_authoring_mesh_sha256"],
                "detail_graph_canonical_sha256":first["detail_graph_canonical_sha256"],
                "artifact_id":first["artifact_id"],
                "artifact_sha256":first["artifact_sha256"],
                "glb_sha256":first["glb_sha256"],
                "idempotency_key":request["idempotency_key"],
                "source_only":true,
                "writer_policy":WRITER_POLICY,
                "runtime_write_performed":false,
                "persistent_user_data_touched":false,
                "input_sha256":""
            });
            value["input_sha256"] = Value::String(canonical_json_hash(&value));
            value
        };
        let get = reopened
            .native_high_durable_get(get_request)
            .expect("Native High durable get after restart");
        assert_eq!(get["replayed"], true);
        assert_eq!(get["restart_hash_verified"], true);
        assert_eq!(get["glb_sha256"], first["glb_sha256"]);
        assert_eq!(get["glb_readback_sha256"], first["glb_readback_sha256"]);
        assert_eq!(
            get["high_worker_build_cohort_sha256"],
            Value::String(cohort)
        );
        assert_eq!(get["replay_count"], 2);
        assert_eq!(get["replay_byte_exact"], true);
        for field in [
            "production_stage_advanced",
            "candidate_confirmed",
            "version_created",
            "export_performed",
        ] {
            assert_eq!(get[field], false, "{field} must remain false");
        }
        assert!(reopened
            .versions(Some(request["project_id"].as_str().unwrap()))
            .unwrap()
            .is_empty());
        let reopened_hashes = {
            let record = reopened
                .store
                .get_native_high_durable(
                    request["project_id"].as_str().unwrap(),
                    request["idempotency_key"].as_str().unwrap(),
                )
                .unwrap()
                .unwrap();
            vec![
                record.source_canonical_mesh_object_sha256,
                record.detail_graph_object_sha256,
                record.high_mesh_artifact_object_sha256,
                record.high_artifact_object_sha256,
                record.high_artifact_readback_object_sha256,
                record.link_object_sha256,
                record.canonical_sha256,
            ]
        };
        assert_eq!(reopened_hashes, durable_hashes);

        let mut conflict = request.clone();
        conflict["high_mesh_request"]["detail_graph"]["nodes"][0]["center_m"] =
            json!([0.0, 0.0, 3.0]);
        conflict["high_mesh_request"]["detail_graph_canonical_sha256"] = Value::String(
            canonical_json_hash(&conflict["high_mesh_request"]["detail_graph"]),
        );
        conflict["high_mesh_request"]["canonical_sha256"] = Value::String(String::new());
        conflict["high_mesh_request"]["canonical_sha256"] =
            Value::String(canonical_json_hash(&conflict["high_mesh_request"]));
        conflict["high_mesh_request_sha256"] =
            conflict["high_mesh_request"]["canonical_sha256"].clone();
        conflict["input_sha256"] = Value::String(request_input_hash(&conflict).unwrap());
        let error = reopened
            .native_high_durable_prepare(conflict)
            .expect_err("same idempotency key must reject conflicting input");
        assert!(error
            .to_string()
            .contains("idempotency key is bound to another input"));
        drop(reopened);
        fs::remove_dir_all(root).expect("restart fixture cleanup");
    }
}
