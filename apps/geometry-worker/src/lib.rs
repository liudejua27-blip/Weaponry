//! Bounded, product-owned geometry compiler for the MCP007 vertical slice.
//!
//! This is intentionally small: it accepts only a canonical GeometryProgram,
//! a few primitive operators, and emits a deterministic glTF 2.0 GLB.  It is
//! not a general scripting engine and never reads files, starts processes, or
//! calls a model/network service.

#![recursion_limit = "256"]

mod high_low_cage_diagnostic;
pub mod integrity;
mod manifold_bridge;
pub mod material_layer_graph;
mod operator_d;
mod production_cage_offset;
mod production_geometric_bake;
mod production_hero_material;
mod production_hero_uv_layout;
mod production_low_retopology;
mod surface_bake;

// This is a deliberately tiny, typed seam for sibling product-owned
// evaluators.  It does not expose the Manifold object model or change the
// GeometryProgram/MCP surface; callers still receive a copied mesh result and
// remain responsible for their own candidate/lineage policy.
pub use manifold_bridge::{manifold_boolean_typed, ManifoldBooleanOutput};

use base64::Engine;
pub use forgecad_worker_protocol::{
    material_pack_manifest, material_pack_manifest_by_id, material_pack_manifest_sha256,
    material_pack_manifest_sha256_by_id, operator_catalog, operator_catalog_sha256,
};
use image::{
    codecs::png::{CompressionType, FilterType, PngEncoder},
    imageops, ExtendedColorType, ImageEncoder,
};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

const MAX_COORDINATE: f32 = 10.0;
const MAX_DIMENSION: f32 = 10.0;
const FICTIONAL_ENERGY_WEAPON_2K_PACK_ID: &str = "forgecad-fictional-energy-weapon-2k";
const FICTIONAL_ENERGY_WEAPON_2K_RESOLUTION: u32 = 2048;
const FICTIONAL_ENERGY_WEAPON_2K_RECIPE_ID: &str =
    "forgecad-first-party-catmullrom-semantic-microdetail-2k@1";
const FICTIONAL_ENERGY_WEAPON_2K_BUILD_BUDGET_MS: u64 = 120_000;

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
    lineage_source_node_ids: Vec<String>,
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    tangents: Vec<[f32; 4]>,
    indices: Vec<u32>,
    uv_chart_count: usize,
    uv_chart_ids: Vec<usize>,
}

#[derive(Debug, Clone)]
struct PrimitiveNodeMesh {
    operator_id: String,
    /// Ordered, deduplicated authoring nodes that contribute to this typed
    /// mesh.  Boolean keeps the operand lineage here even though its output
    /// becomes one semantic source primitive in the GLB.
    lineage_source_node_ids: Vec<String>,
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
    operator_id: String,
    operator: operator_d::ValidatedOperator,
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
        let (positions, normals, uvs, tangents, indices, uv_chart_count, uv_chart_ids) =
            triangulate_uv_charts(&positions, &normals, &indices, false, false)?;
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
                lineage_source_node_ids: vec![node_id.to_owned()],
                positions,
                normals,
                uvs,
                tangents,
                indices,
                uv_chart_count,
                uv_chart_ids,
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
        appearance,
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
    validate_appearance_part_bindings(appearance, &validation.part_outputs)?;
    let continuous_uv = appearance
        .and_then(|value| value.get("material_pack_id"))
        .and_then(Value::as_str)
        .is_some_and(|pack_id| {
            matches!(
                pack_id,
                "forgecad-fictional-energy-weapon" | FICTIONAL_ENERGY_WEAPON_2K_PACK_ID
            )
        });
    let builds_2k_textures = appearance
        .and_then(|value| value.get("material_pack_id"))
        .and_then(Value::as_str)
        == Some(FICTIONAL_ENERGY_WEAPON_2K_PACK_ID);
    let mut sources = std::collections::BTreeMap::<String, PrimitiveNodeMesh>::new();
    let mut source_operators =
        std::collections::BTreeMap::<String, operator_d::ValidatedOperator>::new();
    for node in &validation.nodes {
        let mut mesh = operator_d::compile_operator(
            &node.operator,
            &sources,
            &source_operators,
            validation.max_triangles,
            validation.max_runtime_ms,
        )?;
        mesh.operator_id = node.operator_id.clone();
        if mesh.lineage_source_node_ids.is_empty() {
            mesh.lineage_source_node_ids.push(node.node_id.clone());
        }
        sources.insert(node.node_id.clone(), mesh);
        source_operators.insert(node.node_id.clone(), node.operator.clone());
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
            let (positions, mut normals, uvs, mut tangents, indices, uv_chart_count, uv_chart_ids) =
                triangulate_uv_charts(
                    &source.positions,
                    &source.normals,
                    &source.indices,
                    matches!(
                        source.operator_id.as_str(),
                        "forgecad.geometry.boolean@1" | "forgecad.geometry.authoring-mesh@1"
                    ),
                    continuous_uv,
                )?;
            if source.operator_id == "forgecad.geometry.authoring-mesh@1" {
                for triangle_index in 0..positions.len() / 3 {
                    let base = triangle_index * 3;
                    let triangle_positions =
                        [positions[base], positions[base + 1], positions[base + 2]];
                    let face_cross = cross3(
                        subtract3(triangle_positions[1], triangle_positions[0]),
                        subtract3(triangle_positions[2], triangle_positions[0]),
                    );
                    let face = scale3(face_cross, 1.0 / length3(face_cross));
                    let triangle_uvs = [uvs[base], uvs[base + 1], uvs[base + 2]];
                    for vertex in 0..3 {
                        normals[base + vertex] = face;
                        tangents[base + vertex] =
                            tangent_from_uv_frame(triangle_positions, face, triangle_uvs)
                                .ok_or_else(|| {
                                    GeometryError::Invalid(
                                        "authoring-mesh structural tangent frame is invalid"
                                            .to_owned(),
                                    )
                                })?;
                    }
                }
            }
            part_sources.push(PartSourceMesh {
                source_node_id,
                operator_id: source.operator_id,
                lineage_source_node_ids: source.lineage_source_node_ids,
                positions,
                normals,
                uvs,
                tangents,
                indices,
                uv_chart_count,
                uv_chart_ids,
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
    if continuous_uv {
        repack_continuous_uv_atlas(&mut parts)?;
    }
    let triangle_count = parts
        .iter()
        .flat_map(|part| &part.sources)
        .map(|source| source.indices.len() as u64 / 3)
        .sum::<u64>();
    // A Boolean node declares the conservative sum of its operands during
    // validation.  The actual manifold result may legitimately be smaller,
    // so the generated readback must stay within that estimate rather than
    // pretending a topology-changing operation has an exact triangle count.
    if triangle_count == 0
        || triangle_count > validation.estimated_triangle_count
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
        appearance,
    )?;
    if glb.len() as u64 > validation.max_glb_bytes {
        return Err(GeometryError::Invalid(
            "GeometryProgram@2 GLB exceeds the declared budget".to_owned(),
        ));
    }
    let effective_runtime_budget_ms = if builds_2k_textures {
        FICTIONAL_ENERGY_WEAPON_2K_BUILD_BUDGET_MS
    } else {
        validation.max_runtime_ms
    };
    if started.elapsed().as_millis() as u64 > effective_runtime_budget_ms {
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
            "GeometryProgram@2 strict GLB readback failed: {}; triangle_count={}; part_bindings={:?}; winding_error_count={}; tangent_orthogonality_error_count={}; tangent_handedness_error_count={}",
            inspection.failure_codes.join(","),
            inspection.triangle_count,
            inspection.part_bindings,
            inspection.winding_error_count,
            inspection.tangent_orthogonality_error_count,
            inspection.tangent_handedness_error_count,
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

fn validate_appearance_part_bindings(
    appearance: Option<&Value>,
    part_outputs: &[ValidatedV2PartOutput],
) -> Result<(), GeometryError> {
    let Some(object) = appearance.and_then(Value::as_object) else {
        return Ok(());
    };
    if object.get("schema_version").and_then(Value::as_str) != Some("AppearanceProgram@2") {
        return Ok(());
    }
    let zones = object
        .get("material_zones")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            GeometryError::Invalid("AppearanceProgram@2 material_zones is required".to_owned())
        })?;
    let mut expected = std::collections::BTreeMap::<String, BTreeSet<String>>::new();
    for output in part_outputs {
        expected
            .entry(output.material_zone_id.clone())
            .or_default()
            .insert(output.part_id.clone());
    }
    let mut declared = std::collections::BTreeMap::<String, BTreeSet<String>>::new();
    let mut all_declared_parts = BTreeSet::new();
    for zone in zones {
        let zone = zone.as_object().ok_or_else(|| {
            GeometryError::Invalid("AppearanceProgram@2 material zone must be an object".to_owned())
        })?;
        let zone_id = required_text(zone, "zone_id")?.to_owned();
        let part_ids = zone
            .get("part_ids")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                GeometryError::Invalid(
                    "AppearanceProgram@2 material zone part_ids is required".to_owned(),
                )
            })?;
        let zone_parts = declared.entry(zone_id).or_default();
        for part_id in part_ids {
            let part_id = part_id.as_str().ok_or_else(|| {
                GeometryError::Invalid(
                    "AppearanceProgram@2 material zone part_ids is invalid".to_owned(),
                )
            })?;
            if !zone_parts.insert(part_id.to_owned())
                || !all_declared_parts.insert(part_id.to_owned())
            {
                return Err(GeometryError::Invalid(
                    "AppearanceProgram@2 part_ids must be unique and bind one material zone"
                        .to_owned(),
                ));
            }
        }
    }
    if declared != expected {
        return Err(GeometryError::Invalid(
            "AppearanceProgram@2 part_ids do not exactly match GeometryProgram@2 part outputs"
                .to_owned(),
        ));
    }
    Ok(())
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
    let mut node_counts = std::collections::BTreeMap::<String, u64>::new();
    let mut node_operators =
        std::collections::BTreeMap::<String, operator_d::ValidatedOperator>::new();
    let mut normal_policy_derived_nodes = HashSet::new();
    let mut node_ids = HashSet::new();
    let mut consumed_by_node = HashSet::new();
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
        if !node_ids.insert(node_id.clone()) {
            return Err(GeometryError::Invalid(
                "GeometryProgram@2 node_id must be unique".to_owned(),
            ));
        }
        let operator_id = required_text(node, "operator_id")?.to_owned();
        let inputs = node
            .get("inputs")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                GeometryError::Invalid("GeometryProgram@2 inputs is required".to_owned())
            })?;
        let mut input_node_ids = Vec::with_capacity(inputs.len());
        let mut local_inputs = HashSet::new();
        for input in inputs {
            let input = input.as_str().ok_or_else(|| {
                GeometryError::Invalid(
                    "GeometryProgram@2 inputs must contain identifiers".to_owned(),
                )
            })?;
            let input = required_identifier_value(input, "inputs")?.to_owned();
            if !local_inputs.insert(input.clone()) {
                return Err(GeometryError::Invalid(
                    "GeometryProgram@2 node inputs must not repeat a source node".to_owned(),
                ));
            }
            if !node_ids.contains(&input) {
                return Err(GeometryError::Invalid(
                    "GeometryProgram@2 node inputs must reference an earlier node".to_owned(),
                ));
            }
            if !consumed_by_node.insert(input.clone()) {
                return Err(GeometryError::Invalid(
                    "GeometryProgram@2 nodes may be consumed by exactly one downstream node"
                        .to_owned(),
                ));
            }
            input_node_ids.push(input);
        }
        let parameters = node
            .get("parameters")
            .and_then(Value::as_object)
            .ok_or_else(|| {
                GeometryError::Invalid(
                    "GeometryProgram@2 node parameters must be an object".to_owned(),
                )
            })?;
        let (operator, triangle_count) =
            operator_d::validate_operator(&operator_id, &input_node_ids, parameters, &node_counts)?;
        if let operator_d::ValidatedOperator::Bevel { input, .. } = &operator {
            if !matches!(
                node_operators.get(input),
                Some(operator_d::ValidatedOperator::Primitive(
                    ValidatedV2Primitive::Box { .. }
                ))
            ) {
                return Err(GeometryError::Invalid(
                    "bevel@1 requires one direct primitive@2 box input".to_owned(),
                ));
            }
        }
        if let operator_d::ValidatedOperator::BevelV2 { input, .. } = &operator {
            if !matches!(
                node_operators.get(input),
                Some(operator_d::ValidatedOperator::AuthoringMesh { .. })
            ) {
                return Err(GeometryError::Invalid(
                    "bevel@2 requires one direct authoring-mesh@1 input".to_owned(),
                ));
            }
        }
        let has_upstream_normal_policy = input_node_ids
            .iter()
            .any(|input| normal_policy_derived_nodes.contains(input));
        if matches!(operator, operator_d::ValidatedOperator::Boolean { .. })
            && has_upstream_normal_policy
        {
            return Err(GeometryError::Invalid(
                "normal-policy@1 must run after Boolean topology changes".to_owned(),
            ));
        }
        if matches!(operator, operator_d::ValidatedOperator::NormalPolicy { .. })
            || has_upstream_normal_policy
        {
            normal_policy_derived_nodes.insert(node_id.clone());
        }
        node_counts.insert(node_id.clone(), triangle_count);
        node_operators.insert(node_id.clone(), operator.clone());
        nodes.push(ValidatedV2Node {
            node_id,
            operator_id,
            operator,
        });
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
            let node_count = node_counts.get(source_node_id).ok_or_else(|| {
                GeometryError::Invalid(
                    "GeometryProgram@2 part output references an unknown source node".to_owned(),
                )
            })?;
            estimated_triangle_count = estimated_triangle_count
                .checked_add(*node_count)
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
    if consumed_sources
        .iter()
        .any(|node_id| consumed_by_node.contains(node_id))
    {
        return Err(GeometryError::Invalid(
            "GeometryProgram@2 part output cannot consume an intermediate node".to_owned(),
        ));
    }
    let final_nodes = consumed_sources.len();
    if final_nodes == 0
        || node_counts
            .keys()
            .filter(|node_id| !consumed_by_node.contains(*node_id))
            .count()
            != final_nodes
    {
        return Err(GeometryError::Invalid(
            "GeometryProgram@2 nodes require exactly one explicit downstream or part output consumption"
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

pub fn boolean_operand_lineage_preview(
    program: &Value,
    boolean_node_id: &str,
    max_lineage_runs: usize,
) -> Result<Value, GeometryError> {
    required_identifier_value(boolean_node_id, "boolean_node_id")?;
    if !(1..=4096).contains(&max_lineage_runs) {
        return Err(GeometryError::Invalid(
            "max_lineage_runs must be between 1 and 4096".to_owned(),
        ));
    }
    let validation =
        validate_geometry_program_v2(program, V2CanonicalHashRequirement::PresentAndMatches)?;
    let mut sources = std::collections::BTreeMap::<String, PrimitiveNodeMesh>::new();
    let mut source_operators =
        std::collections::BTreeMap::<String, operator_d::ValidatedOperator>::new();
    for node in &validation.nodes {
        let (mut mesh, lineage) = operator_d::compile_operator_with_boolean_lineage(
            &node.operator,
            &sources,
            &source_operators,
            validation.max_triangles,
            validation.max_runtime_ms,
        )?;
        mesh.operator_id = node.operator_id.clone();
        if mesh.lineage_source_node_ids.is_empty() {
            mesh.lineage_source_node_ids.push(node.node_id.clone());
        }
        if node.node_id == boolean_node_id {
            let lineage = lineage.ok_or_else(|| {
                GeometryError::Invalid("boolean_node_id does not select a Boolean node".to_owned())
            })?;
            let left_sources = sources
                .get(&lineage.left_node_id)
                .map(|value| value.lineage_source_node_ids.clone())
                .ok_or_else(|| {
                    GeometryError::Invalid("Boolean left lineage is missing".to_owned())
                })?;
            let right_sources = sources
                .get(&lineage.right_node_id)
                .map(|value| value.lineage_source_node_ids.clone())
                .ok_or_else(|| {
                    GeometryError::Invalid("Boolean right lineage is missing".to_owned())
                })?;
            if lineage.source_ids.len() != lineage.evaluated_face_ids.len()
                || lineage.source_ids.len() != mesh.indices.len() / 3
            {
                return Err(GeometryError::Invalid(
                    "Boolean lineage length differs from evaluated triangles".to_owned(),
                ));
            }
            let mut runs = Vec::<Value>::new();
            for (triangle_index, (&source_id, &face_id)) in lineage
                .source_ids
                .iter()
                .zip(&lineage.evaluated_face_ids)
                .enumerate()
            {
                let operand = if source_id == 0 { "left" } else { "right" };
                let operand_node_id = if source_id == 0 {
                    &lineage.left_node_id
                } else {
                    &lineage.right_node_id
                };
                if let Some(last) = runs.last_mut() {
                    let same = last.get("operand").and_then(Value::as_str) == Some(operand)
                        && last.get("evaluated_face_id").and_then(Value::as_u64) == Some(face_id)
                        && last
                            .get("output_triangle_start")
                            .and_then(Value::as_u64)
                            .zip(last.get("output_triangle_count").and_then(Value::as_u64))
                            .is_some_and(|(start, count)| start + count == triangle_index as u64);
                    if same {
                        last["output_triangle_count"] =
                            Value::from(last["output_triangle_count"].as_u64().unwrap() + 1);
                        continue;
                    }
                }
                if runs.len() >= max_lineage_runs {
                    return Err(GeometryError::Invalid(
                        "Boolean lineage exceeds max_lineage_runs without truncation".to_owned(),
                    ));
                }
                runs.push(json!({
                    "output_triangle_start":triangle_index,
                    "output_triangle_count":1,
                    "operand":operand,
                    "operand_node_id":operand_node_id,
                    "evaluated_face_id":face_id
                }));
            }
            let lineage_sha256 = canonical_hash(&Value::Array(runs.clone()));
            let left_triangle_count = lineage.source_ids.iter().filter(|id| **id == 0).count();
            let right_triangle_count = lineage.source_ids.iter().filter(|id| **id == 1).count();
            let mut result = json!({
                "schema_version":"BooleanOperandLineage@1",
                "program_sha256":validation.program_sha256,
                "operator_catalog_sha256":validation.catalog_sha256,
                "boolean_node_id":boolean_node_id,
                "operation":lineage.operation,
                "operands":[
                    {"operand":"left","node_id":lineage.left_node_id,"lineage_source_node_ids":left_sources,"output_triangle_count":left_triangle_count},
                    {"operand":"right","node_id":lineage.right_node_id,"lineage_source_node_ids":right_sources,"output_triangle_count":right_triangle_count}
                ],
                "output_triangle_count":lineage.source_ids.len(),
                "lineage_run_count":runs.len(),
                "lineage_runs":runs,
                "lineage_sha256":lineage_sha256,
                "lineage_kind":"evaluated-face-with-operand-run",
                "materialization_status":"preview-only-not-persisted-in-glb",
                "runtime_write_performed":false,
                "limitations":[
                    "EVALUATED_FACE_ID_NOT_ORIGINAL_AUTHORING_FACE_ID",
                    "FACE_IDS_NOT_STABLE_ACROSS_PROGRAM_CHANGE",
                    "LINEAGE_NOT_PERSISTED_IN_CURRENT_GLB",
                    "STRUCTURAL_LINEAGE_DOES_NOT_PROVE_VISUAL_QUALITY"
                ],
                "canonical_sha256":""
            });
            result["canonical_sha256"] = Value::String(canonical_hash(&result));
            return Ok(result);
        }
        sources.insert(node.node_id.clone(), mesh);
        source_operators.insert(node.node_id.clone(), node.operator.clone());
    }
    Err(GeometryError::Invalid(
        "boolean_node_id is unavailable in GeometryProgram@2".to_owned(),
    ))
}

pub fn subdivision_topology_lineage_preview(
    program: &Value,
    subdivision_node_id: &str,
    max_lineage_elements: usize,
) -> Result<Value, GeometryError> {
    required_identifier_value(subdivision_node_id, "subdivision_node_id")?;
    if !(1..=25_000).contains(&max_lineage_elements) {
        return Err(GeometryError::Invalid(
            "max_lineage_elements must be between 1 and 25000".to_owned(),
        ));
    }
    let validation =
        validate_geometry_program_v2(program, V2CanonicalHashRequirement::PresentAndMatches)?;
    let node = validation
        .nodes
        .iter()
        .find(|node| node.node_id == subdivision_node_id)
        .ok_or_else(|| {
            GeometryError::Invalid(
                "subdivision_node_id is unavailable in GeometryProgram@2".to_owned(),
            )
        })?;
    if node.operator_id != "forgecad.geometry.subd-cage@2" {
        return Err(GeometryError::Invalid(
            "subdivision_node_id must select forgecad.geometry.subd-cage@2".to_owned(),
        ));
    }
    let lineage = operator_d::subdivision_topology_lineage(&node.operator)?;
    let control_counts = lineage
        .get("control_counts")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            GeometryError::Invalid("subdivision control counts are missing".to_owned())
        })?;
    let evaluated_counts = lineage
        .get("evaluated_counts")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            GeometryError::Invalid("subdivision evaluated counts are missing".to_owned())
        })?;
    let lineage_element_count = ["vertex_count", "edge_count", "quad_count"]
        .into_iter()
        .chain(["vertex_count", "edge_count", "quad_count", "triangle_count"])
        .enumerate()
        .try_fold(0u64, |total, (index, key)| {
            let source = if index < 3 {
                control_counts
            } else {
                evaluated_counts
            };
            source
                .get(key)
                .and_then(Value::as_u64)
                .and_then(|count| total.checked_add(count))
                .ok_or_else(|| {
                    GeometryError::Invalid(
                        "subdivision lineage element count is invalid".to_owned(),
                    )
                })
        })?;
    if lineage_element_count > max_lineage_elements as u64 {
        return Err(GeometryError::Invalid(format!(
            "subdivision lineage requires {lineage_element_count} elements and exceeds max_lineage_elements without truncation"
        )));
    }
    let lineage_sha256 = canonical_hash(&lineage);
    let mut result = json!({
        "schema_version":"SubdivisionTopologyLineage@1",
        "program_sha256":validation.program_sha256,
        "operator_catalog_sha256":validation.catalog_sha256,
        "subdivision_node_id":subdivision_node_id,
        "lineage_kind":"control-root-to-evaluated-quad-topology@1",
        "lineage_space":"evaluated-quad-topology@1",
        "id_scope":"program-and-evaluation-bound",
        "complete":true,
        "completeness_scope":"all-root-mappings-within-declared-preview-lineage",
        "cross_version_stable":false,
        "artifact_binding_status":"unavailable-preview-only",
        "max_lineage_elements":max_lineage_elements,
        "lineage_element_count":lineage_element_count,
        "lineage":lineage,
        "lineage_sha256":lineage_sha256,
        "materialization_status":"preview-only-not-persisted-in-glb",
        "runtime_write_performed":false,
        "quality_status":"structural_only",
        "limitations":[
            "REGULAR_RECTANGULAR_OPEN_QUAD_GRID_ONLY",
            "INTEGER_EDGE_SHARPNESS_LEVELS_1_TO_2_ONLY",
            "ELEMENT_IDS_CHANGE_WHEN_PROGRAM_OR_EVALUATION_CHANGES",
            "EVALUATED_QUAD_IDS_ARE_NOT_GLTF_TRIANGLE_OR_DEDUPLICATED_VERTEX_IDS",
            "ROOT_ANCESTRY_ONLY_NO_INFLUENCE_WEIGHTS_OR_CORNER_DOMAIN",
            "PREVIEW_NOT_ARTIFACT_OR_READBACK_BOUND",
            "LINEAGE_NOT_PERSISTED_IN_CURRENT_GLB",
            "STRUCTURAL_LINEAGE_DOES_NOT_PROVE_VISUAL_QUALITY"
        ],
        "canonical_sha256":""
    });
    result["canonical_sha256"] = Value::String(canonical_hash(&result));
    Ok(result)
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
        "compile_geometry" => {
            require_closed_payload(payload, &["geometry_program", "appearance_program"])?;
            let program = payload
                .get("geometry_program")
                .ok_or_else(|| GeometryError::Invalid("geometry_program is required".to_owned()))?;
            let appearance = payload
                .get("appearance_program")
                .filter(|value| !value.is_null());
            let artifact = compile_geometry_program_with_appearance(program, appearance)?;
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
        "boolean_operand_lineage" => {
            require_closed_payload(
                payload,
                &["geometry_program", "boolean_node_id", "max_lineage_runs"],
            )?;
            let program = payload
                .get("geometry_program")
                .ok_or_else(|| GeometryError::Invalid("geometry_program is required".to_owned()))?;
            let boolean_node_id = payload
                .get("boolean_node_id")
                .and_then(Value::as_str)
                .ok_or_else(|| GeometryError::Invalid("boolean_node_id is required".to_owned()))?;
            let max_lineage_runs = payload
                .get("max_lineage_runs")
                .and_then(Value::as_u64)
                .and_then(|value| usize::try_from(value).ok())
                .ok_or_else(|| GeometryError::Invalid("max_lineage_runs is invalid".to_owned()))?;
            boolean_operand_lineage_preview(program, boolean_node_id, max_lineage_runs)
        }
        "subdivision_topology_lineage" => {
            require_closed_payload(
                payload,
                &[
                    "geometry_program",
                    "subdivision_node_id",
                    "max_lineage_elements",
                ],
            )?;
            let program = payload
                .get("geometry_program")
                .ok_or_else(|| GeometryError::Invalid("geometry_program is required".to_owned()))?;
            let subdivision_node_id = payload
                .get("subdivision_node_id")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    GeometryError::Invalid("subdivision_node_id is required".to_owned())
                })?;
            let max_lineage_elements = payload
                .get("max_lineage_elements")
                .and_then(Value::as_u64)
                .and_then(|value| usize::try_from(value).ok())
                .ok_or_else(|| {
                    GeometryError::Invalid("max_lineage_elements is invalid".to_owned())
                })?;
            subdivision_topology_lineage_preview(program, subdivision_node_id, max_lineage_elements)
        }
        forgecad_worker_protocol::PRODUCTION_WEAPON_HIGH_LOW_CAGE_DIAGNOSTIC_OPERATION => {
            high_low_cage_diagnostic::diagnose(payload)
        }
        forgecad_worker_protocol::PRODUCTION_WEAPON_HIGH_LOW_CAGE_ARTIFACT_PRODUCER_OPERATION => {
            high_low_cage_diagnostic::produce(payload)
        }
        forgecad_worker_protocol::PRODUCTION_WEAPON_LOW_RETOPOLOGY_OPERATION => {
            production_low_retopology::run(payload)
        }
        forgecad_worker_protocol::PRODUCTION_WEAPON_LOW_QUAD_DRAFT_OPERATION => {
            production_low_retopology::run_quad_draft(payload)
        }
        forgecad_worker_protocol::PRODUCTION_WEAPON_CAGE_OFFSET_OPERATION => {
            production_cage_offset::run(payload)
        }
        forgecad_worker_protocol::PRODUCTION_WEAPON_GEOMETRIC_BAKE_OPERATION => {
            production_geometric_bake::run(payload)
        }
        forgecad_worker_protocol::PRODUCTION_WEAPON_HERO_MATERIAL_OPERATION => {
            production_hero_material::run(payload)
        }
        forgecad_worker_protocol::PRODUCTION_WEAPON_HERO_UV_LAYOUT_OPERATION => {
            production_hero_uv_layout::run(payload)
        }
        forgecad_worker_protocol::PRODUCTION_WEAPON_MATERIAL_LAYER_GRAPH_PLAN_OPERATION => {
            require_closed_payload(payload, &["material_layer_graph"])?;
            let graph = payload.get("material_layer_graph").ok_or_else(|| {
                GeometryError::Invalid("material_layer_graph is required".to_owned())
            })?;
            material_layer_graph::compile_material_layer_graph_result(graph)
                .map_err(|error| GeometryError::Invalid(error.to_string()))
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

const UV_ATLAS_RESOLUTION: f32 = 2048.0;
const UV_ATLAS_PADDING_TEXELS: f32 = 8.0;

#[derive(Clone, Copy)]
struct UvTriangle {
    source: [usize; 3],
    face: [f32; 3],
    projection_class: u8,
}

fn uv_projection_class(face: [f32; 3]) -> u8 {
    let absolute = [face[0].abs(), face[1].abs(), face[2].abs()];
    let axis = if absolute[1] >= absolute[0] && absolute[1] >= absolute[2] {
        1
    } else if absolute[0] >= absolute[2] {
        0
    } else {
        2
    };
    (axis * 2 + usize::from(face[axis] < 0.0)) as u8
}

fn project_uv_position(position: [f32; 3], projection_class: u8) -> [f32; 2] {
    match projection_class {
        0 => [position[2], position[1]],
        1 => [-position[2], position[1]],
        2 => [position[0], position[2]],
        3 => [position[0], -position[2]],
        4 => [position[0], position[1]],
        5 => [-position[0], position[1]],
        _ => unreachable!("projection class is closed to six axis directions"),
    }
}

fn dsu_find(parents: &mut [usize], mut index: usize) -> usize {
    let mut root = index;
    while parents[root] != root {
        root = parents[root];
    }
    while parents[index] != index {
        let next = parents[index];
        parents[index] = root;
        index = next;
    }
    root
}

fn dsu_union(parents: &mut [usize], left: usize, right: usize) {
    let left = dsu_find(parents, left);
    let right = dsu_find(parents, right);
    if left != right {
        let (first, second) = if left < right {
            (left, right)
        } else {
            (right, left)
        };
        parents[second] = first;
    }
}

fn split_overlapping_uv_islands(
    positions: &[[f32; 3]],
    triangles: &[UvTriangle],
    triangle_charts: &[usize],
) -> Vec<usize> {
    const MAX_OVERLAP_COMPARISONS_PER_ISLAND: usize = 1_000_000;
    let mut members = BTreeMap::<usize, Vec<usize>>::new();
    for (triangle_index, chart) in triangle_charts.iter().copied().enumerate() {
        members.entry(chart).or_default().push(triangle_index);
    }
    let mut split_charts = BTreeSet::<usize>::new();
    for (chart, triangle_indices) in &members {
        let mut comparisons = 0usize;
        'pairs: for left in 0..triangle_indices.len() {
            let left_triangle = triangles[triangle_indices[left]];
            let left_uvs = left_triangle.source.map(|source| {
                project_uv_position(positions[source], left_triangle.projection_class)
            });
            for right in (left + 1)..triangle_indices.len() {
                comparisons += 1;
                if comparisons > MAX_OVERLAP_COMPARISONS_PER_ISLAND {
                    split_charts.insert(*chart);
                    break 'pairs;
                }
                let right_triangle = triangles[triangle_indices[right]];
                let right_uvs = right_triangle.source.map(|source| {
                    project_uv_position(positions[source], right_triangle.projection_class)
                });
                if integrity::triangle_intersection_area(left_uvs, right_uvs) > 1.0e-10 {
                    split_charts.insert(*chart);
                    break 'pairs;
                }
            }
        }
    }
    if split_charts.is_empty() {
        return triangle_charts.to_vec();
    }
    let mut compact_good = BTreeMap::<usize, usize>::new();
    let mut next_chart = 0usize;
    let mut result = Vec::with_capacity(triangle_charts.len());
    for chart in triangle_charts {
        if split_charts.contains(chart) {
            result.push(next_chart);
            next_chart += 1;
        } else {
            let assigned = *compact_good.entry(*chart).or_insert_with(|| {
                let assigned = next_chart;
                next_chart += 1;
                assigned
            });
            result.push(assigned);
        }
    }
    result
}

