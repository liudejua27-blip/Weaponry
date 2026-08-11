//! Bounded, product-owned geometry compiler for the MCP007 vertical slice.
//!
//! This is intentionally small: it accepts only a canonical GeometryProgram,
//! a few primitive operators, and emits a deterministic glTF 2.0 GLB.  It is
//! not a general scripting engine and never reads files, starts processes, or
//! calls a model/network service.

pub mod integrity;

use base64::Engine;
pub use forgecad_worker_protocol::{operator_catalog, operator_catalog_sha256};
use image::{imageops, ImageFormat, Rgba, RgbaImage};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::io::Cursor;

const MAX_COORDINATE: f32 = 10.0;
const MAX_DIMENSION: f32 = 10.0;

#[derive(Debug, Clone)]
pub struct GeometryArtifact {
    pub glb: Vec<u8>,
    pub part_ids: Vec<String>,
    pub triangle_count: u64,
    pub program_sha256: String,
    pub uv_status: String,
    pub tangent_status: String,
    pub material_zone_ids: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct RenderPass {
    pub pass: String,
    pub png: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone)]
struct PartMesh {
    part_id: String,
    material_zone_id: String,
    solid: bool,
    /// One semantic Part may contain a deliberately ordered list of primitive
    /// source meshes.  Keeping them separate all the way to GLB lowering is
    /// what lets every decoded triangle retain its source-node lineage while
    /// the glTF mesh/node remains the semantic Part boundary.
    sources: Vec<PartSourceMesh>,
    material: Value,
}

#[derive(Debug, Clone)]
struct PartSourceMesh {
    source_node_id: String,
    operator_id: String,
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    tangents: Vec<[f32; 4]>,
    indices: Vec<u32>,
}

#[derive(Debug)]
struct PrimitiveNodeMesh {
    operator_id: String,
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    indices: Vec<u32>,
}

/// The validated form contains only bounded authoring data. It deliberately
/// does not contain generated mesh/GLB bytes, so the draft-hash path remains
/// pure and cannot create an artifact as a side effect.
#[derive(Debug, Clone)]
struct ValidatedV2GeometryProgram {
    program_sha256: String,
    catalog_sha256: String,
    max_triangles: u64,
    max_glb_bytes: u64,
    max_runtime_ms: u64,
    estimated_triangle_count: u64,
    nodes: Vec<ValidatedV2Node>,
    part_outputs: Vec<ValidatedV2PartOutput>,
}

#[derive(Debug, Clone)]
struct ValidatedV2Node {
    node_id: String,
    primitive: ValidatedV2Primitive,
}

#[derive(Debug, Clone)]
struct ValidatedV2PartOutput {
    part_id: String,
    input_node_ids: Vec<String>,
    material_zone_id: String,
    solid: bool,
}

#[derive(Debug, Clone)]
enum ValidatedV2Primitive {
    Box {
        size_m: [f32; 3],
        position_m: [f32; 3],
        rotation_rad: [f32; 3],
    },
    Cylinder {
        radius_m: f32,
        height_m: f32,
        radial_segments: usize,
        position_m: [f32; 3],
        rotation_rad: [f32; 3],
    },
    Ellipsoid {
        radii_m: [f32; 3],
        longitude_segments: usize,
        latitude_segments: usize,
        position_m: [f32; 3],
        rotation_rad: [f32; 3],
    },
    Sphere {
        radius_m: f32,
        longitude_segments: usize,
        latitude_segments: usize,
        position_m: [f32; 3],
        rotation_rad: [f32; 3],
    },
}

impl ValidatedV2Primitive {
    fn triangle_count(&self) -> u64 {
        match self {
            Self::Box { .. } => 12,
            Self::Cylinder {
                radial_segments, ..
            } => (*radial_segments as u64) * 4,
            Self::Ellipsoid {
                longitude_segments,
                latitude_segments,
                ..
            }
            | Self::Sphere {
                longitude_segments,
                latitude_segments,
                ..
            } => 2 * (*longitude_segments as u64) * ((*latitude_segments as u64) - 1),
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum V2CanonicalHashRequirement {
    OmittedForDraft,
    PresentAndMatches,
}

#[derive(Debug, thiserror::Error)]
pub enum GeometryError {
    #[error("geometry program must be an object")]
    NotObject,
    #[error("geometry program is invalid: {0}")]
    Invalid(String),
}

pub fn compile_geometry_program(program: &Value) -> Result<GeometryArtifact, GeometryError> {
    compile_geometry_program_with_appearance(program, None)
}

/// Validate a non-persistent GeometryProgram@2 authoring draft and return the
/// canonical hash for a semantically valid draft. The draft must intentionally
/// omit `canonical_sha256`; callers add this returned value before calling the
/// write/compile path. Generated-GLB byte/deadline/readback gates still belong
/// to the compiler. This function does not compile a mesh, write CAS/SQLite
/// state, or accept a caller-supplied hash.
pub fn geometry_program_v2_draft_hash(draft: &Value) -> Result<String, GeometryError> {
    Ok(
        validate_geometry_program_v2(draft, V2CanonicalHashRequirement::OmittedForDraft)?
            .program_sha256,
    )
}

/// Compile a canonical GeometryProgram and, when supplied, a hash-bound
/// declarative AppearanceProgram. The worker never executes shader/script
/// payloads or reads external assets.
pub fn compile_geometry_program_with_appearance(
    program: &Value,
    appearance: Option<&Value>,
) -> Result<GeometryArtifact, GeometryError> {
    match program.get("schema_version").and_then(Value::as_str) {
        Some("GeometryProgram@1") => {
            compile_geometry_program_v1_with_appearance(program, appearance)
        }
        Some("GeometryProgram@2") => {
            compile_geometry_program_v2_with_appearance(program, appearance)
        }
        _ => Err(GeometryError::Invalid(
            "schema_version must be GeometryProgram@1 or GeometryProgram@2".to_owned(),
        )),
    }
}

fn compile_geometry_program_v1_with_appearance(
    program: &Value,
    appearance: Option<&Value>,
) -> Result<GeometryArtifact, GeometryError> {
    let object = program.as_object().ok_or(GeometryError::NotObject)?;
    let canonical_sha256 = object
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| GeometryError::Invalid("canonical_sha256 is required".to_owned()))?;
    let mut without_hash = object.clone();
    without_hash.remove("canonical_sha256");
    let program_sha256 = canonical_hash(&Value::Object(without_hash));
    if canonical_sha256 != program_sha256 {
        return Err(GeometryError::Invalid(
            "canonical_sha256 does not match the program".to_owned(),
        ));
    }
    let appearance_zones = validate_appearance(appearance, &program_sha256)?;
    let budgets = object
        .get("budgets")
        .and_then(Value::as_object)
        .ok_or_else(|| GeometryError::Invalid("budgets is required".to_owned()))?;
    let max_nodes = bounded_u64(budgets, "max_nodes", 1, 512)?;
    let max_triangles = bounded_u64(budgets, "max_triangles", 1, 1_000_000)?;
    let _max_runtime_ms = bounded_u64(budgets, "max_runtime_ms", 1, 120_000)?;
    let nodes = object
        .get("nodes")
        .and_then(Value::as_array)
        .ok_or_else(|| GeometryError::Invalid("nodes is required".to_owned()))?;
    if nodes.is_empty() || nodes.len() as u64 > max_nodes {
        return Err(GeometryError::Invalid(
            "node count exceeds the declared budget".to_owned(),
        ));
    }

    let mut node_ids = HashSet::new();
    let mut part_ids = HashSet::new();
    let mut parts = Vec::with_capacity(nodes.len());
    for node in nodes {
        let node = node
            .as_object()
            .ok_or_else(|| GeometryError::Invalid("node must be an object".to_owned()))?;
        let node_id = required_text(node, "node_id")?;
        let part_id = required_text(node, "part_id")?;
        if !node_ids.insert(node_id.to_owned()) || !part_ids.insert(part_id.to_owned()) {
            return Err(GeometryError::Invalid(
                "node_id and part_id must be unique".to_owned(),
            ));
        }
        let operator_id = required_text(node, "operator_id")?;
        if operator_id != "forgecad.geometry.primitive@1"
            && operator_id != "forgecad.geometry.transform@1"
        {
            return Err(GeometryError::Invalid(format!(
                "operator is not in the MCP007 allowlist: {operator_id}"
            )));
        }
        let parameters = node
            .get("parameters")
            .and_then(Value::as_object)
            .ok_or_else(|| {
                GeometryError::Invalid("node parameters must be an object".to_owned())
            })?;
        let shape = parameters
            .get("shape")
            .and_then(Value::as_str)
            .unwrap_or("box");
        let material_zone_id = parameters
            .get("material_zone_id")
            .and_then(Value::as_str)
            .unwrap_or("zone-white-shell")
            .to_owned();
        let position = vec3(parameters, "position", [0.0, 0.0, 0.0], MAX_COORDINATE)?;
        let size = vec3(parameters, "size", [1.0, 1.0, 1.0], MAX_DIMENSION)?;
        let rotation_y = finite_number(parameters, "rotation_y", 0.0)?;
        let segments = parameters
            .get("segments")
            .and_then(Value::as_u64)
            .unwrap_or(16);
        if !(8..=64).contains(&segments) {
            return Err(GeometryError::Invalid(
                "segments must be between 8 and 64".to_owned(),
            ));
        }
        let (mut positions, mut normals, indices) = match shape {
            "box" => box_mesh(size),
            "cylinder" => cylinder_mesh(size, segments as usize),
            "sphere" => sphere_mesh(size, segments as usize),
            _ => {
                return Err(GeometryError::Invalid(format!(
                    "unsupported primitive shape: {shape}"
                )))
            }
        };
        for vertex in &mut positions {
            let rotated = rotate_y(*vertex, rotation_y);
            *vertex = [
                rotated[0] + position[0],
                rotated[1] + position[1],
                rotated[2] + position[2],
            ];
        }
        for normal in &mut normals {
            *normal = normalize(rotate_y(*normal, rotation_y));
        }
        let (positions, normals, uvs, tangents, indices) =
            triangulate_uv_charts(&positions, &normals, &indices)?;
        let material = appearance_zones
            .get(&material_zone_id)
            .cloned()
            .unwrap_or_else(|| material_for_zone(&material_zone_id));
        parts.push(PartMesh {
            part_id: part_id.to_owned(),
            material_zone_id,
            solid: true,
            sources: vec![PartSourceMesh {
                source_node_id: node_id.to_owned(),
                operator_id: operator_id.to_owned(),
                positions,
                normals,
                uvs,
                tangents,
                indices,
            }],
            material,
        });
    }
    let triangle_count = parts
        .iter()
        .flat_map(|part| &part.sources)
        .map(|source| source.indices.len() as u64 / 3)
        .sum::<u64>();
    if triangle_count == 0 || triangle_count > max_triangles {
        return Err(GeometryError::Invalid(
            "triangle count is outside the declared budget".to_owned(),
        ));
    }
    let glb = write_glb(
        &parts,
        &program_sha256,
        triangle_count,
        "ArtifactReadback@1",
        None,
    )?;
    // GeometryProgram@1 stays a transitional compatibility path, but its
    // worker result must not manufacture UV/tangent success from the compiler
    // branch. Read the bytes we are about to return and derive the legacy
    // summary fields from the same physical GLB checks used by Runtime.
    let inspection = integrity::inspect_glb(&glb)?;
    let (uv_status, tangent_status) = physical_uv_tangent_statuses(&inspection);
    let material_zone_ids = ordered_unique_material_zone_ids(&parts);
    Ok(GeometryArtifact {
        glb,
        part_ids: ordered_unique_part_ids(&parts),
        triangle_count,
        program_sha256,
        uv_status,
        tangent_status,
        material_zone_ids,
    })
}

fn compile_geometry_program_v2_with_appearance(
    program: &Value,
    appearance: Option<&Value>,
) -> Result<GeometryArtifact, GeometryError> {
    let started = std::time::Instant::now();
    let validation =
        validate_geometry_program_v2(program, V2CanonicalHashRequirement::PresentAndMatches)?;
    let appearance_zones = validate_appearance(appearance, &validation.program_sha256)?;
    let mut sources = std::collections::BTreeMap::<String, PrimitiveNodeMesh>::new();
    for node in validation.nodes {
        let (positions, normals, indices) = compile_v2_primitive(&node.primitive);
        sources.insert(
            node.node_id,
            PrimitiveNodeMesh {
                operator_id: "forgecad.geometry.primitive@2".to_owned(),
                positions,
                normals,
                indices,
            },
        );
    }
    let mut parts = Vec::with_capacity(validation.part_outputs.len());
    for output in validation.part_outputs {
        let material = if appearance.is_some() {
            appearance_zones
                .get(&output.material_zone_id)
                .cloned()
                .ok_or_else(|| {
                    GeometryError::Invalid(
                        "appearance program does not bind every GeometryProgram@2 material zone"
                            .to_owned(),
                    )
                })?
        } else {
            material_for_zone(&output.material_zone_id)
        };
        let mut part_sources = Vec::with_capacity(output.input_node_ids.len());
        for source_node_id in output.input_node_ids {
            let source = sources.remove(&source_node_id).ok_or_else(|| {
                GeometryError::Invalid(
                    "GeometryProgram@2 part output references an unknown or already consumed source node"
                        .to_owned(),
                )
            })?;
            let (positions, normals, uvs, tangents, indices) =
                triangulate_uv_charts(&source.positions, &source.normals, &source.indices)?;
            part_sources.push(PartSourceMesh {
                source_node_id,
                operator_id: source.operator_id,
                positions,
                normals,
                uvs,
                tangents,
                indices,
            });
        }
        parts.push(PartMesh {
            part_id: output.part_id,
            material_zone_id: output.material_zone_id,
            solid: output.solid,
            sources: part_sources,
            material,
        });
    }
    if !sources.is_empty() {
        return Err(GeometryError::Invalid(
            "GeometryProgram@2 contains unconsumed source nodes".to_owned(),
        ));
    }
    let triangle_count = parts
        .iter()
        .flat_map(|part| &part.sources)
        .map(|source| source.indices.len() as u64 / 3)
        .sum::<u64>();
    if triangle_count != validation.estimated_triangle_count
        || triangle_count == 0
        || triangle_count > validation.max_triangles
    {
        return Err(GeometryError::Invalid(
            "GeometryProgram@2 triangle count is outside the declared budget".to_owned(),
        ));
    }
    let glb = write_glb(
        &parts,
        &validation.program_sha256,
        triangle_count,
        "ArtifactReadback@2",
        Some(&validation.catalog_sha256),
    )?;
    if glb.len() as u64 > validation.max_glb_bytes {
        return Err(GeometryError::Invalid(
            "GeometryProgram@2 GLB exceeds the declared budget".to_owned(),
        ));
    }
    if started.elapsed().as_millis() as u64 > validation.max_runtime_ms {
        return Err(GeometryError::Invalid(
            "GeometryProgram@2 compile exceeded the declared runtime budget".to_owned(),
        ));
    }
    let inspection = integrity::inspect_glb(&glb)?;
    if inspection.program_sha256 != validation.program_sha256
        || inspection.operator_catalog_sha256.as_deref() != Some(validation.catalog_sha256.as_str())
        || !inspection.hard_gate_passed
    {
        return Err(GeometryError::Invalid(format!(
            "GeometryProgram@2 strict GLB readback failed: {}",
            inspection.failure_codes.join(",")
        )));
    }
    let (uv_status, tangent_status) = physical_uv_tangent_statuses(&inspection);
    let material_zone_ids = ordered_unique_material_zone_ids(&parts);
    Ok(GeometryArtifact {
        glb,
        part_ids: ordered_unique_part_ids(&parts),
        triangle_count,
        program_sha256: validation.program_sha256,
        uv_status,
        tangent_status,
        material_zone_ids,
    })
}

/// Shared V2 authoring validation for both the draft-hash read path and the
/// hash-bound compiler. It intentionally validates only typed program
/// semantics: there is no mesh allocation, GLB lowering, CAS write, or
/// persistence in this function.
fn validate_geometry_program_v2(
    program: &Value,
    hash_requirement: V2CanonicalHashRequirement,
) -> Result<ValidatedV2GeometryProgram, GeometryError> {
    let object = program.as_object().ok_or(GeometryError::NotObject)?;
    if matches!(
        hash_requirement,
        V2CanonicalHashRequirement::OmittedForDraft
    ) && object.contains_key("canonical_sha256")
    {
        return Err(GeometryError::Invalid(
            "GeometryProgram@2 draft must omit canonical_sha256".to_owned(),
        ));
    }
    let allowed_root_keys: &[&str] = match hash_requirement {
        V2CanonicalHashRequirement::OmittedForDraft => &[
            "schema_version",
            "project_id",
            "representation_plan_sha256",
            "operator_catalog_sha256",
            "units",
            "budgets",
            "nodes",
            "part_outputs",
        ],
        V2CanonicalHashRequirement::PresentAndMatches => &[
            "schema_version",
            "project_id",
            "representation_plan_sha256",
            "operator_catalog_sha256",
            "units",
            "budgets",
            "nodes",
            "part_outputs",
            "canonical_sha256",
        ],
    };
    require_exact_keys(object, allowed_root_keys, "GeometryProgram@2")?;
    if object.get("schema_version").and_then(Value::as_str) != Some("GeometryProgram@2") {
        return Err(GeometryError::Invalid(
            "schema_version must be GeometryProgram@2".to_owned(),
        ));
    }
    required_identifier(object, "project_id")?;
    required_sha256_text(object, "representation_plan_sha256")?;
    let declared_catalog_sha256 = required_sha256_text(object, "operator_catalog_sha256")?;
    let catalog_sha256 = operator_catalog_sha256();
    if declared_catalog_sha256 != catalog_sha256 {
        return Err(GeometryError::Invalid(
            "operator_catalog_sha256 does not match the active catalog".to_owned(),
        ));
    }

    let mut without_hash = object.clone();
    without_hash.remove("canonical_sha256");
    let program_sha256 = canonical_hash(&Value::Object(without_hash));
    if matches!(
        hash_requirement,
        V2CanonicalHashRequirement::PresentAndMatches
    ) && required_sha256_text(object, "canonical_sha256")? != program_sha256
    {
        return Err(GeometryError::Invalid(
            "canonical_sha256 does not match the program".to_owned(),
        ));
    }

    let units = required_object(object, "units")?;
    require_exact_keys(
        units,
        &["length", "angle", "coordinate_system"],
        "GeometryProgram@2 units",
    )?;
    if units.get("length").and_then(Value::as_str) != Some("meter")
        || units.get("angle").and_then(Value::as_str) != Some("radian")
        || units.get("coordinate_system").and_then(Value::as_str) != Some("right-handed-y-up")
    {
        return Err(GeometryError::Invalid(
            "GeometryProgram@2 units must be meter/radian/right-handed-y-up".to_owned(),
        ));
    }

    let budgets = required_object(object, "budgets")?;
    require_exact_keys(
        budgets,
        [
            "max_nodes",
            "max_triangles",
            "max_glb_bytes",
            "max_worker_memory_bytes",
            "max_runtime_ms",
        ]
        .as_slice(),
        "GeometryProgram@2 budgets",
    )?;
    let max_nodes = bounded_u64(budgets, "max_nodes", 1, 512)?;
    let max_triangles = bounded_u64(budgets, "max_triangles", 1, 250_000)?;
    let max_glb_bytes = bounded_u64(budgets, "max_glb_bytes", 1, 64 * 1024 * 1024)?;
    let _max_worker_memory_bytes =
        bounded_u64(budgets, "max_worker_memory_bytes", 1, 512 * 1024 * 1024)?;
    let max_runtime_ms = bounded_u64(budgets, "max_runtime_ms", 1, 10_000)?;

    let node_values = object
        .get("nodes")
        .and_then(Value::as_array)
        .ok_or_else(|| GeometryError::Invalid("GeometryProgram@2 nodes is required".to_owned()))?;
    if node_values.is_empty() || node_values.len() as u64 > max_nodes {
        return Err(GeometryError::Invalid(
            "GeometryProgram@2 node count exceeds the declared budget".to_owned(),
        ));
    }
    let mut source_primitives = std::collections::BTreeMap::new();
    let mut nodes = Vec::with_capacity(node_values.len());
    for node in node_values {
        let node = node.as_object().ok_or_else(|| {
            GeometryError::Invalid("GeometryProgram@2 node must be an object".to_owned())
        })?;
        require_exact_keys(
            node,
            &["node_id", "operator_id", "inputs", "parameters"],
            "GeometryProgram@2 node",
        )?;
        let node_id = required_identifier(node, "node_id")?.to_owned();
        if required_text(node, "operator_id")? != "forgecad.geometry.primitive@2" {
            return Err(GeometryError::Invalid(
                "operator is not active in OperatorCatalog@1".to_owned(),
            ));
        }
        let inputs = node
            .get("inputs")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                GeometryError::Invalid("GeometryProgram@2 inputs is required".to_owned())
            })?;
        if !inputs.is_empty() {
            return Err(GeometryError::Invalid(
                "primitive@2 accepts exactly zero inputs".to_owned(),
            ));
        }
        let parameters = node
            .get("parameters")
            .and_then(Value::as_object)
            .ok_or_else(|| {
                GeometryError::Invalid(
                    "GeometryProgram@2 node parameters must be an object".to_owned(),
                )
            })?;
        let primitive = validate_v2_primitive_parameters(parameters)?;
        if source_primitives
            .insert(node_id.clone(), primitive.clone())
            .is_some()
        {
            return Err(GeometryError::Invalid(
                "GeometryProgram@2 node_id must be unique".to_owned(),
            ));
        }
        nodes.push(ValidatedV2Node { node_id, primitive });
    }

