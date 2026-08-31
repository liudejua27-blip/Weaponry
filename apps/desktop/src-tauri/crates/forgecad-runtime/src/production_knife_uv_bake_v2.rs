//! Runtime-owned V2 knife UV/Cage/Bake aggregate.
//!
//! This is an additive adapter around the existing per-Part Low and Hero UV
//! durable records, exposed through the existing `surface_pipeline` façade.
//! Until every requested Part has a durable Low row it returns an explicit
//! `NOT_RUN` projection and performs no Worker or CAS write.
//!
//! The adapter keeps the direct V2 High semantic Worker hash separate from
//! the Runtime-owned High GLB/object hashes.  The distinction is part of the
//! source proof and is checked before any UV, Cage or Bake Worker starts.

use super::{canonical_json_bytes, canonical_json_hash, geometry_worker, sha256_hex, Runtime, RuntimeError};
use base64::Engine;
use forgecad_contracts::{is_opaque_id, is_sha256, LOW_QUAD_DRAFT_DURABLE_WORKER_RESULT_KIND};
use forgecad_store::{
    AUTHORING_MESH_V2_HIGH_ARTIFACT_READBACK_OBJECT_KIND,
    AUTHORING_MESH_V2_HIGH_ARTIFACT_GLB_OBJECT_KIND,
    AUTHORING_MESH_V2_HIGH_READBACK_OBJECT_KIND,
    AUTHORING_MESH_V2_HIGH_RESULT_OBJECT_KIND,
    WeaponryKnifeUvBakeV2AggregateRecord, WeaponryKnifeUvBakeV2ComponentRef,
    WEAPONRY_KNIFE_UV_BAKE_V2_RECEIPT_MIME, WEAPONRY_KNIFE_UV_BAKE_V2_RECEIPT_OBJECT_KIND,
    WEAPONRY_KNIFE_UV_BAKE_V2_RECORD_SCHEMA_VERSION,
};
use forgecad_store::hero_uv_durable::{HERO_UV_LAYOUT_CAS_KIND, HERO_UV_LINK_CAS_KIND};
use forgecad_worker_protocol::{
    PRODUCTION_WEAPON_GEOMETRIC_BAKE_AO_SAMPLE_COUNT,
    PRODUCTION_WEAPON_GEOMETRIC_BAKE_ATLAS_POLICY,
    PRODUCTION_WEAPON_GEOMETRIC_BAKE_BUDGET_PROFILE,
    PRODUCTION_WEAPON_GEOMETRIC_BAKE_NORMAL_CONVENTION,
    PRODUCTION_WEAPON_GEOMETRIC_BAKE_POLICY,
    PRODUCTION_WEAPON_GEOMETRIC_BAKE_REQUEST_SCHEMA_VERSION,
    PRODUCTION_WEAPON_GEOMETRIC_BAKE_RESOLUTION,
};
use serde_json::{json, Map, Value};
use std::collections::BTreeSet;

const PREPARE_SCHEMA: &str = "WeaponryKnifeUvBakeV2PrepareRequest@1";
const GET_SCHEMA: &str = "WeaponryKnifeUvBakeV2GetRequest@1";
// Surface operations are intentionally the profile's short, closed names.
// The façade carries the operation namespace; accepting a second long form
// here would make the active request/result schemas ambiguous.
const PREPARE_OPERATION: &str = "production_knife_uv_bake_v2_prepare";
const GET_OPERATION: &str = "production_knife_uv_bake_v2_get";
const WRITER_POLICY: &str = "forgecad-runtime-only-state-writer@1";
const CANONICALIZATION_POLICY: &str = "canonical-json-sha256-excluding-canonical-sha256@1";
const MAX_COMPONENTS: usize = 32;
const MAX_JSON_BYTES: u64 = 8 * 1024 * 1024;
const MAX_GLB_BYTES: u64 = 64 * 1024 * 1024;
const JSON_MIME: &str = "application/json";
const GLB_MIME: &str = "model/gltf-binary";
const CAGE_GLB_KIND: &str = "weaponry-knife-uv-bake-v2-cage-glb@1";
const CAGE_READBACK_KIND: &str = "weaponry-knife-uv-bake-v2-cage-readback@1";
const CAGE_WORKER_KIND: &str = "weaponry-knife-uv-bake-v2-cage-worker-result@1";
const BAKE_WORKER_KIND: &str = "weaponry-knife-uv-bake-v2-worker-result@1";
const BAKE_MAP_KIND_PREFIX: &str = "weaponry-knife-uv-bake-v2-map-";
const CAGE_REQUEST_SCHEMA: &str = "CageOffsetWorkerRequest@1";
const CAGE_OPERATION: &str = "production_weapon_cage_offset";
const CAGE_POLICY: &str = "exact-low-topology-per-vertex-normal-offset@1";
const CAGE_ALGORITHM: &str = "deterministic-welded-area-normal-offset@1";

const PREPARE_FIELDS: &[&str] = &[
    "schema_version", "operation", "project_id", "candidate_id",
    "candidate_state_sha256", "base_version_id", "source_high_artifact_id",
    "source_high_result_sha256", "source_high_result_object_sha256",
    "source_high_readback_sha256", "source_high_readback_object_sha256",
    "source_high_artifact_sha256", "source_high_artifact_object_sha256",
    "source_high_artifact_readback_sha256", "source_high_artifact_readback_object_sha256",
    "components",
    "idempotency_key", "source_only", "runtime_write_performed", "writer_policy",
    "canonicalization_policy", "input_sha256",
];
const GET_FIELDS: &[&str] = &[
    "schema_version", "operation", "project_id", "candidate_id", "candidate_state_sha256",
    "aggregate_id", "idempotency_key", "source_only", "runtime_write_performed",
    "persistent_user_data_touched", "writer_policy", "input_sha256",
];
const COMPONENT_FIELDS: &[&str] = &[
    "part_id", "material_zone_id", "source_high_part_id", "source_high_node_id",
    "source_high_material_zone_id", "low_link_id", "low_artifact_object_sha256",
    "low_artifact_sha256", "low_readback_object_sha256", "low_readback_sha256",
    "visibility_weights", "hero_uv_idempotency_key",
];

#[derive(Debug, Clone)]
struct ComponentInput {
    part_id: String,
    material_zone_id: String,
    source_high_part_id: String,
    source_high_node_id: String,
    source_high_material_zone_id: String,
    low_link_id: String,
    low_artifact_object_sha256: String,
    low_artifact_sha256: String,
    low_readback_object_sha256: String,
    low_readback_sha256: String,
    visibility_weights: Value,
    hero_uv_idempotency_key: String,
}

#[derive(Debug, Clone)]
struct PrepareRequest {
    project_id: String,
    candidate_id: String,
    candidate_state_sha256: String,
    base_version_id: Option<String>,
    source_high_artifact_id: String,
    source_high_result_sha256: String,
    source_high_result_object_sha256: String,
    source_high_readback_sha256: String,
    source_high_readback_object_sha256: String,
    source_high_artifact_sha256: String,
    source_high_artifact_object_sha256: String,
    source_high_artifact_readback_sha256: String,
    source_high_artifact_readback_object_sha256: String,
    components: Vec<ComponentInput>,
    idempotency_key: String,
    input_sha256: String,
}

fn invalid(message: impl Into<String>) -> RuntimeError {
    RuntimeError::InvalidInput(format!("KNIFE_UV_BAKE_V2_INVALID: {}", message.into()))
}

