//! Runtime-owned materialization of one durable direct-V2 High bridge into a
//! bounded embedded GLB.  The public request carries identities only; Runtime
//! reloads and validates every source/result/CAS binding before invoking the
//! fixed High Worker and committing the GLB, durable readback and receipt.

use super::{
    canonical_json_bytes, canonical_json_hash, geometry_worker, is_opaque_id, is_sha256,
    sha256_hex, Runtime, RuntimeError,
};
use base64::Engine;
use forgecad_store::{
    AuthoringMeshV2HighArtifactCasBundle, AuthoringMeshV2HighArtifactCommit,
    AuthoringMeshV2HighArtifactStoreRecord, AuthoringMeshV2HighBridgeStoreRecord, CasObject,
    AUTHORING_MESH_V2_HIGH_ARTIFACT_CANONICALIZATION_POLICY,
    AUTHORING_MESH_V2_HIGH_ARTIFACT_GLB_MIME, AUTHORING_MESH_V2_HIGH_ARTIFACT_GLB_OBJECT_KIND,
    AUTHORING_MESH_V2_HIGH_ARTIFACT_HASH_POLICY, AUTHORING_MESH_V2_HIGH_ARTIFACT_JSON_MIME,
    AUTHORING_MESH_V2_HIGH_ARTIFACT_MAX_GLB_BYTES, AUTHORING_MESH_V2_HIGH_ARTIFACT_MAX_JSON_BYTES,
    AUTHORING_MESH_V2_HIGH_ARTIFACT_READBACK_OBJECT_KIND,
    AUTHORING_MESH_V2_HIGH_ARTIFACT_RECEIPT_OBJECT_KIND,
    AUTHORING_MESH_V2_HIGH_ARTIFACT_RECORD_SCHEMA_VERSION,
    AUTHORING_MESH_V2_HIGH_ARTIFACT_WRITER_POLICY, AUTHORING_MESH_V2_HIGH_JSON_MIME,
    AUTHORING_MESH_V2_HIGH_MAX_JSON_BYTES, AUTHORING_MESH_V2_HIGH_READBACK_OBJECT_KIND,
    AUTHORING_MESH_V2_HIGH_RESULT_OBJECT_KIND,
};
use serde_json::{json, Map, Value};
use std::collections::BTreeSet;

const MAIN_SCHEMA: &str = "AuthoringMeshV2HighArtifact@1";
const PREPARE_SCHEMA: &str = "AuthoringMeshV2HighArtifactPrepareRequest@1";
const GET_SCHEMA: &str = "AuthoringMeshV2HighArtifactGetRequest@1";
const RESULT_SCHEMA: &str = "AuthoringMeshV2HighArtifactResult@1";
const PREPARE_OPERATION: &str = "authoring_mesh_v2_high_artifact_prepare";
const GET_OPERATION: &str = "authoring_mesh_v2_high_artifact_get";
const WORKER_READBACK_SCHEMA: &str = "AuthoringMeshV2HighGlbReadback@1";
const STRICT_READBACK_SCHEMA: &str = "AuthoringMeshV2HighArtifactReadback@1";
const DURABLE_READBACK_SCHEMA: &str = "AuthoringMeshV2HighArtifactStoreReadback@1";
const RECEIPT_SCHEMA: &str = "AuthoringMeshV2HighArtifactReceipt@1";
const REQUEST_CANONICALIZATION: &str = "canonical-json-sha256-excluding-input-sha256@1";
const MAIN_CANONICALIZATION: &str = "canonical-json-sha256-excluding-canonical-sha256@1";
const MAX_RESPONSE_BYTES: u64 = 1024 * 1024;
const SOURCE_SCOPE: &str = "single-v2-revision-part@1";
const ARTIFACT_POLICY: &str = "authoring-mesh-v2-high-bridge-to-low-glb-adapter@1";
const LOW_ARTIFACT_KIND: &str = "production-weapon-high-artifact-glb";
const LOW_READBACK_KIND: &str = "native-high-glb-materialize-result";
const LOW_SOURCE_SCHEMA: &str = "HighMeshArtifact@1";
const LIMITATIONS: [&str; 6] = [
    "SINGLE_V2_REVISION_PART_ONLY",
    "RUNTIME_DERIVES_GLTF_FROM_HIGH_BRIDGE",
    "LOW_CONSUMPTION_REQUIRES_STRICT_GLTF_READBACK",
    "NO_CALLER_SUPPLIED_GLTF_BYTES",
    "NO_STAGE_ADVANCEMENT",
    "NO_VISUAL_OR_HUMAN_ACCEPTANCE",
];

const PREPARE_FIELDS: &[&str] = &[
    "schema_version",
    "operation",
    "project_id",
    "high_artifact_id",
    "high_bridge_id",
    "high_bridge_sha256",
    "high_bridge_object_sha256",
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
    "high_artifact_id",
    "high_artifact_sha256",
    "high_artifact_object_sha256",
    "high_artifact_readback_sha256",
    "high_artifact_readback_object_sha256",
    "high_artifact_receipt_sha256",
    "high_artifact_receipt_object_sha256",
    "high_bridge_id",
    "high_bridge_sha256",
    "high_bridge_object_sha256",
    "max_response_bytes",
    "runtime_write_performed",
    "persistent_user_data_touched",
    "writer_policy",
    "canonicalization_policy",
    "input_sha256",
];

#[derive(Debug, Clone)]
struct PrepareRequest {
    project_id: String,
    artifact_id: String,
    bridge_id: String,
    bridge_sha256: String,
    bridge_object_sha256: String,
    idempotency_key: String,
    input_sha256: String,
}

#[derive(Debug, Clone)]
struct GetRequest {
    project_id: String,
    artifact_id: String,
    artifact_sha256: String,
    artifact_object_sha256: String,
    readback_sha256: String,
    readback_object_sha256: String,
    receipt_sha256: String,
    receipt_object_sha256: String,
    bridge_id: String,
    bridge_sha256: String,
    bridge_object_sha256: String,
}

fn invalid(message: impl Into<String>) -> RuntimeError {
    RuntimeError::InvalidInput(format!(
        "AUTHORING_MESH_V2_HIGH_ARTIFACT_INVALID: {}",
        message.into()
    ))
}

fn exact_object<'a>(
    value: &'a Value,
    fields: &[&str],
) -> Result<&'a Map<String, Value>, RuntimeError> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid("request must be an object"))?;
    let expected = fields.iter().copied().collect::<BTreeSet<_>>();
    let actual = object.keys().map(String::as_str).collect::<BTreeSet<_>>();
    if expected != actual {
        return Err(invalid("request fields differ from the closed envelope"));
    }
    Ok(object)
}

fn text<'a>(object: &'a Map<String, Value>, field: &str) -> Result<&'a str, RuntimeError> {
    object
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid(format!("{field} must be a string")))
}

fn id(object: &Map<String, Value>, field: &str) -> Result<String, RuntimeError> {
    let value = text(object, field)?;
    if !is_opaque_id(value) {
        return Err(invalid(format!("{field} must be an opaque ID")));
    }
    Ok(value.to_owned())
}

fn sha(object: &Map<String, Value>, field: &str) -> Result<String, RuntimeError> {
    let value = text(object, field)?;
    if !is_sha256(value) {
        return Err(invalid(format!("{field} must be a lowercase SHA-256")));
    }
    Ok(value.to_owned())
}