    let output_values = object
        .get("part_outputs")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            GeometryError::Invalid("GeometryProgram@2 part_outputs is required".to_owned())
        })?;
    if output_values.is_empty() || output_values.len() as u64 > max_nodes {
        return Err(GeometryError::Invalid(
            "GeometryProgram@2 part output count exceeds the declared budget".to_owned(),
        ));
    }
    let mut part_ids = HashSet::new();
    let mut consumed_sources = HashSet::new();
    let mut estimated_triangle_count = 0u64;
    let mut part_outputs = Vec::with_capacity(output_values.len());
    for output in output_values {
        let output = output.as_object().ok_or_else(|| {
            GeometryError::Invalid("GeometryProgram@2 part output must be an object".to_owned())
        })?;
        require_exact_keys(
            output,
            &["part_id", "input_node_ids", "material_zone_id", "solid"],
            "GeometryProgram@2 part output",
        )?;
        let part_id = required_identifier(output, "part_id")?.to_owned();
        let material_zone_id = required_identifier(output, "material_zone_id")?.to_owned();
        let solid = output
            .get("solid")
            .and_then(Value::as_bool)
            .ok_or_else(|| {
                GeometryError::Invalid("GeometryProgram@2 part output solid is required".to_owned())
            })?;
        if !part_ids.insert(part_id.clone()) {
            return Err(GeometryError::Invalid(
                "GeometryProgram@2 part outputs must have unique part ids".to_owned(),
            ));
        }
        let input_values = output
            .get("input_node_ids")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                GeometryError::Invalid(
                    "GeometryProgram@2 part output input_node_ids is required".to_owned(),
                )
            })?;
        if input_values.is_empty() || input_values.len() as u64 > max_nodes {
            return Err(GeometryError::Invalid(
                "GeometryProgram@2 part output input_node_ids must contain one or more source nodes"
                    .to_owned(),
            ));
        }
        let mut input_node_ids = Vec::with_capacity(input_values.len());
        let mut local_sources = HashSet::new();
        for input in input_values {
            let source_node_id = input.as_str().ok_or_else(|| {
                GeometryError::Invalid(
                    "GeometryProgram@2 part output input_node_ids must contain identifiers"
                        .to_owned(),
                )
            })?;
            let source_node_id = required_identifier_value(source_node_id, "input_node_ids")?;
            if !local_sources.insert(source_node_id.to_owned()) {
                return Err(GeometryError::Invalid(
                    "GeometryProgram@2 part output input_node_ids must not repeat a source node"
                        .to_owned(),
                ));
            }
            if !consumed_sources.insert(source_node_id.to_owned()) {
                return Err(GeometryError::Invalid(
                    "GeometryProgram@2 source nodes may be consumed by exactly one part output"
                        .to_owned(),
                ));
            }
            let primitive = source_primitives.get(source_node_id).ok_or_else(|| {
                GeometryError::Invalid(
                    "GeometryProgram@2 part output references an unknown source node".to_owned(),
                )
            })?;
            estimated_triangle_count = estimated_triangle_count
                .checked_add(primitive.triangle_count())
                .ok_or_else(|| {
                    GeometryError::Invalid(
                        "GeometryProgram@2 triangle count is outside the declared budget"
                            .to_owned(),
                    )
                })?;
            input_node_ids.push(source_node_id.to_owned());
        }
        part_outputs.push(ValidatedV2PartOutput {
            part_id,
            input_node_ids,
            material_zone_id,
            solid,
        });
    }
    if consumed_sources.len() != source_primitives.len() {
        return Err(GeometryError::Invalid(
            "GeometryProgram@2 nodes require exactly one explicit part output consumption"
                .to_owned(),
        ));
    }
    if estimated_triangle_count == 0 || estimated_triangle_count > max_triangles {
        return Err(GeometryError::Invalid(
            "GeometryProgram@2 triangle count is outside the declared budget".to_owned(),
        ));
    }

    Ok(ValidatedV2GeometryProgram {
        program_sha256,
        catalog_sha256,
        max_triangles,
        max_glb_bytes,
        max_runtime_ms,
        estimated_triangle_count,
        nodes,
        part_outputs,
    })
}

fn required_identifier_value<'a>(value: &'a str, label: &str) -> Result<&'a str, GeometryError> {
    if value.is_empty()
        || value.len() > 128
        || value.contains(['/', '\\'])
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    {
        return Err(GeometryError::Invalid(format!(
            "{label} is not an identifier"
        )));
    }
    Ok(value)
}

/*
 * The V2 validation block deliberately ends above.  Keep the helper next to
 * `required_identifier` so object fields and ordered array entries share the
 * exact same opaque-ID grammar.
 */

pub fn worker_result(request: &Value) -> Result<Value, GeometryError> {
    let operation = request
        .get("operation")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let payload = request
        .get("payload")
        .and_then(Value::as_object)
        .ok_or_else(|| GeometryError::Invalid("payload is required".to_owned()))?;
    match operation {
        "geometry_program_hash" => {
            require_closed_payload(payload, &["geometry_program_draft"])?;
            let draft = payload.get("geometry_program_draft").ok_or_else(|| {
                GeometryError::Invalid("geometry_program_draft is required".to_owned())
            })?;
            Ok(json!({
                "schema_version":"GeometryProgramHashResult@1",
                "geometry_program_schema_version":"GeometryProgram@2",
                "canonical_sha256":geometry_program_v2_draft_hash(draft)?,
                "operator_catalog_sha256":operator_catalog_sha256(),
                "validation_status":"passed"
            }))
        }
        "compile_geometry" | "render_fixed" => {
            require_closed_payload(payload, &["geometry_program", "appearance_program"])?;
            let program = payload
                .get("geometry_program")
                .ok_or_else(|| GeometryError::Invalid("geometry_program is required".to_owned()))?;
            let artifact = compile_geometry_program_with_appearance(
                program,
                payload.get("appearance_program"),
            )?;
            if operation == "render_fixed" {
                let passes = render_fixed_glb(&artifact.glb)?;
                return Ok(json!({
                    "schema_version":"RenderWorkerResult@1",
                    "passes":passes.iter().map(|pass| json!({"pass":pass.pass,"mime":"image/png","width":pass.width,"height":pass.height,"png_base64":base64::engine::general_purpose::STANDARD.encode(&pass.png)})).collect::<Vec<_>>()
                }));
            }
            Ok(json!({
                "schema_version":"GeometryWorkerResult@1",
                "mime":"model/gltf-binary",
                "glb_base64":base64::engine::general_purpose::STANDARD.encode(artifact.glb),
                "part_ids":artifact.part_ids,
                "triangle_count":artifact.triangle_count,
                "program_sha256":artifact.program_sha256,
                "uv_status":artifact.uv_status,
                "tangent_status":artifact.tangent_status,
                "material_zone_ids":artifact.material_zone_ids
            }))
        }
        "render_glb" => {
            require_closed_payload(payload, &["glb_base64", "camera"])?;
            let encoded = payload
                .get("glb_base64")
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| GeometryError::Invalid("glb_base64 is required".to_owned()))?;
            let glb = base64::engine::general_purpose::STANDARD
                .decode(encoded.as_bytes())
                .map_err(|_| GeometryError::Invalid("glb_base64 is invalid".to_owned()))?;
            if glb.is_empty() || glb.len() > 64 * 1024 * 1024 {
                return Err(GeometryError::Invalid(
                    "GLB exceeds the bounded render input".to_owned(),
                ));
            }
            let camera = payload
                .get("camera")
                .ok_or_else(|| GeometryError::Invalid("camera is required".to_owned()))?;
            let passes = render_perspective_glb(&glb, camera)?;
            Ok(json!({
                "schema_version":"RenderWorkerResult@2",
                "width":512,
                "height":512,
                "renderer_revision":"forgecad-renderer-2",
                "passes":passes.iter().map(|pass| json!({"pass":pass.pass,"mime":"image/png","width":pass.width,"height":pass.height,"png_base64":base64::engine::general_purpose::STANDARD.encode(&pass.png)})).collect::<Vec<_>>()
            }))
        }
        _ => Err(GeometryError::Invalid(
            "worker operation is not allowlisted".to_owned(),
        )),
    }
}

fn require_closed_payload(
    payload: &Map<String, Value>,
    allowed: &[&str],
) -> Result<(), GeometryError> {
    if payload.keys().any(|key| !allowed.contains(&key.as_str())) {
        return Err(GeometryError::Invalid(
            "worker payload contains an unknown field".to_owned(),
        ));
    }
    Ok(())
}

/// V1 result fields are retained only for the MCP008 compatibility contract.
/// They are summaries of decoded GLB BIN attributes, never authoring-time
/// assertions. The richer counters remain Runtime's source of truth.
fn physical_uv_tangent_statuses(inspection: &integrity::GlbIntegrity) -> (String, String) {
    let uv_status =
        if inspection.uv_non_finite_count == 0 && inspection.zero_area_uv_triangle_count == 0 {
            "passed"
        } else {
            "failed"
        };
    let tangent_status = if inspection.tangent_non_finite_count == 0
        && inspection.tangent_orthogonality_error_count == 0
        && inspection.tangent_handedness_error_count == 0
    {
        "passed"
    } else {
        "failed"
    };
    (uv_status.to_owned(), tangent_status.to_owned())
}

fn required_text<'a>(object: &'a Map<String, Value>, key: &str) -> Result<&'a str, GeometryError> {
    let value = object.get(key).and_then(Value::as_str).unwrap_or_default();
    if value.is_empty() || value.len() > 128 || value.contains(['/', '\\']) {
        return Err(GeometryError::Invalid(format!("{key} is invalid")));
    }
    Ok(value)
}

/// GeometryProgram@2 identifiers deliberately use the same small ASCII
/// grammar as the public JSON Schema. Keep this narrower than `required_text`:
/// operator IDs contain `@` and are checked separately against the closed
/// catalog, while project/node/Part/material identifiers must not admit a
/// schema-validity drift into the Runtime-owned hash.
fn required_identifier<'a>(
    object: &'a Map<String, Value>,
    key: &str,
) -> Result<&'a str, GeometryError> {
    let value = required_text(object, key)?;
    if !value
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    {
        return Err(GeometryError::Invalid(format!(
            "{key} is not an identifier"
        )));
    }
    Ok(value)
}

fn bounded_u64(
    object: &Map<String, Value>,
    key: &str,
    min: u64,
    max: u64,
) -> Result<u64, GeometryError> {
    let value = object
        .get(key)
        .and_then(Value::as_u64)
        .ok_or_else(|| GeometryError::Invalid(format!("{key} must be an integer")))?;
    if !(min..=max).contains(&value) {
        return Err(GeometryError::Invalid(format!(
            "{key} is outside its budget"
        )));
    }
    Ok(value)
}

fn finite_number(
    object: &Map<String, Value>,
    key: &str,
    default: f32,
) -> Result<f32, GeometryError> {
    let value = object
        .get(key)
        .and_then(Value::as_f64)
        .unwrap_or(default as f64) as f32;
    if !value.is_finite() || value.abs() > 100.0 {
        return Err(GeometryError::Invalid(format!(
            "{key} is not finite or is too large"
        )));
    }
    Ok(value)
}

fn vec3(
    object: &Map<String, Value>,
    key: &str,
    default: [f32; 3],
    limit: f32,
) -> Result<[f32; 3], GeometryError> {
    let values = object.get(key).and_then(Value::as_array);
    let values = match values {
        Some(values) if values.len() == 3 => values,
        Some(_) => {
            return Err(GeometryError::Invalid(format!(
                "{key} must have three values"
            )))
        }
        None => return Ok(default),
    };
    let mut result = [0.0; 3];
    for (index, value) in values.iter().enumerate() {
        let number = value
            .as_f64()
            .ok_or_else(|| GeometryError::Invalid(format!("{key} contains a non-number")))?
            as f32;
        if !number.is_finite() || number.abs() > limit || (key == "size" && number <= 0.0) {
            return Err(GeometryError::Invalid(format!(
                "{key} contains an out-of-range value"
            )));
        }
        result[index] = number;
    }
    Ok(result)
}