fn exact_object<'a>(value: &'a Value, fields: &[&str], label: &str) -> Result<&'a Map<String, Value>, RuntimeError> {
    let object = value.as_object().ok_or_else(|| invalid(format!("{label} must be an object")))?;
    if object.len() != fields.len() || fields.iter().any(|field| !object.contains_key(*field))
        || object.keys().any(|field| !fields.contains(&field.as_str()))
    {
        return Err(invalid(format!("{label} has unknown or missing fields")));
    }
    Ok(object)
}

fn required_id(object: &Map<String, Value>, field: &str) -> Result<String, RuntimeError> {
    object.get(field).and_then(Value::as_str).filter(|value| is_opaque_id(value))
        .map(str::to_owned).ok_or_else(|| invalid(format!("{field} is not an opaque id")))
}

fn required_hash(object: &Map<String, Value>, field: &str) -> Result<String, RuntimeError> {
    object.get(field).and_then(Value::as_str).filter(|value| is_sha256(value))
        .map(str::to_owned).ok_or_else(|| invalid(format!("{field} is not a SHA-256")))
}

fn nullable_id(object: &Map<String, Value>, field: &str) -> Result<Option<String>, RuntimeError> {
    match object.get(field) {
        Some(Value::Null) => Ok(None),
        Some(Value::String(value)) if is_opaque_id(value) => Ok(Some(value.clone())),
        _ => Err(invalid(format!("{field} is not a nullable opaque id"))),
    }
}

fn required_bool(object: &Map<String, Value>, field: &str, expected: bool) -> Result<(), RuntimeError> {
    if object.get(field) != Some(&Value::Bool(expected)) {
        return Err(invalid(format!("{field} policy differs")));
    }
    Ok(())
}

fn input_hash(value: &Value) -> Result<String, RuntimeError> {
    let mut preimage = value.clone();
    let object = preimage.as_object_mut().ok_or_else(|| invalid("request must be an object"))?;
    object.remove("input_sha256");
    object.remove("idempotency_key");
    Ok(canonical_json_hash(&preimage))
}

fn parse_component(value: &Value) -> Result<ComponentInput, RuntimeError> {
    let object = exact_object(value, COMPONENT_FIELDS, "component")?;
    let visibility_weights = object.get("visibility_weights").filter(|value| value.is_array())
        .cloned().ok_or_else(|| invalid("component visibility_weights must be an array"))?;
    Ok(ComponentInput {
        part_id: required_id(object, "part_id")?,
        material_zone_id: required_id(object, "material_zone_id")?,
        source_high_part_id: required_id(object, "source_high_part_id")?,
        source_high_node_id: required_id(object, "source_high_node_id")?,
        source_high_material_zone_id: required_id(object, "source_high_material_zone_id")?,
        low_link_id: required_id(object, "low_link_id")?,
        low_artifact_object_sha256: required_hash(object, "low_artifact_object_sha256")?,
        low_artifact_sha256: required_hash(object, "low_artifact_sha256")?,
        low_readback_object_sha256: required_hash(object, "low_readback_object_sha256")?,
        low_readback_sha256: required_hash(object, "low_readback_sha256")?,
        visibility_weights,
        hero_uv_idempotency_key: required_id(object, "hero_uv_idempotency_key")?,
    })
}

fn parse_prepare(value: &Value) -> Result<PrepareRequest, RuntimeError> {
    let object = exact_object(value, PREPARE_FIELDS, PREPARE_SCHEMA)?;
    if object.get("schema_version").and_then(Value::as_str) != Some(PREPARE_SCHEMA)
        || object.get("operation").and_then(Value::as_str) != Some(PREPARE_OPERATION)
        || object.get("writer_policy").and_then(Value::as_str) != Some(WRITER_POLICY)
        || object.get("canonicalization_policy").and_then(Value::as_str) != Some(CANONICALIZATION_POLICY)
    {
        return Err(invalid("prepare schema, operation or policy differs"));
    }
    required_bool(object, "source_only", true)?;
    required_bool(object, "runtime_write_performed", false)?;
    let input = required_hash(object, "input_sha256")?;
    if input_hash(value)? != input {
        return Err(invalid("input_sha256 does not bind the request"));
    }
    let components_value = object.get("components").and_then(Value::as_array)
        .ok_or_else(|| invalid("components must be an array"))?;
    if components_value.len() < 2 || components_value.len() > MAX_COMPONENTS {
        return Err(invalid("components must contain at least two bounded Parts"));
    }
    let components = components_value.iter().map(parse_component).collect::<Result<Vec<_>, _>>()?;
    let mut parts = BTreeSet::new();
    if components.iter().any(|component| !parts.insert(component.part_id.as_str())) {
        return Err(invalid("component Part IDs must be unique"));
    }
    Ok(PrepareRequest {
        project_id: required_id(object, "project_id")?,
        candidate_id: required_id(object, "candidate_id")?,
        candidate_state_sha256: required_hash(object, "candidate_state_sha256")?,
        base_version_id: nullable_id(object, "base_version_id")?,
        source_high_artifact_id: required_id(object, "source_high_artifact_id")?,
        source_high_result_sha256: required_hash(object, "source_high_result_sha256")?,
        source_high_result_object_sha256: required_hash(object, "source_high_result_object_sha256")?,
        source_high_readback_sha256: required_hash(object, "source_high_readback_sha256")?,
        source_high_readback_object_sha256: required_hash(object, "source_high_readback_object_sha256")?,
        source_high_artifact_sha256: required_hash(object, "source_high_artifact_sha256")?,
        source_high_artifact_object_sha256: required_hash(object, "source_high_artifact_object_sha256")?,
        source_high_artifact_readback_sha256: required_hash(object, "source_high_artifact_readback_sha256")?,
        source_high_artifact_readback_object_sha256: required_hash(object, "source_high_artifact_readback_object_sha256")?,
        components,
        idempotency_key: required_id(object, "idempotency_key")?,
        input_sha256: input,
    })
}

fn parse_get(value: &Value) -> Result<Map<String, Value>, RuntimeError> {
    let object = exact_object(value, GET_FIELDS, GET_SCHEMA)?;
    if object.get("schema_version").and_then(Value::as_str) != Some(GET_SCHEMA)
        || object.get("operation").and_then(Value::as_str) != Some(GET_OPERATION)
        || object.get("writer_policy").and_then(Value::as_str) != Some(WRITER_POLICY)
    {
        return Err(invalid("get schema, operation or policy differs"));
    }
    required_bool(object, "source_only", true)?;
    required_bool(object, "runtime_write_performed", false)?;
    required_bool(object, "persistent_user_data_touched", false)?;
    let input = required_hash(object, "input_sha256")?;
    let mut preimage = value.clone();
    preimage["input_sha256"] = Value::String(String::new());
    if canonical_json_hash(&preimage) != input {
        return Err(invalid("get input_sha256 does not bind the request"));
    }
    Ok(object.clone())
}

