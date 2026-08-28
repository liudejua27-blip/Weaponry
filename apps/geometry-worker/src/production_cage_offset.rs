//! Deterministic topology-correspondent Cage offset source kernel.
//!
//! The Cage keeps the admitted Low primitive, vertex, index and face order
//! exactly. Only positions are displaced along deterministic welded vertex
//! normals. This is structural source evidence, not a bake or stage gate.

use crate::integrity::{self, DiagnosticMesh, DiagnosticPrimitive};
use crate::GeometryError;
use base64::Engine;
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

const AREA_EPSILON: f64 = 1.0e-12;
const VOLUME_EPSILON: f64 = 1.0e-12;
const NORMAL_EPSILON: f64 = 1.0e-18;
const MAX_SOURCE_GLB_BYTES: usize = 64 * 1024 * 1024;
const MAX_SOURCE_GLB_BASE64_BYTES: usize = MAX_SOURCE_GLB_BYTES * 4 / 3 + 4;
pub const REQUEST_SCHEMA_VERSION: &str = "CageOffsetWorkerRequest@1";
pub const RESULT_SCHEMA_VERSION: &str = "CageOffsetWorkerResult@1";
pub const POLICY: &str = "exact-low-topology-per-vertex-normal-offset@1";
pub const ALGORITHM: &str = "deterministic-welded-area-normal-offset@1";
const REQUEST_FIELDS: &[&str] = &[
    "schema_version",
    "preview_only",
    "source_low_artifact_sha256",
    "low_glb_base64",
    "offset_m",
    "max_offset_m",
    "max_coordinate_abs_m",
    "offset_field_policy",
    "algorithm",
    "canonical_sha256",
];