/// Validate the catalog-locked primitive parameters without allocating a mesh.
/// Both the draft-hash path and compiler consume this function so no caller can
/// hash a parameter set that the compiler accepts under a different rule.
fn validate_v2_primitive_parameters(
    parameters: &Map<String, Value>,
) -> Result<ValidatedV2Primitive, GeometryError> {
    let shape = parameters
        .get("shape")
        .and_then(Value::as_str)
        .ok_or_else(|| GeometryError::Invalid("primitive@2 shape is required".to_owned()))?;
    match shape {
        "box" => {
            require_exact_keys(
                parameters,
                &["shape", "size_m", "position_m", "rotation_rad"],
                "primitive@2 box parameters",
            )?;
            Ok(ValidatedV2Primitive::Box {
                size_m: v2_vec3(parameters, "size_m", MAX_DIMENSION, true)?,
                position_m: v2_vec3(parameters, "position_m", MAX_COORDINATE, false)?,
                rotation_rad: v2_vec3(parameters, "rotation_rad", std::f32::consts::TAU, false)?,
            })
        }
        "cylinder" => {
            require_exact_keys(
                parameters,
                &[
                    "shape",
                    "radius_m",
                    "height_m",
                    "radial_segments",
                    "position_m",
                    "rotation_rad",
                ],
                "primitive@2 cylinder parameters",
            )?;
            Ok(ValidatedV2Primitive::Cylinder {
                radius_m: v2_scalar(parameters, "radius_m", MAX_DIMENSION / 2.0, true)?,
                height_m: v2_scalar(parameters, "height_m", MAX_DIMENSION, true)?,
                radial_segments: bounded_u64(parameters, "radial_segments", 8, 64)? as usize,
                position_m: v2_vec3(parameters, "position_m", MAX_COORDINATE, false)?,
                rotation_rad: v2_vec3(parameters, "rotation_rad", std::f32::consts::TAU, false)?,
            })
        }
        "ellipsoid" => {
            require_exact_keys(
                parameters,
                &[
                    "shape",
                    "radii_m",
                    "longitude_segments",
                    "latitude_segments",
                    "position_m",
                    "rotation_rad",
                ],
                "primitive@2 ellipsoid parameters",
            )?;
            Ok(ValidatedV2Primitive::Ellipsoid {
                radii_m: v2_vec3(parameters, "radii_m", MAX_DIMENSION / 2.0, true)?,
                longitude_segments: bounded_u64(parameters, "longitude_segments", 8, 64)? as usize,
                latitude_segments: bounded_u64(parameters, "latitude_segments", 4, 64)? as usize,
                position_m: v2_vec3(parameters, "position_m", MAX_COORDINATE, false)?,
                rotation_rad: v2_vec3(parameters, "rotation_rad", std::f32::consts::TAU, false)?,
            })
        }
        "sphere" => {
            require_exact_keys(
                parameters,
                &[
                    "shape",
                    "radius_m",
                    "longitude_segments",
                    "latitude_segments",
                    "position_m",
                    "rotation_rad",
                ],
                "primitive@2 sphere parameters",
            )?;
            Ok(ValidatedV2Primitive::Sphere {
                radius_m: v2_scalar(parameters, "radius_m", MAX_DIMENSION / 2.0, true)?,
                longitude_segments: bounded_u64(parameters, "longitude_segments", 8, 64)? as usize,
                latitude_segments: bounded_u64(parameters, "latitude_segments", 4, 64)? as usize,
                position_m: v2_vec3(parameters, "position_m", MAX_COORDINATE, false)?,
                rotation_rad: v2_vec3(parameters, "rotation_rad", std::f32::consts::TAU, false)?,
            })
        }
        _ => Err(GeometryError::Invalid(
            "primitive@2 shape is not in the operator catalog".to_owned(),
        )),
    }
}

fn compile_v2_primitive(
    primitive: &ValidatedV2Primitive,
) -> (Vec<[f32; 3]>, Vec<[f32; 3]>, Vec<u32>) {
    let (mut positions, mut normals, indices, position, rotation) = match primitive {
        ValidatedV2Primitive::Box {
            size_m,
            position_m,
            rotation_rad,
        } => {
            let (positions, normals, indices) = box_mesh(*size_m);
            (positions, normals, indices, *position_m, *rotation_rad)
        }
        ValidatedV2Primitive::Cylinder {
            radius_m,
            height_m,
            radial_segments,
            position_m,
            rotation_rad,
        } => {
            let (positions, normals, indices) = cylinder_mesh(
                [*radius_m * 2.0, *height_m, *radius_m * 2.0],
                *radial_segments,
            );
            (positions, normals, indices, *position_m, *rotation_rad)
        }
        ValidatedV2Primitive::Ellipsoid {
            radii_m,
            longitude_segments,
            latitude_segments,
            position_m,
            rotation_rad,
        } => {
            let (positions, normals, indices) = sphere_mesh_with_rings(
                [radii_m[0] * 2.0, radii_m[1] * 2.0, radii_m[2] * 2.0],
                *longitude_segments,
                *latitude_segments,
            );
            (positions, normals, indices, *position_m, *rotation_rad)
        }
        ValidatedV2Primitive::Sphere {
            radius_m,
            longitude_segments,
            latitude_segments,
            position_m,
            rotation_rad,
        } => {
            let (positions, normals, indices) = sphere_mesh_with_rings(
                [*radius_m * 2.0; 3],
                *longitude_segments,
                *latitude_segments,
            );
            (positions, normals, indices, *position_m, *rotation_rad)
        }
    };
    for vertex in &mut positions {
        let rotated = rotate_xyz(*vertex, rotation);
        *vertex = [
            rotated[0] + position[0],
            rotated[1] + position[1],
            rotated[2] + position[2],
        ];
    }
    for normal in &mut normals {
        *normal = normalize(rotate_xyz(*normal, rotation));
    }
    (positions, normals, indices)
}

fn require_exact_keys(
    object: &Map<String, Value>,
    allowed: &[&str],
    label: &str,
) -> Result<(), GeometryError> {
    if object.keys().any(|key| !allowed.contains(&key.as_str()))
        || allowed.iter().any(|key| !object.contains_key(*key))
    {
        return Err(GeometryError::Invalid(format!(
            "{label} must use exactly the closed parameter set"
        )));
    }
    Ok(())
}

fn required_object<'a>(
    object: &'a Map<String, Value>,
    key: &str,
) -> Result<&'a Map<String, Value>, GeometryError> {
    object
        .get(key)
        .and_then(Value::as_object)
        .ok_or_else(|| GeometryError::Invalid(format!("{key} must be an object")))
}

fn required_sha256_text<'a>(
    object: &'a Map<String, Value>,
    key: &str,
) -> Result<&'a str, GeometryError> {
    let value = required_text(object, key)?;
    if value.len() != 64
        || value
            .bytes()
            .any(|byte| !byte.is_ascii_hexdigit() || byte.is_ascii_uppercase())
    {
        return Err(GeometryError::Invalid(format!(
            "{key} must be a lowercase SHA-256"
        )));
    }
    Ok(value)
}

fn v2_scalar(
    object: &Map<String, Value>,
    key: &str,
    limit: f32,
    positive: bool,
) -> Result<f32, GeometryError> {
    let value = object
        .get(key)
        .and_then(Value::as_f64)
        .ok_or_else(|| GeometryError::Invalid(format!("{key} must be a number")))?
        as f32;
    if !value.is_finite() || value.abs() > limit || (positive && value <= 0.0) {
        return Err(GeometryError::Invalid(format!(
            "{key} is outside the product bounds"
        )));
    }
    Ok(value)
}

fn v2_vec3(
    object: &Map<String, Value>,
    key: &str,
    limit: f32,
    positive: bool,
) -> Result<[f32; 3], GeometryError> {
    let values = object
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| GeometryError::Invalid(format!("{key} must be a three-component array")))?;
    if values.len() != 3 {
        return Err(GeometryError::Invalid(format!(
            "{key} must be a three-component array"
        )));
    }
    let mut result = [0.0; 3];
    for (index, value) in values.iter().enumerate() {
        let value = value
            .as_f64()
            .ok_or_else(|| GeometryError::Invalid(format!("{key} contains a non-number")))?
            as f32;
        if !value.is_finite() || value.abs() > limit || (positive && value <= 0.0) {
            return Err(GeometryError::Invalid(format!(
                "{key} is outside the product bounds"
            )));
        }
        result[index] = value;
    }
    Ok(result)
}

fn rotate_y(point: [f32; 3], angle: f32) -> [f32; 3] {
    let (sin, cos) = angle.sin_cos();
    [
        point[0] * cos - point[2] * sin,
        point[1],
        point[0] * sin + point[2] * cos,
    ]
}

fn rotate_xyz(point: [f32; 3], rotation: [f32; 3]) -> [f32; 3] {
    let (sin_x, cos_x) = rotation[0].sin_cos();
    let (sin_y, cos_y) = rotation[1].sin_cos();
    let (sin_z, cos_z) = rotation[2].sin_cos();
    let x_rotated = [
        point[0],
        point[1] * cos_x - point[2] * sin_x,
        point[1] * sin_x + point[2] * cos_x,
    ];
    let y_rotated = [
        x_rotated[0] * cos_y - x_rotated[2] * sin_y,
        x_rotated[1],
        x_rotated[0] * sin_y + x_rotated[2] * cos_y,
    ];
    [
        y_rotated[0] * cos_z - y_rotated[1] * sin_z,
        y_rotated[0] * sin_z + y_rotated[1] * cos_z,
        y_rotated[2],
    ]
}

fn normalize(value: [f32; 3]) -> [f32; 3] {
    let length = (value[0] * value[0] + value[1] * value[1] + value[2] * value[2]).sqrt();
    if length <= f32::EPSILON {
        [0.0, 1.0, 0.0]
    } else {
        [value[0] / length, value[1] / length, value[2] / length]
    }
}

/// Build deterministic local UV charts and MikkTSpace-compatible tangent
/// vectors from the actual triangle geometry.  MCP010B deliberately keeps
/// charts local to each triangle: an atlas/packing policy belongs to MCP010E,
/// but every emitted UV and tangent is already derived from real mesh data.
/// Duplicating chart vertices is intentional; strict topology inspection welds
/// positions before judging a declared solid part.
fn triangulate_uv_charts(
    positions: &[[f32; 3]],
    normals: &[[f32; 3]],
    indices: &[u32],
) -> Result<
    (
        Vec<[f32; 3]>,
        Vec<[f32; 3]>,
        Vec<[f32; 2]>,
        Vec<[f32; 4]>,
        Vec<u32>,
    ),
    GeometryError,
> {
    if positions.is_empty()
        || positions.len() != normals.len()
        || indices.is_empty()
        || indices.len() % 3 != 0
    {
        return Err(GeometryError::Invalid(
            "cannot build UV/tangent data for an invalid mesh".to_owned(),
        ));
    }
    let (min, max) = bounds(positions);
    let extent = [
        (max[0] - min[0]).max(0.0001),
        (max[1] - min[1]).max(0.0001),
        (max[2] - min[2]).max(0.0001),
    ];
    let mut chart_positions = Vec::with_capacity(indices.len());
    let mut chart_normals = Vec::with_capacity(indices.len());
    let mut uvs = Vec::with_capacity(indices.len());
    let mut tangents = Vec::with_capacity(indices.len());
    let mut chart_indices = Vec::with_capacity(indices.len());
    for triangle in indices.chunks_exact(3) {
        let source = [
            triangle[0] as usize,
            triangle[1] as usize,
            triangle[2] as usize,
        ];
        if source.iter().any(|index| *index >= positions.len()) {
            return Err(GeometryError::Invalid(
                "cannot build UV/tangent data for an out-of-range index".to_owned(),
            ));
        }
        let triangle_positions = [
            positions[source[0]],
            positions[source[1]],
            positions[source[2]],
        ];
        let face = cross3(
            subtract3(triangle_positions[1], triangle_positions[0]),
            subtract3(triangle_positions[2], triangle_positions[0]),
        );
        if !finite3(face) || length3(face) <= 1.0e-10 {
            return Err(GeometryError::Invalid(
                "cannot build UV/tangent data for a degenerate triangle".to_owned(),
            ));
        }
        let face = normalize(face);
        let absolute = [face[0].abs(), face[1].abs(), face[2].abs()];
        let projection = if absolute[1] >= absolute[0] && absolute[1] >= absolute[2] {
            1
        } else if absolute[0] >= absolute[2] {
            0
        } else {
            2
        };
        let triangle_uvs = triangle_positions.map(|position| match projection {
            0 => [
                (position[2] - min[2]) / extent[2],
                (position[1] - min[1]) / extent[1],
            ],
            1 => [
                (position[0] - min[0]) / extent[0],
                (position[2] - min[2]) / extent[2],
            ],
            _ => [
                (position[0] - min[0]) / extent[0],
                (position[1] - min[1]) / extent[1],
            ],
        });
        let edge_a = subtract3(triangle_positions[1], triangle_positions[0]);
        let edge_b = subtract3(triangle_positions[2], triangle_positions[0]);
        let uv_a = [
            triangle_uvs[1][0] - triangle_uvs[0][0],
            triangle_uvs[1][1] - triangle_uvs[0][1],
        ];
        let uv_b = [
            triangle_uvs[2][0] - triangle_uvs[0][0],
            triangle_uvs[2][1] - triangle_uvs[0][1],
        ];
        let determinant = uv_a[0] * uv_b[1] - uv_a[1] * uv_b[0];
        if !determinant.is_finite() || determinant.abs() <= 1.0e-8 {
            return Err(GeometryError::Invalid(
                "primitive UV projection produced a zero-area triangle".to_owned(),
            ));
        }
        let reciprocal = 1.0 / determinant;
        let tangent_basis = [
            (edge_a[0] * uv_b[1] - edge_b[0] * uv_a[1]) * reciprocal,
            (edge_a[1] * uv_b[1] - edge_b[1] * uv_a[1]) * reciprocal,
            (edge_a[2] * uv_b[1] - edge_b[2] * uv_a[1]) * reciprocal,
        ];
        let bitangent_basis = [
            (edge_b[0] * uv_a[0] - edge_a[0] * uv_b[0]) * reciprocal,
            (edge_b[1] * uv_a[0] - edge_a[1] * uv_b[0]) * reciprocal,
            (edge_b[2] * uv_a[0] - edge_a[2] * uv_b[0]) * reciprocal,
        ];
        for vertex in 0..3 {
            let normal = normalize(normals[source[vertex]]);
            let tangent = subtract3(tangent_basis, scale3(normal, dot3(normal, tangent_basis)));
            if !finite3(tangent) || length3(tangent) <= f32::EPSILON {
                return Err(GeometryError::Invalid(
                    "primitive UV projection produced an invalid tangent".to_owned(),
                ));
            }
            let tangent = normalize(tangent);
            let handedness = if dot3(cross3(normal, tangent), bitangent_basis) < 0.0 {
                -1.0
            } else {
                1.0
            };
            chart_positions.push(triangle_positions[vertex]);
            chart_normals.push(normal);
            uvs.push(triangle_uvs[vertex]);
            tangents.push([tangent[0], tangent[1], tangent[2], handedness]);
            chart_indices.push((chart_indices.len()) as u32);
        }
    }
    Ok((chart_positions, chart_normals, uvs, tangents, chart_indices))
}

fn subtract3(left: [f32; 3], right: [f32; 3]) -> [f32; 3] {
    [left[0] - right[0], left[1] - right[1], left[2] - right[2]]
}

fn add3(left: [f32; 3], right: [f32; 3]) -> [f32; 3] {
    [left[0] + right[0], left[1] + right[1], left[2] + right[2]]
}

fn scale3(value: [f32; 3], scalar: f32) -> [f32; 3] {
    [value[0] * scalar, value[1] * scalar, value[2] * scalar]
}

fn dot3(left: [f32; 3], right: [f32; 3]) -> f32 {
    left[0] * right[0] + left[1] * right[1] + left[2] * right[2]
}

fn cross3(left: [f32; 3], right: [f32; 3]) -> [f32; 3] {
    [
        left[1] * right[2] - left[2] * right[1],
        left[2] * right[0] - left[0] * right[2],
        left[0] * right[1] - left[1] * right[0],
    ]
}

fn length3(value: [f32; 3]) -> f32 {
    dot3(value, value).sqrt()
}

fn finite3(value: [f32; 3]) -> bool {
    value.into_iter().all(f32::is_finite)
}