fn strict_high_source(runtime: &Runtime, request: &PrepareRequest) -> Result<(Value, Vec<u8>), RuntimeError> {
    let high = runtime.store.get_authoring_mesh_v2_high_artifact_for_low(
        &request.project_id, &request.source_high_artifact_id,
    )?.ok_or_else(|| invalid("durable V2 High artifact is unavailable"))?;
    // The direct Worker result semantic hash and the Runtime durable GLB hash
    // are separate source identities.  Do not infer equality from names.
    if high.project_id != request.project_id
        || high.materialized_candidate_id != request.candidate_id
        || high.materialized_candidate_state_sha256 != request.candidate_state_sha256
        || high.high_result_sha256 != request.source_high_result_sha256
        || high.high_result_object_sha256 != request.source_high_result_object_sha256
        || high.high_readback_sha256 != request.source_high_readback_sha256
        || high.high_readback_object_sha256 != request.source_high_readback_object_sha256
        || high.high_artifact_sha256 != request.source_high_artifact_sha256
        || high.high_artifact_object_sha256 != request.source_high_artifact_object_sha256
        || high.high_artifact_readback_sha256 != request.source_high_artifact_readback_sha256
        || high.high_artifact_readback_object_sha256 != request.source_high_artifact_readback_object_sha256
        || high.high_artifact_sha256 != high.high_artifact_object_sha256
    {
        return Err(invalid("V2 High semantic/GLB/readback source proof differs"));
    }
    let object = runtime.store.get_object(&high.high_artifact_object_sha256)?
        .ok_or_else(|| invalid("V2 High GLB CAS object is unavailable"))?;
    if object.mime != GLB_MIME || object.kind != "authoring-mesh-v2-high-artifact-glb@1"
        || object.size_bytes == 0 || object.size_bytes > MAX_GLB_BYTES
    {
        return Err(invalid("V2 High GLB CAS metadata differs"));
    }
    let glb = runtime.cas_read_bounded(&high.high_artifact_object_sha256, MAX_GLB_BYTES)?;
    if sha256_hex(&glb) != high.high_artifact_object_sha256 {
        return Err(invalid("V2 High GLB bytes do not match durable object hash"));
    }
    let inspection = super::native_high_glb_readback::inspect_authoring_mesh_v2_high_glb(&glb)
        .map_err(|error| invalid(format!("V2 High strict readback failed: {error}")))?;
    let parts = inspection.get("part_ids").and_then(Value::as_array)
        .ok_or_else(|| invalid("V2 High Part inventory is missing"))?;
    if request.components.iter().any(|component| {
        !parts.iter().any(|part| part.as_str() == Some(component.source_high_part_id.as_str()))
    }) {
        return Err(invalid("V2 High source proof does not cover every requested Part"));
    }
    Ok((json!({
        "schema_version":"WeaponryKnifeUvBakeV2SourceProof@1",
        "source_high_artifact_id":high.artifact_id,
        "direct_worker_semantic_high_result_sha256":high.high_result_sha256,
        "direct_worker_semantic_high_result_object_sha256":high.high_result_object_sha256,
        "direct_worker_high_readback_sha256":high.high_readback_sha256,
        "direct_worker_high_readback_object_sha256":high.high_readback_object_sha256,
        "runtime_durable_high_glb_artifact_sha256":high.high_artifact_sha256,
        "runtime_durable_high_glb_object_sha256":high.high_artifact_object_sha256,
        "runtime_durable_high_glb_readback_sha256":high.high_artifact_readback_sha256,
        "runtime_durable_high_glb_readback_object_sha256":high.high_artifact_readback_object_sha256,
        "part_ids":parts,
        "semantic_vs_glb_policy":"explicit-fields-never-name-equality@1",
        "high_source_policy":"direct-v2-high-position-normal-uv0-optional-tangent@1"
    }), glb))
}

fn low_exists(runtime: &Runtime, request: &PrepareRequest) -> Result<Vec<Value>, RuntimeError> {
    let mut lows = Vec::with_capacity(request.components.len());
    for component in &request.components {
        let Some(low) = runtime.store.get_low_quad_draft_durable_by_link_id(&component.low_link_id)? else {
            return Ok(Vec::new());
        };
        if low.project_id != request.project_id
            || low.candidate_id != request.candidate_id
            || low.candidate_state_sha256 != request.candidate_state_sha256
            || low.base_version_id != request.base_version_id
            || low.source_high_artifact_id != request.source_high_artifact_id
            || low.source_high_artifact_sha256 != request.source_high_artifact_sha256
            || low.source_high_artifact_object_sha256 != request.source_high_artifact_object_sha256
            || low.source_high_artifact_readback_sha256 != request.source_high_artifact_readback_sha256
            || low.source_high_artifact_readback_object_sha256 != request.source_high_artifact_readback_object_sha256
            || low.artifact_object_sha256 != component.low_artifact_object_sha256
            || low.artifact_sha256 != component.low_artifact_sha256
            || low.readback_object_sha256 != component.low_readback_object_sha256
            || low.readback_sha256 != component.low_readback_sha256
        {
            return Err(invalid(format!("Low source proof differs for Part {}", component.part_id)));
        }
        let low_worker = verify_semantic_json_cas(
            runtime,
            &low.worker_result_object_sha256,
            LOW_QUAD_DRAFT_DURABLE_WORKER_RESULT_KIND,
            &low.worker_result_sha256,
        )?;
        for (field, expected) in [
            ("source_high_part_id", component.source_high_part_id.as_str()),
            ("source_high_node_id", component.source_high_node_id.as_str()),
            ("source_high_material_zone_id", component.source_high_material_zone_id.as_str()),
        ] {
            if low_worker.get(field).and_then(Value::as_str) != Some(expected) {
                return Err(invalid(format!("Low source {field} differs for Part {}", component.part_id)));
            }
        }
        lows.push(serde_json::to_value(low).map_err(|error| invalid(error.to_string()))?);
    }
    Ok(lows)
}

fn hero_uv_request(request: &PrepareRequest, component: &ComponentInput) -> Value {
    let mut value = json!({
        "schema_version":"HeroUvDurablePrepareRequest@1",
        "project_id":request.project_id,
        "candidate_id":request.candidate_id,
        "candidate_state_sha256":request.candidate_state_sha256,
        "base_version_id":request.base_version_id,
        "source_low_artifact_id":component.low_artifact_object_sha256,
        "source_low_artifact_object_sha256":component.low_artifact_object_sha256,
        "source_low_artifact_sha256":component.low_artifact_sha256,
        "source_low_artifact_readback_object_sha256":component.low_readback_object_sha256,
        "source_low_artifact_readback_sha256":component.low_readback_sha256,
        "resolution":4096,"padding_texels":32,"min_mip_level":5,
        "hard_edge_angle_deg":60.0,"stretch_threshold":32.0,
        "visibility_weights":component.visibility_weights,
        "idempotency_key":component.hero_uv_idempotency_key,
        "max_response_bytes":1_048_576,
        "source_only":true,"runtime_write_performed":false,
        "writer_policy":"forgecad-runtime-only-state-writer@1",
        "canonicalization_policy":"canonical-json-sha256-excluding-canonical-sha256@1",
        "input_sha256":""
    });
    value["input_sha256"] = Value::String(input_hash(&value).expect("Hero UV request object"));
    value
}

fn cage_request(low_hash: &str, low_glb: &[u8]) -> Value {
    let mut value = json!({
        "schema_version":CAGE_REQUEST_SCHEMA,
        "preview_only":true,
        "source_low_artifact_sha256":low_hash,
        "low_glb_base64":base64::engine::general_purpose::STANDARD.encode(low_glb),
        "offset_m":0.001,"max_offset_m":0.2,"max_coordinate_abs_m":10.0,
        "offset_field_policy":CAGE_POLICY,
        "algorithm":CAGE_ALGORITHM,
        "canonical_sha256":""
    });
    value["canonical_sha256"] = Value::String(canonical_json_hash(&value));
    value
}