fn validate_common(
    value: &Value,
    object: &Map<String, Value>,
    schema: &str,
    operation: &str,
    persistent: bool,
) -> Result<String, RuntimeError> {
    if object.get("schema_version").and_then(Value::as_str) != Some(schema)
        || object.get("operation").and_then(Value::as_str) != Some(operation)
        || object.get("max_response_bytes").and_then(Value::as_u64) != Some(MAX_RESPONSE_BYTES)
        || object.get("runtime_write_performed") != Some(&Value::Bool(false))
        || (persistent && object.get("persistent_user_data_touched") != Some(&Value::Bool(false)))
        || object.get("writer_policy").and_then(Value::as_str)
            != Some(AUTHORING_MESH_V2_HIGH_ARTIFACT_WRITER_POLICY)
        || object
            .get("canonicalization_policy")
            .and_then(Value::as_str)
            != Some(REQUEST_CANONICALIZATION)
    {
        return Err(invalid("request marker, bound or policy differs"));
    }
    let input = sha(object, "input_sha256")?;
    let mut preimage = value.clone();
    preimage["input_sha256"] = Value::String(String::new());
    if canonical_json_hash(&preimage) != input {
        return Err(invalid("input_sha256 does not bind the request"));
    }
    Ok(input)
}

fn parse_prepare(value: &Value) -> Result<PrepareRequest, RuntimeError> {
    let object = exact_object(value, PREPARE_FIELDS)?;
    let input = validate_common(value, object, PREPARE_SCHEMA, PREPARE_OPERATION, false)?;
    Ok(PrepareRequest {
        project_id: id(object, "project_id")?,
        artifact_id: id(object, "high_artifact_id")?,
        bridge_id: id(object, "high_bridge_id")?,
        bridge_sha256: sha(object, "high_bridge_sha256")?,
        bridge_object_sha256: sha(object, "high_bridge_object_sha256")?,
        idempotency_key: id(object, "idempotency_key")?,
        input_sha256: input,
    })
}

fn parse_get(value: &Value) -> Result<GetRequest, RuntimeError> {
    let object = exact_object(value, GET_FIELDS)?;
    let _ = validate_common(value, object, GET_SCHEMA, GET_OPERATION, true)?;
    Ok(GetRequest {
        project_id: id(object, "project_id")?,
        artifact_id: id(object, "high_artifact_id")?,
        artifact_sha256: sha(object, "high_artifact_sha256")?,
        artifact_object_sha256: sha(object, "high_artifact_object_sha256")?,
        readback_sha256: sha(object, "high_artifact_readback_sha256")?,
        readback_object_sha256: sha(object, "high_artifact_readback_object_sha256")?,
        receipt_sha256: sha(object, "high_artifact_receipt_sha256")?,
        receipt_object_sha256: sha(object, "high_artifact_receipt_object_sha256")?,
        bridge_id: id(object, "high_bridge_id")?,
        bridge_sha256: sha(object, "high_bridge_sha256")?,
        bridge_object_sha256: sha(object, "high_bridge_object_sha256")?,
    })
}

fn read_json(runtime: &Runtime, hash: &str, kind: &str, max: u64) -> Result<Value, RuntimeError> {
    let object = runtime
        .store
        .get_object(hash)?
        .ok_or_else(|| invalid("required CAS JSON object is unavailable"))?;
    if object.mime != AUTHORING_MESH_V2_HIGH_JSON_MIME
        || object.kind != kind
        || object.size_bytes == 0
        || object.size_bytes > max
    {
        return Err(invalid("required CAS JSON metadata differs"));
    }
    let bytes = runtime.cas_read_bounded(hash, max)?;
    if sha256_hex(&bytes) != hash {
        return Err(invalid("required CAS JSON bytes differ"));
    }
    let value: Value = serde_json::from_slice(&bytes)
        .map_err(|error| invalid(format!("required CAS JSON is invalid: {error}")))?;
    if canonical_json_bytes(&value).map_err(|error| invalid(error.to_string()))? != bytes {
        return Err(invalid("required CAS JSON is not canonical"));
    }
    Ok(value)
}

fn load_bridge(
    runtime: &Runtime,
    project_id: &str,
    bridge_id: &str,
    bridge_sha256: &str,
    bridge_object_sha256: &str,
) -> Result<AuthoringMeshV2HighBridgeStoreRecord, RuntimeError> {
    let bridge = runtime
        .store
        .get_authoring_mesh_v2_high_bridge_by_id(project_id, bridge_id)?
        .ok_or_else(|| invalid("durable High bridge is unavailable"))?;
    if bridge.bridge_sha256 != bridge_sha256
        || bridge.bridge_object_sha256 != bridge_object_sha256
        || bridge.project_id != project_id
    {
        return Err(invalid("durable High bridge identity differs"));
    }
    Ok(bridge)
}

fn worker_request(high_result: Value, bridge: &AuthoringMeshV2HighBridgeStoreRecord) -> Value {
    let mut request = json!({
        "schema_version": forgecad_worker_protocol::AUTHORING_MESH_V2_HIGH_ARTIFACT_MATERIALIZE_REQUEST_SCHEMA_VERSION,
        "operation": forgecad_worker_protocol::AUTHORING_MESH_V2_HIGH_ARTIFACT_MATERIALIZE_OPERATION,
        "high_result": high_result,
        "high_result_sha256": bridge.high_result_sha256,
        "source_high_worker_build_cohort_sha256": bridge.high_worker_build_cohort_sha256,
        "canonical_sha256": ""
    });
    request["canonical_sha256"] = Value::String(canonical_json_hash(&request));
    request
}

fn string_array(value: &Value, field: &str) -> Result<Vec<String>, RuntimeError> {
    let values = value
        .get(field)
        .and_then(Value::as_array)
        .ok_or_else(|| invalid(format!("Worker readback {field} is missing")))?;
    let mut seen = BTreeSet::new();
    let mut output = Vec::with_capacity(values.len());
    for value in values {
        let value = value
            .as_str()
            .filter(|value| is_opaque_id(value))
            .ok_or_else(|| invalid(format!("Worker readback {field} contains an invalid ID")))?;
        if !seen.insert(value.to_owned()) {
            return Err(invalid(format!(
                "Worker readback {field} contains duplicates"
            )));
        }
        output.push(value.to_owned());
    }
    if output.is_empty() {
        return Err(invalid(format!("Worker readback {field} is empty")));
    }
    Ok(output)
}