fn validate_appearance(
    appearance: Option<&Value>,
    geometry_program_sha256: &str,
) -> Result<HashMap<String, Value>, GeometryError> {
    let Some(appearance) = appearance else {
        return Ok(HashMap::new());
    };
    let object = appearance
        .as_object()
        .ok_or_else(|| GeometryError::Invalid("appearance program must be an object".to_owned()))?;
    if object.get("schema_version").and_then(Value::as_str) != Some("AppearanceProgram@1") {
        return Err(GeometryError::Invalid(
            "appearance schema_version must be AppearanceProgram@1".to_owned(),
        ));
    }
    if object.get("project_id").and_then(Value::as_str).is_none() {
        return Err(GeometryError::Invalid(
            "appearance project_id is required".to_owned(),
        ));
    }
    let expected_geometry = object
        .get("geometry_program_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            GeometryError::Invalid("appearance geometry_program_sha256 is required".to_owned())
        })?;
    if expected_geometry != geometry_program_sha256 {
        return Err(GeometryError::Invalid(
            "appearance is not bound to the geometry program".to_owned(),
        ));
    }
    let canonical_sha256 = object
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            GeometryError::Invalid("appearance canonical_sha256 is required".to_owned())
        })?;
    let mut without_hash = object.clone();
    without_hash.remove("canonical_sha256");
    if canonical_hash(&Value::Object(without_hash)) != canonical_sha256 {
        return Err(GeometryError::Invalid(
            "appearance canonical_sha256 does not match the program".to_owned(),
        ));
    }
    let zones = object
        .get("material_zones")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            GeometryError::Invalid("appearance material_zones is required".to_owned())
        })?;
    if zones.is_empty() || zones.len() > 32 {
        return Err(GeometryError::Invalid(
            "appearance material_zones is outside its budget".to_owned(),
        ));
    }
    let mut result = HashMap::new();
    for zone in zones {
        let zone = zone
            .as_object()
            .ok_or_else(|| GeometryError::Invalid("material zone must be an object".to_owned()))?;
        let zone_id = required_text(zone, "zone_id")?.to_owned();
        if result.contains_key(&zone_id) {
            return Err(GeometryError::Invalid(
                "material zone ids must be unique".to_owned(),
            ));
        }
        let part_ids = zone
            .get("part_ids")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                GeometryError::Invalid("material zone part_ids is required".to_owned())
            })?;
        if part_ids.is_empty() || part_ids.iter().any(|value| value.as_str().is_none()) {
            return Err(GeometryError::Invalid(
                "material zone part_ids is invalid".to_owned(),
            ));
        }
        let base_color = color4(zone, "base_color")?;
        let metallic = bounded_float(zone, "metallic", 0.0, 1.0)?;
        let roughness = bounded_float(zone, "roughness", 0.0001, 1.0)?;
        let emissive = color3(zone, "emissive")?;
        result.insert(
            zone_id.clone(),
            json!({"name":zone_id,"pbrMetallicRoughness":{"baseColorFactor":base_color,"metallicFactor":metallic,"roughnessFactor":roughness},"emissiveFactor":emissive}),
        );
    }
    Ok(result)
}

fn bounded_float(
    object: &Map<String, Value>,
    key: &str,
    min: f32,
    max: f32,
) -> Result<f32, GeometryError> {
    let value = object
        .get(key)
        .and_then(Value::as_f64)
        .ok_or_else(|| GeometryError::Invalid(format!("{key} must be a number")))?
        as f32;
    if !value.is_finite() || value < min || value > max {
        return Err(GeometryError::Invalid(format!(
            "{key} is outside its range"
        )));
    }
    Ok(value)
}

fn color3(object: &Map<String, Value>, key: &str) -> Result<[f32; 3], GeometryError> {
    let values = object
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| GeometryError::Invalid(format!("{key} must be a three-component color")))?;
    if values.len() != 3 {
        return Err(GeometryError::Invalid(format!(
            "{key} must be a three-component color"
        )));
    }
    Ok([
        bounded_number(values[0].as_f64(), key)?,
        bounded_number(values[1].as_f64(), key)?,
        bounded_number(values[2].as_f64(), key)?,
    ])
}

fn color4(object: &Map<String, Value>, key: &str) -> Result<[f32; 4], GeometryError> {
    let values = object
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| GeometryError::Invalid(format!("{key} must be a four-component color")))?;
    if values.len() != 4 {
        return Err(GeometryError::Invalid(format!(
            "{key} must be a four-component color"
        )));
    }
    Ok([
        bounded_number(values[0].as_f64(), key)?,
        bounded_number(values[1].as_f64(), key)?,
        bounded_number(values[2].as_f64(), key)?,
        bounded_number(values[3].as_f64(), key)?,
    ])
}

fn bounded_number(value: Option<f64>, key: &str) -> Result<f32, GeometryError> {
    let value =
        value.ok_or_else(|| GeometryError::Invalid(format!("{key} contains a non-number")))? as f32;
    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
        return Err(GeometryError::Invalid(format!(
            "{key} contains an out-of-range value"
        )));
    }
    Ok(value)
}

fn box_mesh(size: [f32; 3]) -> (Vec<[f32; 3]>, Vec<[f32; 3]>, Vec<u32>) {
    let [x, y, z] = [size[0] / 2.0, size[1] / 2.0, size[2] / 2.0];
    let faces = [
        (
            [1.0, 0.0, 0.0],
            [[x, -y, -z], [x, -y, z], [x, y, z], [x, y, -z]],
        ),
        (
            [-1.0, 0.0, 0.0],
            [[-x, -y, z], [-x, -y, -z], [-x, y, -z], [-x, y, z]],
        ),
        (
            [0.0, 1.0, 0.0],
            [[-x, y, -z], [x, y, -z], [x, y, z], [-x, y, z]],
        ),
        (
            [0.0, -1.0, 0.0],
            [[-x, -y, z], [x, -y, z], [x, -y, -z], [-x, -y, -z]],
        ),
        (
            [0.0, 0.0, 1.0],
            [[x, -y, z], [-x, -y, z], [-x, y, z], [x, y, z]],
        ),
        (
            [0.0, 0.0, -1.0],
            [[-x, -y, -z], [x, -y, -z], [x, y, -z], [-x, y, -z]],
        ),
    ];
    let mut positions = Vec::with_capacity(24);
    let mut normals = Vec::with_capacity(24);
    let mut indices = Vec::with_capacity(36);
    for (face_index, (normal, vertices)) in faces.into_iter().enumerate() {
        let base = (face_index * 4) as u32;
        positions.extend(vertices);
        normals.extend([normal; 4]);
        indices.extend([base, base + 2, base + 1, base, base + 3, base + 2]);
    }
    (positions, normals, indices)
}

fn cylinder_mesh(size: [f32; 3], segments: usize) -> (Vec<[f32; 3]>, Vec<[f32; 3]>, Vec<u32>) {
    let radius_x = size[0] / 2.0;
    let radius_z = size[2] / 2.0;
    let half_height = size[1] / 2.0;
    let mut positions = Vec::with_capacity(segments * 4 + 2);
    let mut normals = Vec::with_capacity(segments * 4 + 2);
    let mut indices = Vec::new();
    // The side, bottom cap and top cap deliberately do not share vertices:
    // their normals and UV charts differ, while strict topology readback welds
    // coincident positions before evaluating a declared solid part.
    for ring in [-half_height, half_height] {
        for i in 0..segments {
            let angle = std::f32::consts::TAU * i as f32 / segments as f32;
            positions.push([radius_x * angle.cos(), ring, radius_z * angle.sin()]);
            normals.push(normalize([
                angle.cos() / radius_x,
                0.0,
                angle.sin() / radius_z,
            ]));
        }
    }
    for i in 0..segments {
        let next = (i + 1) % segments;
        let bottom = i as u32;
        let top = (segments + i) as u32;
        indices.extend([
            bottom,
            top,
            next as u32,
            next as u32,
            top,
            (segments + next) as u32,
        ]);
    }
    let bottom_start = positions.len() as u32;
    for i in 0..segments {
        let angle = std::f32::consts::TAU * i as f32 / segments as f32;
        positions.push([radius_x * angle.cos(), -half_height, radius_z * angle.sin()]);
        normals.push([0.0, -1.0, 0.0]);
    }
    let bottom_center = positions.len() as u32;
    positions.push([0.0, -half_height, 0.0]);
    normals.push([0.0, -1.0, 0.0]);
    let top_start = positions.len() as u32;
    for i in 0..segments {
        let angle = std::f32::consts::TAU * i as f32 / segments as f32;
        positions.push([radius_x * angle.cos(), half_height, radius_z * angle.sin()]);
        normals.push([0.0, 1.0, 0.0]);
    }
    let top_center = positions.len() as u32;
    positions.push([0.0, half_height, 0.0]);
    normals.push([0.0, 1.0, 0.0]);
    for i in 0..segments {
        let next = (i + 1) % segments;
        indices.extend([
            bottom_center,
            bottom_start + i as u32,
            bottom_start + next as u32,
        ]);
        indices.extend([top_center, top_start + next as u32, top_start + i as u32]);
    }
    (positions, normals, indices)
}

fn sphere_mesh(size: [f32; 3], segments: usize) -> (Vec<[f32; 3]>, Vec<[f32; 3]>, Vec<u32>) {
    sphere_mesh_with_rings(size, segments, (segments / 2).max(4))
}

fn sphere_mesh_with_rings(
    size: [f32; 3],
    segments: usize,
    rings: usize,
) -> (Vec<[f32; 3]>, Vec<[f32; 3]>, Vec<u32>) {
    let radii = [size[0] / 2.0, size[1] / 2.0, size[2] / 2.0];
    let mut positions = Vec::with_capacity(2 + (rings - 1) * (segments + 1));
    let mut normals = Vec::with_capacity(2 + (rings - 1) * (segments + 1));
    let mut indices = Vec::new();
    positions.push([0.0, radii[1], 0.0]);
    normals.push([0.0, 1.0, 0.0]);
    for ring in 1..rings {
        let v = ring as f32 / rings as f32;
        let phi = std::f32::consts::PI * v;
        for segment in 0..=segments {
            let u = segment as f32 / segments as f32;
            // Close the UV seam with the exact same angular sample as the
            // first vertex.  Evaluating sin/cos(TAU) independently leaves a
            // tiny non-zero seam that can become a different weld bucket
            // after a primitive rotation, producing a false boundary edge
            // during strict GLB topology readback.
            let theta = if segment == segments {
                0.0
            } else {
                std::f32::consts::TAU * u
            };
            let unit = [phi.sin() * theta.cos(), phi.cos(), phi.sin() * theta.sin()];
            let normal = normalize([unit[0] / radii[0], unit[1] / radii[1], unit[2] / radii[2]]);
            positions.push([unit[0] * radii[0], unit[1] * radii[1], unit[2] * radii[2]]);
            normals.push(normal);
        }
    }
    let bottom = positions.len() as u32;
    positions.push([0.0, -radii[1], 0.0]);
    normals.push([0.0, -1.0, 0.0]);
    let stride = segments + 1;
    for segment in 0..segments {
        let first = 1 + segment as u32;
        indices.extend([0, first + 1, first]);
    }
    for ring in 0..(rings - 2) {
        for segment in 0..segments {
            let a = 1 + (ring * stride + segment) as u32;
            let b = a + 1;
            let c = 1 + ((ring + 1) * stride + segment) as u32;
            let d = c + 1;
            indices.extend([a, b, c, b, d, c]);
        }
    }
    let last_ring = 1 + ((rings - 2) * stride) as u32;
    for segment in 0..segments {
        indices.extend([
            last_ring + segment as u32,
            last_ring + segment as u32 + 1,
            bottom,
        ]);
    }
    (positions, normals, indices)
}

fn write_glb(
    parts: &[PartMesh],
    program_sha256: &str,
    triangle_count: u64,
    artifact_schema_version: &str,
    operator_catalog_sha256: Option<&str>,
) -> Result<Vec<u8>, GeometryError> {
    let mut binary = Vec::new();
    let mut buffer_views = Vec::new();
    let mut accessors = Vec::new();
    let mut meshes = Vec::new();
    let mut nodes = Vec::new();
    let mut materials = Vec::new();
    for (mesh_index, part) in parts.iter().enumerate() {
        let material_index = materials.len();
        materials.push(part.material.clone());
        let part_lineage = json!({
            "part_id":part.part_id,
            "material_zone_id":part.material_zone_id,
            "solid":part.solid,
        });
        let mut primitives = Vec::with_capacity(part.sources.len());
        for source in &part.sources {
            let positions_offset = append_f32_vec(&mut binary, &source.positions);
            let positions_length = source.positions.len() * 12;
            let normals_offset = append_f32_vec(&mut binary, &source.normals);
            let normals_length = source.normals.len() * 12;
            let uvs_offset = append_f32_vec2(&mut binary, &source.uvs);
            let uvs_length = source.uvs.len() * 8;
            let tangents_offset = append_f32_vec4(&mut binary, &source.tangents);
            let tangents_length = source.tangents.len() * 16;
            let indices_offset = append_u32_vec(&mut binary, &source.indices);
            let indices_length = source.indices.len() * 4;
            let pos_view = buffer_views.len();
            buffer_views.push(json!({"buffer":0,"byteOffset":positions_offset,"byteLength":positions_length,"target":34962}));
            let norm_view = buffer_views.len();
            buffer_views.push(json!({"buffer":0,"byteOffset":normals_offset,"byteLength":normals_length,"target":34962}));
            let uv_view = buffer_views.len();
            buffer_views.push(
                json!({"buffer":0,"byteOffset":uvs_offset,"byteLength":uvs_length,"target":34962}),
            );
            let tangent_view = buffer_views.len();
            buffer_views.push(json!({"buffer":0,"byteOffset":tangents_offset,"byteLength":tangents_length,"target":34962}));
            let index_view = buffer_views.len();
            buffer_views.push(json!({"buffer":0,"byteOffset":indices_offset,"byteLength":indices_length,"target":34963}));
            let (min, max) = bounds(&source.positions);
            let pos_accessor = accessors.len();
            accessors.push(json!({"bufferView":pos_view,"componentType":5126,"count":source.positions.len(),"type":"VEC3","min":min,"max":max}));
            let norm_accessor = accessors.len();
            accessors.push(json!({"bufferView":norm_view,"componentType":5126,"count":source.normals.len(),"type":"VEC3"}));
            let uv_accessor = accessors.len();
            accessors.push(json!({"bufferView":uv_view,"componentType":5126,"count":source.uvs.len(),"type":"VEC2","min":[0.0,0.0],"max":[1.0,1.0]}));
            let tangent_accessor = accessors.len();
            accessors.push(json!({"bufferView":tangent_view,"componentType":5126,"count":source.tangents.len(),"type":"VEC4"}));
            let index_accessor = accessors.len();
            accessors.push(json!({"bufferView":index_view,"componentType":5125,"count":source.indices.len(),"type":"SCALAR"}));
            primitives.push(json!({
                "attributes":{"POSITION":pos_accessor,"NORMAL":norm_accessor,"TEXCOORD_0":uv_accessor,"TANGENT":tangent_accessor},
                "indices":index_accessor,
                "material":material_index,
                "extras":{
                    "part_id":part.part_id,
                    "source_node_id":source.source_node_id,
                    "operator_id":source.operator_id,
                    "material_zone_id":part.material_zone_id,
                    "solid":part.solid,
                }
            }));
        }
        meshes.push(
            json!({"name":part.part_id,"primitives":primitives,"extras":part_lineage.clone()}),
        );
        nodes.push(json!({"name":part.part_id,"mesh":mesh_index,"extras":part_lineage}));
    }
    while binary.len() % 4 != 0 {
        binary.push(0);
    }
    let mut forgecad = json!({
        "schema_version":artifact_schema_version,
        "program_sha256":program_sha256,
        "triangle_count":triangle_count,
        "part_ids":ordered_unique_part_ids(parts),
        "source_node_ids":ordered_source_node_ids(parts),
        "material_zone_ids":ordered_unique_material_zone_ids(parts),
    });
    if let Some(operator_catalog_sha256) = operator_catalog_sha256 {
        forgecad["operator_catalog_sha256"] = Value::String(operator_catalog_sha256.to_owned());
        forgecad["part_bindings"] = Value::Array(
            parts
                .iter()
                .flat_map(|part| {
                    part.sources.iter().map(move |source| {
                        json!({
                            "part_id":part.part_id,
                            "source_node_id":source.source_node_id,
                            "material_zone_id":part.material_zone_id,
                            "solid":part.solid,
                            "triangle_count":source.indices.len() / 3,
                        })
                    })
                })
                .collect(),
        );
    }
    let root = json!({
        "asset":{"version":"2.0","generator":"ForgeCAD MCP010B bounded geometry compiler"},
        "scene":0,
        "scenes":[{"nodes":(0..nodes.len()).collect::<Vec<_>>() }],
        "nodes":nodes,
        "meshes":meshes,
        "materials":materials,
        "buffers":[{"byteLength":binary.len()}],
        "bufferViews":buffer_views,
        "accessors":accessors,
        "extras":{"forgecad":forgecad}
    });
    let mut json_bytes =
        serde_json::to_vec(&root).map_err(|error| GeometryError::Invalid(error.to_string()))?;
    while json_bytes.len() % 4 != 0 {
        json_bytes.push(b' ');
    }
    let total_length = 12 + 8 + json_bytes.len() + 8 + binary.len();
    let mut glb = Vec::with_capacity(total_length);
    glb.extend_from_slice(b"glTF");
    glb.extend_from_slice(&2u32.to_le_bytes());
    glb.extend_from_slice(&(total_length as u32).to_le_bytes());
    glb.extend_from_slice(&(json_bytes.len() as u32).to_le_bytes());
    glb.extend_from_slice(b"JSON");
    glb.extend_from_slice(&json_bytes);
    glb.extend_from_slice(&(binary.len() as u32).to_le_bytes());
    glb.extend_from_slice(b"BIN\0");
    glb.extend_from_slice(&binary);
    Ok(glb)
}

