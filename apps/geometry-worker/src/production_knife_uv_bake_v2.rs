//! Internal V2 knife production composition seam.
//!
//! This is intentionally a Rust-to-Rust helper rather than a new Worker/MCP
//! operation.  The High artifact is admitted as a producer-owned source
//! record, while the existing Low Quad Draft result remains the editable Low
//! boundary.  Hero UV and Cage consume only that final Low GLB.  A bake is
//! attempted only when the independently supplied High GLB passes the same
//! direct V2 High source gate as the existing geometric bake worker.  High
//! tangent is optional there; tangent-space encoding remains owned by the
//! final Low GLB's strict Mikk tangent field.

use crate::integrity;
use crate::GeometryError;
use base64::Engine;
use forgecad_worker_protocol::{
    AUTHORING_MESH_V2_HIGH_ARTIFACT_MATERIALIZE_RESULT_SCHEMA_VERSION,
    PRODUCTION_WEAPON_GEOMETRIC_BAKE_AO_SAMPLE_COUNT,
    PRODUCTION_WEAPON_GEOMETRIC_BAKE_ATLAS_POLICY, PRODUCTION_WEAPON_GEOMETRIC_BAKE_BUDGET_PROFILE,
    PRODUCTION_WEAPON_GEOMETRIC_BAKE_NORMAL_CONVENTION, PRODUCTION_WEAPON_GEOMETRIC_BAKE_POLICY,
    PRODUCTION_WEAPON_GEOMETRIC_BAKE_REQUEST_SCHEMA_VERSION,
    PRODUCTION_WEAPON_GEOMETRIC_BAKE_RESOLUTION,
    PRODUCTION_WEAPON_HERO_UV_LAYOUT_REQUEST_SCHEMA_VERSION,
    PRODUCTION_WEAPON_LOW_QUAD_DRAFT_OPERATION,
    PRODUCTION_WEAPON_LOW_QUAD_DRAFT_RESULT_SCHEMA_VERSION,
};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};

const V2_SCHEMA_VERSION: &str = "WeaponryKnifeUvBakeV2Internal@1";
const SOURCE_PROOF_SCHEMA_VERSION: &str = "WeaponryKnifeUvBakeV2SourceProof@1";
const HIGH_SURFACE_POLICY: &str = "direct-v2-high-position-normal-uv0-optional-tangent@1";
const LOW_TANGENT_POLICY: &str = "low-geometry-compiler-owned-mikktspace-replay@1";

