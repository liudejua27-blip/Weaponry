//! Bounded, deterministic Native High artifact to GLB lowering.
//!
//! This module owns no state and performs no I/O.  It emits one embedded-only
//! glTF 2.0 container from the already validated `HighMeshArtifact`; all
//! durable ownership remains with the Runtime/CAS boundary.

use super::{
    authoring_mesh_v2::AuthoringMeshV2HighResult, canonical_bytes, HighMeshArtifact,
    HighMeshGeometry, HighMeshPrimitive, ARTIFACT_SCHEMA_VERSION,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fmt;

const GLB_MAGIC: &[u8; 4] = b"glTF";
const JSON_CHUNK: &[u8; 4] = b"JSON";
const BIN_CHUNK: &[u8; 4] = b"BIN\0";
const MAX_GLB_BYTES: usize = 64 * 1024 * 1024;

pub const HIGH_MESH_GLB_SCHEMA_VERSION: &str = "HighMeshArtifactGlb@1";
/// The direct V2 adapter deliberately keeps the existing embedded GLB
/// container marker so the Low/Cage consumers can use the same parser.  The
/// V2 source/result binding is carried in the root and primitive lineage
/// fields, and is additionally returned as a closed typed readback below.
pub const AUTHORING_MESH_V2_HIGH_ARTIFACT_MATERIALIZE_RESULT_SCHEMA_VERSION: &str =
    "AuthoringMeshV2HighArtifactMaterializeResult@1";
pub const AUTHORING_MESH_V2_HIGH_ARTIFACT_MATERIALIZE_READBACK_SCHEMA_VERSION: &str =
    "AuthoringMeshV2HighGlbReadback@1";
pub const AUTHORING_MESH_V2_HIGH_ARTIFACT_MATERIALIZE_GLB_KIND: &str =
    "authoring-mesh-v2-high-artifact-glb@1";
pub const AUTHORING_MESH_V2_HIGH_ARTIFACT_MATERIALIZE_READBACK_KIND: &str =
    "authoring-mesh-v2-high-artifact-readback@1";
pub const AUTHORING_MESH_V2_HIGH_ARTIFACT_MATERIALIZE_MIME: &str = "model/gltf-binary";

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

/// Strict, V2-specific readback for the GLB that is derived from an
/// `AuthoringMeshV2HighResult@2`.  The existing [`HighGlbReadback`] remains
/// available for the historical Native High adapter; this type preserves the
/// direct V2 result/readback hashes and revision lineage needed by the Low
/// durable seam.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthoringMeshV2HighGlbReadback {
    pub schema_version: String,
    pub glb_sha256: String,
    pub artifact_id: String,
    pub artifact_sha256: String,
    pub mesh_id: String,
    pub lineage_id: String,
    pub revision_id: String,
    pub revision_index: u64,
    pub revision_sha256: String,
    pub source_mesh_sha256: String,
    pub high_evaluation_sha256: String,
    pub high_result_sha256: String,
    pub high_readback_sha256: String,
    pub high_worker_build_cohort_sha256: Option<String>,
    pub part_ids: Vec<String>,
    pub source_node_ids: Vec<String>,
    pub material_zone_ids: Vec<String>,
    pub primitive_count: usize,
    pub triangle_count: u64,
    pub byte_length: usize,
    pub canonical_sha256: String,
}

/// A deterministic embedded-only GLB plus its exact V2 source binding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthoringMeshV2HighGlbArtifact {
    pub glb: Vec<u8>,
    pub glb_sha256: String,
    pub artifact_id: String,
    pub artifact_sha256: String,
    pub readback: AuthoringMeshV2HighGlbReadback,
}

/// Lower direct V2 High output without a cohort binding.  This is useful for
/// pure source/unit tests; Runtime durable callers should use
/// [`lower_authoring_mesh_v2_high_result_with_cohort`] and pass the actual
/// Worker response cohort.
pub fn lower_authoring_mesh_v2_high_result(
    result: &AuthoringMeshV2HighResult,
) -> Result<AuthoringMeshV2HighGlbArtifact, HighGlbError> {
    lower_authoring_mesh_v2_high_result_with_cohort(result, None)
}