fn public_strict_readback(
    glb: &[u8],
    worker: &Value,
    bridge: &AuthoringMeshV2HighBridgeStoreRecord,
    source_program: &Value,
) -> Result<Value, RuntimeError> {
    let worker_readback = worker
        .get("strict_readback")
        .ok_or_else(|| invalid("Worker strict readback is missing"))?;
    let high_mesh_id = format!("high-mesh-{}", &bridge.high_result_sha256[..24]);
    let glb_sha256 = sha256_hex(glb);
    if worker_readback
        .get("schema_version")
        .and_then(Value::as_str)
        != Some(WORKER_READBACK_SCHEMA)
        || worker_readback.get("glb_sha256").and_then(Value::as_str) != Some(glb_sha256.as_str())
        || worker_readback.get("artifact_id").and_then(Value::as_str) != Some(high_mesh_id.as_str())
        || worker_readback
            .get("artifact_sha256")
            .and_then(Value::as_str)
            != Some(bridge.high_result_sha256.as_str())
        || worker_readback.get("mesh_id").and_then(Value::as_str) != Some(bridge.mesh_id.as_str())
        || worker_readback.get("lineage_id").and_then(Value::as_str)
            != Some(bridge.lineage_id.as_str())
        || worker_readback.get("revision_id").and_then(Value::as_str)
            != Some(bridge.revision_id.as_str())
        || worker_readback
            .get("revision_index")
            .and_then(Value::as_u64)
            != Some(bridge.revision_index)
        || worker_readback
            .get("revision_sha256")
            .and_then(Value::as_str)
            != Some(bridge.revision_sha256.as_str())
        || worker_readback
            .get("source_mesh_sha256")
            .and_then(Value::as_str)
            != Some(bridge.high_projected_source_mesh_sha256.as_str())
        || worker_readback
            .get("high_evaluation_sha256")
            .and_then(Value::as_str)
            != Some(bridge.high_evaluation_sha256.as_str())
        || worker_readback
            .get("high_result_sha256")
            .and_then(Value::as_str)
            != Some(bridge.high_result_sha256.as_str())
        || worker_readback
            .get("high_readback_sha256")
            .and_then(Value::as_str)
            != Some(bridge.high_readback_sha256.as_str())
        || worker_readback
            .get("high_worker_build_cohort_sha256")
            .and_then(Value::as_str)
            != Some(bridge.high_worker_build_cohort_sha256.as_str())
    {
        return Err(invalid("Worker strict readback source binding differs"));
    }
    let part_ids = string_array(worker_readback, "part_ids")?;
    let source_node_ids = string_array(worker_readback, "source_node_ids")?;
    let material_zone_ids = string_array(worker_readback, "material_zone_ids")?;
    let outputs = source_program
        .get("part_outputs")
        .and_then(Value::as_array)
        .filter(|values| !values.is_empty() && values.len() <= 128)
        .ok_or_else(|| invalid("materialized source program Part outputs are missing"))?;
    if outputs.len() != part_ids.len()
        || outputs.len()
            != worker_readback
                .get("primitive_count")
                .and_then(Value::as_u64)
                .and_then(|value| usize::try_from(value).ok())
                .unwrap_or_default()
    {
        return Err(invalid(
            "Worker strict readback Part/primitive counts differ",
        ));
    }
    let local = super::native_high_glb_readback::inspect_authoring_mesh_v2_high_glb(glb)
        .map_err(|error| invalid(error.to_string()))?;
    let base_primitive_count = local
        .get("base_primitive_count")
        .and_then(Value::as_u64)
        .ok_or_else(|| invalid("independent Runtime base primitive count is missing"))?;
    let detail_primitive_count = local
        .get("detail_primitive_count")
        .and_then(Value::as_u64)
        .ok_or_else(|| invalid("independent Runtime detail primitive count is missing"))?;
    let base_triangle_count = local
        .get("base_triangle_count")
        .and_then(Value::as_u64)
        .ok_or_else(|| invalid("independent Runtime base triangle count is missing"))?;
    let detail_triangle_count = local
        .get("detail_triangle_count")
        .and_then(Value::as_u64)
        .ok_or_else(|| invalid("independent Runtime detail triangle count is missing"))?;
    let primitive_bindings = local
        .get("primitive_bindings")
        .and_then(Value::as_array)
        .filter(|values| values.len() == part_ids.len())
        .ok_or_else(|| invalid("independent Runtime primitive bindings are missing"))?;
    if local.get("glb_sha256").and_then(Value::as_str) != Some(glb_sha256.as_str())
        || local.get("source_artifact_id").and_then(Value::as_str) != Some(high_mesh_id.as_str())
        || local.get("source_artifact_sha256").and_then(Value::as_str)
            != Some(bridge.high_result_sha256.as_str())
        || local.get("part_ids") != Some(&json!(part_ids))
        || local.get("source_node_ids") != Some(&json!(source_node_ids))
        || local.get("material_zone_ids") != Some(&json!(material_zone_ids))
        || base_primitive_count != part_ids.len() as u64
        || detail_primitive_count != 0
    {
        return Err(invalid("independent Runtime GLB lineage differs"));
    }
    let triangles = local
        .get("triangle_count")
        .and_then(Value::as_u64)
        .ok_or_else(|| invalid("independent Runtime triangle count is missing"))?;
    if worker_readback
        .get("triangle_count")
        .and_then(Value::as_u64)
        != Some(triangles)
        || worker_readback
            .get("primitive_count")
            .and_then(Value::as_u64)
            != Some(base_primitive_count + detail_primitive_count)
    {
        return Err(invalid("Worker and Runtime triangle counts differ"));
    }
    let mut bindings = Vec::with_capacity(outputs.len());
    for (index, (output, primitive)) in outputs.iter().zip(primitive_bindings).enumerate() {
        let output_part_id = output
            .get("part_id")
            .and_then(Value::as_str)
            .filter(|value| is_opaque_id(value))
            .ok_or_else(|| invalid("materialized source Part id is invalid"))?;
        let output_nodes = output
            .get("input_node_ids")
            .and_then(Value::as_array)
            .filter(|values| !values.is_empty() && values.len() <= 16)
            .ok_or_else(|| invalid("materialized source Part node set is missing"))?;
        let primitive_part_id = primitive
            .get("part_id")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid("GLB primitive Part id is missing"))?;
        let primitive_nodes = primitive
            .get("source_node_ids")
            .and_then(Value::as_array)
            .ok_or_else(|| invalid("GLB primitive source node set is missing"))?;
        if primitive_part_id != output_part_id
            || primitive_nodes != output_nodes
            || part_ids.get(index).map(String::as_str) != Some(output_part_id)
            || primitive.get("material_zone_id") != output.get("material_zone_id")
        {
            return Err(invalid("GLB primitive Part/node/material binding differs"));
        }
        let output_material_zone_id = output
            .get("material_zone_id")
            .and_then(Value::as_str)
            .filter(|value| is_opaque_id(value))
            .ok_or_else(|| invalid("materialized source material zone is invalid"))?;
        let output_solid = output
            .get("solid")
            .and_then(Value::as_bool)
            .ok_or_else(|| invalid("materialized source solid flag is missing"))?;
        let triangle_count = primitive
            .get("triangle_count")
            .and_then(Value::as_u64)
            .filter(|value| *value > 0)
            .ok_or_else(|| invalid("GLB primitive triangle count is invalid"))?;
        let source_part_output_sha256 = canonical_json_hash(output);
        let owner_node_id = output_nodes
            .first()
            .and_then(Value::as_str)
            .ok_or_else(|| invalid("materialized source Part owner node is missing"))?;
        if output_part_id == bridge.part_id {
            if output_material_zone_id != bridge.material_zone_id || output_solid != bridge.solid {
                return Err(invalid("selected High bridge Part binding differs"));
            }
        }
        bindings.push(json!({
            "part_id": output_part_id,
            "source_node_id": owner_node_id,
            "material_zone_id": output_material_zone_id,
            "source_part_output_sha256": source_part_output_sha256,
            "solid": output_solid,
            "triangle_count": triangle_count
        }));
    }
    Ok(json!({
        "schema_version": STRICT_READBACK_SCHEMA,
        "glb_sha256": glb_sha256.clone(),
        "glb_object_sha256": glb_sha256,
        "source_artifact_id": high_mesh_id,
        "source_artifact_sha256": bridge.high_result_sha256,
        "part_ids": part_ids,
        "material_zone_ids": material_zone_ids,
        "source_node_ids": source_node_ids,
        "part_bindings": bindings,
        "base_primitive_count": base_primitive_count,
        "detail_primitive_count": detail_primitive_count,
        "base_triangle_count": base_triangle_count,
        "detail_triangle_count": detail_triangle_count,
        "triangle_count": triangles,
        "byte_length": glb.len(),
        "embedded_only": true,
        "external_uri": false,
        "scripts": false,
        "validator_status": "passed",
        "hard_gate_passed": true
    }))
}