fn bake_request(high_hash: &str, high_glb: &[u8], low_hash: &str, low_glb: &[u8], cage_hash: &str, cage_glb: &[u8]) -> Value {
    let mut value = json!({
        "schema_version":PRODUCTION_WEAPON_GEOMETRIC_BAKE_REQUEST_SCHEMA_VERSION,
        "bake_policy":PRODUCTION_WEAPON_GEOMETRIC_BAKE_POLICY,
        "bake_policy_sha256":sha256_hex(PRODUCTION_WEAPON_GEOMETRIC_BAKE_POLICY.as_bytes()),
        "budget_profile":PRODUCTION_WEAPON_GEOMETRIC_BAKE_BUDGET_PROFILE,
        "atlas_policy":PRODUCTION_WEAPON_GEOMETRIC_BAKE_ATLAS_POLICY,
        "high_glb_base64":base64::engine::general_purpose::STANDARD.encode(high_glb),
        "low_glb_base64":base64::engine::general_purpose::STANDARD.encode(low_glb),
        "cage_glb_base64":base64::engine::general_purpose::STANDARD.encode(cage_glb),
        "high_artifact_sha256":high_hash,"low_artifact_sha256":low_hash,"cage_artifact_sha256":cage_hash,
        "resolution":PRODUCTION_WEAPON_GEOMETRIC_BAKE_RESOLUTION,
        "normal_convention":PRODUCTION_WEAPON_GEOMETRIC_BAKE_NORMAL_CONVENTION,
        "max_ray_distance_m":0.1,"ao_sample_count":PRODUCTION_WEAPON_GEOMETRIC_BAKE_AO_SAMPLE_COUNT,
        "surface_bake_reuse_allowed":false,"canonical_sha256":""
    });
    value["canonical_sha256"] = Value::String(canonical_json_hash(&value));
    value
}

fn decode_hash_bound(object: &Map<String, Value>, encoded: &str, hash_field: &str) -> Result<Vec<u8>, RuntimeError> {
    let bytes = base64::engine::general_purpose::STANDARD.decode(encoded.as_bytes())
        .map_err(|_| invalid(format!("{hash_field} base64 is invalid")))?;
    let hash = object.get(hash_field).and_then(Value::as_str).ok_or_else(|| invalid(format!("{hash_field} is missing")))?;
    if bytes.is_empty() || bytes.len() as u64 > MAX_GLB_BYTES || sha256_hex(&bytes) != hash {
        return Err(invalid(format!("{hash_field} bytes do not match hash")));
    }
    Ok(bytes)
}

fn canonical_result(value: &Value) -> Result<(Vec<u8>, String), RuntimeError> {
    let bytes = canonical_json_bytes(value).map_err(|error| invalid(error.to_string()))?;
    Ok((bytes.clone(), sha256_hex(&bytes)))
}

fn put_json(runtime: &Runtime, value: &Value, kind: &str) -> Result<(String, String), RuntimeError> {
    let (bytes, hash) = canonical_result(value)?;
    if bytes.len() as u64 > MAX_JSON_BYTES {
        return Err(invalid("derived JSON exceeds its bound"));
    }
    let object = runtime.store.put_object(&bytes, Some(&hash), JSON_MIME, kind, &super::now_string())?;
    Ok((object.record.sha256, value.get("canonical_sha256").and_then(Value::as_str).unwrap_or(&hash).to_owned()))
}

fn put_bytes(runtime: &Runtime, bytes: &[u8], expected: &str, mime: &str, kind: &str) -> Result<String, RuntimeError> {
    let object = runtime.store.put_object(bytes, Some(expected), mime, kind, &super::now_string())?;
    Ok(object.record.sha256)
}

fn result_hash(value: &Value) -> Result<String, RuntimeError> {
    value.get("canonical_sha256").and_then(Value::as_str).filter(|value| is_sha256(value))
        .map(str::to_owned).ok_or_else(|| invalid("Worker canonical_sha256 is missing"))
}

fn validate_hero_uv_structural(
    value: &Value,
    request: &PrepareRequest,
    component: &ComponentInput,
) -> Result<(), RuntimeError> {
    let layout = value
        .get("layout")
        .and_then(Value::as_object)
        .ok_or_else(|| invalid(format!("Hero UV layout is missing for Part {}", component.part_id)))?;
    for (field, expected) in [
        ("quality_status", "structural_only"),
        ("structural_status", "PASS_SOURCE_STRUCTURAL"),
        ("visual_status", "NOT_PROVEN"),
        ("human_status", "NOT_RUN"),
        ("engine_status", "NOT_RUN"),
        ("distribution_status", "NOT_RUN"),
    ] {
        if layout.get(field).and_then(Value::as_str) != Some(expected) {
            return Err(invalid(format!("Hero UV {field} is not structurally passed for Part {}", component.part_id)));
        }
    }
    if layout.get("low_artifact_sha256").and_then(Value::as_str) != Some(component.low_artifact_sha256.as_str()) {
        return Err(invalid(format!("Hero UV Low source differs for Part {}", component.part_id)));
    }
    let metrics = layout
        .get("metrics")
        .and_then(Value::as_object)
        .ok_or_else(|| invalid(format!("Hero UV metrics are missing for Part {}", component.part_id)))?;
    let zero_metrics = [
        "uv0_overlap_count", "uv1_overlap_count", "uv0_out_of_bounds_triangle_count",
        "uv1_out_of_bounds_triangle_count", "uv0_zero_area_triangle_count",
        "uv0_inverted_triangle_count", "stretch_exceeded_triangle_count",
        "non_manifold_edge_count", "hard_edge_without_seam_count",
    ];
    if zero_metrics.iter().any(|field| metrics.get(*field).and_then(Value::as_u64) != Some(0))
        || metrics.get("triangle_count").and_then(Value::as_u64).unwrap_or(0) == 0
        || metrics.get("uv0_structural_gate") != Some(&Value::Bool(true))
        || metrics.get("uv1_structural_gate") != Some(&Value::Bool(true))
        || metrics.get("mip_padding_passed") != Some(&Value::Bool(true))
        || metrics.get("first_person_weighting_applied") != Some(&Value::Bool(true))
    {
        return Err(invalid(format!("Hero UV coverage/structural metrics failed for Part {}", component.part_id)));
    }
    let max_stretch = metrics.get("max_stretch_ratio").and_then(Value::as_f64)
        .ok_or_else(|| invalid(format!("Hero UV max stretch is missing for Part {}", component.part_id)))?;
    if !max_stretch.is_finite() || max_stretch > 32.0 {
        return Err(invalid(format!("Hero UV stretch threshold failed for Part {}", component.part_id)));
    }
    let weights = layout.get("visibility_weights").and_then(Value::as_array)
        .ok_or_else(|| invalid(format!("Hero UV visibility coverage is missing for Part {}", component.part_id)))?;
    if weights.is_empty() || !weights.iter().any(|weight| {
        weight.get("part_id").and_then(Value::as_str) == Some(component.part_id.as_str())
    }) {
        return Err(invalid(format!("Hero UV visibility coverage does not include Part {}", component.part_id)));
    }
    let _ = request;
    Ok(())
}

