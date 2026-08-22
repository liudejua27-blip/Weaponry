//! Runtime-owned structural material-surface quality for the production
//! `topology -> material-surface` boundary.
//!
//! The topology source and Appearance output are deliberately distinct
//! candidates. This module proves their renderable mesh/accessor bytes are
//! identical while independently revalidating the durable 2K PBR provenance
//! chain. It never advances the production head and never claims visual,
//! artistic, commercial-FPS, human-review or commercial-engine quality.

use super::{
    canonical_json_bytes, canonical_json_hash, exact_object, is_opaque_id, is_sha256, sha256_hex,
    strict_glb_inspection, validate_artifact_readback_v2_output, Runtime, RuntimeError,
    MAX_DERIVED_JSON_BYTES, MAX_GEOMETRY_ARTIFACT_BYTES,
};
use forgecad_contracts::{
    CandidateMaterialSurfaceQualityGetRequest, CandidateMaterialSurfaceQualityHardGate,
    CandidateMaterialSurfaceQualityPrepareRequest, CandidateMaterialSurfaceQualityRecord,
};
use forgecad_store::CasObject;
use serde_json::{json, Map, Value};
use std::collections::BTreeSet;

const PREPARE_SCHEMA: &str = "CandidateMaterialSurfaceQualityPrepareRequest@1";
const GET_SCHEMA: &str = "CandidateMaterialSurfaceQualityGetRequest@1";
const PREPARE_RESULT_SCHEMA: &str = "CandidateMaterialSurfaceQualityPrepareResult@1";
const GET_RESULT_SCHEMA: &str = "CandidateMaterialSurfaceQualityGetResult@1";
const REPORT_KIND: &str = "candidate-material-surface-quality-report";
const JSON_MIME: &str = "application/json";
const MAX_REPORT_BYTES: u64 = 1024 * 1024;
const POLICY: &str = "candidate-material-surface-structural-hard-gate@1";
const MATERIALIZATION_STATUS: &str = "runtime-owned-durable-candidate-material-surface-quality";
const MATERIAL_PACK_ID: &str = "forgecad-fictional-energy-weapon-2k";
const MATERIAL_PACK_VERSION: &str = "1.0.0";
const MATERIAL_PACK_LICENSE: &str = "CC0-1.0";

const PREPARE_FIELDS: &[&str] = &[
    "schema_version",
    "material_surface_quality_id",
    "project_id",
    "source_candidate_id",
    "source_candidate_state_sha256",
    "source_artifact_id",
    "source_artifact_sha256",
    "source_artifact_readback_sha256",
    "source_artifact_readback_object_sha256",
    "source_geometry_candidate_evidence_sha256",
    "source_geometry_program_sha256",
    "source_topology_quality_id",
    "source_topology_quality_report_object_sha256",
    "source_topology_quality_canonical_sha256",
    "output_candidate_id",
    "output_candidate_state_sha256",
    "output_artifact_id",
    "output_artifact_sha256",
    "output_artifact_readback_sha256",
    "output_artifact_readback_object_sha256",
    "output_geometry_program_sha256",
    "appearance_source_lineage_sidecar_object_sha256",
    "appearance_source_lineage_canonical_sha256",
    "appearance_program_object_sha256",
    "appearance_program_sha256",
    "material_layer_stack_sha256",
    "material_pack_manifest_object_sha256",
    "material_pack_manifest_sha256",
    "material_pack_provenance_sha256",
    "texture_build_receipt_object_sha256",
    "texture_build_receipt_canonical_sha256",
    "candidate_surface_bake_receipt_object_sha256",
    "candidate_surface_bake_receipt_canonical_sha256",
    "uv_binding_sha256",
    "tangent_binding_sha256",
    "material_zone_inventory_sha256",
    "material_provenance_sha256",
    "lod_scope",
    "geometry_preservation_projection_sha256",
    "material_surface_quality_policy",
    "material_surface_quality_policy_sha256",
    "from_stage",
    "to_stage",
    "input_sha256",
    "idempotency_key",
];

fn invalid(message: impl Into<String>) -> RuntimeError {
    RuntimeError::InvalidInput(format!(
        "CANDIDATE_MATERIAL_SURFACE_QUALITY_INVALID: {}",
        message.into()
    ))
}