/// Lower one validated direct V2 High result to a Low-consumable GLB.  The
/// source result is never mutated.  The conversion uses the existing GLB
/// writer after projecting evaluated V2 parts into its embedded-only shape;
/// the writer itself performs a second lowering and strict readback, so this
/// function also establishes byte-exact replay.
pub fn lower_authoring_mesh_v2_high_result_with_cohort(
    result: &AuthoringMeshV2HighResult,
    worker_build_cohort_sha256: Option<&str>,
) -> Result<AuthoringMeshV2HighGlbArtifact, HighGlbError> {
    validate_v2_result(result, worker_build_cohort_sha256)?;
    lower_authoring_mesh_v2_high_result_typed(result, worker_build_cohort_sha256)
}

/// Lower the exact JSON value received over the worker wire.  The raw value
/// is authenticated before serde converts evaluator coordinates to f32; this
/// preserves the wire high-result hash as the GLB artifact/source identity
/// while still using the typed geometry for the deterministic GLB writer.
pub fn lower_authoring_mesh_v2_high_result_wire(
    value: &Value,
    worker_build_cohort_sha256: Option<&str>,
) -> Result<AuthoringMeshV2HighGlbArtifact, HighGlbError> {
    super::authoring_mesh_v2::verify_wire_result(value).map_err(|error| {
        HighGlbError(format!(
            "AUTHORING_MESH_V2_HIGH_WIRE_RESULT_INVALID:{}",
            error.0
        ))
    })?;
    let result: AuthoringMeshV2HighResult = serde_json::from_value(value.clone())?;
    validate_v2_geometry(&result, worker_build_cohort_sha256)?;
    lower_authoring_mesh_v2_high_result_typed(&result, worker_build_cohort_sha256)
}

fn lower_authoring_mesh_v2_high_result_typed(
    result: &AuthoringMeshV2HighResult,
    worker_build_cohort_sha256: Option<&str>,
) -> Result<AuthoringMeshV2HighGlbArtifact, HighGlbError> {
    let artifact = v2_result_as_glb_artifact(result, worker_build_cohort_sha256)?;
    let first = lower_high_mesh_artifact_with_surface_attributes(&artifact)?;
    let second = lower_high_mesh_artifact_with_surface_attributes(&artifact)?;
    if first.glb != second.glb {
        return Err(HighGlbError(
            "AUTHORING_MESH_V2_HIGH_GLB_REPLAY_NON_DETERMINISTIC".to_owned(),
        ));
    }
    let strict =
        readback_authoring_mesh_v2_high_glb_typed(&first.glb, result, worker_build_cohort_sha256)?;
    let glb_sha256 = sha256(&first.glb);
    if glb_sha256 != strict.glb_sha256 {
        return Err(HighGlbError(
            "AUTHORING_MESH_V2_HIGH_GLB_HASH_MISMATCH".to_owned(),
        ));
    }
    Ok(AuthoringMeshV2HighGlbArtifact {
        glb: first.glb,
        glb_sha256,
        artifact_id: artifact.artifact_id,
        artifact_sha256: artifact.artifact_sha256,
        readback: strict,
    })
}

/// Recompute the GLB source/geometry binding and V2 hash lineage.  This is
/// intentionally separate from the generic GLB readback so a consumer cannot
/// accept a valid old Native High artifact under a direct V2 identity.
pub fn readback_authoring_mesh_v2_high_glb(
    glb: &[u8],
    result: &AuthoringMeshV2HighResult,
    expected_cohort: Option<&str>,
) -> Result<AuthoringMeshV2HighGlbReadback, HighGlbError> {
    validate_v2_result(result, expected_cohort)?;
    readback_authoring_mesh_v2_high_glb_typed(glb, result, expected_cohort)
}