fn process_component(runtime: &Runtime, request: &PrepareRequest, high_glb: &[u8], component: &ComponentInput) -> Result<WeaponryKnifeUvBakeV2ComponentRef, RuntimeError> {
    let low_glb = runtime.cas_read_bounded(&component.low_artifact_object_sha256, MAX_GLB_BYTES)?;
    if sha256_hex(&low_glb) != component.low_artifact_object_sha256 {
        return Err(invalid(format!("Low GLB bytes differ for Part {}", component.part_id)));
    }
    let uv = runtime.hero_uv_durable_prepare(hero_uv_request(request, component))?;
    let hero_uv_link_id = uv.get("link_id").and_then(Value::as_str).map(str::to_owned);
    let hero_uv_link_object_sha256 = uv.get("link_object_sha256").and_then(Value::as_str).map(str::to_owned);
    let hero_uv_layout_object_sha256 = uv.get("layout_object_sha256").and_then(Value::as_str).map(str::to_owned);
    if hero_uv_link_id.is_none() || hero_uv_link_object_sha256.is_none() || hero_uv_layout_object_sha256.is_none() {
        return Err(invalid(format!("Hero UV durable result is incomplete for Part {}", component.part_id)));
    }
    validate_hero_uv_structural(&uv, request, component)?;

    let cage_payload = cage_request(&component.low_artifact_sha256, &low_glb);
    let cage_first = geometry_worker::production_weapon_cage_offset(&cage_payload)
        .map_err(|error| invalid(error.to_string()))?;
    let cage_second = geometry_worker::production_weapon_cage_offset(&cage_payload)
        .map_err(|error| invalid(error.to_string()))?;
    if cage_first.result != cage_second.result || cage_first.build_cohort_sha256 != cage_second.build_cohort_sha256 {
        return Err(invalid(format!("Cage Worker replay differs for Part {}", component.part_id)));
    }
    let cage = cage_first.result;
    let cage_object = cage.as_object().ok_or_else(|| invalid("Cage Worker result is not an object"))?;
    if cage_object.get("operation").and_then(Value::as_str) != Some(CAGE_OPERATION)
        || cage_object.get("source_low_artifact_sha256").and_then(Value::as_str) != Some(component.low_artifact_sha256.as_str())
        || cage_object.get("exact_topology_correspondence") != Some(&Value::Bool(true))
    {
        return Err(invalid(format!("Cage Worker source proof differs for Part {}", component.part_id)));
    }
    let cage_glb = decode_hash_bound(cage_object, cage_object.get("cage_glb_base64").and_then(Value::as_str).ok_or_else(|| invalid("Cage GLB is missing"))?, "cage_artifact_sha256")?;
    let cage_hash = cage_object.get("cage_artifact_sha256").and_then(Value::as_str).ok_or_else(|| invalid("Cage artifact hash is missing"))?.to_owned();
    let cage_object_hash = put_bytes(runtime, &cage_glb, &cage_hash, GLB_MIME, CAGE_GLB_KIND)?;
    let (cage_worker_object_hash, _) = put_json(runtime, &cage, CAGE_WORKER_KIND)?;
    let cage_readback = json!({
        "schema_version":"WeaponryKnifeUvBakeV2CageReadback@1",
        "source_low_artifact_sha256":component.low_artifact_sha256,
        "cage_artifact_sha256":cage_hash,
        "cage_artifact_object_sha256":cage_object_hash,
        "worker_result_object_sha256":cage_worker_object_hash,
        "worker_readback":cage["cage_artifact_readback"],
        "structural_status":"PASS_SOURCE_STRUCTURAL",
        "canonical_sha256":""
    });
    let mut cage_readback = cage_readback;
    cage_readback["canonical_sha256"] = Value::String(canonical_json_hash(&cage_readback));
    let (cage_readback_object_hash, cage_readback_hash) = put_json(runtime, &cage_readback, CAGE_READBACK_KIND)?;

    let bake_payload = bake_request(&request.source_high_artifact_sha256, high_glb, &component.low_artifact_sha256, &low_glb, &cage_hash, &cage_glb);
    let bake_first = geometry_worker::production_weapon_geometric_bake_2k(&bake_payload)
        .map_err(|error| invalid(error.to_string()))?;
    let bake_second = geometry_worker::production_weapon_geometric_bake_2k(&bake_payload)
        .map_err(|error| invalid(error.to_string()))?;
    if bake_first.result != bake_second.result || bake_first.build_cohort_sha256 != bake_second.build_cohort_sha256 {
        return Err(invalid(format!("Bake Worker replay differs for Part {}", component.part_id)));
    }
    super::production_weapon_high_low_bake::validate_production_weapon_geometric_bake_result(
        &bake_first.result, &request.source_high_artifact_sha256, &component.low_artifact_sha256, &cage_hash,
    )?;
    let bake_worker_hash = result_hash(&bake_first.result)?;
    let (bake_worker_object_hash, _) = put_json(runtime, &bake_first.result, BAKE_WORKER_KIND)?;
    let bake_object = bake_first.result.as_object().ok_or_else(|| invalid("Bake Worker result is not an object"))?;
    let mut map_hashes = Vec::new();
    for (map_name, field) in [
        ("tangent-normal", "tangent_normal_png_base64"), ("ao", "ao_png_base64"),
        ("curvature", "curvature_png_base64"), ("thickness", "thickness_png_base64"),
        ("position", "position_png_base64"), ("object-id", "object_id_png_base64"),
        ("material-id", "material_id_png_base64"), ("part-id", "part_id_png_base64"),
    ] {
        let encoded = bake_object.get(field).and_then(Value::as_str).ok_or_else(|| invalid(format!("Bake map {map_name} is missing")))?;
        let hash_field = match map_name {
            "tangent-normal" => "tangent_normal_png_sha256",
            _ => match map_name {
                "ao" => "ao_png_sha256", "curvature" => "curvature_png_sha256", "thickness" => "thickness_png_sha256",
                "position" => "position_png_sha256", "object-id" => "object_id_png_sha256",
                "material-id" => "material_id_png_sha256", "part-id" => "part_id_png_sha256", _ => unreachable!(),
            },
        };
        let expected = bake_object.get(hash_field).and_then(Value::as_str).filter(|value| is_sha256(value)).ok_or_else(|| invalid(format!("Bake map {map_name} hash is missing")))?;
        let bytes = base64::engine::general_purpose::STANDARD.decode(encoded.as_bytes()).map_err(|_| invalid(format!("Bake map {map_name} base64 is invalid")))?;
        let object_hash = put_bytes(runtime, &bytes, expected, "image/png", &format!("{BAKE_MAP_KIND_PREFIX}{map_name}@1"))?;
        map_hashes.push(object_hash);
    }
    Ok(WeaponryKnifeUvBakeV2ComponentRef {
        part_id: component.part_id.clone(), material_zone_id: component.material_zone_id.clone(),
        source_high_part_id: component.source_high_part_id.clone(),
        source_high_node_id: component.source_high_node_id.clone(),
        source_high_material_zone_id: component.source_high_material_zone_id.clone(),
        low_link_id: component.low_link_id.clone(), low_artifact_object_sha256: component.low_artifact_object_sha256.clone(),
        low_artifact_sha256: component.low_artifact_sha256.clone(), low_readback_object_sha256: component.low_readback_object_sha256.clone(),
        low_readback_sha256: component.low_readback_sha256.clone(), hero_uv_link_id, hero_uv_link_object_sha256,
        hero_uv_layout_object_sha256, cage_artifact_object_sha256: Some(cage_object_hash), cage_artifact_sha256: Some(cage_hash),
        cage_readback_object_sha256: Some(cage_readback_object_hash), cage_readback_sha256: Some(cage_readback_hash),
        bake_worker_result_object_sha256: Some(bake_worker_object_hash), bake_worker_result_sha256: Some(bake_worker_hash),
        bake_output_object_sha256s: map_hashes, uv_status: "PASS_SOURCE_STRUCTURAL".to_owned(),
        cage_status: "PASS_SOURCE_STRUCTURAL".to_owned(), bake_status: "PASS_SOURCE_STRUCTURAL".to_owned(),
    })
}