fn validate_ids_and_hashes(
    request: &CandidateMaterialSurfaceQualityPrepareRequest,
) -> Result<(), RuntimeError> {
    let ids = [
        request.material_surface_quality_id.as_str(),
        request.project_id.as_str(),
        request.source_candidate_id.as_str(),
        request.source_artifact_id.as_str(),
        request.source_topology_quality_id.as_str(),
        request.output_candidate_id.as_str(),
        request.output_artifact_id.as_str(),
        request.idempotency_key.as_str(),
    ];
    let hashes = [
        request.source_candidate_state_sha256.as_str(),
        request.source_artifact_sha256.as_str(),
        request.source_artifact_readback_sha256.as_str(),
        request.source_artifact_readback_object_sha256.as_str(),
        request.source_geometry_candidate_evidence_sha256.as_str(),
        request.source_geometry_program_sha256.as_str(),
        request
            .source_topology_quality_report_object_sha256
            .as_str(),
        request.source_topology_quality_canonical_sha256.as_str(),
        request.output_candidate_state_sha256.as_str(),
        request.output_artifact_sha256.as_str(),
        request.output_artifact_readback_sha256.as_str(),
        request.output_artifact_readback_object_sha256.as_str(),
        request.output_geometry_program_sha256.as_str(),
        request
            .appearance_source_lineage_sidecar_object_sha256
            .as_str(),
        request.appearance_source_lineage_canonical_sha256.as_str(),
        request.appearance_program_object_sha256.as_str(),
        request.appearance_program_sha256.as_str(),
        request.material_layer_stack_sha256.as_str(),
        request.material_pack_manifest_object_sha256.as_str(),
        request.material_pack_manifest_sha256.as_str(),
        request.material_pack_provenance_sha256.as_str(),
        request.texture_build_receipt_object_sha256.as_str(),
        request.texture_build_receipt_canonical_sha256.as_str(),
        request
            .candidate_surface_bake_receipt_object_sha256
            .as_str(),
        request
            .candidate_surface_bake_receipt_canonical_sha256
            .as_str(),
        request.uv_binding_sha256.as_str(),
        request.tangent_binding_sha256.as_str(),
        request.material_zone_inventory_sha256.as_str(),
        request.material_provenance_sha256.as_str(),
        request.geometry_preservation_projection_sha256.as_str(),
        request.material_surface_quality_policy_sha256.as_str(),
        request.input_sha256.as_str(),
    ];
    if ids.iter().any(|value| !is_opaque_id(value)) {
        return Err(invalid("one or more identifiers are malformed"));
    }
    if hashes.iter().any(|value| !is_sha256(value)) {
        return Err(invalid("one or more SHA-256 bindings are malformed"));
    }
    if request.source_candidate_id == request.output_candidate_id {
        return Err(invalid("source and output candidates must be distinct"));
    }
    if request.lod_scope != "lod0-only@1"
        || request.material_surface_quality_policy != POLICY
        || request.material_surface_quality_policy_sha256 != sha256_hex(POLICY.as_bytes())
        || request.from_stage != "topology"
        || request.to_stage != "material-surface"
    {
        return Err(invalid("policy, LOD scope or stage binding differs"));
    }
    Ok(())
}

fn prepare_request(
    value: &Value,
) -> Result<(CandidateMaterialSurfaceQualityPrepareRequest, String), RuntimeError> {
    let object = exact_object(value, PREPARE_FIELDS, PREPARE_SCHEMA)?;
    if object.get("schema_version").and_then(Value::as_str) != Some(PREPARE_SCHEMA) {
        return Err(invalid("prepare schema_version differs"));
    }
    let request: CandidateMaterialSurfaceQualityPrepareRequest =
        serde_json::from_value(value.clone())
            .map_err(|error| invalid(format!("prepare request is malformed: {error}")))?;
    validate_ids_and_hashes(&request)?;
    let mut input = object.clone();
    input.remove("input_sha256");
    input.remove("idempotency_key");
    let request_sha256 = canonical_json_hash(&Value::Object(input));
    if request.input_sha256 != request_sha256 {
        return Err(invalid(format!(
            "input_sha256 differs; expected {request_sha256}"
        )));
    }
    Ok((request, request_sha256))
}

fn get_request(value: &Value) -> Result<CandidateMaterialSurfaceQualityGetRequest, RuntimeError> {
    let object = exact_object(
        value,
        &[
            "schema_version",
            "material_surface_quality_id",
            "project_id",
            "source_candidate_id",
            "output_candidate_id",
        ],
        GET_SCHEMA,
    )?;
    if object.get("schema_version").and_then(Value::as_str) != Some(GET_SCHEMA) {
        return Err(invalid("get schema_version differs"));
    }
    let request: CandidateMaterialSurfaceQualityGetRequest = serde_json::from_value(value.clone())
        .map_err(|error| invalid(format!("get request is malformed: {error}")))?;
    if !is_opaque_id(&request.material_surface_quality_id)
        || !is_opaque_id(&request.project_id)
        || !is_opaque_id(&request.source_candidate_id)
        || !is_opaque_id(&request.output_candidate_id)
        || request.source_candidate_id == request.output_candidate_id
    {
        return Err(invalid("get scope is malformed"));
    }
    Ok(request)
}

fn read_json(runtime: &Runtime, sha256: &str) -> Result<Value, RuntimeError> {
    let bytes = runtime.cas_read_bounded(sha256, MAX_DERIVED_JSON_BYTES)?;
    serde_json::from_slice(&bytes)
        .map_err(|error| invalid(format!("durable JSON object is invalid: {error}")))
}

fn readback_binding(
    runtime: &Runtime,
    object_sha256: &str,
    expected_candidate_id: &str,
    expected_artifact_id: &str,
    expected_artifact_sha256: &str,
    expected_readback_sha256: &str,
    expected_program_sha256: &str,
) -> Result<Value, RuntimeError> {
    let value = read_json(runtime, object_sha256)?;
    validate_artifact_readback_v2_output(&value)?;
    if value.get("candidate_id").and_then(Value::as_str) != Some(expected_candidate_id)
        || value.get("artifact_id").and_then(Value::as_str) != Some(expected_artifact_id)
        || value.get("object_sha256").and_then(Value::as_str) != Some(expected_artifact_sha256)
        || value.get("canonical_sha256").and_then(Value::as_str) != Some(expected_readback_sha256)
        || value.get("program_sha256").and_then(Value::as_str) != Some(expected_program_sha256)
        || value.get("validator_status").and_then(Value::as_str) != Some("passed")
        || value.get("hard_gate_passed").and_then(Value::as_bool) != Some(true)
    {
        return Err(invalid("ArtifactReadback@2 binding or hard gate differs"));
    }
    Ok(value)
}