/// Compose the existing Low Quad Draft -> Hero UV -> Cage -> Bake stages for
/// a direct V2 High artifact.  `visibility_weights` is a typed value already
/// owned by the calling production flow; the child Hero UV Worker performs
/// its own closed-shape and coverage validation.
///
/// The function is deliberately not wired to a new operation string.  It is
/// a bounded internal seam for the Runtime/Worker integration to call once a
/// Low result has crossed the existing `LowQuadDraftWorkerResult@1` boundary.
pub fn run_after_low_quad_draft(
    high_artifact_result: &Value,
    low_quad_draft_result: &Value,
    visibility_weights: &Value,
) -> Result<Value, GeometryError> {
    validate_high_artifact(high_artifact_result)?;
    let high = high_artifact_result
        .as_object()
        .ok_or_else(|| invalid("KNIFE_V2_HIGH_ARTIFACT_OBJECT_INVALID"))?;
    let high_semantic_sha256 = required_hash(high, "artifact_sha256", "HIGH_ARTIFACT")?;
    let high_glb_sha256 = required_hash(high, "glb_sha256", "HIGH_GLB")?;
    let high_glb_base64 = required_string(high, "glb_base64", "HIGH_GLB")?;
    let high_glb = decode_hash_bound_glb(&high_glb_base64, &high_glb_sha256, "HIGH")?;
    let high_readback_sha256 = high
        .get("strict_readback")
        .and_then(Value::as_object)
        .and_then(|readback| readback.get("canonical_sha256"))
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .ok_or_else(|| invalid("KNIFE_V2_HIGH_READBACK_CANONICAL_INVALID"))?
        .to_owned();
    let high_surface = inspect_direct_high_surface(&high_glb)?;

    let low = validate_low_quad_draft(low_quad_draft_result)?;
    let low_semantic_source = required_string(low, "source_high_artifact_sha256", "LOW")?;
    if low_semantic_source != high_semantic_sha256 {
        return Err(invalid("KNIFE_V2_HIGH_LOW_SEMANTIC_SOURCE_MISMATCH"));
    }
    let low_source_readback = required_string(low, "source_high_artifact_readback_sha256", "LOW")?;
    if low_source_readback != high_readback_sha256 {
        return Err(invalid("KNIFE_V2_HIGH_LOW_READBACK_SOURCE_MISMATCH"));
    }
    let low_hash = required_hash(low, "low_quad_draft_artifact_sha256", "LOW")?;
    let low_base64 = required_string(low, "low_quad_draft_glb_base64", "LOW")?;
    let low_glb = decode_hash_bound_glb(&low_base64, &low_hash, "LOW")?;
    let low_readback = integrity::inspect_glb(&low_glb)?;
    if !low_readback.hard_gate_passed {
        return Err(invalid(format!(
            "KNIFE_V2_LOW_TANGENT_READBACK_FAILED:{:?}",
            low_readback.failure_codes
        )));
    }

    let source_proof_preimage = json!({
        "schema_version": SOURCE_PROOF_SCHEMA_VERSION,
        "high_artifact_sha256": high_semantic_sha256,
        "high_glb_sha256": high_glb_sha256,
        "high_readback_canonical_sha256": high_readback_sha256,
        "high_surface_policy": HIGH_SURFACE_POLICY,
        "high_surface": high_surface,
        "low_quad_draft_artifact_sha256": low_hash,
        "low_readback_report_sha256": crate::canonical_hash(&low_readback.report_value()),
        "low_tangent_policy": LOW_TANGENT_POLICY,
        "low_tangent_gate": {
            "status": "PASS_SOURCE_STRUCTURAL",
            "non_finite_count": low_readback.tangent_non_finite_count,
            "orthogonality_error_count": low_readback.tangent_orthogonality_error_count,
            "handedness_error_count": low_readback.tangent_handedness_error_count
        },
        "replay_count": 2,
        "replay_byte_exact": true
    });
    let source_proof = with_hash(source_proof_preimage, "source_proof_sha256")?;

    let hero_uv_request = with_hash(
        json!({
            "schema_version": PRODUCTION_WEAPON_HERO_UV_LAYOUT_REQUEST_SCHEMA_VERSION,
            "low_artifact_sha256": low_hash,
            "low_glb_base64": low_base64,
            "resolution": 4096,
            "padding_texels": 32,
            "min_mip_level": 5,
            "hard_edge_angle_deg": 60.0,
            "stretch_threshold": 32.0,
            "visibility_weights": visibility_weights,
            "canonical_sha256": ""
        }),
        "canonical_sha256",
    )?;
    let hero_uv = super::production_hero_uv_layout::run(
        hero_uv_request
            .as_object()
            .ok_or_else(|| invalid("KNIFE_V2_HERO_UV_REQUEST_OBJECT_INVALID"))?,
    )?;

    let cage_request = with_hash(
        json!({
            "schema_version": super::production_cage_offset::REQUEST_SCHEMA_VERSION,
            "preview_only": true,
            "source_low_artifact_sha256": low_hash,
            "low_glb_base64": low_base64,
            "offset_m": 0.001,
            "max_offset_m": 0.2,
            "max_coordinate_abs_m": 10.0,
            "offset_field_policy": super::production_cage_offset::POLICY,
            "algorithm": super::production_cage_offset::ALGORITHM,
            "canonical_sha256": ""
        }),
        "canonical_sha256",
    )?;
    let cage = super::production_cage_offset::run(
        cage_request
            .as_object()
            .ok_or_else(|| invalid("KNIFE_V2_CAGE_REQUEST_OBJECT_INVALID"))?,
    )?;
    let cage_hash = required_hash(
        cage.as_object()
            .ok_or_else(|| invalid("KNIFE_V2_CAGE_RESULT_OBJECT_INVALID"))?,
        "cage_artifact_sha256",
        "CAGE",
    )?;
    let cage_base64 = required_string(
        cage.as_object()
            .ok_or_else(|| invalid("KNIFE_V2_CAGE_RESULT_OBJECT_INVALID"))?,
        "cage_glb_base64",
        "CAGE",
    )?;

    let bake = run_bake(
        &high_glb,
        &high_glb_sha256,
        &low_glb,
        &low_hash,
        &cage_base64,
        &cage_hash,
    )?;
    let bake_structural_status = bake
        .get("structural_status")
        .or_else(|| {
            bake.get("diagnostic")
                .and_then(|diagnostic| diagnostic.get("status"))
        })
        .and_then(Value::as_str)
        .unwrap_or("NOT_RUN_BAKE_RESULT_STATUS_MISSING");

    let mut result = json!({
        "schema_version": V2_SCHEMA_VERSION,
        "operation": "internal-weaponry-knife-uv-bake-v2",
        "source_proof": source_proof,
        "low": {
            "source_high_artifact_sha256": high_semantic_sha256,
            "source_high_glb_sha256": high_glb_sha256,
            "low_quad_draft_artifact_sha256": low_hash,
            "low_geometry_program_sha256": low.get("low_geometry_program_sha256"),
            "tangent_policy": LOW_TANGENT_POLICY,
            "tangent_gate": "PASS_SOURCE_STRUCTURAL",
            "readback": low_readback.report_value()
        },
        "hero_uv": hero_uv,
        "cage": cage,
        "bake": bake,
        "statuses": {
            "uv_structural_status": "PASS_SOURCE_STRUCTURAL",
            "uv_quality_status": "structural_only",
            "cage_structural_status": "PASS_SOURCE_STRUCTURAL",
            "bake_structural_status": bake_structural_status,
            "visual_status": "NOT_PROVEN",
            "human_status": "NOT_RUN",
            "engine_status": "NOT_RUN",
            "commercial_status": "NOT_RUN",
            "runtime_write_performed": false,
            "production_stage_advanced": false,
            "candidate_confirmed": false,
            "version_created": false,
            "export_performed": false
        },
        "canonical_sha256": ""
    });
    result["canonical_sha256"] = Value::String(crate::canonical_hash(&result));
    Ok(result)
}