fn not_run(request: &PrepareRequest, reason: &str) -> Value {
    json!({
        "schema_version":"WeaponryKnifeUvBakeV2PrepareResult@1", "operation":PREPARE_OPERATION,
        "project_id":request.project_id, "candidate_id":request.candidate_id,
        "candidate_state_sha256":request.candidate_state_sha256, "aggregate_id":Value::Null,
        "replayed":false, "restart_hash_verified":false, "runtime_write_performed":false,
        "persistent_user_data_touched":false, "uv_status":"NOT_RUN", "cage_status":"NOT_RUN",
        "bake_status":"NOT_RUN", "quality_status":"NOT_RUN", "visual_status":"NOT_PROVEN",
        "human_status":"NOT_RUN", "engine_status":"NOT_RUN", "commercial_status":"NOT_RUN",
        "blocking_reason":reason, "executable_path": "await-all-component-low-durable-links-then-run-v2-adapter@1",
        "production_stage_advanced":false, "candidate_confirmed":false, "version_created":false,
        "export_performed":false, "source_only":true, "canonical_sha256":""
    })
}

fn seal_result(mut result: Value) -> Value {
    result["canonical_sha256"] = Value::String(canonical_json_hash(&result));
    result
}

fn verify_cas_object(
    runtime: &Runtime,
    hash: &str,
    mime: &str,
    kind: &str,
    kind_prefix: bool,
    max_bytes: u64,
) -> Result<Vec<u8>, RuntimeError> {
    if !is_sha256(hash) {
        return Err(invalid("child CAS reference is not a SHA-256"));
    }
    let object = runtime
        .store
        .get_object(hash)?
        .ok_or_else(|| invalid("child CAS object is unavailable"))?;
    let kind_matches = if kind_prefix {
        object.kind.starts_with(kind)
    } else {
        object.kind == kind
    };
    if object.mime != mime || !kind_matches || object.size_bytes == 0 || object.size_bytes > max_bytes {
        return Err(invalid("child CAS metadata differs"));
    }
    let bytes = runtime.cas_read_bounded(hash, max_bytes)?;
    if sha256_hex(&bytes) != hash || bytes.len() as u64 != object.size_bytes {
        return Err(invalid("child CAS bytes do not match its object hash"));
    }
    Ok(bytes)
}

fn verify_json_cas(
    runtime: &Runtime,
    hash: &str,
    kind: &str,
) -> Result<Value, RuntimeError> {
    let bytes = verify_cas_object(runtime, hash, JSON_MIME, kind, false, MAX_JSON_BYTES)?;
    let value: Value = serde_json::from_slice(&bytes)
        .map_err(|error| invalid(format!("child JSON CAS is invalid: {error}")))?;
    if canonical_json_bytes(&value).map_err(|error| invalid(error.to_string()))? != bytes {
        return Err(invalid("child JSON CAS is not canonical"));
    }
    Ok(value)
}

fn verify_semantic_json_cas(
    runtime: &Runtime,
    hash: &str,
    kind: &str,
    expected_semantic_hash: &str,
) -> Result<Value, RuntimeError> {
    let value = verify_json_cas(runtime, hash, kind)?;
    let semantic = value
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .ok_or_else(|| invalid("child JSON semantic hash is missing"))?;
    if semantic != expected_semantic_hash {
        return Err(invalid("child JSON semantic hash differs"));
    }
    let mut preimage = value.clone();
    preimage["canonical_sha256"] = Value::String(String::new());
    if canonical_json_hash(&preimage) != semantic {
        return Err(invalid("child JSON canonical preimage differs"));
    }
    Ok(value)
}