fn le_u32(bytes: &[u8], offset: usize) -> Result<usize, RuntimeError> {
    let raw: [u8; 4] = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| invalid("GLB header is truncated"))?
        .try_into()
        .map_err(|_| invalid("GLB header is truncated"))?;
    Ok(u32::from_le_bytes(raw) as usize)
}

fn parse_glb(bytes: &[u8]) -> Result<(Value, &[u8]), RuntimeError> {
    if bytes.len() < 20 || &bytes[0..4] != b"glTF" || le_u32(bytes, 4)? != 2 {
        return Err(invalid("artifact is not a GLB v2 object"));
    }
    if le_u32(bytes, 8)? != bytes.len() {
        return Err(invalid("GLB declared length differs"));
    }
    let json_length = le_u32(bytes, 12)?;
    if bytes.get(16..20) != Some(&b"JSON"[..]) || 20 + json_length + 8 > bytes.len() {
        return Err(invalid("GLB JSON chunk is invalid"));
    }
    let json: Value = serde_json::from_slice(&bytes[20..20 + json_length])
        .map_err(|error| invalid(format!("GLB JSON is invalid: {error}")))?;
    let bin_header = 20 + json_length;
    let bin_length = le_u32(bytes, bin_header)?;
    if bytes.get(bin_header + 4..bin_header + 8) != Some(&b"BIN\0"[..])
        || bin_header + 8 + bin_length != bytes.len()
    {
        return Err(invalid("GLB BIN chunk is invalid"));
    }
    Ok((json, &bytes[bin_header + 8..]))
}

fn element_width(accessor: &Map<String, Value>) -> Result<usize, RuntimeError> {
    let components = match accessor.get("type").and_then(Value::as_str) {
        Some("SCALAR") => 1,
        Some("VEC2") => 2,
        Some("VEC3") => 3,
        Some("VEC4") => 4,
        _ => return Err(invalid("renderable accessor type is unsupported")),
    };
    let component_width = match accessor.get("componentType").and_then(Value::as_u64) {
        Some(5121) => 1,
        Some(5123) => 2,
        Some(5125) | Some(5126) => 4,
        _ => return Err(invalid("renderable accessor component type is unsupported")),
    };
    Ok(components * component_width)
}

fn renderable_projection(root: &Value, binary: &[u8]) -> Result<Value, RuntimeError> {
    let root_object = root
        .as_object()
        .ok_or_else(|| invalid("GLB JSON root is not an object"))?;
    let meshes = root_object
        .get("meshes")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("GLB meshes are unavailable"))?;
    let accessors = root_object
        .get("accessors")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("GLB accessors are unavailable"))?;
    let views = root_object
        .get("bufferViews")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("GLB bufferViews are unavailable"))?;
    let mut referenced_accessors = BTreeSet::new();
    let mut mesh_projection = meshes.clone();
    for mesh in &mut mesh_projection {
        let primitives = mesh
            .get_mut("primitives")
            .and_then(Value::as_array_mut)
            .ok_or_else(|| invalid("GLB mesh primitives are invalid"))?;
        for primitive in primitives {
            let object = primitive
                .as_object_mut()
                .ok_or_else(|| invalid("GLB primitive is invalid"))?;
            object.remove("material");
            // Material-surface authoring is allowed to repack UVs and
            // regenerate MikkTSpace tangents. Geometry preservation therefore
            // binds the renderable scene/mesh structure plus positions,
            // normals and indices; UV/tangent integrity is checked
            // independently below.
            let attributes = object
                .get_mut("attributes")
                .and_then(Value::as_object_mut)
                .ok_or_else(|| invalid("GLB primitive attributes are invalid"))?;
            attributes.retain(|semantic, _| matches!(semantic.as_str(), "POSITION" | "NORMAL"));
            for index in attributes.values() {
                referenced_accessors.insert(
                    index
                        .as_u64()
                        .ok_or_else(|| invalid("GLB attribute accessor is invalid"))?
                        as usize,
                );
            }
            referenced_accessors.insert(
                object
                    .get("indices")
                    .and_then(Value::as_u64)
                    .ok_or_else(|| invalid("GLB primitive index accessor is invalid"))?
                    as usize,
            );
            if let Some(extras) = object.get_mut("extras").and_then(Value::as_object_mut) {
                extras.remove("uv_chart_assignment_sha256");
                extras.remove("uv_chart_ids");
            }
        }
    }
    let mut accessor_projection = Vec::with_capacity(referenced_accessors.len());
    for index in referenced_accessors {
        let accessor = accessors
            .get(index)
            .and_then(Value::as_object)
            .ok_or_else(|| invalid("GLB accessor index is out of range"))?;
        let view_index = accessor
            .get("bufferView")
            .and_then(Value::as_u64)
            .ok_or_else(|| invalid("GLB accessor bufferView is invalid"))?
            as usize;
        let view = views
            .get(view_index)
            .and_then(Value::as_object)
            .ok_or_else(|| invalid("GLB bufferView index is out of range"))?;
        let view_offset = view.get("byteOffset").and_then(Value::as_u64).unwrap_or(0) as usize;
        let accessor_offset = accessor
            .get("byteOffset")
            .and_then(Value::as_u64)
            .unwrap_or(0) as usize;
        let count = accessor
            .get("count")
            .and_then(Value::as_u64)
            .ok_or_else(|| invalid("GLB accessor count is invalid"))? as usize;
        let width = element_width(accessor)?;
        let stride = view
            .get("byteStride")
            .and_then(Value::as_u64)
            .unwrap_or(width as u64) as usize;
        if stride < width {
            return Err(invalid("GLB accessor stride is invalid"));
        }
        let start = view_offset
            .checked_add(accessor_offset)
            .ok_or_else(|| invalid("GLB accessor offset overflows"))?;
        let mut packed = Vec::with_capacity(count.saturating_mul(width));
        for item in 0..count {
            let offset = start
                .checked_add(item.saturating_mul(stride))
                .ok_or_else(|| invalid("GLB accessor range overflows"))?;
            let end = offset
                .checked_add(width)
                .ok_or_else(|| invalid("GLB accessor range overflows"))?;
            let bytes = binary
                .get(offset..end)
                .ok_or_else(|| invalid("GLB accessor range exceeds BIN"))?;
            packed.extend_from_slice(bytes);
        }
        accessor_projection.push(json!({
            "accessor_index":index,
            "accessor":Value::Object(accessor.clone()),
            "buffer_view":Value::Object(view.clone()),
            "packed_bytes_sha256":sha256_hex(&packed)
        }));
    }
    let mut projection = json!({
        "schema_version":"CandidateMaterialSurfaceGeometryProjection@1",
        "scene":root_object.get("scene"),
        "scenes":root_object.get("scenes"),
        "nodes":root_object.get("nodes"),
        "meshes":mesh_projection,
        "accessors":accessor_projection,
        "canonical_sha256":""
    });
    projection["canonical_sha256"] = Value::String(canonical_json_hash(&projection));
    Ok(projection)
}