fn semantic_object(
    mut value: Value,
    additional_identity: Option<&str>,
) -> Result<(Value, Vec<u8>, String), RuntimeError> {
    value["canonical_sha256"] = Value::String(String::new());
    if let Some(field) = additional_identity {
        value[field] = Value::String(String::new());
    }
    let semantic = canonical_json_hash(&value);
    value["canonical_sha256"] = Value::String(semantic.clone());
    if let Some(field) = additional_identity {
        value[field] = Value::String(semantic.clone());
    }
    let bytes = canonical_json_bytes(&value).map_err(|error| invalid(error.to_string()))?;
    if bytes.len() as u64 > AUTHORING_MESH_V2_HIGH_ARTIFACT_MAX_JSON_BYTES {
        return Err(invalid("durable JSON exceeds its byte bound"));
    }
    Ok((value, bytes, semantic))
}

fn durable_readback(
    record_seed: &AuthoringMeshV2HighArtifactStoreRecord,
    strict: &Value,
) -> Result<(Value, Vec<u8>, String), RuntimeError> {
    semantic_object(
        json!({
            "schema_version": DURABLE_READBACK_SCHEMA,
            "project_id": record_seed.project_id,
            "artifact_id": record_seed.artifact_id,
            "bridge_id": record_seed.bridge_id,
            "bridge_sha256": record_seed.bridge_sha256,
            "bridge_object_sha256": record_seed.bridge_object_sha256,
            "revision_id": record_seed.revision_id,
            "revision_index": record_seed.revision_index,
            "revision_sha256": record_seed.revision_sha256,
            "revision_object_sha256": record_seed.revision_object_sha256,
            "source_binding_id": record_seed.source_binding_id,
            "source_binding_sha256": record_seed.source_binding_sha256,
            "source_binding_object_sha256": record_seed.source_binding_object_sha256,
            "high_result_sha256": record_seed.high_result_sha256,
            "high_result_object_sha256": record_seed.high_result_object_sha256,
            "high_readback_sha256": record_seed.high_readback_sha256,
            "high_readback_object_sha256": record_seed.high_readback_object_sha256,
            "high_worker_algorithm_sha256": record_seed.high_worker_algorithm_sha256,
            "high_worker_build_cohort_sha256": record_seed.high_worker_build_cohort_sha256,
            "high_artifact_sha256": record_seed.high_artifact_sha256,
            "high_artifact_object_sha256": record_seed.high_artifact_object_sha256,
            "high_artifact_readback_sha256": "",
            "high_artifact_size_bytes": record_seed.high_artifact_size_bytes,
            "replay_count": 2,
            "replay_byte_exact": true,
            "non_destructive": true,
            "strict_readback": strict,
            "structural_status": "PASS_SOURCE_STRUCTURAL",
            "visual_status": "NOT_RUN",
            "human_status": "NOT_RUN",
            "engine_status": "NOT_RUN",
            "canonical_sha256": ""
        }),
        Some("high_artifact_readback_sha256"),
    )
}

fn receipt(
    record_seed: &AuthoringMeshV2HighArtifactStoreRecord,
    readback_sha256: &str,
    readback_object_sha256: &str,
) -> Result<(Value, Vec<u8>, String), RuntimeError> {
    semantic_object(
        json!({
            "schema_version": RECEIPT_SCHEMA,
            "project_id": record_seed.project_id,
            "artifact_id": record_seed.artifact_id,
            "bridge_id": record_seed.bridge_id,
            "bridge_sha256": record_seed.bridge_sha256,
            "bridge_object_sha256": record_seed.bridge_object_sha256,
            "revision_id": record_seed.revision_id,
            "revision_index": record_seed.revision_index,
            "revision_sha256": record_seed.revision_sha256,
            "revision_object_sha256": record_seed.revision_object_sha256,
            "source_binding_id": record_seed.source_binding_id,
            "source_binding_sha256": record_seed.source_binding_sha256,
            "source_binding_object_sha256": record_seed.source_binding_object_sha256,
            "high_result_sha256": record_seed.high_result_sha256,
            "high_result_object_sha256": record_seed.high_result_object_sha256,
            "high_readback_sha256": record_seed.high_readback_sha256,
            "high_readback_object_sha256": record_seed.high_readback_object_sha256,
            "high_worker_algorithm_sha256": record_seed.high_worker_algorithm_sha256,
            "high_worker_build_cohort_sha256": record_seed.high_worker_build_cohort_sha256,
            "high_artifact_sha256": record_seed.high_artifact_sha256,
            "high_artifact_object_sha256": record_seed.high_artifact_object_sha256,
            "high_artifact_readback_sha256": readback_sha256,
            "high_artifact_readback_object_sha256": readback_object_sha256,
            "receipt_sha256": "",
            "receipt_status": "prepared",
            "materialization_status": "prepared",
            "structural_status": "PASS_SOURCE_STRUCTURAL",
            "visual_status": "NOT_RUN",
            "human_status": "NOT_RUN",
            "engine_status": "NOT_RUN",
            "canonical_sha256": ""
        }),
        Some("receipt_sha256"),
    )
}