fn ordered_unique_part_ids(parts: &[PartMesh]) -> Vec<String> {
    let mut seen = HashSet::new();
    parts
        .iter()
        .filter_map(|part| {
            seen.insert(part.part_id.clone())
                .then(|| part.part_id.clone())
        })
        .collect()
}

fn ordered_source_node_ids(parts: &[PartMesh]) -> Vec<String> {
    parts
        .iter()
        .flat_map(|part| {
            part.sources
                .iter()
                .map(|source| source.source_node_id.clone())
        })
        .collect()
}

fn ordered_unique_material_zone_ids(parts: &[PartMesh]) -> Vec<String> {
    let mut seen = HashSet::new();
    parts
        .iter()
        .filter_map(|part| {
            seen.insert(part.material_zone_id.clone())
                .then(|| part.material_zone_id.clone())
        })
        .collect()
}

/// Render a deterministic, bounded orthographic preview directly from the
/// product-owned GLB. This is intentionally a small software renderer for
/// fixed evidence (not a second scene/state writer or a general DCC).
pub fn render_fixed_glb(glb: &[u8]) -> Result<Vec<RenderPass>, GeometryError> {
    let (root, binary) = parse_glb(glb)?;
    let meshes = root
        .get("meshes")
        .and_then(Value::as_array)
        .ok_or_else(|| GeometryError::Invalid("GLB meshes are missing".to_owned()))?;
    let accessors = root
        .get("accessors")
        .and_then(Value::as_array)
        .ok_or_else(|| GeometryError::Invalid("GLB accessors are missing".to_owned()))?;
    let views = root
        .get("bufferViews")
        .and_then(Value::as_array)
        .ok_or_else(|| GeometryError::Invalid("GLB bufferViews are missing".to_owned()))?;
    let mut vertices = Vec::<([f32; 3], [f32; 3], usize)>::new();
    let mut triangles = Vec::<([usize; 3], usize)>::new();
    for (mesh_index, mesh) in meshes.iter().enumerate() {
        let primitives = mesh
            .get("primitives")
            .and_then(Value::as_array)
            .ok_or_else(|| GeometryError::Invalid("GLB primitive list is missing".to_owned()))?;
        for primitive in primitives {
            let attributes = primitive
                .get("attributes")
                .and_then(Value::as_object)
                .ok_or_else(|| {
                    GeometryError::Invalid("GLB primitive attributes are missing".to_owned())
                })?;
            let position_accessor = attributes
                .get("POSITION")
                .and_then(Value::as_u64)
                .ok_or_else(|| {
                    GeometryError::Invalid("GLB POSITION accessor is missing".to_owned())
                })? as usize;
            let normal_accessor = attributes
                .get("NORMAL")
                .and_then(Value::as_u64)
                .map(|value| value as usize);
            let index_accessor = primitive
                .get("indices")
                .and_then(Value::as_u64)
                .ok_or_else(|| GeometryError::Invalid("GLB index accessor is missing".to_owned()))?
                as usize;
            let positions = read_vec3_accessor(accessors, views, &binary, position_accessor)?;
            let normals = normal_accessor
                .map(|index| read_vec3_accessor(accessors, views, &binary, index))
                .transpose()?
                .unwrap_or_else(|| vec![[0.0, 1.0, 0.0]; positions.len()]);
            if normals.len() != positions.len() {
                return Err(GeometryError::Invalid(
                    "GLB normal count does not match positions".to_owned(),
                ));
            }
            let base = vertices.len();
            vertices.extend(
                positions
                    .into_iter()
                    .zip(normals.into_iter())
                    .map(|(position, normal)| (position, normal, mesh_index)),
            );
            for indices in
                read_indices_accessor(accessors, views, &binary, index_accessor)?.chunks_exact(3)
            {
                triangles.push((
                    [
                        base + indices[0] as usize,
                        base + indices[1] as usize,
                        base + indices[2] as usize,
                    ],
                    mesh_index,
                ));
            }
        }
    }
    if vertices.is_empty() || triangles.is_empty() {
        return Err(GeometryError::Invalid(
            "GLB has no renderable triangles".to_owned(),
        ));
    }
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    for (position, _, _) in &vertices {
        for axis in 0..3 {
            min[axis] = min[axis].min(position[axis]);
            max[axis] = max[axis].max(position[axis]);
        }
    }
    let scale = ((max[0] - min[0]).max(max[1] - min[1])).max(0.0001);
    let width = 256u32;
    let height = 256u32;
    let mut passes = Vec::new();
    for pass in ["beauty", "silhouette", "normal", "part-id"] {
        let mut image = RgbaImage::from_pixel(width, height, Rgba([8, 12, 18, 255]));
        for (triangle, mesh_index) in &triangles {
            let projected =
                triangle.map(|index| project(vertices[index].0, min, scale, width, height));
            let color = match pass {
                "silhouette" => [236, 240, 244, 255],
                "normal" => normal_color(vertices[triangle[0]].1),
                "part-id" => part_color(*mesh_index),
                _ => material_color(&root, *mesh_index),
            };
            rasterize_triangle(&mut image, projected, color);
        }
        let mut bytes = Vec::new();
        image::DynamicImage::ImageRgba8(image)
            .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)
            .map_err(|error| {
                GeometryError::Invalid(format!("fixed render encode failed: {error}"))
            })?;
        passes.push(RenderPass {
            pass: pass.to_owned(),
            png: bytes,
            width,
            height,
        });
    }
    Ok(passes)
}

#[derive(Clone, Copy)]
struct PerspectiveVertex {
    screen_x: f32,
    screen_y: f32,
    depth: f32,
    world: [f32; 3],
    normal: [f32; 3],
    uv: [f32; 2],
}

#[derive(Clone, Copy)]
struct RasterHit {
    depth: f32,
    world: [f32; 3],
    normal: [f32; 3],
    mesh_index: usize,
    material_index: usize,
    edge: bool,
    uv_stretch: f32,
}

type Mat4 = [[f32; 4]; 4];

/// Render a self-contained GLB using the C-stage fixed camera contract. This
/// is deliberately a small deterministic software renderer: node transforms,
/// perspective projection, a depth buffer, fixed GGX-like direct lighting and
/// deterministic 2x supersampling are all product-owned and offline. It does
/// not accept shaders, scripts, URLs or material paths from the request.
pub fn render_perspective_glb(
    glb: &[u8],
    camera: &Value,
) -> Result<Vec<RenderPass>, GeometryError> {
    let (root, binary) = parse_glb(glb)?;
    let meshes = root
        .get("meshes")
        .and_then(Value::as_array)
        .ok_or_else(|| GeometryError::Invalid("GLB meshes are missing".to_owned()))?;
    let accessors = root
        .get("accessors")
        .and_then(Value::as_array)
        .ok_or_else(|| GeometryError::Invalid("GLB accessors are missing".to_owned()))?;
    let views = root
        .get("bufferViews")
        .and_then(Value::as_array)
        .ok_or_else(|| GeometryError::Invalid("GLB bufferViews are missing".to_owned()))?;
    let nodes = root
        .get("nodes")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let instances = scene_mesh_instances(&root, &nodes)?;
    if instances.is_empty() {
        return Err(GeometryError::Invalid(
            "GLB has no scene mesh instances".to_owned(),
        ));
    }
    let (camera_position, forward, right, up, fov_y, near, far) = parse_camera(camera)?;
    let width = 512u32;
    let height = 512u32;
    let sample_width = width * 2;
    let sample_height = height * 2;
    let mut hits = vec![None::<RasterHit>; (sample_width * sample_height) as usize];
    let focal = 1.0 / (fov_y.to_radians() * 0.5).tan();
    let aspect = width as f32 / height as f32;
    let mut rendered_triangles = 0usize;

    for (mesh_index, transform) in instances {
        let mesh = meshes
            .get(mesh_index)
            .and_then(Value::as_object)
            .ok_or_else(|| GeometryError::Invalid("GLB scene mesh is invalid".to_owned()))?;
        let primitives = mesh
            .get("primitives")
            .and_then(Value::as_array)
            .ok_or_else(|| GeometryError::Invalid("GLB primitive list is missing".to_owned()))?;
        for primitive in primitives {
            let attributes = primitive
                .get("attributes")
                .and_then(Value::as_object)
                .ok_or_else(|| {
                    GeometryError::Invalid("GLB primitive attributes are missing".to_owned())
                })?;
            let position_accessor = attributes
                .get("POSITION")
                .and_then(Value::as_u64)
                .ok_or_else(|| {
                    GeometryError::Invalid("GLB POSITION accessor is missing".to_owned())
                })? as usize;
            let normal_accessor = attributes
                .get("NORMAL")
                .and_then(Value::as_u64)
                .ok_or_else(|| {
                    GeometryError::Invalid("GLB NORMAL accessor is missing".to_owned())
                })? as usize;
            let uv_accessor = attributes
                .get("TEXCOORD_0")
                .and_then(Value::as_u64)
                .ok_or_else(|| {
                    GeometryError::Invalid("GLB TEXCOORD_0 accessor is missing".to_owned())
                })? as usize;
            let index_accessor = primitive
                .get("indices")
                .and_then(Value::as_u64)
                .ok_or_else(|| GeometryError::Invalid("GLB index accessor is missing".to_owned()))?
                as usize;
            let positions = read_vec3_accessor(accessors, views, &binary, position_accessor)?;
            let normals = read_vec3_accessor(accessors, views, &binary, normal_accessor)?;
            let uvs = read_vec2_accessor(accessors, views, &binary, uv_accessor)?;
            let indices = read_indices_accessor(accessors, views, &binary, index_accessor)?;
            if positions.len() != normals.len() || positions.len() != uvs.len() {
                return Err(GeometryError::Invalid(
                    "GLB render attributes have mismatched counts".to_owned(),
                ));
            }
            let material_index = primitive
                .get("material")
                .and_then(Value::as_u64)
                .unwrap_or(0) as usize;
            for index_triplet in indices.chunks_exact(3) {
                let mut projected = [PerspectiveVertex {
                    screen_x: 0.0,
                    screen_y: 0.0,
                    depth: 0.0,
                    world: [0.0; 3],
                    normal: [0.0, 1.0, 0.0],
                    uv: [0.0; 2],
                }; 3];
                let mut rejected = false;
                for (slot, source_index) in index_triplet.iter().enumerate() {
                    let source_index = *source_index as usize;
                    let world = transform_point(
                        transform,
                        positions.get(source_index).copied().ok_or_else(|| {
                            GeometryError::Invalid("GLB render index is invalid".to_owned())
                        })?,
                    );
                    let normal = normalize(transform_direction(transform, normals[source_index]));
                    let relative = subtract3(world, camera_position);
                    let z = dot3(relative, forward);
                    if !z.is_finite() || z <= near || z >= far {
                        rejected = true;
                        break;
                    }
                    let x = dot3(relative, right);
                    let y = dot3(relative, up);
                    let ndc_x = (x * focal / aspect) / z;
                    let ndc_y = (y * focal) / z;
                    projected[slot] = PerspectiveVertex {
                        screen_x: (ndc_x * 0.5 + 0.5) * sample_width as f32,
                        screen_y: (1.0 - (ndc_y * 0.5 + 0.5)) * sample_height as f32,
                        depth: (z - near) / (far - near),
                        world,
                        normal,
                        uv: uvs[source_index],
                    };
                }
                if rejected {
                    continue;
                }
                let area = edge2(projected[0], projected[1], projected[2]);
                if !area.is_finite() || area.abs() < 0.0001 {
                    continue;
                }
                rendered_triangles += 1;
                rasterize_perspective_triangle(
                    &mut hits,
                    sample_width,
                    sample_height,
                    projected,
                    area,
                    mesh_index,
                    material_index,
                );
            }
        }
    }
    if rendered_triangles == 0 {
        return Err(GeometryError::Invalid(
            "camera produced no visible triangles".to_owned(),
        ));
    }
    let mut passes = Vec::with_capacity(9);
    for pass in [
        "beauty",
        "silhouette",
        "depth",
        "normal",
        "ao",
        "part-id",
        "material-id",
        "wireframe",
        "uv-stretch",
    ] {
        let mut image = RgbaImage::from_pixel(sample_width, sample_height, Rgba([8, 12, 18, 255]));
        for y in 0..sample_height {
            for x in 0..sample_width {
                let index = (y * sample_width + x) as usize;
                let Some(hit) = hits[index] else { continue };
                let color = match pass {
                    "silhouette" => [236, 240, 244, 255],
                    "depth" => {
                        let value = ((1.0 - hit.depth.clamp(0.0, 1.0)) * 255.0) as u8;
                        [value, value, value, 255]
                    }
                    "normal" => normal_color(hit.normal),
                    "ao" => ao_color(&hits, sample_width, sample_height, x, y, hit),
                    "part-id" => part_color(hit.mesh_index),
                    "material-id" => material_color_id(hit.material_index),
                    "wireframe" => {
                        if hit.edge {
                            [250, 250, 250, 255]
                        } else {
                            [8, 12, 18, 255]
                        }
                    }
                    "uv-stretch" => uv_stretch_color(hit.uv_stretch),
                    _ => shade_material(
                        &root,
                        hit.material_index,
                        hit.normal,
                        hit.world,
                        camera_position,
                    ),
                };
                image.put_pixel(x, y, Rgba(color));
            }
        }
        let filter = if matches!(pass, "part-id" | "material-id" | "depth" | "silhouette") {
            imageops::FilterType::Nearest
        } else {
            imageops::FilterType::Triangle
        };
        let image = imageops::resize(&image, width, height, filter);
        let mut bytes = Vec::new();
        image::DynamicImage::ImageRgba8(image)
            .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)
            .map_err(|error| {
                GeometryError::Invalid(format!("fixed render encode failed: {error}"))
            })?;
        passes.push(RenderPass {
            pass: pass.to_owned(),
            png: bytes,
            width,
            height,
        });
    }
    Ok(passes)
}

fn parse_camera(
    camera: &Value,
) -> Result<([f32; 3], [f32; 3], [f32; 3], [f32; 3], f32, f32, f32), GeometryError> {
    let object = camera
        .as_object()
        .ok_or_else(|| GeometryError::Invalid("camera must be an object".to_owned()))?;
    if object.get("schema_version").and_then(Value::as_str) != Some("CameraCalibration@1")
        || object.get("projection").and_then(Value::as_str) != Some("perspective")
        || object.get("coordinate_system").and_then(Value::as_str)
            != Some("right-handed-y-up-meter")
        || object
            .get("resolution")
            .and_then(|value| value.get("width"))
            .and_then(Value::as_u64)
            != Some(512)
        || object
            .get("resolution")
            .and_then(|value| value.get("height"))
            .and_then(Value::as_u64)
            != Some(512)
    {
        return Err(GeometryError::Invalid(
            "CameraCalibration@1 is not the fixed perspective contract".to_owned(),
        ));
    }
    let transform = object
        .get("transform")
        .and_then(Value::as_object)
        .ok_or_else(|| GeometryError::Invalid("camera transform is missing".to_owned()))?;
    let position = required_vec3(transform.get("position_m"), "camera position")?;
    let target = required_vec3(transform.get("target_m"), "camera target")?;
    let up_input = required_vec3(transform.get("up"), "camera up")?;
    let forward = normalize(subtract3(target, position));
    let right = normalize(cross3(forward, up_input));
    let up = normalize(cross3(right, forward));
    if length3(forward) <= f32::EPSILON
        || length3(right) <= f32::EPSILON
        || length3(up) <= f32::EPSILON
    {
        return Err(GeometryError::Invalid(
            "camera basis is degenerate".to_owned(),
        ));
    }
    let fov_y = object
        .get("fov_y_degrees")
        .and_then(Value::as_f64)
        .ok_or_else(|| GeometryError::Invalid("camera fov is missing".to_owned()))?
        as f32;
    let near = object
        .get("near_m")
        .and_then(Value::as_f64)
        .ok_or_else(|| GeometryError::Invalid("camera near is missing".to_owned()))?
        as f32;
    let far = object
        .get("far_m")
        .and_then(Value::as_f64)
        .ok_or_else(|| GeometryError::Invalid("camera far is missing".to_owned()))?
        as f32;
    if !(fov_y > 1.0 && fov_y < 179.0 && near > 0.0 && far > near) {
        return Err(GeometryError::Invalid(
            "camera perspective limits are invalid".to_owned(),
        ));
    }
    Ok((position, forward, right, up, fov_y, near, far))
}