/// Build deterministic, connected UV islands and MikkTSpace tangent vectors
/// from the actual candidate geometry. Adjacent triangles with the same
/// dominant signed projection share an island and therefore exact UV values
/// along their common edge. A projection change, disconnected component, or
/// Boolean face boundary is an explicit seam. Islands are packed into a
/// bounded 2048px grid with eight texels of padding; no path, script, random
/// source, or external unwrapping process participates in product truth.
///
/// Output vertices remain triangle-expanded so the existing strict topology
/// reader can preserve source lineage and face-normal boundaries. Continuity
/// is physical: duplicated vertices on a non-seam welded edge carry identical
/// TEXCOORD_0 values.
fn triangulate_uv_charts(
    positions: &[[f32; 3]],
    normals: &[[f32; 3]],
    indices: &[u32],
    force_face_normals: bool,
    continuous_uv: bool,
) -> Result<
    (
        Vec<[f32; 3]>,
        Vec<[f32; 3]>,
        Vec<[f32; 2]>,
        Vec<[f32; 4]>,
        Vec<u32>,
        usize,
        Vec<usize>,
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
    let triangle_count = indices.len() / 3;
    let mut triangles = Vec::with_capacity(triangle_count);
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
        let face_length = length3(face);
        if !finite3(face) || !face_length.is_finite() || face_length <= 1.0e-10 {
            return Err(GeometryError::Invalid(
                "cannot build UV/tangent data for a degenerate triangle".to_owned(),
            ));
        }
        let face = scale3(face, 1.0 / face_length);
        triangles.push(UvTriangle {
            source,
            face,
            projection_class: uv_projection_class(face),
        });
    }

    let mut parents = (0..triangle_count).collect::<Vec<_>>();
    if continuous_uv && !force_face_normals {
        let mut edge_owners = BTreeMap::<(usize, usize), Vec<usize>>::new();
        for (triangle_index, triangle) in triangles.iter().enumerate() {
            for edge in [(0, 1), (1, 2), (2, 0)] {
                let left = triangle.source[edge.0];
                let right = triangle.source[edge.1];
                let key = if left < right {
                    (left, right)
                } else {
                    (right, left)
                };
                edge_owners.entry(key).or_default().push(triangle_index);
            }
        }
        for owners in edge_owners.values() {
            if owners.len() == 2
                && triangles[owners[0]].projection_class == triangles[owners[1]].projection_class
            {
                dsu_union(&mut parents, owners[0], owners[1]);
            }
        }
    }
    let mut root_to_chart = BTreeMap::<usize, usize>::new();
    let mut triangle_charts = Vec::with_capacity(triangle_count);
    for triangle_index in 0..triangle_count {
        let root = if !continuous_uv || force_face_normals {
            triangle_index
        } else {
            dsu_find(&mut parents, triangle_index)
        };
        let next_chart = root_to_chart.len();
        let chart = *root_to_chart.entry(root).or_insert(next_chart);
        triangle_charts.push(chart);
    }
    if continuous_uv && !force_face_normals {
        triangle_charts = split_overlapping_uv_islands(positions, &triangles, &triangle_charts);
    }
    let chart_count = triangle_charts
        .iter()
        .copied()
        .max()
        .map(|value| value + 1)
        .unwrap_or(0);
    let columns = (chart_count as f32).sqrt().ceil().max(1.0) as usize;
    let rows = chart_count.div_ceil(columns).max(1);
    let cell_u = 1.0 / columns as f32;
    let cell_v = 1.0 / rows as f32;
    let atlas_resolution = if continuous_uv {
        UV_ATLAS_RESOLUTION
    } else {
        512.0
    };
    let padding_texels = if continuous_uv {
        UV_ATLAS_PADDING_TEXELS
    } else {
        4.0
    };
    let padding_u = (padding_texels / atlas_resolution).min(cell_u * 0.2);
    let padding_v = (padding_texels / atlas_resolution).min(cell_v * 0.2);

    let mut chart_bounds = vec![([f32::INFINITY; 2], [f32::NEG_INFINITY; 2]); chart_count];
    for (triangle, chart) in triangles.iter().zip(triangle_charts.iter().copied()) {
        for source in triangle.source {
            let projected = project_uv_position(positions[source], triangle.projection_class);
            for axis in 0..2 {
                chart_bounds[chart].0[axis] = chart_bounds[chart].0[axis].min(projected[axis]);
                chart_bounds[chart].1[axis] = chart_bounds[chart].1[axis].max(projected[axis]);
            }
        }
    }
    let mut chart_positions = Vec::with_capacity(indices.len());
    let mut chart_normals = Vec::with_capacity(indices.len());
    let mut uvs = Vec::with_capacity(indices.len());
    let mut chart_indices = Vec::with_capacity(indices.len());
    for (triangle_index, triangle) in triangles.iter().enumerate() {
        let source = triangle.source;
        let triangle_positions = source.map(|index| positions[index]);
        let face = triangle.face;
        let chart = triangle_charts[triangle_index];
        let chart_column = chart % columns;
        let chart_row = chart / columns;
        let (chart_min, chart_max) = chart_bounds[chart];
        let chart_extent = [chart_max[0] - chart_min[0], chart_max[1] - chart_min[1]];
        if continuous_uv
            && !force_face_normals
            && (chart_extent.iter().any(|value| !value.is_finite())
                || chart_extent[0] <= 1.0e-8
                || chart_extent[1] <= 1.0e-8)
        {
            return Err(GeometryError::Invalid(
                "continuous UV island projection has zero area".to_owned(),
            ));
        }
        let structural_uv_fallback = !continuous_uv
            && (chart_extent.iter().any(|value| !value.is_finite())
                || chart_extent[0] <= 1.0e-8
                || chart_extent[1] <= 1.0e-8);
        let projected_uvs = if continuous_uv {
            triangle_positions.map(|position| {
                let projected = project_uv_position(position, triangle.projection_class);
                [
                    (projected[0] - chart_min[0]) / chart_extent[0].max(1.0e-8),
                    (projected[1] - chart_min[1]) / chart_extent[1].max(1.0e-8),
                ]
            })
        } else if structural_uv_fallback {
            // Source-space precision can collapse one projected extent for a
            // valid very small triangle. The structural preview atlas owns an
            // isolated chart per triangle, so a fixed local triangle is a
            // deterministic, non-overlapping fallback with a valid tangent
            // frame. It is explicitly not the later authored Hero UV.
            [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]]
        } else {
            // The structural atlas gives every triangle its own chart. Scale
            // within that chart instead of the whole object bounds: imported
            // meshes can contain legitimate millimetre-scale faces on a
            // metre-scale weapon, and global normalization would collapse
            // those UVs below the tangent determinant tolerance. This remains
            // deterministic preview UV, not a claim of authored Hero UV.
            triangle_positions.map(|position| {
                let projected = project_uv_position(position, triangle.projection_class);
                [
                    (projected[0] - chart_min[0]) / chart_extent[0],
                    (projected[1] - chart_min[1]) / chart_extent[1],
                ]
            })
        };
        let triangle_uvs = projected_uvs.map(|uv| {
            [
                chart_column as f32 * cell_u + padding_u + uv[0] * (cell_u - 2.0 * padding_u),
                chart_row as f32 * cell_v + padding_v + uv[1] * (cell_v - 2.0 * padding_v),
            ]
        });
        let triangle_uvs = if force_face_normals || structural_uv_fallback {
            // Manifold can return machine-epsilon slivers. Boolean faces are
            // explicit seams, so a stable local triangle remains valid while
            // retaining a finite tangent frame and disjoint atlas cell.
            [
                [
                    chart_column as f32 * cell_u + padding_u,
                    chart_row as f32 * cell_v + padding_v,
                ],
                [
                    chart_column as f32 * cell_u + cell_u - padding_u,
                    chart_row as f32 * cell_v + padding_v,
                ],
                [
                    chart_column as f32 * cell_u + padding_u,
                    chart_row as f32 * cell_v + cell_v - padding_v,
                ],
            ]
        } else {
            triangle_uvs
        };
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
        for vertex in 0..3 {
            // Boolean intersections introduce intentional hard edges and can
            // place a shared vertex at a concave cut where an averaged
            // vertex normal points across the local face. Keep the Boolean
            // result's tangent frame fail-closed by using the actual face
            // normal for that chart; ordinary primitives retain their
            // authored/smoothed normals.
            let normal = if force_face_normals {
                face
            } else {
                normalize(normals[source[vertex]])
            };
            chart_positions.push(triangle_positions[vertex]);
            chart_normals.push(normal);
            uvs.push(triangle_uvs[vertex]);
            chart_indices.push((chart_indices.len()) as u32);
        }
    }

    let mut tangent_mesh = MikkTriangleMesh {
        positions: chart_positions,
        normals: chart_normals,
        uvs,
        tangents: vec![[0.0; 4]; triangle_count * 3],
    };
    if !mikktspace::generate_tangents(&mut tangent_mesh) {
        return Err(GeometryError::Invalid(
            "MikkTSpace rejected the UV chart mesh".to_owned(),
        ));
    }
    if tangent_mesh
        .tangents
        .iter()
        .any(|tangent| !tangent.iter().all(|component| component.is_finite()))
    {
        return Err(GeometryError::Invalid(
            "MikkTSpace produced a non-finite tangent".to_owned(),
        ));
    }
    if force_face_normals {
        // MikkTSpace can leave one vertex of a machine-epsilon sliver chart
        // at its zero initializer even though the chart has a finite UV
        // frame. Keep Mikk's result wherever it is valid, and repair only
        // that missing vector from the same triangle/UV basis that strict
        // readback uses.
        for triangle_index in 0..triangle_count {
            let base = triangle_index * 3;
            let triangle_positions = [
                tangent_mesh.positions[base],
                tangent_mesh.positions[base + 1],
                tangent_mesh.positions[base + 2],
            ];
            let triangle_normals = [
                tangent_mesh.normals[base],
                tangent_mesh.normals[base + 1],
                tangent_mesh.normals[base + 2],
            ];
            let triangle_uvs = [
                tangent_mesh.uvs[base],
                tangent_mesh.uvs[base + 1],
                tangent_mesh.uvs[base + 2],
            ];
            for vertex in 0..3 {
                if let Some(fallback) = tangent_from_uv_frame(
                    triangle_positions,
                    triangle_normals[vertex],
                    triangle_uvs,
                ) {
                    tangent_mesh.tangents[base + vertex] = fallback;
                }
            }
        }
    }
    Ok((
        tangent_mesh.positions,
        tangent_mesh.normals,
        tangent_mesh.uvs,
        tangent_mesh.tangents,
        chart_indices,
        chart_count,
        if continuous_uv {
            triangle_charts
        } else {
            Vec::new()
        },
    ))
}

fn repack_continuous_uv_atlas(parts: &mut [PartMesh]) -> Result<(), GeometryError> {
    let total_chart_count = parts
        .iter()
        .flat_map(|part| &part.sources)
        .map(|source| source.uv_chart_count)
        .sum::<usize>();
    if total_chart_count == 0 {
        return Err(GeometryError::Invalid(
            "continuous UV atlas has no charts".to_owned(),
        ));
    }
    let columns = (total_chart_count as f32).sqrt().ceil().max(1.0) as usize;
    let rows = total_chart_count.div_ceil(columns).max(1);
    let cell_u = 1.0 / columns as f32;
    let cell_v = 1.0 / rows as f32;
    let padding = UV_ATLAS_PADDING_TEXELS / UV_ATLAS_RESOLUTION;
    if cell_u <= 2.0 * padding || cell_v <= 2.0 * padding {
        return Err(GeometryError::Invalid(
            "continuous UV atlas cannot preserve its fixed eight-texel padding budget".to_owned(),
        ));
    }

    let mut chart_offset = 0usize;
    for source in parts.iter_mut().flat_map(|part| &mut part.sources) {
        if source.uv_chart_count == 0
            || source.uv_chart_ids.len() * 3 != source.uvs.len()
            || source.positions.len() != source.uvs.len()
            || source.normals.len() != source.uvs.len()
        {
            return Err(GeometryError::Invalid(
                "continuous UV source chart metadata is incomplete".to_owned(),
            ));
        }
        let local_columns = (source.uv_chart_count as f32).sqrt().ceil().max(1.0) as usize;
        let local_rows = source.uv_chart_count.div_ceil(local_columns).max(1);
        let local_cell_u = 1.0 / local_columns as f32;
        let local_cell_v = 1.0 / local_rows as f32;
        let local_padding_u = (padding).min(local_cell_u * 0.2);
        let local_padding_v = (padding).min(local_cell_v * 0.2);
        let local_inner_u = local_cell_u - 2.0 * local_padding_u;
        let local_inner_v = local_cell_v - 2.0 * local_padding_v;
        if local_inner_u <= 0.0 || local_inner_v <= 0.0 {
            return Err(GeometryError::Invalid(
                "continuous UV local chart has no padded interior".to_owned(),
            ));
        }
        for (triangle_index, local_chart) in source.uv_chart_ids.iter_mut().enumerate() {
            if *local_chart >= source.uv_chart_count {
                return Err(GeometryError::Invalid(
                    "continuous UV chart id is out of range".to_owned(),
                ));
            }
            let local_column = *local_chart % local_columns;
            let local_row = *local_chart / local_columns;
            let global_chart = chart_offset + *local_chart;
            let global_column = global_chart % columns;
            let global_row = global_chart / columns;
            for vertex in 0..3 {
                let uv_index = triangle_index * 3 + vertex;
                let local = source.uvs[uv_index];
                let normalized = [
                    (local[0] - (local_column as f32 * local_cell_u + local_padding_u))
                        / local_inner_u,
                    (local[1] - (local_row as f32 * local_cell_v + local_padding_v))
                        / local_inner_v,
                ];
                if normalized
                    .iter()
                    .any(|value| !value.is_finite() || *value < -1.0e-5 || *value > 1.00001)
                {
                    return Err(GeometryError::Invalid(
                        "continuous UV chart normalization escaped its source cell".to_owned(),
                    ));
                }
                source.uvs[uv_index] = [
                    global_column as f32 * cell_u
                        + padding
                        + normalized[0].clamp(0.0, 1.0) * (cell_u - 2.0 * padding),
                    global_row as f32 * cell_v
                        + padding
                        + normalized[1].clamp(0.0, 1.0) * (cell_v - 2.0 * padding),
                ];
            }
            *local_chart = global_chart;
        }
        regenerate_mikk_tangents(source)?;
        chart_offset += source.uv_chart_count;
    }
    if chart_offset != total_chart_count {
        return Err(GeometryError::Invalid(
            "continuous UV atlas chart accounting drifted".to_owned(),
        ));
    }
    Ok(())
}

fn regenerate_mikk_tangents(source: &mut PartSourceMesh) -> Result<(), GeometryError> {
    let mut mesh = MikkTriangleMesh {
        positions: source.positions.clone(),
        normals: source.normals.clone(),
        uvs: source.uvs.clone(),
        tangents: vec![[0.0; 4]; source.positions.len()],
    };
    if !mikktspace::generate_tangents(&mut mesh) {
        return Err(GeometryError::Invalid(
            "MikkTSpace rejected the globally packed continuous UV atlas".to_owned(),
        ));
    }
    for triangle_index in 0..(mesh.positions.len() / 3) {
        let base = triangle_index * 3;
        let positions = [
            mesh.positions[base],
            mesh.positions[base + 1],
            mesh.positions[base + 2],
        ];
        let uvs = [mesh.uvs[base], mesh.uvs[base + 1], mesh.uvs[base + 2]];
        for vertex in 0..3 {
            let tangent = mesh.tangents[base + vertex];
            if !tangent.iter().all(|value| value.is_finite()) {
                return Err(GeometryError::Invalid(
                    "MikkTSpace produced a non-finite continuous UV tangent".to_owned(),
                ));
            }
            // MikkTSpace groups coincident position/normal/UV corners across
            // a continuous island. The strict ForgeCAD profile validates the
            // physical tangent against every decoded triangle, so finalize
            // each expanded corner from that same finite Mikk input frame.
            // This prevents an averaged corner from carrying another
            // triangle's basis while keeping the pinned Mikk admission pass.
            mesh.tangents[base + vertex] = tangent_from_uv_frame(
                positions,
                mesh.normals[base + vertex],
                uvs,
            )
            .ok_or_else(|| {
                GeometryError::Invalid("continuous UV tangent frame is degenerate".to_owned())
            })?;
        }
    }
    source.tangents = mesh.tangents;
    Ok(())
}

fn tangent_from_uv_frame(
    positions: [[f32; 3]; 3],
    normal: [f32; 3],
    uvs: [[f32; 2]; 3],
) -> Option<[f32; 4]> {
    let edge_a = subtract3(positions[1], positions[0]);
    let edge_b = subtract3(positions[2], positions[0]);
    let uv_a = [uvs[1][0] - uvs[0][0], uvs[1][1] - uvs[0][1]];
    let uv_b = [uvs[2][0] - uvs[0][0], uvs[2][1] - uvs[0][1]];
    let uv_area = uv_a[0] * uv_b[1] - uv_a[1] * uv_b[0];
    if !uv_area.is_finite() || uv_area.abs() <= 1.0e-8 {
        return None;
    }
    let reciprocal = 1.0 / uv_area;
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
    let normal = normalize(normal);
    let tangent_projected = subtract3(tangent_basis, scale3(normal, dot3(normal, tangent_basis)));
    let bitangent_projected = subtract3(
        bitangent_basis,
        scale3(normal, dot3(normal, bitangent_basis)),
    );
    let tangent_length = length3(tangent_projected);
    let bitangent_length = length3(bitangent_projected);
    if !finite3(tangent_projected)
        || !finite3(bitangent_projected)
        || !tangent_length.is_finite()
        || !bitangent_length.is_finite()
        || tangent_length <= 1.0e-12
        || bitangent_length <= 1.0e-12
    {
        return None;
    }
    let tangent = scale3(tangent_projected, 1.0 / tangent_length);
    let bitangent = scale3(bitangent_projected, 1.0 / bitangent_length);
    let orientation = dot3(cross3(normal, tangent), bitangent);
    let sign = if orientation.is_finite() && orientation < 0.0 {
        -1.0
    } else {
        1.0
    };
    Some([tangent[0], tangent[1], tangent[2], sign])
}

struct MikkTriangleMesh {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    tangents: Vec<[f32; 4]>,
}

impl mikktspace::Geometry for MikkTriangleMesh {
    fn num_faces(&self) -> usize {
        self.positions.len() / 3
    }

    fn num_vertices_of_face(&self, _face: usize) -> usize {
        3
    }

    fn position(&self, face: usize, vertex: usize) -> [f32; 3] {
        self.positions[face * 3 + vertex]
    }

    fn normal(&self, face: usize, vertex: usize) -> [f32; 3] {
        self.normals[face * 3 + vertex]
    }

    fn tex_coord(&self, face: usize, vertex: usize) -> [f32; 2] {
        self.uvs[face * 3 + vertex]
    }

    fn set_tangent_encoded(&mut self, tangent: [f32; 4], face: usize, vertex: usize) {
        self.tangents[face * 3 + vertex] = tangent;
    }
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
    let schema_version = object.get("schema_version").and_then(Value::as_str);
    if !matches!(
        schema_version,
        Some("AppearanceProgram@1" | "AppearanceProgram@2" | "AppearanceProgram@3")
    ) {
        return Err(GeometryError::Invalid(
            "appearance schema_version must be AppearanceProgram@1, AppearanceProgram@2 or AppearanceProgram@3"
                .to_owned(),
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
    let max_zones = if matches!(
        schema_version,
        Some("AppearanceProgram@2" | "AppearanceProgram@3")
    ) {
        64
    } else {
        32
    };
    if zones.is_empty() || zones.len() > max_zones {
        return Err(GeometryError::Invalid(
            "appearance material_zones is outside its budget".to_owned(),
        ));
    }
    if schema_version == Some("AppearanceProgram@2") {
        const APPEARANCE_V2_KEYS: &[&str] = &[
            "schema_version",
            "project_id",
            "geometry_program_sha256",
            "material_pack_id",
            "material_pack_manifest_sha256",
            "material_zones",
            "canonical_sha256",
        ];
        if object
            .keys()
            .any(|key| !APPEARANCE_V2_KEYS.contains(&key.as_str()))
        {
            return Err(GeometryError::Invalid(
                "AppearanceProgram@2 contains unknown fields".to_owned(),
            ));
        }
        return validate_appearance_v2(object, geometry_program_sha256, zones);
    }
    if schema_version == Some("AppearanceProgram@3") {
        const APPEARANCE_V3_KEYS: &[&str] = &[
            "schema_version",
            "project_id",
            "geometry_program_sha256",
            "material_pack_id",
            "material_pack_manifest_sha256",
            "material_zones",
            "material_layer_stack",
            "material_layer_stack_sha256",
            "canonical_sha256",
        ];
        if object
            .keys()
            .any(|key| !APPEARANCE_V3_KEYS.contains(&key.as_str()))
        {
            return Err(GeometryError::Invalid(
                "AppearanceProgram@3 contains unknown fields".to_owned(),
            ));
        }
        let stack = validate_material_layer_stack(object, zones)?;
        let stack_sha256 = stack
            .get("canonical_sha256")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                GeometryError::Invalid("MaterialLayerStack@1 hash is missing".to_owned())
            })?;
        let mut result = validate_appearance_v2(object, geometry_program_sha256, zones)?;
        for material in result.values_mut() {
            material["extras"]["forgecad"]["material_layer_stack_sha256"] =
                Value::String(stack_sha256.to_owned());
            material["extras"]["forgecad"]["material_layer_stack"] = stack.clone();
        }
        return Ok(result);
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

fn validate_material_layer_stack(
    appearance: &Map<String, Value>,
    zones: &[Value],
) -> Result<Value, GeometryError> {
    let stack = appearance
        .get("material_layer_stack")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            GeometryError::Invalid(
                "AppearanceProgram@3 material_layer_stack is required".to_owned(),
            )
        })?;
    const STACK_KEYS: &[&str] = &[
        "schema_version",
        "stack_id",
        "material_pack_id",
        "material_pack_manifest_sha256",
        "uv_source",
        "layers",
        "budget",
        "canonical_sha256",
    ];
    if stack.keys().any(|key| !STACK_KEYS.contains(&key.as_str()))
        || stack.get("schema_version").and_then(Value::as_str) != Some("MaterialLayerStack@1")
        || stack.get("material_pack_id") != appearance.get("material_pack_id")
        || stack.get("material_pack_manifest_sha256")
            != appearance.get("material_pack_manifest_sha256")
        || stack.get("uv_source").and_then(Value::as_str) != Some("TEXCOORD_0")
    {
        return Err(GeometryError::Invalid(
            "MaterialLayerStack@1 identity or pack binding is invalid".to_owned(),
        ));
    }
    let declared_hash = stack
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            GeometryError::Invalid("MaterialLayerStack@1 canonical hash is required".to_owned())
        })?;
    let mut preimage = stack.clone();
    preimage.remove("canonical_sha256");
    if canonical_hash(&Value::Object(preimage)) != declared_hash
        || appearance
            .get("material_layer_stack_sha256")
            .and_then(Value::as_str)
            != Some(declared_hash)
    {
        return Err(GeometryError::Invalid(
            "MaterialLayerStack@1 canonical binding is invalid".to_owned(),
        ));
    }
    let budget = stack
        .get("budget")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            GeometryError::Invalid("MaterialLayerStack@1 budget is required".to_owned())
        })?;
    const BUDGET_KEYS: &[&str] = &[
        "resolution",
        "padding_texels",
        "max_output_textures",
        "max_output_bytes",
        "max_runtime_ms",
    ];
    if budget
        .keys()
        .any(|key| !BUDGET_KEYS.contains(&key.as_str()))
        || budget.get("resolution").and_then(Value::as_u64) != Some(2048)
        || budget.get("padding_texels").and_then(Value::as_u64) != Some(8)
        || budget.get("max_output_textures").and_then(Value::as_u64) != Some(8)
        || budget.get("max_output_bytes").and_then(Value::as_u64) != Some(67_108_864)
        || budget.get("max_runtime_ms").and_then(Value::as_u64) != Some(120_000)
    {
        return Err(GeometryError::Invalid(
            "MaterialLayerStack@1 budget drifted".to_owned(),
        ));
    }
    let layers = stack
        .get("layers")
        .and_then(Value::as_array)
        .filter(|layers| layers.len() == 3)
        .ok_or_else(|| {
            GeometryError::Invalid("MaterialLayerStack@1 requires three layers".to_owned())
        })?;
    let expected = [
        ("decal", "forgecad-first-party-fictional-safety-markings@1"),
        ("wear", "forgecad-first-party-geometry-edge-ao-wear@1"),
        ("clearcoat", "forgecad-first-party-zone-clearcoat-mask@1"),
    ];
    let known_zones = zones
        .iter()
        .filter_map(|zone| zone.get("zone_id").and_then(Value::as_str))
        .collect::<BTreeSet<_>>();
    let known_parts = zones
        .iter()
        .filter_map(Value::as_object)
        .filter_map(|zone| zone.get("part_ids").and_then(Value::as_array))
        .flatten()
        .filter_map(Value::as_str)
        .collect::<BTreeSet<_>>();
    for (order, (layer, (kind, recipe_id))) in layers.iter().zip(expected).enumerate() {
        let layer = layer.as_object().ok_or_else(|| {
            GeometryError::Invalid("MaterialLayerStack@1 layer is invalid".to_owned())
        })?;
        if layer.get("order").and_then(Value::as_u64) != Some(order as u64)
            || layer.get("kind").and_then(Value::as_str) != Some(kind)
            || layer.get("recipe_id").and_then(Value::as_str) != Some(recipe_id)
            || layer.keys().any(|key| {
                matches!(
                    key.as_str(),
                    "script" | "shader" | "url" | "path" | "plugin" | "expression"
                )
            })
        {
            return Err(GeometryError::Invalid(
                "MaterialLayerStack@1 layer order, recipe or safety boundary is invalid".to_owned(),
            ));
        }
        let targets = layer
            .get("targets")
            .and_then(Value::as_object)
            .ok_or_else(|| {
                GeometryError::Invalid("MaterialLayerStack@1 targets are required".to_owned())
            })?;
        let target_zones = targets
            .get("material_zone_ids")
            .and_then(Value::as_array)
            .filter(|values| !values.is_empty())
            .ok_or_else(|| {
                GeometryError::Invalid("MaterialLayerStack@1 target zones are required".to_owned())
            })?;
        if target_zones
            .iter()
            .any(|value| value.as_str().is_none_or(|id| !known_zones.contains(id)))
            || targets
                .get("part_ids")
                .and_then(Value::as_array)
                .is_none_or(|values| {
                    values
                        .iter()
                        .any(|value| value.as_str().is_none_or(|id| !known_parts.contains(id)))
                })
        {
            return Err(GeometryError::Invalid(
                "MaterialLayerStack@1 targets are not bound to Appearance material zones"
                    .to_owned(),
            ));
        }
    }
    Ok(Value::Object(stack.clone()))
}