fn seed_record(
    request: &PrepareRequest,
    bridge: &AuthoringMeshV2HighBridgeStoreRecord,
    glb_sha256: &str,
    glb_size: u64,
) -> AuthoringMeshV2HighArtifactStoreRecord {
    AuthoringMeshV2HighArtifactStoreRecord {
        schema_version: AUTHORING_MESH_V2_HIGH_ARTIFACT_RECORD_SCHEMA_VERSION.to_owned(),
        project_id: request.project_id.clone(),
        artifact_id: request.artifact_id.clone(),
        bridge_id: bridge.bridge_id.clone(),
        bridge_sha256: bridge.bridge_sha256.clone(),
        bridge_object_sha256: bridge.bridge_object_sha256.clone(),
        source_binding_id: bridge.source_binding_id.clone(),
        source_binding_sha256: bridge.source_binding_sha256.clone(),
        source_binding_object_sha256: bridge.source_binding_object_sha256.clone(),
        mesh_id: bridge.mesh_id.clone(),
        lineage_id: bridge.lineage_id.clone(),
        revision_id: bridge.revision_id.clone(),
        revision_index: bridge.revision_index,
        revision_sha256: bridge.revision_sha256.clone(),
        revision_object_sha256: bridge.revision_object_sha256.clone(),
        materialized_candidate_id: bridge.materialized_candidate_id.clone(),
        materialized_candidate_state_sha256: bridge.materialized_candidate_state_sha256.clone(),
        materialized_program_sha256: bridge.materialized_program_sha256.clone(),
        materialized_program_object_sha256: bridge.materialized_program_object_sha256.clone(),
        representation_plan_sha256: bridge.representation_plan_sha256.clone(),
        source_node_id: bridge.source_node_id.clone(),
        part_id: bridge.part_id.clone(),
        material_zone_id: bridge.material_zone_id.clone(),
        solid: bridge.solid,
        high_execution_request_sha256: bridge.high_execution_request_sha256.clone(),
        high_evaluation_sha256: bridge.high_evaluation_sha256.clone(),
        high_result_sha256: bridge.high_result_sha256.clone(),
        high_result_object_sha256: bridge.high_result_object_sha256.clone(),
        high_readback_sha256: bridge.high_readback_sha256.clone(),
        high_readback_object_sha256: bridge.high_readback_object_sha256.clone(),
        high_worker_algorithm_sha256: bridge.high_worker_algorithm_sha256.clone(),
        high_worker_build_cohort_sha256: bridge.high_worker_build_cohort_sha256.clone(),
        high_replay_count: bridge.high_replay_count,
        high_replay_byte_exact: bridge.high_replay_byte_exact,
        high_non_destructive: bridge.high_non_destructive,
        high_source_vertex_count: bridge.high_source_vertex_count,
        high_source_triangle_count: bridge.high_source_triangle_count,
        high_evaluated_part_count: bridge.high_evaluated_part_count,
        high_evaluated_triangle_count: bridge.high_evaluated_triangle_count,
        high_artifact_sha256: glb_sha256.to_owned(),
        high_artifact_object_sha256: glb_sha256.to_owned(),
        high_artifact_size_bytes: glb_size,
        high_artifact_readback_sha256: "0".repeat(64),
        high_artifact_readback_object_sha256: "0".repeat(64),
        receipt_sha256: "0".repeat(64),
        receipt_object_sha256: "0".repeat(64),
        materialized_artifact_hash_policy: AUTHORING_MESH_V2_HIGH_ARTIFACT_HASH_POLICY.to_owned(),
        materialization_status: "prepared".to_owned(),
        structural_status: "PASS_SOURCE_STRUCTURAL".to_owned(),
        visual_status: "NOT_RUN".to_owned(),
        human_status: "NOT_RUN".to_owned(),
        engine_status: "NOT_RUN".to_owned(),
        high_mesh_created: true,
        high_stage_unlocked: false,
        production_stage_advanced: false,
        candidate_confirmed: false,
        version_created: false,
        export_performed: false,
        runtime_write_performed: true,
        persistent_user_data_touched: true,
        writer_policy: AUTHORING_MESH_V2_HIGH_ARTIFACT_WRITER_POLICY.to_owned(),
        canonicalization_policy: AUTHORING_MESH_V2_HIGH_ARTIFACT_CANONICALIZATION_POLICY.to_owned(),
        canonical_sha256: "0".repeat(64),
        request_input_sha256: request.input_sha256.clone(),
        idempotency_key: request.idempotency_key.clone(),
        created_at: super::authoring_mesh_v2_high_bridge::contract_timestamp(),
    }
}

fn record_canonical(
    record: &AuthoringMeshV2HighArtifactStoreRecord,
) -> Result<String, RuntimeError> {
    let mut value = serde_json::to_value(record).map_err(|error| invalid(error.to_string()))?;
    value["canonical_sha256"] = Value::String(String::new());
    Ok(canonical_json_hash(&value))
}

fn stage(
    runtime: &Runtime,
    reservation: &forgecad_store::CasReservation,
    bytes: &[u8],
    expected: &str,
    mime: &str,
    kind: &str,
    objects: &mut Vec<CasObject>,
) -> Result<CasObject, RuntimeError> {
    let object = runtime.store.put_object_reserved(
        reservation,
        bytes,
        Some(expected),
        mime,
        kind,
        &super::authoring_mesh_v2_high_bridge::contract_timestamp(),
    )?;
    objects.push(object.clone());
    Ok(object)
}

fn cleanup(runtime: &Runtime, reservation: &forgecad_store::CasReservation, objects: &[CasObject]) {
    for object in objects.iter().rev() {
        let _ = runtime
            .store
            .release_cas_reservation_object(reservation, object, true);
    }
}

fn read_durable_json(runtime: &Runtime, hash: &str, kind: &str) -> Result<Value, RuntimeError> {
    read_json(
        runtime,
        hash,
        kind,
        AUTHORING_MESH_V2_HIGH_ARTIFACT_MAX_JSON_BYTES,
    )
}