fn required_vec3(value: Option<&Value>, label: &str) -> Result<[f32; 3], GeometryError> {
    let values = value
        .and_then(Value::as_array)
        .ok_or_else(|| GeometryError::Invalid(format!("{label} is missing")))?;
    if values.len() != 3 {
        return Err(GeometryError::Invalid(format!(
            "{label} must have three values"
        )));
    }
    let result = [
        values[0]
            .as_f64()
            .ok_or_else(|| GeometryError::Invalid(format!("{label} is invalid")))? as f32,
        values[1]
            .as_f64()
            .ok_or_else(|| GeometryError::Invalid(format!("{label} is invalid")))? as f32,
        values[2]
            .as_f64()
            .ok_or_else(|| GeometryError::Invalid(format!("{label} is invalid")))? as f32,
    ];
    if result.iter().any(|value| !value.is_finite()) {
        return Err(GeometryError::Invalid(format!("{label} is non-finite")));
    }
    Ok(result)
}

fn scene_mesh_instances(
    root: &Value,
    nodes: &[Value],
) -> Result<Vec<(usize, Mat4)>, GeometryError> {
    if nodes.is_empty() {
        return Err(GeometryError::Invalid("GLB nodes are missing".to_owned()));
    }
    let mut instances = Vec::new();
    let mut visited = HashSet::new();
    let scene_index = root.get("scene").and_then(Value::as_u64).unwrap_or(0) as usize;
    let roots = root
        .get("scenes")
        .and_then(Value::as_array)
        .and_then(|scenes| scenes.get(scene_index))
        .and_then(|scene| scene.get("nodes"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_else(|| {
            (0..nodes.len())
                .map(|index| Value::from(index as u64))
                .collect()
        });
    for root_node in roots {
        let node_index = root_node
            .as_u64()
            .ok_or_else(|| GeometryError::Invalid("scene node index is invalid".to_owned()))?
            as usize;
        collect_node_instances(nodes, node_index, identity4(), &mut visited, &mut instances)?;
    }
    Ok(instances)
}

fn collect_node_instances(
    nodes: &[Value],
    index: usize,
    parent: Mat4,
    visited: &mut HashSet<usize>,
    instances: &mut Vec<(usize, Mat4)>,
) -> Result<(), GeometryError> {
    if !visited.insert(index) {
        return Err(GeometryError::Invalid(
            "GLB node graph contains a cycle or duplicate instance".to_owned(),
        ));
    }
    let node = nodes
        .get(index)
        .and_then(Value::as_object)
        .ok_or_else(|| GeometryError::Invalid("GLB node is invalid".to_owned()))?;
    let transform = mat4_mul(parent, node_transform(node)?);
    if let Some(mesh) = node.get("mesh").and_then(Value::as_u64) {
        instances.push((mesh as usize, transform));
    }
    if let Some(children) = node.get("children").and_then(Value::as_array) {
        for child in children {
            collect_node_instances(
                nodes,
                child.as_u64().ok_or_else(|| {
                    GeometryError::Invalid("GLB child index is invalid".to_owned())
                })? as usize,
                transform,
                visited,
                instances,
            )?;
        }
    }
    Ok(())
}

fn identity4() -> Mat4 {
    [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]
}
fn mat4_mul(a: Mat4, b: Mat4) -> Mat4 {
    let mut out = [[0.0; 4]; 4];
    for row in 0..4 {
        for col in 0..4 {
            out[col][row] = (0..4).map(|k| a[k][row] * b[col][k]).sum();
        }
    }
    out
}
fn node_transform(node: &Map<String, Value>) -> Result<Mat4, GeometryError> {
    if let Some(matrix) = node.get("matrix").and_then(Value::as_array) {
        if matrix.len() != 16 {
            return Err(GeometryError::Invalid(
                "GLB node matrix is invalid".to_owned(),
            ));
        }
        let mut result = [[0.0; 4]; 4];
        for (index, value) in matrix.iter().enumerate() {
            result[index % 4][index / 4] = value.as_f64().ok_or_else(|| {
                GeometryError::Invalid("GLB node matrix is non-numeric".to_owned())
            })? as f32;
        }
        return Ok(result);
    }
    let translation = node
        .get("translation")
        .map(|value| required_vec3(Some(value), "node translation"))
        .transpose()?
        .unwrap_or([0.0; 3]);
    let scale = node
        .get("scale")
        .map(|value| required_vec3(Some(value), "node scale"))
        .transpose()?
        .unwrap_or([1.0; 3]);
    let rotation =
        node.get("rotation")
            .and_then(Value::as_array)
            .map(|values| {
                if values.len() != 4 {
                    return Err(GeometryError::Invalid(
                        "node rotation is invalid".to_owned(),
                    ));
                }
                Ok([
                    values[0].as_f64().ok_or_else(|| {
                        GeometryError::Invalid("node rotation is invalid".to_owned())
                    })? as f32,
                    values[1].as_f64().ok_or_else(|| {
                        GeometryError::Invalid("node rotation is invalid".to_owned())
                    })? as f32,
                    values[2].as_f64().ok_or_else(|| {
                        GeometryError::Invalid("node rotation is invalid".to_owned())
                    })? as f32,
                    values[3].as_f64().ok_or_else(|| {
                        GeometryError::Invalid("node rotation is invalid".to_owned())
                    })? as f32,
                ])
            })
            .transpose()?
            .unwrap_or([0.0, 0.0, 0.0, 1.0]);
    let [x, y, z, w] = rotation;
    let xx = x * x;
    let yy = y * y;
    let zz = z * z;
    Ok([
        [
            (1.0 - 2.0 * (yy + zz)) * scale[0],
            (2.0 * (x * y + w * z)) * scale[1],
            (2.0 * (x * z - w * y)) * scale[2],
            translation[0],
        ],
        [
            (2.0 * (x * y - w * z)) * scale[0],
            (1.0 - 2.0 * (xx + zz)) * scale[1],
            (2.0 * (y * z + w * x)) * scale[2],
            translation[1],
        ],
        [
            (2.0 * (x * z + w * y)) * scale[0],
            (2.0 * (y * z - w * x)) * scale[1],
            (1.0 - 2.0 * (xx + yy)) * scale[2],
            translation[2],
        ],
        [0.0, 0.0, 0.0, 1.0],
    ])
}
fn transform_point(matrix: Mat4, point: [f32; 3]) -> [f32; 3] {
    [
        matrix[0][0] * point[0] + matrix[1][0] * point[1] + matrix[2][0] * point[2] + matrix[3][0],
        matrix[0][1] * point[0] + matrix[1][1] * point[1] + matrix[2][1] * point[2] + matrix[3][1],
        matrix[0][2] * point[0] + matrix[1][2] * point[1] + matrix[2][2] * point[2] + matrix[3][2],
    ]
}
fn transform_direction(matrix: Mat4, direction: [f32; 3]) -> [f32; 3] {
    [
        matrix[0][0] * direction[0] + matrix[1][0] * direction[1] + matrix[2][0] * direction[2],
        matrix[0][1] * direction[0] + matrix[1][1] * direction[1] + matrix[2][1] * direction[2],
        matrix[0][2] * direction[0] + matrix[1][2] * direction[1] + matrix[2][2] * direction[2],
    ]
}
fn edge2(a: PerspectiveVertex, b: PerspectiveVertex, c: PerspectiveVertex) -> f32 {
    (b.screen_x - a.screen_x) * (c.screen_y - a.screen_y)
        - (b.screen_y - a.screen_y) * (c.screen_x - a.screen_x)
}
fn rasterize_perspective_triangle(
    hits: &mut [Option<RasterHit>],
    width: u32,
    height: u32,
    vertices: [PerspectiveVertex; 3],
    area: f32,
    mesh_index: usize,
    material_index: usize,
) {
    let min_x = vertices
        .iter()
        .map(|vertex| vertex.screen_x)
        .fold(f32::INFINITY, f32::min)
        .floor()
        .max(0.0) as u32;
    let max_x = vertices
        .iter()
        .map(|vertex| vertex.screen_x)
        .fold(f32::NEG_INFINITY, f32::max)
        .ceil()
        .min(width as f32 - 1.0) as u32;
    let min_y = vertices
        .iter()
        .map(|vertex| vertex.screen_y)
        .fold(f32::INFINITY, f32::min)
        .floor()
        .max(0.0) as u32;
    let max_y = vertices
        .iter()
        .map(|vertex| vertex.screen_y)
        .fold(f32::NEG_INFINITY, f32::max)
        .ceil()
        .min(height as f32 - 1.0) as u32;
    let uv_area = (vertices[1].uv[0] - vertices[0].uv[0]) * (vertices[2].uv[1] - vertices[0].uv[1])
        - (vertices[1].uv[1] - vertices[0].uv[1]) * (vertices[2].uv[0] - vertices[0].uv[0]);
    let stretch = ((area.abs() / uv_area.abs().max(0.000001)).sqrt() / 64.0).clamp(0.0, 64.0);
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let p = PerspectiveVertex {
                screen_x: x as f32 + 0.5,
                screen_y: y as f32 + 0.5,
                depth: 0.0,
                world: [0.0; 3],
                normal: [0.0; 3],
                uv: [0.0; 2],
            };
            let w0 = edge2(vertices[1], vertices[2], p) / area;
            let w1 = edge2(vertices[2], vertices[0], p) / area;
            let w2 = edge2(vertices[0], vertices[1], p) / area;
            if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                continue;
            }
            let depth = w0 * vertices[0].depth + w1 * vertices[1].depth + w2 * vertices[2].depth;
            let idx = (y * width + x) as usize;
            if hits[idx].is_some_and(|old| depth >= old.depth) {
                continue;
            }
            hits[idx] = Some(RasterHit {
                depth,
                world: [
                    w0 * vertices[0].world[0]
                        + w1 * vertices[1].world[0]
                        + w2 * vertices[2].world[0],
                    w0 * vertices[0].world[1]
                        + w1 * vertices[1].world[1]
                        + w2 * vertices[2].world[1],
                    w0 * vertices[0].world[2]
                        + w1 * vertices[1].world[2]
                        + w2 * vertices[2].world[2],
                ],
                normal: normalize([
                    w0 * vertices[0].normal[0]
                        + w1 * vertices[1].normal[0]
                        + w2 * vertices[2].normal[0],
                    w0 * vertices[0].normal[1]
                        + w1 * vertices[1].normal[1]
                        + w2 * vertices[2].normal[1],
                    w0 * vertices[0].normal[2]
                        + w1 * vertices[1].normal[2]
                        + w2 * vertices[2].normal[2],
                ]),
                mesh_index,
                material_index,
                edge: w0.min(w1).min(w2) < 0.025,
                uv_stretch: stretch,
            });
        }
    }
}
fn ao_color(
    hits: &[Option<RasterHit>],
    width: u32,
    height: u32,
    x: u32,
    y: u32,
    hit: RasterHit,
) -> [u8; 4] {
    let mut samples = 0;
    let mut occluded = 0;
    for dy in -2i32..=2 {
        for dx in -2i32..=2 {
            if dx == 0 && dy == 0 {
                continue;
            }
            let nx = x as i32 + dx;
            let ny = y as i32 + dy;
            if nx < 0 || ny < 0 || nx >= width as i32 || ny >= height as i32 {
                continue;
            }
            if let Some(other) = hits[(ny as u32 * width + nx as u32) as usize] {
                samples += 1;
                if other.depth > hit.depth + 0.002 {
                    occluded += 1;
                }
            }
        }
    }
    let factor = 1.0 - 0.65 * (occluded as f32 / (samples.max(1) as f32));
    let value = (factor * 255.0) as u8;
    [value, value, value, 255]
}
fn material_color_id(index: usize) -> [u8; 4] {
    [
        ((index.wrapping_mul(83) + 37) % 220 + 20) as u8,
        ((index.wrapping_mul(47) + 91) % 170 + 40) as u8,
        ((index.wrapping_mul(19) + 151) % 120 + 80) as u8,
        255,
    ]
}
fn uv_stretch_color(value: f32) -> [u8; 4] {
    let t = (value.max(0.0).ln_1p() / 4.0).clamp(0.0, 1.0);
    [
        (255.0 * t) as u8,
        (255.0 * (1.0 - (t - 0.5).abs() * 2.0)) as u8,
        (255.0 * (1.0 - t)) as u8,
        255,
    ]
}
fn shade_material(
    root: &Value,
    material_index: usize,
    normal: [f32; 3],
    world: [f32; 3],
    camera: [f32; 3],
) -> [u8; 4] {
    let (base, metallic, roughness, emissive) = material_parameters(root, material_index);
    let n = normalize(normal);
    let light = normalize([0.45, 0.8, 0.6]);
    let view = normalize(subtract3(camera, world));
    let half = normalize(add3(light, view));
    let ndotl = dot3(n, light).max(0.0);
    let ndotv = dot3(n, view).max(0.0);
    let ndoth = dot3(n, half).max(0.0);
    let vdoth = dot3(view, half).max(0.0);
    let alpha = roughness.max(0.04).powi(2);
    let d = (alpha * alpha)
        / (std::f32::consts::PI * ((ndoth * ndoth * (alpha * alpha - 1.0) + 1.0).powi(2)));
    let k = ((roughness + 1.0) * (roughness + 1.0)) / 8.0;
    let g1 = ndotv / (ndotv * (1.0 - k) + k);
    let g2 = ndotl / (ndotl * (1.0 - k) + k);
    let f0 = [
        0.04 * (1.0 - metallic) + base[0] * metallic,
        0.04 * (1.0 - metallic) + base[1] * metallic,
        0.04 * (1.0 - metallic) + base[2] * metallic,
    ];
    let fresnel = 1.0 - vdoth;
    let fresnel = fresnel.powi(5);
    let mut color = [0.0; 3];
    for i in 0..3 {
        let spec = (d * g1 * g2 * f0[i] * (1.0 - fresnel) + f0[i] * fresnel) * ndotl;
        color[i] = base[i] * (1.0 - metallic) * ndotl * 0.85 + spec + base[i] * 0.08 + emissive[i];
    }
    [
        linear_to_srgb(color[0]),
        linear_to_srgb(color[1]),
        linear_to_srgb(color[2]),
        255,
    ]
}
fn material_parameters(root: &Value, index: usize) -> ([f32; 3], f32, f32, [f32; 3]) {
    let material = root
        .get("materials")
        .and_then(Value::as_array)
        .and_then(|items| items.get(index));
    let pbr = material.and_then(|item| item.get("pbrMetallicRoughness"));
    let factor = pbr
        .and_then(|item| item.get("baseColorFactor"))
        .and_then(Value::as_array);
    let base = [
        factor
            .and_then(|v| v.first())
            .and_then(Value::as_f64)
            .unwrap_or(0.6) as f32,
        factor
            .and_then(|v| v.get(1))
            .and_then(Value::as_f64)
            .unwrap_or(0.65) as f32,
        factor
            .and_then(|v| v.get(2))
            .and_then(Value::as_f64)
            .unwrap_or(0.7) as f32,
    ];
    let metallic = pbr
        .and_then(|item| item.get("metallicFactor"))
        .and_then(Value::as_f64)
        .unwrap_or(0.0) as f32;
    let roughness = pbr
        .and_then(|item| item.get("roughnessFactor"))
        .and_then(Value::as_f64)
        .unwrap_or(0.5) as f32;
    let emissive = material
        .and_then(|item| item.get("emissiveFactor"))
        .and_then(Value::as_array)
        .map(|v| {
            [
                v.first().and_then(Value::as_f64).unwrap_or(0.0) as f32,
                v.get(1).and_then(Value::as_f64).unwrap_or(0.0) as f32,
                v.get(2).and_then(Value::as_f64).unwrap_or(0.0) as f32,
            ]
        })
        .unwrap_or([0.0; 3]);
    (
        base,
        metallic.clamp(0.0, 1.0),
        roughness.clamp(0.04, 1.0),
        emissive,
    )
}
fn linear_to_srgb(value: f32) -> u8 {
    let value = value.max(0.0);
    let encoded = if value <= 0.0031308 {
        12.92 * value
    } else {
        1.055 * value.powf(1.0 / 2.4) - 0.055
    };
    (encoded.clamp(0.0, 1.0) * 255.0) as u8
}

fn parse_glb(glb: &[u8]) -> Result<(Value, Vec<u8>), GeometryError> {
    if glb.len() < 20
        || &glb[..4] != b"glTF"
        || u32::from_le_bytes(glb[4..8].try_into().unwrap()) != 2
    {
        return Err(GeometryError::Invalid("GLB header is invalid".to_owned()));
    }
    let total = u32::from_le_bytes(glb[8..12].try_into().unwrap()) as usize;
    if total != glb.len() {
        return Err(GeometryError::Invalid("GLB length is invalid".to_owned()));
    }
    let json_len = u32::from_le_bytes(glb[12..16].try_into().unwrap()) as usize;
    if &glb[16..20] != b"JSON" || 20 + json_len + 8 > glb.len() {
        return Err(GeometryError::Invalid(
            "GLB JSON chunk is invalid".to_owned(),
        ));
    }
    let root = serde_json::from_slice(&glb[20..20 + json_len])
        .map_err(|error| GeometryError::Invalid(error.to_string()))?;
    let binary_offset = 20 + json_len;
    let binary_len =
        u32::from_le_bytes(glb[binary_offset..binary_offset + 4].try_into().unwrap()) as usize;
    if &glb[binary_offset + 4..binary_offset + 8] != b"BIN\0"
        || binary_offset + 8 + binary_len != glb.len()
    {
        return Err(GeometryError::Invalid(
            "GLB BIN chunk is invalid".to_owned(),
        ));
    }
    Ok((root, glb[binary_offset + 8..].to_vec()))
}