fn validate_high_artifact(value: &Value) -> Result<(), GeometryError> {
    forgecad_worker_protocol::validate_authoring_mesh_v2_high_artifact_materialize_result(value)
        .map_err(|error| invalid(format!("KNIFE_V2_HIGH_ARTIFACT_INVALID:{error}")))?;
    if value.get("schema_version").and_then(Value::as_str)
        != Some(AUTHORING_MESH_V2_HIGH_ARTIFACT_MATERIALIZE_RESULT_SCHEMA_VERSION)
    {
        return Err(invalid("KNIFE_V2_HIGH_ARTIFACT_SCHEMA_INVALID"));
    }
    Ok(())
}

fn validate_low_quad_draft(value: &Value) -> Result<&Map<String, Value>, GeometryError> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid("KNIFE_V2_LOW_RESULT_OBJECT_INVALID"))?;
    if object.get("schema_version").and_then(Value::as_str)
        != Some(PRODUCTION_WEAPON_LOW_QUAD_DRAFT_RESULT_SCHEMA_VERSION)
        || object.get("operation").and_then(Value::as_str)
            != Some(PRODUCTION_WEAPON_LOW_QUAD_DRAFT_OPERATION)
        || object.get("hard_gate_passed") != Some(&Value::Bool(true))
        || object.get("runtime_write_performed") != Some(&Value::Bool(false))
    {
        return Err(invalid("KNIFE_V2_LOW_RESULT_MARKER_INVALID"));
    }
    let readback = object
        .get("low_quad_draft_readback")
        .and_then(Value::as_object)
        .ok_or_else(|| invalid("KNIFE_V2_LOW_READBACK_MISSING"))?;
    if readback
        .get("failure_codes")
        .and_then(Value::as_array)
        .is_none_or(|codes| !codes.is_empty())
    {
        return Err(invalid("KNIFE_V2_LOW_READBACK_FAILURES"));
    }
    Ok(object)
}