fn main_value(
    runtime: &Runtime,
    record: &AuthoringMeshV2HighArtifactStoreRecord,
    bridge: &AuthoringMeshV2HighBridgeStoreRecord,
) -> Result<Value, RuntimeError> {
    let glb = runtime.cas_read_bounded(
        &record.high_artifact_object_sha256,
        record.high_artifact_size_bytes,
    )?;
    if sha256_hex(&glb) != record.high_artifact_sha256 {
        return Err(invalid("durable High GLB hash differs"));
    }
    let durable = read_durable_json(
        runtime,
        &record.high_artifact_readback_object_sha256,
        AUTHORING_MESH_V2_HIGH_ARTIFACT_READBACK_OBJECT_KIND,
    )?;
    let strict = durable
        .get("strict_readback")
        .cloned()
        .ok_or_else(|| invalid("durable strict readback is missing"))?;
    let part_ids = strict
        .get("part_ids")
        .cloned()
        .ok_or_else(|| invalid("durable Part inventory is missing"))?;
    let material_zone_ids = strict
        .get("material_zone_ids")
        .cloned()
        .ok_or_else(|| invalid("durable material inventory is missing"))?;
    let part_bindings = strict
        .get("part_bindings")
        .cloned()
        .ok_or_else(|| invalid("durable Part bindings are missing"))?;
    let inventory_sha = canonical_json_hash(&json!({
        "part_ids": part_ids,
        "material_zone_ids": material_zone_ids,
        "part_bindings": part_bindings
    }));
    let high_mesh_id = format!("high-mesh-{}", &record.high_result_sha256[..24]);
    let mut main = json!({
        "schema_version": MAIN_SCHEMA,
        "high_artifact_id": record.artifact_id,
        "high_artifact_sha256": record.high_artifact_sha256,
        "high_artifact_object_sha256": record.high_artifact_object_sha256,
        "high_artifact_receipt_sha256": record.receipt_sha256,
        "high_artifact_receipt_object_sha256": record.receipt_object_sha256,
        "project_id": record.project_id,
        "source_scope": SOURCE_SCOPE,
        "high_bridge_id": record.bridge_id,
        "high_bridge_sha256": record.bridge_sha256,
        "high_bridge_object_sha256": record.bridge_object_sha256,
        "source_candidate_id": record.materialized_candidate_id,
        "source_candidate_state_sha256": record.materialized_candidate_state_sha256,
        "source_revision_schema_version": "AuthoringMeshRevision@2",
        "source_mesh_id": record.mesh_id,
        "source_lineage_id": record.lineage_id,
        "source_revision_id": record.revision_id,
        "source_revision_index": record.revision_index,
        "source_revision_sha256": record.revision_sha256,
        "source_revision_object_sha256": record.revision_object_sha256,
        "source_binding_id": record.source_binding_id,
        "source_binding_sha256": record.source_binding_sha256,
        "source_binding_object_sha256": record.source_binding_object_sha256,
        "materialized_candidate_id": record.materialized_candidate_id,
        "materialized_candidate_state_sha256": record.materialized_candidate_state_sha256,
        "materialized_program_sha256": record.materialized_program_sha256,
        "materialized_program_object_sha256": record.materialized_program_object_sha256,
        "materialized_artifact_id": bridge.materialized_artifact_id,
        "materialized_artifact_sha256": bridge.materialized_artifact_sha256,
        "materialized_artifact_object_sha256": bridge.materialized_artifact_object_sha256,
        "materialized_artifact_readback_sha256": bridge.materialized_artifact_readback_sha256,
        "materialized_artifact_readback_object_sha256": bridge.materialized_artifact_readback_object_sha256,
        "representation_plan_sha256": record.representation_plan_sha256,
        "source_node_id": record.source_node_id,
        "source_part_id": record.part_id,
        "source_material_zone_id": record.material_zone_id,
        "source_solid": record.solid,
        "source_part_output_sha256": bridge.source_part_output_sha256,
        "preserved_part_ids": bridge.preserved_part_ids,
        "high_mesh_artifact_id": high_mesh_id,
        "high_mesh_artifact_sha256": record.high_result_sha256,
        "high_mesh_artifact_object_sha256": record.high_result_object_sha256,
        "high_execution_request_sha256": record.high_execution_request_sha256,
        "high_evaluation_sha256": record.high_evaluation_sha256,
        "high_result_sha256": record.high_result_sha256,
        "high_result_object_sha256": record.high_result_object_sha256,
        "high_readback_sha256": record.high_readback_sha256,
        "high_readback_object_sha256": record.high_readback_object_sha256,
        "high_worker_algorithm_sha256": record.high_worker_algorithm_sha256,
        "high_worker_build_cohort_sha256": record.high_worker_build_cohort_sha256,
        "high_replay_count": record.high_replay_count,
        "high_replay_byte_exact": record.high_replay_byte_exact,
        "high_non_destructive": record.high_non_destructive,
        "high_source_vertex_count": record.high_source_vertex_count,
        "high_source_triangle_count": record.high_source_triangle_count,
        "high_evaluated_part_count": record.high_evaluated_part_count,
        "high_evaluated_triangle_count": record.high_evaluated_triangle_count,
        "high_part_ids": part_ids,
        "high_material_zone_ids": material_zone_ids,
        "high_part_inventory_sha256": inventory_sha,
        "high_artifact_policy": ARTIFACT_POLICY,
        "high_artifact_kind": AUTHORING_MESH_V2_HIGH_ARTIFACT_GLB_OBJECT_KIND,
        "high_artifact_mime": AUTHORING_MESH_V2_HIGH_ARTIFACT_GLB_MIME,
        "high_artifact_size_bytes": record.high_artifact_size_bytes,
        "high_artifact_readback_schema_version": DURABLE_READBACK_SCHEMA,
        "high_artifact_readback_kind": AUTHORING_MESH_V2_HIGH_ARTIFACT_READBACK_OBJECT_KIND,
        "low_compatibility_artifact_kind": LOW_ARTIFACT_KIND,
        "low_compatibility_readback_kind": LOW_READBACK_KIND,
        "low_compatibility_source_schema_version": LOW_SOURCE_SCHEMA,
        "glb_source_schema_version": LOW_SOURCE_SCHEMA,
        "glb_source_artifact_id": high_mesh_id,
        "glb_source_artifact_sha256": record.high_result_sha256,
        "glb_materialization_operation": forgecad_worker_protocol::AUTHORING_MESH_V2_HIGH_ARTIFACT_MATERIALIZE_OPERATION,
        "strict_readback": strict,
        "glb_sha256": record.high_artifact_sha256,
        "glb_object_sha256": record.high_artifact_object_sha256,
        "glb_size_bytes": record.high_artifact_size_bytes,
        "high_artifact_readback_sha256": record.high_artifact_readback_sha256,
        "high_artifact_readback_object_sha256": record.high_artifact_readback_object_sha256,
        "high_topology_status": "PASS_SOURCE_STRUCTURAL",
        "high_authoring_topology_status": "NOT_RUN",
        "uv_status": "NOT_RUN",
        "tangent_status": "NOT_RUN",
        "validator_status": "passed",
        "structural_status": "PASS_SOURCE_STRUCTURAL",
        "high_artifact_status": "PASS_SOURCE_STRUCTURAL",
        "quality_status": "structural_only",
        "visual_status": "NOT_RUN",
        "human_status": "NOT_RUN",
        "engine_status": "NOT_RUN",
        "distribution_status": "NOT_RUN",
        "high_artifact_hard_gate_passed": true,
        "source_only": true,
        "high_mesh_created": true,
        "high_stage_unlocked": false,
        "production_stage_advanced": false,
        "candidate_confirmed": false,
        "version_created": false,
        "export_performed": false,
        "runtime_write_performed": true,
        "persistent_user_data_touched": true,
        "scope_limitations": LIMITATIONS,
        "writer_policy": AUTHORING_MESH_V2_HIGH_ARTIFACT_WRITER_POLICY,
        "canonicalization_policy": MAIN_CANONICALIZATION,
        "canonical_sha256": "",
        "created_at": record.created_at
    });
    main["canonical_sha256"] = Value::String(canonical_json_hash(&main));
    Ok(main)
}

fn result(
    runtime: &Runtime,
    record: &AuthoringMeshV2HighArtifactStoreRecord,
    bridge: &AuthoringMeshV2HighBridgeStoreRecord,
    operation: &str,
    status: &str,
    idempotency: Option<&str>,
) -> Result<Value, RuntimeError> {
    let main = main_value(runtime, record, bridge)?;
    let prepared = status == "prepared";
    let replayed = status == "replayed";
    let strict = main["strict_readback"].clone();
    let mut output = json!({
        "schema_version": RESULT_SCHEMA,
        "operation": operation,
        "request_kind": if operation == PREPARE_OPERATION {"prepare"} else {"get"},
        "status": status,
        "project_id": record.project_id,
        "high_artifact_id": record.artifact_id,
        "high_artifact_sha256": record.high_artifact_sha256,
        "high_artifact_object_sha256": record.high_artifact_object_sha256,
        "high_artifact_readback_sha256": record.high_artifact_readback_sha256,
        "high_artifact_readback_object_sha256": record.high_artifact_readback_object_sha256,
        "high_artifact_readback_schema_version": DURABLE_READBACK_SCHEMA,
        "high_artifact_kind": AUTHORING_MESH_V2_HIGH_ARTIFACT_GLB_OBJECT_KIND,
        "high_artifact_mime": AUTHORING_MESH_V2_HIGH_ARTIFACT_GLB_MIME,
        "high_artifact_size_bytes": record.high_artifact_size_bytes,
        "high_bridge_id": record.bridge_id,
        "high_bridge_sha256": record.bridge_sha256,
        "high_bridge_object_sha256": record.bridge_object_sha256,
        "source_candidate_id": record.materialized_candidate_id,
        "source_candidate_state_sha256": record.materialized_candidate_state_sha256,
        "source_revision_id": record.revision_id,
        "source_revision_index": record.revision_index,
        "source_revision_sha256": record.revision_sha256,
        "source_revision_object_sha256": record.revision_object_sha256,
        "source_binding_id": record.source_binding_id,
        "source_binding_sha256": record.source_binding_sha256,
        "source_binding_object_sha256": record.source_binding_object_sha256,
        "materialized_candidate_id": record.materialized_candidate_id,
        "materialized_candidate_state_sha256": record.materialized_candidate_state_sha256,
        "high_mesh_artifact_id": main["high_mesh_artifact_id"],
        "high_mesh_artifact_sha256": record.high_result_sha256,
        "high_mesh_artifact_object_sha256": record.high_result_object_sha256,
        "high_worker_build_cohort_sha256": record.high_worker_build_cohort_sha256,
        "glb_sha256": record.high_artifact_sha256,
        "glb_object_sha256": record.high_artifact_object_sha256,
        "glb_size_bytes": record.high_artifact_size_bytes,
        "high_part_ids": main["high_part_ids"],
        "high_material_zone_ids": main["high_material_zone_ids"],
        "strict_readback": strict,
        "high_artifact": main,
        "request_input_sha256": record.request_input_sha256,
        "idempotency_key": idempotency,
        "replayed": replayed,
        "restart_hash_verified": false,
        "store_effect": if prepared {"inserted"} else {"not-touched"},
        "cas_effect": if prepared {"inserted"} else {"not-touched"},
        "atomicity_status": if prepared {"committed"} else {"not-touched"},
        "store_commit_status": if prepared {"committed"} else {"not-touched"},
        "cas_commit_status": if prepared {"committed"} else {"not-touched"},
        "runtime_write_performed": prepared,
        "persistent_user_data_touched": prepared,
        "partial_result_exposed": false,
        "high_artifact_status": "PASS_SOURCE_STRUCTURAL",
        "high_status": "NOT_RUN",
        "quality_status": "structural_only",
        "visual_status": "NOT_RUN",
        "human_status": "NOT_RUN",
        "engine_status": "NOT_RUN",
        "distribution_status": "NOT_RUN",
        "high_artifact_hard_gate_passed": true,
        "source_only": true,
        "high_mesh_created": true,
        "high_stage_unlocked": false,
        "production_stage_advanced": false,
        "candidate_confirmed": false,
        "version_created": false,
        "export_performed": false,
        "writer_policy": AUTHORING_MESH_V2_HIGH_ARTIFACT_WRITER_POLICY,
        "canonicalization_policy": MAIN_CANONICALIZATION,
        "canonical_sha256": "",
        "high_artifact_receipt_sha256": record.receipt_sha256,
        "high_artifact_receipt_object_sha256": record.receipt_object_sha256,
        "high_artifact_readback_kind": AUTHORING_MESH_V2_HIGH_ARTIFACT_READBACK_OBJECT_KIND,
        "low_compatibility_artifact_kind": LOW_ARTIFACT_KIND,
        "low_compatibility_readback_kind": LOW_READBACK_KIND,
        "low_compatibility_source_schema_version": LOW_SOURCE_SCHEMA
    });
    output["canonical_sha256"] = Value::String(canonical_json_hash(&output));
    Ok(output)
}