fn accessor_view<'a>(
    accessors: &'a [Value],
    views: &'a [Value],
    index: usize,
) -> Result<(&'a Value, &'a Value), GeometryError> {
    let accessor = accessors
        .get(index)
        .ok_or_else(|| GeometryError::Invalid("GLB accessor index is invalid".to_owned()))?;
    let view_index = accessor
        .get("bufferView")
        .and_then(Value::as_u64)
        .ok_or_else(|| GeometryError::Invalid("GLB accessor bufferView is missing".to_owned()))?
        as usize;
    let view = views
        .get(view_index)
        .ok_or_else(|| GeometryError::Invalid("GLB bufferView index is invalid".to_owned()))?;
    Ok((accessor, view))
}

fn read_vec3_accessor(
    accessors: &[Value],
    views: &[Value],
    binary: &[u8],
    index: usize,
) -> Result<Vec<[f32; 3]>, GeometryError> {
    let (accessor, view) = accessor_view(accessors, views, index)?;
    if accessor.get("componentType").and_then(Value::as_u64) != Some(5126)
        || accessor.get("type").and_then(Value::as_str) != Some("VEC3")
    {
        return Err(GeometryError::Invalid(
            "GLB VEC3 accessor is not float".to_owned(),
        ));
    }
    let count = accessor
        .get("count")
        .and_then(Value::as_u64)
        .ok_or_else(|| GeometryError::Invalid("GLB accessor count is missing".to_owned()))?
        as usize;
    let offset = view.get("byteOffset").and_then(Value::as_u64).unwrap_or(0) as usize
        + accessor
            .get("byteOffset")
            .and_then(Value::as_u64)
            .unwrap_or(0) as usize;
    if offset + count.saturating_mul(12) > binary.len() {
        return Err(GeometryError::Invalid(
            "GLB VEC3 accessor exceeds BIN".to_owned(),
        ));
    }
    let mut values = Vec::with_capacity(count);
    for chunk in binary[offset..offset + count * 12].chunks_exact(12) {
        values.push([
            f32::from_le_bytes(chunk[0..4].try_into().unwrap()),
            f32::from_le_bytes(chunk[4..8].try_into().unwrap()),
            f32::from_le_bytes(chunk[8..12].try_into().unwrap()),
        ]);
    }
    Ok(values)
}

fn read_indices_accessor(
    accessors: &[Value],
    views: &[Value],
    binary: &[u8],
    index: usize,
) -> Result<Vec<u32>, GeometryError> {
    let (accessor, view) = accessor_view(accessors, views, index)?;
    if accessor.get("componentType").and_then(Value::as_u64) != Some(5125)
        || accessor.get("type").and_then(Value::as_str) != Some("SCALAR")
    {
        return Err(GeometryError::Invalid(
            "GLB index accessor is not uint32".to_owned(),
        ));
    }
    let count = accessor
        .get("count")
        .and_then(Value::as_u64)
        .ok_or_else(|| GeometryError::Invalid("GLB index count is missing".to_owned()))?
        as usize;
    let offset = view.get("byteOffset").and_then(Value::as_u64).unwrap_or(0) as usize
        + accessor
            .get("byteOffset")
            .and_then(Value::as_u64)
            .unwrap_or(0) as usize;
    if offset + count.saturating_mul(4) > binary.len() {
        return Err(GeometryError::Invalid(
            "GLB index accessor exceeds BIN".to_owned(),
        ));
    }
    Ok(binary[offset..offset + count * 4]
        .chunks_exact(4)
        .map(|chunk| u32::from_le_bytes(chunk.try_into().unwrap()))
        .collect())
}

fn read_vec2_accessor(
    accessors: &[Value],
    views: &[Value],
    binary: &[u8],
    index: usize,
) -> Result<Vec<[f32; 2]>, GeometryError> {
    let (accessor, view) = accessor_view(accessors, views, index)?;
    if accessor.get("componentType").and_then(Value::as_u64) != Some(5126)
        || accessor.get("type").and_then(Value::as_str) != Some("VEC2")
    {
        return Err(GeometryError::Invalid(
            "GLB VEC2 accessor is not float".to_owned(),
        ));
    }
    let count = accessor
        .get("count")
        .and_then(Value::as_u64)
        .ok_or_else(|| GeometryError::Invalid("GLB VEC2 accessor count is missing".to_owned()))?
        as usize;
    let offset = view.get("byteOffset").and_then(Value::as_u64).unwrap_or(0) as usize
        + accessor
            .get("byteOffset")
            .and_then(Value::as_u64)
            .unwrap_or(0) as usize;
    if offset + count.saturating_mul(8) > binary.len() {
        return Err(GeometryError::Invalid(
            "GLB VEC2 accessor exceeds BIN".to_owned(),
        ));
    }
    Ok(binary[offset..offset + count * 8]
        .chunks_exact(8)
        .map(|chunk| {
            [
                f32::from_le_bytes(chunk[0..4].try_into().unwrap()),
                f32::from_le_bytes(chunk[4..8].try_into().unwrap()),
            ]
        })
        .collect())
}

fn project(position: [f32; 3], min: [f32; 3], scale: f32, width: u32, height: u32) -> [f32; 2] {
    let margin = 18.0;
    let x = margin + (position[0] - min[0]) / scale * (width as f32 - 2.0 * margin);
    let y =
        height as f32 - margin - (position[1] - min[1]) / scale * (height as f32 - 2.0 * margin);
    [x, y]
}

fn rasterize_triangle(image: &mut RgbaImage, points: [[f32; 2]; 3], color: [u8; 4]) {
    let min_x = points
        .iter()
        .map(|point| point[0])
        .fold(f32::INFINITY, f32::min)
        .floor()
        .max(0.0) as u32;
    let max_x = points
        .iter()
        .map(|point| point[0])
        .fold(f32::NEG_INFINITY, f32::max)
        .ceil()
        .min(image.width() as f32 - 1.0) as u32;
    let min_y = points
        .iter()
        .map(|point| point[1])
        .fold(f32::INFINITY, f32::min)
        .floor()
        .max(0.0) as u32;
    let max_y = points
        .iter()
        .map(|point| point[1])
        .fold(f32::NEG_INFINITY, f32::max)
        .ceil()
        .min(image.height() as f32 - 1.0) as u32;
    let area = edge(points[0], points[1], points[2]);
    if area.abs() < f32::EPSILON {
        return;
    }
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let point = [x as f32 + 0.5, y as f32 + 0.5];
            let w0 = edge(points[1], points[2], point);
            let w1 = edge(points[2], points[0], point);
            let w2 = edge(points[0], points[1], point);
            if (w0 >= 0.0 && w1 >= 0.0 && w2 >= 0.0) || (w0 <= 0.0 && w1 <= 0.0 && w2 <= 0.0) {
                image.put_pixel(x, y, Rgba(color));
            }
        }
    }
}