#[derive(Debug, Clone, PartialEq)]
pub struct CageOffsetPolicy {
    pub offset_m: f32,
    pub max_offset_m: f32,
    pub max_coordinate_abs_m: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CageOffsetFieldEntry {
    pub primitive_ordinal: u32,
    pub vertex_index: u32,
    pub part_id: String,
    pub source_position: [f32; 3],
    pub normal: [f32; 3],
    pub offset_m: f32,
    pub derived_position: [f32; 3],
}

#[derive(Debug, Clone, PartialEq)]
pub struct DerivedCagePrimitive {
    pub part_id: String,
    pub source_node_id: String,
    pub material_zone_id: String,
    pub solid: bool,
    pub positions: Vec<[f32; 3]>,
    pub indices: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DerivedCageMesh {
    pub primitives: Vec<DerivedCagePrimitive>,
    pub offset_field: Vec<CageOffsetFieldEntry>,
    pub source_triangle_count: usize,
    pub cage_triangle_count: usize,
    pub exact_topology_correspondence: bool,
    pub offset_field_derived: bool,
    pub containment_status: String,
    pub quality_status: String,
    pub production_stage_advanced: bool,
    pub promotion_eligible: bool,
}

/// Closed Worker adapter for the exact-topology offset kernel. It emits the
/// derived mesh projection and per-vertex field but no GLB, bake map, CAS write
/// or production-stage transition.
pub fn run(payload: &Map<String, Value>) -> Result<Value, GeometryError> {
    if payload
        .keys()
        .any(|field| !REQUEST_FIELDS.contains(&field.as_str()))
        || payload.len() != REQUEST_FIELDS.len()
        || payload.get("schema_version").and_then(Value::as_str) != Some(REQUEST_SCHEMA_VERSION)
        || payload.get("preview_only").and_then(Value::as_bool) != Some(true)
        || payload.get("offset_field_policy").and_then(Value::as_str) != Some(POLICY)
        || payload.get("algorithm").and_then(Value::as_str) != Some(ALGORITHM)
    {
        return Err(invalid("CAGE_OFFSET_REQUEST_INVALID"));
    }
    let canonical = required_sha(payload, "canonical_sha256")?;
    let mut preimage = payload.clone();
    preimage.remove("canonical_sha256");
    if crate::canonical_hash(&Value::Object(preimage)) != canonical {
        return Err(invalid("CAGE_OFFSET_REQUEST_CANONICAL_MISMATCH"));
    }
    let source_hash = required_sha(payload, "source_low_artifact_sha256")?;
    let encoded = payload
        .get("low_glb_base64")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("CAGE_OFFSET_SOURCE_GLB_MISSING"))?;
    if encoded.len() > MAX_SOURCE_GLB_BASE64_BYTES {
        return Err(invalid("CAGE_OFFSET_SOURCE_GLB_TOO_LARGE"));
    }
    let glb = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|_| invalid("CAGE_OFFSET_SOURCE_GLB_INVALID"))?;
    if glb.is_empty() || glb.len() > MAX_SOURCE_GLB_BYTES || sha256_hex(&glb) != source_hash {
        return Err(invalid("CAGE_OFFSET_SOURCE_HASH_MISMATCH"));
    }
    let inspection = integrity::inspect_glb(&glb)?;
    if !inspection.hard_gate_passed {
        return Err(invalid("CAGE_OFFSET_SOURCE_READBACK_FAILED"));
    }
    let mesh = integrity::extract_diagnostic_mesh(&glb, 1_000_000)?;
    let derived = derive_topology_correspondent_cage(
        &mesh,
        &CageOffsetPolicy {
            offset_m: required_f32(payload, "offset_m")?,
            max_offset_m: required_f32(payload, "max_offset_m")?,
            max_coordinate_abs_m: required_f32(payload, "max_coordinate_abs_m")?,
        },
    )?;
    let cage_mesh = cage_mesh_value(&derived);
    let field = offset_field_value(&derived);
    let cage_mesh_sha256 = crate::canonical_hash(&cage_mesh);
    let offset_field_sha256 = crate::canonical_hash(&field);
    let cage_artifact = lower_cage_glb(&derived, &cage_mesh_sha256)?;
    let cage_artifact_sha256 = sha256_hex(&cage_artifact.glb);
    let cage_readback = integrity::inspect_glb(&cage_artifact.glb)?;
    if !cage_readback.hard_gate_passed
        || cage_readback.triangle_count as usize != derived.cage_triangle_count
    {
        return Err(invalid(format!(
            "CAGE_OFFSET_DERIVED_READBACK_FAILED: failures={:?} expected_triangles={} readback_triangles={} boundary={} non_manifold={} winding={} tangent_handedness={}",
            cage_readback.failure_codes,
            derived.cage_triangle_count,
            cage_readback.triangle_count,
            cage_readback.boundary_edge_count,
            cage_readback.non_manifold_edge_count,
            cage_readback.winding_error_count,
            cage_readback.tangent_handedness_error_count
        )));
    }
    let (offset_min_m, offset_max_m) = derived
        .offset_field
        .iter()
        .map(|entry| entry.offset_m)
        .fold((f32::INFINITY, f32::NEG_INFINITY), |(min, max), value| {
            (min.min(value), max.max(value))
        });
    let mut result = json!({
        "schema_version":RESULT_SCHEMA_VERSION,
        "operation":forgecad_worker_protocol::PRODUCTION_WEAPON_CAGE_OFFSET_OPERATION,
        "source_low_artifact_sha256":source_hash,
        "offset_field_policy":POLICY,
        "algorithm":ALGORITHM,
        "algorithm_sha256":sha256_hex(ALGORITHM.as_bytes()),
        "source_triangle_count":derived.source_triangle_count,
        "cage_triangle_count":derived.cage_triangle_count,
        "cage_mesh":cage_mesh,
        "cage_mesh_sha256":cage_mesh_sha256,
        "offset_field":field,
        "offset_field_sha256":offset_field_sha256,
        "cage_offset_min_m":offset_min_m,
        "cage_offset_max_m":offset_max_m,
        "cage_offset_space":"low-vertex-welded-outward-normal@1",
        "cage_artifact_sha256":cage_artifact_sha256,
        "cage_glb_base64":base64::engine::general_purpose::STANDARD.encode(&cage_artifact.glb),
        "cage_artifact_readback":cage_readback.report_value(),
        "cage_program_sha256":cage_artifact.program_sha256,
        "exact_topology_correspondence":true,
        "offset_field_derived":true,
        "cage_topology_status":"PASS_SOURCE_STRUCTURAL",
        "diagnostic":{
            "status":"NOT_RUN_NO_HIGH_REFERENCE",
            "self_intersection_count":Value::Null,
            "cross_part_count":Value::Null,
            "skew_count":Value::Null,
            "penetration_count":Value::Null,
            "out_of_range_count":0,
            "policy":"low-topology-offset-only-no-high-ray-diagnostic@1",
            "note":"Self-intersection, cross-Part, skew and penetration require an independently admitted High artifact; this operation intentionally consumes Low only."
        },
        "containment_status":"STRUCTURAL_OFFSET_ONLY",
        "quality_status":"structural_only",
        "runtime_write_performed":false,
        "production_stage_advanced":false,
        "promotion_eligible":false,
        "candidate_confirmed":false,
        "version_created":false,
        "export_performed":false,
        "canonical_sha256":""
    });
    result["canonical_sha256"] = Value::String(wire_canonical_hash(&result)?);
    if serde_json::to_vec(&result)
        .map_err(|_| invalid("CAGE_OFFSET_RESULT_SERIALIZE_FAILED"))?
        .len()
        > forgecad_worker_protocol::MAX_WORKER_RESPONSE_BYTES
    {
        return Err(invalid("CAGE_OFFSET_RESULT_TOO_LARGE"));
    }
    Ok(result)
}