fn validate_appearance_v2(
    object: &Map<String, Value>,
    geometry_program_sha256: &str,
    zones: &[Value],
) -> Result<HashMap<String, Value>, GeometryError> {
    let pack_id = object
        .get("material_pack_id")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            GeometryError::Invalid("AppearanceProgram@2 material_pack_id is required".to_owned())
        })?;
    let manifest = material_pack_manifest_by_id(pack_id).ok_or_else(|| {
        GeometryError::Invalid(
            "AppearanceProgram@2 material_pack_id is not compile-time allowlisted".to_owned(),
        )
    })?;
    validate_material_pack_texture_inventory(pack_id, &manifest)?;
    let manifest_sha256 = object
        .get("material_pack_manifest_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            GeometryError::Invalid(
                "AppearanceProgram@2 material_pack_manifest_sha256 is required".to_owned(),
            )
        })?;
    if material_pack_manifest_sha256_by_id(pack_id).as_deref() != Some(manifest_sha256) {
        return Err(GeometryError::Invalid(
            "AppearanceProgram@2 material pack manifest hash does not match the offline pack"
                .to_owned(),
        ));
    }
    if object
        .get("geometry_program_sha256")
        .and_then(Value::as_str)
        != Some(geometry_program_sha256)
    {
        return Err(GeometryError::Invalid(
            "AppearanceProgram@2 is not bound to the geometry program".to_owned(),
        ));
    }
    let definitions = manifest
        .get("material_definitions")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            GeometryError::Invalid("offline material definitions are missing".to_owned())
        })?;
    let texture_sets = manifest
        .get("texture_sets")
        .and_then(Value::as_array)
        .ok_or_else(|| GeometryError::Invalid("offline texture sets are missing".to_owned()))?;
    let mut result = HashMap::new();
    for zone in zones {
        let zone = zone
            .as_object()
            .ok_or_else(|| GeometryError::Invalid("material zone must be an object".to_owned()))?;
        const ZONE_KEYS: &[&str] = &["zone_id", "part_ids", "material_id", "texture_set_id"];
        if zone.keys().any(|key| !ZONE_KEYS.contains(&key.as_str())) {
            return Err(GeometryError::Invalid(
                "AppearanceProgram@2 material zone contains unknown fields".to_owned(),
            ));
        }
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
        if part_ids.is_empty()
            || part_ids.len() > 512
            || part_ids.iter().any(|value| value.as_str().is_none())
        {
            return Err(GeometryError::Invalid(
                "AppearanceProgram@2 material zone part_ids is invalid".to_owned(),
            ));
        }
        let material_id = required_text(zone, "material_id")?;
        let definition = definitions
            .iter()
            .find(|definition| {
                definition.get("material_id").and_then(Value::as_str) == Some(material_id)
            })
            .ok_or_else(|| GeometryError::Invalid(format!("unknown material_id: {material_id}")))?;
        let expected_texture_set = definition
            .get("texture_set_id")
            .cloned()
            .unwrap_or(Value::Null);
        let requested_texture_set = zone.get("texture_set_id").cloned().unwrap_or(Value::Null);
        if requested_texture_set != expected_texture_set {
            return Err(GeometryError::Invalid(
                "AppearanceProgram@2 texture_set_id does not match the material definition"
                    .to_owned(),
            ));
        }
        if let Some(texture_set_id) = requested_texture_set.as_str() {
            if !texture_sets.iter().any(|texture_set| {
                texture_set.get("texture_set_id").and_then(Value::as_str) == Some(texture_set_id)
            }) {
                return Err(GeometryError::Invalid(
                    "AppearanceProgram@2 references an unknown texture set".to_owned(),
                ));
            }
        }
        let mut material = pack_material_json(
            definition,
            requested_texture_set.as_str(),
            pack_id,
            manifest_sha256,
        );
        // glTF material names mirror the semantic MaterialZone so the strict
        // readback can prove every triangle's binding without trusting an
        // external pack lookup. The stable material_id remains in extras.
        material["name"] = Value::String(zone_id.clone());
        result.insert(zone_id, material);
    }
    Ok(result)
}

fn validate_material_pack_texture_inventory(
    pack_id: &str,
    manifest: &Value,
) -> Result<(), GeometryError> {
    if pack_id == FICTIONAL_ENERGY_WEAPON_2K_PACK_ID {
        let sources = manifest
            .get("source_textures")
            .and_then(Value::as_array)
            .filter(|textures| textures.len() == 7)
            .ok_or_else(|| {
                GeometryError::Invalid("2K material source inventory is invalid".to_owned())
            })?;
        for source in sources {
            let texture_id = source
                .get("texture_id")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    GeometryError::Invalid("2K material source ID is invalid".to_owned())
                })?;
            let bytes = weapon_source_texture_bytes(texture_id)?;
            let decoded = image::load_from_memory(bytes).map_err(|error| {
                GeometryError::Invalid(format!(
                    "2K material source decode failed: {texture_id}: {error}"
                ))
            })?;
            let actual_sha256 = sha256_hex(bytes);
            if source.get("source_pack_id").and_then(Value::as_str)
                != Some("forgecad-fictional-energy-weapon")
                || source.get("sha256").and_then(Value::as_str) != Some(actual_sha256.as_str())
                || source.get("width").and_then(Value::as_u64) != Some(decoded.width() as u64)
                || source.get("height").and_then(Value::as_u64) != Some(decoded.height() as u64)
            {
                return Err(GeometryError::Invalid(format!(
                    "2K material source bytes drifted: {texture_id}"
                )));
            }
        }
        return Ok(());
    }
    let textures = manifest
        .get("textures")
        .and_then(Value::as_array)
        .filter(|textures| !textures.is_empty() && textures.len() <= 64)
        .ok_or_else(|| {
            GeometryError::Invalid("offline material pack texture inventory is invalid".to_owned())
        })?;
    let mut texture_ids = BTreeSet::new();
    for texture in textures {
        let texture = texture.as_object().ok_or_else(|| {
            GeometryError::Invalid("offline material pack texture entry is invalid".to_owned())
        })?;
        let texture_id = required_text(texture, "texture_id")?;
        if !texture_ids.insert(texture_id.to_owned()) {
            return Err(GeometryError::Invalid(
                "offline material pack texture IDs must be unique".to_owned(),
            ));
        }
        let expected_sha256 = texture
            .get("sha256")
            .and_then(Value::as_str)
            .filter(|value| {
                value.len() == 64
                    && value
                        .bytes()
                        .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
            })
            .ok_or_else(|| {
                GeometryError::Invalid(
                    "offline material pack texture SHA-256 is invalid".to_owned(),
                )
            })?;
        let expected_width = texture
            .get("width")
            .and_then(Value::as_u64)
            .filter(|value| (1..=8192).contains(value))
            .ok_or_else(|| {
                GeometryError::Invalid("offline material pack texture width is invalid".to_owned())
            })?;
        let expected_height = texture
            .get("height")
            .and_then(Value::as_u64)
            .filter(|value| (1..=8192).contains(value))
            .ok_or_else(|| {
                GeometryError::Invalid("offline material pack texture height is invalid".to_owned())
            })?;
        let bytes = pack_texture_bytes(pack_id, texture_id)?;
        let actual_sha256 = Sha256::digest(&bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        if actual_sha256 != expected_sha256 {
            return Err(GeometryError::Invalid(format!(
                "offline material pack texture hash drifted: {texture_id}"
            )));
        }
        let decoded = image::load_from_memory(&bytes).map_err(|error| {
            GeometryError::Invalid(format!(
                "offline material pack texture decode failed: {texture_id}: {error}"
            ))
        })?;
        if u64::from(decoded.width()) != expected_width
            || u64::from(decoded.height()) != expected_height
        {
            return Err(GeometryError::Invalid(format!(
                "offline material pack texture dimensions drifted: {texture_id}"
            )));
        }
    }
    Ok(())
}

fn pack_material_json(
    definition: &Value,
    texture_set_id: Option<&str>,
    material_pack_id: &str,
    material_pack_manifest_sha256: &str,
) -> Value {
    let material_id = definition
        .get("material_id")
        .and_then(Value::as_str)
        .unwrap_or("unknown-material");
    let base_color = definition
        .get("base_color_factor")
        .cloned()
        .unwrap_or_else(|| json!([0.7, 0.7, 0.7, 1.0]));
    let metallic = definition
        .get("metallic_factor")
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    let roughness = definition
        .get("roughness_factor")
        .and_then(Value::as_f64)
        .unwrap_or(0.5);
    let emissive = definition
        .get("emissive_factor")
        .cloned()
        .unwrap_or_else(|| json!([0.0, 0.0, 0.0]));
    let clearcoat = definition
        .get("clearcoat_factor")
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    let emissive_strength = definition
        .get("emissive_strength")
        .and_then(Value::as_f64)
        .unwrap_or(1.0);
    let texture_keys = match texture_set_id {
        Some("metal-surface" | "weapon-metal-surface") => json!({
            "base_color":"metal010_color",
            "normal":"metal010_normal_gl",
            "metallic_roughness":"metal010_metallic_roughness"
        }),
        // The bundled Plastic006 color map is a black engineering-plastic
        // surface.  The white dielectric armor uses the same texture set for
        // its normal/roughness provenance, but must keep its authored white
        // baseColorFactor instead of multiplying by a black albedo.
        Some("plastic-surface" | "weapon-plastic-surface")
            if matches!(
                material_id,
                "white-dielectric-clearcoat" | "energy-white-clearcoat"
            ) =>
        {
            json!({
                "normal":"plastic006_normal_gl",
                "metallic_roughness":"plastic006_metallic_roughness"
            })
        }
        Some("plastic-surface" | "weapon-plastic-surface") => json!({
            "base_color":"plastic006_color",
            "normal":"plastic006_normal_gl",
            "metallic_roughness":"plastic006_metallic_roughness"
        }),
        _ => json!({}),
    };
    let mut material = json!({
        "name":material_id,
        "pbrMetallicRoughness":{"baseColorFactor":base_color,"metallicFactor":metallic,"roughnessFactor":roughness},
        "emissiveFactor":emissive,
        "extras":{"forgecad":{"material_pack_id":material_pack_id,"material_pack_manifest_sha256":material_pack_manifest_sha256,"material_id":material_id,"texture_set_id":texture_set_id,"texture_keys":texture_keys}}
    });
    if clearcoat > 0.0 {
        material["extensions"] = json!({
            "KHR_materials_clearcoat":{"clearcoatFactor":clearcoat},
            "KHR_materials_emissive_strength":{"emissiveStrength":emissive_strength}
        });
    } else if emissive_strength > 1.0 {
        material["extensions"] = json!({
            "KHR_materials_emissive_strength":{"emissiveStrength":emissive_strength}
        });
    }
    material
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
    appearance: Option<&Value>,
) -> Result<Vec<u8>, GeometryError> {
    let mut binary = Vec::new();
    let mut buffer_views = Vec::new();
    let mut accessors = Vec::new();
    let mut meshes = Vec::new();
    let mut nodes = Vec::new();
    let mut materials = Vec::new();
    let mut material_texture_keys = Vec::<Map<String, Value>>::new();
    let mut texture_keys = Vec::<String>::new();
    let mut selected_material_pack_id: Option<String> = None;
    let mut selected_material_pack_manifest_sha256: Option<String> = None;
    let deduplicate_material_zones = appearance
        .and_then(|value| value.get("schema_version"))
        .and_then(Value::as_str)
        .is_some_and(|value| matches!(value, "AppearanceProgram@2" | "AppearanceProgram@3"));
    for (mesh_index, part) in parts.iter().enumerate() {
        let material = part.material.clone();
        if let Some(metadata) = material
            .get("extras")
            .and_then(|value| value.get("forgecad"))
            .and_then(Value::as_object)
        {
            if let Some(pack_id) = metadata.get("material_pack_id").and_then(Value::as_str) {
                let manifest_sha256 = metadata
                    .get("material_pack_manifest_sha256")
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        GeometryError::Invalid(
                            "material pack metadata lacks its manifest hash".to_owned(),
                        )
                    })?;
                if selected_material_pack_id
                    .as_deref()
                    .is_some_and(|selected| selected != pack_id)
                    || selected_material_pack_manifest_sha256
                        .as_deref()
                        .is_some_and(|selected| selected != manifest_sha256)
                {
                    return Err(GeometryError::Invalid(
                        "one GLB cannot mix MaterialPack identities".to_owned(),
                    ));
                }
                selected_material_pack_id = Some(pack_id.to_owned());
                selected_material_pack_manifest_sha256 = Some(manifest_sha256.to_owned());
            }
        }
        let keys = material
            .get("extras")
            .and_then(|value| value.get("forgecad"))
            .and_then(|value| value.get("texture_keys"))
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        for key in keys.values().filter_map(Value::as_str) {
            if !texture_keys.iter().any(|existing| existing == key) {
                texture_keys.push(key.to_owned());
            }
        }
        let material_index = if deduplicate_material_zones {
            if let Some(index) = materials.iter().position(|existing| existing == &material) {
                if material_texture_keys.get(index) != Some(&keys) {
                    return Err(GeometryError::Invalid(
                        "one MaterialZone cannot resolve to different texture bindings".to_owned(),
                    ));
                }
                index
            } else {
                let index = materials.len();
                material_texture_keys.push(keys);
                materials.push(material);
                index
            }
        } else {
            let index = materials.len();
            material_texture_keys.push(keys);
            materials.push(material);
            index
        };
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
            let mut primitive_extras = json!({
                "part_id":part.part_id,
                "source_node_id":source.source_node_id,
                "lineage_source_node_ids":source.lineage_source_node_ids,
                "operator_id":source.operator_id,
                "material_zone_id":part.material_zone_id,
                "solid":part.solid,
            });
            if !source.uv_chart_ids.is_empty() {
                let chart_ids = Value::Array(
                    source
                        .uv_chart_ids
                        .iter()
                        .map(|value| Value::from(*value as u64))
                        .collect(),
                );
                primitive_extras["uv_chart_assignment_sha256"] =
                    Value::String(canonical_hash(&chart_ids));
                primitive_extras["uv_chart_ids"] = chart_ids;
            }
            primitives.push(json!({
                "attributes":{"POSITION":pos_accessor,"NORMAL":norm_accessor,"TEXCOORD_0":uv_accessor,"TANGENT":tangent_accessor},
                "indices":index_accessor,
                "material":material_index,
                "extras":primitive_extras
            }));
        }
        meshes.push(
            json!({"name":part.part_id,"primitives":primitives,"extras":part_lineage.clone()}),
        );
        nodes.push(json!({"name":part.part_id,"mesh":mesh_index,"extras":part_lineage}));
    }
    let material_layer_stack = parts.iter().find_map(|part| {
        part.material
            .get("extras")
            .and_then(|value| value.get("forgecad"))
            .and_then(|value| value.get("material_layer_stack"))
            .cloned()
    });
    let surface_bake = material_layer_stack
        .as_ref()
        .map(|stack| surface_bake::build(parts, stack))
        .transpose()?;
    if let Some(bake) = &surface_bake {
        for key in [
            surface_bake::BASE_COLOR_ID,
            surface_bake::NORMAL_ID,
            surface_bake::METALLIC_ROUGHNESS_ID,
            surface_bake::AO_ID,
            surface_bake::CLEARCOAT_ID,
            surface_bake::CLEARCOAT_ROUGHNESS_ID,
        ] {
            if !texture_keys.iter().any(|existing| existing == key) {
                texture_keys.push(key.to_owned());
            }
        }
        for (material, keys) in materials.iter_mut().zip(material_texture_keys.iter_mut()) {
            keys.insert(
                "base_color".to_owned(),
                Value::String(surface_bake::BASE_COLOR_ID.to_owned()),
            );
            keys.insert(
                "normal".to_owned(),
                Value::String(surface_bake::NORMAL_ID.to_owned()),
            );
            keys.insert(
                "metallic_roughness".to_owned(),
                Value::String(surface_bake::METALLIC_ROUGHNESS_ID.to_owned()),
            );
            keys.insert(
                "ao".to_owned(),
                Value::String(surface_bake::AO_ID.to_owned()),
            );
            let zone_id = material
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or_default();
            if bake.clearcoat_zone_ids.contains(zone_id) {
                keys.insert(
                    "clearcoat".to_owned(),
                    Value::String(surface_bake::CLEARCOAT_ID.to_owned()),
                );
                keys.insert(
                    "clearcoat_roughness".to_owned(),
                    Value::String(surface_bake::CLEARCOAT_ROUGHNESS_ID.to_owned()),
                );
            }
            material["extras"]["forgecad"]["texture_keys"] = Value::Object(keys.clone());
            material["extras"]["forgecad"]["surface_bake_sha256"] =
                bake.metadata["canonical_sha256"].clone();
        }
    }
    let mut images = Vec::new();
    let mut textures = Vec::new();
    let mut texture_indices = HashMap::<String, usize>::new();
    let mut embedded_texture_outputs = Vec::new();
    for key in &texture_keys {
        let pack_id = selected_material_pack_id.as_deref().ok_or_else(|| {
            GeometryError::Invalid("textured material lacks a MaterialPack identity".to_owned())
        })?;
        let bytes = surface_bake
            .as_ref()
            .and_then(|bake| bake.outputs.iter().find(|output| output.texture_id == *key))
            .map(|output| output.bytes.clone())
            .map(Ok)
            .unwrap_or_else(|| pack_texture_bytes(pack_id, key))?;
        if bytes.len() > 16 * 1024 * 1024 {
            return Err(GeometryError::Invalid(
                "offline material pack texture exceeds its per-image bound".to_owned(),
            ));
        }
        while binary.len() % 4 != 0 {
            binary.push(0);
        }
        let offset = binary.len();
        binary.extend_from_slice(&bytes);
        while binary.len() % 4 != 0 {
            binary.push(0);
        }
        let stored_length = binary.len() - offset;
        let view = buffer_views.len();
        buffer_views.push(json!({"buffer":0,"byteOffset":offset,"byteLength":stored_length}));
        let image_index = images.len();
        images.push(json!({"bufferView":view,"mimeType":"image/png","name":key}));
        textures.push(json!({"source":image_index}));
        texture_indices.insert(key.clone(), textures.len() - 1);
        if pack_id == FICTIONAL_ENERGY_WEAPON_2K_PACK_ID
            && surface_bake
                .as_ref()
                .is_none_or(|bake| !bake.outputs.iter().any(|output| output.texture_id == *key))
        {
            let decoded = image::load_from_memory(&bytes).map_err(|error| {
                GeometryError::Invalid(format!("2K embedded PNG decode failed: {key}: {error}"))
            })?;
            let (semantic, color_space, normal_convention) = texture_output_semantics(key)?;
            embedded_texture_outputs.push(json!({
                "texture_id":key,
                "sha256":sha256_hex(&bytes),
                "size_bytes":bytes.len(),
                "width":decoded.width(),
                "height":decoded.height(),
                "mime":"image/png",
                "semantic":semantic,
                "color_space":color_space,
                "normal_convention":normal_convention,
            }));
        }
    }
    for (material, keys) in materials.iter_mut().zip(material_texture_keys.iter()) {
        let Some(pbr) = material
            .get_mut("pbrMetallicRoughness")
            .and_then(Value::as_object_mut)
        else {
            continue;
        };
        for (slot, field) in [
            ("base_color", "baseColorTexture"),
            ("metallic_roughness", "metallicRoughnessTexture"),
        ] {
            if let Some(key) = keys.get(slot).and_then(Value::as_str) {
                if let Some(index) = texture_indices.get(key) {
                    pbr.insert(field.to_owned(), json!({"index":index}));
                }
            }
        }
        for (slot, field) in [("normal", "normalTexture"), ("ao", "occlusionTexture")] {
            if let Some(key) = keys.get(slot).and_then(Value::as_str) {
                if let Some(index) = texture_indices.get(key) {
                    material[field] = json!({"index":index});
                }
            }
        }
        if let Some(key) = keys.get("emissive").and_then(Value::as_str) {
            if let Some(index) = texture_indices.get(key) {
                material["emissiveTexture"] = json!({"index":index});
            }
        }
        let clearcoat_index = keys
            .get("clearcoat")
            .and_then(Value::as_str)
            .and_then(|key| texture_indices.get(key));
        let clearcoat_roughness_index = keys
            .get("clearcoat_roughness")
            .and_then(Value::as_str)
            .and_then(|key| texture_indices.get(key));
        if let (Some(clearcoat_index), Some(clearcoat_roughness_index)) =
            (clearcoat_index, clearcoat_roughness_index)
        {
            material["extensions"]["KHR_materials_clearcoat"] = json!({
                "clearcoatFactor":1.0,
                "clearcoatTexture":{"index":clearcoat_index},
                "clearcoatRoughnessFactor":1.0,
                "clearcoatRoughnessTexture":{"index":clearcoat_roughness_index}
            });
        }
    }
    while binary.len() % 4 != 0 {
        binary.push(0);
    }
    let uv_chart_count = parts
        .iter()
        .flat_map(|part| &part.sources)
        .map(|source| source.uv_chart_count as u64)
        .sum::<u64>();
    let continuous_uv = selected_material_pack_id.as_deref().is_some_and(|pack_id| {
        matches!(
            pack_id,
            "forgecad-fictional-energy-weapon" | FICTIONAL_ENERGY_WEAPON_2K_PACK_ID
        )
    });
    let uv_atlas = if continuous_uv {
        json!({
            "schema_version":"UvAtlas@1",
            "packing":"connected-dominant-axis-islands@1",
            "resolution":2048,
            "padding_texels":8,
            "charts":uv_chart_count,
            "atlas_count":1,
            "seam_policy":"edge-shared-or-explicit-seam@1",
            "overlap_policy":"disjoint-grid-cells@1"
        })
    } else {
        json!({"schema_version":"UvAtlas@1","packing":"triangle-chart-grid","resolution":512,"padding_texels":4,"charts":triangle_count})
    };
    let mut forgecad = json!({
        "schema_version":artifact_schema_version,
        "program_sha256":program_sha256,
        "triangle_count":triangle_count,
        "part_ids":ordered_unique_part_ids(parts),
        "source_node_ids":ordered_source_node_ids(parts),
        "material_zone_ids":ordered_unique_material_zone_ids(parts),
        "uv_atlas":uv_atlas,
    });
    if let (Some(pack_id), Some(manifest_sha256)) = (
        selected_material_pack_id,
        selected_material_pack_manifest_sha256,
    ) {
        forgecad["material_pack_id"] = Value::String(pack_id.clone());
        forgecad["material_pack_manifest_sha256"] = Value::String(manifest_sha256);
        forgecad["texture_count"] = Value::from(texture_keys.len() as u64);
        if pack_id == FICTIONAL_ENERGY_WEAPON_2K_PACK_ID {
            let mut texture_build = json!({
                "schema_version":"EmbeddedTextureBuild@1",
                "recipe_id":FICTIONAL_ENERGY_WEAPON_2K_RECIPE_ID,
                "algorithm":"catmullrom-plus-fixed-semantic-microdetail@1",
                "worker_algorithm_sha256":weapon_2k_algorithm_sha256(),
                "resolution":FICTIONAL_ENERGY_WEAPON_2K_RESOLUTION,
                "build_budget_ms":FICTIONAL_ENERGY_WEAPON_2K_BUILD_BUDGET_MS,
                "embedded_only":true,
                "external_uri":false,
                "network_at_runtime":false,
                "outputs":embedded_texture_outputs,
                "canonical_sha256":""
            });
            let mut texture_build_preimage = texture_build
                .as_object()
                .expect("texture build is an object")
                .clone();
            texture_build_preimage.remove("canonical_sha256");
            texture_build["canonical_sha256"] =
                Value::String(canonical_hash(&Value::Object(texture_build_preimage)));
            forgecad["texture_build"] = texture_build;
            if let Some(surface_bake) = &surface_bake {
                forgecad["surface_bake"] = surface_bake.metadata.clone();
            }
        }
    }
    if let Some(appearance) = appearance {
        let appearance_schema_version = appearance.get("schema_version").and_then(Value::as_str);
        if matches!(
            appearance_schema_version,
            Some("AppearanceProgram@2" | "AppearanceProgram@3")
        ) {
            let appearance_program_sha256 = appearance
                .get("canonical_sha256")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    GeometryError::Invalid(
                        "validated AppearanceProgram lacks its canonical hash".to_owned(),
                    )
                })?;
            forgecad["appearance_program_schema_version"] = Value::String(
                appearance_schema_version
                    .expect("matched schema")
                    .to_owned(),
            );
            forgecad["appearance_program_sha256"] =
                Value::String(appearance_program_sha256.to_owned());
        }
    }
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
    let mut root = json!({
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
    if !images.is_empty() {
        root["images"] = Value::Array(images);
        root["textures"] = Value::Array(textures);
    }
    if materials.iter().any(|material| {
        material
            .get("extensions")
            .and_then(|extensions| extensions.get("KHR_materials_clearcoat"))
            .is_some()
    }) {
        root["extensionsUsed"] = json!(["KHR_materials_clearcoat"]);
    }
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

fn pack_texture_bytes(material_pack_id: &str, key: &str) -> Result<Vec<u8>, GeometryError> {
    if material_pack_id == FICTIONAL_ENERGY_WEAPON_2K_PACK_ID {
        return fictional_energy_weapon_2k_texture_bytes(key);
    }
    let weapon_pack = material_pack_id == "forgecad-fictional-energy-weapon";
    if material_pack_id != "forgecad-hard-surface-robot" && !weapon_pack {
        return Err(GeometryError::Invalid(
            "offline material pack is unavailable".to_owned(),
        ));
    }
    match (weapon_pack, key) {
        (false, "metal010_color") => Ok(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../packages/forgecad-assets/forgecad-hard-surface-robot/1.0.0/textures/metal010_color.png"
        )).to_vec()),
        (true, "metal010_color") => Ok(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../packages/forgecad-assets/forgecad-fictional-energy-weapon/1.0.0/textures/metal010_color.png"
        )).to_vec()),
        (false, "metal010_normal_gl") => Ok(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../packages/forgecad-assets/forgecad-hard-surface-robot/1.0.0/textures/metal010_normal_gl.png"
        )).to_vec()),
        (true, "metal010_normal_gl") => Ok(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../packages/forgecad-assets/forgecad-fictional-energy-weapon/1.0.0/textures/metal010_normal_gl.png"
        )).to_vec()),
        (false, "metal010_roughness") => Ok(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../packages/forgecad-assets/forgecad-hard-surface-robot/1.0.0/textures/metal010_roughness.png"
        )).to_vec()),
        (true, "metal010_roughness") => Ok(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../packages/forgecad-assets/forgecad-fictional-energy-weapon/1.0.0/textures/metal010_roughness.png"
        )).to_vec()),
        (false, "metal010_metalness") => Ok(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../packages/forgecad-assets/forgecad-hard-surface-robot/1.0.0/textures/metal010_metalness.png"
        )).to_vec()),
        (true, "metal010_metalness") => Ok(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../packages/forgecad-assets/forgecad-fictional-energy-weapon/1.0.0/textures/metal010_metalness.png"
        )).to_vec()),
        (false, "metal010_metallic_roughness") => pack_metallic_roughness_texture(
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../packages/forgecad-assets/forgecad-hard-surface-robot/1.0.0/textures/metal010_roughness.png"
            )),
            Some(include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../packages/forgecad-assets/forgecad-hard-surface-robot/1.0.0/textures/metal010_metalness.png"
            ))),
        ),
        (true, "metal010_metallic_roughness") => pack_metallic_roughness_texture(
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../packages/forgecad-assets/forgecad-fictional-energy-weapon/1.0.0/textures/metal010_roughness.png"
            )),
            Some(include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../packages/forgecad-assets/forgecad-fictional-energy-weapon/1.0.0/textures/metal010_metalness.png"
            ))),
        ),
        (false, "plastic006_color") => Ok(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../packages/forgecad-assets/forgecad-hard-surface-robot/1.0.0/textures/plastic006_color.png"
        )).to_vec()),
        (true, "plastic006_color") => Ok(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../packages/forgecad-assets/forgecad-fictional-energy-weapon/1.0.0/textures/plastic006_color.png"
        )).to_vec()),
        (false, "plastic006_normal_gl") => Ok(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../packages/forgecad-assets/forgecad-hard-surface-robot/1.0.0/textures/plastic006_normal_gl.png"
        )).to_vec()),
        (true, "plastic006_normal_gl") => Ok(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../packages/forgecad-assets/forgecad-fictional-energy-weapon/1.0.0/textures/plastic006_normal_gl.png"
        )).to_vec()),
        (false, "plastic006_roughness") => Ok(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../packages/forgecad-assets/forgecad-hard-surface-robot/1.0.0/textures/plastic006_roughness.png"
        )).to_vec()),
        (true, "plastic006_roughness") => Ok(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../packages/forgecad-assets/forgecad-fictional-energy-weapon/1.0.0/textures/plastic006_roughness.png"
        )).to_vec()),
        (false, "plastic006_metallic_roughness") => pack_metallic_roughness_texture(
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../packages/forgecad-assets/forgecad-hard-surface-robot/1.0.0/textures/plastic006_roughness.png"
            )),
            None,
        ),
        (true, "plastic006_metallic_roughness") => pack_metallic_roughness_texture(
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../packages/forgecad-assets/forgecad-fictional-energy-weapon/1.0.0/textures/plastic006_roughness.png"
            )),
            None,
        ),
        _ => Err(GeometryError::Invalid(format!(
            "offline material pack texture is unavailable: {key}"
        ))),
    }
}