fn validate_record_bindings(
    runtime: &Runtime,
    record: &CandidateMaterialSurfaceQualityRecord,
) -> Result<(), RuntimeError> {
    let source = runtime
        .candidate(&record.source_candidate_id)?
        .ok_or_else(|| invalid("source candidate is unavailable"))?;
    let output = runtime
        .candidate(&record.output_candidate_id)?
        .ok_or_else(|| invalid("output candidate is unavailable"))?;
    if source.project_id != record.project_id
        || output.project_id != record.project_id
        || source.canonical_sha256 != record.source_candidate_state_sha256
        || output.canonical_sha256 != record.output_candidate_state_sha256
        || source.prepared_object_id.as_deref() != Some(record.source_artifact_id.as_str())
        || source.prepared_object_sha256.as_deref() != Some(record.source_artifact_sha256.as_str())
        || output.prepared_object_id.as_deref() != Some(record.output_artifact_id.as_str())
        || output.prepared_object_sha256.as_deref() != Some(record.output_artifact_sha256.as_str())
        || record.source_candidate_id == record.output_candidate_id
    {
        return Err(invalid("current source/output candidate binding differs"));
    }
    let topology = runtime
        .store
        .get_candidate_topology_quality(&record.source_topology_quality_id)?
        .ok_or_else(|| invalid("source topology quality is unavailable"))?;
    if topology.project_id != record.project_id
        || topology.candidate_id != record.source_candidate_id
        || topology.candidate_state_sha256 != record.source_candidate_state_sha256
        || topology.artifact_id != record.source_artifact_id
        || topology.artifact_sha256 != record.source_artifact_sha256
        || topology.artifact_readback_sha256 != record.source_artifact_readback_sha256
        || topology.artifact_readback_object_sha256 != record.source_artifact_readback_object_sha256
        || topology.geometry_candidate_evidence_sha256
            != record.source_geometry_candidate_evidence_sha256
        || topology.geometry_program_sha256 != record.source_geometry_program_sha256
        || topology.canonical_sha256 != record.source_topology_quality_canonical_sha256
        || !topology.hard_gate_passed
        || topology.validator_status != "passed"
    {
        return Err(invalid("source CandidateTopologyQuality binding differs"));
    }
    if record.source_geometry_program_sha256 != record.output_geometry_program_sha256 {
        return Err(invalid(
            "Appearance output must preserve the exact source GeometryProgram",
        ));
    }
    let source_evidence = runtime
        .store
        .get_geometry_candidate_evidence(&record.source_candidate_id)?
        .ok_or_else(|| invalid("source GeometryCandidateEvidence is unavailable"))?;
    if source_evidence.canonical_sha256 != record.source_geometry_candidate_evidence_sha256
        || source_evidence.artifact_object_sha256 != record.source_artifact_sha256
        || source_evidence.artifact_readback_object_sha256
            != record.source_artifact_readback_object_sha256
        || source_evidence.geometry_program_sha256 != record.source_geometry_program_sha256
    {
        return Err(invalid("source GeometryCandidateEvidence binding differs"));
    }
    let lineage = runtime
        .store
        .get_appearance_source_lineage_link(
            &record.output_candidate_id,
            &record.appearance_program_sha256,
        )?
        .ok_or_else(|| invalid("AppearanceSourceLineage is unavailable"))?;
    super::appearance_source_lineage::validate_link(runtime, &lineage)?;
    if lineage.project_id != record.project_id
        || lineage.candidate_id != record.output_candidate_id
        || lineage.candidate_state_sha256 != record.output_candidate_state_sha256
        || lineage.sidecar_object_sha256 != record.appearance_source_lineage_sidecar_object_sha256
        || lineage.canonical_sha256 != record.appearance_source_lineage_canonical_sha256
        || lineage.appearance_program_object_sha256 != record.appearance_program_object_sha256
        || lineage.appearance_program_sha256 != record.appearance_program_sha256
        || lineage.material_layer_stack_sha256.as_deref()
            != Some(record.material_layer_stack_sha256.as_str())
        || lineage.material_pack_id != MATERIAL_PACK_ID
        || lineage.material_pack_version != MATERIAL_PACK_VERSION
        || lineage.material_pack_license_spdx != MATERIAL_PACK_LICENSE
        || lineage.material_pack_manifest_object_sha256
            != record.material_pack_manifest_object_sha256
        || lineage.material_pack_manifest_sha256 != record.material_pack_manifest_sha256
        || lineage.material_pack_provenance_sha256 != record.material_pack_provenance_sha256
        || lineage.texture_build_receipt_object_sha256 != record.texture_build_receipt_object_sha256
        || lineage.texture_build_receipt_sha256 != record.texture_build_receipt_canonical_sha256
        || lineage
            .candidate_surface_bake_receipt_object_sha256
            .as_deref()
            != Some(record.candidate_surface_bake_receipt_object_sha256.as_str())
        || lineage.candidate_surface_bake_receipt_sha256.as_deref()
            != Some(
                record
                    .candidate_surface_bake_receipt_canonical_sha256
                    .as_str(),
            )
        || lineage.uv_binding_sha256 != record.uv_binding_sha256
        || lineage.lod_candidate_ids.first() != Some(&record.output_candidate_id)
        || lineage.lod_artifact_sha256s.first() != Some(&record.output_artifact_sha256)
        || lineage.lod_artifact_readback_sha256s.first()
            != Some(&record.output_artifact_readback_sha256)
        || lineage.lod_artifact_readback_object_sha256s.first()
            != Some(&record.output_artifact_readback_object_sha256)
        || lineage.geometry_program_sha256 != record.output_geometry_program_sha256
        || lineage.appearance_program_schema_version != "AppearanceProgram@3"
    {
        return Err(invalid("AppearanceSourceLineage or 2K PBR binding differs"));
    }
    let texture = read_json(runtime, &record.texture_build_receipt_object_sha256)?;
    if texture.get("schema_version").and_then(Value::as_str) != Some("TextureBuildReceipt@2")
        || texture.get("canonical_sha256").and_then(Value::as_str)
            != Some(record.texture_build_receipt_canonical_sha256.as_str())
    {
        return Err(invalid("TextureBuild@2 receipt binding differs"));
    }
    let surface = read_json(
        runtime,
        &record.candidate_surface_bake_receipt_object_sha256,
    )?;
    super::validate_candidate_surface_bake_receipt_output(&surface)?;
    if surface.get("schema_version").and_then(Value::as_str)
        != Some("CandidateSurfaceBakeReceipt@1")
        || surface.get("canonical_sha256").and_then(Value::as_str)
            != Some(
                record
                    .candidate_surface_bake_receipt_canonical_sha256
                    .as_str(),
            )
    {
        return Err(invalid("CandidateSurfaceBake@1 receipt binding differs"));
    }
    let source_readback = readback_binding(
        runtime,
        &record.source_artifact_readback_object_sha256,
        &record.source_candidate_id,
        &record.source_artifact_sha256,
        &record.source_artifact_sha256,
        &record.source_artifact_readback_sha256,
        &record.source_geometry_program_sha256,
    )?;
    let output_readback = readback_binding(
        runtime,
        &record.output_artifact_readback_object_sha256,
        &record.output_candidate_id,
        &record.output_artifact_sha256,
        &record.output_artifact_sha256,
        &record.output_artifact_readback_sha256,
        &record.output_geometry_program_sha256,
    )?;
    let expected_material_zone_inventory = canonical_json_hash(&json!({
        "schema_version":"CandidateMaterialZoneInventoryBinding@1",
        "candidate_id":record.output_candidate_id,
        "artifact_readback_sha256":record.output_artifact_readback_sha256,
        "appearance_program_sha256":record.appearance_program_sha256,
        "material_zone_ids":output_readback.get("material_zone_ids"),
        "part_bindings":output_readback.get("part_bindings")
    }));
    if record.material_zone_inventory_sha256 != expected_material_zone_inventory {
        return Err(invalid(format!(
            "material_zone_inventory_sha256 differs; expected {expected_material_zone_inventory}"
        )));
    }
    let expected_material_provenance = canonical_json_hash(&json!({
        "schema_version":"CandidateMaterialProvenanceBinding@1",
        "appearance_program_sha256":record.appearance_program_sha256,
        "material_layer_stack_sha256":record.material_layer_stack_sha256,
        "material_pack_manifest_sha256":record.material_pack_manifest_sha256,
        "material_pack_provenance_sha256":record.material_pack_provenance_sha256,
        "texture_build_receipt_canonical_sha256":record.texture_build_receipt_canonical_sha256,
        "candidate_surface_bake_receipt_canonical_sha256":record.candidate_surface_bake_receipt_canonical_sha256,
        "uv_binding_sha256":record.uv_binding_sha256,
        "material_zone_inventory_sha256":record.material_zone_inventory_sha256
    }));
    if record.material_provenance_sha256 != expected_material_provenance {
        return Err(invalid(format!(
            "material_provenance_sha256 differs; expected {expected_material_provenance}"
        )));
    }
    for field in [
        "part_ids",
        "source_node_ids",
        "part_bindings",
        "triangle_count",
        "connected_component_count",
    ] {
        if source_readback.get(field) != output_readback.get(field) {
            return Err(invalid(format!(
                "source/output ArtifactReadback geometry field {field} differs"
            )));
        }
    }
    let source_bytes =
        runtime.cas_read_bounded(&record.source_artifact_sha256, MAX_GEOMETRY_ARTIFACT_BYTES)?;
    let output_bytes =
        runtime.cas_read_bounded(&record.output_artifact_sha256, MAX_GEOMETRY_ARTIFACT_BYTES)?;
    let source_integrity = strict_glb_inspection(&source_bytes)?;
    let output_integrity = strict_glb_inspection(&output_bytes)?;
    if !source_integrity.hard_gate_passed
        || !output_integrity.hard_gate_passed
        || output_integrity.uv_non_finite_count != 0
        || output_integrity.zero_area_uv_triangle_count != 0
        || output_integrity.tangent_non_finite_count != 0
        || output_integrity.tangent_orthogonality_error_count != 0
        || output_integrity.tangent_handedness_error_count != 0
    {
        return Err(invalid(
            "source/output strict GLB, UV or tangent hard gate failed",
        ));
    }
    let expected_tangent_binding = canonical_json_hash(&json!({
        "schema_version":"CandidateTangentBinding@1",
        "artifact_readback_sha256":record.output_artifact_readback_sha256,
        "uv_binding_sha256":record.uv_binding_sha256,
        "generator":"mikktspace@0.3.0",
        "tangent_non_finite_count":output_integrity.tangent_non_finite_count,
        "tangent_orthogonality_error_count":output_integrity.tangent_orthogonality_error_count,
        "tangent_handedness_error_count":output_integrity.tangent_handedness_error_count
    }));
    if record.tangent_binding_sha256 != expected_tangent_binding {
        return Err(invalid(format!(
            "tangent_binding_sha256 differs; expected {expected_tangent_binding}"
        )));
    }
    let (source_root, source_bin) = parse_glb(&source_bytes)?;
    let (output_root, output_bin) = parse_glb(&output_bytes)?;
    let source_projection = renderable_projection(&source_root, source_bin)?;
    let output_projection = renderable_projection(&output_root, output_bin)?;
    if source_projection != output_projection
        || source_projection
            .get("canonical_sha256")
            .and_then(Value::as_str)
            != Some(record.geometry_preservation_projection_sha256.as_str())
    {
        let expected = source_projection
            .get("canonical_sha256")
            .and_then(Value::as_str)
            .unwrap_or("unavailable");
        return Err(invalid(format!(
            "renderable geometry is not byte-exact or projection differs; expected {expected}"
        )));
    }
    Ok(())
}