fn exact_request_record(
    record: &AuthoringMeshV2HighArtifactStoreRecord,
    request: &PrepareRequest,
) -> Result<(), RuntimeError> {
    if record.project_id != request.project_id
        || record.artifact_id != request.artifact_id
        || record.bridge_id != request.bridge_id
        || record.bridge_sha256 != request.bridge_sha256
        || record.bridge_object_sha256 != request.bridge_object_sha256
        || record.request_input_sha256 != request.input_sha256
    {
        return Err(invalid(
            "idempotency key is bound to another High artifact request",
        ));
    }
    Ok(())
}

pub(crate) fn prepare(runtime: &Runtime, value: &Value) -> Result<Value, RuntimeError> {
    let request = parse_prepare(value)?;
    if let Some(record) = runtime
        .store
        .get_authoring_mesh_v2_high_artifact(&request.project_id, &request.idempotency_key)?
    {
        exact_request_record(&record, &request)?;
        let bridge = load_bridge(
            runtime,
            &request.project_id,
            &request.bridge_id,
            &request.bridge_sha256,
            &request.bridge_object_sha256,
        )?;
        return result(
            runtime,
            &record,
            &bridge,
            PREPARE_OPERATION,
            "replayed",
            None,
        );
    }
    let bridge = load_bridge(
        runtime,
        &request.project_id,
        &request.bridge_id,
        &request.bridge_sha256,
        &request.bridge_object_sha256,
    )?;
    let runtime_cohort = super::build_cohort_sha256()
        .ok_or_else(|| invalid("same-cohort Runtime/High Worker build is required"))?;
    if runtime_cohort != bridge.high_worker_build_cohort_sha256 {
        return Err(invalid("High bridge Worker cohort differs from Runtime"));
    }
    let high_result = read_json(
        runtime,
        &bridge.high_result_object_sha256,
        AUTHORING_MESH_V2_HIGH_RESULT_OBJECT_KIND,
        AUTHORING_MESH_V2_HIGH_MAX_JSON_BYTES,
    )?;
    if high_result.get("canonical_sha256").and_then(Value::as_str)
        != Some(bridge.high_result_sha256.as_str())
    {
        return Err(invalid("High bridge result semantic hash differs"));
    }
    // Re-read the upstream High readback before spawning the second worker;
    // Store already validates its complete result/readback pairing.
    let _ = read_json(
        runtime,
        &bridge.high_readback_object_sha256,
        AUTHORING_MESH_V2_HIGH_READBACK_OBJECT_KIND,
        AUTHORING_MESH_V2_HIGH_MAX_JSON_BYTES,
    )?;
    let source_program = read_json(
        runtime,
        &bridge.materialized_program_object_sha256,
        "geometry-program-v2",
        AUTHORING_MESH_V2_HIGH_MAX_JSON_BYTES,
    )?;
    let worker_request = worker_request(high_result, &bridge);
    let worker = geometry_worker::production_weapon_authoring_mesh_v2_high_artifact_materialize(
        &worker_request,
    )
    .map_err(|error| invalid(error.to_string()))?;
    if worker.build_cohort_sha256.as_deref() != Some(runtime_cohort.as_str())
        || worker
            .result
            .get("source_high_worker_build_cohort_sha256")
            .and_then(Value::as_str)
            != Some(runtime_cohort.as_str())
        || worker
            .result
            .get("high_result_sha256")
            .and_then(Value::as_str)
            != Some(bridge.high_result_sha256.as_str())
    {
        return Err(invalid(
            "High artifact Worker cohort/result binding differs",
        ));
    }
    let glb = base64::engine::general_purpose::STANDARD
        .decode(
            worker
                .result
                .get("glb_base64")
                .and_then(Value::as_str)
                .ok_or_else(|| invalid("High artifact Worker GLB is missing"))?,
        )
        .map_err(|_| invalid("High artifact Worker GLB base64 is invalid"))?;
    let glb_sha = sha256_hex(&glb);
    if worker.result.get("glb_sha256").and_then(Value::as_str) != Some(glb_sha.as_str())
        || glb.is_empty()
        || glb.len() as u64 > AUTHORING_MESH_V2_HIGH_ARTIFACT_MAX_GLB_BYTES
    {
        return Err(invalid("High artifact Worker GLB hash or bound differs"));
    }
    let strict = public_strict_readback(&glb, &worker.result, &bridge, &source_program)?;
    let mut record = seed_record(&request, &bridge, &glb_sha, glb.len() as u64);
    let (_readback, readback_bytes, readback_sha) = durable_readback(&record, &strict)?;
    record.high_artifact_readback_sha256 = readback_sha;
    record.high_artifact_readback_object_sha256 = sha256_hex(&readback_bytes);
    let (_receipt, receipt_bytes, receipt_sha) = receipt(
        &record,
        &record.high_artifact_readback_sha256,
        &record.high_artifact_readback_object_sha256,
    )?;
    record.receipt_sha256 = receipt_sha;
    record.receipt_object_sha256 = sha256_hex(&receipt_bytes);
    record.canonical_sha256 = record_canonical(&record)?;

    let reservation = runtime.store.begin_cas_reservation();
    let mut objects = Vec::new();
    let operation = (|| {
        let artifact = stage(
            runtime,
            &reservation,
            &glb,
            &record.high_artifact_object_sha256,
            AUTHORING_MESH_V2_HIGH_ARTIFACT_GLB_MIME,
            AUTHORING_MESH_V2_HIGH_ARTIFACT_GLB_OBJECT_KIND,
            &mut objects,
        )?;
        let readback = stage(
            runtime,
            &reservation,
            &readback_bytes,
            &record.high_artifact_readback_object_sha256,
            AUTHORING_MESH_V2_HIGH_ARTIFACT_JSON_MIME,
            AUTHORING_MESH_V2_HIGH_ARTIFACT_READBACK_OBJECT_KIND,
            &mut objects,
        )?;
        let receipt = stage(
            runtime,
            &reservation,
            &receipt_bytes,
            &record.receipt_object_sha256,
            AUTHORING_MESH_V2_HIGH_ARTIFACT_JSON_MIME,
            AUTHORING_MESH_V2_HIGH_ARTIFACT_RECEIPT_OBJECT_KIND,
            &mut objects,
        )?;
        runtime
            .store
            .record_authoring_mesh_v2_high_artifact_with_replay(
                &AuthoringMeshV2HighArtifactCommit {
                    record: record.clone(),
                    cas: AuthoringMeshV2HighArtifactCasBundle {
                        artifact: artifact.record,
                        readback: readback.record,
                        receipt: receipt.record,
                    },
                },
            )
            .map_err(RuntimeError::from)
    })();
    let (stored, replayed) = match operation {
        Ok(value) => value,
        Err(error) => {
            cleanup(runtime, &reservation, &objects);
            return Err(error);
        }
    };
    if replayed {
        return result(
            runtime,
            &stored,
            &bridge,
            PREPARE_OPERATION,
            "replayed",
            None,
        );
    }
    result(
        runtime,
        &stored,
        &bridge,
        PREPARE_OPERATION,
        "prepared",
        Some(&request.idempotency_key),
    )
}