#[derive(Clone, Copy)]
enum WeaponTextureSemantic {
    SrgbColor,
    LinearScalar,
    OpenGlNormal,
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn weapon_2k_algorithm_sha256() -> String {
    sha256_hex(
        b"catmullrom-plus-fixed-semantic-microdetail@1|2048|fixed-64px-integer-tile|sRGB-baseColor|linear-data|OpenGL+Y|no-rng-no-time-no-network",
    )
}

fn texture_output_semantics(
    key: &str,
) -> Result<(&'static str, &'static str, Value), GeometryError> {
    match key {
        "metal010_color" | "plastic006_color" => Ok(("baseColor", "sRGB", Value::Null)),
        "metal010_normal_gl" | "plastic006_normal_gl" => {
            Ok(("normal", "linear", Value::String("OpenGL+Y".to_owned())))
        }
        "metal010_roughness" | "plastic006_roughness" => Ok(("roughness", "linear", Value::Null)),
        "metal010_metalness" => Ok(("metallic", "linear", Value::Null)),
        "metal010_metallic_roughness" | "plastic006_metallic_roughness" => {
            Ok(("metallicRoughness", "linear", Value::Null))
        }
        _ => Err(GeometryError::Invalid(format!(
            "texture output semantics are unavailable: {key}"
        ))),
    }
}

fn weapon_source_texture_bytes(key: &str) -> Result<&'static [u8], GeometryError> {
    match key {
        "metal010_color" => Ok(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../packages/forgecad-assets/forgecad-fictional-energy-weapon/1.0.0/textures/metal010_color.png"
        ))),
        "metal010_normal_gl" => Ok(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../packages/forgecad-assets/forgecad-fictional-energy-weapon/1.0.0/textures/metal010_normal_gl.png"
        ))),
        "metal010_roughness" => Ok(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../packages/forgecad-assets/forgecad-fictional-energy-weapon/1.0.0/textures/metal010_roughness.png"
        ))),
        "metal010_metalness" => Ok(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../packages/forgecad-assets/forgecad-fictional-energy-weapon/1.0.0/textures/metal010_metalness.png"
        ))),
        "plastic006_color" => Ok(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../packages/forgecad-assets/forgecad-fictional-energy-weapon/1.0.0/textures/plastic006_color.png"
        ))),
        "plastic006_normal_gl" => Ok(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../packages/forgecad-assets/forgecad-fictional-energy-weapon/1.0.0/textures/plastic006_normal_gl.png"
        ))),
        "plastic006_roughness" => Ok(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../packages/forgecad-assets/forgecad-fictional-energy-weapon/1.0.0/textures/plastic006_roughness.png"
        ))),
        _ => Err(GeometryError::Invalid(format!(
            "2K material source texture is unavailable: {key}"
        ))),
    }
}

fn microdetail_delta(x: u32, y: u32, seed: u32) -> i16 {
    // A fixed 64x64 tile keeps the output losslessly compressible while still
    // adding real high-frequency information that is absent from any pure
    // resize. The arithmetic is integer-only and has no platform RNG.
    let mut value = (x & 63)
        .wrapping_mul(0x45d9f3b)
        .wrapping_add((y & 63).wrapping_mul(0x119de1f3))
        .wrapping_add(seed.wrapping_mul(0x27d4eb2d));
    value ^= value >> 16;
    value = value.wrapping_mul(0x7feb352d);
    value ^= value >> 15;
    (value % 5) as i16 - 2
}

fn encode_rgb8_png(bytes: &[u8], width: u32, height: u32) -> Result<Vec<u8>, GeometryError> {
    let mut encoded = Vec::new();
    PngEncoder::new_with_quality(&mut encoded, CompressionType::Default, FilterType::Adaptive)
        .write_image(bytes, width, height, ExtendedColorType::Rgb8)
        .map_err(|error| GeometryError::Invalid(format!("2K RGB PNG encode failed: {error}")))?;
    Ok(encoded)
}

fn encode_luma8_png(bytes: &[u8], width: u32, height: u32) -> Result<Vec<u8>, GeometryError> {
    let mut encoded = Vec::new();
    PngEncoder::new_with_quality(&mut encoded, CompressionType::Default, FilterType::Adaptive)
        .write_image(bytes, width, height, ExtendedColorType::L8)
        .map_err(|error| GeometryError::Invalid(format!("2K scalar PNG encode failed: {error}")))?;
    Ok(encoded)
}

fn derive_weapon_texture_2k(
    source_bytes: &[u8],
    semantic: WeaponTextureSemantic,
    seed: u32,
) -> Result<Vec<u8>, GeometryError> {
    let decoded = image::load_from_memory(source_bytes)
        .map_err(|error| GeometryError::Invalid(format!("2K source PNG decode failed: {error}")))?;
    let size = FICTIONAL_ENERGY_WEAPON_2K_RESOLUTION;
    match semantic {
        WeaponTextureSemantic::SrgbColor => {
            let source = decoded.to_rgb8();
            let mut output =
                imageops::resize(&source, size, size, imageops::FilterType::CatmullRom);
            for (x, y, pixel) in output.enumerate_pixels_mut() {
                let detail = microdetail_delta(x, y, seed);
                for channel in &mut pixel.0 {
                    *channel = (*channel as i16 + detail).clamp(0, 255) as u8;
                }
            }
            encode_rgb8_png(output.as_raw(), size, size)
        }
        WeaponTextureSemantic::LinearScalar => {
            let source = decoded.to_luma8();
            let mut output =
                imageops::resize(&source, size, size, imageops::FilterType::CatmullRom);
            for (x, y, pixel) in output.enumerate_pixels_mut() {
                let detail = microdetail_delta(x, y, seed) * 2;
                pixel.0[0] = (pixel.0[0] as i16 + detail).clamp(0, 255) as u8;
            }
            encode_luma8_png(output.as_raw(), size, size)
        }
        WeaponTextureSemantic::OpenGlNormal => {
            let source = decoded.to_rgb8();
            let mut output =
                imageops::resize(&source, size, size, imageops::FilterType::CatmullRom);
            for (x, y, pixel) in output.enumerate_pixels_mut() {
                let dx = microdetail_delta(x, y, seed) as f32 / 255.0;
                let dy = microdetail_delta(x.wrapping_add(17), y.wrapping_add(29), seed ^ 0xa5)
                    as f32
                    / 255.0;
                let mut nx = pixel.0[0] as f32 / 127.5 - 1.0 + dx;
                let mut ny = pixel.0[1] as f32 / 127.5 - 1.0 + dy;
                let mut nz = pixel.0[2] as f32 / 127.5 - 1.0;
                let length = (nx * nx + ny * ny + nz * nz).sqrt().max(1.0e-8);
                nx /= length;
                ny /= length;
                nz /= length;
                pixel.0 = [
                    ((nx * 0.5 + 0.5) * 255.0).round().clamp(0.0, 255.0) as u8,
                    ((ny * 0.5 + 0.5) * 255.0).round().clamp(0.0, 255.0) as u8,
                    ((nz * 0.5 + 0.5) * 255.0).round().clamp(0.0, 255.0) as u8,
                ];
            }
            encode_rgb8_png(output.as_raw(), size, size)
        }
    }
}

fn fictional_energy_weapon_2k_texture_bytes(key: &str) -> Result<Vec<u8>, GeometryError> {
    match key {
        "metal010_color" => derive_weapon_texture_2k(
            weapon_source_texture_bytes(key)?,
            WeaponTextureSemantic::SrgbColor,
            11,
        ),
        "metal010_normal_gl" => derive_weapon_texture_2k(
            weapon_source_texture_bytes(key)?,
            WeaponTextureSemantic::OpenGlNormal,
            23,
        ),
        "metal010_roughness" | "metal010_metalness" => derive_weapon_texture_2k(
            weapon_source_texture_bytes(key)?,
            WeaponTextureSemantic::LinearScalar,
            if key == "metal010_roughness" { 37 } else { 41 },
        ),
        "plastic006_color" => derive_weapon_texture_2k(
            weapon_source_texture_bytes(key)?,
            WeaponTextureSemantic::SrgbColor,
            53,
        ),
        "plastic006_normal_gl" => derive_weapon_texture_2k(
            weapon_source_texture_bytes(key)?,
            WeaponTextureSemantic::OpenGlNormal,
            67,
        ),
        "plastic006_roughness" => derive_weapon_texture_2k(
            weapon_source_texture_bytes(key)?,
            WeaponTextureSemantic::LinearScalar,
            79,
        ),
        "metal010_metallic_roughness" => pack_2k_metallic_roughness_texture(
            &fictional_energy_weapon_2k_texture_bytes("metal010_roughness")?,
            Some(&fictional_energy_weapon_2k_texture_bytes(
                "metal010_metalness",
            )?),
        ),
        "plastic006_metallic_roughness" => pack_2k_metallic_roughness_texture(
            &fictional_energy_weapon_2k_texture_bytes("plastic006_roughness")?,
            None,
        ),
        _ => Err(GeometryError::Invalid(format!(
            "2K material output texture is unavailable: {key}"
        ))),
    }
}

fn pack_2k_metallic_roughness_texture(
    roughness_bytes: &[u8],
    metallic_bytes: Option<&[u8]>,
) -> Result<Vec<u8>, GeometryError> {
    let roughness = image::load_from_memory(roughness_bytes)
        .map_err(|error| GeometryError::Invalid(format!("2K roughness decode failed: {error}")))?
        .to_luma8();
    let metallic = metallic_bytes
        .map(|bytes| {
            image::load_from_memory(bytes)
                .map(|image| image.to_luma8())
                .map_err(|error| {
                    GeometryError::Invalid(format!("2K metallic decode failed: {error}"))
                })
        })
        .transpose()?;
    if roughness.dimensions()
        != (
            FICTIONAL_ENERGY_WEAPON_2K_RESOLUTION,
            FICTIONAL_ENERGY_WEAPON_2K_RESOLUTION,
        )
        || metallic
            .as_ref()
            .is_some_and(|value| value.dimensions() != roughness.dimensions())
    {
        return Err(GeometryError::Invalid(
            "2K metallic-roughness inputs have invalid dimensions".to_owned(),
        ));
    }
    let mut packed = Vec::with_capacity(
        FICTIONAL_ENERGY_WEAPON_2K_RESOLUTION as usize
            * FICTIONAL_ENERGY_WEAPON_2K_RESOLUTION as usize
            * 3,
    );
    for (index, roughness_value) in roughness.as_raw().iter().enumerate() {
        let metallic_value = metallic
            .as_ref()
            .map(|value| value.as_raw()[index])
            .unwrap_or(0);
        packed.extend_from_slice(&[255, *roughness_value, metallic_value]);
    }
    encode_rgb8_png(
        &packed,
        FICTIONAL_ENERGY_WEAPON_2K_RESOLUTION,
        FICTIONAL_ENERGY_WEAPON_2K_RESOLUTION,
    )
}