fn record_from_request(
    runtime: &Runtime,
    request: CandidateMaterialSurfaceQualityPrepareRequest,
    request_sha256: String,
) -> Result<CandidateMaterialSurfaceQualityRecord, RuntimeError> {
    let created_at = runtime
        .candidate(&request.output_candidate_id)?
        .ok_or_else(|| invalid("output candidate is unavailable"))?
        .updated_at;
    let mut record = CandidateMaterialSurfaceQualityRecord {
        schema_version: "CandidateMaterialSurfaceQuality@1".to_owned(),
        material_surface_quality_id: request.material_surface_quality_id,
        project_id: request.project_id,
        source_candidate_id: request.source_candidate_id,
        source_candidate_state_sha256: request.source_candidate_state_sha256,
        source_artifact_id: request.source_artifact_id,
        source_artifact_sha256: request.source_artifact_sha256,
        source_artifact_readback_sha256: request.source_artifact_readback_sha256,
        source_artifact_readback_object_sha256: request.source_artifact_readback_object_sha256,
        source_geometry_candidate_evidence_sha256: request
            .source_geometry_candidate_evidence_sha256,
        source_geometry_program_sha256: request.source_geometry_program_sha256,
        source_topology_quality_id: request.source_topology_quality_id,
        source_topology_quality_report_object_sha256: request
            .source_topology_quality_report_object_sha256,
        source_topology_quality_canonical_sha256: request.source_topology_quality_canonical_sha256,
        output_candidate_id: request.output_candidate_id,
        output_candidate_state_sha256: request.output_candidate_state_sha256,
        output_artifact_id: request.output_artifact_id,
        output_artifact_sha256: request.output_artifact_sha256,
        output_artifact_readback_sha256: request.output_artifact_readback_sha256,
        output_artifact_readback_object_sha256: request.output_artifact_readback_object_sha256,
        output_geometry_program_sha256: request.output_geometry_program_sha256,
        appearance_source_lineage_sidecar_object_sha256: request
            .appearance_source_lineage_sidecar_object_sha256,
        appearance_source_lineage_canonical_sha256: request
            .appearance_source_lineage_canonical_sha256,
        appearance_program_object_sha256: request.appearance_program_object_sha256,
        appearance_program_sha256: request.appearance_program_sha256,
        material_layer_stack_sha256: request.material_layer_stack_sha256,
        material_pack_id: MATERIAL_PACK_ID.to_owned(),
        material_pack_version: MATERIAL_PACK_VERSION.to_owned(),
        material_pack_license_spdx: MATERIAL_PACK_LICENSE.to_owned(),
        material_pack_manifest_object_sha256: request.material_pack_manifest_object_sha256,
        material_pack_manifest_sha256: request.material_pack_manifest_sha256,
        material_pack_provenance_sha256: request.material_pack_provenance_sha256,
        texture_build_receipt_object_sha256: request.texture_build_receipt_object_sha256,
        texture_build_receipt_canonical_sha256: request.texture_build_receipt_canonical_sha256,
        candidate_surface_bake_receipt_object_sha256: request
            .candidate_surface_bake_receipt_object_sha256,
        candidate_surface_bake_receipt_canonical_sha256: request
            .candidate_surface_bake_receipt_canonical_sha256,
        uv_binding_sha256: request.uv_binding_sha256,
        tangent_binding_sha256: request.tangent_binding_sha256,
        material_zone_inventory_sha256: request.material_zone_inventory_sha256,
        material_provenance_sha256: request.material_provenance_sha256,
        lod_scope: request.lod_scope,
        source_output_candidate_binding_status: "distinct-candidates-verified".to_owned(),
        geometry_preservation_projection_sha256: request.geometry_preservation_projection_sha256,
        geometry_preservation_status: "source-output-renderable-geometry-byte-exact".to_owned(),
        material_surface_quality_policy: request.material_surface_quality_policy,
        material_surface_quality_policy_sha256: request.material_surface_quality_policy_sha256,
        from_stage: request.from_stage,
        to_stage: request.to_stage,
        hard_gate: CandidateMaterialSurfaceQualityHardGate {
            distinct_candidates: true,
            source_topology_quality: true,
            source_artifact_readback: true,
            output_artifact_readback: true,
            geometry_preserved: true,
            appearance_source_lineage: true,
            material_pack_2k: true,
            texture_build_v2: true,
            surface_bake_v1: true,
            uv_integrity: true,
            tangent_integrity: true,
            material_provenance: true,
        },
        validator_status: "passed".to_owned(),
        hard_gate_passed: true,
        visual_quality_status: "NOT_PROVEN".to_owned(),
        artistic_quality_status: "NOT_PROVEN".to_owned(),
        human_review_status: "NOT_RUN".to_owned(),
        commercial_fps_quality_status: "NOT_PROVEN".to_owned(),
        commercial_engine_status: "NOT_RUN".to_owned(),
        materialization_status: MATERIALIZATION_STATUS.to_owned(),
        quality_status: "structural_only".to_owned(),
        runtime_write_performed: true,
        production_stage_advanced: false,
        candidate_confirmed: false,
        version_created: false,
        export_performed: false,
        request_sha256,
        input_sha256: request.input_sha256,
        canonical_sha256: String::new(),
        created_at,
    };
    validate_record_bindings(runtime, &record)?;
    let mut canonical = record.clone();
    canonical.canonical_sha256.clear();
    record.canonical_sha256 = canonical_json_hash(
        &serde_json::to_value(&canonical)
            .map_err(|error| invalid(format!("record cannot be canonicalized: {error}")))?,
    );
    Ok(record)
}