/// Revalidate every child of an aggregate after Store readback.  This is
/// intentionally independent from the receipt's JSON: deleting or replacing
/// one child must make get fail closed, even if the immutable receipt itself
/// is still present.
fn revalidate_aggregate(runtime: &Runtime, record: &WeaponryKnifeUvBakeV2AggregateRecord) -> Result<(), RuntimeError> {
    let candidate = runtime
        .candidate(&record.candidate_id)?
        .ok_or_else(|| invalid("aggregate candidate is unavailable after restart"))?;
    if candidate.project_id != record.project_id
        || candidate.canonical_sha256 != record.candidate_state_sha256
        || candidate.base_version_id != record.base_version_id
    {
        return Err(invalid("aggregate candidate binding differs after restart"));
    }
    let high = runtime
        .store
        .get_authoring_mesh_v2_high_artifact_for_low(&record.project_id, &record.source_high_artifact_id)?
        .ok_or_else(|| invalid("aggregate V2 High source is unavailable after restart"))?;
    if high.materialized_candidate_id != record.candidate_id
        || high.materialized_candidate_state_sha256 != record.candidate_state_sha256
        || high.high_result_sha256 != record.source_high_result_sha256
        || high.high_result_object_sha256 != record.source_high_result_object_sha256
        || high.high_readback_sha256 != record.source_high_readback_sha256
        || high.high_readback_object_sha256 != record.source_high_readback_object_sha256
        || high.high_artifact_sha256 != record.source_high_artifact_sha256
        || high.high_artifact_object_sha256 != record.source_high_artifact_object_sha256
        || high.high_artifact_readback_sha256 != record.source_high_artifact_readback_sha256
        || high.high_artifact_readback_object_sha256 != record.source_high_artifact_readback_object_sha256
    {
        return Err(invalid("aggregate V2 High source proof differs after restart"));
    }
    let high_glb = verify_cas_object(
        runtime,
        &record.source_high_artifact_object_sha256,
        GLB_MIME,
        AUTHORING_MESH_V2_HIGH_ARTIFACT_GLB_OBJECT_KIND,
        false,
        MAX_GLB_BYTES,
    )?;
    let inspection = super::native_high_glb_readback::inspect_authoring_mesh_v2_high_glb(&high_glb)
        .map_err(|error| invalid(format!("aggregate High strict readback failed: {error}")))?;
    let part_ids = inspection
        .get("part_ids")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("aggregate High Part inventory is missing after restart"))?;
    if record.components.iter().any(|component| {
        !part_ids.iter().any(|part| part.as_str() == Some(component.source_high_part_id.as_str()))
    }) {
        return Err(invalid("aggregate High Part inventory no longer covers a child"));
    }
    let high_readback = verify_json_cas(
        runtime,
        &record.source_high_artifact_readback_object_sha256,
        AUTHORING_MESH_V2_HIGH_ARTIFACT_READBACK_OBJECT_KIND,
    )?;
    if high_readback.get("glb_sha256").and_then(Value::as_str)
            != Some(record.source_high_artifact_sha256.as_str())
        || high_readback.get("glb_object_sha256").and_then(Value::as_str)
            != Some(record.source_high_artifact_object_sha256.as_str())
    {
        return Err(invalid("aggregate High durable readback binding differs"));
    }
    // The direct Worker result/readback are independently checked by the
    // High-artifact Store getter.  Verify their CAS bytes here as well so a
    // restart cannot return an aggregate whose receipt outlives its source.
    verify_json_cas(runtime, &record.source_high_result_object_sha256, AUTHORING_MESH_V2_HIGH_RESULT_OBJECT_KIND)?;
    verify_json_cas(runtime, &record.source_high_readback_object_sha256, AUTHORING_MESH_V2_HIGH_READBACK_OBJECT_KIND)?;

    for component in &record.components {
        let low = runtime
            .store
            .get_low_quad_draft_durable_by_link_id(&component.low_link_id)?
            .ok_or_else(|| invalid(format!("Low child {} is unavailable after restart", component.part_id)))?;
        if low.project_id != record.project_id
            || low.candidate_id != record.candidate_id
            || low.candidate_state_sha256 != record.candidate_state_sha256
            || low.base_version_id != record.base_version_id
            || low.source_high_artifact_id != record.source_high_artifact_id
            || low.source_high_artifact_sha256 != record.source_high_artifact_sha256
            || low.source_high_artifact_object_sha256 != record.source_high_artifact_object_sha256
            || low.source_high_artifact_readback_sha256 != record.source_high_artifact_readback_sha256
            || low.source_high_artifact_readback_object_sha256 != record.source_high_artifact_readback_object_sha256
            || low.artifact_object_sha256 != component.low_artifact_object_sha256
            || low.artifact_sha256 != component.low_artifact_sha256
            || low.readback_object_sha256 != component.low_readback_object_sha256
            || low.readback_sha256 != component.low_readback_sha256
        {
            return Err(invalid(format!("Low child {} binding differs after restart", component.part_id)));
        }
        let low_worker = verify_semantic_json_cas(
            runtime,
            &low.worker_result_object_sha256,
            LOW_QUAD_DRAFT_DURABLE_WORKER_RESULT_KIND,
            &low.worker_result_sha256,
        )?;
        for (field, expected) in [
            ("source_high_part_id", component.source_high_part_id.as_str()),
            ("source_high_node_id", component.source_high_node_id.as_str()),
            ("source_high_material_zone_id", component.source_high_material_zone_id.as_str()),
        ] {
            if low_worker.get(field).and_then(Value::as_str) != Some(expected) {
                return Err(invalid(format!("Low child {} {field} binding differs after restart", component.part_id)));
            }
        }
        let hero_id = component.hero_uv_link_id.as_deref()
            .ok_or_else(|| invalid(format!("Hero UV child is missing for {}", component.part_id)))?;
        let hero = runtime.store.get_hero_uv_by_link_id(hero_id)?
            .ok_or_else(|| invalid(format!("Hero UV child {} is unavailable after restart", component.part_id)))?;
        if hero.project_id != record.project_id
            || hero.candidate_id != record.candidate_id
            || hero.candidate_state_sha256 != record.candidate_state_sha256
            || hero.source_low_artifact_object_sha256 != component.low_artifact_object_sha256
            || hero.source_low_artifact_sha256 != component.low_artifact_sha256
            || Some(hero.link_object_sha256.clone()) != component.hero_uv_link_object_sha256
            || Some(hero.layout_object_sha256.clone()) != component.hero_uv_layout_object_sha256
        {
            return Err(invalid(format!("Hero UV child {} binding differs after restart", component.part_id)));
        }
        verify_semantic_json_cas(
            runtime,
            &component.hero_uv_layout_object_sha256.clone().ok_or_else(|| invalid("Hero UV layout object is missing"))?,
            HERO_UV_LAYOUT_CAS_KIND,
            &hero.layout_canonical_sha256,
        )?;
        let hero_link = verify_json_cas(
            runtime,
            &component.hero_uv_link_object_sha256.clone().ok_or_else(|| invalid("Hero UV link object is missing"))?,
            HERO_UV_LINK_CAS_KIND,
        )?;
        if hero_link.get("link_id").and_then(Value::as_str) != Some(hero.link_id.as_str())
            || hero_link.get("project_id").and_then(Value::as_str) != Some(record.project_id.as_str())
            || hero_link.get("candidate_id").and_then(Value::as_str) != Some(record.candidate_id.as_str())
            || hero_link.get("source_low_artifact_sha256").and_then(Value::as_str) != Some(component.low_artifact_sha256.as_str())
        {
            return Err(invalid(format!("Hero UV link object {} binding differs after restart", component.part_id)));
        }
        verify_cas_object(runtime, &component.cage_artifact_object_sha256.clone().ok_or_else(|| invalid("cage object is missing"))?, GLB_MIME, CAGE_GLB_KIND, false, MAX_GLB_BYTES)?;
        verify_semantic_json_cas(runtime, &component.cage_readback_object_sha256.clone().ok_or_else(|| invalid("cage readback object is missing"))?, CAGE_READBACK_KIND, component.cage_readback_sha256.as_deref().ok_or_else(|| invalid("cage readback hash is missing"))?)?;
        verify_semantic_json_cas(runtime, &component.bake_worker_result_object_sha256.clone().ok_or_else(|| invalid("bake Worker object is missing"))?, BAKE_WORKER_KIND, component.bake_worker_result_sha256.as_deref().ok_or_else(|| invalid("bake Worker hash is missing"))?)?;
        for hash in &component.bake_output_object_sha256s {
            verify_cas_object(runtime, hash, "image/png", BAKE_MAP_KIND_PREFIX, true, MAX_JSON_BYTES)?;
        }
    }
    Ok(())
}