/// Build a canonical glTF metallic-roughness texture from the source pack's
/// separate linear grayscale maps.  glTF stores roughness in G and metallic
/// in B; R is intentionally set to 255 because it is unused.  This is kept in
/// the fixed worker rather than accepting a user path or image payload, so the
/// result stays deterministic and source-bound.
fn pack_metallic_roughness_texture(
    roughness_bytes: &[u8],
    metallic_bytes: Option<&[u8]>,
) -> Result<Vec<u8>, GeometryError> {
    let roughness = image::load_from_memory(roughness_bytes)
        .map_err(|error| {
            GeometryError::Invalid(format!("roughness texture decode failed: {error}"))
        })?
        .to_luma8();
    let metallic = metallic_bytes
        .map(|bytes| {
            image::load_from_memory(bytes)
                .map(|image| image.to_luma8())
                .map_err(|error| {
                    GeometryError::Invalid(format!("metallic texture decode failed: {error}"))
                })
        })
        .transpose()?;
    if let Some(metallic) = metallic.as_ref() {
        if metallic.dimensions() != roughness.dimensions() {
            return Err(GeometryError::Invalid(
                "metallic and roughness textures must have matching dimensions".to_owned(),
            ));
        }
    }
    let (width, height) = roughness.dimensions();
    let mut packed = Vec::with_capacity(width as usize * height as usize * 3);
    for y in 0..height {
        for x in 0..width {
            let roughness_value = roughness.get_pixel(x, y)[0];
            let metallic_value = metallic
                .as_ref()
                .map(|texture| texture.get_pixel(x, y)[0])
                .unwrap_or(0);
            packed.extend_from_slice(&[255, roughness_value, metallic_value]);
        }
    }
    let mut encoded = Vec::new();
    PngEncoder::new_with_quality(&mut encoded, CompressionType::Best, FilterType::NoFilter)
        .write_image(&packed, width, height, ExtendedColorType::Rgb8)
        .map_err(|error| {
            GeometryError::Invalid(format!("metallic-roughness texture encode failed: {error}"))
        })?;
    Ok(encoded)
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
    use std::collections::BTreeSet;

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

    fn panel_v2_program() -> Value {
        let mut program = json!({
            "schema_version":"GeometryProgram@2",
            "project_id":"panel-v2-structural-gate",
            "representation_plan_sha256":"c".repeat(64),
            "operator_catalog_sha256":operator_catalog_sha256(),
            "units":{"length":"meter","angle":"radian","coordinate_system":"right-handed-y-up"},
            "budgets":{
                "max_nodes":4,
                "max_triangles":10000,
                "max_glb_bytes":4194304,
                "max_worker_memory_bytes":536870912,
                "max_runtime_ms":10000
            },
            "nodes":[{
                "node_id":"panel-v2",
                "operator_id":"forgecad.geometry.panel@2",
                "inputs":[],
                "parameters":{
                    "shape":"panel",
                    "size_m":[2.4,1.6,0.4],
                    "thickness_m":0.4,
                    "inset_m":0.25,
                    "recess_depth_m":0.12,
                    "border_width_m":0.18,
                    "bevel_m":0.08,
                    "bevel_segments":2,
                    "support_loop_count":2,
                    "support_loop_width_m":0.03,
                    "position_m":[0.0,0.0,0.0],
                    "rotation_rad":[0.0,0.0,0.0]
                }
            }],
            "part_outputs":[{"part_id":"panel-v2-part","input_node_ids":["panel-v2"],"material_zone_id":"zone-white-shell","solid":true}]
        });
        program["canonical_sha256"] = Value::String(canonical_hash(&without_hash(&program)));
        program
    }

    fn vent_array_v2_program() -> Value {
        let mut program = json!({
            "schema_version":"GeometryProgram@2",
            "project_id":"vent-array-v2-structural-gate",
            "representation_plan_sha256":"f".repeat(64),
            "operator_catalog_sha256":operator_catalog_sha256(),
            "units":{"length":"meter","angle":"radian","coordinate_system":"right-handed-y-up"},
            "budgets":{
                "max_nodes":4,
                "max_triangles":4000,
                "max_glb_bytes":4194304,
                "max_worker_memory_bytes":536870912,
                "max_runtime_ms":10000
            },
            "nodes":[{
                "node_id":"vent-array-v2",
                "operator_id":"forgecad.geometry.vent-array@2",
                "inputs":[],
                "parameters":{
                    "shape":"vent-array",
                    "width_m":1.6,
                    "height_m":0.8,
                    "depth_m":0.26,
                    "face_thickness_m":0.08,
                    "backing_depth_m":0.08,
                    "backing_gap_m":0.10,
                    "slot_count":4,
                    "slot_width_m":0.16,
                    "slot_spacing_m":0.12,
                    "slot_margin_m":0.16,
                    "slot_edge_bevel_m":0.02,
                    "bevel_segments":2,
                    "position_m":[0.0,0.0,0.0],
                    "rotation_rad":[0.0,0.0,0.0]
                }
            }],
            "part_outputs":[{"part_id":"vent-array-v2-part","input_node_ids":["vent-array-v2"],"material_zone_id":"zone-black-mechanical","solid":true}]
        });
        program["canonical_sha256"] = Value::String(canonical_hash(&without_hash(&program)));
        program
    }

    fn recessed_channel_program() -> Value {
        let mut program = json!({
            "schema_version":"GeometryProgram@2",
            "project_id":"recessed-channel-structural-gate",
            "representation_plan_sha256":"e".repeat(64),
            "operator_catalog_sha256":operator_catalog_sha256(),
            "units":{"length":"meter","angle":"radian","coordinate_system":"right-handed-y-up"},
            "budgets":{
                "max_nodes":4,
                "max_triangles":512,
                "max_glb_bytes":4194304,
                "max_worker_memory_bytes":536870912,
                "max_runtime_ms":10000
            },
            "nodes":[{
                "node_id":"recessed-channel",
                "operator_id":"forgecad.geometry.recessed-channel@1",
                "inputs":[],
                "parameters":{
                    "shape":"recessed-channel",
                    "stations":[
                        {"point_m":[-0.8,0.0,0.0],"width_m":0.30,"depth_m":0.12},
                        {"point_m":[0.0,0.08,0.0],"width_m":0.36,"depth_m":0.16},
                        {"point_m":[0.82,0.0,0.0],"width_m":0.28,"depth_m":0.10}
                    ],
                    "path_frame":"planar-xy-z-up@1",
                    "floor_width_ratio":0.42,
                    "edge_bevel_m":0.01,
                    "start_transition_m":0.08,
                    "end_transition_m":0.10,
                    "transition_segments":2,
                    "position_m":[0.0,0.0,0.0],
                    "rotation_rad":[0.0,0.0,0.0]
                }
            }],
            "part_outputs":[{"part_id":"recessed-channel-part","input_node_ids":["recessed-channel"],"material_zone_id":"zone-black-mechanical","solid":true}]
        });
        program["canonical_sha256"] = Value::String(canonical_hash(&without_hash(&program)));
        program
    }

    fn energy_core_program() -> Value {
        let mut program = json!({
            "schema_version":"GeometryProgram@2",
            "project_id":"energy-core-structural-gate",
            "representation_plan_sha256":"9".repeat(64),
            "operator_catalog_sha256":operator_catalog_sha256(),
            "units":{"length":"meter","angle":"radian","coordinate_system":"right-handed-y-up"},
            "budgets":{
                "max_nodes":8,
                "max_triangles":1024,
                "max_glb_bytes":4194304,
                "max_worker_memory_bytes":536870912,
                "max_runtime_ms":10000
            },
            "nodes":[
                {"node_id":"core-guard","operator_id":"forgecad.geometry.energy-core@1","inputs":[],"parameters":{"shape":"energy-core","component":"guard-ring","outer_radius_m":0.48,"inner_radius_m":0.40,"depth_m":0.08,"radial_segments":32,"position_m":[0.0,0.0,0.0],"rotation_rad":[0.0,0.0,0.0]}},
                {"node_id":"core-mechanical-ring","operator_id":"forgecad.geometry.energy-core@1","inputs":[],"parameters":{"shape":"energy-core","component":"mechanical-ring","outer_radius_m":0.38,"inner_radius_m":0.28,"depth_m":0.06,"radial_segments":32,"position_m":[0.0,0.075,0.0],"rotation_rad":[0.0,0.0,0.0]}},
                {"node_id":"core-emitter","operator_id":"forgecad.geometry.energy-core@1","inputs":[],"parameters":{"shape":"energy-core","component":"emitter-core","outer_radius_m":0.25,"inner_radius_m":0.0,"depth_m":0.04,"radial_segments":32,"position_m":[0.0,0.13,0.0],"rotation_rad":[0.0,0.0,0.0]}},
                {"node_id":"core-backplate","operator_id":"forgecad.geometry.energy-core@1","inputs":[],"parameters":{"shape":"energy-core","component":"mechanical-backplate","outer_radius_m":0.34,"inner_radius_m":0.0,"depth_m":0.06,"radial_segments":32,"position_m":[0.0,-0.075,0.0],"rotation_rad":[0.0,0.0,0.0]}}
            ],
            "part_outputs":[
                {"part_id":"energy-core-guard","input_node_ids":["core-guard"],"material_zone_id":"zone-black-mechanical","solid":true},
                {"part_id":"energy-core-mechanical-ring","input_node_ids":["core-mechanical-ring"],"material_zone_id":"zone-brushed-metal","solid":true},
                {"part_id":"energy-core-emitter","input_node_ids":["core-emitter"],"material_zone_id":"zone-emissive-amber","solid":true},
                {"part_id":"energy-core-backplate","input_node_ids":["core-backplate"],"material_zone_id":"zone-black-mechanical","solid":true}
            ]
        });
        program["canonical_sha256"] = Value::String(canonical_hash(&without_hash(&program)));
        program
    }

    #[test]
    fn panel_v2_compiles_to_strict_readback_bound_lineage() {
        let program = panel_v2_program();
        let first = compile_geometry_program(&program).expect("panel@2 compile");
        let second = compile_geometry_program(&program).expect("panel@2 deterministic compile");
        assert_eq!(first.glb, second.glb);
        assert!(first.triangle_count > 92);
        let inspection = integrity::inspect_glb(&first.glb).expect("panel@2 strict GLB readback");
        assert!(
            inspection.hard_gate_passed,
            "{:?}",
            inspection.failure_codes
        );
        assert_eq!(
            inspection.program_sha256,
            program["canonical_sha256"].as_str().expect("program hash")
        );
        assert_eq!(
            inspection.operator_catalog_sha256.as_deref(),
            Some(operator_catalog_sha256().as_str())
        );
        assert_eq!(inspection.part_ids, vec!["panel-v2-part"]);
        assert_eq!(inspection.source_node_ids, vec!["panel-v2"]);
        assert_eq!(inspection.triangle_count, first.triangle_count);
        assert_eq!(inspection.boundary_edge_count, 0);
        assert_eq!(inspection.non_manifold_edge_count, 0);
    }

    #[test]
    fn panel_v2_geometry_program_fails_closed_on_budget_and_hash_drift() {
        let required_triangles = compile_geometry_program(&panel_v2_program())
            .expect("panel@2 baseline compile")
            .triangle_count;
        let mut under_budget = panel_v2_program();
        under_budget["budgets"]["max_triangles"] = json!(required_triangles - 1);
        under_budget["canonical_sha256"] =
            Value::String(canonical_hash(&without_hash(&under_budget)));
        assert!(compile_geometry_program(&under_budget).is_err());

        let mut unknown = panel_v2_program();
        unknown["nodes"][0]["parameters"]["script"] = json!("mesh.inset");
        unknown["canonical_sha256"] = Value::String(canonical_hash(&without_hash(&unknown)));
        assert!(compile_geometry_program(&unknown).is_err());

        let mut wrong_catalog = panel_v2_program();
        wrong_catalog["operator_catalog_sha256"] = Value::String("d".repeat(64));
        wrong_catalog["canonical_sha256"] =
            Value::String(canonical_hash(&without_hash(&wrong_catalog)));
        assert!(compile_geometry_program(&wrong_catalog).is_err());
    }

    #[test]
    fn vent_array_v2_compiles_deterministically_with_strict_lineage_readback() {
        let program = vent_array_v2_program();
        let first = compile_geometry_program(&program).expect("vent-array@2 compile");
        let second =
            compile_geometry_program(&program).expect("vent-array@2 deterministic compile");
        assert_eq!(first.glb, second.glb);
        assert_eq!(first.triangle_count, 312);
        let inspection =
            integrity::inspect_glb(&first.glb).expect("vent-array@2 strict GLB readback");
        assert!(
            inspection.hard_gate_passed,
            "{:?}",
            inspection.failure_codes
        );
        assert_eq!(
            inspection.program_sha256,
            program["canonical_sha256"].as_str().expect("program hash")
        );
        assert_eq!(
            inspection.operator_catalog_sha256.as_deref(),
            Some(operator_catalog_sha256().as_str())
        );
        assert_eq!(inspection.part_ids, vec!["vent-array-v2-part"]);
        assert_eq!(inspection.source_node_ids, vec!["vent-array-v2"]);
        assert_eq!(inspection.triangle_count, 312);
        assert_eq!(inspection.boundary_edge_count, 0);
        assert_eq!(inspection.non_manifold_edge_count, 0);
    }

    #[test]
    fn vent_array_v2_backing_gap_changes_deterministic_glb_without_topology_drift() {
        let first_program = vent_array_v2_program();
        let mut second_program = first_program.clone();
        second_program["nodes"][0]["parameters"]["backing_gap_m"] = json!(0.06);
        second_program["nodes"][0]["parameters"]["depth_m"] = json!(0.22);
        second_program["canonical_sha256"] =
            Value::String(canonical_hash(&without_hash(&second_program)));

        let first = compile_geometry_program(&first_program).expect("first legal gap fixture");
        let second = compile_geometry_program(&second_program).expect("second legal gap fixture");
        let second_replay =
            compile_geometry_program(&second_program).expect("second deterministic replay");

        assert_eq!(first.triangle_count, 312);
        assert_eq!(second.triangle_count, 312);
        assert_eq!(second.glb, second_replay.glb);
        assert_ne!(first.glb, second.glb, "backing gap must change GLB bytes");
        assert_ne!(first.program_sha256, second.program_sha256);

        for artifact in [&first, &second] {
            let inspection = integrity::inspect_glb(&artifact.glb)
                .expect("legal backing-gap fixture strict GLB readback");
            assert!(
                inspection.hard_gate_passed,
                "{:?}",
                inspection.failure_codes
            );
            assert_eq!(inspection.triangle_count, 312);
            assert_eq!(inspection.boundary_edge_count, 0);
            assert_eq!(inspection.non_manifold_edge_count, 0);
            assert_eq!(inspection.part_ids, vec!["vent-array-v2-part"]);
            assert_eq!(inspection.source_node_ids, vec!["vent-array-v2"]);
        }
    }

    #[test]
    fn vent_array_v2_geometry_program_rejects_budget_and_operator_parameter_drift() {
        let mut under_budget = vent_array_v2_program();
        under_budget["budgets"]["max_triangles"] = json!(311);
        under_budget["canonical_sha256"] =
            Value::String(canonical_hash(&without_hash(&under_budget)));
        assert!(compile_geometry_program(&under_budget).is_err());

        let mut wrong_branch = vent_array_v2_program();
        wrong_branch["nodes"][0]["operator_id"] = json!("forgecad.geometry.vent-array@1");
        wrong_branch["canonical_sha256"] =
            Value::String(canonical_hash(&without_hash(&wrong_branch)));
        assert!(compile_geometry_program(&wrong_branch).is_err());

        let mut unknown = vent_array_v2_program();
        unknown["nodes"][0]["parameters"]["script"] = json!("bpy.ops.mesh.inset()");
        unknown["canonical_sha256"] = Value::String(canonical_hash(&without_hash(&unknown)));
        assert!(compile_geometry_program(&unknown).is_err());
    }

    #[test]
    fn recessed_channel_compiles_deterministically_with_strict_lineage_readback() {
        let program = recessed_channel_program();
        let first = compile_geometry_program(&program).expect("recessed-channel compile");
        let second =
            compile_geometry_program(&program).expect("recessed-channel deterministic compile");
        assert_eq!(first.glb, second.glb);
        assert_eq!(first.triangle_count, 220);
        let inspection =
            integrity::inspect_glb(&first.glb).expect("recessed-channel strict GLB readback");
        assert!(
            inspection.hard_gate_passed,
            "{:?}",
            inspection.failure_codes
        );
        assert_eq!(
            inspection.program_sha256,
            program["canonical_sha256"].as_str().expect("program hash")
        );
        assert_eq!(
            inspection.operator_catalog_sha256.as_deref(),
            Some(operator_catalog_sha256().as_str())
        );
        assert_eq!(inspection.part_ids, vec!["recessed-channel-part"]);
        assert_eq!(inspection.source_node_ids, vec!["recessed-channel"]);
        assert_eq!(inspection.triangle_count, 220);
        assert_eq!(inspection.connected_component_count, 1);
        assert_eq!(inspection.boundary_edge_count, 0);
        assert_eq!(inspection.non_manifold_edge_count, 0);
    }

    #[test]
    fn recessed_channel_legal_parameter_variants_are_deterministic_closed_and_connected() {
        let base = recessed_channel_program();
        let baseline = compile_geometry_program(&base).expect("baseline recessed-channel compile");
        let variants = [
            ("path", {
                let mut program = base.clone();
                program["nodes"][0]["parameters"]["stations"][1]["point_m"] =
                    json!([0.0, 0.18, 0.0]);
                program
            }),
            ("width", {
                let mut program = base.clone();
                program["nodes"][0]["parameters"]["stations"][1]["width_m"] = json!(0.40);
                program
            }),
            ("depth", {
                let mut program = base.clone();
                program["nodes"][0]["parameters"]["stations"][1]["depth_m"] = json!(0.18);
                program
            }),
            ("bevel", {
                let mut program = base.clone();
                program["nodes"][0]["parameters"]["edge_bevel_m"] = json!(0.005);
                program
            }),
            ("start_transition", {
                let mut program = base.clone();
                program["nodes"][0]["parameters"]["start_transition_m"] = json!(0.12);
                program
            }),
            ("end_transition", {
                let mut program = base.clone();
                program["nodes"][0]["parameters"]["end_transition_m"] = json!(0.14);
                program
            }),
            ("transition_segments", {
                let mut program = base.clone();
                program["nodes"][0]["parameters"]["transition_segments"] = json!(3);
                program
            }),
        ];
        for (label, mut variant) in variants {
            variant["canonical_sha256"] = Value::String(canonical_hash(&without_hash(&variant)));
            let first = compile_geometry_program(&variant)
                .unwrap_or_else(|error| panic!("{label} variant should compile: {error}"));
            let replay = compile_geometry_program(&variant)
                .unwrap_or_else(|error| panic!("{label} variant replay should compile: {error}"));
            assert_eq!(
                first.glb, replay.glb,
                "{label} variant must be deterministic"
            );
            assert_ne!(
                baseline.glb, first.glb,
                "{label} variant must change GLB bytes"
            );
            assert!(
                first.triangle_count
                    <= variant["budgets"]["max_triangles"]
                        .as_u64()
                        .expect("variant triangle budget")
            );
            let inspection = integrity::inspect_glb(&first.glb)
                .unwrap_or_else(|error| panic!("{label} variant strict readback: {error}"));
            assert!(
                inspection.hard_gate_passed,
                "{label}: {:?}",
                inspection.failure_codes
            );
            assert_eq!(
                inspection.connected_component_count, 1,
                "{label} component count"
            );
            assert_eq!(inspection.boundary_edge_count, 0, "{label} boundary edges");
            assert_eq!(
                inspection.non_manifold_edge_count, 0,
                "{label} non-manifold edges"
            );
        }
    }

    #[test]
    fn recessed_channel_rejects_budget_hash_and_operator_drift() {
        let mut under_budget = recessed_channel_program();
        under_budget["budgets"]["max_triangles"] = json!(219);
        under_budget["canonical_sha256"] =
            Value::String(canonical_hash(&without_hash(&under_budget)));
        assert!(compile_geometry_program(&under_budget).is_err());

        let mut wrong_branch = recessed_channel_program();
        wrong_branch["nodes"][0]["operator_id"] = json!("forgecad.geometry.vent-array@2");
        wrong_branch["canonical_sha256"] =
            Value::String(canonical_hash(&without_hash(&wrong_branch)));
        assert!(compile_geometry_program(&wrong_branch).is_err());

        let mut unknown = recessed_channel_program();
        unknown["nodes"][0]["parameters"]["url"] = json!("https://example.invalid");
        unknown["canonical_sha256"] = Value::String(canonical_hash(&without_hash(&unknown)));
        assert!(compile_geometry_program(&unknown).is_err());

        let mut hash_drift = recessed_channel_program();
        hash_drift["canonical_sha256"] = json!("0".repeat(64));
        assert!(compile_geometry_program(&hash_drift).is_err());
    }

    #[test]
    fn energy_core_emits_four_deterministic_semantic_watertight_parts() {
        let program = energy_core_program();
        let first = compile_geometry_program(&program).expect("energy-core compile");
        let replay = compile_geometry_program(&program).expect("energy-core deterministic replay");
        assert_eq!(first.glb, replay.glb);
        assert_eq!(first.triangle_count, 768);
        let inspection =
            integrity::inspect_glb(&first.glb).expect("energy-core strict GLB readback");
        assert!(
            inspection.hard_gate_passed,
            "{:?}",
            inspection.failure_codes
        );
        assert_eq!(
            inspection.program_sha256,
            program["canonical_sha256"].as_str().expect("program hash")
        );
        assert_eq!(
            inspection.operator_catalog_sha256.as_deref(),
            Some(operator_catalog_sha256().as_str())
        );
        assert_eq!(inspection.triangle_count, 768);
        assert_eq!(inspection.connected_component_count, 4);
        assert_eq!(inspection.boundary_edge_count, 0);
        assert_eq!(inspection.non_manifold_edge_count, 0);
        assert_eq!(inspection.winding_error_count, 0);
        assert_eq!(
            inspection.part_ids,
            [
                "energy-core-guard",
                "energy-core-mechanical-ring",
                "energy-core-emitter",
                "energy-core-backplate"
            ]
        );
        assert_eq!(
            inspection.source_node_ids,
            [
                "core-guard",
                "core-mechanical-ring",
                "core-emitter",
                "core-backplate"
            ]
        );
        assert_eq!(
            inspection.material_zone_ids,
            [
                "zone-black-mechanical",
                "zone-brushed-metal",
                "zone-emissive-amber"
            ]
        );
    }

    #[test]
    fn energy_core_legal_radius_depth_and_component_variants_change_glb() {
        let base = energy_core_program();
        let baseline = compile_geometry_program(&base).expect("baseline energy-core compile");
        let variants = [
            ("outer-radius", "outer_radius_m", json!(0.46)),
            ("inner-radius", "inner_radius_m", json!(0.39)),
            ("depth", "depth_m", json!(0.07)),
            ("radial-segments", "radial_segments", json!(24)),
        ];
        for (label, parameter, value) in variants {
            let mut variant = base.clone();
            variant["nodes"][0]["parameters"][parameter] = value;
            variant["canonical_sha256"] = Value::String(canonical_hash(&without_hash(&variant)));
            let first = compile_geometry_program(&variant)
                .unwrap_or_else(|error| panic!("{label} variant should compile: {error}"));
            let replay = compile_geometry_program(&variant)
                .unwrap_or_else(|error| panic!("{label} replay should compile: {error}"));
            assert_eq!(first.glb, replay.glb, "{label} must be deterministic");
            assert_ne!(baseline.glb, first.glb, "{label} must change GLB bytes");
            let inspection = integrity::inspect_glb(&first.glb)
                .unwrap_or_else(|error| panic!("{label} strict readback: {error}"));
            assert!(
                inspection.hard_gate_passed,
                "{label}: {:?}",
                inspection.failure_codes
            );
            assert_eq!(inspection.boundary_edge_count, 0, "{label} boundary edges");
            assert_eq!(
                inspection.non_manifold_edge_count, 0,
                "{label} non-manifold edges"
            );
        }
    }

    #[test]
    fn energy_core_rejects_semantic_relationship_budget_and_open_fields() {
        let mut ring_without_hole = energy_core_program();
        ring_without_hole["nodes"][0]["parameters"]["inner_radius_m"] = json!(0.0);
        ring_without_hole["canonical_sha256"] =
            Value::String(canonical_hash(&without_hash(&ring_without_hole)));
        assert!(compile_geometry_program(&ring_without_hole).is_err());

        let mut solid_with_hole = energy_core_program();
        solid_with_hole["nodes"][2]["parameters"]["inner_radius_m"] = json!(0.1);
        solid_with_hole["canonical_sha256"] =
            Value::String(canonical_hash(&without_hash(&solid_with_hole)));
        assert!(compile_geometry_program(&solid_with_hole).is_err());

        let mut negative_inner_radius = energy_core_program();
        negative_inner_radius["nodes"][2]["parameters"]["inner_radius_m"] = json!(-0.1);
        negative_inner_radius["canonical_sha256"] =
            Value::String(canonical_hash(&without_hash(&negative_inner_radius)));
        assert!(compile_geometry_program(&negative_inner_radius).is_err());

        let mut solid_with_tiny_hole = energy_core_program();
        solid_with_tiny_hole["nodes"][2]["parameters"]["inner_radius_m"] = json!(0.000001);
        solid_with_tiny_hole["canonical_sha256"] =
            Value::String(canonical_hash(&without_hash(&solid_with_tiny_hole)));
        assert!(compile_geometry_program(&solid_with_tiny_hole).is_err());

        let mut inverted = energy_core_program();
        inverted["nodes"][1]["parameters"]["inner_radius_m"] = json!(0.39);
        inverted["nodes"][1]["parameters"]["outer_radius_m"] = json!(0.38);
        inverted["canonical_sha256"] = Value::String(canonical_hash(&without_hash(&inverted)));
        assert!(compile_geometry_program(&inverted).is_err());

        let mut under_budget = energy_core_program();
        under_budget["budgets"]["max_triangles"] = json!(767);
        under_budget["canonical_sha256"] =
            Value::String(canonical_hash(&without_hash(&under_budget)));
        assert!(compile_geometry_program(&under_budget).is_err());

        let mut executable = energy_core_program();
        executable["nodes"][0]["parameters"]["python"] =
            json!("bpy.ops.mesh.primitive_torus_add()");
        executable["canonical_sha256"] = Value::String(canonical_hash(&without_hash(&executable)));
        assert!(compile_geometry_program(&executable).is_err());
    }

    fn v2_draft_program() -> Value {
        let mut draft = v2_program();
        draft
            .as_object_mut()
            .expect("V2 program object")
            .remove("canonical_sha256");
        draft
    }

    fn d_operator_program() -> Value {
        let mut program = json!({
            "schema_version":"GeometryProgram@2",
            "project_id":"project-d",
            "representation_plan_sha256":"d".repeat(64),
            "operator_catalog_sha256":operator_catalog_sha256(),
            "units":{"length":"meter","angle":"radian","coordinate_system":"right-handed-y-up"},
            "budgets":{
                "max_nodes":32,
                "max_triangles":100000,
                "max_glb_bytes":67108864,
                "max_worker_memory_bytes":536870912,
                "max_runtime_ms":10000
            },
            "nodes":[
                {"node_id":"base","operator_id":"forgecad.geometry.primitive@2","inputs":[],"parameters":{"shape":"box","size_m":[0.8,0.8,0.8],"position_m":[-1.5,0.0,0.0],"rotation_rad":[0.0,0.0,0.0]}},
                {"node_id":"transformed","operator_id":"forgecad.geometry.transform@2","inputs":["base"],"parameters":{"shape":"transform","translation_m":[0.0,0.6,0.0],"rotation_rad":[0.0,0.2,0.0],"scale":[1.0,0.8,1.0]}},
                {"node_id":"mirrored","operator_id":"forgecad.geometry.mirror@1","inputs":["transformed"],"parameters":{"shape":"mirror","axis":"x","offset_m":0.0}},
                {"node_id":"arrayed","operator_id":"forgecad.geometry.array@1","inputs":["mirrored"],"parameters":{"shape":"array","count":2,"offset_m":[1.0,0.0,0.0]}},
                {"node_id":"panel","operator_id":"forgecad.geometry.panel@1","inputs":[],"parameters":{"shape":"panel","size_m":[1.6,0.8,0.3],"thickness_m":0.18,"bevel_m":0.08,"position_m":[0.0,1.0,0.0],"rotation_rad":[0.0,0.0,0.0]}},
                {"node_id":"vent","operator_id":"forgecad.geometry.vent-array@1","inputs":[],"parameters":{"shape":"vent-array","width_m":1.2,"height_m":0.6,"depth_m":0.18,"slot_count":4,"slot_width_m":0.12,"slot_spacing_m":0.12,"position_m":[0.0,1.0,0.25],"rotation_rad":[0.0,0.0,0.0]}},
                {"node_id":"joint","operator_id":"forgecad.geometry.joint-stack@1","inputs":[],"parameters":{"shape":"joint-stack","radius_m":0.22,"depth_m":0.12,"ring_count":3,"ring_spacing_m":0.18,"radial_segments":12,"position_m":[-1.0,0.0,0.0],"rotation_rad":[0.0,0.0,0.0]}},
                {"node_id":"extrude","operator_id":"forgecad.geometry.profile-extrude@1","inputs":[],"parameters":{"shape":"profile-extrude","profile":[[-0.3,-0.2],[0.3,-0.2],[0.35,0.15],[0.0,0.3],[-0.35,0.15]],"depth_m":0.25,"position_m":[1.0,0.5,0.0],"rotation_rad":[0.0,0.0,0.0]}},
                {"node_id":"loft","operator_id":"forgecad.geometry.profile-loft@1","inputs":[],"parameters":{"shape":"profile-loft","profiles":[{"height_m":0.0,"points":[[-0.3,-0.2],[0.3,-0.2],[0.3,0.2],[-0.3,0.2]]},{"height_m":0.4,"points":[[-0.2,-0.12],[0.2,-0.12],[0.2,0.12],[-0.2,0.12]]}],"position_m":[1.0,1.0,0.0],"rotation_rad":[0.0,0.0,0.0]}},
                {"node_id":"longitudinal-loft","operator_id":"forgecad.geometry.longitudinal-section-loft@1","inputs":[],"parameters":{"shape":"longitudinal-section-loft","sections":[{"station_m":-0.6,"points":[[-0.18,-0.12],[0.18,-0.12],[0.24,0.0],[0.18,0.12],[-0.18,0.12],[-0.24,0.0]]},{"station_m":0.0,"points":[[-0.30,-0.20],[0.30,-0.20],[0.38,0.0],[0.30,0.20],[-0.30,0.20],[-0.38,0.0]]},{"station_m":0.8,"points":[[-0.16,-0.10],[0.16,-0.10],[0.22,0.0],[0.16,0.10],[-0.16,0.10],[-0.22,0.0]]}],"position_m":[0.0,0.0,0.0],"rotation_rad":[0.0,0.0,0.0]}},
                {"node_id":"revolve","operator_id":"forgecad.geometry.revolve@1","inputs":[],"parameters":{"shape":"revolve","profile":[[0.2,-0.2],[0.35,0.0],[0.2,0.2]],"radial_segments":16,"position_m":[-1.0,1.0,0.0],"rotation_rad":[0.0,0.0,0.0]}},
                {"node_id":"sweep","operator_id":"forgecad.geometry.tube-sweep@1","inputs":[],"parameters":{"shape":"tube-sweep","path":[[-0.5,0.0,0.0],[0.0,0.3,0.2],[0.5,0.0,0.0]],"radius_m":0.08,"radial_segments":12,"cap_ends":true,"position_m":[0.0,1.8,0.0],"rotation_rad":[0.0,0.0,0.0]}},
                {"node_id":"aggregate","operator_id":"forgecad.geometry.part-output@1","inputs":["panel","vent"],"parameters":{"shape":"part-output"}}
            ],
            "part_outputs":[
                {"part_id":"arrayed-part","input_node_ids":["arrayed"],"material_zone_id":"zone-white-shell","solid":true},
                {"part_id":"panel-vent","input_node_ids":["aggregate"],"material_zone_id":"zone-black-mechanical","solid":true},
                {"part_id":"joint-part","input_node_ids":["joint"],"material_zone_id":"zone-black-mechanical","solid":true},
                {"part_id":"extrude-part","input_node_ids":["extrude"],"material_zone_id":"zone-white-shell","solid":true},
                {"part_id":"loft-part","input_node_ids":["loft"],"material_zone_id":"zone-white-shell","solid":true},
                {"part_id":"longitudinal-loft-part","input_node_ids":["longitudinal-loft"],"material_zone_id":"zone-white-shell","solid":true},
                {"part_id":"revolve-part","input_node_ids":["revolve"],"material_zone_id":"zone-black-mechanical","solid":true},
                {"part_id":"sweep-part","input_node_ids":["sweep"],"material_zone_id":"zone-emissive-amber","solid":true}
            ]
        });
        let hash = canonical_hash(&program);
        program
            .as_object_mut()
            .expect("D program object")
            .insert("canonical_sha256".to_owned(), Value::String(hash));
        program
    }

    #[test]
    fn mcp010d_hard_surface_operators_compile_with_deterministic_lineage() {
        let program = d_operator_program();
        let first = compile_geometry_program(&program).expect("D operator program");
        let second = compile_geometry_program(&program).expect("D operator program second");
        assert_eq!(first.glb, second.glb);
        assert_eq!(first.part_ids.len(), 8);
        assert!(first.triangle_count > 200);
        let inspection = integrity::inspect_glb(&first.glb).expect("D strict readback");
        assert!(
            inspection.hard_gate_passed,
            "{:?}",
            inspection.failure_codes
        );
        assert_eq!(
            inspection.operator_catalog_sha256.as_deref(),
            Some(operator_catalog_sha256().as_str())
        );
        assert!(inspection.source_node_ids.iter().any(|id| id == "arrayed"));
        assert!(inspection
            .source_node_ids
            .iter()
            .any(|id| id == "aggregate"));
        assert!(inspection
            .source_node_ids
            .iter()
            .any(|id| id == "longitudinal-loft"));
    }

    fn bevel_normal_program() -> Value {
        let mut program = json!({
            "schema_version":"GeometryProgram@2",
            "project_id":"project-bevel-normal",
            "representation_plan_sha256":"e".repeat(64),
            "operator_catalog_sha256":operator_catalog_sha256(),
            "units":{"length":"meter","angle":"radian","coordinate_system":"right-handed-y-up"},
            "budgets":{"max_nodes":8,"max_triangles":1000,"max_glb_bytes":4194304,"max_worker_memory_bytes":134217728,"max_runtime_ms":5000},
            "nodes":[
                {"node_id":"box","operator_id":"forgecad.geometry.primitive@2","inputs":[],"parameters":{"shape":"box","size_m":[1.2,0.8,0.6],"position_m":[0.2,0.4,-0.1],"rotation_rad":[0.1,0.2,0.05]}},
                {"node_id":"beveled","operator_id":"forgecad.geometry.bevel@1","inputs":["box"],"parameters":{"shape":"bevel","width_m":0.08,"segments":2,"profile":0.5,"edge_scope":"all-source-box-edges","clamp_overlap":false}},
                {"node_id":"shaded","operator_id":"forgecad.geometry.normal-policy@1","inputs":["beveled"],"parameters":{"shape":"normal-policy","weighting":"face-area-x-corner-angle","crease_angle_rad":1.0471975511965976,"keep_sharp":true,"output_domain":"corner"}}
            ],
            "part_outputs":[{"part_id":"rounded-shell","input_node_ids":["shaded"],"material_zone_id":"zone-white-shell","solid":true}]
        });
        let hash = canonical_hash(&program);
        program["canonical_sha256"] = Value::String(hash);
        program
    }

    #[test]
    fn bevel_and_corner_normal_policy_compile_deterministically_with_strict_readback() {
        let program = bevel_normal_program();
        let first = compile_geometry_program(&program).expect("bevel normal program");
        let second = compile_geometry_program(&program).expect("bevel normal program repeat");
        assert_eq!(first.glb, second.glb);
        assert_eq!(first.triangle_count, 300);
        let inspection = integrity::inspect_glb(&first.glb).expect("strict bevel readback");
        assert!(
            inspection.hard_gate_passed,
            "{:?}",
            inspection.failure_codes
        );
        assert_eq!(inspection.triangle_count, 300);
        assert!(inspection.source_node_ids.iter().any(|id| id == "shaded"));
        let (root, _) = glb_root_and_bin_offset(&first.glb);
        assert_eq!(
            root["meshes"][0]["primitives"][0]["extras"]["lineage_source_node_ids"],
            json!(["box"])
        );
    }

    #[test]
    fn bevel_rejects_non_source_box_and_overlap_without_clamp() {
        let mut non_source = bevel_normal_program();
        non_source["nodes"][1]["inputs"][0] = Value::String("moved".to_owned());
        non_source["nodes"].as_array_mut().expect("nodes").insert(
            1,
            json!({"node_id":"moved","operator_id":"forgecad.geometry.transform@2","inputs":["box"],"parameters":{"shape":"transform","translation_m":[0.0,0.0,0.0],"rotation_rad":[0.0,0.0,0.0],"scale":[1.0,1.0,1.0]}}),
        );
        non_source["canonical_sha256"] = Value::String(canonical_hash(&without_hash(&non_source)));
        assert!(compile_geometry_program(&non_source).is_err());
        let mut non_source_draft = non_source.clone();
        non_source_draft
            .as_object_mut()
            .expect("draft object")
            .remove("canonical_sha256");
        assert!(geometry_program_v2_draft_hash(&non_source_draft).is_err());

        let mut overlap = bevel_normal_program();
        overlap["nodes"][1]["parameters"]["width_m"] = json!(0.5);
        overlap["canonical_sha256"] = Value::String(canonical_hash(&without_hash(&overlap)));
        assert!(compile_geometry_program(&overlap).is_err());
    }

    #[test]
    fn boolean_rejects_an_upstream_normal_policy_at_draft_hash_boundary() {
        let draft = json!({
            "schema_version":"GeometryProgram@2",
            "project_id":"project-normal-before-boolean",
            "representation_plan_sha256":"d".repeat(64),
            "operator_catalog_sha256":operator_catalog_sha256(),
            "units":{"length":"meter","angle":"radian","coordinate_system":"right-handed-y-up"},
            "budgets":{"max_nodes":8,"max_triangles":1000,"max_glb_bytes":4194304,"max_worker_memory_bytes":134217728,"max_runtime_ms":5000},
            "nodes":[
                {"node_id":"left","operator_id":"forgecad.geometry.primitive@2","inputs":[],"parameters":{"shape":"box","size_m":[1.0,1.0,1.0],"position_m":[0.0,0.0,0.0],"rotation_rad":[0.0,0.0,0.0]}},
                {"node_id":"shaded-left","operator_id":"forgecad.geometry.normal-policy@1","inputs":["left"],"parameters":{"shape":"normal-policy","weighting":"face-area-x-corner-angle","crease_angle_rad":1.0471975511965976,"keep_sharp":true,"output_domain":"corner"}},
                {"node_id":"right","operator_id":"forgecad.geometry.primitive@2","inputs":[],"parameters":{"shape":"box","size_m":[1.0,1.0,1.0],"position_m":[0.25,0.0,0.0],"rotation_rad":[0.0,0.0,0.0]}},
                {"node_id":"combined","operator_id":"forgecad.geometry.boolean@1","inputs":["shaded-left","right"],"parameters":{"shape":"union"}}
            ],
            "part_outputs":[{"part_id":"combined-part","input_node_ids":["combined"],"material_zone_id":"zone-black-mechanical","solid":true}]
        });
        assert!(geometry_program_v2_draft_hash(&draft).is_err());
    }

    fn multi_loop_profile_loft_program() -> Value {
        let mut program = json!({
            "schema_version":"GeometryProgram@2",
            "project_id":"project-multi-loop-profile-loft",
            "representation_plan_sha256":"a".repeat(64),
            "operator_catalog_sha256":operator_catalog_sha256(),
            "units":{"length":"meter","angle":"radian","coordinate_system":"right-handed-y-up"},
            "budgets":{
                "max_nodes":1,
                "max_triangles":100000,
                "max_glb_bytes":67108864,
                "max_worker_memory_bytes":536870912,
                "max_runtime_ms":10000
            },
            "nodes":[{
                "node_id":"receiver-negative-space",
                "operator_id":"forgecad.geometry.multi-loop-profile-loft@1",
                "inputs":[],
                "parameters":{
                    "shape":"multi-loop-profile-loft",
                    "stations":[
                        {
                            "station_id":"rear",
                            "station_m":-1.0,
                            "components":[
                                {
                                    "component_id":"shell",
                                    "outer":{"points":[[-1.2,-0.8],[1.2,-0.8],[1.2,0.8],[-1.2,0.8]],"corner_indices":[0,1,2,3]},
                                    "holes":[{"hole_id":"void","points":[[-0.4,-0.25],[-0.4,0.25],[0.4,0.25],[0.4,-0.25]],"corner_indices":[0,1,2,3]}]
                                },
                                {
                                    "component_id":"island",
                                    "outer":{"points":[[-0.15,-0.1],[0.15,-0.1],[0.15,0.1],[-0.15,0.1]],"corner_indices":[0,1,2,3]},
                                    "holes":[]
                                }
                            ]
                        },
                        {
                            "station_id":"front",
                            "station_m":1.5,
                            "components":[
                                {
                                    "component_id":"island",
                                    "outer":{"points":[[-0.12,-0.08],[0.12,-0.08],[0.12,0.08],[-0.12,0.08]],"corner_indices":[0,1,2,3]},
                                    "holes":[]
                                },
                                {
                                    "component_id":"shell",
                                    "outer":{"points":[[-1.0,-0.7],[1.0,-0.7],[1.0,0.7],[-1.0,0.7]],"corner_indices":[0,1,2,3]},
                                    "holes":[{"hole_id":"void","points":[[-0.32,-0.2],[-0.32,0.2],[0.32,0.2],[0.32,-0.2]],"corner_indices":[0,1,2,3]}]
                                }
                            ]
                        }
                    ],
                    "resample_points":8,
                    "interpolation":"linear",
                    "interpolation_rings":1,
                    "preserve_corners":true,
                    "position_m":[0.0,0.0,0.0],
                    "rotation_rad":[0.0,0.0,0.0]
                }
            }],
            "part_outputs":[{
                "part_id":"receiver-core",
                "input_node_ids":["receiver-negative-space"],
                "material_zone_id":"zone-white-shell",
                "solid":true
            }]
        });
        program["canonical_sha256"] = Value::String(canonical_hash(&program));
        program
    }

    #[test]
    fn multi_loop_profile_loft_compiles_to_deterministic_strict_glb_with_lineage() {
        let program = multi_loop_profile_loft_program();
        let first = compile_geometry_program(&program).expect("multi-loop profile loft artifact");
        let second =
            compile_geometry_program(&program).expect("multi-loop profile loft repeat artifact");
        assert_eq!(first.glb, second.glb);
        assert_eq!(first.program_sha256, second.program_sha256);
        assert!(first.triangle_count > 0);

        let inspection = integrity::inspect_glb(&first.glb).expect("multi-loop strict readback");
        assert!(
            inspection.hard_gate_passed,
            "{:?}",
            inspection.failure_codes
        );
        assert_eq!(inspection.boundary_edge_count, 0);
        assert_eq!(inspection.non_manifold_edge_count, 0);
        assert_eq!(inspection.part_bindings.len(), 1);
        assert!(inspection.part_bindings[0].solid);
        assert_eq!(inspection.part_bindings[0].part_id, "receiver-core");
        assert!(inspection
            .source_node_ids
            .iter()
            .any(|id| id == "receiver-negative-space"));
    }

    fn surface_patch_program() -> Value {
        let control_points = (0..4)
            .flat_map(|v| {
                (0..4).map(move |u| {
                    let x = -0.8 + u as f64 * 0.5333333333;
                    let y = 1.2 + v as f64 * 0.4;
                    let edge = (u as f64 - 1.5).abs() / 1.5;
                    let crown = (1.0 - edge * edge) * 0.16;
                    json!([x, y, crown])
                })
            })
            .collect::<Vec<_>>();
        let mut program = json!({
            "schema_version":"GeometryProgram@2",
            "project_id":"project-surface-patch",
            "representation_plan_sha256":"f".repeat(64),
            "operator_catalog_sha256":operator_catalog_sha256(),
            "units":{"length":"meter","angle":"radian","coordinate_system":"right-handed-y-up"},
            "budgets":{
                "max_nodes":4,
                "max_triangles":10000,
                "max_glb_bytes":67108864,
                "max_worker_memory_bytes":536870912,
                "max_runtime_ms":10000
            },
            "nodes":[{
                "node_id":"chest-surface",
                "operator_id":"forgecad.geometry.surface-patch@1",
                "inputs":[],
                "parameters":{
                    "shape":"surface-patch",
                    "control_points":control_points,
                    "u_segments":8,
                    "v_segments":8,
                    "position_m":[0.0,0.0,0.0],
                    "rotation_rad":[0.0,0.0,0.0]
                }
            }],
            "part_outputs":[{
                "part_id":"chest-surface",
                "input_node_ids":["chest-surface"],
                "material_zone_id":"zone-white-shell",
                "solid":false
            }]
        });
        program["canonical_sha256"] = Value::String(canonical_hash(&without_hash(&program)));
        program
    }

    #[test]
    fn mcp010f_surface_patch_is_smooth_deterministic_and_explicitly_open() {
        let program = surface_patch_program();
        let first = compile_geometry_program(&program).expect("surface patch artifact");
        let second = compile_geometry_program(&program).expect("surface patch repeat");
        assert_eq!(first.glb, second.glb);
        assert_eq!(first.triangle_count, 128);
        let inspection = integrity::inspect_glb(&first.glb).expect("surface patch readback");
        assert!(
            inspection.hard_gate_passed,
            "{:?}",
            inspection.failure_codes
        );
        assert_eq!(inspection.boundary_edge_count, 0);
        assert_eq!(inspection.non_manifold_edge_count, 0);
        assert_eq!(inspection.part_bindings.len(), 1);
        assert!(!inspection.part_bindings[0].solid);
        assert_eq!(inspection.part_bindings[0].part_id, "chest-surface");

        let mut invalid = program;
        invalid["nodes"][0]["parameters"]["u_segments"] = json!(3);
        invalid["canonical_sha256"] = Value::String(canonical_hash(&without_hash(&invalid)));
        assert!(compile_geometry_program(&invalid).is_err());
    }

    fn surface_shell_program() -> Value {
        let mut program = surface_patch_program();
        program["project_id"] = json!("project-surface-shell");
        program["nodes"][0]["operator_id"] = json!("forgecad.geometry.surface-shell@1");
        program["nodes"][0]["parameters"]["shape"] = json!("surface-shell");
        program["nodes"][0]["parameters"]["thickness_m"] = json!(0.08);
        program["part_outputs"][0]["solid"] = json!(true);
        program["canonical_sha256"] = Value::String(canonical_hash(&without_hash(&program)));
        program
    }

    #[test]
    fn mcp010f_surface_shell_is_watertight_deterministic_and_thickness_bounded() {
        let program = surface_shell_program();
        let first = compile_geometry_program(&program).expect("surface shell artifact");
        let second = compile_geometry_program(&program).expect("surface shell repeat");
        assert_eq!(first.glb, second.glb);
        assert_eq!(first.triangle_count, 320);
        let inspection = integrity::inspect_glb(&first.glb).expect("surface shell readback");
        assert!(
            inspection.hard_gate_passed,
            "{:?}",
            inspection.failure_codes
        );
        assert_eq!(inspection.boundary_edge_count, 0);
        assert_eq!(inspection.non_manifold_edge_count, 0);
        assert_eq!(inspection.part_bindings.len(), 1);
        assert!(inspection.part_bindings[0].solid);

        let mut invalid = program;
        invalid["nodes"][0]["parameters"]["thickness_m"] = json!(0.00001);
        invalid["canonical_sha256"] = Value::String(canonical_hash(&without_hash(&invalid)));
        assert!(compile_geometry_program(&invalid).is_err());
    }

    fn subd_cage_program(subdivision_levels: usize) -> Value {
        let control_points = (0..3)
            .flat_map(|v| {
                (0..3).map(move |u| {
                    let x = -0.9 + u as f64 * 0.9;
                    let y = 1.0 + v as f64 * 0.55;
                    let z = 0.12 * (1.0 - (u as f64 - 1.0).abs() * 0.35) + 0.06 * v as f64;
                    json!([x, y, z])
                })
            })
            .collect::<Vec<_>>();
        let mut program = json!({
            "schema_version":"GeometryProgram@2",
            "project_id":format!("project-subd-cage-{subdivision_levels}"),
            "representation_plan_sha256":"a".repeat(64),
            "operator_catalog_sha256":operator_catalog_sha256(),
            "units":{"length":"meter","angle":"radian","coordinate_system":"right-handed-y-up"},
            "budgets":{
                "max_nodes":4,
                "max_triangles":10000,
                "max_glb_bytes":67108864,
                "max_worker_memory_bytes":536870912,
                "max_runtime_ms":10000
            },
            "nodes":[{
                "node_id":"subd-chest-cage",
                "operator_id":"forgecad.geometry.subd-cage@1",
                "inputs":[],
                "parameters":{
                    "shape":"subd-cage",
                    "control_points":control_points,
                    "u_points":3,
                    "v_points":3,
                    "subdivision_levels":subdivision_levels,
                    "position_m":[0.0,0.0,0.0],
                    "rotation_rad":[0.0,0.0,0.0]
                }
            }],
            "part_outputs":[{
                "part_id":"subd-chest-cage",
                "input_node_ids":["subd-chest-cage"],
                "material_zone_id":"zone-white-shell",
                "solid":false
            }]
        });
        program["canonical_sha256"] = Value::String(canonical_hash(&without_hash(&program)));
        program
    }

    #[test]
    fn mcp010f_subd_cage_is_editable_deterministic_and_bounded() {
        let level_one = subd_cage_program(1);
        let first = compile_geometry_program(&level_one).expect("subd cage level one artifact");
        let second = compile_geometry_program(&level_one).expect("subd cage level one repeat");
        assert_eq!(first.glb, second.glb);
        assert_eq!(first.triangle_count, 32);
        let inspection = integrity::inspect_glb(&first.glb).expect("subd cage readback");
        assert!(
            inspection.hard_gate_passed,
            "{:?}",
            inspection.failure_codes
        );
        assert_eq!(inspection.part_bindings.len(), 1);
        assert!(!inspection.part_bindings[0].solid);
        assert!(compile_geometry_program(&subd_cage_program(2)).is_ok());

        let mut invalid = level_one;
        invalid["nodes"][0]["parameters"]["control_points"] = json!([
            [-0.9, 1.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.9, 1.0, 0.0],
            [-0.9, 1.55, 0.0],
            [0.0, 1.55, 0.0],
            [0.9, 1.55, 0.0],
            [-0.9, 2.1, 0.0],
            [0.0, 2.1, 0.0]
        ]);
        invalid["canonical_sha256"] = Value::String(canonical_hash(&without_hash(&invalid)));
        assert!(compile_geometry_program(&invalid).is_err());
    }

    fn subd_crease_program(sharpness_levels: u64) -> Value {
        let control_points = vec![
            json!([-1.0, -1.0, 0.0]),
            json!([0.0, -1.0, 0.0]),
            json!([1.0, -1.0, 0.0]),
            json!([-1.0, 0.0, 0.0]),
            json!([0.0, 0.0, 1.0]),
            json!([1.0, 0.0, 0.0]),
            json!([-1.0, 1.0, 0.0]),
            json!([0.0, 1.0, 0.0]),
            json!([1.0, 1.0, 0.0]),
        ];
        let mut program = json!({
            "schema_version":"GeometryProgram@2",
            "project_id":format!("project-subd-crease-{sharpness_levels}"),
            "representation_plan_sha256":"a".repeat(64),
            "operator_catalog_sha256":operator_catalog_sha256(),
            "units":{"length":"meter","angle":"radian","coordinate_system":"right-handed-y-up"},
            "budgets":{
                "max_nodes":4,
                "max_triangles":128,
                "max_glb_bytes":67108864,
                "max_worker_memory_bytes":536870912,
                "max_runtime_ms":10000
            },
            "nodes":[{
                "node_id":"subd-crease-cage",
                "operator_id":"forgecad.geometry.subd-cage@2",
                "inputs":[],
                "parameters":{
                    "shape":"subd-cage",
                    "control_points":control_points,
                    "u_points":3,
                    "v_points":3,
                    "subdivision_levels":2,
                    "crease_method":"uniform-integer-level-decay@1",
                    "crease_edges":[
                        {"vertex_a":3,"vertex_b":4,"sharpness_levels":sharpness_levels},
                        {"vertex_a":4,"vertex_b":5,"sharpness_levels":sharpness_levels}
                    ],
                    "position_m":[0.0,0.0,0.0],
                    "rotation_rad":[0.0,0.0,0.0]
                }
            }],
            "part_outputs":[{
                "part_id":"subd-crease-cage",
                "input_node_ids":["subd-crease-cage"],
                "material_zone_id":"zone-white-shell",
                "solid":false
            }]
        });
        program["canonical_sha256"] = Value::String(canonical_hash(&without_hash(&program)));
        program
    }

    #[test]
    fn blender_clean_room_subd_creases_compile_to_deterministic_strict_glb() {
        let level_one_sharpness = subd_crease_program(1);
        let first = compile_geometry_program(&level_one_sharpness).expect("crease GLB");
        let repeat = compile_geometry_program(&level_one_sharpness).expect("crease repeat GLB");
        assert_eq!(first.glb, repeat.glb);
        assert_eq!(first.triangle_count, 128);
        let inspection = integrity::inspect_glb(&first.glb).expect("crease strict readback");
        assert!(
            inspection.hard_gate_passed,
            "{:?}",
            inspection.failure_codes
        );
        assert_eq!(inspection.triangle_count, 128);
        assert_eq!(inspection.part_bindings.len(), 1);
        assert!(!inspection.part_bindings[0].solid);

        let level_two_sharpness = subd_crease_program(2);
        let sharper = compile_geometry_program(&level_two_sharpness).expect("sharper crease GLB");
        assert_eq!(sharper.triangle_count, 128);
        assert_ne!(first.glb, sharper.glb);

        let mut fractional = level_two_sharpness.clone();
        fractional["nodes"][0]["parameters"]["crease_edges"][0]["sharpness_levels"] = json!(1.5);
        fractional["canonical_sha256"] = Value::String(canonical_hash(&without_hash(&fractional)));
        assert!(compile_geometry_program(&fractional).is_err());

        let mut stale_catalog = level_two_sharpness;
        stale_catalog["operator_catalog_sha256"] = json!("f".repeat(64));
        stale_catalog["canonical_sha256"] =
            Value::String(canonical_hash(&without_hash(&stale_catalog)));
        assert!(compile_geometry_program(&stale_catalog).is_err());
    }

    fn authoring_mesh_program() -> Value {
        let mut program = json!({
            "schema_version":"GeometryProgram@2",
            "project_id":"project-authoring-mesh",
            "representation_plan_sha256":"b".repeat(64),
            "operator_catalog_sha256":operator_catalog_sha256(),
            "units":{"length":"meter","angle":"radian","coordinate_system":"right-handed-y-up"},
            "budgets":{
                "max_nodes":1,
                "max_triangles":8,
                "max_glb_bytes":67108864,
                "max_worker_memory_bytes":536870912,
                "max_runtime_ms":10000
            },
            "nodes":[{
                "node_id":"authored-panel",
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
                "part_id":"authored-panel",
                "input_node_ids":["authored-panel"],
                "material_zone_id":"zone-authored-shell",
                "solid":false
            }]
        });
        program["canonical_sha256"] = Value::String(canonical_hash(&without_hash(&program)));
        program
    }

    #[test]
    fn clean_room_authoring_mesh_compiles_deterministically_and_fails_closed() {
        let program = authoring_mesh_program();
        let first = compile_geometry_program(&program).expect("authoring mesh GLB");
        let repeat = compile_geometry_program(&program).expect("authoring mesh repeat GLB");
        assert_eq!(first.glb, repeat.glb);
        assert_eq!(first.triangle_count, 2);
        let inspection = integrity::inspect_glb(&first.glb).expect("strict readback");
        assert!(
            inspection.hard_gate_passed,
            "{:?}",
            inspection.failure_codes
        );
        assert_eq!(inspection.triangle_count, 2);
        assert_eq!(inspection.part_bindings.len(), 1);
        assert!(!inspection.part_bindings[0].solid);

        let mut wrong_winding = program.clone();
        wrong_winding["nodes"][0]["parameters"]["loops"][3]["edge_forward"] = json!(true);
        wrong_winding["canonical_sha256"] =
            Value::String(canonical_hash(&without_hash(&wrong_winding)));
        assert!(compile_geometry_program(&wrong_winding).is_err());

        let mut dangling = program.clone();
        dangling["nodes"][0]["parameters"]["loops"][0]["vertex_id"] = json!("missing");
        dangling["canonical_sha256"] = Value::String(canonical_hash(&without_hash(&dangling)));
        assert!(compile_geometry_program(&dangling).is_err());

        let mut unused_vertex = program.clone();
        unused_vertex["nodes"][0]["parameters"]["vertices"]
            .as_array_mut()
            .unwrap()
            .push(json!({"element_id":"v4","position_m":[0.0,0.0,1.0]}));
        unused_vertex["canonical_sha256"] =
            Value::String(canonical_hash(&without_hash(&unused_vertex)));
        assert!(compile_geometry_program(&unused_vertex).is_err());

        let mut duplicate_vertex_id = program.clone();
        duplicate_vertex_id["nodes"][0]["parameters"]["vertices"][1]["element_id"] = json!("v0");
        duplicate_vertex_id["canonical_sha256"] =
            Value::String(canonical_hash(&without_hash(&duplicate_vertex_id)));
        assert!(compile_geometry_program(&duplicate_vertex_id).is_err());

        let mut coordinate_overflow = program.clone();
        coordinate_overflow["nodes"][0]["parameters"]["vertices"][0]["position_m"][0] = json!(10.1);
        coordinate_overflow["canonical_sha256"] =
            Value::String(canonical_hash(&without_hash(&coordinate_overflow)));
        assert!(compile_geometry_program(&coordinate_overflow).is_err());

        let mut triangle_budget_overflow = program.clone();
        triangle_budget_overflow["budgets"]["max_triangles"] = json!(1);
        triangle_budget_overflow["canonical_sha256"] =
            Value::String(canonical_hash(&without_hash(&triangle_budget_overflow)));
        assert!(compile_geometry_program(&triangle_budget_overflow).is_err());

        let mut vertex_array_overflow = program.clone();
        vertex_array_overflow["nodes"][0]["parameters"]["vertices"] = Value::Array(
            (0..1537)
                .map(|index| {
                    json!({
                        "element_id":format!("v-{index:04}"),
                        "position_m":[0.0, 0.0, 0.0]
                    })
                })
                .collect(),
        );
        vertex_array_overflow["canonical_sha256"] =
            Value::String(canonical_hash(&without_hash(&vertex_array_overflow)));
        assert!(compile_geometry_program(&vertex_array_overflow).is_err());

        let mut repeated_vertex_and_edge = program.clone();
        repeated_vertex_and_edge["nodes"][0]["parameters"]["loops"] = json!([
            {"element_id":"l0","face_id":"f0","ordinal":0,"vertex_id":"v0","edge_id":"e01","edge_forward":true},
            {"element_id":"l1","face_id":"f0","ordinal":1,"vertex_id":"v1","edge_id":"e12","edge_forward":true},
            {"element_id":"l2","face_id":"f0","ordinal":2,"vertex_id":"v2","edge_id":"e12","edge_forward":false},
            {"element_id":"l3","face_id":"f0","ordinal":3,"vertex_id":"v1","edge_id":"e01","edge_forward":false}
        ]);
        repeated_vertex_and_edge["nodes"][0]["parameters"]["faces"] =
            json!([{"element_id":"f0","loop_ids":["l0","l1","l2","l3"]}]);
        repeated_vertex_and_edge["canonical_sha256"] =
            Value::String(canonical_hash(&without_hash(&repeated_vertex_and_edge)));
        assert!(compile_geometry_program(&repeated_vertex_and_edge).is_err());

        let mut duplicate_face = program.clone();
        duplicate_face["nodes"][0]["parameters"]["vertices"] = json!([
            {"element_id":"v0","position_m":[0.0,0.0,0.0]},
            {"element_id":"v1","position_m":[1.0,0.0,0.0]},
            {"element_id":"v2","position_m":[0.0,1.0,0.0]}
        ]);
        duplicate_face["nodes"][0]["parameters"]["edges"] = json!([
            {"element_id":"e01","vertex_ids":["v0","v1"]},
            {"element_id":"e02","vertex_ids":["v0","v2"]},
            {"element_id":"e12","vertex_ids":["v1","v2"]}
        ]);
        duplicate_face["nodes"][0]["parameters"]["loops"] = json!([
            {"element_id":"l00","face_id":"f0","ordinal":0,"vertex_id":"v0","edge_id":"e01","edge_forward":true},
            {"element_id":"l01","face_id":"f0","ordinal":1,"vertex_id":"v1","edge_id":"e12","edge_forward":true},
            {"element_id":"l02","face_id":"f0","ordinal":2,"vertex_id":"v2","edge_id":"e02","edge_forward":false},
            {"element_id":"l10","face_id":"f1","ordinal":0,"vertex_id":"v0","edge_id":"e02","edge_forward":true},
            {"element_id":"l11","face_id":"f1","ordinal":1,"vertex_id":"v2","edge_id":"e12","edge_forward":false},
            {"element_id":"l12","face_id":"f1","ordinal":2,"vertex_id":"v1","edge_id":"e01","edge_forward":false}
        ]);
        duplicate_face["nodes"][0]["parameters"]["faces"] = json!([
            {"element_id":"f0","loop_ids":["l00","l01","l02"]},
            {"element_id":"f1","loop_ids":["l10","l11","l12"]}
        ]);
        duplicate_face["canonical_sha256"] =
            Value::String(canonical_hash(&without_hash(&duplicate_face)));
        assert!(compile_geometry_program(&duplicate_face).is_err());

        let mut non_manifold = program.clone();
        non_manifold["nodes"][0]["parameters"]["vertices"] = json!([
            {"element_id":"v0","position_m":[0.0,0.0,0.0]},
            {"element_id":"v1","position_m":[1.0,0.0,0.0]},
            {"element_id":"v2","position_m":[0.0,1.0,0.0]},
            {"element_id":"v3","position_m":[0.0,-1.0,0.0]},
            {"element_id":"v4","position_m":[0.0,0.0,1.0]}
        ]);
        non_manifold["nodes"][0]["parameters"]["edges"] = json!([
            {"element_id":"e01","vertex_ids":["v0","v1"]},
            {"element_id":"e02","vertex_ids":["v0","v2"]},
            {"element_id":"e03","vertex_ids":["v0","v3"]},
            {"element_id":"e04","vertex_ids":["v0","v4"]},
            {"element_id":"e12","vertex_ids":["v1","v2"]},
            {"element_id":"e13","vertex_ids":["v1","v3"]},
            {"element_id":"e14","vertex_ids":["v1","v4"]}
        ]);
        non_manifold["nodes"][0]["parameters"]["loops"] = json!([
            {"element_id":"l00","face_id":"f0","ordinal":0,"vertex_id":"v0","edge_id":"e01","edge_forward":true},
            {"element_id":"l01","face_id":"f0","ordinal":1,"vertex_id":"v1","edge_id":"e12","edge_forward":true},
            {"element_id":"l02","face_id":"f0","ordinal":2,"vertex_id":"v2","edge_id":"e02","edge_forward":false},
            {"element_id":"l10","face_id":"f1","ordinal":0,"vertex_id":"v0","edge_id":"e03","edge_forward":true},
            {"element_id":"l11","face_id":"f1","ordinal":1,"vertex_id":"v3","edge_id":"e13","edge_forward":false},
            {"element_id":"l12","face_id":"f1","ordinal":2,"vertex_id":"v1","edge_id":"e01","edge_forward":false},
            {"element_id":"l20","face_id":"f2","ordinal":0,"vertex_id":"v0","edge_id":"e01","edge_forward":true},
            {"element_id":"l21","face_id":"f2","ordinal":1,"vertex_id":"v1","edge_id":"e14","edge_forward":true},
            {"element_id":"l22","face_id":"f2","ordinal":2,"vertex_id":"v4","edge_id":"e04","edge_forward":false}
        ]);
        non_manifold["nodes"][0]["parameters"]["faces"] = json!([
            {"element_id":"f0","loop_ids":["l00","l01","l02"]},
            {"element_id":"f1","loop_ids":["l10","l11","l12"]},
            {"element_id":"f2","loop_ids":["l20","l21","l22"]}
        ]);
        non_manifold["canonical_sha256"] =
            Value::String(canonical_hash(&without_hash(&non_manifold)));
        assert!(compile_geometry_program(&non_manifold).is_err());

        for field in ["python", "path", "url", "env", "plugin", "network"] {
            let mut executable = program.clone();
            executable["nodes"][0]["parameters"][field] = json!("forbidden");
            executable["canonical_sha256"] =
                Value::String(canonical_hash(&without_hash(&executable)));
            assert!(compile_geometry_program(&executable).is_err());
        }
    }

    fn authoring_box_bevel_v2_program(
        source_edge_id: &str,
        width_m: f32,
        segments: usize,
        profile: f32,
        clamp_overlap: bool,
    ) -> Value {
        let vertex_values = [
            ("v000", [-1.0, -1.0, -1.0]),
            ("v001", [-1.0, -1.0, 1.0]),
            ("v010", [-1.0, 1.0, -1.0]),
            ("v011", [-1.0, 1.0, 1.0]),
            ("v100", [1.0, -1.0, -1.0]),
            ("v101", [1.0, -1.0, 1.0]),
            ("v110", [1.0, 1.0, -1.0]),
            ("v111", [1.0, 1.0, 1.0]),
        ];
        let face_values = [
            ("f-back", ["v000", "v010", "v110", "v100"]),
            ("f-bottom", ["v000", "v100", "v101", "v001"]),
            ("f-front", ["v001", "v101", "v111", "v011"]),
            ("f-left", ["v000", "v001", "v011", "v010"]),
            ("f-right", ["v100", "v110", "v111", "v101"]),
            ("f-top", ["v010", "v011", "v111", "v110"]),
        ];
        let vertices = vertex_values
            .iter()
            .map(
                |(element_id, position_m)| json!({"element_id":element_id,"position_m":position_m}),
            )
            .collect::<Vec<_>>();
        let mut edge_pairs = BTreeSet::new();
        for (_, face_vertices) in &face_values {
            for ordinal in 0..face_vertices.len() {
                let left = face_vertices[ordinal];
                let right = face_vertices[(ordinal + 1) % face_vertices.len()];
                edge_pairs.insert(if left < right {
                    (left, right)
                } else {
                    (right, left)
                });
            }
        }
        let edges = edge_pairs
            .iter()
            .map(|(left, right)| {
                json!({
                    "element_id":format!("e-{left}-{right}"),
                    "vertex_ids":[left, right]
                })
            })
            .collect::<Vec<_>>();
        let mut loops = Vec::new();
        let mut faces = Vec::new();
        for (face_id, face_vertices) in &face_values {
            let mut loop_ids = Vec::new();
            for ordinal in 0..face_vertices.len() {
                let start = face_vertices[ordinal];
                let end = face_vertices[(ordinal + 1) % face_vertices.len()];
                let (left, right) = if start < end {
                    (start, end)
                } else {
                    (end, start)
                };
                let loop_id = format!("l-{face_id}-{ordinal}");
                loops.push(json!({
                    "element_id":loop_id,
                    "face_id":face_id,
                    "ordinal":ordinal,
                    "vertex_id":start,
                    "edge_id":format!("e-{left}-{right}"),
                    "edge_forward":start == left
                }));
                loop_ids.push(json!(format!("l-{face_id}-{ordinal}")));
            }
            faces.push(json!({"element_id":face_id,"loop_ids":loop_ids}));
        }
        let mut program = json!({
            "schema_version":"GeometryProgram@2",
            "project_id":"project-authoring-edge-bevel",
            "representation_plan_sha256":"c".repeat(64),
            "operator_catalog_sha256":operator_catalog_sha256(),
            "units":{"length":"meter","angle":"radian","coordinate_system":"right-handed-y-up"},
            "budgets":{
                "max_nodes":2,
                "max_triangles":64,
                "max_glb_bytes":67108864,
                "max_worker_memory_bytes":536870912,
                "max_runtime_ms":10000
            },
            "nodes":[
                {
                    "node_id":"authoring-box",
                    "operator_id":"forgecad.geometry.authoring-mesh@1",
                    "inputs":[],
                    "parameters":{
                        "shape":"authoring-mesh",
                        "topology_policy":"triangle-quad-manifold-with-boundary@1",
                        "vertices":vertices,
                        "edges":edges,
                        "loops":loops,
                        "faces":faces,
                        "position_m":[0.0,0.0,0.0],
                        "rotation_rad":[0.0,0.0,0.0]
                    }
                },
                {
                    "node_id":"selected-edge-bevel",
                    "operator_id":"forgecad.geometry.bevel@2",
                    "inputs":["authoring-box"],
                    "parameters":{
                        "shape":"bevel",
                        "source_edge_ids":[source_edge_id],
                        "width_m":width_m,
                        "segments":segments,
                        "profile":profile,
                        "clamp_overlap":clamp_overlap
                    }
                }
            ],
            "part_outputs":[{
                "part_id":"beveled-authoring-box",
                "input_node_ids":["selected-edge-bevel"],
                "material_zone_id":"zone-authored-shell",
                "solid":true
            }]
        });
        program["canonical_sha256"] = Value::String(canonical_hash(&without_hash(&program)));
        program
    }

    #[test]
    fn selected_authoring_edge_bevel_v2_is_watertight_deterministic_and_fail_closed() {
        let program = authoring_box_bevel_v2_program("e-v010-v110", 0.2, 3, 0.5, false);
        let first = compile_geometry_program(&program).expect("selected edge bevel@2 GLB");
        let repeat = compile_geometry_program(&program).expect("deterministic bevel@2 GLB");
        assert_eq!(first.glb, repeat.glb);
        assert_eq!(first.triangle_count, 24);
        let inspection = integrity::inspect_glb(&first.glb).expect("bevel@2 strict readback");
        assert!(
            inspection.hard_gate_passed,
            "{:?}",
            inspection.failure_codes
        );
        assert_eq!(inspection.triangle_count, 24);
        assert_eq!(inspection.part_bindings.len(), 1);
        assert!(inspection.part_bindings[0].solid);

        let one_segment = compile_geometry_program(&authoring_box_bevel_v2_program(
            "e-v010-v110",
            0.2,
            1,
            0.5,
            false,
        ))
        .expect("one segment bevel@2");
        assert_eq!(one_segment.triangle_count, 16);
        assert_ne!(first.glb, one_segment.glb);

        let another_edge = compile_geometry_program(&authoring_box_bevel_v2_program(
            "e-v010-v011",
            0.2,
            3,
            0.5,
            false,
        ))
        .expect("another selected edge");
        assert_ne!(first.glb, another_edge.glb);

        let narrower = compile_geometry_program(&authoring_box_bevel_v2_program(
            "e-v010-v110",
            0.1,
            3,
            0.5,
            false,
        ))
        .expect("narrow bevel@2");
        assert_ne!(first.glb, narrower.glb);

        let softer = compile_geometry_program(&authoring_box_bevel_v2_program(
            "e-v010-v110",
            0.2,
            3,
            0.75,
            false,
        ))
        .expect("profile bevel@2");
        assert_ne!(first.glb, softer.glb);

        assert!(compile_geometry_program(&authoring_box_bevel_v2_program(
            "e-missing",
            0.2,
            3,
            0.5,
            false,
        ))
        .is_err());
        assert!(compile_geometry_program(&authoring_box_bevel_v2_program(
            "e-v010-v110",
            0.8,
            3,
            0.5,
            false,
        ))
        .is_err());
        assert!(compile_geometry_program(&authoring_box_bevel_v2_program(
            "e-v010-v110",
            0.8,
            3,
            0.5,
            true,
        ))
        .is_ok());

        let mut multiple_edges = program.clone();
        multiple_edges["nodes"][1]["parameters"]["source_edge_ids"] =
            json!(["e-v010-v110", "e-v010-v011"]);
        multiple_edges["canonical_sha256"] =
            Value::String(canonical_hash(&without_hash(&multiple_edges)));
        assert!(compile_geometry_program(&multiple_edges).is_err());

        let mut boundary_mesh = authoring_mesh_program();
        boundary_mesh["budgets"]["max_nodes"] = json!(2);
        boundary_mesh["budgets"]["max_triangles"] = json!(64);
        boundary_mesh["nodes"].as_array_mut().unwrap().push(json!({
            "node_id":"boundary-bevel",
            "operator_id":"forgecad.geometry.bevel@2",
            "inputs":["authored-panel"],
            "parameters":{
                "shape":"bevel",
                "source_edge_ids":["e01"],
                "width_m":0.2,
                "segments":2,
                "profile":0.5,
                "clamp_overlap":false
            }
        }));
        boundary_mesh["part_outputs"][0]["input_node_ids"] = json!(["boundary-bevel"]);
        boundary_mesh["part_outputs"][0]["solid"] = json!(true);
        boundary_mesh["canonical_sha256"] =
            Value::String(canonical_hash(&without_hash(&boundary_mesh)));
        assert!(compile_geometry_program(&boundary_mesh).is_err());

        for field in ["python", "path", "url", "env", "plugin", "network"] {
            let mut executable = program.clone();
            executable["nodes"][1]["parameters"][field] = json!("forbidden");
            executable["canonical_sha256"] =
                Value::String(canonical_hash(&without_hash(&executable)));
            assert!(compile_geometry_program(&executable).is_err());
        }
    }

    #[test]
    fn subdivision_topology_lineage_is_complete_deterministic_and_fail_closed() {
        let program = subd_crease_program(2);
        let first = subdivision_topology_lineage_preview(&program, "subd-crease-cage", 25_000)
            .expect("subdivision topology lineage");
        let second = subdivision_topology_lineage_preview(&program, "subd-crease-cage", 25_000)
            .expect("deterministic subdivision topology lineage");
        assert_eq!(first, second);
        assert_eq!(first["schema_version"], "SubdivisionTopologyLineage@1");
        assert_eq!(first["complete"], true);
        assert_eq!(first["runtime_write_performed"], false);
        assert_eq!(first["lineage_element_count"], 442);
        assert_eq!(first["lineage"]["control_counts"]["vertex_count"], 9);
        assert_eq!(first["lineage"]["control_counts"]["edge_count"], 12);
        assert_eq!(first["lineage"]["control_counts"]["quad_count"], 4);
        assert_eq!(first["lineage"]["evaluated_counts"]["vertex_count"], 81);
        assert_eq!(first["lineage"]["evaluated_counts"]["edge_count"], 144);
        assert_eq!(first["lineage"]["evaluated_counts"]["quad_count"], 64);
        assert_eq!(first["lineage"]["evaluated_counts"]["triangle_count"], 128);
        assert!(first["lineage"]["control_edge_to_evaluated_edge_ids"]
            .as_array()
            .unwrap()
            .iter()
            .all(|chain| chain.as_array().unwrap().len() == 4));
        assert!(first["lineage"]["control_quad_descendant_ranges"]
            .as_array()
            .unwrap()
            .iter()
            .all(|range| range["evaluated_quad_count"] == 16
                && range["evaluated_triangle_count"] == 32));
        assert_eq!(first["lineage_sha256"], canonical_hash(&first["lineage"]));
        assert!(serde_json::to_vec(&first).unwrap().len() < 1024 * 1024);
        assert!(subdivision_topology_lineage_preview(&program, "subd-crease-cage", 441,).is_err());
        assert!(subdivision_topology_lineage_preview(&program, "missing", 25_000).is_err());

        let mut unknown = json!({
            "operation":"subdivision_topology_lineage",
            "payload":{
                "geometry_program":program,
                "subdivision_node_id":"subd-crease-cage",
                "max_lineage_elements":25000,
                "python":"print('forbidden')"
            }
        });
        assert!(worker_result(&unknown).is_err());
        unknown["payload"].as_object_mut().unwrap().remove("python");
        assert!(worker_result(&unknown).is_ok());
    }

    #[test]
    fn subdivision_topology_lineage_maximum_envelope_stays_under_one_mib() {
        let control_points = (0..16)
            .flat_map(|v| {
                (0..16).map(move |u| {
                    json!([u as f64 * 0.1, v as f64 * 0.1, ((u + v) % 3) as f64 * 0.01])
                })
            })
            .collect::<Vec<_>>();
        let mut program = json!({
            "schema_version":"GeometryProgram@2",
            "project_id":"subdivision-lineage-max-envelope",
            "representation_plan_sha256":"b".repeat(64),
            "operator_catalog_sha256":operator_catalog_sha256(),
            "units":{"length":"meter","angle":"radian","coordinate_system":"right-handed-y-up"},
            "budgets":{"max_nodes":4,"max_triangles":7200,"max_glb_bytes":67108864,"max_worker_memory_bytes":536870912,"max_runtime_ms":10000},
            "nodes":[{
                "node_id":"max-cage","operator_id":"forgecad.geometry.subd-cage@2","inputs":[],
                "parameters":{
                    "shape":"subd-cage","control_points":control_points,"u_points":16,"v_points":16,
                    "subdivision_levels":2,"crease_method":"uniform-integer-level-decay@1",
                    "crease_edges":[{"vertex_a":17,"vertex_b":18,"sharpness_levels":2}],
                    "position_m":[0.0,0.0,0.0],"rotation_rad":[0.0,0.0,0.0]
                }
            }],
            "part_outputs":[{"part_id":"max-cage","input_node_ids":["max-cage"],"material_zone_id":"zone-shell","solid":false}]
        });
        program["canonical_sha256"] = Value::String(canonical_hash(&without_hash(&program)));
        let result = subdivision_topology_lineage_preview(&program, "max-cage", 25_000)
            .expect("maximum bounded lineage envelope");
        assert_eq!(result["lineage_element_count"], 22_802);
        assert_eq!(result["lineage"]["evaluated_counts"]["vertex_count"], 3_721);
        assert_eq!(result["lineage"]["evaluated_counts"]["edge_count"], 7_320);
        assert_eq!(result["lineage"]["evaluated_counts"]["quad_count"], 3_600);
        assert_eq!(
            result["lineage"]["evaluated_counts"]["triangle_count"],
            7_200
        );
        let bytes = serde_json::to_vec(&result).expect("maximum lineage JSON");
        assert!(
            bytes.len() < 1024 * 1024,
            "maximum lineage is {} bytes",
            bytes.len()
        );
    }

    fn boolean_program(shape: &str) -> Value {
        let mut program = json!({
            "schema_version":"GeometryProgram@2",
            "project_id":"project-boolean",
            "representation_plan_sha256":"e".repeat(64),
            "operator_catalog_sha256":operator_catalog_sha256(),
            "units":{"length":"meter","angle":"radian","coordinate_system":"right-handed-y-up"},
            "budgets":{
                "max_nodes":8,
                "max_triangles":10000,
                "max_glb_bytes":67108864,
                "max_worker_memory_bytes":536870912,
                "max_runtime_ms":10000
            },
            "nodes":[
                {"node_id":"left","operator_id":"forgecad.geometry.primitive@2","inputs":[],"parameters":{"shape":"box","size_m":[1.0,1.0,1.0],"position_m":[-0.25,0.0,0.0],"rotation_rad":[0.0,0.0,0.0]}},
                {"node_id":"right","operator_id":"forgecad.geometry.primitive@2","inputs":[],"parameters":{"shape":"box","size_m":[1.0,1.0,1.0],"position_m":[0.25,0.0,0.0],"rotation_rad":[0.0,0.0,0.0]}},
                {"node_id":"boolean","operator_id":"forgecad.geometry.boolean@1","inputs":["left","right"],"parameters":{"shape":shape}}
            ],
            "part_outputs":[
                {"part_id":"boolean-part","input_node_ids":["boolean"],"material_zone_id":"zone-black-mechanical","solid":true}
            ]
        });
        let hash = canonical_hash(&program);
        program["canonical_sha256"] = Value::String(hash);
        program
    }

    #[test]
    fn mcp010d_boolean_worker_compiles_real_union_difference_and_intersection() {
        for shape in ["union", "difference", "intersection"] {
            let program = boolean_program(shape);
            let first = compile_geometry_program(&program).expect("Boolean compile");
            let second = compile_geometry_program(&program).expect("Boolean deterministic compile");
            assert_eq!(
                first.glb, second.glb,
                "Boolean {shape} is not deterministic"
            );
            assert!(first.triangle_count > 0);
            assert!(first.triangle_count <= 192);
            let inspection = integrity::inspect_glb(&first.glb).expect("Boolean readback");
            assert!(
                inspection.hard_gate_passed,
                "{:?}",
                inspection.failure_codes
            );
            assert!(inspection.source_node_ids.iter().any(|id| id == "boolean"));
            let (root, _) = glb_root_and_bin_offset(&first.glb);
            assert_eq!(
                root["meshes"][0]["primitives"][0]["extras"]["lineage_source_node_ids"],
                json!(["left", "right"])
            );
        }
    }

    #[test]
    fn boolean_operand_lineage_is_deterministic_bounded_and_explicitly_evaluated() {
        for shape in ["union", "difference", "intersection"] {
            let program = boolean_program(shape);
            let first = boolean_operand_lineage_preview(&program, "boolean", 4096)
                .expect("Boolean lineage preview");
            let second = boolean_operand_lineage_preview(&program, "boolean", 4096)
                .expect("deterministic Boolean lineage preview");
            assert_eq!(first, second);
            assert_eq!(first["operation"], shape);
            assert_eq!(first["lineage_kind"], "evaluated-face-with-operand-run");
            assert_eq!(first["runtime_write_performed"], false);
            assert!(first["output_triangle_count"].as_u64().unwrap() > 0);
            assert!(first["lineage_run_count"].as_u64().unwrap() > 0);
            assert!(first["lineage_runs"].as_array().unwrap().iter().all(|row| {
                matches!(row["operand"].as_str(), Some("left" | "right"))
                    && row["output_triangle_count"].as_u64().unwrap() > 0
            }));
            assert!(first["limitations"]
                .as_array()
                .unwrap()
                .iter()
                .any(|value| value == "EVALUATED_FACE_ID_NOT_ORIGINAL_AUTHORING_FACE_ID"));
        }
        let program = boolean_program("union");
        assert!(boolean_operand_lineage_preview(&program, "left", 4096).is_err());
        assert!(boolean_operand_lineage_preview(&program, "boolean", 0).is_err());
    }

    fn curved_boolean_program(shape: &str) -> Value {
        let mut program = json!({
            "schema_version":"GeometryProgram@2",
            "project_id":"project-curved-boolean",
            "representation_plan_sha256":"f".repeat(64),
            "operator_catalog_sha256":operator_catalog_sha256(),
            "units":{"length":"meter","angle":"radian","coordinate_system":"right-handed-y-up"},
            "budgets":{
                "max_nodes":8,
                "max_triangles":50000,
                "max_glb_bytes":67108864,
                "max_worker_memory_bytes":536870912,
                "max_runtime_ms":10000
            },
            "nodes":[
                {"node_id":"profile-left","operator_id":"forgecad.geometry.profile-extrude@1","inputs":[],"parameters":{"shape":"profile-extrude","profile":[[-0.72,-0.50],[0.60,-0.50],[0.72,0.0],[0.60,0.50],[-0.72,0.50]],"depth_m":1.0,"position_m":[0.0,0.0,0.0],"rotation_rad":[0.0,0.0,0.0]}},
                {"node_id":"profile-right","operator_id":"forgecad.geometry.profile-extrude@1","inputs":[],"parameters":{"shape":"profile-extrude","profile":[[-0.36,-0.36],[0.28,-0.36],[0.42,-0.06],[0.28,0.36],[-0.36,0.36]],"depth_m":0.72,"position_m":[0.35,0.0,0.0],"rotation_rad":[0.0,0.0,0.0]}},
                {"node_id":"boolean","operator_id":"forgecad.geometry.boolean@1","inputs":["profile-left","profile-right"],"parameters":{"shape":shape}}
            ],
            "part_outputs":[
                {"part_id":"curved-boolean-part","input_node_ids":["boolean"],"material_zone_id":"zone-black-mechanical","solid":true}
            ]
        });
        let hash = canonical_hash(&program);
        program["canonical_sha256"] = Value::String(hash);
        program
    }

    #[test]
    fn mcp010d_boolean_worker_accepts_curved_mesh_operands_and_preserves_lineage() {
        for shape in ["union", "difference", "intersection"] {
            let program = curved_boolean_program(shape);
            let first = compile_geometry_program(&program).expect("curved Boolean compile");
            let second =
                compile_geometry_program(&program).expect("curved Boolean deterministic compile");
            assert_eq!(
                first.glb, second.glb,
                "curved Boolean {shape} is not deterministic"
            );
            assert!(first.triangle_count > 0);
            let inspection = integrity::inspect_glb(&first.glb).expect("curved Boolean readback");
            assert!(
                inspection.hard_gate_passed,
                "{shape}: {:?}",
                inspection.failure_codes
            );
            assert!(inspection.source_node_ids.iter().any(|id| id == "boolean"));
            let (root, _) = glb_root_and_bin_offset(&first.glb);
            assert_eq!(
                root["meshes"][0]["primitives"][0]["extras"]["lineage_source_node_ids"],
                json!(["profile-left", "profile-right"])
            );
        }
    }

    fn panel_sphere_boolean_program(shape: &str) -> Value {
        let mut program = json!({
            "schema_version":"GeometryProgram@2",
            "project_id":"project-panel-sphere-boolean",
            "representation_plan_sha256":"a".repeat(64),
            "operator_catalog_sha256":operator_catalog_sha256(),
            "units":{"length":"meter","angle":"radian","coordinate_system":"right-handed-y-up"},
            "budgets":{
                "max_nodes":8,
                "max_triangles":100000,
                "max_glb_bytes":67108864,
                "max_worker_memory_bytes":536870912,
                "max_runtime_ms":10000
            },
            "nodes":[
                {"node_id":"chest-panel","operator_id":"forgecad.geometry.panel@1","inputs":[],"parameters":{"shape":"panel","size_m":[1.66,1.12,0.68],"thickness_m":0.18,"bevel_m":0.12,"position_m":[0.0,1.98,0.04],"rotation_rad":[0.0,0.0,0.0]}},
                {"node_id":"residual-sphere","operator_id":"forgecad.geometry.primitive@2","inputs":[],"parameters":{"shape":"sphere","radius_m":0.13,"longitude_segments":16,"latitude_segments":8,"position_m":[0.0,1.98,0.08],"rotation_rad":[0.0,0.0,0.0]}},
                {"node_id":"residual-sphere-boolean","operator_id":"forgecad.geometry.boolean@1","inputs":["chest-panel","residual-sphere"],"parameters":{"shape":shape}}
            ],
            "part_outputs":[
                {"part_id":"chest-shell","input_node_ids":["residual-sphere-boolean"],"material_zone_id":"zone-white-shell","solid":true}
            ]
        });
        let hash = canonical_hash(&program);
        program["canonical_sha256"] = Value::String(hash);
        program
    }

    #[test]
    fn mcp010f_boolean_residual_keeps_panel_sphere_tangents_valid() {
        let program = panel_sphere_boolean_program("union");
        let artifact = compile_geometry_program(&program).expect("panel-sphere Boolean compile");
        let inspection = integrity::inspect_glb(&artifact.glb).expect("panel-sphere readback");
        assert!(
            inspection.hard_gate_passed,
            "{:?}",
            inspection.failure_codes
        );
        assert_eq!(inspection.tangent_handedness_error_count, 0);
        assert!(inspection
            .source_node_ids
            .iter()
            .any(|id| id == "residual-sphere-boolean"));
    }

    #[test]
    fn mcp010d_boolean_contract_rejects_unsupported_shapes_and_arity() {
        let mut invalid_shape = boolean_program("xor");
        invalid_shape["canonical_sha256"] =
            Value::String(canonical_hash(&without_hash(&invalid_shape)));
        assert!(compile_geometry_program(&invalid_shape).is_err());

        let mut invalid_arity = boolean_program("union");
        invalid_arity["nodes"][2]["inputs"] = json!(["left"]);
        invalid_arity["canonical_sha256"] =
            Value::String(canonical_hash(&without_hash(&invalid_arity)));
        assert!(compile_geometry_program(&invalid_arity).is_err());
    }

    #[test]
    fn mcp010d_dag_and_operator_parameters_fail_closed() {
        let mut cycle = d_operator_program();
        cycle["nodes"][1]["inputs"] = json!(["transformed"]);
        cycle["canonical_sha256"] = Value::String(canonical_hash(&without_hash(&cycle)));
        assert!(compile_geometry_program(&cycle).is_err());

        let mut unknown_parameter = d_operator_program();
        unknown_parameter["nodes"][4]["parameters"]["script"] = json!("nope");
        unknown_parameter["canonical_sha256"] =
            Value::String(canonical_hash(&without_hash(&unknown_parameter)));
        assert!(compile_geometry_program(&unknown_parameter).is_err());

        let mut boolean = d_operator_program();
        boolean["nodes"][0]["operator_id"] =
            Value::String("forgecad.geometry.boolean@1".to_owned());
        boolean["nodes"][0]["parameters"] = json!({"shape":"difference"});
        boolean["canonical_sha256"] = Value::String(canonical_hash(&without_hash(&boolean)));
        assert!(compile_geometry_program(&boolean).is_err());
    }

    #[test]
    fn mcp010e_offline_pack_uv_atlas_and_pbr_textures_are_embedded() {
        let geometry = d_operator_program();
        let geometry_hash = geometry["canonical_sha256"].as_str().unwrap().to_owned();
        let mut appearance = json!({
            "schema_version":"AppearanceProgram@2",
            "project_id":"project-d",
            "geometry_program_sha256":geometry_hash,
            "material_pack_id":"forgecad-hard-surface-robot",
            "material_pack_manifest_sha256":material_pack_manifest_sha256(),
            "material_zones":[
                {"zone_id":"zone-white-shell","part_ids":["arrayed-part","extrude-part","loft-part","longitudinal-loft-part"],"material_id":"white-dielectric-clearcoat","texture_set_id":"plastic-surface"},
                {"zone_id":"zone-black-mechanical","part_ids":["panel-vent","joint-part","revolve-part"],"material_id":"dark-painted-metal","texture_set_id":"metal-surface"},
                {"zone_id":"zone-emissive-amber","part_ids":["sweep-part"],"material_id":"warm-orange-emissive","texture_set_id":null}
            ],
            "canonical_sha256":""
        });
        appearance["canonical_sha256"] = Value::String(canonical_hash(&without_hash(&appearance)));
        let artifact = compile_geometry_program_with_appearance(&geometry, Some(&appearance))
            .expect("MCP010E appearance compile");
        let inspection = integrity::inspect_glb(&artifact.glb).expect("MCP010E readback");
        assert!(
            inspection.hard_gate_passed,
            "{:?}",
            inspection.failure_codes
        );
        let (root, _) = glb_root_and_bin_offset(&artifact.glb);
        assert!(root
            .get("images")
            .and_then(Value::as_array)
            .is_some_and(|v| v.len() >= 5));
        assert!(root
            .get("textures")
            .and_then(Value::as_array)
            .is_some_and(|v| v.len() >= 5));
        assert_eq!(root["extras"]["forgecad"]["uv_atlas"]["resolution"], 512);
        assert!(
            root["extras"]["forgecad"]["texture_count"]
                .as_u64()
                .unwrap()
                >= 5
        );
        let image_names = root["images"]
            .as_array()
            .expect("embedded material images")
            .iter()
            .filter_map(|image| image.get("name").and_then(Value::as_str))
            .collect::<Vec<_>>();
        assert!(image_names.contains(&"metal010_metallic_roughness"));
        assert!(image_names.contains(&"plastic006_metallic_roughness"));
        assert!(!image_names.contains(&"metal010_roughness"));
        assert!(!image_names.contains(&"metal010_metalness"));
        let metal_material = root["materials"]
            .as_array()
            .expect("materials")
            .iter()
            .find(|material| material["name"] == "zone-black-mechanical")
            .expect("metal material");
        assert!(metal_material["pbrMetallicRoughness"]
            .get("metallicRoughnessTexture")
            .is_some());

        let mut invalid = appearance;
        invalid["material_pack_manifest_sha256"] = Value::String("0".repeat(64));
        invalid["canonical_sha256"] = Value::String(canonical_hash(&without_hash(&invalid)));
        assert!(compile_geometry_program_with_appearance(&geometry, Some(&invalid)).is_err());
    }

    #[test]
    fn fictional_energy_weapon_pack_lowers_deterministically_and_fails_closed() {
        let geometry = d_operator_program();
        let geometry_hash = geometry["canonical_sha256"].as_str().unwrap().to_owned();
        let manifest_sha256 =
            material_pack_manifest_sha256_by_id("forgecad-fictional-energy-weapon")
                .expect("weapon MaterialPack manifest");
        let mut appearance = json!({
            "schema_version":"AppearanceProgram@2",
            "project_id":"project-d",
            "geometry_program_sha256":geometry_hash,
            "material_pack_id":"forgecad-fictional-energy-weapon",
            "material_pack_manifest_sha256":manifest_sha256,
            "material_zones":[
                {"zone_id":"zone-white-shell","part_ids":["arrayed-part","extrude-part","loft-part","longitudinal-loft-part"],"material_id":"energy-white-clearcoat","texture_set_id":"weapon-plastic-surface"},
                {"zone_id":"zone-black-mechanical","part_ids":["panel-vent","joint-part","revolve-part"],"material_id":"energy-brushed-gold","texture_set_id":"weapon-metal-surface"},
                {"zone_id":"zone-emissive-amber","part_ids":["sweep-part"],"material_id":"energy-cyan-emissive","texture_set_id":null}
            ],
            "canonical_sha256":""
        });
        appearance["canonical_sha256"] = Value::String(canonical_hash(&without_hash(&appearance)));
        let first = compile_geometry_program_with_appearance(&geometry, Some(&appearance))
            .expect("weapon MaterialPack compile");
        let repeat = compile_geometry_program_with_appearance(&geometry, Some(&appearance))
            .expect("weapon MaterialPack deterministic repeat");
        assert_eq!(first.glb, repeat.glb);
        let inspection = integrity::inspect_glb(&first.glb).expect("weapon MaterialPack readback");
        assert!(
            inspection.hard_gate_passed,
            "{:?}",
            inspection.failure_codes
        );
        assert_eq!(inspection.external_uri_count, 0);
        let (root, _) = glb_root_and_bin_offset(&first.glb);
        assert_eq!(
            root["extras"]["forgecad"]["material_pack_id"],
            "forgecad-fictional-energy-weapon"
        );
        assert_eq!(
            root["extras"]["forgecad"]["material_pack_manifest_sha256"],
            material_pack_manifest_sha256_by_id("forgecad-fictional-energy-weapon")
                .expect("weapon manifest hash")
        );
        assert_eq!(root["extras"]["forgecad"]["uv_atlas"]["resolution"], 2048);
        assert_eq!(root["extras"]["forgecad"]["uv_atlas"]["padding_texels"], 8);
        assert_eq!(
            root["extras"]["forgecad"]["uv_atlas"]["packing"],
            "connected-dominant-axis-islands@1"
        );
        assert!(root["extras"]["forgecad"]["uv_atlas"]["charts"]
            .as_u64()
            .is_some_and(|charts| charts > 0 && charts < first.triangle_count));
        assert!(root["meshes"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|mesh| mesh["primitives"].as_array().unwrap())
            .all(|primitive| primitive["extras"]["uv_chart_ids"].is_array()
                && primitive["extras"]["uv_chart_assignment_sha256"].is_string()));
        assert!(root["extras"]["forgecad"]["texture_count"]
            .as_u64()
            .is_some_and(|count| count >= 5));
        assert!(root["materials"]
            .as_array()
            .unwrap()
            .iter()
            .any(
                |material| material["extras"]["forgecad"]["material_id"] == "energy-cyan-emissive"
            ));

        let (_, bin_offset) = glb_root_and_bin_offset(&first.glb);
        let primitive = &root["meshes"][0]["primitives"][0];
        let uv_accessor = primitive["attributes"]["TEXCOORD_0"]
            .as_u64()
            .expect("weapon UV accessor") as usize;
        let uv_offset = bin_offset + accessor_byte_offset(&root, uv_accessor);
        let mut tampered_uv = first.glb.clone();
        tampered_uv[uv_offset..uv_offset + 8].copy_from_slice(&[0; 8]);
        assert!(integrity::inspect_glb(&tampered_uv).is_err());

        for (field, value) in [
            ("path", "/tmp/weapon-pack"),
            ("url", "https://example.invalid/pack"),
            ("plugin", "arbitrary-material-loader"),
        ] {
            let mut forbidden = appearance.clone();
            forbidden[field] = Value::String(value.to_owned());
            forbidden["canonical_sha256"] =
                Value::String(canonical_hash(&without_hash(&forbidden)));
            assert!(compile_geometry_program_with_appearance(&geometry, Some(&forbidden)).is_err());
        }
        let mut unknown_pack = appearance;
        unknown_pack["material_pack_id"] = Value::String("unknown-pack".to_owned());
        unknown_pack["canonical_sha256"] =
            Value::String(canonical_hash(&without_hash(&unknown_pack)));
        assert!(compile_geometry_program_with_appearance(&geometry, Some(&unknown_pack)).is_err());

        let mut wrong_part = json!({
            "schema_version":"AppearanceProgram@2",
            "project_id":"project-d",
            "geometry_program_sha256":geometry["canonical_sha256"],
            "material_pack_id":"forgecad-fictional-energy-weapon",
            "material_pack_manifest_sha256":material_pack_manifest_sha256_by_id("forgecad-fictional-energy-weapon").unwrap(),
            "material_zones":[
                {"zone_id":"zone-white-shell","part_ids":["not-a-real-part"],"material_id":"energy-white-clearcoat","texture_set_id":"weapon-plastic-surface"},
                {"zone_id":"zone-black-mechanical","part_ids":["panel-vent","joint-part","revolve-part"],"material_id":"energy-brushed-gold","texture_set_id":"weapon-metal-surface"},
                {"zone_id":"zone-emissive-amber","part_ids":["sweep-part"],"material_id":"energy-cyan-emissive","texture_set_id":null}
            ],
            "canonical_sha256":""
        });
        wrong_part["canonical_sha256"] = Value::String(canonical_hash(&without_hash(&wrong_part)));
        let error = compile_geometry_program_with_appearance(&geometry, Some(&wrong_part))
            .expect_err("appearance Part bindings must match geometry exactly");
        assert!(error
            .to_string()
            .contains("part_ids do not exactly match GeometryProgram@2 part outputs"));
    }

    #[test]
    fn fictional_energy_weapon_continuous_uv_accepts_the_raw_stdio_primitive_fixture() {
        let mut geometry = v2_program();
        geometry["nodes"] = json!([
            {"node_id":"shell","operator_id":"forgecad.geometry.primitive@2","inputs":[],"parameters":{"shape":"box","size_m":[1.2,1.6,0.55],"position_m":[0.0,1.7,0.0],"rotation_rad":[0.0,0.0,0.0]}},
            {"node_id":"joint","operator_id":"forgecad.geometry.primitive@2","inputs":[],"parameters":{"shape":"cylinder","radius_m":0.3,"height_m":0.8,"radial_segments":16,"position_m":[0.0,0.55,0.0],"rotation_rad":[0.0,0.0,0.0]}},
            {"node_id":"sensor","operator_id":"forgecad.geometry.primitive@2","inputs":[],"parameters":{"shape":"ellipsoid","radii_m":[0.25,0.35,0.2],"longitude_segments":16,"latitude_segments":8,"position_m":[0.0,2.65,0.0],"rotation_rad":[0.0,0.0,0.0]}},
            {"node_id":"accent","operator_id":"forgecad.geometry.primitive@2","inputs":[],"parameters":{"shape":"sphere","radius_m":0.12,"longitude_segments":16,"latitude_segments":8,"position_m":[0.0,1.7,-0.36],"rotation_rad":[0.0,0.0,0.0]}}
        ]);
        geometry["part_outputs"] = json!([
            {"part_id":"shell","input_node_ids":["shell","accent"],"material_zone_id":"zone-white-shell","solid":true},
            {"part_id":"joint","input_node_ids":["joint"],"material_zone_id":"zone-black-mechanical","solid":true},
            {"part_id":"sensor","input_node_ids":["sensor"],"material_zone_id":"zone-emissive-amber","solid":true}
        ]);
        geometry["budgets"]["max_glb_bytes"] = Value::from(64 * 1024 * 1024u64);
        geometry["canonical_sha256"] = Value::String(canonical_hash(&without_hash(&geometry)));
        let geometry_hash = geometry["canonical_sha256"].as_str().unwrap().to_owned();
        let mut appearance = json!({
            "schema_version":"AppearanceProgram@2",
            "project_id":"project-test",
            "geometry_program_sha256":geometry_hash,
            "material_pack_id":"forgecad-fictional-energy-weapon",
            "material_pack_manifest_sha256":material_pack_manifest_sha256_by_id("forgecad-fictional-energy-weapon").unwrap(),
            "material_zones":[
                {"zone_id":"zone-white-shell","part_ids":["shell"],"material_id":"energy-white-clearcoat","texture_set_id":"weapon-plastic-surface"},
                {"zone_id":"zone-black-mechanical","part_ids":["joint"],"material_id":"energy-dark-painted-metal","texture_set_id":"weapon-metal-surface"},
                {"zone_id":"zone-emissive-amber","part_ids":["sensor"],"material_id":"energy-cyan-emissive","texture_set_id":null}
            ],
            "canonical_sha256":""
        });
        appearance["canonical_sha256"] = Value::String(canonical_hash(&without_hash(&appearance)));
        let artifact = compile_geometry_program_with_appearance(&geometry, Some(&appearance))
            .expect("raw stdio primitive fixture continuous UV");
        let inspection = integrity::inspect_glb(&artifact.glb)
            .expect("raw stdio primitive fixture strict continuous UV readback");
        assert!(
            inspection.hard_gate_passed,
            "{:?}",
            inspection.failure_codes
        );
    }

    #[test]
    fn fictional_energy_weapon_texture_bytes_match_manifest_hashes_and_dimensions() {
        let manifest = material_pack_manifest_by_id("forgecad-fictional-energy-weapon")
            .expect("weapon MaterialPack manifest");
        for texture in manifest["textures"].as_array().expect("texture inventory") {
            let texture_id = texture["texture_id"].as_str().expect("texture id");
            let bytes = pack_texture_bytes("forgecad-fictional-energy-weapon", texture_id)
                .expect("compiled-in texture bytes");
            let actual_sha256 = Sha256::digest(&bytes)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>();
            assert_eq!(actual_sha256, texture["sha256"]);
            let decoded = image::load_from_memory(&bytes).expect("admitted PNG");
            assert_eq!(decoded.width() as u64, texture["width"]);
            assert_eq!(decoded.height() as u64, texture["height"]);
        }
    }

    #[test]
    fn fictional_energy_weapon_2k_outputs_are_manifest_bound_and_not_plain_resizes() {
        let manifest = material_pack_manifest_by_id(FICTIONAL_ENERGY_WEAPON_2K_PACK_ID)
            .expect("weapon 2K MaterialPack manifest");
        let direct_outputs = manifest["textures"].as_array().expect("direct outputs");
        let derived_outputs = manifest["derived_outputs"]
            .as_array()
            .expect("derived outputs");
        for key in [
            "metal010_color",
            "metal010_normal_gl",
            "metal010_roughness",
            "metal010_metalness",
            "plastic006_color",
            "plastic006_normal_gl",
            "plastic006_roughness",
            "metal010_metallic_roughness",
            "plastic006_metallic_roughness",
        ] {
            let bytes = fictional_energy_weapon_2k_texture_bytes(key).expect("2K output");
            let decoded = image::load_from_memory(&bytes).expect("2K output PNG");
            let expected = direct_outputs
                .iter()
                .chain(derived_outputs)
                .find(|value| value["texture_id"] == key)
                .expect("manifest output");
            assert_eq!(sha256_hex(&bytes), expected["sha256"]);
            assert_eq!(decoded.width(), 2048);
            assert_eq!(decoded.height(), 2048);
        }

        let output = image::load_from_memory(
            &fictional_energy_weapon_2k_texture_bytes("metal010_color").unwrap(),
        )
        .unwrap()
        .to_rgb8();
        let source =
            image::load_from_memory(weapon_source_texture_bytes("metal010_color").unwrap())
                .unwrap()
                .to_rgb8();
        for filter in [
            imageops::FilterType::Nearest,
            imageops::FilterType::Triangle,
            imageops::FilterType::CatmullRom,
            imageops::FilterType::Gaussian,
            imageops::FilterType::Lanczos3,
        ] {
            let plain = imageops::resize(&source, 2048, 2048, filter);
            assert_ne!(
                output.as_raw(),
                plain.as_raw(),
                "2K output must contain bounded semantic microdetail beyond a plain resize"
            );
        }
    }

    fn layered_weapon_appearance_v3(geometry_hash: &str) -> Value {
        let manifest_sha256 =
            material_pack_manifest_sha256_by_id(FICTIONAL_ENERGY_WEAPON_2K_PACK_ID).unwrap();
        let mut stack = json!({
            "schema_version":"MaterialLayerStack@1",
            "stack_id":"fictional-energy-weapon-surface-v1",
            "material_pack_id":FICTIONAL_ENERGY_WEAPON_2K_PACK_ID,
            "material_pack_manifest_sha256":manifest_sha256,
            "uv_source":"TEXCOORD_0",
            "layers":[
                {"layer_id":"fictional-safety-markings","order":0,"kind":"decal","recipe_id":"forgecad-first-party-fictional-safety-markings@1","blend_policy":"precompose-baseColor-no-custom-shader","targets":{"part_ids":["shell"],"material_zone_ids":["zone-white-shell"]},"opacity":0.65},
                {"layer_id":"bounded-edge-wear","order":1,"kind":"wear","recipe_id":"forgecad-first-party-geometry-edge-ao-wear@1","blend_policy":"precompose-baseColor-metallicRoughness-no-custom-shader","targets":{"part_ids":["shell","joint","core"],"material_zone_ids":["zone-white-shell","zone-black-mechanical"]},"edge_width_texels":8,"strength":0.35},
                {"layer_id":"texture-backed-clearcoat","order":2,"kind":"clearcoat","recipe_id":"forgecad-first-party-zone-clearcoat-mask@1","blend_policy":"KHR_materials_clearcoat","targets":{"part_ids":["shell"],"material_zone_ids":["zone-white-shell"]},"factor":0.82,"roughness":0.12}
            ],
            "budget":{"resolution":2048,"padding_texels":8,"max_output_textures":8,"max_output_bytes":67108864,"max_runtime_ms":120000},
            "canonical_sha256":""
        });
        stack["canonical_sha256"] = Value::String(canonical_hash(&without_hash(&stack)));
        let mut appearance = json!({
            "schema_version":"AppearanceProgram@3",
            "project_id":"project-test",
            "geometry_program_sha256":geometry_hash,
            "material_pack_id":FICTIONAL_ENERGY_WEAPON_2K_PACK_ID,
            "material_pack_manifest_sha256":manifest_sha256,
            "material_zones":[
                {"zone_id":"zone-white-shell","part_ids":["shell"],"material_id":"energy-white-clearcoat","texture_set_id":"weapon-plastic-surface"},
                {"zone_id":"zone-black-mechanical","part_ids":["joint","core"],"material_id":"energy-dark-painted-metal","texture_set_id":"weapon-metal-surface"},
                {"zone_id":"zone-emissive-amber","part_ids":["sensor"],"material_id":"energy-cyan-emissive","texture_set_id":null}
            ],
            "material_layer_stack_sha256":stack["canonical_sha256"],
            "material_layer_stack":stack,
            "canonical_sha256":""
        });
        appearance["canonical_sha256"] = Value::String(canonical_hash(&without_hash(&appearance)));
        appearance
    }

    #[test]
    fn material_layer_stack_v1_is_hash_bound_to_actual_parts_and_zones() {
        let mut geometry = v2_program();
        geometry["budgets"]["max_glb_bytes"] = Value::from(64 * 1024 * 1024u64);
        geometry["canonical_sha256"] = Value::String(canonical_hash(&without_hash(&geometry)));
        let geometry_hash = geometry["canonical_sha256"].as_str().unwrap();
        let appearance = layered_weapon_appearance_v3(geometry_hash);
        let first = compile_geometry_program_with_appearance(&geometry, Some(&appearance))
            .expect("typed layer stack transport");
        let second = compile_geometry_program_with_appearance(&geometry, Some(&appearance))
            .expect("deterministic typed layer stack transport");
        assert_eq!(first.glb, second.glb);
        let inspection = integrity::inspect_glb(&first.glb).expect("layered strict readback");
        assert!(
            inspection.hard_gate_passed,
            "{:?}",
            inspection.failure_codes
        );
        let (root, bin_offset) = glb_root_and_bin_offset(&first.glb);
        assert!(root["materials"]
            .as_array()
            .unwrap()
            .iter()
            .all(|material| {
                material["extras"]["forgecad"]["material_layer_stack_sha256"]
                    == appearance["material_layer_stack_sha256"]
            }));
        let surface_bake = &root["extras"]["forgecad"]["surface_bake"];
        assert_eq!(surface_bake["schema_version"], "CandidateSurfaceBake@1");
        assert_eq!(surface_bake["outputs"].as_array().unwrap().len(), 6);
        assert_eq!(surface_bake["resolution"], 2048);
        assert_eq!(surface_bake["padding_texels"], 8);
        let clearcoat_material = root["materials"]
            .as_array()
            .unwrap()
            .iter()
            .find(|material| material["name"] == "zone-white-shell")
            .expect("clearcoat zone material");
        assert!(
            clearcoat_material["extensions"]["KHR_materials_clearcoat"]["clearcoatTexture"]
                ["index"]
                .as_u64()
                .is_some()
        );
        assert!(clearcoat_material["extensions"]["KHR_materials_clearcoat"]
            ["clearcoatRoughnessTexture"]["index"]
            .as_u64()
            .is_some());
        let surface_image = root["images"]
            .as_array()
            .unwrap()
            .iter()
            .find(|image| image["name"] == surface_bake::NORMAL_ID)
            .expect("embedded candidate normal image");
        let surface_view = surface_image["bufferView"].as_u64().unwrap() as usize;
        let surface_offset = root["bufferViews"][surface_view]["byteOffset"]
            .as_u64()
            .unwrap() as usize;
        let mut tampered_surface = first.glb.clone();
        tampered_surface[bin_offset + surface_offset + 33] ^= 1;
        assert!(integrity::inspect_glb(&tampered_surface).is_err());

        let mut unknown_target = appearance.clone();
        unknown_target["material_layer_stack"]["layers"][0]["targets"]["part_ids"] =
            json!(["not-a-part"]);
        unknown_target["material_layer_stack"]["canonical_sha256"] = Value::String(canonical_hash(
            &without_hash(&unknown_target["material_layer_stack"]),
        ));
        unknown_target["material_layer_stack_sha256"] =
            unknown_target["material_layer_stack"]["canonical_sha256"].clone();
        unknown_target["canonical_sha256"] =
            Value::String(canonical_hash(&without_hash(&unknown_target)));
        assert!(
            compile_geometry_program_with_appearance(&geometry, Some(&unknown_target)).is_err()
        );

        let mut executable = appearance;
        executable["material_layer_stack"]["layers"][0]["shader"] = json!("arbitrary");
        executable["material_layer_stack"]["canonical_sha256"] = Value::String(canonical_hash(
            &without_hash(&executable["material_layer_stack"]),
        ));
        executable["material_layer_stack_sha256"] =
            executable["material_layer_stack"]["canonical_sha256"].clone();
        executable["canonical_sha256"] = Value::String(canonical_hash(&without_hash(&executable)));
        assert!(compile_geometry_program_with_appearance(&geometry, Some(&executable)).is_err());
    }

    #[test]
    fn fictional_energy_weapon_2k_glb_physically_reads_actual_embedded_pngs() {
        let mut geometry = v2_program();
        geometry["budgets"]["max_glb_bytes"] = Value::from(64 * 1024 * 1024u64);
        geometry["budgets"]["max_runtime_ms"] = Value::from(10_000u64);
        geometry["canonical_sha256"] = Value::String(canonical_hash(&without_hash(&geometry)));
        let geometry_hash = geometry["canonical_sha256"].as_str().unwrap().to_owned();
        let mut appearance = json!({
            "schema_version":"AppearanceProgram@2",
            "project_id":"project-test",
            "geometry_program_sha256":geometry_hash,
            "material_pack_id":FICTIONAL_ENERGY_WEAPON_2K_PACK_ID,
            "material_pack_manifest_sha256":material_pack_manifest_sha256_by_id(FICTIONAL_ENERGY_WEAPON_2K_PACK_ID).unwrap(),
            "material_zones":[
                {"zone_id":"zone-white-shell","part_ids":["shell"],"material_id":"energy-white-clearcoat","texture_set_id":"weapon-plastic-surface"},
                {"zone_id":"zone-black-mechanical","part_ids":["joint","core"],"material_id":"energy-dark-painted-metal","texture_set_id":"weapon-metal-surface"},
                {"zone_id":"zone-emissive-amber","part_ids":["sensor"],"material_id":"energy-cyan-emissive","texture_set_id":null}
            ],
            "canonical_sha256":""
        });
        appearance["canonical_sha256"] = Value::String(canonical_hash(&without_hash(&appearance)));
        let artifact = compile_geometry_program_with_appearance(&geometry, Some(&appearance))
            .expect("2K weapon artifact");
        let inspection = integrity::inspect_glb(&artifact.glb).expect("strict 2K GLB readback");
        assert!(
            inspection.hard_gate_passed,
            "{:?}",
            inspection.failure_codes
        );
        assert!(artifact.glb.len() < 64 * 1024 * 1024);
        let (root, bin_offset) = glb_root_and_bin_offset(&artifact.glb);
        let build = &root["extras"]["forgecad"]["texture_build"];
        assert_eq!(build["resolution"], 2048);
        assert_eq!(build["outputs"].as_array().unwrap().len(), 5);
        assert!(build["outputs"].as_array().unwrap().iter().all(|output| {
            output["width"] == 2048
                && output["height"] == 2048
                && output["sha256"]
                    .as_str()
                    .is_some_and(|value| value.len() == 64)
        }));

        let first_image = &root["images"][0];
        let view = first_image["bufferView"].as_u64().unwrap() as usize;
        let offset = root["bufferViews"][view]["byteOffset"].as_u64().unwrap() as usize;
        let mut tampered = artifact.glb.clone();
        tampered[bin_offset + offset + 33] ^= 1;
        assert!(integrity::inspect_glb(&tampered).is_err());
    }

    #[test]
    fn mcp010e_metallic_roughness_texture_uses_g_for_roughness_and_b_for_metallic() {
        let roughness = image::load_from_memory(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../packages/forgecad-assets/forgecad-hard-surface-robot/1.0.0/textures/metal010_roughness.png"
        )))
        .expect("metal roughness source")
        .to_luma8();
        let metallic = image::load_from_memory(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../packages/forgecad-assets/forgecad-hard-surface-robot/1.0.0/textures/metal010_metalness.png"
        )))
        .expect("metallic source")
        .to_luma8();
        let packed = image::load_from_memory(
            &pack_texture_bytes("forgecad-hard-surface-robot", "metal010_metallic_roughness")
                .expect("packed MR texture"),
        )
        .expect("packed MR PNG")
        .to_rgb8();
        assert_eq!(packed.dimensions(), roughness.dimensions());
        assert_eq!(packed.dimensions(), metallic.dimensions());
        for y in (0..packed.height()).step_by(31) {
            for x in (0..packed.width()).step_by(29) {
                let source_roughness = roughness.get_pixel(x, y)[0];
                let source_metallic = metallic.get_pixel(x, y)[0];
                let pixel = packed.get_pixel(x, y);
                assert_eq!(pixel[0], 255, "unused R channel must be deterministic");
                assert_eq!(pixel[1], source_roughness, "roughness must be glTF G");
                assert_eq!(pixel[2], source_metallic, "metallic must be glTF B");
            }
        }

        let plastic = image::load_from_memory(
            &pack_texture_bytes(
                "forgecad-hard-surface-robot",
                "plastic006_metallic_roughness",
            )
            .expect("plastic packed MR"),
        )
        .expect("plastic packed MR PNG")
        .to_rgb8();
        assert!(plastic.pixels().all(|pixel| pixel[2] == 0));
    }

    #[test]
    fn mcp010e_white_shell_keeps_factor() {
        let manifest = material_pack_manifest();
        let definition = manifest["material_definitions"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["material_id"] == "white-dielectric-clearcoat")
            .unwrap();
        let white = pack_material_json(
            definition,
            Some("plastic-surface"),
            "forgecad-hard-surface-robot",
            &material_pack_manifest_sha256(),
        );
        assert!(white["pbrMetallicRoughness"]
            .get("baseColorTexture")
            .is_none());
        assert_eq!(
            white["pbrMetallicRoughness"]["baseColorFactor"],
            json!([0.82, 0.86, 0.9, 1.0])
        );
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
    fn operator_catalog_is_closed_and_hash_bound() {
        let catalog = operator_catalog();
        let declared = catalog["canonical_sha256"]
            .as_str()
            .expect("catalog hash")
            .to_owned();
        let mut without_hash = catalog.as_object().expect("catalog object").clone();
        without_hash.remove("canonical_sha256");
        assert_eq!(declared, canonical_hash(&Value::Object(without_hash)));
        assert_eq!(
            catalog["operators"].as_array().expect("operators").len(),
            28
        );
        assert_eq!(
            catalog["operators"][0]["operator_id"],
            "forgecad.geometry.primitive@2"
        );
        assert!(catalog["operators"]
            .as_array()
            .expect("operators")
            .iter()
            .any(|entry| entry["operator_id"] == "forgecad.geometry.profile-loft@2"));
        assert!(catalog["operators"]
            .as_array()
            .expect("operators")
            .iter()
            .any(|entry| {
                entry["operator_id"] == "forgecad.geometry.multi-loop-profile-loft@1"
            }));
        assert!(catalog["operators"]
            .as_array()
            .expect("operators")
            .iter()
            .any(|entry| entry["operator_id"] == "forgecad.geometry.bevel@1"));
        assert!(catalog["operators"]
            .as_array()
            .expect("operators")
            .iter()
            .any(|entry| entry["operator_id"] == "forgecad.geometry.bevel@2"));
        assert!(catalog["operators"]
            .as_array()
            .expect("operators")
            .iter()
            .any(|entry| entry["operator_id"] == "forgecad.geometry.normal-policy@1"));
        assert!(catalog["operators"]
            .as_array()
            .expect("operators")
            .iter()
            .any(|entry| entry["operator_id"] == "forgecad.geometry.vent-array@2"));
        assert!(catalog["operators"]
            .as_array()
            .expect("operators")
            .iter()
            .any(|entry| entry["operator_id"] == "forgecad.geometry.recessed-channel@1"));
        assert!(catalog["operators"]
            .as_array()
            .expect("operators")
            .iter()
            .any(|entry| entry["operator_id"] == "forgecad.geometry.energy-core@1"));
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

    #[test]
    fn geometric_bake_2k_derives_three_hash_bound_maps_and_replays_byte_exact() {
        fn artifact_variant(project_id: &str) -> GeometryArtifact {
            let mut program = v2_program();
            program["project_id"] = Value::String(project_id.to_owned());
            program["canonical_sha256"] = Value::String(canonical_hash(&without_hash(&program)));
            compile_geometry_program(&program).expect("geometric bake fixture artifact")
        }

        let high = artifact_variant("geometric-bake-high");
        let low = artifact_variant("geometric-bake-low");
        let cage = artifact_variant("geometric-bake-cage");
        let high_hash = sha256_hex(&high.glb);
        let low_hash = sha256_hex(&low.glb);
        let cage_hash = sha256_hex(&cage.glb);
        let mut request = json!({
            "operation":forgecad_worker_protocol::PRODUCTION_WEAPON_GEOMETRIC_BAKE_OPERATION,
            "payload":{
                "schema_version":forgecad_worker_protocol::PRODUCTION_WEAPON_GEOMETRIC_BAKE_REQUEST_SCHEMA_VERSION,
                "bake_policy":forgecad_worker_protocol::PRODUCTION_WEAPON_GEOMETRIC_BAKE_POLICY,
                "bake_policy_sha256":sha256_hex(forgecad_worker_protocol::PRODUCTION_WEAPON_GEOMETRIC_BAKE_POLICY.as_bytes()),
                "budget_profile":forgecad_worker_protocol::PRODUCTION_WEAPON_GEOMETRIC_BAKE_BUDGET_PROFILE,
                "atlas_policy":forgecad_worker_protocol::PRODUCTION_WEAPON_GEOMETRIC_BAKE_ATLAS_POLICY,
                "high_glb_base64":base64::engine::general_purpose::STANDARD.encode(&high.glb),
                "low_glb_base64":base64::engine::general_purpose::STANDARD.encode(&low.glb),
                "cage_glb_base64":base64::engine::general_purpose::STANDARD.encode(&cage.glb),
                "high_artifact_sha256":high_hash,
                "low_artifact_sha256":low_hash,
                "cage_artifact_sha256":cage_hash,
                "resolution":2048,
                "normal_convention":"OpenGL+Y",
                "max_ray_distance_m":10.0,
                "ao_sample_count":8,
                "surface_bake_reuse_allowed":false,
                "canonical_sha256":""
            }
        });
        let request_hash = canonical_hash(&without_hash(&request["payload"]));
        request["payload"]["canonical_sha256"] = Value::String(request_hash);
        let first = worker_result(&request).expect("geometric bake result");
        let second = worker_result(&request).expect("geometric bake replay");
        assert_eq!(first, second);
        assert_eq!(
            first["schema_version"],
            "ProductionWeaponGeometricBakeResult@1"
        );
        assert_eq!(first["resolution"], 2048);
        assert_eq!(first["normal_convention"], "OpenGL+Y");
        assert!(first["coverage"]["covered_pixels"].as_u64().unwrap() > 0);
        assert!(first["diagnostic"]["ray_sample_count"].as_u64().unwrap() > 0);
        for (field, hash_field) in [
            ("normal_png_base64", "normal_png_sha256"),
            ("ao_png_base64", "ao_png_sha256"),
            ("curvature_png_base64", "curvature_png_sha256"),
        ] {
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(first[field].as_str().unwrap())
                .expect("bake PNG base64");
            assert_eq!(sha256_hex(&bytes), first[hash_field].as_str().unwrap());
            let decoded = image::load_from_memory(&bytes).expect("bake PNG decode");
            assert_eq!(decoded.width(), 2048);
            assert_eq!(decoded.height(), 2048);
        }
        let mut drifted = request.clone();
        drifted["payload"]["surface_bake"] = json!(true);
        assert!(worker_result(&drifted).is_err());
    }

    fn without_hash(value: &Value) -> Value {
        let mut object = value.as_object().unwrap().clone();
        object.remove("canonical_sha256");
        Value::Object(object)
    }
}