/// Canonicalize the result after the same JSON wire round-trip used by the
/// isolated Worker envelope.  Some f32-origin numbers have a lexical form in
/// the in-memory `serde_json::Number` that is normalized when the parent
/// Runtime parses the response; hashing the wire projection keeps restart
/// verification deterministic across that process boundary.
fn wire_canonical_hash(value: &Value) -> Result<String, GeometryError> {
    let bytes = serde_json::to_vec(value)
        .map_err(|_| invalid("CAGE_OFFSET_RESULT_CANONICAL_SERIALIZE_FAILED"))?;
    let mut wire: Value = serde_json::from_slice(&bytes)
        .map_err(|_| invalid("CAGE_OFFSET_RESULT_CANONICAL_PARSE_FAILED"))?;
    wire["canonical_sha256"] = Value::String(String::new());
    Ok(crate::canonical_hash(&wire))
}

fn lower_cage_glb(
    mesh: &DerivedCageMesh,
    program_sha256: &str,
) -> Result<crate::GeometryArtifact, GeometryError> {
    let mut parts = Vec::<crate::PartMesh>::new();
    let mut field_cursor = 0usize;
    for (primitive_ordinal, primitive) in mesh.primitives.iter().enumerate() {
        let end = field_cursor
            .checked_add(primitive.positions.len())
            .ok_or_else(|| invalid("CAGE_OFFSET_FIELD_COUNT_OVERFLOW"))?;
        let entries = mesh
            .offset_field
            .get(field_cursor..end)
            .ok_or_else(|| invalid("CAGE_OFFSET_FIELD_COUNT_MISMATCH"))?;
        if entries.iter().enumerate().any(|(vertex, entry)| {
            entry.primitive_ordinal as usize != primitive_ordinal
                || entry.vertex_index as usize != vertex
                || entry.derived_position != primitive.positions[vertex]
        }) {
            return Err(invalid("CAGE_OFFSET_FIELD_ORDER_MISMATCH"));
        }
        field_cursor = end;
        let normals = entries.iter().map(|entry| entry.normal).collect::<Vec<_>>();
        let (positions, normals, uvs, tangents, indices, uv_chart_count, uv_chart_ids) =
            crate::triangulate_uv_charts(
                &primitive.positions,
                &normals,
                &primitive.indices,
                true,
                false,
            )?;
        if positions != primitive.positions || indices != primitive.indices {
            return Err(invalid("CAGE_OFFSET_LOWERING_CHANGED_TOPOLOGY_ORDER"));
        }
        let source = crate::PartSourceMesh {
            source_node_id: primitive.source_node_id.clone(),
            operator_id: "forgecad.worker.cage-offset@1".to_owned(),
            lineage_source_node_ids: vec![primitive.source_node_id.clone()],
            positions,
            normals,
            uvs,
            tangents,
            indices,
            uv_chart_count,
            uv_chart_ids,
        };
        if let Some(part) = parts
            .iter_mut()
            .find(|part| part.part_id == primitive.part_id)
        {
            if part.material_zone_id != primitive.material_zone_id || part.solid != primitive.solid
            {
                return Err(invalid("CAGE_OFFSET_PART_BINDING_MISMATCH"));
            }
            part.sources.push(source);
        } else {
            parts.push(crate::PartMesh {
                part_id: primitive.part_id.clone(),
                material_zone_id: primitive.material_zone_id.clone(),
                solid: primitive.solid,
                sources: vec![source],
                material: crate::material_for_zone(&primitive.material_zone_id),
            });
        }
    }
    if field_cursor != mesh.offset_field.len() {
        return Err(invalid("CAGE_OFFSET_FIELD_COUNT_MISMATCH"));
    }
    let triangle_count = parts
        .iter()
        .flat_map(|part| &part.sources)
        .map(|source| source.indices.len() as u64 / 3)
        .sum::<u64>();
    if triangle_count as usize != mesh.cage_triangle_count {
        return Err(invalid("CAGE_OFFSET_LOWERING_COUNT_MISMATCH"));
    }
    let glb = crate::write_glb(
        &parts,
        program_sha256,
        triangle_count,
        "ArtifactReadback@2",
        Some(&crate::operator_catalog_sha256()),
        None,
    )?;
    Ok(crate::GeometryArtifact {
        glb,
        part_ids: crate::ordered_unique_part_ids(&parts),
        triangle_count,
        program_sha256: program_sha256.to_owned(),
        uv_status: "passed".to_owned(),
        tangent_status: "passed".to_owned(),
        material_zone_ids: crate::ordered_unique_material_zone_ids(&parts),
    })
}