fn result_value(
    record: &CandidateMaterialSurfaceQualityRecord,
    replayed: bool,
    schema_version: &str,
    runtime_write: bool,
) -> Result<Value, RuntimeError> {
    let value = serde_json::to_value(record)
        .map_err(|error| invalid(format!("record cannot be serialized: {error}")))?;
    Ok(json!({
        "schema_version":schema_version,
        "material_surface_quality":value,
        "replayed":replayed,
        "runtime_write":runtime_write,
        "production_stage_advanced":false,
        "candidate_confirmed":false,
        "version_created":false,
        "export_performed":false
    }))
}

fn clean_reservation(
    runtime: &Runtime,
    reservation: &forgecad_store::CasReservation,
    objects: &[CasObject],
    cleanup_new: bool,
) {
    for object in objects {
        let _ = runtime.store.release_cas_reservation_object(
            reservation,
            object,
            cleanup_new && object.created_new,
        );
    }
}

impl Runtime {
    pub fn candidate_material_surface_quality_prepare(
        &self,
        request: Value,
    ) -> Result<Value, RuntimeError> {
        let (request, request_sha256) = prepare_request(&request)?;
        let record = record_from_request(self, request, request_sha256)?;
        let record_value = serde_json::to_value(&record)
            .map_err(|error| invalid(format!("report cannot be serialized: {error}")))?;
        let bytes = canonical_json_bytes(&record_value)
            .map_err(|error| invalid(format!("report JSON is invalid: {error}")))?;
        if bytes.len() > MAX_REPORT_BYTES as usize {
            return Err(invalid("material-surface report exceeds 1 MiB"));
        }
        let reservation = self.store.begin_cas_reservation();
        let report = match self.store.put_object_reserved(
            &reservation,
            &bytes,
            None,
            JSON_MIME,
            REPORT_KIND,
            &record.created_at,
        ) {
            Ok(object) => object,
            Err(error) => return Err(error.into()),
        };
        match self
            .store
            .record_candidate_material_surface_quality_with_replay(&record, &report.record)
        {
            Ok((stored, replayed)) => {
                clean_reservation(self, &reservation, std::slice::from_ref(&report), false);
                result_value(&stored, replayed, PREPARE_RESULT_SCHEMA, true)
            }
            Err(error) => {
                clean_reservation(self, &reservation, std::slice::from_ref(&report), true);
                Err(error.into())
            }
        }
    }