pub(crate) fn get(runtime: &Runtime, value: &Value) -> Result<Value, RuntimeError> {
    let request = parse_get(value)?;
    let record = runtime
        .store
        .get_authoring_mesh_v2_high_artifact_by_id(&request.project_id, &request.artifact_id)?
        .ok_or_else(|| invalid("durable High artifact is unavailable"))?;
    if record.high_artifact_sha256 != request.artifact_sha256
        || record.high_artifact_object_sha256 != request.artifact_object_sha256
        || record.high_artifact_readback_sha256 != request.readback_sha256
        || record.high_artifact_readback_object_sha256 != request.readback_object_sha256
        || record.receipt_sha256 != request.receipt_sha256
        || record.receipt_object_sha256 != request.receipt_object_sha256
        || record.bridge_id != request.bridge_id
        || record.bridge_sha256 != request.bridge_sha256
        || record.bridge_object_sha256 != request.bridge_object_sha256
    {
        return Err(invalid("durable High artifact exact lookup differs"));
    }
    let bridge = load_bridge(
        runtime,
        &request.project_id,
        &request.bridge_id,
        &request.bridge_sha256,
        &request.bridge_object_sha256,
    )?;
    result(runtime, &record, &bridge, GET_OPERATION, "found", None)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prepare_request() -> Value {
        let mut request = json!({
            "schema_version": PREPARE_SCHEMA,
            "operation": PREPARE_OPERATION,
            "project_id": "project-high-artifact-test",
            "high_artifact_id": "high-artifact-test",
            "high_bridge_id": "high-bridge-test",
            "high_bridge_sha256": "a".repeat(64),
            "high_bridge_object_sha256": "b".repeat(64),
            "idempotency_key": "high-artifact-idempotency-test",
            "max_response_bytes": MAX_RESPONSE_BYTES,
            "runtime_write_performed": false,
            "writer_policy": AUTHORING_MESH_V2_HIGH_ARTIFACT_WRITER_POLICY,
            "canonicalization_policy": REQUEST_CANONICALIZATION,
            "input_sha256": ""
        });
        request["input_sha256"] = Value::String(canonical_json_hash(&request));
        request
    }

    fn get_request() -> Value {
        let mut request = json!({
            "schema_version": GET_SCHEMA,
            "operation": GET_OPERATION,
            "project_id": "project-high-artifact-test",
            "high_artifact_id": "high-artifact-test",
            "high_artifact_sha256": "a".repeat(64),
            "high_artifact_object_sha256": "b".repeat(64),
            "high_artifact_readback_sha256": "c".repeat(64),
            "high_artifact_readback_object_sha256": "d".repeat(64),
            "high_artifact_receipt_sha256": "e".repeat(64),
            "high_artifact_receipt_object_sha256": "f".repeat(64),
            "high_bridge_id": "high-bridge-test",
            "high_bridge_sha256": "1".repeat(64),
            "high_bridge_object_sha256": "2".repeat(64),
            "max_response_bytes": MAX_RESPONSE_BYTES,
            "runtime_write_performed": false,
            "persistent_user_data_touched": false,
            "writer_policy": AUTHORING_MESH_V2_HIGH_ARTIFACT_WRITER_POLICY,
            "canonicalization_policy": REQUEST_CANONICALIZATION,
            "input_sha256": ""
        });
        request["input_sha256"] = Value::String(canonical_json_hash(&request));
        request
    }

    #[test]
    fn prepare_parser_accepts_only_the_closed_hash_bound_envelope() {
        let request = prepare_request();
        let parsed = parse_prepare(&request).expect("valid prepare request");
        assert_eq!(parsed.project_id, "project-high-artifact-test");
        assert_eq!(parsed.artifact_id, "high-artifact-test");
        assert_eq!(parsed.bridge_id, "high-bridge-test");

        let mut extra = request.clone();
        extra["unexpected"] = Value::Bool(true);
        assert!(exact_object(&extra, PREPARE_FIELDS).is_err());

        let mut drifted = request;
        drifted["project_id"] = Value::String("other-project".to_owned());
        assert!(parse_prepare(&drifted)
            .expect_err("changed input must fail hash binding")
            .to_string()
            .contains("input_sha256"));
    }

    #[test]
    fn get_parser_requires_read_only_markers_and_exact_hash_bound_fields() {
        let request = get_request();
        let parsed = parse_get(&request).expect("valid get request");
        assert_eq!(parsed.project_id, "project-high-artifact-test");
        assert_eq!(parsed.artifact_id, "high-artifact-test");

        let mut write_claim = request.clone();
        write_claim["runtime_write_performed"] = Value::Bool(true);
        write_claim["input_sha256"] = Value::String(canonical_json_hash(&{
            let mut preimage = write_claim.clone();
            preimage["input_sha256"] = Value::String(String::new());
            preimage
        }));
        assert!(parse_get(&write_claim).is_err());

        let mut extra = request;
        extra["readback_policy"] = Value::String("caller-defined".to_owned());
        assert!(parse_get(&extra).is_err());
    }

    #[test]
    fn durable_identity_hash_preimage_excludes_each_self_referential_field() {
        let mut readback = json!({
            "schema_version": DURABLE_READBACK_SCHEMA,
            "high_artifact_readback_sha256": "",
            "canonical_sha256": ""
        });
        let semantic = canonical_json_hash(&readback);
        readback["canonical_sha256"] = Value::String(semantic.clone());
        readback["high_artifact_readback_sha256"] = Value::String(semantic.clone());
        let (normalized, bytes, hash) = semantic_object(
            json!({
                "schema_version": DURABLE_READBACK_SCHEMA,
                "high_artifact_readback_sha256": ""
            }),
            Some("high_artifact_readback_sha256"),
        )
        .expect("durable identity preimage");
        assert_eq!(hash, semantic);
        assert_eq!(normalized, readback);
        let expected_bytes = canonical_json_bytes(&readback).expect("canonical readback bytes");
        assert_eq!(bytes, expected_bytes);
    }
}