pub(crate) fn prepare(runtime: &Runtime, value: Value) -> Result<Value, RuntimeError> {
    let request = parse_prepare(&value)?;
    if let Some(existing) = runtime.store.weaponry_knife_uv_bake_v2_get(&request.project_id, &request.idempotency_key)? {
        if existing.input_sha256 != request.input_sha256 {
            return Err(invalid("aggregate idempotency key is bound to another input"));
        }
        revalidate_aggregate(runtime, &existing)?;
        return Ok(seal_result(json!({"schema_version":"WeaponryKnifeUvBakeV2PrepareResult@1","operation":PREPARE_OPERATION,"project_id":existing.project_id,"candidate_id":existing.candidate_id,"candidate_state_sha256":existing.candidate_state_sha256,"aggregate_id":existing.aggregate_id,"record":existing,"replayed":true,"restart_hash_verified":false,"runtime_write_performed":false,"persistent_user_data_touched":false,"uv_status":existing.uv_status,"cage_status":existing.cage_status,"bake_status":existing.bake_status,"quality_status":existing.quality_status,"visual_status":existing.visual_status,"human_status":existing.human_status,"engine_status":existing.engine_status,"commercial_status":existing.commercial_status,"production_stage_advanced":false,"candidate_confirmed":false,"version_created":false,"export_performed":false,"source_only":true,"canonical_sha256":""})));
    }
    let candidate = runtime.candidate(&request.candidate_id)?.ok_or_else(|| invalid("candidate is unavailable"))?;
    if candidate.project_id != request.project_id || candidate.canonical_sha256 != request.candidate_state_sha256 || candidate.base_version_id != request.base_version_id {
        return Err(invalid("candidate project/state/base-version binding differs"));
    }
    let (source_proof, high_glb) = strict_high_source(runtime, &request)?;
    let lows = low_exists(runtime, &request)?;
    if lows.is_empty() {
        let mut result = not_run(&request, "one-or-more-requested-Part-Low-durable-links-unavailable");
        result["canonical_sha256"] = Value::String(canonical_json_hash(&result));
        return Ok(result);
    }
    let mut components = Vec::with_capacity(request.components.len());
    for component in &request.components {
        components.push(process_component(runtime, &request, &high_glb, component)?);
    }
    let source_proof_sha256 = canonical_json_hash(&source_proof);
    let aggregate_seed = json!({"project_id":request.project_id.clone(),"candidate_id":request.candidate_id.clone(),"source_high_artifact_id":request.source_high_artifact_id.clone(),"source_proof_sha256":source_proof_sha256,"components":components.clone(),"input_sha256":request.input_sha256.clone()});
    let aggregate_id = format!("knife-uv-bake-v2-aggregate-{}", &canonical_json_hash(&aggregate_seed)[..24]);
    let receipt_value = json!({"schema_version":"WeaponryKnifeUvBakeV2AggregateReceipt@1","aggregate_id":aggregate_id,"project_id":request.project_id.clone(),"candidate_id":request.candidate_id.clone(),"source_proof":source_proof.clone(),"components":components.clone(),"request_sha256":request.input_sha256.clone(),"quality_status":"structural_only","visual_status":"NOT_PROVEN","human_status":"NOT_RUN","engine_status":"NOT_RUN","commercial_status":"NOT_RUN","production_stage_advanced":false,"candidate_confirmed":false,"version_created":false,"export_performed":false,"canonical_sha256":""});
    let mut receipt_value = receipt_value;
    receipt_value["canonical_sha256"] = Value::String(canonical_json_hash(&receipt_value));
    let (receipt_bytes, receipt_hash) = canonical_result(&receipt_value)?;
    let receipt_object = runtime.store.put_object(&receipt_bytes, Some(&receipt_hash), WEAPONRY_KNIFE_UV_BAKE_V2_RECEIPT_MIME, WEAPONRY_KNIFE_UV_BAKE_V2_RECEIPT_OBJECT_KIND, &super::now_string())?;
    let record_without_hash = WeaponryKnifeUvBakeV2AggregateRecord {
        schema_version:WEAPONRY_KNIFE_UV_BAKE_V2_RECORD_SCHEMA_VERSION.to_owned(), project_id:request.project_id.clone(), candidate_id:request.candidate_id.clone(), candidate_state_sha256:request.candidate_state_sha256.clone(), base_version_id:request.base_version_id.clone(), aggregate_id:aggregate_id.clone(), source_high_artifact_id:request.source_high_artifact_id.clone(), source_high_result_sha256:request.source_high_result_sha256.clone(), source_high_result_object_sha256:request.source_high_result_object_sha256.clone(), source_high_readback_sha256:request.source_high_readback_sha256.clone(), source_high_readback_object_sha256:request.source_high_readback_object_sha256.clone(), source_high_artifact_sha256:request.source_high_artifact_sha256.clone(), source_high_artifact_object_sha256:request.source_high_artifact_object_sha256.clone(), source_high_artifact_readback_sha256:request.source_high_artifact_readback_sha256.clone(), source_high_artifact_readback_object_sha256:request.source_high_artifact_readback_object_sha256.clone(), components, source_proof_sha256, uv_status:"PASS_SOURCE_STRUCTURAL".to_owned(), cage_status:"PASS_SOURCE_STRUCTURAL".to_owned(), bake_status:"PASS_SOURCE_STRUCTURAL".to_owned(), visual_status:"NOT_PROVEN".to_owned(), human_status:"NOT_RUN".to_owned(), engine_status:"NOT_RUN".to_owned(), commercial_status:"NOT_RUN".to_owned(), runtime_write_performed:true, persistent_user_data_touched:true, production_stage_advanced:false, candidate_confirmed:false, version_created:false, export_performed:false, quality_status:"structural_only".to_owned(), request_sha256:request.input_sha256.clone(), input_sha256:request.input_sha256.clone(), idempotency_key:request.idempotency_key.clone(), receipt_object_sha256:receipt_object.record.sha256.clone(), canonical_sha256:"".repeat(64), created_at:super::now_string(),
    };
    let mut record = record_without_hash;
    let mut record_value = serde_json::to_value(&record).map_err(|error| invalid(error.to_string()))?;
    record_value["canonical_sha256"] = Value::String(String::new());
    record.canonical_sha256 = canonical_json_hash(&record_value);
    let (stored, replayed) = runtime.store.weaponry_knife_uv_bake_v2_commit(&record, &receipt_object.record)?;
    Ok(seal_result(json!({"schema_version":"WeaponryKnifeUvBakeV2PrepareResult@1","operation":PREPARE_OPERATION,"project_id":stored.project_id,"candidate_id":stored.candidate_id,"candidate_state_sha256":stored.candidate_state_sha256,"aggregate_id":stored.aggregate_id,"record":stored,"replayed":replayed,"restart_hash_verified":false,"runtime_write_performed":!replayed,"persistent_user_data_touched":!replayed,"uv_status":stored.uv_status,"cage_status":stored.cage_status,"bake_status":stored.bake_status,"quality_status":stored.quality_status,"visual_status":stored.visual_status,"human_status":stored.human_status,"engine_status":stored.engine_status,"commercial_status":stored.commercial_status,"production_stage_advanced":false,"candidate_confirmed":false,"version_created":false,"export_performed":false,"source_only":true,"canonical_sha256":""})))
}

pub(crate) fn get(runtime: &Runtime, value: Value) -> Result<Value, RuntimeError> {
    let object = parse_get(&value)?;
    let record = runtime.store.weaponry_knife_uv_bake_v2_get(
        required_id(&object, "project_id")?.as_str(), required_id(&object, "idempotency_key")?.as_str(),
    )?.ok_or_else(|| invalid("V2 UV/Cage/Bake aggregate is unavailable"))?;
    if record.aggregate_id != required_id(&object, "aggregate_id")?
        || record.candidate_id != required_id(&object, "candidate_id")?
        || record.candidate_state_sha256 != required_hash(&object, "candidate_state_sha256")?
    {
        return Err(invalid("aggregate get binding differs"));
    }
    revalidate_aggregate(runtime, &record)?;
    // A single GET cannot prove that the caller crossed a fresh-process
    // boundary.  Keep the transport flag false; the live probe records the
    // independent close/reopen proof beside the exact post-restart GET.
    Ok(seal_result(json!({"schema_version":"WeaponryKnifeUvBakeV2GetResult@1","operation":GET_OPERATION,"record":record,"replayed":false,"restart_hash_verified":false,"runtime_write_performed":false,"persistent_user_data_touched":false,"production_stage_advanced":false,"candidate_confirmed":false,"version_created":false,"export_performed":false,"source_only":true,"canonical_sha256":""})))
}

impl Runtime {
    /// Source-only V2 aggregate operation routed by the existing Surface
    /// façade once the live Low component set is present.
    pub(crate) fn production_knife_uv_bake_v2_prepare(&self, value: Value) -> Result<Value, RuntimeError> {
        prepare(self, value)
    }

    pub(crate) fn production_knife_uv_bake_v2_get(&self, value: Value) -> Result<Value, RuntimeError> {
        get(self, value)
    }
}