fn cage_mesh_value(mesh: &DerivedCageMesh) -> Value {
    Value::Array(
        mesh.primitives
            .iter()
            .map(|primitive| {
                json!({
                    "part_id":primitive.part_id,
                    "source_node_id":primitive.source_node_id,
                    "material_zone_id":primitive.material_zone_id,
                    "solid":primitive.solid,
                    "positions":primitive.positions,
                    "indices":primitive.indices
                })
            })
            .collect(),
    )
}

fn offset_field_value(mesh: &DerivedCageMesh) -> Value {
    Value::Array(
        mesh.offset_field
            .iter()
            .map(|entry| {
                json!({
                    "primitive_ordinal":entry.primitive_ordinal,
                    "vertex_index":entry.vertex_index,
                    "part_id":entry.part_id,
                    "source_position":entry.source_position,
                    "normal":entry.normal,
                    "offset_m":entry.offset_m,
                    "derived_position":entry.derived_position
                })
            })
            .collect(),
    )
}

fn required_sha(payload: &Map<String, Value>, field: &str) -> Result<String, GeometryError> {
    let value = payload
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("CAGE_OFFSET_HASH_INVALID"))?;
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(invalid("CAGE_OFFSET_HASH_INVALID"));
    }
    Ok(value.to_owned())
}