fn edge(a: [f32; 2], b: [f32; 2], c: [f32; 2]) -> f32 {
    (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
}
fn normal_color(normal: [f32; 3]) -> [u8; 4] {
    [
        ((normal[0] * 0.5 + 0.5) * 255.0) as u8,
        ((normal[1] * 0.5 + 0.5) * 255.0) as u8,
        ((normal[2] * 0.5 + 0.5) * 255.0) as u8,
        255,
    ]
}
fn part_color(index: usize) -> [u8; 4] {
    [
        ((index.wrapping_mul(97) + 53) % 220 + 20) as u8,
        ((index.wrapping_mul(53) + 79) % 170 + 40) as u8,
        ((index.wrapping_mul(31) + 131) % 120 + 80) as u8,
        255,
    ]
}
fn material_color(root: &Value, mesh_index: usize) -> [u8; 4] {
    let material_index = root
        .get("meshes")
        .and_then(Value::as_array)
        .and_then(|meshes| meshes.get(mesh_index))
        .and_then(|mesh| mesh.get("primitives"))
        .and_then(Value::as_array)
        .and_then(|primitives| primitives.first())
        .and_then(|primitive| primitive.get("material"))
        .and_then(Value::as_u64)
        .unwrap_or(0) as usize;
    let factor = root
        .get("materials")
        .and_then(Value::as_array)
        .and_then(|materials| materials.get(material_index))
        .and_then(|material| material.get("pbrMetallicRoughness"))
        .and_then(|pbr| pbr.get("baseColorFactor"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_else(|| {
            vec![
                Value::from(0.6),
                Value::from(0.65),
                Value::from(0.7),
                Value::from(1.0),
            ]
        });
    [
        (factor
            .first()
            .and_then(Value::as_f64)
            .unwrap_or(0.6)
            .clamp(0.0, 1.0)
            * 255.0) as u8,
        (factor
            .get(1)
            .and_then(Value::as_f64)
            .unwrap_or(0.65)
            .clamp(0.0, 1.0)
            * 255.0) as u8,
        (factor
            .get(2)
            .and_then(Value::as_f64)
            .unwrap_or(0.7)
            .clamp(0.0, 1.0)
            * 255.0) as u8,
        255,
    ]
}

fn append_f32_vec(binary: &mut Vec<u8>, values: &[[f32; 3]]) -> usize {
    while binary.len() % 4 != 0 {
        binary.push(0);
    }
    let offset = binary.len();
    for value in values {
        for component in value {
            binary.extend_from_slice(&component.to_le_bytes());
        }
    }
    offset
}

fn append_u32_vec(binary: &mut Vec<u8>, values: &[u32]) -> usize {
    while binary.len() % 4 != 0 {
        binary.push(0);
    }
    let offset = binary.len();
    for value in values {
        binary.extend_from_slice(&value.to_le_bytes());
    }
    offset
}

fn append_f32_vec2(binary: &mut Vec<u8>, values: &[[f32; 2]]) -> usize {
    while binary.len() % 4 != 0 {
        binary.push(0);
    }
    let offset = binary.len();
    for value in values {
        for component in value {
            binary.extend_from_slice(&component.to_le_bytes());
        }
    }
    offset
}

fn append_f32_vec4(binary: &mut Vec<u8>, values: &[[f32; 4]]) -> usize {
    while binary.len() % 4 != 0 {
        binary.push(0);
    }
    let offset = binary.len();
    for value in values {
        for component in value {
            binary.extend_from_slice(&component.to_le_bytes());
        }
    }
    offset
}

fn bounds(values: &[[f32; 3]]) -> ([f32; 3], [f32; 3]) {
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    for value in values {
        for index in 0..3 {
            min[index] = min[index].min(value[index]);
            max[index] = max[index].max(value[index]);
        }
    }
    (min, max)
}

fn material_for_zone(zone: &str) -> Value {
    let (base, metallic, roughness, emissive) =
        if zone.contains("mechanical") || zone.contains("black") {
            ([0.035, 0.045, 0.055, 1.0], 0.75, 0.3, [0.0, 0.0, 0.0])
        } else if zone.contains("emissive") || zone.contains("amber") {
            ([0.16, 0.06, 0.01, 1.0], 0.2, 0.25, [1.0, 0.12, 0.01])
        } else {
            ([0.76, 0.8, 0.84, 1.0], 0.7, 0.26, [0.0, 0.0, 0.0])
        };
    json!({"name":zone,"pbrMetallicRoughness":{"baseColorFactor":base,"metallicFactor":metallic,"roughnessFactor":roughness},"emissiveFactor":emissive})
}

fn canonical_hash(value: &Value) -> String {
    let mut bytes = Vec::new();
    write_canonical(value, &mut bytes);
    let mut digest = Sha256::new();
    digest.update(bytes);
    digest
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn write_canonical(value: &Value, output: &mut Vec<u8>) {
    match value {
        Value::Null => output.extend_from_slice(b"null"),
        Value::Bool(value) => output.extend_from_slice(if *value { b"true" } else { b"false" }),
        Value::Number(value) => output.extend_from_slice(value.to_string().as_bytes()),
        Value::String(value) => {
            serde_json::to_writer(&mut *output, value).expect("string serializes")
        }
        Value::Array(values) => {
            output.push(b'[');
            for (index, value) in values.iter().enumerate() {
                if index != 0 {
                    output.push(b',');
                }
                write_canonical(value, output);
            }
            output.push(b']');
        }
        Value::Object(values) => {
            let mut keys = values.keys().collect::<Vec<_>>();
            keys.sort_unstable();
            output.push(b'{');
            for (index, key) in keys.iter().enumerate() {
                if index != 0 {
                    output.push(b',');
                }
                serde_json::to_writer(&mut *output, key).expect("key serializes");
                output.push(b':');
                write_canonical(&values[*key], output);
            }
            output.push(b'}');
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn program() -> Value {
        let mut program = json!({
            "schema_version":"GeometryProgram@1",
            "project_id":"project-test",
            "representation_plan_sha256":"a".repeat(64),
            "nodes":[
                {"node_id":"torso","operator_id":"forgecad.geometry.primitive@1","part_id":"torso","parameters":{"shape":"box","size":[1.2,1.6,0.55],"position":[0.0,1.7,0.0],"material_zone_id":"zone-white-shell"}},
                {"node_id":"core","operator_id":"forgecad.geometry.primitive@1","part_id":"core","parameters":{"shape":"cylinder","size":[0.55,1.2,0.55],"position":[0.0,1.5,0.0],"material_zone_id":"zone-black-mechanical"}}
            ],
            "budgets":{"max_nodes":16,"max_triangles":10000,"max_runtime_ms":1000}
        });
        let hash = canonical_hash(&program);
        program
            .as_object_mut()
            .unwrap()
            .insert("canonical_sha256".to_owned(), Value::String(hash));
        program
    }

    #[test]
    fn deterministic_multi_part_glb_is_non_empty() {
        let first = compile_geometry_program(&program()).expect("compile");
        let second = compile_geometry_program(&program()).expect("compile second");
        assert_eq!(first.glb, second.glb);
        assert!(first.glb.starts_with(b"glTF"));
        assert_eq!(first.part_ids, vec!["torso", "core"]);
        assert!(first.triangle_count > 0);
    }

    #[test]
    fn v1_uv_tangent_statuses_follow_decoded_glb_bytes() {
        let artifact = compile_geometry_program(&program()).expect("V1 artifact");
        let inspection = integrity::inspect_glb(&artifact.glb).expect("V1 GLB readback");
        assert_eq!(
            (artifact.uv_status, artifact.tangent_status),
            physical_uv_tangent_statuses(&inspection)
        );

        let (root, bin_offset) = glb_root_and_bin_offset(&artifact.glb);
        let tangent_accessor = root["meshes"][0]["primitives"][0]["attributes"]["TANGENT"]
            .as_u64()
            .expect("tangent accessor") as usize;
        let tangent_offset = bin_offset + accessor_byte_offset(&root, tangent_accessor);
        let mut corrupted = artifact.glb.clone();
        corrupted[tangent_offset..tangent_offset + 4].copy_from_slice(&f32::NAN.to_le_bytes());
        let corrupted_inspection =
            integrity::inspect_glb(&corrupted).expect("NaN tangent is inspectable");
        assert_eq!(
            physical_uv_tangent_statuses(&corrupted_inspection).1,
            "failed"
        );
    }

    #[test]
    fn unknown_operator_and_budget_fail_closed() {
        let mut value = program();
        value["nodes"][0]["operator_id"] = Value::String("forgecad.geometry.python@1".to_owned());
        value["canonical_sha256"] = Value::String(canonical_hash(&without_hash(&value)));
        assert!(compile_geometry_program(&value).is_err());
        let mut value = program();
        value["budgets"]["max_triangles"] = Value::Number(serde_json::Number::from(1));
        value["canonical_sha256"] = Value::String(canonical_hash(&without_hash(&value)));
        assert!(compile_geometry_program(&value).is_err());
    }

    fn v2_program() -> Value {
        let mut program = json!({
            "schema_version":"GeometryProgram@2",
            "project_id":"project-test",
            "representation_plan_sha256":"b".repeat(64),
            "operator_catalog_sha256":operator_catalog_sha256(),
            "units":{"length":"meter","angle":"radian","coordinate_system":"right-handed-y-up"},
            "budgets":{
                "max_nodes":16,
                "max_triangles":10000,
                "max_glb_bytes":1048576,
                "max_worker_memory_bytes":536870912,
                "max_runtime_ms":10000
            },
            "nodes":[
                {"node_id":"shell","operator_id":"forgecad.geometry.primitive@2","inputs":[],"parameters":{"shape":"box","size_m":[1.2,1.6,0.55],"position_m":[0.0,1.7,0.0],"rotation_rad":[0.0,0.0,0.0]}},
                {"node_id":"shell-accent","operator_id":"forgecad.geometry.primitive@2","inputs":[],"parameters":{"shape":"box","size_m":[0.8,0.12,0.15],"position_m":[0.0,2.15,0.32],"rotation_rad":[0.0,0.0,0.0]}},
                {"node_id":"joint","operator_id":"forgecad.geometry.primitive@2","inputs":[],"parameters":{"shape":"cylinder","radius_m":0.3,"height_m":0.8,"radial_segments":16,"position_m":[0.0,0.7,0.0],"rotation_rad":[0.0,0.0,0.0]}},
                {"node_id":"sensor","operator_id":"forgecad.geometry.primitive@2","inputs":[],"parameters":{"shape":"ellipsoid","radii_m":[0.25,0.35,0.2],"longitude_segments":16,"latitude_segments":8,"position_m":[0.0,2.6,0.0],"rotation_rad":[0.0,0.0,0.0]}},
                {"node_id":"core","operator_id":"forgecad.geometry.primitive@2","inputs":[],"parameters":{"shape":"sphere","radius_m":0.28,"longitude_segments":16,"latitude_segments":8,"position_m":[0.0,1.65,0.15],"rotation_rad":[0.0,0.0,0.0]}}
            ],
            "part_outputs":[
                {"part_id":"shell","input_node_ids":["shell","shell-accent"],"material_zone_id":"zone-white-shell","solid":true},
                {"part_id":"joint","input_node_ids":["joint"],"material_zone_id":"zone-black-mechanical","solid":true},
                {"part_id":"sensor","input_node_ids":["sensor"],"material_zone_id":"zone-emissive-amber","solid":true},
                {"part_id":"core","input_node_ids":["core"],"material_zone_id":"zone-black-mechanical","solid":true}
            ]
        });
        let hash = canonical_hash(&program);
        program
            .as_object_mut()
            .expect("object")
            .insert("canonical_sha256".to_owned(), Value::String(hash));
        program
    }

    fn v2_draft_program() -> Value {
        let mut draft = v2_program();
        draft
            .as_object_mut()
            .expect("V2 program object")
            .remove("canonical_sha256");
        draft
    }

    #[test]
    fn v2_draft_hash_is_key_order_stable_and_compiler_compatible() {
        let draft = v2_draft_program();
        let mut reordered = Map::new();
        for key in [
            "part_outputs",
            "nodes",
            "budgets",
            "units",
            "operator_catalog_sha256",
            "representation_plan_sha256",
            "project_id",
            "schema_version",
        ] {
            reordered.insert(key.to_owned(), draft.get(key).expect("draft field").clone());
        }
        let reordered = Value::Object(reordered);
        let hash = geometry_program_v2_draft_hash(&draft).expect("draft hash");
        assert_eq!(
            hash,
            geometry_program_v2_draft_hash(&reordered).expect("reordered draft hash")
        );

        let mut canonical_program = reordered;
        canonical_program["canonical_sha256"] = Value::String(hash.clone());
        let artifact = compile_geometry_program(&canonical_program)
            .expect("hash-bound compiler accepts draft result");
        assert_eq!(artifact.program_sha256, hash);
    }

    #[test]
    fn v2_draft_hash_rejects_non_draft_or_invalid_programs() {
        assert!(
            geometry_program_v2_draft_hash(&program()).is_err(),
            "V1 must not enter the V2 helper"
        );
        assert!(
            geometry_program_v2_draft_hash(&v2_program()).is_err(),
            "prefilled hash must be rejected"
        );

        let mut catalog_mismatch = v2_draft_program();
        catalog_mismatch["operator_catalog_sha256"] = Value::String("0".repeat(64));
        assert!(geometry_program_v2_draft_hash(&catalog_mismatch).is_err());

        let mut unknown_root_key = v2_draft_program();
        unknown_root_key["untrusted_extension"] = json!(true);
        assert!(geometry_program_v2_draft_hash(&unknown_root_key).is_err());

        let mut unknown_parameter_key = v2_draft_program();
        unknown_parameter_key["nodes"][0]["parameters"]["script"] = json!("not allowed");
        assert!(geometry_program_v2_draft_hash(&unknown_parameter_key).is_err());

        let mut invalid_variant = v2_draft_program();
        invalid_variant["nodes"][1]["parameters"]["shape"] = json!("cone");
        assert!(geometry_program_v2_draft_hash(&invalid_variant).is_err());

        let mut invalid_input = v2_draft_program();
        invalid_input["nodes"][0]["inputs"] = json!(["unavailable-source"]);
        assert!(geometry_program_v2_draft_hash(&invalid_input).is_err());

        let mut impossible_triangle_budget = v2_draft_program();
        impossible_triangle_budget["budgets"]["max_triangles"] = json!(1);
        assert!(geometry_program_v2_draft_hash(&impossible_triangle_budget).is_err());

        let mut invalid_identifier = v2_draft_program();
        invalid_identifier["project_id"] = json!("project with spaces");
        assert!(geometry_program_v2_draft_hash(&invalid_identifier).is_err());

        let mut oversized_position = v2_draft_program();
        oversized_position["nodes"][0]["parameters"]["position_m"] = json!([10.1, 0.0, 0.0]);
        assert!(geometry_program_v2_draft_hash(&oversized_position).is_err());

        let mut oversized_radius = v2_draft_program();
        oversized_radius["nodes"][3]["parameters"]["radius_m"] = json!(5.1);
        assert!(geometry_program_v2_draft_hash(&oversized_radius).is_err());
    }

    #[test]
    fn v2_primitives_are_deterministic_and_strictly_read_back() {
        let program = v2_program();
        let first = compile_geometry_program(&program).expect("first V2 compile");
        for _ in 0..4 {
            assert_eq!(
                first.glb,
                compile_geometry_program(&program)
                    .expect("repeat V2 compile")
                    .glb
            );
        }
        let inspection = integrity::inspect_glb(&first.glb).expect("strict GLB readback");
        assert!(
            inspection.hard_gate_passed,
            "{:?}",
            inspection.failure_codes
        );
        assert_eq!(inspection.degenerate_triangle_count, 0);
        assert_eq!(inspection.boundary_edge_count, 0);
        assert_eq!(inspection.non_manifold_edge_count, 0);
        assert_eq!(inspection.winding_error_count, 0);
        assert_eq!(inspection.zero_area_uv_triangle_count, 0);
        assert_eq!(inspection.tangent_orthogonality_error_count, 0);
        assert_eq!(first.part_ids, vec!["shell", "joint", "sensor", "core"]);
        assert_eq!(inspection.part_bindings.len(), 5);
        let (root, _) = glb_root_and_bin_offset(&first.glb);
        assert_eq!(root["meshes"].as_array().expect("meshes").len(), 4);
        assert_eq!(root["nodes"].as_array().expect("nodes").len(), 4);
        assert_eq!(
            root["meshes"][0]["primitives"]
                .as_array()
                .expect("semantic shell primitives")
                .len(),
            2
        );
        assert_eq!(root["meshes"][0]["extras"]["part_id"], "shell");
        assert_eq!(
            root["meshes"][0]["primitives"][0]["extras"]["source_node_id"],
            "shell"
        );
        assert_eq!(
            root["meshes"][0]["primitives"][1]["extras"]["source_node_id"],
            "shell-accent"
        );
        assert_eq!(
            root["extras"]["forgecad"]["part_bindings"]
                .as_array()
                .expect("part bindings")
                .iter()
                .map(|binding| binding["source_node_id"].as_str().expect("source id"))
                .collect::<Vec<_>>(),
            vec!["shell", "shell-accent", "joint", "sensor", "core"]
        );
        assert_eq!(
            inspection
                .part_bindings
                .iter()
                .map(|binding| (binding.part_id.as_str(), binding.source_node_id.as_str()))
                .collect::<Vec<_>>(),
            vec![
                ("shell", "shell"),
                ("shell", "shell-accent"),
                ("joint", "joint"),
                ("sensor", "sensor"),
                ("core", "core"),
            ]
        );
        assert_eq!(
            inspection.operator_catalog_sha256.as_deref(),
            Some(operator_catalog_sha256().as_str())
        );
    }

    #[test]
    fn c_fixed_perspective_renderer_emits_deterministic_nine_aov_set() {
        let artifact = compile_geometry_program(&v2_program()).expect("V2 artifact");
        let camera = json!({
            "schema_version":"CameraCalibration@1",
            "camera_hash":"a".repeat(64),
            "projection":"perspective",
            "transform":{"position_m":[4.0,3.0,6.0],"target_m":[0.0,1.5,0.0],"up":[0.0,1.0,0.0]},
            "fov_y_degrees":42.0,
            "near_m":0.05,
            "far_m":20.0,
            "resolution":{"width":512,"height":512},
            "coordinate_system":"right-handed-y-up-meter",
            "renderer_revision":"forgecad-renderer-2",
            "canonical_sha256":"b".repeat(64)
        });
        let first = render_perspective_glb(&artifact.glb, &camera).expect("C renderer");
        let second = render_perspective_glb(&artifact.glb, &camera).expect("C renderer repeat");
        assert_eq!(first.len(), 9);
        assert_eq!(
            first
                .iter()
                .map(|pass| pass.pass.as_str())
                .collect::<Vec<_>>(),
            vec![
                "beauty",
                "silhouette",
                "depth",
                "normal",
                "ao",
                "part-id",
                "material-id",
                "wireframe",
                "uv-stretch"
            ]
        );
        assert_eq!(
            first
                .iter()
                .map(|pass| (pass.width, pass.height))
                .collect::<Vec<_>>(),
            vec![(512, 512); 9]
        );
        assert_eq!(
            first
                .iter()
                .map(|pass| pass.png.clone())
                .collect::<Vec<_>>(),
            second
                .iter()
                .map(|pass| pass.png.clone())
                .collect::<Vec<_>>()
        );
        assert!(first
            .iter()
            .all(|pass| pass.png.starts_with(b"\x89PNG\r\n\x1a\n")));
    }

    #[test]
    fn operator_catalog_is_closed_and_hash_bound() {
        let catalog = operator_catalog();
        let declared = catalog["canonical_sha256"]
            .as_str()
            .expect("catalog hash")
            .to_owned();
        let mut without_hash = catalog.as_object().expect("catalog object").clone();
        without_hash.remove("canonical_sha256");
        assert_eq!(declared, canonical_hash(&Value::Object(without_hash)));
        assert_eq!(catalog["operators"].as_array().expect("operators").len(), 1);
        assert_eq!(
            catalog["operators"][0]["operator_id"],
            "forgecad.geometry.primitive@2"
        );
    }

    #[test]
    fn v2_catalog_and_part_output_mismatch_fail_closed() {
        let mut catalog_mismatch = v2_program();
        catalog_mismatch["operator_catalog_sha256"] = Value::String("0".repeat(64));
        catalog_mismatch["canonical_sha256"] =
            Value::String(canonical_hash(&without_hash(&catalog_mismatch)));
        assert!(compile_geometry_program(&catalog_mismatch).is_err());

        let mut dangling_output = v2_program();
        dangling_output["part_outputs"][0]["input_node_ids"] = json!(["missing"]);
        dangling_output["canonical_sha256"] =
            Value::String(canonical_hash(&without_hash(&dangling_output)));
        assert!(compile_geometry_program(&dangling_output).is_err());
    }

    #[test]
    fn v2_part_output_aggregation_rejects_ambiguous_or_unconsumed_sources() {
        let mut empty_inputs = v2_draft_program();
        empty_inputs["part_outputs"][0]["input_node_ids"] = json!([]);
        assert!(geometry_program_v2_draft_hash(&empty_inputs).is_err());

        let mut duplicate_inside_part = v2_draft_program();
        duplicate_inside_part["part_outputs"][0]["input_node_ids"] = json!(["shell", "shell"]);
        assert!(geometry_program_v2_draft_hash(&duplicate_inside_part).is_err());

        let mut unknown_source = v2_draft_program();
        unknown_source["part_outputs"][0]["input_node_ids"] = json!(["not-a-node"]);
        assert!(geometry_program_v2_draft_hash(&unknown_source).is_err());

        let mut reused_across_parts = v2_draft_program();
        reused_across_parts["part_outputs"][1]["input_node_ids"] = json!(["joint", "shell"]);
        assert!(geometry_program_v2_draft_hash(&reused_across_parts).is_err());

        let mut unconsumed_node = v2_draft_program();
        unconsumed_node["part_outputs"]
            .as_array_mut()
            .expect("part outputs")
            .pop();
        assert!(geometry_program_v2_draft_hash(&unconsumed_node).is_err());

        let mut legacy_single_source_field = v2_draft_program();
        let output = legacy_single_source_field["part_outputs"][0]
            .as_object_mut()
            .expect("part output");
        output.remove("input_node_ids");
        output.insert("source_node_id".to_owned(), json!("shell"));
        assert!(geometry_program_v2_draft_hash(&legacy_single_source_field).is_err());
    }

    fn glb_root_and_bin_offset(glb: &[u8]) -> (Value, usize) {
        let json_length = u32::from_le_bytes(glb[12..16].try_into().expect("json length")) as usize;
        let root = serde_json::from_slice(&glb[20..20 + json_length]).expect("GLB root");
        (root, 20 + json_length + 8)
    }

    fn accessor_byte_offset(root: &Value, accessor_index: usize) -> usize {
        let accessor = &root["accessors"][accessor_index];
        let view_index = accessor["bufferView"].as_u64().expect("buffer view") as usize;
        let view_offset = root["bufferViews"][view_index]["byteOffset"]
            .as_u64()
            .unwrap_or(0) as usize;
        let accessor_offset = accessor["byteOffset"].as_u64().unwrap_or(0) as usize;
        view_offset + accessor_offset
    }

    #[test]
    fn strict_readback_rejects_actual_bin_and_lineage_corruption() {
        let artifact = compile_geometry_program(&v2_program()).expect("V2 artifact");
        let (root, bin_offset) = glb_root_and_bin_offset(&artifact.glb);
        let primitive = &root["meshes"][0]["primitives"][0];

        let mut invalid_index = artifact.glb.clone();
        let index_accessor = primitive["indices"].as_u64().expect("index accessor") as usize;
        let index_offset = bin_offset + accessor_byte_offset(&root, index_accessor);
        invalid_index[index_offset..index_offset + 4].copy_from_slice(&u32::MAX.to_le_bytes());
        let invalid_index_report =
            integrity::inspect_glb(&invalid_index).expect("corrupt index is inspectable");
        assert!(invalid_index_report.invalid_index_count > 0);
        assert!(!invalid_index_report.hard_gate_passed);

        let mut nan_uv = artifact.glb.clone();
        let uv_accessor = primitive["attributes"]["TEXCOORD_0"]
            .as_u64()
            .expect("UV accessor") as usize;
        let uv_offset = bin_offset + accessor_byte_offset(&root, uv_accessor);
        nan_uv[uv_offset..uv_offset + 4].copy_from_slice(&f32::NAN.to_le_bytes());
        let nan_uv_report = integrity::inspect_glb(&nan_uv).expect("NaN UV is inspectable");
        assert!(nan_uv_report.uv_non_finite_count > 0);
        assert!(!nan_uv_report.hard_gate_passed);

        let mut missing_lineage = artifact.glb.clone();
        let json_length =
            u32::from_le_bytes(missing_lineage[12..16].try_into().expect("json length")) as usize;
        let json = &mut missing_lineage[20..20 + json_length];
        let needle = b"source_node_id";
        let starts = (0..=json.len().saturating_sub(needle.len()))
            .filter(|start| &json[*start..*start + needle.len()] == needle)
            .collect::<Vec<_>>();
        for start in starts {
            json[start] = b'x';
        }
        let lineage_report =
            integrity::inspect_glb(&missing_lineage).expect("missing lineage is inspectable");
        assert!(lineage_report.metadata_mismatch_count > 0 || lineage_report.part_coverage < 1.0);
        assert!(!lineage_report.hard_gate_passed);

        let mut wrong_material = artifact.glb.clone();
        let json_length =
            u32::from_le_bytes(wrong_material[12..16].try_into().expect("json length")) as usize;
        let json = &mut wrong_material[20..20 + json_length];
        let material_name = b"\"name\":\"zone-white-shell\"";
        let start = (0..=json.len().saturating_sub(material_name.len()))
            .find(|start| &json[*start..*start + material_name.len()] == material_name)
            .expect("material name");
        let zone_start = start + b"\"name\":\"".len();
        json[zone_start] = b'x';
        let material_report =
            integrity::inspect_glb(&wrong_material).expect("wrong material is inspectable");
        assert!(material_report.material_zone_coverage < 1.0);
        assert!(!material_report.hard_gate_passed);
    }

    fn without_hash(value: &Value) -> Value {
        let mut object = value.as_object().unwrap().clone();
        object.remove("canonical_sha256");
        Value::Object(object)
    }
}