    pub fn candidate_material_surface_quality_get(
        &self,
        request: Value,
    ) -> Result<Value, RuntimeError> {
        let request = get_request(&request)?;
        let record = self
            .store
            .get_candidate_material_surface_quality(&request.material_surface_quality_id)?
            .ok_or_else(|| invalid("material-surface quality is unavailable"))?;
        if record.project_id != request.project_id
            || record.source_candidate_id != request.source_candidate_id
            || record.output_candidate_id != request.output_candidate_id
        {
            return Err(invalid("material-surface quality scope differs"));
        }
        validate_record_bindings(self, &record)?;
        result_value(&record, true, GET_RESULT_SCHEMA, false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn policy_hash_is_frozen() {
        assert_eq!(sha256_hex(POLICY.as_bytes()).len(), 64);
        assert_eq!(MATERIAL_PACK_ID, "forgecad-fictional-energy-weapon-2k");
    }

    #[test]
    fn identical_get_candidates_are_rejected() {
        let value = json!({
            "schema_version":GET_SCHEMA,
            "material_surface_quality_id":"material-quality-1",
            "project_id":"project-1",
            "source_candidate_id":"candidate-1",
            "output_candidate_id":"candidate-1"
        });
        assert!(get_request(&value)
            .unwrap_err()
            .to_string()
            .contains("get scope is malformed"));
    }

    #[test]
    fn renderable_projection_ignores_material_uv_and_tangent_but_detects_geometry_changes() {
        let root = json!({
            "scene":0,
            "scenes":[{"nodes":[0]}],
            "nodes":[{"name":"part-1","mesh":0}],
            "meshes":[{"primitives":[{
                "attributes":{"POSITION":0,"NORMAL":1,"TANGENT":2,"TEXCOORD_0":3},
                "indices":4,
                "material":0
            }]}],
            "bufferViews":[
                {"buffer":0,"byteOffset":0,"byteLength":36},
                {"buffer":0,"byteOffset":36,"byteLength":36},
                {"buffer":0,"byteOffset":72,"byteLength":48},
                {"buffer":0,"byteOffset":120,"byteLength":24},
                {"buffer":0,"byteOffset":144,"byteLength":12}
            ],
            "accessors":[
                {"bufferView":0,"componentType":5126,"count":3,"type":"VEC3"},
                {"bufferView":1,"componentType":5126,"count":3,"type":"VEC3"},
                {"bufferView":2,"componentType":5126,"count":3,"type":"VEC4"},
                {"bufferView":3,"componentType":5126,"count":3,"type":"VEC2"},
                {"bufferView":4,"componentType":5125,"count":3,"type":"SCALAR"}
            ]
        });
        let binary = vec![0u8; 156];
        let baseline = renderable_projection(&root, &binary).expect("baseline projection");
        let mut material_only = root.clone();
        material_only["meshes"][0]["primitives"][0]["material"] = json!(7);
        assert_eq!(
            baseline,
            renderable_projection(&material_only, &binary).expect("material projection")
        );
        let mut surface_only = root.clone();
        surface_only["meshes"][0]["primitives"][0]["attributes"]["TANGENT"] = json!(3);
        surface_only["meshes"][0]["primitives"][0]["attributes"]["TEXCOORD_0"] = json!(2);
        assert_eq!(
            baseline,
            renderable_projection(&surface_only, &binary).expect("surface projection")
        );
        let mut changed_node = root.clone();
        changed_node["nodes"][0]["mesh"] = json!(1);
        assert_ne!(
            baseline,
            renderable_projection(&changed_node, &binary).expect("node projection")
        );
        let mut changed_binary = binary;
        changed_binary[0] = 1;
        assert_ne!(
            baseline,
            renderable_projection(&root, &changed_binary).expect("changed projection")
        );
    }

    #[test]
    fn renderable_projection_rejects_accessor_end_overflow() {
        let root = json!({
            "scene":0,
            "scenes":[{"nodes":[0]}],
            "nodes":[{"mesh":0}],
            "meshes":[{"primitives":[{
                "attributes":{"POSITION":0,"NORMAL":0,"TANGENT":0,"TEXCOORD_0":0},
                "indices":0
            }]}],
            "bufferViews":[{"buffer":0,"byteOffset":u64::MAX,"byteLength":12}],
            "accessors":[{"bufferView":0,"componentType":5126,"count":1,"type":"VEC3"}]
        });
        assert!(renderable_projection(&root, &[])
            .unwrap_err()
            .to_string()
            .contains("range overflows"));
    }
}