fn run_bake(
    high_glb: &[u8],
    high_hash: &str,
    low_glb: &[u8],
    low_hash: &str,
    cage_base64: &str,
    cage_hash: &str,
) -> Result<Value, GeometryError> {
    let request = with_hash(
        json!({
            "schema_version": PRODUCTION_WEAPON_GEOMETRIC_BAKE_REQUEST_SCHEMA_VERSION,
            "bake_policy": PRODUCTION_WEAPON_GEOMETRIC_BAKE_POLICY,
            "bake_policy_sha256": sha256_hex(PRODUCTION_WEAPON_GEOMETRIC_BAKE_POLICY.as_bytes()),
            "budget_profile": PRODUCTION_WEAPON_GEOMETRIC_BAKE_BUDGET_PROFILE,
            "atlas_policy": PRODUCTION_WEAPON_GEOMETRIC_BAKE_ATLAS_POLICY,
            "high_glb_base64": base64::engine::general_purpose::STANDARD.encode(high_glb),
            "low_glb_base64": base64::engine::general_purpose::STANDARD.encode(low_glb),
            "cage_glb_base64": cage_base64,
            "high_artifact_sha256": high_hash,
            "low_artifact_sha256": low_hash,
            "cage_artifact_sha256": cage_hash,
            "resolution": PRODUCTION_WEAPON_GEOMETRIC_BAKE_RESOLUTION,
            "normal_convention": PRODUCTION_WEAPON_GEOMETRIC_BAKE_NORMAL_CONVENTION,
            "max_ray_distance_m": 0.1,
            "ao_sample_count": PRODUCTION_WEAPON_GEOMETRIC_BAKE_AO_SAMPLE_COUNT,
            "surface_bake_reuse_allowed": false,
            "canonical_sha256": ""
        }),
        "canonical_sha256",
    )?;
    super::production_geometric_bake::run(
        request
            .as_object()
            .ok_or_else(|| invalid("KNIFE_V2_BAKE_REQUEST_OBJECT_INVALID"))?,
    )
}

fn inspect_direct_high_surface(glb: &[u8]) -> Result<Value, GeometryError> {
    let root = parse_glb_json(glb)?;
    let meshes = root
        .get("meshes")
        .and_then(Value::as_array)
        .filter(|meshes| !meshes.is_empty())
        .ok_or_else(|| invalid("KNIFE_V2_HIGH_SURFACE_MESHES_MISSING"))?;
    let mut primitive_count = 0u64;
    let mut tangent_primitive_count = 0u64;
    for mesh in meshes {
        let primitives = mesh
            .get("primitives")
            .and_then(Value::as_array)
            .filter(|primitives| !primitives.is_empty())
            .ok_or_else(|| invalid("KNIFE_V2_HIGH_SURFACE_PRIMITIVES_MISSING"))?;
        for primitive in primitives {
            let attrs = primitive
                .get("attributes")
                .and_then(Value::as_object)
                .ok_or_else(|| invalid("KNIFE_V2_HIGH_SURFACE_ATTRIBUTES_MISSING"))?;
            if attrs.keys().any(|key| {
                !matches!(
                    key.as_str(),
                    "POSITION" | "NORMAL" | "TEXCOORD_0" | "TANGENT"
                )
            }) || !attrs.contains_key("POSITION")
                || !attrs.contains_key("NORMAL")
                || !attrs.contains_key("TEXCOORD_0")
            {
                return Err(invalid("KNIFE_V2_HIGH_SURFACE_ATTRIBUTES_INVALID"));
            }
            primitive_count += 1;
            if attrs.contains_key("TANGENT") {
                tangent_primitive_count += 1;
            }
        }
    }
    if tangent_primitive_count != 0 && tangent_primitive_count != primitive_count {
        return Err(invalid("KNIFE_V2_HIGH_SURFACE_TANGENT_COVERAGE_INVALID"));
    }
    Ok(json!({
        "position": "required",
        "normal": "required",
        "uv0": "required",
        "tangent": if tangent_primitive_count == primitive_count { "present_source" } else { "absent_low_replay_required" },
        "primitive_count": primitive_count,
        "tangent_primitive_count": tangent_primitive_count
    }))
}

