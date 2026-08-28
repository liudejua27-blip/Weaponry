//! Bounded, deterministic Native High artifact to GLB lowering.
//!
//! This module owns no state and performs no I/O.  It emits one embedded-only
//! glTF 2.0 container from the already validated `HighMeshArtifact`; all
//! durable ownership remains with the Runtime/CAS boundary.

use super::{canonical_bytes, HighMeshArtifact, HighMeshPrimitive, ARTIFACT_SCHEMA_VERSION};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fmt;

const GLB_MAGIC: &[u8; 4] = b"glTF";
const JSON_CHUNK: &[u8; 4] = b"JSON";
const BIN_CHUNK: &[u8; 4] = b"BIN\0";
const MAX_GLB_BYTES: usize = 64 * 1024 * 1024;

pub const HIGH_MESH_GLB_SCHEMA_VERSION: &str = "HighMeshArtifactGlb@1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HighGlbError(pub String);

impl fmt::Display for HighGlbError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for HighGlbError {}

impl From<serde_json::Error> for HighGlbError {
    fn from(error: serde_json::Error) -> Self {
        Self(format!("HIGH_GLB_JSON_INVALID:{error}"))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct HighGlbReadback {
    pub glb_sha256: String,
    pub source_artifact_id: String,
    pub source_artifact_sha256: String,
    pub part_ids: Vec<String>,
    pub base_primitive_count: usize,
    pub detail_primitive_count: usize,
    pub base_triangle_count: u64,
    pub detail_triangle_count: u64,
    pub triangle_count: u64,
    pub byte_length: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HighGlbArtifact {
    pub glb: Vec<u8>,
    pub glb_sha256: String,
    pub readback: HighGlbReadback,
}

/// Lower twice and fail closed if the byte stream is not replay-identical.
pub fn lower_high_mesh_artifact(
    artifact: &HighMeshArtifact,
) -> Result<HighGlbArtifact, HighGlbError> {
    let first = lower_once(artifact)?;
    let second = lower_once(artifact)?;
    if first != second {
        return Err(HighGlbError("HIGH_GLB_REPLAY_NON_DETERMINISTIC".to_owned()));
    }
    let readback = readback_high_mesh_glb(&first, artifact)?;
    let glb_sha256 = sha256(&first);
    Ok(HighGlbArtifact {
        glb: first,
        glb_sha256,
        readback,
    })
}

pub fn lower_high_mesh_artifact_bytes(
    artifact: &HighMeshArtifact,
) -> Result<Vec<u8>, HighGlbError> {
    Ok(lower_high_mesh_artifact(artifact)?.glb)
}

/// Alias kept small and explicit for callers that use the lowering term.
pub fn lower_to_glb(artifact: &HighMeshArtifact) -> Result<Vec<u8>, HighGlbError> {
    lower_high_mesh_artifact_bytes(artifact)
}

pub fn readback_high_mesh_glb(
    glb: &[u8],
    expected: &HighMeshArtifact,
) -> Result<HighGlbReadback, HighGlbError> {
    validate_artifact(expected)?;
    validate_container(glb, Some(expected))
}

pub fn inspect_high_mesh_glb(glb: &[u8]) -> Result<HighGlbReadback, HighGlbError> {
    validate_container(glb, None)
}

pub fn strict_readback(
    glb: &[u8],
    expected: &HighMeshArtifact,
) -> Result<HighGlbReadback, HighGlbError> {
    readback_high_mesh_glb(glb, expected)
}

fn validate_artifact(artifact: &HighMeshArtifact) -> Result<Vec<&HighMeshPrimitive>, HighGlbError> {
    if artifact.schema_version != ARTIFACT_SCHEMA_VERSION {
        return Err(HighGlbError("HIGH_GLB_ARTIFACT_SCHEMA_MISMATCH".to_owned()));
    }
    if !is_sha256(&artifact.artifact_sha256) {
        return Err(HighGlbError(
            "HIGH_GLB_SOURCE_ARTIFACT_HASH_INVALID".to_owned(),
        ));
    }
    if artifact.artifact_id != format!("high-mesh-{}", &artifact.artifact_sha256[..24]) {
        return Err(HighGlbError(
            "HIGH_GLB_SOURCE_ARTIFACT_ID_INVALID".to_owned(),
        ));
    }
    if artifact.replay_count != 2 || !artifact.replay_byte_exact {
        return Err(HighGlbError(
            "HIGH_GLB_SOURCE_REPLAY_NOT_VERIFIED".to_owned(),
        ));
    }
    let mut primitives =
        Vec::with_capacity(artifact.base_parts.len() + artifact.detail_primitives.len());
    let mut ids = std::collections::BTreeSet::new();
    for primitive in artifact
        .base_parts
        .iter()
        .chain(&artifact.detail_primitives)
    {
        if !ids.insert(primitive.primitive_id.clone()) {
            return Err(HighGlbError("HIGH_GLB_DUPLICATE_PRIMITIVE_ID".to_owned()));
        }
        for (label, value) in [
            ("primitive_id", &primitive.primitive_id),
            ("part_id", &primitive.part_id),
            ("source_node_id", &primitive.source_node_id),
            ("material_zone_id", &primitive.material_zone_id),
        ] {
            safe_id(value, label)?;
        }
        for lineage in &primitive.source_element_lineage {
            safe_id(lineage, "source_element_lineage")?;
        }
        if primitive.geometry.positions_m.len() > 300_000
            || primitive.geometry.indices.len() > 600_000
        {
            return Err(HighGlbError("HIGH_GLB_GEOMETRY_BUDGET_EXCEEDED".to_owned()));
        }
        for position in &primitive.geometry.positions_m {
            if position.iter().any(|value| !value.is_finite()) {
                return Err(HighGlbError("HIGH_GLB_NON_FINITE_POSITION".to_owned()));
            }
        }
        for triangle in &primitive.geometry.indices {
            if triangle
                .iter()
                .any(|index| *index as usize >= primitive.geometry.positions_m.len())
            {
                return Err(HighGlbError("HIGH_GLB_INDEX_OUT_OF_RANGE".to_owned()));
            }
        }
        primitives.push(primitive);
    }
    if artifact.base_parts.is_empty() {
        return Err(HighGlbError("HIGH_GLB_BASE_PARTS_EMPTY".to_owned()));
    }
    let base_triangles = artifact
        .base_parts
        .iter()
        .map(|primitive| primitive.geometry.indices.len() as u64)
        .sum::<u64>();
    let detail_triangles = artifact
        .detail_primitives
        .iter()
        .map(|primitive| primitive.geometry.indices.len() as u64)
        .sum::<u64>();
    if artifact.base_triangle_count != base_triangles
        || artifact.detail_triangle_count != detail_triangles
        || artifact.triangle_count != base_triangles + detail_triangles
    {
        return Err(HighGlbError("HIGH_GLB_TRIANGLE_COUNT_MISMATCH".to_owned()));
    }
    Ok(primitives)
}

fn lower_once(artifact: &HighMeshArtifact) -> Result<Vec<u8>, HighGlbError> {
    let primitives = validate_artifact(artifact)?;
    let mut binary = Vec::new();
    let mut buffer_views = Vec::new();
    let mut accessors = Vec::new();
    let mut meshes = Vec::new();
    let mut nodes = Vec::new();
    let mut primitive_lineage = Vec::new();

    for (index, primitive) in primitives.iter().enumerate() {
        align4(&mut binary);
        let position_offset = binary.len();
        for position in &primitive.geometry.positions_m {
            for value in position {
                binary.extend_from_slice(&value.to_le_bytes());
            }
        }
        align4(&mut binary);
        let index_offset = binary.len();
        for triangle in &primitive.geometry.indices {
            for value in triangle {
                binary.extend_from_slice(&value.to_le_bytes());
            }
        }
        let position_view = buffer_views.len();
        buffer_views.push(json!({
            "buffer": 0,
            "byteOffset": position_offset,
            "byteLength": primitive.geometry.positions_m.len() * 12,
            "target": 34962
        }));
        let index_view = buffer_views.len();
        buffer_views.push(json!({
            "buffer": 0,
            "byteOffset": index_offset,
            "byteLength": primitive.geometry.indices.len() * 12,
            "target": 34963
        }));
        let position_accessor = accessors.len();
        let (min, max) = bounds(&primitive.geometry.positions_m);
        let mut position_json = json!({
            "bufferView": position_view,
            "componentType": 5126,
            "count": primitive.geometry.positions_m.len(),
            "type": "VEC3"
        });
        if !primitive.geometry.positions_m.is_empty() {
            position_json["min"] = json!(min);
            position_json["max"] = json!(max);
        }
        accessors.push(position_json);
        let index_accessor = accessors.len();
        accessors.push(json!({
            "bufferView": index_view,
            "componentType": 5125,
            "count": primitive.geometry.indices.len() * 3,
            "type": "SCALAR"
        }));

        let lineage = json!({
            "source_schema_version": artifact.schema_version,
            "source_artifact_id": artifact.artifact_id,
            "source_artifact_sha256": artifact.artifact_sha256,
            "primitive_id": primitive.primitive_id,
            "kind": primitive.kind,
            "part_id": primitive.part_id,
            "source_node_id": primitive.source_node_id,
            "material_zone_id": primitive.material_zone_id,
            "source_element_lineage": primitive.source_element_lineage,
            "position_count": primitive.geometry.positions_m.len(),
            "triangle_count": primitive.geometry.indices.len()
        });
        primitive_lineage.push(lineage.clone());
        let mesh_name = if index < artifact.base_parts.len() {
            primitive.part_id.clone()
        } else {
            primitive.primitive_id.clone()
        };
        meshes.push(json!({
            "name": mesh_name,
            "primitives": [{
                "attributes": {"POSITION": position_accessor},
                "indices": index_accessor,
                "mode": 4,
                "extras": lineage
            }],
            "extras": lineage
        }));
        nodes.push(json!({
            "name": mesh_name,
            "mesh": index,
            "extras": lineage
        }));
    }
    align4(&mut binary);
    if binary.len() > MAX_GLB_BYTES {
        return Err(HighGlbError("HIGH_GLB_OUTPUT_BUDGET_EXCEEDED".to_owned()));
    }
    let root = json!({
        "asset": {
            "version": "2.0",
            "generator": "ForgeCAD Native High GLB Lowering@1",
            "extras": {"unit": "meter", "meter": 1.0, "length": "meter"}
        },
        "scene": 0,
        "scenes": [{"nodes": (0..nodes.len()).collect::<Vec<_>>() }],
        "nodes": nodes,
        "meshes": meshes,
        "buffers": [{"byteLength": binary.len()}],
        "bufferViews": buffer_views,
        "accessors": accessors,
        "extras": {"forgecad": {
            "schema_version": HIGH_MESH_GLB_SCHEMA_VERSION,
            "source_schema_version": artifact.schema_version,
            "source_artifact_id": artifact.artifact_id,
            "source_artifact_sha256": artifact.artifact_sha256,
            "part_ids": artifact.part_ids,
            "material_zone_ids": artifact.material_zone_ids,
            "base_primitive_count": artifact.base_parts.len(),
            "detail_primitive_count": artifact.detail_primitives.len(),
            "base_triangle_count": artifact.base_triangle_count,
            "detail_triangle_count": artifact.detail_triangle_count,
            "triangle_count": artifact.triangle_count,
            "units": {"length": "meter", "meter": 1.0},
            "embedded_only": true,
            "external_uri": false,
            "scripts": false,
            "primitive_lineage": primitive_lineage
        }}
    });
    let mut json_bytes = canonical_bytes(&root);
    while json_bytes.len() % 4 != 0 {
        json_bytes.push(b' ');
    }
    let total_length = 12usize
        .checked_add(8 + json_bytes.len())
        .and_then(|length| length.checked_add(8 + binary.len()))
        .ok_or_else(|| HighGlbError("HIGH_GLB_LENGTH_OVERFLOW".to_owned()))?;
    if total_length > MAX_GLB_BYTES || total_length > u32::MAX as usize {
        return Err(HighGlbError("HIGH_GLB_OUTPUT_BUDGET_EXCEEDED".to_owned()));
    }
    let mut glb = Vec::with_capacity(total_length);
    glb.extend_from_slice(GLB_MAGIC);
    glb.extend_from_slice(&2u32.to_le_bytes());
    glb.extend_from_slice(&(total_length as u32).to_le_bytes());
    glb.extend_from_slice(&(json_bytes.len() as u32).to_le_bytes());
    glb.extend_from_slice(JSON_CHUNK);
    glb.extend_from_slice(&json_bytes);
    glb.extend_from_slice(&(binary.len() as u32).to_le_bytes());
    glb.extend_from_slice(BIN_CHUNK);
    glb.extend_from_slice(&binary);
    Ok(glb)
}

fn validate_container(
    glb: &[u8],
    expected: Option<&HighMeshArtifact>,
) -> Result<HighGlbReadback, HighGlbError> {
    if glb.len() < 28 || glb.len() > MAX_GLB_BYTES || &glb[..4] != GLB_MAGIC {
        return Err(HighGlbError("HIGH_GLB_HEADER_INVALID".to_owned()));
    }
    if read_u32(glb, 4)? != 2 || read_u32(glb, 8)? as usize != glb.len() {
        return Err(HighGlbError("HIGH_GLB_HEADER_INVALID".to_owned()));
    }
    let json_length = read_u32(glb, 12)? as usize;
    if &glb[16..20] != JSON_CHUNK || json_length % 4 != 0 {
        return Err(HighGlbError("HIGH_GLB_JSON_CHUNK_INVALID".to_owned()));
    }
    let json_end = 20usize
        .checked_add(json_length)
        .ok_or_else(|| HighGlbError("HIGH_GLB_CHUNK_OVERFLOW".to_owned()))?;
    if json_end.checked_add(8).is_none() || json_end + 8 > glb.len() {
        return Err(HighGlbError("HIGH_GLB_CHUNK_INVALID".to_owned()));
    }
    let bin_length = read_u32(glb, json_end)? as usize;
    if &glb[json_end + 4..json_end + 8] != BIN_CHUNK {
        return Err(HighGlbError("HIGH_GLB_BIN_CHUNK_INVALID".to_owned()));
    }
    let bin_start = json_end + 8;
    if bin_start.checked_add(bin_length) != Some(glb.len()) || bin_length % 4 != 0 {
        return Err(HighGlbError("HIGH_GLB_BIN_CHUNK_INVALID".to_owned()));
    }
    let root: Value = serde_json::from_slice(&glb[20..json_end])?;
    reject_external(&root)?;
    let object = root
        .as_object()
        .ok_or_else(|| HighGlbError("HIGH_GLB_ROOT_INVALID".to_owned()))?;
    if object
        .get("asset")
        .and_then(Value::as_object)
        .and_then(|asset| asset.get("version"))
        .and_then(Value::as_str)
        != Some("2.0")
    {
        return Err(HighGlbError("HIGH_GLB_ASSET_VERSION_INVALID".to_owned()));
    }
    let asset_extras = object["asset"]
        .get("extras")
        .ok_or_else(|| HighGlbError("HIGH_GLB_UNITS_MISSING".to_owned()))?;
    if asset_extras.get("unit").and_then(Value::as_str) != Some("meter")
        || asset_extras.get("meter").and_then(Value::as_f64) != Some(1.0)
    {
        return Err(HighGlbError("HIGH_GLB_UNITS_INVALID".to_owned()));
    }
    let forgecad = object
        .get("extras")
        .and_then(|value| value.get("forgecad"))
        .and_then(Value::as_object)
        .ok_or_else(|| HighGlbError("HIGH_GLB_LINEAGE_MISSING".to_owned()))?;
    if forgecad.get("schema_version").and_then(Value::as_str) != Some(HIGH_MESH_GLB_SCHEMA_VERSION)
    {
        return Err(HighGlbError("HIGH_GLB_LINEAGE_SCHEMA_INVALID".to_owned()));
    }
    let source_id = text(forgecad, "source_artifact_id")?.to_owned();
    let source_hash = text(forgecad, "source_artifact_sha256")?.to_owned();
    if !is_sha256(&source_hash) {
        return Err(HighGlbError(
            "HIGH_GLB_SOURCE_ARTIFACT_HASH_INVALID".to_owned(),
        ));
    }
    let part_ids = forgecad
        .get("part_ids")
        .and_then(Value::as_array)
        .ok_or_else(|| HighGlbError("HIGH_GLB_PART_IDS_MISSING".to_owned()))?
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| HighGlbError("HIGH_GLB_PART_ID_INVALID".to_owned()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let buffers = object
        .get("buffers")
        .and_then(Value::as_array)
        .ok_or_else(|| HighGlbError("HIGH_GLB_BUFFERS_MISSING".to_owned()))?;
    if buffers.len() != 1
        || buffers[0].get("uri").is_some()
        || buffers[0].get("byteLength").and_then(Value::as_u64) != Some(bin_length as u64)
    {
        return Err(HighGlbError("HIGH_GLB_EMBEDDED_BUFFER_INVALID".to_owned()));
    }
    let meshes = object
        .get("meshes")
        .and_then(Value::as_array)
        .ok_or_else(|| HighGlbError("HIGH_GLB_MESHES_MISSING".to_owned()))?;
    let nodes = object
        .get("nodes")
        .and_then(Value::as_array)
        .ok_or_else(|| HighGlbError("HIGH_GLB_NODES_MISSING".to_owned()))?;
    let views = object
        .get("bufferViews")
        .and_then(Value::as_array)
        .ok_or_else(|| HighGlbError("HIGH_GLB_BUFFERVIEWS_MISSING".to_owned()))?;
    let accessors = object
        .get("accessors")
        .and_then(Value::as_array)
        .ok_or_else(|| HighGlbError("HIGH_GLB_ACCESSORS_MISSING".to_owned()))?;
    if meshes.len() != nodes.len() {
        return Err(HighGlbError("HIGH_GLB_NODE_MESH_COUNT_MISMATCH".to_owned()));
    }
    let expected_primitives = expected.map(|artifact| {
        artifact
            .base_parts
            .iter()
            .chain(&artifact.detail_primitives)
            .collect::<Vec<_>>()
    });
    let lineage = forgecad
        .get("primitive_lineage")
        .and_then(Value::as_array)
        .ok_or_else(|| HighGlbError("HIGH_GLB_PRIMITIVE_LINEAGE_MISSING".to_owned()))?;
    if lineage.len() != meshes.len() {
        return Err(HighGlbError("HIGH_GLB_PRIMITIVE_COUNT_MISMATCH".to_owned()));
    }
    let mut base_triangles = 0u64;
    let mut detail_triangles = 0u64;
    for index in 0..meshes.len() {
        let mesh = meshes[index]
            .as_object()
            .ok_or_else(|| HighGlbError("HIGH_GLB_MESH_INVALID".to_owned()))?;
        let node = nodes[index]
            .as_object()
            .ok_or_else(|| HighGlbError("HIGH_GLB_NODE_INVALID".to_owned()))?;
        if node.get("mesh").and_then(Value::as_u64) != Some(index as u64) {
            return Err(HighGlbError(
                "HIGH_GLB_NODE_MESH_BINDING_INVALID".to_owned(),
            ));
        }
        let primitive = mesh
            .get("primitives")
            .and_then(Value::as_array)
            .and_then(|values| values.first())
            .and_then(Value::as_object)
            .ok_or_else(|| HighGlbError("HIGH_GLB_PRIMITIVE_INVALID".to_owned()))?;
        if mesh
            .get("primitives")
            .and_then(Value::as_array)
            .map_or(true, |values| values.len() != 1)
            || primitive.get("mode").and_then(Value::as_u64) != Some(4)
        {
            return Err(HighGlbError("HIGH_GLB_PRIMITIVE_INVALID".to_owned()));
        }
        let line = lineage[index]
            .as_object()
            .ok_or_else(|| HighGlbError("HIGH_GLB_PRIMITIVE_LINEAGE_INVALID".to_owned()))?;
        for holder in [node, mesh, primitive] {
            if holder
                .get("extras")
                .and_then(|value| value.get("source_artifact_sha256"))
                .and_then(Value::as_str)
                != Some(source_hash.as_str())
            {
                return Err(HighGlbError(
                    "HIGH_GLB_SOURCE_HASH_LINEAGE_MISMATCH".to_owned(),
                ));
            }
        }
        let position_accessor = primitive
            .get("attributes")
            .and_then(|value| value.get("POSITION"))
            .and_then(Value::as_u64)
            .ok_or_else(|| HighGlbError("HIGH_GLB_POSITION_ACCESSOR_MISSING".to_owned()))?
            as usize;
        let index_accessor = primitive
            .get("indices")
            .and_then(Value::as_u64)
            .ok_or_else(|| HighGlbError("HIGH_GLB_INDEX_ACCESSOR_MISSING".to_owned()))?
            as usize;
        let positions = read_positions(accessors, views, &glb[bin_start..], position_accessor)?;
        let indices = read_indices(accessors, views, &glb[bin_start..], index_accessor)?;
        if indices.len() % 3 != 0
            || indices
                .iter()
                .any(|index| *index as usize >= positions.len())
        {
            return Err(HighGlbError("HIGH_GLB_TRIANGLE_PAYLOAD_INVALID".to_owned()));
        }
        let triangles = (indices.len() / 3) as u64;
        if line.get("triangle_count").and_then(Value::as_u64) != Some(triangles) {
            return Err(HighGlbError(
                "HIGH_GLB_TRIANGLE_LINEAGE_MISMATCH".to_owned(),
            ));
        }
        if index
            < forgecad
                .get("base_primitive_count")
                .and_then(Value::as_u64)
                .unwrap_or(0) as usize
        {
            base_triangles += triangles;
        } else {
            detail_triangles += triangles;
        }
        if let Some(expected_primitives) = &expected_primitives {
            let expected_primitive = expected_primitives[index];
            if line.get("primitive_id").and_then(Value::as_str)
                != Some(expected_primitive.primitive_id.as_str())
                || line.get("part_id").and_then(Value::as_str)
                    != Some(expected_primitive.part_id.as_str())
                || expected_primitive.geometry.positions_m != positions
                || expected_primitive
                    .geometry
                    .indices
                    .iter()
                    .flat_map(|triangle| triangle.iter().copied())
                    .collect::<Vec<_>>()
                    != indices
            {
                return Err(HighGlbError("HIGH_GLB_SOURCE_GEOMETRY_MISMATCH".to_owned()));
            }
        }
    }
    let triangle_count = base_triangles + detail_triangles;
    if forgecad.get("triangle_count").and_then(Value::as_u64) != Some(triangle_count)
        || expected.is_some_and(|artifact| artifact.triangle_count != triangle_count)
    {
        return Err(HighGlbError("HIGH_GLB_TRIANGLE_COUNT_MISMATCH".to_owned()));
    }
    if let Some(artifact) = expected {
        if source_id != artifact.artifact_id
            || source_hash != artifact.artifact_sha256
            || part_ids != artifact.part_ids
        {
            return Err(HighGlbError("HIGH_GLB_SOURCE_LINEAGE_MISMATCH".to_owned()));
        }
    }
    Ok(HighGlbReadback {
        glb_sha256: sha256(glb),
        source_artifact_id: source_id,
        source_artifact_sha256: source_hash,
        part_ids,
        base_primitive_count: forgecad
            .get("base_primitive_count")
            .and_then(Value::as_u64)
            .unwrap_or(0) as usize,
        detail_primitive_count: forgecad
            .get("detail_primitive_count")
            .and_then(Value::as_u64)
            .unwrap_or(0) as usize,
        base_triangle_count: base_triangles,
        detail_triangle_count: detail_triangles,
        triangle_count,
        byte_length: glb.len(),
    })
}

fn read_positions(
    accessors: &[Value],
    views: &[Value],
    binary: &[u8],
    index: usize,
) -> Result<Vec<[f32; 3]>, HighGlbError> {
    let (bytes, count) = accessor_bytes(accessors, views, binary, index, 5126, "VEC3", 12)?;
    if bytes.len() != count * 12 {
        return Err(HighGlbError("HIGH_GLB_POSITION_BYTES_INVALID".to_owned()));
    }
    Ok(bytes
        .chunks_exact(12)
        .map(|chunk| {
            [
                f32::from_le_bytes(chunk[0..4].try_into().unwrap()),
                f32::from_le_bytes(chunk[4..8].try_into().unwrap()),
                f32::from_le_bytes(chunk[8..12].try_into().unwrap()),
            ]
        })
        .collect())
}

fn read_indices(
    accessors: &[Value],
    views: &[Value],
    binary: &[u8],
    index: usize,
) -> Result<Vec<u32>, HighGlbError> {
    let (bytes, count) = accessor_bytes(accessors, views, binary, index, 5125, "SCALAR", 4)?;
    if bytes.len() != count * 4 {
        return Err(HighGlbError("HIGH_GLB_INDEX_BYTES_INVALID".to_owned()));
    }
    Ok(bytes
        .chunks_exact(4)
        .map(|chunk| u32::from_le_bytes(chunk.try_into().unwrap()))
        .collect())
}

fn accessor_bytes<'a>(
    accessors: &'a [Value],
    views: &'a [Value],
    binary: &'a [u8],
    index: usize,
    component_type: u64,
    kind: &str,
    element_size: usize,
) -> Result<(&'a [u8], usize), HighGlbError> {
    let accessor = accessors
        .get(index)
        .and_then(Value::as_object)
        .ok_or_else(|| HighGlbError("HIGH_GLB_ACCESSOR_INVALID".to_owned()))?;
    if accessor.get("componentType").and_then(Value::as_u64) != Some(component_type)
        || accessor.get("type").and_then(Value::as_str) != Some(kind)
        || accessor.get("byteOffset").is_some()
    {
        return Err(HighGlbError("HIGH_GLB_ACCESSOR_LAYOUT_INVALID".to_owned()));
    }
    let count = accessor
        .get("count")
        .and_then(Value::as_u64)
        .ok_or_else(|| HighGlbError("HIGH_GLB_ACCESSOR_COUNT_INVALID".to_owned()))?
        as usize;
    let view_index = accessor
        .get("bufferView")
        .and_then(Value::as_u64)
        .ok_or_else(|| HighGlbError("HIGH_GLB_ACCESSOR_VIEW_MISSING".to_owned()))?
        as usize;
    let view = views
        .get(view_index)
        .and_then(Value::as_object)
        .ok_or_else(|| HighGlbError("HIGH_GLB_BUFFERVIEW_INVALID".to_owned()))?;
    let offset = view.get("byteOffset").and_then(Value::as_u64).unwrap_or(0) as usize;
    let length = view
        .get("byteLength")
        .and_then(Value::as_u64)
        .ok_or_else(|| HighGlbError("HIGH_GLB_BUFFERVIEW_LENGTH_INVALID".to_owned()))?
        as usize;
    if offset % 4 != 0
        || length != count * element_size
        || offset
            .checked_add(length)
            .is_none_or(|end| end > binary.len())
    {
        return Err(HighGlbError(
            "HIGH_GLB_BUFFERVIEW_LAYOUT_INVALID".to_owned(),
        ));
    }
    Ok((&binary[offset..offset + length], count))
}

fn reject_external(value: &Value) -> Result<(), HighGlbError> {
    match value {
        Value::Object(object) => {
            for (key, child) in object {
                if matches!(
                    key.as_str(),
                    "uri" | "script" | "javascript" | "code" | "path"
                ) {
                    return Err(HighGlbError("HIGH_GLB_EXTERNAL_URI_OR_SCRIPT".to_owned()));
                }
                reject_external(child)?;
            }
        }
        Value::Array(values) => {
            for child in values {
                reject_external(child)?;
            }
        }
        Value::String(string) => {
            let lower = string.to_ascii_lowercase();
            if ["http://", "https://", "file://", "data:", "javascript:"]
                .iter()
                .any(|prefix| lower.contains(prefix))
            {
                return Err(HighGlbError("HIGH_GLB_EXTERNAL_URI_OR_SCRIPT".to_owned()));
            }
        }
        _ => {}
    }
    Ok(())
}

fn bounds(positions: &[[f32; 3]]) -> ([f32; 3], [f32; 3]) {
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    for position in positions {
        for axis in 0..3 {
            min[axis] = min[axis].min(position[axis]);
            max[axis] = max[axis].max(position[axis]);
        }
    }
    (min, max)
}

fn align4(bytes: &mut Vec<u8>) {
    while bytes.len() % 4 != 0 {
        bytes.push(0);
    }
}
fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, HighGlbError> {
    let end = offset
        .checked_add(4)
        .ok_or_else(|| HighGlbError("HIGH_GLB_HEADER_INVALID".to_owned()))?;
    let value = bytes
        .get(offset..end)
        .ok_or_else(|| HighGlbError("HIGH_GLB_HEADER_INVALID".to_owned()))?;
    Ok(u32::from_le_bytes(value.try_into().unwrap()))
}
fn text<'a>(
    object: &'a serde_json::Map<String, Value>,
    key: &str,
) -> Result<&'a str, HighGlbError> {
    object
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| HighGlbError(format!("HIGH_GLB_FIELD_MISSING:{key}")))
}
fn safe_id(value: &str, label: &str) -> Result<(), HighGlbError> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"_.:-".contains(&byte))
    {
        return Err(HighGlbError(format!("HIGH_GLB_ID_INVALID:{label}")));
    }
    Ok(())
}
fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}
fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::request;

    #[test]
    fn high_artifact_glb_is_embedded_and_replay_exact() {
        let artifact = crate::run(&request()).expect("artifact");
        let first = lower_high_mesh_artifact(&artifact).expect("glb");
        let second = lower_high_mesh_artifact(&artifact).expect("repeat glb");
        assert_eq!(first.glb, second.glb);
        assert_eq!(first.readback.triangle_count, artifact.triangle_count);
        assert_eq!(first.readback.part_ids, artifact.part_ids);
        assert_eq!(
            first.readback.source_artifact_sha256,
            artifact.artifact_sha256
        );
        assert_eq!(&first.glb[..4], b"glTF");
        assert!(!first.glb.windows(4).any(|window| window == b"http"));
    }

    #[test]
    fn high_artifact_glb_tamper_fails_strict_readback() {
        let artifact = crate::run(&request()).expect("artifact");
        let mut glb = lower_high_mesh_artifact_bytes(&artifact).expect("glb");
        let json_length = u32::from_le_bytes(glb[12..16].try_into().unwrap()) as usize;
        let json_start = 20;
        let mut root: Value =
            serde_json::from_slice(&glb[json_start..json_start + json_length]).unwrap();
        root["extras"]["forgecad"]["source_artifact_sha256"] = Value::String("0".repeat(64));
        let mut json_bytes = canonical_bytes(&root);
        while json_bytes.len() % 4 != 0 {
            json_bytes.push(b' ');
        }
        let json_byte_length = json_bytes.len();
        let old_end = json_start + json_length;
        glb.splice(json_start..old_end, json_bytes);
        let total = glb.len() as u32;
        glb[8..12].copy_from_slice(&total.to_le_bytes());
        glb[12..16].copy_from_slice(&(json_byte_length as u32).to_le_bytes());
        assert!(readback_high_mesh_glb(&glb, &artifact).is_err());
    }
}