fn required_f32(payload: &Map<String, Value>, field: &str) -> Result<f32, GeometryError> {
    payload
        .get(field)
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite())
        .map(|value| value as f32)
        .filter(|value| value.is_finite())
        .ok_or_else(|| invalid("CAGE_OFFSET_NUMBER_INVALID"))
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub fn derive_topology_correspondent_cage(
    low: &DiagnosticMesh,
    policy: &CageOffsetPolicy,
) -> Result<DerivedCageMesh, GeometryError> {
    if low.primitives.is_empty()
        || low.triangle_count == 0
        || !policy.offset_m.is_finite()
        || !policy.max_offset_m.is_finite()
        || !policy.max_coordinate_abs_m.is_finite()
        || policy.offset_m <= 0.0
        || policy.max_offset_m <= 0.0
        || policy.offset_m > policy.max_offset_m
        || policy.max_coordinate_abs_m <= 0.0
    {
        return Err(invalid("CAGE_OFFSET_POLICY_INVALID"));
    }

    let mut primitives = Vec::with_capacity(low.primitives.len());
    let mut offset_field = Vec::new();
    for (primitive_ordinal, primitive) in low.primitives.iter().enumerate() {
        let normals = welded_outward_normals(primitive)?;
        let mut positions = Vec::with_capacity(primitive.positions.len());
        for (vertex_index, (position, normal)) in primitive
            .positions
            .iter()
            .copied()
            .zip(normals.into_iter())
            .enumerate()
        {
            if position
                .iter()
                .any(|value| !value.is_finite() || value.abs() > policy.max_coordinate_abs_m)
            {
                return Err(invalid("CAGE_OFFSET_SOURCE_COORDINATE_OUT_OF_RANGE"));
            }
            let derived = [
                position[0] + normal[0] * policy.offset_m,
                position[1] + normal[1] * policy.offset_m,
                position[2] + normal[2] * policy.offset_m,
            ];
            if derived
                .iter()
                .any(|value| !value.is_finite() || value.abs() > policy.max_coordinate_abs_m)
            {
                return Err(invalid("CAGE_OFFSET_COORDINATE_OUT_OF_RANGE"));
            }
            let displacement = [
                derived[0] - position[0],
                derived[1] - position[1],
                derived[2] - position[2],
            ];
            let signed = dot3(displacement, normal);
            let tangent = [
                displacement[0] - normal[0] * signed,
                displacement[1] - normal[1] * signed,
                displacement[2] - normal[2] * signed,
            ];
            if signed <= 0.0
                || (signed - policy.offset_m).abs() > 1.0e-5
                || length3(tangent) > 1.0e-5
            {
                return Err(invalid("CAGE_OFFSET_DIRECTION_INVALID"));
            }
            positions.push(derived);
            offset_field.push(CageOffsetFieldEntry {
                primitive_ordinal: primitive_ordinal as u32,
                vertex_index: vertex_index as u32,
                part_id: primitive.part_id.clone(),
                source_position: position,
                normal,
                offset_m: policy.offset_m,
                derived_position: derived,
            });
        }
        let cage = DiagnosticPrimitive {
            part_id: primitive.part_id.clone(),
            source_node_id: primitive.source_node_id.clone(),
            material_zone_id: primitive.material_zone_id.clone(),
            solid: primitive.solid,
            positions: positions.clone(),
            indices: primitive.indices.clone(),
        };
        // Revalidate the displaced surface. A collapse, order change or
        // degenerate face cannot be hidden by the exact-topology claim.
        welded_outward_normals(&cage)?;
        primitives.push(DerivedCagePrimitive {
            part_id: cage.part_id,
            source_node_id: cage.source_node_id,
            material_zone_id: cage.material_zone_id,
            solid: cage.solid,
            positions,
            indices: cage.indices,
        });
    }
    let cage_triangle_count = primitives
        .iter()
        .map(|primitive| primitive.indices.len() / 3)
        .sum();
    if cage_triangle_count != low.triangle_count {
        return Err(invalid("CAGE_TOPOLOGY_CORRESPONDENCE_MISMATCH"));
    }
    Ok(DerivedCageMesh {
        primitives,
        offset_field,
        source_triangle_count: low.triangle_count,
        cage_triangle_count,
        exact_topology_correspondence: true,
        offset_field_derived: true,
        containment_status: "STRUCTURAL_OFFSET_ONLY".to_owned(),
        quality_status: "structural_only".to_owned(),
        production_stage_advanced: false,
        promotion_eligible: false,
    })
}