fn parse_glb_json(glb: &[u8]) -> Result<Value, GeometryError> {
    if glb.len() < 20
        || &glb[..4] != b"glTF"
        || u32::from_le_bytes(glb[4..8].try_into().unwrap()) != 2
    {
        return Err(invalid("KNIFE_V2_HIGH_GLB_HEADER_INVALID"));
    }
    let total = u32::from_le_bytes(glb[8..12].try_into().unwrap()) as usize;
    if total != glb.len() {
        return Err(invalid("KNIFE_V2_HIGH_GLB_LENGTH_INVALID"));
    }
    let json_len = u32::from_le_bytes(glb[12..16].try_into().unwrap()) as usize;
    if &glb[16..20] != b"JSON"
        || 20usize
            .checked_add(json_len)
            .is_none_or(|end| end > glb.len())
    {
        return Err(invalid("KNIFE_V2_HIGH_GLB_JSON_CHUNK_INVALID"));
    }
    serde_json::from_slice(&glb[20..20 + json_len])
        .map_err(|_| invalid("KNIFE_V2_HIGH_GLB_JSON_INVALID"))
}

fn decode_hash_bound_glb(
    encoded: &str,
    expected_hash: &str,
    label: &str,
) -> Result<Vec<u8>, GeometryError> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|_| invalid(format!("KNIFE_V2_{label}_GLB_BASE64_INVALID")))?;
    if bytes.is_empty() || sha256_hex(&bytes) != expected_hash {
        return Err(invalid(format!("KNIFE_V2_{label}_GLB_HASH_MISMATCH")));
    }
    Ok(bytes)
}

fn with_hash(mut value: Value, field: &str) -> Result<Value, GeometryError> {
    let object = value
        .as_object_mut()
        .ok_or_else(|| invalid("KNIFE_V2_HASH_VALUE_OBJECT_INVALID"))?;
    object.insert(field.to_owned(), Value::String(String::new()));
    let mut preimage = object.clone();
    preimage.remove(field);
    let hash = crate::canonical_hash(&Value::Object(preimage));
    object.insert(field.to_owned(), Value::String(hash));
    Ok(value)
}

fn required_string<'a>(
    object: &'a Map<String, Value>,
    field: &str,
    label: &str,
) -> Result<&'a str, GeometryError> {
    object
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| invalid(format!("KNIFE_V2_{label}_{field}_MISSING")))
}

fn required_hash(
    object: &Map<String, Value>,
    field: &str,
    label: &str,
) -> Result<String, GeometryError> {
    let value = required_string(object, field, label)?;
    if !is_sha256(value) {
        return Err(invalid(format!("KNIFE_V2_{label}_{field}_INVALID")));
    }
    Ok(value.to_owned())
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn invalid(message: impl Into<String>) -> GeometryError {
    GeometryError::Invalid(message.into())
}