fn readback_authoring_mesh_v2_high_glb_typed(
    glb: &[u8],
    result: &AuthoringMeshV2HighResult,
    expected_cohort: Option<&str>,
) -> Result<AuthoringMeshV2HighGlbReadback, HighGlbError> {
    let artifact = v2_result_as_glb_artifact(result, expected_cohort)?;
    let generic = readback_high_mesh_glb_with_surface_attributes(glb, &artifact)?;
    let expected_part_ids = artifact.part_ids.clone();
    let expected_material_zone_ids = artifact.material_zone_ids.clone();
    if generic.part_ids != expected_part_ids
        || generic.base_primitive_count != artifact.base_parts.len()
        || generic.detail_primitive_count != 0
        || generic.triangle_count != artifact.triangle_count
    {
        return Err(HighGlbError(
            "AUTHORING_MESH_V2_HIGH_GLB_LINEAGE_MISMATCH".to_owned(),
        ));
    }
    let mut source_node_ids = Vec::new();
    let mut source_node_set = BTreeSet::new();
    for primitive in &artifact.base_parts {
        let values = if primitive.source_node_ids.is_empty() {
            std::slice::from_ref(&primitive.source_node_id)
        } else {
            primitive.source_node_ids.as_slice()
        };
        for source_node_id in values {
            if source_node_set.insert(source_node_id.clone()) {
                source_node_ids.push(source_node_id.clone());
            }
        }
    }
    let mut readback = AuthoringMeshV2HighGlbReadback {
        schema_version: AUTHORING_MESH_V2_HIGH_ARTIFACT_MATERIALIZE_READBACK_SCHEMA_VERSION
            .to_owned(),
        glb_sha256: generic.glb_sha256,
        artifact_id: artifact.artifact_id.clone(),
        artifact_sha256: artifact.artifact_sha256.clone(),
        mesh_id: result.mesh_id.clone(),
        lineage_id: result.lineage_id.clone(),
        revision_id: result.revision_id.clone(),
        revision_index: result.revision_index,
        revision_sha256: result.revision_sha256.clone(),
        source_mesh_sha256: result.readback.projected_source_mesh_sha256.clone(),
        high_evaluation_sha256: result.readback.high_evaluation_sha256.clone(),
        high_result_sha256: result.canonical_sha256.clone(),
        high_readback_sha256: result.readback.canonical_sha256.clone(),
        high_worker_build_cohort_sha256: expected_cohort.map(str::to_owned),
        part_ids: expected_part_ids,
        source_node_ids,
        material_zone_ids: expected_material_zone_ids,
        primitive_count: artifact.base_parts.len(),
        triangle_count: artifact.triangle_count,
        byte_length: glb.len(),
        canonical_sha256: String::new(),
    };
    readback.canonical_sha256 = hash_without_field(&readback, "canonical_sha256");
    Ok(readback)
}

fn validate_v2_result(
    result: &AuthoringMeshV2HighResult,
    expected_cohort: Option<&str>,
) -> Result<(), HighGlbError> {
    super::authoring_mesh_v2::verify_readback(result).map_err(|error| {
        HighGlbError(format!("AUTHORING_MESH_V2_HIGH_RESULT_INVALID:{}", error.0))
    })?;
    validate_v2_geometry(result, expected_cohort)
}