fn welded_outward_normals(primitive: &DiagnosticPrimitive) -> Result<Vec<[f32; 3]>, GeometryError> {
    if !primitive.solid || primitive.indices.is_empty() || primitive.indices.len() % 3 != 0 {
        return Err(invalid("CAGE_OFFSET_REQUIRES_CLOSED_SOLID_PARTS"));
    }
    let mut welded_by_position = BTreeMap::<[u32; 3], u32>::new();
    let mut welded_positions = Vec::<[f64; 3]>::new();
    let mut source_to_welded = vec![0u32; primitive.positions.len()];
    for (source_index, position) in primitive.positions.iter().copied().enumerate() {
        if position.iter().any(|value| !value.is_finite()) {
            return Err(invalid("CAGE_OFFSET_NON_FINITE_POSITION"));
        }
        let key = [bits(position[0]), bits(position[1]), bits(position[2])];
        let welded = if let Some(index) = welded_by_position.get(&key) {
            *index
        } else {
            let index = welded_positions.len() as u32;
            welded_by_position.insert(key, index);
            welded_positions.push(to64(position));
            index
        };
        source_to_welded[source_index] = welded;
    }

    let mut faces = Vec::new();
    let mut directed = BTreeMap::<(u32, u32), usize>::new();
    let mut undirected = BTreeMap::<(u32, u32), usize>::new();
    let mut signed_volume = 0.0f64;
    for triangle in primitive.indices.chunks_exact(3) {
        if triangle
            .iter()
            .any(|index| *index as usize >= source_to_welded.len())
        {
            return Err(invalid("CAGE_OFFSET_INDEX_OUT_OF_RANGE"));
        }
        let face = [
            source_to_welded[triangle[0] as usize],
            source_to_welded[triangle[1] as usize],
            source_to_welded[triangle[2] as usize],
        ];
        if face[0] == face[1] || face[1] == face[2] || face[2] == face[0] {
            return Err(invalid("CAGE_OFFSET_DEGENERATE_FACE"));
        }
        let a = welded_positions[face[0] as usize];
        let b = welded_positions[face[1] as usize];
        let c = welded_positions[face[2] as usize];
        let area_vector = cross64(sub64(b, a), sub64(c, a));
        if length64(area_vector) <= AREA_EPSILON {
            return Err(invalid("CAGE_OFFSET_DEGENERATE_FACE"));
        }
        signed_volume += dot64(a, cross64(b, c)) / 6.0;
        for [from, to] in [[face[0], face[1]], [face[1], face[2]], [face[2], face[0]]] {
            *directed.entry((from, to)).or_default() += 1;
            let edge = if from < to { (from, to) } else { (to, from) };
            *undirected.entry(edge).or_default() += 1;
        }
        faces.push((face, area_vector));
    }
    if undirected.values().any(|count| *count != 2)
        || directed
            .iter()
            .any(|(&(a, b), count)| *count != 1 || directed.get(&(b, a)) != Some(&1))
        || signed_volume.abs() <= VOLUME_EPSILON
    {
        return Err(invalid("CAGE_OFFSET_MANIFOLD_VALIDATION_FAILED"));
    }

    // Winding is made outward from the signed volume without changing Low
    // topology. The sign only selects the normal direction.
    let orientation = if signed_volume > 0.0 { 1.0 } else { -1.0 };
    let mut sums = vec![[0.0f64; 3]; welded_positions.len()];
    for (face, area_vector) in faces {
        let weighted = mul64(area_vector, orientation);
        for vertex in face {
            sums[vertex as usize] = add64(sums[vertex as usize], weighted);
        }
    }
    let welded_normals = sums
        .into_iter()
        .map(|sum| {
            let length = length64(sum);
            if !length.is_finite() || length <= NORMAL_EPSILON {
                return Err(invalid("CAGE_OFFSET_NORMAL_UNAVAILABLE"));
            }
            Ok([
                (sum[0] / length) as f32,
                (sum[1] / length) as f32,
                (sum[2] / length) as f32,
            ])
        })
        .collect::<Result<Vec<_>, GeometryError>>()?;
    Ok(source_to_welded
        .into_iter()
        .map(|index| welded_normals[index as usize])
        .collect())
}

fn bits(value: f32) -> u32 {
    if value == 0.0 {
        0
    } else {
        value.to_bits()
    }
}

fn to64(value: [f32; 3]) -> [f64; 3] {
    [value[0] as f64, value[1] as f64, value[2] as f64]
}

fn add64(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn sub64(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn mul64(a: [f64; 3], scalar: f64) -> [f64; 3] {
    [a[0] * scalar, a[1] * scalar, a[2] * scalar]
}

fn dot64(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross64(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn length64(value: [f64; 3]) -> f64 {
    dot64(value, value).sqrt()
}

fn dot3(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn length3(value: [f32; 3]) -> f32 {
    dot3(value, value).sqrt()
}

fn invalid(code: impl Into<String>) -> GeometryError {
    GeometryError::Invalid(code.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;
    use serde_json::{json, Value};

    fn tetrahedron() -> DiagnosticMesh {
        let positions = vec![
            [1.0, 1.0, 1.0],
            [-1.0, -1.0, 1.0],
            [-1.0, 1.0, -1.0],
            [1.0, -1.0, -1.0],
        ];
        // Consistent closed winding; signed-volume direction is normalized by
        // the kernel rather than mutating this index order.
        let indices = vec![0, 2, 1, 0, 1, 3, 0, 3, 2, 1, 2, 3];
        DiagnosticMesh {
            primitives: vec![DiagnosticPrimitive {
                part_id: "body".to_owned(),
                source_node_id: "body-node".to_owned(),
                material_zone_id: "zone-shell".to_owned(),
                solid: true,
                positions,
                indices,
            }],
            triangle_count: 4,
        }
    }

    fn policy() -> CageOffsetPolicy {
        CageOffsetPolicy {
            offset_m: 0.05,
            max_offset_m: 0.2,
            max_coordinate_abs_m: 10.0,
        }
    }

    fn box_artifact() -> crate::GeometryArtifact {
        let mut program = json!({
            "schema_version":"GeometryProgram@2",
            "project_id":"cage-offset-worker-fixture",
            "representation_plan_sha256":"a".repeat(64),
            "operator_catalog_sha256":crate::operator_catalog_sha256(),
            "units":{"length":"meter","angle":"radian","coordinate_system":"right-handed-y-up"},
            "budgets":{"max_nodes":2,"max_triangles":100,"max_glb_bytes":4194304,"max_worker_memory_bytes":536870912,"max_runtime_ms":10000},
            "nodes":[{"node_id":"body-node","operator_id":"forgecad.geometry.primitive@2","inputs":[],"parameters":{"shape":"box","size_m":[2.0,2.0,2.0],"position_m":[0.0,0.0,0.0],"rotation_rad":[0.0,0.0,0.0]}}],
            "part_outputs":[{"part_id":"body","input_node_ids":["body-node"],"material_zone_id":"zone-shell","solid":true}]
        });
        let preimage = program.as_object().unwrap().clone();
        program["canonical_sha256"] =
            Value::String(crate::canonical_hash(&Value::Object(preimage)));
        crate::compile_geometry_program(&program).unwrap()
    }

    #[test]
    fn cage_preserves_exact_topology_and_emits_vertex_field() {
        let low = tetrahedron();
        let cage = derive_topology_correspondent_cage(&low, &policy()).unwrap();
        assert_eq!(cage.cage_triangle_count, low.triangle_count);
        assert_eq!(cage.primitives[0].indices, low.primitives[0].indices);
        assert_eq!(
            cage.primitives[0].positions.len(),
            low.primitives[0].positions.len()
        );
        assert_eq!(cage.offset_field.len(), low.primitives[0].positions.len());
        assert!(cage.exact_topology_correspondence);
        assert!(cage.offset_field_derived);
        assert!(!cage.production_stage_advanced);
        assert!(!cage.promotion_eligible);
        for entry in &cage.offset_field {
            assert!((length3(entry.normal) - 1.0).abs() < 1.0e-5);
            assert_eq!(entry.offset_m, 0.05);
        }
    }

    #[test]
    fn replay_is_exact_and_invalid_offsets_fail_closed() {
        let low = tetrahedron();
        assert_eq!(
            derive_topology_correspondent_cage(&low, &policy()).unwrap(),
            derive_topology_correspondent_cage(&low, &policy()).unwrap()
        );
        for offset in [0.0, -0.1, f32::NAN, f32::INFINITY, 0.21] {
            let mut invalid = policy();
            invalid.offset_m = offset;
            assert!(derive_topology_correspondent_cage(&low, &invalid).is_err());
        }
    }

    #[test]
    fn open_or_non_solid_low_is_rejected() {
        let mut open = tetrahedron();
        open.primitives[0].indices.truncate(9);
        open.triangle_count = 3;
        assert!(derive_topology_correspondent_cage(&open, &policy()).is_err());
        let mut nonsolid = tetrahedron();
        nonsolid.primitives[0].solid = false;
        assert!(derive_topology_correspondent_cage(&nonsolid, &policy()).is_err());
    }

    #[test]
    fn closed_worker_adapter_is_hash_bound_exact_topology_and_non_promoting() {
        let artifact = box_artifact();
        let source_hash = sha256_hex(&artifact.glb);
        let mut request = json!({
            "schema_version":REQUEST_SCHEMA_VERSION,
            "preview_only":true,
            "source_low_artifact_sha256":source_hash,
            "low_glb_base64":base64::engine::general_purpose::STANDARD.encode(&artifact.glb),
            "offset_m":0.05,
            "max_offset_m":0.2,
            "max_coordinate_abs_m":10.0,
            "offset_field_policy":POLICY,
            "algorithm":ALGORITHM,
            "canonical_sha256":""
        });
        let mut preimage = request.as_object().unwrap().clone();
        preimage.remove("canonical_sha256");
        request["canonical_sha256"] =
            Value::String(crate::canonical_hash(&Value::Object(preimage)));
        let first = run(request.as_object().unwrap()).unwrap();
        let replay = run(request.as_object().unwrap()).unwrap();
        assert_eq!(first, replay);
        let wire: Value = serde_json::from_slice(&serde_json::to_vec(&first).unwrap()).unwrap();
        let mut wire_preimage = wire;
        wire_preimage["canonical_sha256"] = Value::String(String::new());
        assert_eq!(
            first["canonical_sha256"],
            crate::canonical_hash(&wire_preimage)
        );
        let dispatched = crate::worker_result(&json!({
            "operation":forgecad_worker_protocol::PRODUCTION_WEAPON_CAGE_OFFSET_OPERATION,
            "payload":request.clone()
        }))
        .unwrap();
        assert_eq!(first, dispatched);
        assert_eq!(first["exact_topology_correspondence"], true);
        assert_eq!(first["offset_field_derived"], true);
        assert_eq!(first["production_stage_advanced"], false);
        assert_eq!(first["promotion_eligible"], false);
        let cage_glb = base64::engine::general_purpose::STANDARD
            .decode(first["cage_glb_base64"].as_str().unwrap())
            .unwrap();
        assert_eq!(
            sha256_hex(&cage_glb),
            first["cage_artifact_sha256"].as_str().unwrap()
        );
        let low_mesh = integrity::extract_diagnostic_mesh(&artifact.glb, 100).unwrap();
        let cage_mesh = integrity::extract_diagnostic_mesh(&cage_glb, 100).unwrap();
        assert_eq!(low_mesh.triangle_count, cage_mesh.triangle_count);
        assert_eq!(
            low_mesh.primitives[0].indices,
            cage_mesh.primitives[0].indices
        );
        assert_ne!(
            low_mesh.primitives[0].positions,
            cage_mesh.primitives[0].positions
        );
        let mut tampered = request;
        tampered["source_low_artifact_sha256"] = Value::String("b".repeat(64));
        assert!(run(tampered.as_object().unwrap()).is_err());
    }
}