fn validate_v2_geometry(
    result: &AuthoringMeshV2HighResult,
    expected_cohort: Option<&str>,
) -> Result<(), HighGlbError> {
    if result.evaluation.evaluated_parts.is_empty()
        || result.evaluation.evaluated_parts.len() > 128
        || result.canonical_sha256.len() != 64
        || !is_sha256(&result.canonical_sha256)
    {
        return Err(HighGlbError(
            "AUTHORING_MESH_V2_HIGH_RESULT_INVALID".to_owned(),
        ));
    }
    if let Some(cohort) = expected_cohort {
        if !is_sha256(cohort) {
            return Err(HighGlbError(
                "AUTHORING_MESH_V2_HIGH_COHORT_INVALID".to_owned(),
            ));
        }
    }
    let mut output_ids = BTreeSet::new();
    let mut part_ids = BTreeSet::new();
    let mut total_vertices = 0usize;
    let mut total_triangles = 0usize;
    for part in &result.evaluation.evaluated_parts {
        if !output_ids.insert(part.output_part_id.clone()) || !part_ids.insert(part.part_id.clone())
        {
            return Err(HighGlbError(
                "AUTHORING_MESH_V2_HIGH_DUPLICATE_EVALUATED_PART".to_owned(),
            ));
        }
        if part.positions_m.is_empty() || part.indices.is_empty() {
            return Err(HighGlbError(
                "AUTHORING_MESH_V2_HIGH_EMPTY_EVALUATED_PART".to_owned(),
            ));
        }
        total_vertices = total_vertices
            .checked_add(part.positions_m.len())
            .ok_or_else(|| {
                HighGlbError("AUTHORING_MESH_V2_HIGH_OUTPUT_BUDGET_EXCEEDED".to_owned())
            })?;
        total_triangles = total_triangles
            .checked_add(part.indices.len())
            .ok_or_else(|| {
                HighGlbError("AUTHORING_MESH_V2_HIGH_OUTPUT_BUDGET_EXCEEDED".to_owned())
            })?;
    }
    if total_vertices > 300_000 || total_triangles > 600_000 {
        return Err(HighGlbError(
            "AUTHORING_MESH_V2_HIGH_OUTPUT_BUDGET_EXCEEDED".to_owned(),
        ));
    }
    Ok(())
}

fn v2_result_as_glb_artifact(
    result: &AuthoringMeshV2HighResult,
    worker_build_cohort_sha256: Option<&str>,
) -> Result<HighMeshArtifact, HighGlbError> {
    let artifact_sha256 = result.canonical_sha256.clone();
    let artifact_id = format!("high-mesh-{}", &artifact_sha256[..24]);
    let base_parts = result
        .evaluation
        .evaluated_parts
        .iter()
        .map(|part| HighMeshPrimitive {
            primitive_id: part.output_part_id.clone(),
            kind: "authoring_mesh_v2_high_evaluated".to_owned(),
            part_id: part.part_id.clone(),
            source_node_ids: part.source_node_ids.clone(),
            source_node_id: part.source_node_id.clone(),
            material_zone_id: part.material_zone_id.clone(),
            source_element_lineage: part.source_element_lineage.clone(),
            geometry: HighMeshGeometry {
                positions_m: part.positions_m.clone(),
                indices: part.indices.clone(),
            },
        })
        .collect::<Vec<_>>();
    let part_ids = base_parts
        .iter()
        .map(|part| part.part_id.clone())
        .collect::<Vec<_>>();
    // Keep the source/evaluated Part order in the embedded readback.  A
    // sorted set would make the material-zone inventory deterministic, but
    // it would no longer be position-aligned with the GLB primitive order;
    // consumers use that order to retain the blade/guard/grip/pommel
    // semantic mapping.  Deduplicate only after the first occurrence so the
    // inventory remains stable without reordering primitives.
    let mut material_zone_ids = Vec::new();
    let mut material_zone_set = BTreeSet::new();
    for part in &base_parts {
        if material_zone_set.insert(part.material_zone_id.clone()) {
            material_zone_ids.push(part.material_zone_id.clone());
        }
    }
    let triangle_count = base_parts
        .iter()
        .map(|part| part.geometry.indices.len() as u64)
        .sum();
    Ok(HighMeshArtifact {
        schema_version: ARTIFACT_SCHEMA_VERSION.to_owned(),
        operation: super::OPERATION.to_owned(),
        policy: "forgecad-authoring-mesh-v2-high-evaluated-glb@1".to_owned(),
        artifact_id,
        artifact_sha256: artifact_sha256.clone(),
        source_authoring_mesh_sha256: result.readback.projected_source_mesh_sha256.clone(),
        detail_graph_canonical_sha256: result.readback.high_evaluation_sha256.clone(),
        request_sha256: result.revision_sha256.clone(),
        input_sha256: result.revision_sha256.clone(),
        high_worker_algorithm_sha256: result.high_worker_algorithm_sha256.clone(),
        high_worker_build_cohort_sha256: worker_build_cohort_sha256.unwrap_or_default().to_owned(),
        replay_count: 2,
        replay_byte_exact: true,
        base_parts,
        detail_primitives: Vec::new(),
        detail_lineage: Vec::new(),
        part_ids,
        material_zone_ids,
        triangle_count,
        base_triangle_count: triangle_count,
        detail_triangle_count: 0,
        non_destructive: result.non_destructive,
        high_topology_status: result.evaluation.evaluator_contract.topology.clone(),
        high_authoring_topology_status: "source-preserved".to_owned(),
        uv_status: "NOT_RUN".to_owned(),
        tangent_status: "NOT_RUN".to_owned(),
        structural_status: result.evaluation.structural_status.clone(),
        visual_status: result.evaluation.visual_status.clone(),
        human_status: result.evaluation.human_status.clone(),
        engine_status: "NOT_RUN".to_owned(),
        distribution_status: "NOT_RUN".to_owned(),
        quality_status: result.quality_status.clone(),
        hard_gate_passed: false,
        runtime_write_performed: false,
        production_stage_advanced: false,
        candidate_confirmed: false,
        version_created: false,
        export_performed: false,
        canonical_sha256: artifact_sha256.clone(),
    })
}

fn hash_without_field<T: serde::Serialize>(value: &T, field: &str) -> String {
    let mut value = serde_json::to_value(value).expect("V2 High GLB readback serializable");
    value[field] = Value::String(String::new());
    sha256(&canonical_bytes(&value))
}

/// Lower twice and fail closed if the byte stream is not replay-identical.
pub fn lower_high_mesh_artifact(
    artifact: &HighMeshArtifact,
) -> Result<HighGlbArtifact, HighGlbError> {
    lower_high_mesh_artifact_with_attributes(artifact, false)
}

/// Lower a validated artifact with the minimum vertex streams required by
/// the fixed perspective renderer.  This is deliberately an explicit path:
/// legacy Native High bytes retain their historical POSITION-only shape,
/// while direct V2 High bytes always carry deterministic NORMAL and
/// TEXCOORD_0 streams.
fn lower_high_mesh_artifact_with_surface_attributes(
    artifact: &HighMeshArtifact,
) -> Result<HighGlbArtifact, HighGlbError> {
    lower_high_mesh_artifact_with_attributes(artifact, true)
}

fn lower_high_mesh_artifact_with_attributes(
    artifact: &HighMeshArtifact,
    include_surface_attributes: bool,
) -> Result<HighGlbArtifact, HighGlbError> {
    let first = lower_once(artifact, include_surface_attributes)?;
    let second = lower_once(artifact, include_surface_attributes)?;
    if first != second {
        return Err(HighGlbError("HIGH_GLB_REPLAY_NON_DETERMINISTIC".to_owned()));
    }
    let readback = if include_surface_attributes {
        readback_high_mesh_glb_with_surface_attributes(&first, artifact)?
    } else {
        readback_high_mesh_glb(&first, artifact)?
    };
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

fn readback_high_mesh_glb_with_surface_attributes(
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
        if !primitive.source_node_ids.is_empty() {
            if primitive.source_node_ids.first() != Some(&primitive.source_node_id)
                || primitive.source_node_ids.len() > 16
            {
                return Err(HighGlbError("HIGH_GLB_SOURCE_NODE_SET_INVALID".to_owned()));
            }
            let mut source_nodes = BTreeSet::new();
            for source_node_id in &primitive.source_node_ids {
                safe_id(source_node_id, "source_node_ids")?;
                if !source_nodes.insert(source_node_id) {
                    return Err(HighGlbError("HIGH_GLB_DUPLICATE_SOURCE_NODE_ID".to_owned()));
                }
            }
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

fn lower_once(
    artifact: &HighMeshArtifact,
    include_surface_attributes: bool,
) -> Result<Vec<u8>, HighGlbError> {
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
        let (normal_values, uv_values) = if include_surface_attributes {
            (
                vertex_normals(&primitive.geometry)?,
                planar_uvs(&primitive.geometry.positions_m),
            )
        } else {
            (Vec::new(), Vec::new())
        };
        let normal_offset = if include_surface_attributes {
            align4(&mut binary);
            let offset = binary.len();
            for normal in &normal_values {
                for value in normal {
                    binary.extend_from_slice(&value.to_le_bytes());
                }
            }
            Some(offset)
        } else {
            None
        };
        let uv_offset = if include_surface_attributes {
            align4(&mut binary);
            let offset = binary.len();
            for uv in &uv_values {
                for value in uv {
                    binary.extend_from_slice(&value.to_le_bytes());
                }
            }
            Some(offset)
        } else {
            None
        };
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
        let normal_view = normal_offset.map(|offset| {
            let view = buffer_views.len();
            buffer_views.push(json!({
                "buffer": 0,
                "byteOffset": offset,
                "byteLength": normal_values.len() * 12,
                "target": 34962
            }));
            view
        });
        let uv_view = uv_offset.map(|offset| {
            let view = buffer_views.len();
            buffer_views.push(json!({
                "buffer": 0,
                "byteOffset": offset,
                "byteLength": uv_values.len() * 8,
                "target": 34962
            }));
            view
        });
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
        let normal_accessor = normal_view.map(|view| {
            let accessor = accessors.len();
            let (min, max) = bounds(&normal_values);
            accessors.push(json!({
                "bufferView": view,
                "componentType": 5126,
                "count": normal_values.len(),
                "type": "VEC3",
                "min": min,
                "max": max
            }));
            accessor
        });
        let uv_accessor = uv_view.map(|view| {
            let accessor = accessors.len();
            accessors.push(json!({
                "bufferView": view,
                "componentType": 5126,
                "count": uv_values.len(),
                "type": "VEC2"
            }));
            accessor
        });
        let index_accessor = accessors.len();
        accessors.push(json!({
            "bufferView": index_view,
            "componentType": 5125,
            "count": primitive.geometry.indices.len() * 3,
            "type": "SCALAR"
        }));

        let attributes = if include_surface_attributes {
            json!({
                "POSITION": position_accessor,
                "NORMAL": normal_accessor.expect("normal accessor for surface GLB"),
                "TEXCOORD_0": uv_accessor.expect("UV accessor for surface GLB")
            })
        } else {
            json!({"POSITION": position_accessor})
        };

        let mut lineage = json!({
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
        if !primitive.source_node_ids.is_empty() {
            lineage["source_node_ids"] = json!(primitive.source_node_ids);
        }
        primitive_lineage.push(lineage.clone());
        let mesh_name = if index < artifact.base_parts.len() {
            primitive.part_id.clone()
        } else {
            primitive.primitive_id.clone()
        };
        let mut gltf_primitive = json!({
            "attributes": attributes,
            "indices": index_accessor,
            "mode": 4,
            "extras": lineage
        });
        if include_surface_attributes {
            let material_index = artifact
                .material_zone_ids
                .iter()
                .position(|zone| zone == &primitive.material_zone_id)
                .ok_or_else(|| {
                    HighGlbError("HIGH_GLB_MATERIAL_ZONE_NOT_IN_INVENTORY".to_owned())
                })?;
            gltf_primitive["material"] = json!(material_index);
        }
        meshes.push(json!({
            "name": mesh_name,
            "primitives": [gltf_primitive],
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
    let mut root = json!({
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
    if include_surface_attributes {
        // V2 High rendering needs a glTF material index per primitive.  These
        // are deliberately neutral transport materials derived only from the
        // existing material-zone inventory; they do not claim authored PBR,
        // texture, wear, engraving, or commercial material quality.
        root["materials"] = Value::Array(
            artifact
                .material_zone_ids
                .iter()
                .map(|zone| {
                    json!({
                        "name": zone,
                        "pbrMetallicRoughness": {
                            "baseColorFactor": [0.5, 0.5, 0.5, 1.0],
                            "metallicFactor": 0.0,
                            "roughnessFactor": 0.7
                        }
                    })
                })
                .collect(),
        );
    }
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

/// Derive deterministic vertex normals from the evaluated triangle winding.
/// The V2 High GLB is intentionally self-contained: the fixed perspective
/// renderer must not have to invent a normal stream after the CAS boundary.
/// Degenerate-only vertices use a fixed up-vector fallback; topology/readback
/// gates remain responsible for rejecting invalid geometry before this point.
fn vertex_normals(geometry: &HighMeshGeometry) -> Result<Vec<[f32; 3]>, HighGlbError> {
    let mut accumulated = vec![[0.0_f32; 3]; geometry.positions_m.len()];
    for triangle in &geometry.indices {
        let a = geometry.positions_m[triangle[0] as usize];
        let b = geometry.positions_m[triangle[1] as usize];
        let c = geometry.positions_m[triangle[2] as usize];
        let ab = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let ac = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
        let face = [
            ab[1] * ac[2] - ab[2] * ac[1],
            ab[2] * ac[0] - ab[0] * ac[2],
            ab[0] * ac[1] - ab[1] * ac[0],
        ];
        if face.iter().any(|value| !value.is_finite()) {
            return Err(HighGlbError(
                "HIGH_GLB_NORMAL_DERIVATION_NON_FINITE".to_owned(),
            ));
        }
        if face.iter().map(|value| value * value).sum::<f32>() <= 1.0e-20 {
            continue;
        }
        for index in triangle {
            let slot = &mut accumulated[*index as usize];
            slot[0] += face[0];
            slot[1] += face[1];
            slot[2] += face[2];
        }
    }
    accumulated
        .into_iter()
        .map(|normal| {
            let length = normal.iter().map(|value| value * value).sum::<f32>().sqrt();
            if !length.is_finite() {
                return Err(HighGlbError(
                    "HIGH_GLB_NORMAL_DERIVATION_NON_FINITE".to_owned(),
                ));
            }
            if length <= f32::EPSILON {
                Ok([0.0, 1.0, 0.0])
            } else {
                Ok([normal[0] / length, normal[1] / length, normal[2] / length])
            }
        })
        .collect()
}

/// Provide the minimum deterministic UV stream needed by the fixed renderer.
/// This is not a production unwrap and is not reported as UV-ready.  It is a
/// bounded object-space planar projection, choosing the thinnest axis as the
/// projection axis so a blade receives stable face-facing coordinates while
/// every primitive remains renderable before the dedicated UV stage.
fn planar_uvs(positions: &[[f32; 3]]) -> Vec<[f32; 2]> {
    if positions.is_empty() {
        return Vec::new();
    }
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    for position in positions {
        for axis in 0..3 {
            min[axis] = min[axis].min(position[axis]);
            max[axis] = max[axis].max(position[axis]);
        }
    }
    let extent = [max[0] - min[0], max[1] - min[1], max[2] - min[2]];
    let drop_axis = if extent[0] <= extent[1] && extent[0] <= extent[2] {
        0
    } else if extent[1] <= extent[2] {
        1
    } else {
        2
    };
    let (u_axis, v_axis) = match drop_axis {
        0 => (1, 2),
        1 => (0, 2),
        _ => (0, 1),
    };
    let u_extent = extent[u_axis].max(f32::EPSILON);
    let v_extent = extent[v_axis].max(f32::EPSILON);
    positions
        .iter()
        .map(|position| {
            [
                ((position[u_axis] - min[u_axis]) / u_extent).clamp(0.0, 1.0),
                ((position[v_axis] - min[v_axis]) / v_extent).clamp(0.0, 1.0),
            ]
        })
        .collect()
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
            if holder.get("extras") != Some(&Value::Object(line.clone())) {
                return Err(HighGlbError(
                    "HIGH_GLB_PRIMITIVE_LINEAGE_HOLDER_MISMATCH".to_owned(),
                ));
            }
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
                || line.get("source_node_id").and_then(Value::as_str)
                    != Some(expected_primitive.source_node_id.as_str())
                || line.get("material_zone_id").and_then(Value::as_str)
                    != Some(expected_primitive.material_zone_id.as_str())
                || line.get("source_element_lineage")
                    != Some(&json!(expected_primitive.source_element_lineage))
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
            .all(|byte| byte.is_ascii_alphanumeric() || b"_.:-@".contains(&byte))
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
