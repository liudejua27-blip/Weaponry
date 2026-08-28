//! Strict, product-owned GLB readback for ForgeCAD geometry artifacts.
//!
//! This module intentionally derives every mesh conclusion from the GLB JSON
//! and BIN chunks. `extras` may bind the decoded mesh to a typed program, but
//! never substitutes for decoded positions, indices, UVs, tangents, or
//! topology.

use crate::GeometryError;
use forgecad_core::canonical_json_hash;
use forgecad_worker_protocol::material_pack_manifest_by_id;
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

const MAX_GLB_BYTES: usize = 64 * 1024 * 1024;
const POSITION_WELD_SCALE: f64 = 1_000_000.0;
const DEGENERATE_AREA_EPSILON: f32 = 1.0e-10;
const UV_AREA_EPSILON: f32 = 1.0e-8;
const ORTHOGONALITY_EPSILON: f32 = 1.0e-3;

/// This identifier is included in every V2 receipt so that a later validator
/// change cannot silently reinterpret an older result.
pub const READBACK_CONFIG: &str =
    "forgecad-strict-glb-readback@2:glb-bin-accessor-topology-uv-tangent-handedness-scene-graph-closed-static-profile";

type BindingKey = (String, String, String, bool);

#[derive(Debug, Clone, Copy)]
struct ContinuousUvAtlas {
    chart_count: usize,
    columns: usize,
    rows: usize,
    padding_uv: f32,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PartBinding {
    pub part_id: String,
    pub source_node_id: String,
    pub material_zone_id: String,
    pub solid: bool,
    pub triangle_count: u64,
}

#[derive(Debug, Clone)]
pub struct GlbIntegrity {
    pub artifact_schema_version: String,
    pub program_sha256: String,
    pub operator_catalog_sha256: Option<String>,
    pub readback_config_sha256: String,
    pub part_ids: Vec<String>,
    pub source_node_ids: Vec<String>,
    pub material_zone_ids: Vec<String>,
    pub part_bindings: Vec<PartBinding>,
    pub triangle_count: u64,
    pub connected_component_count: u64,
    pub invalid_index_count: u64,
    pub non_finite_count: u64,
    pub degenerate_triangle_count: u64,
    pub boundary_edge_count: u64,
    pub non_manifold_edge_count: u64,
    pub winding_error_count: u64,
    pub uv_non_finite_count: u64,
    pub zero_area_uv_triangle_count: u64,
    pub tangent_non_finite_count: u64,
    pub tangent_orthogonality_error_count: u64,
    pub tangent_handedness_error_count: u64,
    pub external_uri_count: u64,
    pub metadata_mismatch_count: u64,
    pub part_coverage: f64,
    pub source_coverage: f64,
    pub material_zone_coverage: f64,
    pub glb_parse_status: String,
    pub validator_status: String,
    pub hard_gate_passed: bool,
    pub failure_codes: Vec<String>,
    pub aspect_ratio: f64,
}

impl GlbIntegrity {
    pub fn report_value(&self) -> Value {
        json!({
            "invalid_index_count":self.invalid_index_count,
            "non_finite_count":self.non_finite_count,
            "degenerate_triangle_count":self.degenerate_triangle_count,
            "boundary_edge_count":self.boundary_edge_count,
            "non_manifold_edge_count":self.non_manifold_edge_count,
            "winding_error_count":self.winding_error_count,
            "uv_non_finite_count":self.uv_non_finite_count,
            "zero_area_uv_triangle_count":self.zero_area_uv_triangle_count,
            "tangent_non_finite_count":self.tangent_non_finite_count,
            "tangent_orthogonality_error_count":self.tangent_orthogonality_error_count,
            "tangent_handedness_error_count":self.tangent_handedness_error_count,
            "external_uri_count":self.external_uri_count,
            "metadata_mismatch_count":self.metadata_mismatch_count,
            "connected_component_count":self.connected_component_count,
            "part_coverage":self.part_coverage,
            "source_coverage":self.source_coverage,
            "material_zone_coverage":self.material_zone_coverage,
            "glb_parse_status":self.glb_parse_status,
            "failure_codes":self.failure_codes,
        })
    }
}

#[derive(Default)]
struct Metrics {
    invalid_index_count: u64,
    non_finite_count: u64,
    degenerate_triangle_count: u64,
    boundary_edge_count: u64,
    non_manifold_edge_count: u64,
    winding_error_count: u64,
    uv_non_finite_count: u64,
    zero_area_uv_triangle_count: u64,
    tangent_non_finite_count: u64,
    tangent_orthogonality_error_count: u64,
    tangent_handedness_error_count: u64,
    external_uri_count: u64,
    metadata_mismatch_count: u64,
    lineage_missing_triangle_count: u64,
    part_bound_triangle_count: u64,
    source_bound_triangle_count: u64,
    material_bound_triangle_count: u64,
}

#[derive(Default)]
struct PartTopology {
    vertices: BTreeMap<[i64; 3], usize>,
    edges: BTreeMap<(usize, usize), Vec<bool>>,
    solid: bool,
}

/// A V2 artifact is not a general glTF admission format.  These are the only
/// binary accessor roles emitted by the product compiler.  Tracking the roles
/// lets the readback prove that every byte in the sole BIN chunk is consumed by
/// the static mesh it actually inspects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum V2AccessorRole {
    Position,
    Normal,
    Texcoord0,
    Tangent,
    Indices,
}

impl V2AccessorRole {
    fn layout(self) -> (&'static str, u64, usize, u64) {
        match self {
            Self::Position => ("VEC3", 5126, 12, 34962),
            Self::Normal => ("VEC3", 5126, 12, 34962),
            Self::Texcoord0 => ("VEC2", 5126, 8, 34962),
            Self::Tangent => ("VEC4", 5126, 16, 34962),
            Self::Indices => ("SCALAR", 5125, 4, 34963),
        }
    }
}

impl PartTopology {
    fn vertex(&mut self, position: [f32; 3]) -> usize {
        let key = position.map(|component| (component as f64 * POSITION_WELD_SCALE).round() as i64);
        let next = self.vertices.len();
        *self.vertices.entry(key).or_insert(next)
    }

    fn add_triangle(&mut self, positions: [[f32; 3]; 3]) {
        let vertices = positions.map(|position| self.vertex(position));
        for (from, to) in [
            (vertices[0], vertices[1]),
            (vertices[1], vertices[2]),
            (vertices[2], vertices[0]),
        ] {
            let key = if from < to { (from, to) } else { (to, from) };
            let forward = from < to;
            self.edges.entry(key).or_default().push(forward);
        }
    }

    fn connected_component_count(&self) -> usize {
        if self.vertices.is_empty() {
            return 0;
        }
        let mut parents = (0..self.vertices.len()).collect::<Vec<_>>();
        for &(first, second) in self.edges.keys() {
            let mut first_root = first;
            while parents[first_root] != first_root {
                let parent = parents[first_root];
                parents[first_root] = parents[parent];
                first_root = parents[first_root];
            }
            let mut second_root = second;
            while parents[second_root] != second_root {
                let parent = parents[second_root];
                parents[second_root] = parents[parent];
                second_root = parents[second_root];
            }
            if first_root != second_root {
                parents[first_root] = second_root;
            }
        }
        let mut roots = BTreeSet::new();
        for index in 0..parents.len() {
            let mut root = index;
            while parents[root] != root {
                let parent = parents[root];
                parents[root] = parents[parent];
                root = parents[root];
            }
            roots.insert(root);
        }
        roots.len()
    }
}

fn continuous_uv_atlas(
    forgecad: &Map<String, Value>,
) -> Result<Option<ContinuousUvAtlas>, GeometryError> {
    let Some(atlas) = forgecad.get("uv_atlas").and_then(Value::as_object) else {
        return Ok(None);
    };
    if atlas.get("packing").and_then(Value::as_str) != Some("connected-dominant-axis-islands@1") {
        return Ok(None);
    }
    if atlas.get("schema_version").and_then(Value::as_str) != Some("UvAtlas@1")
        || atlas.get("resolution").and_then(Value::as_u64) != Some(2048)
        || atlas.get("padding_texels").and_then(Value::as_u64) != Some(8)
        || atlas.get("atlas_count").and_then(Value::as_u64) != Some(1)
        || atlas.get("seam_policy").and_then(Value::as_str)
            != Some("edge-shared-or-explicit-seam@1")
        || atlas.get("overlap_policy").and_then(Value::as_str) != Some("disjoint-grid-cells@1")
    {
        return Err(GeometryError::Invalid(
            "continuous UV atlas policy metadata is invalid".to_owned(),
        ));
    }
    let chart_count = atlas
        .get("charts")
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .filter(|value| *value > 0)
        .ok_or_else(|| {
            GeometryError::Invalid("continuous UV atlas chart count is invalid".to_owned())
        })?;
    let columns = (chart_count as f32).sqrt().ceil().max(1.0) as usize;
    let rows = chart_count.div_ceil(columns).max(1);
    let padding_uv = 8.0 / 2048.0;
    if 1.0 / columns as f32 <= 2.0 * padding_uv || 1.0 / rows as f32 <= 2.0 * padding_uv {
        return Err(GeometryError::Invalid(
            "continuous UV atlas exceeds its fixed padding budget".to_owned(),
        ));
    }
    Ok(Some(ContinuousUvAtlas {
        chart_count,
        columns,
        rows,
        padding_uv,
    }))
}

fn validate_continuous_uv_primitive(
    extras: Option<&Map<String, Value>>,
    positions: &[[f32; 3]],
    uvs: &[[f32; 2]],
    indices: &[u32],
    atlas: ContinuousUvAtlas,
    seen_chart_ids: &mut BTreeSet<usize>,
) -> Result<(), GeometryError> {
    const MAX_OVERLAP_COMPARISONS: usize = 1_000_000;
    let extras = extras.ok_or_else(|| {
        GeometryError::Invalid("continuous UV primitive metadata is missing".to_owned())
    })?;
    let chart_values = extras
        .get("uv_chart_ids")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            GeometryError::Invalid("continuous UV primitive chart ids are missing".to_owned())
        })?;
    if chart_values.len() != indices.len() / 3 {
        return Err(GeometryError::Invalid(
            "continuous UV primitive chart count does not match its triangles".to_owned(),
        ));
    }
    let encoded = serde_json::to_vec(chart_values).map_err(|error| {
        GeometryError::Invalid(format!("continuous UV chart encoding failed: {error}"))
    })?;
    let actual_hash = hex_sha256(&encoded);
    if extras
        .get("uv_chart_assignment_sha256")
        .and_then(Value::as_str)
        != Some(actual_hash.as_str())
    {
        return Err(GeometryError::Invalid(
            "continuous UV primitive chart assignment hash drifted".to_owned(),
        ));
    }
    let chart_ids = chart_values
        .iter()
        .map(|value| {
            value
                .as_u64()
                .and_then(|value| usize::try_from(value).ok())
                .filter(|value| *value < atlas.chart_count)
                .ok_or_else(|| {
                    GeometryError::Invalid(
                        "continuous UV primitive chart id is out of range".to_owned(),
                    )
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    seen_chart_ids.extend(chart_ids.iter().copied());

    type PositionKey = [i64; 3];
    type EdgeRecord = (usize, [f32; 2], [f32; 2]);
    let mut edges = BTreeMap::<(PositionKey, PositionKey), Vec<EdgeRecord>>::new();
    let mut chart_triangles = BTreeMap::<usize, Vec<[[f32; 2]; 3]>>::new();
    let cell_u = 1.0 / atlas.columns as f32;
    let cell_v = 1.0 / atlas.rows as f32;
    for (triangle_index, triangle) in indices.chunks_exact(3).enumerate() {
        let source = [
            triangle[0] as usize,
            triangle[1] as usize,
            triangle[2] as usize,
        ];
        if source
            .iter()
            .any(|index| *index >= positions.len() || *index >= uvs.len())
        {
            return Err(GeometryError::Invalid(
                "continuous UV primitive index is out of range".to_owned(),
            ));
        }
        let chart = chart_ids[triangle_index];
        let column = chart % atlas.columns;
        let row = chart / atlas.columns;
        let min_uv = [
            column as f32 * cell_u + atlas.padding_uv,
            row as f32 * cell_v + atlas.padding_uv,
        ];
        let max_uv = [
            (column + 1) as f32 * cell_u - atlas.padding_uv,
            (row + 1) as f32 * cell_v - atlas.padding_uv,
        ];
        let triangle_uvs = source.map(|index| uvs[index]);
        if triangle_uvs
            .iter()
            .flatten()
            .any(|value| !value.is_finite())
            || triangle_uvs.iter().any(|uv| {
                uv[0] < min_uv[0] - 1.0e-6
                    || uv[0] > max_uv[0] + 1.0e-6
                    || uv[1] < min_uv[1] - 1.0e-6
                    || uv[1] > max_uv[1] + 1.0e-6
            })
        {
            return Err(GeometryError::Invalid(
                "continuous UV primitive violates its atlas cell or padding".to_owned(),
            ));
        }
        chart_triangles.entry(chart).or_default().push(triangle_uvs);
        let position_keys = source.map(|index| {
            positions[index]
                .map(|component| (component as f64 * POSITION_WELD_SCALE).round() as i64)
        });
        for (left, right) in [(0, 1), (1, 2), (2, 0)] {
            let (key, uv_left, uv_right) = if position_keys[left] < position_keys[right] {
                (
                    (position_keys[left], position_keys[right]),
                    triangle_uvs[left],
                    triangle_uvs[right],
                )
            } else {
                (
                    (position_keys[right], position_keys[left]),
                    triangle_uvs[right],
                    triangle_uvs[left],
                )
            };
            edges
                .entry(key)
                .or_default()
                .push((chart, uv_left, uv_right));
        }
    }
    for records in edges.values() {
        if records.len() == 2 && records[0].0 == records[1].0 {
            for axis in 0..2 {
                if (records[0].1[axis] - records[1].1[axis]).abs() > 1.0e-6
                    || (records[0].2[axis] - records[1].2[axis]).abs() > 1.0e-6
                {
                    return Err(GeometryError::Invalid(
                        "continuous UV shared edge carries discontinuous coordinates".to_owned(),
                    ));
                }
            }
        }
    }
    let mut comparisons = 0usize;
    for (chart, triangles) in &chart_triangles {
        for left in 0..triangles.len() {
            for right in (left + 1)..triangles.len() {
                comparisons += 1;
                if comparisons > MAX_OVERLAP_COMPARISONS {
                    return Err(GeometryError::Invalid(
                        "continuous UV overlap validation exceeds its bounded comparison budget"
                            .to_owned(),
                    ));
                }
                if triangle_intersection_area(triangles[left], triangles[right]) > 1.0e-10 {
                    return Err(GeometryError::Invalid(format!(
                        "continuous UV chart {chart} contains positive-area overlap between triangles {left} and {right}"
                    )));
                }
            }
        }
    }
    Ok(())
}

pub(crate) fn triangle_intersection_area(subject: [[f32; 2]; 3], clip: [[f32; 2]; 3]) -> f64 {
    let clip_area = signed_polygon_area(&clip);
    if clip_area.abs() <= 1.0e-12 {
        return 0.0;
    }
    let orientation = if clip_area > 0.0 { 1.0 } else { -1.0 };
    let mut polygon = subject.to_vec();
    for edge in 0..3 {
        let start = clip[edge];
        let end = clip[(edge + 1) % 3];
        let input = std::mem::take(&mut polygon);
        if input.is_empty() {
            break;
        }
        let mut previous = *input.last().unwrap_or(&input[0]);
        let mut previous_inside = oriented_side(start, end, previous) * orientation >= -1.0e-9;
        for current in input {
            let current_inside = oriented_side(start, end, current) * orientation >= -1.0e-9;
            if current_inside != previous_inside {
                if let Some(point) = segment_line_intersection(previous, current, start, end) {
                    polygon.push(point);
                }
            }
            if current_inside {
                polygon.push(current);
            }
            previous = current;
            previous_inside = current_inside;
        }
    }
    signed_polygon_area(&polygon).abs()
}

fn signed_polygon_area(points: &[[f32; 2]]) -> f64 {
    if points.len() < 3 {
        return 0.0;
    }
    let mut area = 0.0f64;
    for index in 0..points.len() {
        let next = (index + 1) % points.len();
        area += points[index][0] as f64 * points[next][1] as f64
            - points[index][1] as f64 * points[next][0] as f64;
    }
    area * 0.5
}

fn oriented_side(start: [f32; 2], end: [f32; 2], point: [f32; 2]) -> f64 {
    (end[0] - start[0]) as f64 * (point[1] - start[1]) as f64
        - (end[1] - start[1]) as f64 * (point[0] - start[0]) as f64
}

fn segment_line_intersection(
    segment_start: [f32; 2],
    segment_end: [f32; 2],
    line_start: [f32; 2],
    line_end: [f32; 2],
) -> Option<[f32; 2]> {
    let segment_delta = [
        segment_end[0] - segment_start[0],
        segment_end[1] - segment_start[1],
    ];
    let start_side = oriented_side(line_start, line_end, segment_start);
    let end_side = oriented_side(line_start, line_end, segment_end);
    let denominator = start_side - end_side;
    if denominator.abs() <= 1.0e-15 {
        return None;
    }
    let t = (start_side / denominator) as f32;
    Some([
        segment_start[0] + segment_delta[0] * t,
        segment_start[1] + segment_delta[1] * t,
    ])
}

fn hex_sha256(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Read the actual GLB chunks, accessors and triangle payload. Any malformed
/// chunk, offset, accessor or required attribute is rejected instead of being
/// converted into a `passed` metadata value.
pub fn inspect_glb(glb: &[u8]) -> Result<GlbIntegrity, GeometryError> {
    let (root, binary) = parse_glb(glb)?;
    let forgecad = root
        .get("extras")
        .and_then(|value| value.get("forgecad"))
        .and_then(Value::as_object)
        .ok_or_else(|| GeometryError::Invalid("GLB ForgeCAD lineage is missing".to_owned()))?;
    let artifact_schema_version = required_text(forgecad, "schema_version")?.to_owned();
    let program_sha256 = required_sha256(forgecad, "program_sha256")?.to_owned();
    let operator_catalog_sha256 = forgecad
        .get("operator_catalog_sha256")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let continuous_uv_atlas = continuous_uv_atlas(forgecad)?;
    let meshes = required_array(&root, "meshes")?;
    let nodes = required_array(&root, "nodes")?;
    let accessors = required_array(&root, "accessors")?;
    let views = required_array(&root, "bufferViews")?;
    let materials: &[Value] = root
        .get("materials")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);

    let mut metrics = Metrics::default();
    metrics.external_uri_count = external_uri_count(&root);
    let is_v2 = artifact_schema_version == "ArtifactReadback@2";
    let mut v2_accessor_uses = BTreeMap::<usize, V2AccessorRole>::new();
    let mut continuous_uv_chart_ids = BTreeSet::<usize>::new();
    if is_v2 {
        enforce_canonical_v2_asset_profile(&root, meshes, &mut metrics);
        enforce_canonical_v2_scene_graph(&root, meshes, nodes, &mut metrics);
    }
    let mut topology = BTreeMap::<String, PartTopology>::new();
    let mut bindings = BTreeMap::<BindingKey, u64>::new();
    let mut binding_order = Vec::<BindingKey>::new();
    let mut source_bindings = BTreeMap::<String, BindingKey>::new();
    let mut part_meshes = BTreeMap::<String, usize>::new();
    let mut triangle_count = 0u64;
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];

    for (mesh_index, mesh) in meshes.iter().enumerate() {
        let mesh_object = mesh
            .as_object()
            .ok_or_else(|| GeometryError::Invalid("GLB mesh is not an object".to_owned()))?;
        if is_v2 {
            enforce_canonical_v2_mesh_profile(mesh_object, &mut metrics);
        }
        let mesh_lineage = mesh_object.get("extras").and_then(Value::as_object);
        let node_lineages = matching_node_lineages(nodes, mesh_index);
        if is_v2 && node_lineages.len() != 1 {
            metrics.metadata_mismatch_count += 1;
        }
        let node_lineage = node_lineages.first();
        if is_v2 {
            let mesh_part_lineage = merge_lineage(mesh_lineage, node_lineage, None, &mut metrics);
            if let Some(part_id) = mesh_part_lineage.part_id {
                if part_meshes.insert(part_id, mesh_index).is_some() {
                    metrics.metadata_mismatch_count += 1;
                }
            } else {
                metrics.metadata_mismatch_count += 1;
            }
        }
        let primitives = mesh_object
            .get("primitives")
            .and_then(Value::as_array)
            .ok_or_else(|| GeometryError::Invalid("GLB primitive list is missing".to_owned()))?;
        if primitives.is_empty() {
            return Err(GeometryError::Invalid(
                "GLB mesh has no primitives".to_owned(),
            ));
        }
        for primitive in primitives {
            let primitive_object = primitive.as_object().ok_or_else(|| {
                GeometryError::Invalid("GLB primitive is not an object".to_owned())
            })?;
            if is_v2 {
                enforce_canonical_v2_primitive_profile(primitive_object, &mut metrics);
            }
            let primitive_lineage = primitive_object.get("extras").and_then(Value::as_object);
            let primitive_has_source_node_id = primitive_lineage
                .and_then(|lineage| lineage.get("source_node_id"))
                .and_then(Value::as_str)
                .is_some_and(|value| !value.is_empty());
            let lineage =
                merge_lineage(mesh_lineage, node_lineage, primitive_lineage, &mut metrics);
            let lineage_key = lineage.complete().map(|lineage| {
                (
                    lineage.part_id,
                    lineage.source_node_id,
                    lineage.material_zone_id,
                    lineage.solid,
                )
            });
            if let Some(lineage_key) = &lineage_key {
                match source_bindings.entry(lineage_key.1.clone()) {
                    std::collections::btree_map::Entry::Vacant(entry) => {
                        entry.insert(lineage_key.clone());
                        binding_order.push(lineage_key.clone());
                    }
                    std::collections::btree_map::Entry::Occupied(_) => {
                        // One typed primitive node may feed exactly one output
                        // primitive.  Repeating it would make source lineage
                        // ambiguous even when the surrounding metadata matches.
                        metrics.metadata_mismatch_count += 1;
                    }
                }
            }
            if primitive_object
                .get("mode")
                .and_then(Value::as_u64)
                .unwrap_or(4)
                != 4
            {
                return Err(GeometryError::Invalid(
                    "GLB primitive must use triangle mode".to_owned(),
                ));
            }
            let attributes = primitive_object
                .get("attributes")
                .and_then(Value::as_object)
                .ok_or_else(|| {
                    GeometryError::Invalid("GLB primitive attributes are missing".to_owned())
                })?;
            let position_accessor = required_index(attributes, "POSITION")?;
            let normal_accessor = required_index(attributes, "NORMAL")?;
            let uv_accessor = required_index(attributes, "TEXCOORD_0")?;
            let tangent_accessor = required_index(attributes, "TANGENT")?;
            let index_accessor = primitive_object
                .get("indices")
                .and_then(Value::as_u64)
                .and_then(|value| usize::try_from(value).ok())
                .ok_or_else(|| {
                    GeometryError::Invalid("GLB primitive index accessor is missing".to_owned())
                })?;
            if is_v2 {
                record_v2_accessor_use(
                    &mut v2_accessor_uses,
                    position_accessor,
                    V2AccessorRole::Position,
                    &mut metrics,
                );
                record_v2_accessor_use(
                    &mut v2_accessor_uses,
                    normal_accessor,
                    V2AccessorRole::Normal,
                    &mut metrics,
                );
                record_v2_accessor_use(
                    &mut v2_accessor_uses,
                    uv_accessor,
                    V2AccessorRole::Texcoord0,
                    &mut metrics,
                );
                record_v2_accessor_use(
                    &mut v2_accessor_uses,
                    tangent_accessor,
                    V2AccessorRole::Tangent,
                    &mut metrics,
                );
                record_v2_accessor_use(
                    &mut v2_accessor_uses,
                    index_accessor,
                    V2AccessorRole::Indices,
                    &mut metrics,
                );
            }
            let positions = read_vec3(&accessors, &views, &binary, position_accessor)?;
            let normals = read_vec3(&accessors, &views, &binary, normal_accessor)?;
            let uvs = read_vec2(&accessors, &views, &binary, uv_accessor)?;
            let tangents = read_vec4(&accessors, &views, &binary, tangent_accessor)?;
            let indices = read_indices(&accessors, &views, &binary, index_accessor)?;
            if positions.len() != normals.len()
                || positions.len() != uvs.len()
                || positions.len() != tangents.len()
            {
                return Err(GeometryError::Invalid(
                    "GLB vertex attribute counts do not match".to_owned(),
                ));
            }
            if let Some(atlas) = continuous_uv_atlas {
                validate_continuous_uv_primitive(
                    primitive_lineage,
                    &positions,
                    &uvs,
                    &indices,
                    atlas,
                    &mut continuous_uv_chart_ids,
                )?;
            }
            for position in &positions {
                if !finite3(*position) {
                    metrics.non_finite_count += 1;
                } else {
                    for axis in 0..3 {
                        min[axis] = min[axis].min(position[axis]);
                        max[axis] = max[axis].max(position[axis]);
                    }
                }
            }
            for normal in &normals {
                if !finite3(*normal) || length3(*normal) <= f32::EPSILON {
                    metrics.non_finite_count += 1;
                }
            }
            for uv in &uvs {
                if !uv[0].is_finite() || !uv[1].is_finite() {
                    metrics.uv_non_finite_count += 1;
                    metrics.non_finite_count += 1;
                }
            }
            for tangent in &tangents {
                if !finite4(*tangent) {
                    metrics.tangent_non_finite_count += 1;
                    metrics.non_finite_count += 1;
                    continue;
                }
                if (tangent[3].abs() - 1.0).abs() > ORTHOGONALITY_EPSILON {
                    metrics.tangent_handedness_error_count += 1;
                }
            }
            let material = primitive_object
                .get("material")
                .and_then(Value::as_u64)
                .and_then(|index| materials.get(index as usize));
            let material_lineage_matches = material.is_some_and(|material| {
                lineage
                    .material_zone_id
                    .as_deref()
                    .is_some_and(|zone| material.get("name").and_then(Value::as_str) == Some(zone))
            });
            if !material_lineage_matches && lineage.material_zone_id.is_some() {
                metrics.lineage_missing_triangle_count += (indices.len() / 3) as u64;
            }
            if indices.len() % 3 != 0 {
                metrics.invalid_index_count += (indices.len() % 3) as u64;
            }
            for triangle in indices.chunks_exact(3) {
                triangle_count += 1;
                if triangle
                    .iter()
                    .any(|index| *index as usize >= positions.len())
                {
                    metrics.invalid_index_count += 1;
                    continue;
                }
                let indices = [
                    triangle[0] as usize,
                    triangle[1] as usize,
                    triangle[2] as usize,
                ];
                let triangle_positions = [
                    positions[indices[0]],
                    positions[indices[1]],
                    positions[indices[2]],
                ];
                let triangle_normals = [
                    normals[indices[0]],
                    normals[indices[1]],
                    normals[indices[2]],
                ];
                let triangle_uvs = [uvs[indices[0]], uvs[indices[1]], uvs[indices[2]]];
                let triangle_tangents = [
                    tangents[indices[0]],
                    tangents[indices[1]],
                    tangents[indices[2]],
                ];
                let face = cross3(
                    sub3(triangle_positions[1], triangle_positions[0]),
                    sub3(triangle_positions[2], triangle_positions[0]),
                );
                let face_length = length3(face);
                if !face_length.is_finite() || face_length <= DEGENERATE_AREA_EPSILON {
                    metrics.degenerate_triangle_count += 1;
                    continue;
                }
                let normal_average = normalize3(add3(
                    add3(triangle_normals[0], triangle_normals[1]),
                    triangle_normals[2],
                ));
                let normalized_face = scale3(face, 1.0 / face_length);
                if length3(normal_average) <= f32::EPSILON
                    || dot3(normalized_face, normal_average) <= 0.0
                {
                    metrics.winding_error_count += 1;
                }
                let uv_area = (triangle_uvs[1][0] - triangle_uvs[0][0])
                    * (triangle_uvs[2][1] - triangle_uvs[0][1])
                    - (triangle_uvs[1][1] - triangle_uvs[0][1])
                        * (triangle_uvs[2][0] - triangle_uvs[0][0]);
                if !uv_area.is_finite() || uv_area.abs() <= UV_AREA_EPSILON {
                    metrics.zero_area_uv_triangle_count += 1;
                }
                let tangent_frame =
                    tangent_frame_from_geometry(triangle_positions, triangle_uvs, uv_area);
                for (normal, tangent) in triangle_normals.iter().zip(triangle_tangents.iter()) {
                    let tangent3 = [tangent[0], tangent[1], tangent[2]];
                    if finite3(*normal)
                        && finite3(tangent3)
                        && length3(*normal) > f32::EPSILON
                        && length3(tangent3) > f32::EPSILON
                        && dot3(normalize3(*normal), normalize3(tangent3)).abs()
                            > ORTHOGONALITY_EPSILON
                    {
                        metrics.tangent_orthogonality_error_count += 1;
                    }
                    if let Some((tangent_basis, bitangent_basis)) = tangent_frame {
                        if finite3(*normal)
                            && finite4(*tangent)
                            && (tangent[3].abs() - 1.0).abs() <= ORTHOGONALITY_EPSILON
                            && !tangent_handedness_matches_geometry(
                                *normal,
                                *tangent,
                                tangent_basis,
                                bitangent_basis,
                            )
                        {
                            metrics.tangent_handedness_error_count += 1;
                        }
                    }
                }
                if lineage.part_id.is_some() {
                    metrics.part_bound_triangle_count += 1;
                }
                if lineage.source_node_id.is_some() {
                    metrics.source_bound_triangle_count += 1;
                }
                if lineage.material_zone_id.is_some() && material_lineage_matches {
                    metrics.material_bound_triangle_count += 1;
                }
                if is_v2 && !primitive_has_source_node_id {
                    metrics.lineage_missing_triangle_count += 1;
                }
                if let Some((part_id, source_node_id, material_zone_id, solid)) = &lineage_key {
                    let part = topology.entry(part_id.clone()).or_default();
                    part.solid |= *solid;
                    part.add_triangle(triangle_positions);
                    *bindings
                        .entry((
                            part_id.clone(),
                            source_node_id.clone(),
                            material_zone_id.clone(),
                            *solid,
                        ))
                        .or_default() += 1;
                } else {
                    metrics.lineage_missing_triangle_count += 1;
                }
            }
        }
    }

    if is_v2 {
        enforce_canonical_v2_static_bin_layout(
            &root,
            &binary,
            accessors,
            views,
            &v2_accessor_uses,
            &mut metrics,
        );
        validate_embedded_texture_build(&root, &binary, views, forgecad)?;
    }
    if let Some(atlas) = continuous_uv_atlas {
        if continuous_uv_chart_ids.len() != atlas.chart_count
            || continuous_uv_chart_ids
                .iter()
                .copied()
                .ne(0..atlas.chart_count)
        {
            return Err(GeometryError::Invalid(
                "continuous UV atlas chart coverage does not match physical primitive metadata"
                    .to_owned(),
            ));
        }
    }

    for part in topology.values() {
        for directions in part.edges.values() {
            match directions.len() {
                1 => {
                    if part.solid {
                        metrics.boundary_edge_count += 1;
                    }
                }
                2 => {
                    // A non-solid review representation may contain separate
                    // hard-surface shells that touch at the same quantized
                    // position without declaring a welded solid. Per-triangle
                    // normal/winding validation above still applies; the
                    // closed-edge orientation rule is meaningful only when
                    // the Part explicitly claims solid topology.
                    if part.solid && directions[0] == directions[1] {
                        metrics.winding_error_count += 1;
                    }
                }
                _ => {
                    if part.solid {
                        metrics.non_manifold_edge_count += 1;
                    }
                }
            }
        }
    }
    let connected_component_count = topology
        .values()
        .map(PartTopology::connected_component_count)
        .sum::<usize>() as u64;

    let mut part_bindings = Vec::with_capacity(binding_order.len());
    for (part_id, source_node_id, material_zone_id, solid) in &binding_order {
        let key = (
            part_id.clone(),
            source_node_id.clone(),
            material_zone_id.clone(),
            *solid,
        );
        if let Some(triangle_count) = bindings.get(&key) {
            part_bindings.push(PartBinding {
                part_id: part_id.clone(),
                source_node_id: source_node_id.clone(),
                material_zone_id: material_zone_id.clone(),
                solid: *solid,
                triangle_count: *triangle_count,
            });
        } else {
            metrics.metadata_mismatch_count += 1;
        }
    }
    if part_bindings.len() != bindings.len() {
        metrics.metadata_mismatch_count += 1;
    }
    compare_declared_metadata(forgecad, &part_bindings, triangle_count, &mut metrics);
    let coverage = |covered_triangles: u64| {
        if triangle_count == 0 {
            0.0
        } else {
            covered_triangles as f64 / triangle_count as f64
        }
    };
    let part_coverage = coverage(metrics.part_bound_triangle_count);
    let source_coverage = coverage(metrics.source_bound_triangle_count);
    let material_zone_coverage = coverage(metrics.material_bound_triangle_count);
    let part_ids = unique_strings_in_order(part_bindings.iter().map(|binding| &binding.part_id));
    let source_node_ids =
        unique_strings_in_order(part_bindings.iter().map(|binding| &binding.source_node_id));
    let material_zone_ids = unique_strings_in_order(
        part_bindings
            .iter()
            .map(|binding| &binding.material_zone_id),
    );
    let mut failure_codes = failure_codes(
        &metrics,
        triangle_count,
        part_coverage,
        source_coverage,
        material_zone_coverage,
        &artifact_schema_version,
        operator_catalog_sha256.as_deref(),
    );
    failure_codes.sort();
    failure_codes.dedup();
    let hard_gate_passed = failure_codes.is_empty();
    let width = (max[0] - min[0]).max(0.0001) as f64;
    let height = (max[1] - min[1]).max(0.0001) as f64;
    Ok(GlbIntegrity {
        artifact_schema_version,
        program_sha256,
        operator_catalog_sha256,
        readback_config_sha256: sha256(READBACK_CONFIG.as_bytes()),
        part_ids,
        source_node_ids,
        material_zone_ids,
        part_bindings,
        triangle_count,
        connected_component_count,
        invalid_index_count: metrics.invalid_index_count,
        non_finite_count: metrics.non_finite_count,
        degenerate_triangle_count: metrics.degenerate_triangle_count,
        boundary_edge_count: metrics.boundary_edge_count,
        non_manifold_edge_count: metrics.non_manifold_edge_count,
        winding_error_count: metrics.winding_error_count,
        uv_non_finite_count: metrics.uv_non_finite_count,
        zero_area_uv_triangle_count: metrics.zero_area_uv_triangle_count,
        tangent_non_finite_count: metrics.tangent_non_finite_count,
        tangent_orthogonality_error_count: metrics.tangent_orthogonality_error_count,
        tangent_handedness_error_count: metrics.tangent_handedness_error_count,
        external_uri_count: metrics.external_uri_count,
        metadata_mismatch_count: metrics.metadata_mismatch_count,
        part_coverage,
        source_coverage,
        material_zone_coverage,
        glb_parse_status: "passed".to_owned(),
        validator_status: if hard_gate_passed { "passed" } else { "failed" }.to_owned(),
        hard_gate_passed,
        failure_codes: failure_codes.into_iter().map(str::to_owned).collect(),
        aspect_ratio: width / height,
    })
}

#[derive(Default)]
struct Lineage {
    part_id: Option<String>,
    source_node_id: Option<String>,
    material_zone_id: Option<String>,
    solid: bool,
}

impl Lineage {
    fn complete(&self) -> Option<CompleteLineage> {
        Some(CompleteLineage {
            part_id: self.part_id.clone()?,
            source_node_id: self.source_node_id.clone()?,
            material_zone_id: self.material_zone_id.clone()?,
            solid: self.solid,
        })
    }
}

#[derive(Debug, Clone)]
struct CompleteLineage {
    part_id: String,
    source_node_id: String,
    material_zone_id: String,
    solid: bool,
}

/// The V2 writer emits one glTF mesh/node pair per semantic Part. A Part may
/// have several primitive `source_node_id`s, while multiple Parts may reuse
/// one identical AppearanceProgram@2/@3 MaterialZone/material index. The node
/// intentionally carries mesh-level lineage; source lineage remains on each
/// primitive.
#[derive(Debug, Clone, PartialEq, Eq)]
struct PartLineage {
    part_id: String,
    material_zone_id: String,
    solid: bool,
}

/// The V2 product artifact is a closed static profile, rather than a generic
/// glTF scene.  Embedded PNG textures are the sole permitted image path for
/// MCP010E; animation, skinning, morph targets, cameras and external URIs stay
/// rejected.  Texture bytes are accounted for as image buffer views below so
/// they cannot hide an uninspected payload in BIN.
fn enforce_canonical_v2_asset_profile(root: &Value, meshes: &[Value], metrics: &mut Metrics) {
    let Some(root) = root.as_object() else {
        metrics.metadata_mismatch_count += 1;
        return;
    };
    if !has_only_keys(
        root,
        &[
            "asset",
            "scene",
            "scenes",
            "nodes",
            "meshes",
            "materials",
            "buffers",
            "bufferViews",
            "accessors",
            "images",
            "textures",
            "extensionsUsed",
            "extras",
        ],
    ) {
        metrics.metadata_mismatch_count += 1;
    }
    if let Some(extensions_used) = root.get("extensionsUsed") {
        if extensions_used != &json!(["KHR_materials_clearcoat"]) {
            metrics.metadata_mismatch_count += 1;
        }
    }

    let Some(asset) = root.get("asset").and_then(Value::as_object) else {
        metrics.metadata_mismatch_count += 1;
        return;
    };
    if !has_only_keys(asset, &["version", "generator"])
        || asset.get("version").and_then(Value::as_str) != Some("2.0")
        || asset
            .get("generator")
            .and_then(Value::as_str)
            .is_none_or(str::is_empty)
    {
        metrics.metadata_mismatch_count += 1;
    }

    let Some(scenes) = root.get("scenes").and_then(Value::as_array) else {
        metrics.metadata_mismatch_count += 1;
        return;
    };
    for scene in scenes {
        let Some(scene) = scene.as_object() else {
            metrics.metadata_mismatch_count += 1;
            continue;
        };
        if !has_only_keys(scene, &["nodes"]) {
            metrics.metadata_mismatch_count += 1;
        }
    }

    let Some(materials) = root.get("materials").and_then(Value::as_array) else {
        metrics.metadata_mismatch_count += 1;
        return;
    };
    if materials.is_empty() || materials.len() > meshes.len() {
        metrics.metadata_mismatch_count += 1;
    }
    for material in materials {
        let Some(material) = material.as_object() else {
            metrics.metadata_mismatch_count += 1;
            continue;
        };
        if !has_only_keys(
            material,
            &[
                "name",
                "pbrMetallicRoughness",
                "emissiveFactor",
                "normalTexture",
                "occlusionTexture",
                "emissiveTexture",
                "extensions",
                "extras",
            ],
        ) {
            metrics.metadata_mismatch_count += 1;
        }
        let Some(pbr) = material
            .get("pbrMetallicRoughness")
            .and_then(Value::as_object)
        else {
            metrics.metadata_mismatch_count += 1;
            continue;
        };
        if !has_only_keys(
            pbr,
            &[
                "baseColorFactor",
                "metallicFactor",
                "roughnessFactor",
                "baseColorTexture",
                "metallicRoughnessTexture",
            ],
        ) {
            metrics.metadata_mismatch_count += 1;
        }
        for texture_key in [
            "baseColorTexture",
            "metallicRoughnessTexture",
            "normalTexture",
            "occlusionTexture",
            "emissiveTexture",
        ] {
            let Some(texture) = material.get(texture_key).or_else(|| pbr.get(texture_key)) else {
                continue;
            };
            if !has_exact_keys(texture.as_object().unwrap_or(&Map::new()), &["index"])
                || texture.get("index").and_then(Value::as_u64).is_none()
            {
                metrics.metadata_mismatch_count += 1;
            }
        }
        if let Some(images) = root.get("images").and_then(Value::as_array) {
            for image in images {
                let Some(image) = image.as_object() else {
                    metrics.metadata_mismatch_count += 1;
                    continue;
                };
                if !has_exact_keys(image, &["bufferView", "mimeType", "name"])
                    || image.get("mimeType").and_then(Value::as_str) != Some("image/png")
                    || image.get("bufferView").and_then(Value::as_u64).is_none()
                {
                    metrics.metadata_mismatch_count += 1;
                }
            }
        }
        if let Some(textures) = root.get("textures").and_then(Value::as_array) {
            for texture in textures {
                let Some(texture) = texture.as_object() else {
                    metrics.metadata_mismatch_count += 1;
                    continue;
                };
                if !has_exact_keys(texture, &["source"])
                    || texture.get("source").and_then(Value::as_u64).is_none()
                {
                    metrics.metadata_mismatch_count += 1;
                }
            }
        }
        if let Some(extensions) = material.get("extensions").and_then(Value::as_object) {
            if extensions.keys().any(|key| {
                !matches!(
                    key.as_str(),
                    "KHR_materials_clearcoat" | "KHR_materials_emissive_strength"
                )
            }) {
                metrics.metadata_mismatch_count += 1;
            }
            if let Some(clearcoat) = extensions
                .get("KHR_materials_clearcoat")
                .and_then(Value::as_object)
            {
                if !has_only_keys(
                    clearcoat,
                    &[
                        "clearcoatFactor",
                        "clearcoatTexture",
                        "clearcoatRoughnessFactor",
                        "clearcoatRoughnessTexture",
                    ],
                ) || clearcoat
                    .get("clearcoatFactor")
                    .and_then(Value::as_f64)
                    .is_none_or(|value| !(0.0..=1.0).contains(&value))
                {
                    metrics.metadata_mismatch_count += 1;
                }
                for field in ["clearcoatTexture", "clearcoatRoughnessTexture"] {
                    let Some(texture) = clearcoat.get(field) else {
                        continue;
                    };
                    if !has_exact_keys(texture.as_object().unwrap_or(&Map::new()), &["index"])
                        || texture
                            .get("index")
                            .and_then(Value::as_u64)
                            .is_none_or(|index| {
                                root.get("textures")
                                    .and_then(Value::as_array)
                                    .is_none_or(|textures| index as usize >= textures.len())
                            })
                    {
                        metrics.metadata_mismatch_count += 1;
                    }
                }
                if clearcoat.get("clearcoatRoughnessTexture").is_some()
                    && clearcoat
                        .get("clearcoatRoughnessFactor")
                        .and_then(Value::as_f64)
                        .is_none_or(|value| !(0.0..=1.0).contains(&value))
                {
                    metrics.metadata_mismatch_count += 1;
                }
            }
        }
    }
}

fn enforce_canonical_v2_mesh_profile(mesh: &Map<String, Value>, metrics: &mut Metrics) {
    if !has_only_keys(mesh, &["name", "primitives", "extras"])
        || !mesh.contains_key("name")
        || !mesh.contains_key("extras")
    {
        metrics.metadata_mismatch_count += 1;
    }
}

fn enforce_canonical_v2_primitive_profile(primitive: &Map<String, Value>, metrics: &mut Metrics) {
    if !has_only_keys(
        primitive,
        &["attributes", "indices", "material", "extras", "mode"],
    ) || !primitive.contains_key("indices")
        || !primitive.contains_key("material")
        || !primitive.contains_key("extras")
    {
        metrics.metadata_mismatch_count += 1;
    }
    let Some(attributes) = primitive.get("attributes").and_then(Value::as_object) else {
        metrics.metadata_mismatch_count += 1;
        return;
    };
    if !has_exact_keys(attributes, &["POSITION", "NORMAL", "TEXCOORD_0", "TANGENT"]) {
        metrics.metadata_mismatch_count += 1;
    }
}

fn png_payload_without_glb_padding(bytes: &[u8]) -> Result<&[u8], GeometryError> {
    const PNG_SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
    if bytes.len() < PNG_SIGNATURE.len() || &bytes[..8] != PNG_SIGNATURE {
        return Err(GeometryError::Invalid(
            "embedded texture is not a PNG payload".to_owned(),
        ));
    }
    let mut offset = 8usize;
    loop {
        if offset.checked_add(12).is_none_or(|end| end > bytes.len()) {
            return Err(GeometryError::Invalid(
                "embedded PNG chunk is truncated".to_owned(),
            ));
        }
        let length = u32::from_be_bytes(
            bytes[offset..offset + 4]
                .try_into()
                .map_err(|_| GeometryError::Invalid("embedded PNG length is invalid".to_owned()))?,
        ) as usize;
        let chunk_end = offset
            .checked_add(12)
            .and_then(|value| value.checked_add(length))
            .filter(|end| *end <= bytes.len())
            .ok_or_else(|| GeometryError::Invalid("embedded PNG chunk overflowed".to_owned()))?;
        let kind = &bytes[offset + 4..offset + 8];
        if kind == b"IEND" {
            if length != 0 {
                return Err(GeometryError::Invalid(
                    "embedded PNG IEND length is invalid".to_owned(),
                ));
            }
            let trailing = &bytes[chunk_end..];
            if trailing.len() > 3 || trailing.iter().any(|byte| *byte != 0) {
                return Err(GeometryError::Invalid(
                    "embedded PNG has non-canonical trailing bytes".to_owned(),
                ));
            }
            return Ok(&bytes[..chunk_end]);
        }
        offset = chunk_end;
    }
}

fn embedded_image_bytes<'a>(
    image: &Value,
    views: &[Value],
    binary: &'a [u8],
) -> Result<&'a [u8], GeometryError> {
    let view_index = image
        .get("bufferView")
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| GeometryError::Invalid("embedded PNG bufferView is missing".to_owned()))?;
    let view = views
        .get(view_index)
        .and_then(Value::as_object)
        .ok_or_else(|| GeometryError::Invalid("embedded PNG bufferView is invalid".to_owned()))?;
    let offset = view
        .get("byteOffset")
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| GeometryError::Invalid("embedded PNG byteOffset is invalid".to_owned()))?;
    let length = view
        .get("byteLength")
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| GeometryError::Invalid("embedded PNG byteLength is invalid".to_owned()))?;
    let end = offset
        .checked_add(length)
        .filter(|end| *end <= binary.len())
        .ok_or_else(|| GeometryError::Invalid("embedded PNG range overflowed".to_owned()))?;
    png_payload_without_glb_padding(&binary[offset..end])
}

fn validate_embedded_texture_build(
    root: &Value,
    binary: &[u8],
    views: &[Value],
    forgecad: &Map<String, Value>,
) -> Result<(), GeometryError> {
    let pack_id = forgecad.get("material_pack_id").and_then(Value::as_str);
    if pack_id != Some("forgecad-fictional-energy-weapon-2k") {
        if forgecad.get("texture_build").is_some() {
            return Err(GeometryError::Invalid(
                "embedded 2K texture build metadata is attached to the wrong pack".to_owned(),
            ));
        }
        return Ok(());
    }
    let texture_build = forgecad
        .get("texture_build")
        .and_then(Value::as_object)
        .ok_or_else(|| GeometryError::Invalid("2K texture build metadata is missing".to_owned()))?;
    if !has_exact_keys(
        texture_build,
        &[
            "schema_version",
            "recipe_id",
            "algorithm",
            "worker_algorithm_sha256",
            "resolution",
            "build_budget_ms",
            "embedded_only",
            "external_uri",
            "network_at_runtime",
            "outputs",
            "canonical_sha256",
        ],
    ) || texture_build.get("schema_version").and_then(Value::as_str)
        != Some("EmbeddedTextureBuild@1")
        || texture_build.get("recipe_id").and_then(Value::as_str)
            != Some("forgecad-first-party-catmullrom-semantic-microdetail-2k@1")
        || texture_build.get("algorithm").and_then(Value::as_str)
            != Some("catmullrom-plus-fixed-semantic-microdetail@1")
        || texture_build.get("resolution").and_then(Value::as_u64) != Some(2048)
        || texture_build.get("build_budget_ms").and_then(Value::as_u64) != Some(120_000)
        || texture_build.get("embedded_only").and_then(Value::as_bool) != Some(true)
        || texture_build.get("external_uri").and_then(Value::as_bool) != Some(false)
        || texture_build
            .get("network_at_runtime")
            .and_then(Value::as_bool)
            != Some(false)
    {
        return Err(GeometryError::Invalid(
            "2K texture build policy drifted".to_owned(),
        ));
    }
    let algorithm_hash = hex_sha256(
        b"catmullrom-plus-fixed-semantic-microdetail@1|2048|fixed-64px-integer-tile|sRGB-baseColor|linear-data|OpenGL+Y|no-rng-no-time-no-network",
    );
    if texture_build
        .get("worker_algorithm_sha256")
        .and_then(Value::as_str)
        != Some(algorithm_hash.as_str())
    {
        return Err(GeometryError::Invalid(
            "2K texture Worker algorithm identity drifted".to_owned(),
        ));
    }
    let expected_canonical = texture_build
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| GeometryError::Invalid("2K texture build hash is missing".to_owned()))?;
    let mut preimage = texture_build.clone();
    preimage.remove("canonical_sha256");
    if canonical_json_hash(&Value::Object(preimage)) != expected_canonical {
        return Err(GeometryError::Invalid(
            "2K texture build hash drifted".to_owned(),
        ));
    }

    let manifest = material_pack_manifest_by_id("forgecad-fictional-energy-weapon-2k")
        .ok_or_else(|| GeometryError::Invalid("2K material pack is unavailable".to_owned()))?;
    if forgecad
        .get("material_pack_manifest_sha256")
        .and_then(Value::as_str)
        != manifest.get("canonical_sha256").and_then(Value::as_str)
    {
        return Err(GeometryError::Invalid(
            "2K material pack manifest binding drifted".to_owned(),
        ));
    }
    let mut manifest_outputs = BTreeMap::<String, &Value>::new();
    for output in manifest
        .get("textures")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .chain(
            manifest
                .get("derived_outputs")
                .and_then(Value::as_array)
                .into_iter()
                .flatten(),
        )
    {
        let id = output
            .get("texture_id")
            .and_then(Value::as_str)
            .ok_or_else(|| GeometryError::Invalid("2K manifest output ID is missing".to_owned()))?;
        if manifest_outputs.insert(id.to_owned(), output).is_some() {
            return Err(GeometryError::Invalid(
                "2K manifest output IDs are not unique".to_owned(),
            ));
        }
    }
    let images = root
        .get("images")
        .and_then(Value::as_array)
        .ok_or_else(|| GeometryError::Invalid("2K GLB image inventory is missing".to_owned()))?;
    let outputs = texture_build
        .get("outputs")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            GeometryError::Invalid("2K texture output inventory is missing".to_owned())
        })?;
    if forgecad.get("texture_count").and_then(Value::as_u64) != Some(images.len() as u64) {
        return Err(GeometryError::Invalid(
            "2K embedded image count does not match the build receipt".to_owned(),
        ));
    }
    let mut image_by_name = BTreeMap::<String, &Value>::new();
    for image in images {
        let name = image.get("name").and_then(Value::as_str).ok_or_else(|| {
            GeometryError::Invalid("2K embedded image name is missing".to_owned())
        })?;
        if image_by_name.insert(name.to_owned(), image).is_some() {
            return Err(GeometryError::Invalid(
                "2K embedded image names are duplicated".to_owned(),
            ));
        }
    }
    let mut seen = BTreeSet::new();
    for output in outputs {
        let output = output
            .as_object()
            .ok_or_else(|| GeometryError::Invalid("2K output entry is invalid".to_owned()))?;
        let name = output
            .get("texture_id")
            .and_then(Value::as_str)
            .ok_or_else(|| GeometryError::Invalid("2K output ID is missing".to_owned()))?;
        if !seen.insert(name.to_owned()) {
            return Err(GeometryError::Invalid(
                "2K texture outputs are duplicated".to_owned(),
            ));
        }
        let image = image_by_name.get(name).ok_or_else(|| {
            GeometryError::Invalid(format!("2K texture output lacks an embedded image: {name}"))
        })?;
        let png = embedded_image_bytes(image, views, binary)?;
        let decoded = image::load_from_memory(png).map_err(|error| {
            GeometryError::Invalid(format!("2K embedded PNG decode failed: {name}: {error}"))
        })?;
        let manifest_output = manifest_outputs.get(name).ok_or_else(|| {
            GeometryError::Invalid(format!(
                "2K embedded output is absent from manifest: {name}"
            ))
        })?;
        let actual_hash = hex_sha256(png);
        for key in ["semantic", "color_space", "normal_convention"] {
            if output.get(key) != manifest_output.get(key) {
                return Err(GeometryError::Invalid(format!(
                    "2K embedded output {key} drifted: {name}"
                )));
            }
        }
        if output.get("sha256").and_then(Value::as_str) != Some(actual_hash.as_str())
            || manifest_output.get("sha256").and_then(Value::as_str) != Some(actual_hash.as_str())
            || output.get("size_bytes").and_then(Value::as_u64) != Some(png.len() as u64)
            || output.get("width").and_then(Value::as_u64) != Some(2048)
            || output.get("height").and_then(Value::as_u64) != Some(2048)
            || output.get("mime").and_then(Value::as_str) != Some("image/png")
            || manifest_output.get("width").and_then(Value::as_u64) != Some(2048)
            || manifest_output.get("height").and_then(Value::as_u64) != Some(2048)
            || decoded.width() != 2048
            || decoded.height() != 2048
        {
            return Err(GeometryError::Invalid(format!(
                "2K embedded PNG physical bytes drifted: {name}"
            )));
        }
    }
    if let Some(surface_bake) = forgecad.get("surface_bake") {
        validate_candidate_surface_bake(surface_bake, &image_by_name, binary, views, &mut seen)?;
    }
    if seen.len() != images.len() || seen.len() != image_by_name.len() {
        return Err(GeometryError::Invalid(
            "2K embedded image inventory is not fully receipt-bound".to_owned(),
        ));
    }
    Ok(())
}

fn validate_candidate_surface_bake(
    surface_bake: &Value,
    images: &BTreeMap<String, &Value>,
    binary: &[u8],
    views: &[Value],
    seen: &mut BTreeSet<String>,
) -> Result<(), GeometryError> {
    let surface_bake = surface_bake
        .as_object()
        .ok_or_else(|| GeometryError::Invalid("candidate surface bake is invalid".to_owned()))?;
    if !has_exact_keys(
        surface_bake,
        &[
            "schema_version",
            "material_layer_stack_sha256",
            "algorithm",
            "worker_algorithm_sha256",
            "normal_bake_policy",
            "ao_bake_policy",
            "layer_lowering",
            "resolution",
            "padding_texels",
            "embedded_only",
            "external_uri",
            "network_at_runtime",
            "outputs",
            "total_output_bytes",
            "canonical_sha256",
        ],
    ) || surface_bake.get("schema_version").and_then(Value::as_str)
        != Some("CandidateSurfaceBake@1")
        || surface_bake.get("algorithm").and_then(Value::as_str)
            != Some("candidate-uv-tbn-normal-plus-fixed-self-occlusion-layer-lowering@1")
        || surface_bake
            .get("normal_bake_policy")
            .and_then(Value::as_str)
            != Some("evaluated-candidate-surface-tangent-field-not-high-low-cage@1")
        || surface_bake.get("ao_bake_policy").and_then(Value::as_str)
            != Some("fixed-8-ray-candidate-self-occlusion-not-screen-space@1")
        || surface_bake.get("resolution").and_then(Value::as_u64) != Some(2048)
        || surface_bake.get("padding_texels").and_then(Value::as_u64) != Some(8)
        || surface_bake.get("embedded_only").and_then(Value::as_bool) != Some(true)
        || surface_bake.get("external_uri").and_then(Value::as_bool) != Some(false)
        || surface_bake
            .get("network_at_runtime")
            .and_then(Value::as_bool)
            != Some(false)
    {
        return Err(GeometryError::Invalid(
            "candidate surface bake policy drifted".to_owned(),
        ));
    }
    let expected_algorithm_hash = hex_sha256(b"candidate-uv-tbn-normal@1|fixed-8-ray-self-occlusion@1|fictional-safety-decal@1|geometry-normal-ao-wear@1|zone-clearcoat@1|2048|8px|no-rng-no-time-no-network");
    if surface_bake
        .get("worker_algorithm_sha256")
        .and_then(Value::as_str)
        != Some(expected_algorithm_hash.as_str())
        || surface_bake
            .get("material_layer_stack_sha256")
            .and_then(Value::as_str)
            .is_none_or(|value| value.len() != 64)
        || surface_bake.get("layer_lowering")
            != Some(&json!([
                "decal-to-baseColor",
                "wear-to-baseColor-metallicRoughness",
                "clearcoat-to-KHR_materials_clearcoat"
            ]))
    {
        return Err(GeometryError::Invalid(
            "candidate surface bake algorithm or layer lowering drifted".to_owned(),
        ));
    }
    let canonical = surface_bake
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            GeometryError::Invalid("candidate surface bake hash is missing".to_owned())
        })?;
    let mut preimage = surface_bake.clone();
    preimage.remove("canonical_sha256");
    if canonical_json_hash(&Value::Object(preimage)) != canonical {
        return Err(GeometryError::Invalid(
            "candidate surface bake hash drifted".to_owned(),
        ));
    }
    let outputs = surface_bake
        .get("outputs")
        .and_then(Value::as_array)
        .filter(|outputs| outputs.len() == 6)
        .ok_or_else(|| {
            GeometryError::Invalid("candidate surface bake output count drifted".to_owned())
        })?;
    let expected = BTreeMap::from([
        (
            "forgecad_candidate_layered_base_color",
            ("layered-baseColor", "sRGB", None),
        ),
        (
            "forgecad_candidate_surface_normal",
            ("candidate-surface-normal", "linear", Some("OpenGL+Y")),
        ),
        (
            "forgecad_candidate_layered_metallic_roughness",
            ("layered-metallicRoughness", "linear", None),
        ),
        (
            "forgecad_candidate_surface_ao",
            ("candidate-self-occlusion", "linear", None),
        ),
        (
            "forgecad_candidate_zone_clearcoat",
            ("zone-clearcoat-factor", "linear", None),
        ),
        (
            "forgecad_candidate_zone_clearcoat_roughness",
            ("zone-clearcoat-roughness", "linear", None),
        ),
    ]);
    let mut total = 0u64;
    for output in outputs {
        let output = output.as_object().ok_or_else(|| {
            GeometryError::Invalid("candidate surface bake output is invalid".to_owned())
        })?;
        if !has_exact_keys(
            output,
            &[
                "texture_id",
                "sha256",
                "size_bytes",
                "width",
                "height",
                "mime",
                "semantic",
                "color_space",
                "normal_convention",
            ],
        ) {
            return Err(GeometryError::Invalid(
                "candidate surface bake output fields drifted".to_owned(),
            ));
        }
        let id = output
            .get("texture_id")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                GeometryError::Invalid("candidate surface output ID is missing".to_owned())
            })?;
        let (semantic, color_space, normal_convention) = expected.get(id).ok_or_else(|| {
            GeometryError::Invalid(format!("unknown candidate surface output: {id}"))
        })?;
        if !seen.insert(id.to_owned()) {
            return Err(GeometryError::Invalid(
                "candidate surface output duplicates an embedded image".to_owned(),
            ));
        }
        let image = images.get(id).ok_or_else(|| {
            GeometryError::Invalid(format!("candidate surface output lacks an image: {id}"))
        })?;
        let png = embedded_image_bytes(image, views, binary)?;
        let decoded = image::load_from_memory(png).map_err(|error| {
            GeometryError::Invalid(format!(
                "candidate surface PNG decode failed: {id}: {error}"
            ))
        })?;
        let size = png.len() as u64;
        total += size;
        if output.get("semantic").and_then(Value::as_str) != Some(*semantic)
            || output.get("color_space").and_then(Value::as_str) != Some(*color_space)
            || output.get("normal_convention").and_then(Value::as_str) != *normal_convention
            || output.get("sha256").and_then(Value::as_str) != Some(hex_sha256(png).as_str())
            || output.get("size_bytes").and_then(Value::as_u64) != Some(size)
            || output.get("width").and_then(Value::as_u64) != Some(2048)
            || output.get("height").and_then(Value::as_u64) != Some(2048)
            || output.get("mime").and_then(Value::as_str) != Some("image/png")
            || decoded.width() != 2048
            || decoded.height() != 2048
        {
            return Err(GeometryError::Invalid(format!(
                "candidate surface output physical bytes drifted: {id}"
            )));
        }
    }
    if surface_bake
        .get("total_output_bytes")
        .and_then(Value::as_u64)
        != Some(total)
        || total > 67_108_864
    {
        return Err(GeometryError::Invalid(
            "candidate surface output byte budget drifted".to_owned(),
        ));
    }
    Ok(())
}

/// Every V2 accessor and buffer view has exactly one static use.  Together
/// with contiguous, exact byte ranges this makes the BIN payload a closed
/// input to the readback, rather than an opaque carrier for renderer-only
/// geometry or deformations.
fn enforce_canonical_v2_static_bin_layout(
    root: &Value,
    binary: &[u8],
    accessors: &[Value],
    views: &[Value],
    accessor_uses: &BTreeMap<usize, V2AccessorRole>,
    metrics: &mut Metrics,
) {
    let Some(buffers) = root.get("buffers").and_then(Value::as_array) else {
        metrics.metadata_mismatch_count += 1;
        return;
    };
    if buffers.len() != 1 {
        metrics.metadata_mismatch_count += 1;
    }
    let Some(buffer) = buffers.first().and_then(Value::as_object) else {
        metrics.metadata_mismatch_count += 1;
        return;
    };
    if !has_exact_keys(buffer, &["byteLength"])
        || buffer.get("byteLength").and_then(Value::as_u64) != Some(binary.len() as u64)
    {
        metrics.metadata_mismatch_count += 1;
    }

    if accessor_uses.len() != accessors.len() {
        metrics.metadata_mismatch_count += 1;
    }
    let mut view_uses = BTreeMap::<usize, V2AccessorRole>::new();
    let image_view_indices = root
        .get("images")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|image| image.get("bufferView").and_then(Value::as_u64))
        .filter_map(|index| usize::try_from(index).ok())
        .collect::<BTreeSet<_>>();
    for (accessor_index, role) in accessor_uses {
        let Some(accessor) = accessors.get(*accessor_index).and_then(Value::as_object) else {
            metrics.metadata_mismatch_count += 1;
            continue;
        };
        let (expected_type, expected_component, element_size, _) = role.layout();
        if !has_only_keys(
            accessor,
            &["bufferView", "componentType", "count", "type", "min", "max"],
        ) || accessor.get("type").and_then(Value::as_str) != Some(expected_type)
            || accessor.get("componentType").and_then(Value::as_u64) != Some(expected_component)
        {
            metrics.metadata_mismatch_count += 1;
        }
        let Some(count) = accessor
            .get("count")
            .and_then(Value::as_u64)
            .and_then(|value| usize::try_from(value).ok())
        else {
            metrics.metadata_mismatch_count += 1;
            continue;
        };
        let Some(view_index) = accessor
            .get("bufferView")
            .and_then(Value::as_u64)
            .and_then(|value| usize::try_from(value).ok())
        else {
            metrics.metadata_mismatch_count += 1;
            continue;
        };
        if count == 0 || count.checked_mul(element_size).is_none() {
            metrics.metadata_mismatch_count += 1;
        }
        if view_uses.insert(view_index, *role).is_some() {
            metrics.metadata_mismatch_count += 1;
        }
    }

    if view_uses.len() + image_view_indices.len() != views.len() {
        metrics.metadata_mismatch_count += 1;
    }
    let mut expected_offset = 0usize;
    for (view_index, view) in views.iter().enumerate() {
        let Some(view) = view.as_object() else {
            metrics.metadata_mismatch_count += 1;
            continue;
        };
        let image_view = image_view_indices.contains(&view_index);
        let role = view_uses.get(&view_index).copied();
        if image_view {
            if role.is_some()
                || !has_exact_keys(view, &["buffer", "byteOffset", "byteLength"])
                || view.get("buffer").and_then(Value::as_u64) != Some(0)
            {
                metrics.metadata_mismatch_count += 1;
            }
        } else {
            let Some(role) = role else {
                metrics.metadata_mismatch_count += 1;
                continue;
            };
            let (_, _, _, expected_target) = role.layout();
            if !has_exact_keys(view, &["buffer", "byteOffset", "byteLength", "target"])
                || view.get("buffer").and_then(Value::as_u64) != Some(0)
                || view.get("target").and_then(Value::as_u64) != Some(expected_target)
            {
                metrics.metadata_mismatch_count += 1;
            }
        }
        let Some(offset) = view
            .get("byteOffset")
            .and_then(Value::as_u64)
            .and_then(|value| usize::try_from(value).ok())
        else {
            metrics.metadata_mismatch_count += 1;
            continue;
        };
        let Some(length) = view
            .get("byteLength")
            .and_then(Value::as_u64)
            .and_then(|value| usize::try_from(value).ok())
        else {
            metrics.metadata_mismatch_count += 1;
            continue;
        };
        if offset != expected_offset || length == 0 {
            metrics.metadata_mismatch_count += 1;
        }
        let Some(next_offset) = offset.checked_add(length) else {
            metrics.metadata_mismatch_count += 1;
            continue;
        };
        if next_offset > binary.len() {
            metrics.metadata_mismatch_count += 1;
        }
        expected_offset = next_offset;
    }
    if expected_offset != binary.len() {
        metrics.metadata_mismatch_count += 1;
    }

    for (accessor_index, role) in accessor_uses {
        let Some(accessor) = accessors.get(*accessor_index).and_then(Value::as_object) else {
            continue;
        };
        let Some(view_index) = accessor
            .get("bufferView")
            .and_then(Value::as_u64)
            .and_then(|value| usize::try_from(value).ok())
        else {
            continue;
        };
        let Some(view_length) = views
            .get(view_index)
            .and_then(Value::as_object)
            .and_then(|view| view.get("byteLength"))
            .and_then(Value::as_u64)
            .and_then(|value| usize::try_from(value).ok())
        else {
            continue;
        };
        let (_, _, element_size, _) = role.layout();
        let expected_length = accessor
            .get("count")
            .and_then(Value::as_u64)
            .and_then(|value| usize::try_from(value).ok())
            .and_then(|count| count.checked_mul(element_size));
        if expected_length != Some(view_length) {
            metrics.metadata_mismatch_count += 1;
        }
    }
}

fn record_v2_accessor_use(
    accessor_uses: &mut BTreeMap<usize, V2AccessorRole>,
    accessor_index: usize,
    role: V2AccessorRole,
    metrics: &mut Metrics,
) {
    if accessor_uses.insert(accessor_index, role).is_some() {
        metrics.metadata_mismatch_count += 1;
    }
}

fn has_only_keys(object: &Map<String, Value>, allowed: &[&str]) -> bool {
    object.keys().all(|key| allowed.contains(&key.as_str()))
}

fn has_exact_keys(object: &Map<String, Value>, expected: &[&str]) -> bool {
    object.len() == expected.len() && has_only_keys(object, expected)
}

/// V2 is deliberately not a general glTF scene admission path.  The current
/// product writer has a single, flat scene whose roots enumerate every mesh
/// exactly once.  Check that representation before any metadata is merged:
/// otherwise an unreferenced or duplicate instance could be hidden by the
/// first matching node with valid extras.
fn enforce_canonical_v2_scene_graph(
    root: &Value,
    meshes: &[Value],
    nodes: &[Value],
    metrics: &mut Metrics,
) {
    let Some(scenes) = root.get("scenes").and_then(Value::as_array) else {
        metrics.metadata_mismatch_count += 1;
        return;
    };
    if scenes.len() != 1 || root.get("scene").and_then(Value::as_u64) != Some(0) {
        metrics.metadata_mismatch_count += 1;
    }
    let Some(scene) = scenes.first().and_then(Value::as_object) else {
        metrics.metadata_mismatch_count += 1;
        return;
    };
    let Some(scene_nodes) = scene.get("nodes").and_then(Value::as_array) else {
        metrics.metadata_mismatch_count += 1;
        return;
    };
    if scene_nodes.len() != nodes.len() {
        metrics.metadata_mismatch_count += 1;
    }

    let mut scene_references = vec![false; nodes.len()];
    for (expected_node_index, scene_node) in scene_nodes.iter().enumerate() {
        let Some(node_index) = scene_node
            .as_u64()
            .and_then(|value| usize::try_from(value).ok())
        else {
            metrics.metadata_mismatch_count += 1;
            continue;
        };
        if node_index >= nodes.len() {
            metrics.metadata_mismatch_count += 1;
            continue;
        }
        // The writer's deterministic graph is ordered by mesh/node index.
        if node_index != expected_node_index || scene_references[node_index] {
            metrics.metadata_mismatch_count += 1;
        }
        scene_references[node_index] = true;
    }

    let mut mesh_instance_counts = vec![0usize; meshes.len()];
    for (node_index, node) in nodes.iter().enumerate() {
        let Some(node) = node.as_object() else {
            metrics.metadata_mismatch_count += 1;
            continue;
        };
        if !scene_references.get(node_index).copied().unwrap_or(false) {
            metrics.metadata_mismatch_count += 1;
        }
        if !node_has_canonical_v2_shape(node) || !node_has_identity_transform(node) {
            metrics.metadata_mismatch_count += 1;
        }
        let Some(mesh_index) = node
            .get("mesh")
            .and_then(Value::as_u64)
            .and_then(|value| usize::try_from(value).ok())
        else {
            metrics.metadata_mismatch_count += 1;
            continue;
        };
        let Some(mesh) = meshes.get(mesh_index).and_then(Value::as_object) else {
            metrics.metadata_mismatch_count += 1;
            continue;
        };
        if mesh_index != node_index {
            metrics.metadata_mismatch_count += 1;
        }
        mesh_instance_counts[mesh_index] += 1;

        let node_lineage = complete_part_lineage(node.get("extras").and_then(Value::as_object));
        let mesh_lineage = complete_part_lineage(mesh.get("extras").and_then(Value::as_object));
        if node_lineage.is_none() || node_lineage != mesh_lineage {
            metrics.metadata_mismatch_count += 1;
        }
    }
    if mesh_instance_counts.iter().any(|count| *count != 1) {
        metrics.metadata_mismatch_count += 1;
    }
}

fn node_has_canonical_v2_shape(node: &Map<String, Value>) -> bool {
    node.keys().all(|key| {
        matches!(
            key.as_str(),
            "name" | "mesh" | "extras" | "translation" | "rotation" | "scale" | "matrix"
        )
    })
}

fn node_has_identity_transform(node: &Map<String, Value>) -> bool {
    let has_matrix = node.contains_key("matrix");
    let has_trs = node.contains_key("translation")
        || node.contains_key("rotation")
        || node.contains_key("scale");
    if has_matrix && has_trs {
        return false;
    }
    optional_identity_vector(node, "translation", &[0.0, 0.0, 0.0])
        && optional_identity_rotation(node)
        && optional_identity_vector(node, "scale", &[1.0, 1.0, 1.0])
        && optional_identity_vector(
            node,
            "matrix",
            &[
                1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
            ],
        )
}

fn optional_identity_vector(node: &Map<String, Value>, key: &str, expected: &[f64]) -> bool {
    let Some(value) = node.get(key) else {
        return true;
    };
    let Some(values) = value.as_array() else {
        return false;
    };
    values.len() == expected.len()
        && values
            .iter()
            .zip(expected)
            .all(|(value, expected)| value.as_f64() == Some(*expected))
}

fn optional_identity_rotation(node: &Map<String, Value>) -> bool {
    let Some(value) = node.get("rotation") else {
        return true;
    };
    let Some(values) = value.as_array() else {
        return false;
    };
    values.len() == 4
        && values[0].as_f64() == Some(0.0)
        && values[1].as_f64() == Some(0.0)
        && values[2].as_f64() == Some(0.0)
        && values[3]
            .as_f64()
            .is_some_and(|value| value == 1.0 || value == -1.0)
}

fn complete_part_lineage(extras: Option<&Map<String, Value>>) -> Option<PartLineage> {
    let extras = extras?;
    Some(PartLineage {
        part_id: extras
            .get("part_id")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())?
            .to_owned(),
        material_zone_id: extras
            .get("material_zone_id")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())?
            .to_owned(),
        solid: extras.get("solid").and_then(Value::as_bool)?,
    })
}

fn matching_node_lineages(nodes: &[Value], mesh_index: usize) -> Vec<Map<String, Value>> {
    nodes
        .iter()
        .filter(|node| node.get("mesh").and_then(Value::as_u64) == Some(mesh_index as u64))
        .filter_map(|node| node.get("extras").and_then(Value::as_object).cloned())
        .collect()
}

fn merge_lineage(
    mesh: Option<&Map<String, Value>>,
    node: Option<&Map<String, Value>>,
    primitive: Option<&Map<String, Value>>,
    metrics: &mut Metrics,
) -> Lineage {
    let mut lineage = Lineage::default();
    for (key, slot) in [
        ("part_id", &mut lineage.part_id),
        ("source_node_id", &mut lineage.source_node_id),
        ("material_zone_id", &mut lineage.material_zone_id),
    ] {
        let values = [mesh, node, primitive].map(|value| {
            value
                .and_then(|value| value.get(key))
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
        });
        let selected = values.into_iter().flatten().next();
        if let Some(selected) = selected {
            if values.into_iter().flatten().any(|value| value != selected) {
                metrics.metadata_mismatch_count += 1;
            }
        }
        *slot = selected.map(str::to_owned);
    }
    let solid_values = [mesh, node, primitive].map(|value| {
        value
            .and_then(|value| value.get("solid"))
            .and_then(Value::as_bool)
    });
    let selected_solid = solid_values.into_iter().flatten().next();
    if let Some(selected_solid) = selected_solid {
        if solid_values
            .into_iter()
            .flatten()
            .any(|value| value != selected_solid)
        {
            metrics.metadata_mismatch_count += 1;
        }
    }
    lineage.solid = selected_solid.unwrap_or(false);
    lineage
}

fn compare_declared_metadata(
    forgecad: &Map<String, Value>,
    bindings: &[PartBinding],
    triangle_count: u64,
    metrics: &mut Metrics,
) {
    let declared_triangles = forgecad.get("triangle_count").and_then(Value::as_u64);
    if declared_triangles != Some(triangle_count) {
        metrics.metadata_mismatch_count += 1;
    }
    if forgecad.get("schema_version").and_then(Value::as_str) == Some("ArtifactReadback@2") {
        let Some(declared) = forgecad.get("part_bindings").and_then(Value::as_array) else {
            metrics.metadata_mismatch_count += 1;
            return;
        };
        let values = declared
            .iter()
            .filter_map(|value| {
                let value = value.as_object()?;
                Some(PartBinding {
                    part_id: value.get("part_id")?.as_str()?.to_owned(),
                    source_node_id: value.get("source_node_id")?.as_str()?.to_owned(),
                    material_zone_id: value.get("material_zone_id")?.as_str()?.to_owned(),
                    solid: value.get("solid")?.as_bool()?,
                    triangle_count: value.get("triangle_count")?.as_u64()?,
                })
            })
            .collect::<Vec<_>>();
        if values.len() != declared.len() {
            metrics.metadata_mismatch_count += 1;
            return;
        }
        if values != bindings {
            metrics.metadata_mismatch_count += 1;
        }
        let mut source_node_ids = BTreeMap::<String, ()>::new();
        for binding in &values {
            if source_node_ids
                .insert(binding.source_node_id.clone(), ())
                .is_some()
            {
                metrics.metadata_mismatch_count += 1;
                break;
            }
        }
        compare_declared_identifier_array(
            forgecad,
            "part_ids",
            unique_strings_in_order(bindings.iter().map(|binding| &binding.part_id)),
            metrics,
        );
        compare_declared_identifier_array(
            forgecad,
            "source_node_ids",
            unique_strings_in_order(bindings.iter().map(|binding| &binding.source_node_id)),
            metrics,
        );
        compare_declared_identifier_array(
            forgecad,
            "material_zone_ids",
            unique_strings_in_order(bindings.iter().map(|binding| &binding.material_zone_id)),
            metrics,
        );
    }
}

fn compare_declared_identifier_array(
    forgecad: &Map<String, Value>,
    key: &str,
    expected: Vec<String>,
    metrics: &mut Metrics,
) {
    let Some(declared) = forgecad.get(key).and_then(Value::as_array) else {
        metrics.metadata_mismatch_count += 1;
        return;
    };
    let values = declared
        .iter()
        .map(|value| value.as_str().map(str::to_owned))
        .collect::<Option<Vec<_>>>();
    if values.as_deref() != Some(expected.as_slice()) {
        metrics.metadata_mismatch_count += 1;
    }
}

fn unique_strings_in_order<'a>(values: impl Iterator<Item = &'a String>) -> Vec<String> {
    let mut seen = BTreeMap::<String, ()>::new();
    values
        .filter_map(|value| {
            seen.insert(value.clone(), ())
                .is_none()
                .then(|| value.clone())
        })
        .collect()
}

fn failure_codes(
    metrics: &Metrics,
    triangle_count: u64,
    part_coverage: f64,
    source_coverage: f64,
    material_zone_coverage: f64,
    artifact_schema_version: &str,
    operator_catalog_sha256: Option<&str>,
) -> Vec<&'static str> {
    let mut failures = Vec::new();
    if artifact_schema_version != "ArtifactReadback@2" {
        failures.push("LEGACY_ARTIFACT_SCHEMA");
    }
    if operator_catalog_sha256
        .filter(|value| is_sha256(value))
        .is_none()
    {
        failures.push("OPERATOR_CATALOG_BINDING_MISSING");
    }
    if triangle_count == 0 {
        failures.push("EMPTY_TRIANGLES");
    }
    for (count, code) in [
        (metrics.invalid_index_count, "INVALID_INDEX"),
        (metrics.non_finite_count, "NON_FINITE"),
        (metrics.degenerate_triangle_count, "DEGENERATE_TRIANGLE"),
        (metrics.boundary_edge_count, "BOUNDARY_EDGE"),
        (metrics.non_manifold_edge_count, "NON_MANIFOLD_EDGE"),
        (metrics.winding_error_count, "WINDING"),
        (metrics.uv_non_finite_count, "UV_NON_FINITE"),
        (metrics.zero_area_uv_triangle_count, "UV_ZERO_AREA"),
        (metrics.tangent_non_finite_count, "TANGENT_NON_FINITE"),
        (
            metrics.tangent_orthogonality_error_count,
            "TANGENT_ORTHOGONALITY",
        ),
        (metrics.tangent_handedness_error_count, "TANGENT_HANDEDNESS"),
        (metrics.external_uri_count, "EXTERNAL_URI"),
        (metrics.metadata_mismatch_count, "METADATA_MISMATCH"),
        (metrics.lineage_missing_triangle_count, "LINEAGE_COVERAGE"),
    ] {
        if count > 0 {
            failures.push(code);
        }
    }
    if (part_coverage - 1.0).abs() > f64::EPSILON {
        failures.push("PART_LINEAGE_COVERAGE");
    }
    if (source_coverage - 1.0).abs() > f64::EPSILON {
        failures.push("SOURCE_LINEAGE_COVERAGE");
    }
    if (material_zone_coverage - 1.0).abs() > f64::EPSILON {
        failures.push("MATERIAL_ZONE_LINEAGE_COVERAGE");
    }
    failures
}

fn parse_glb(glb: &[u8]) -> Result<(Value, &[u8]), GeometryError> {
    if glb.len() < 20 || glb.len() > MAX_GLB_BYTES || &glb[..4] != b"glTF" {
        return Err(GeometryError::Invalid("GLB header is invalid".to_owned()));
    }
    if read_u32(glb, 4)? != 2 || read_u32(glb, 8)? as usize != glb.len() {
        return Err(GeometryError::Invalid(
            "GLB version or length is invalid".to_owned(),
        ));
    }
    let json_length = read_u32(glb, 12)? as usize;
    let json_end = 20usize
        .checked_add(json_length)
        .ok_or_else(|| GeometryError::Invalid("GLB JSON length overflows".to_owned()))?;
    if json_end
        .checked_add(8)
        .is_none_or(|value| value > glb.len())
        || &glb[16..20] != b"JSON"
    {
        return Err(GeometryError::Invalid(
            "GLB JSON chunk is invalid".to_owned(),
        ));
    }
    let root: Value = serde_json::from_slice(&glb[20..json_end])
        .map_err(|error| GeometryError::Invalid(format!("GLB JSON decode failed: {error}")))?;
    let binary_length = read_u32(glb, json_end)? as usize;
    let binary_kind = json_end + 4;
    let binary_offset = json_end + 8;
    let binary_end = binary_offset
        .checked_add(binary_length)
        .ok_or_else(|| GeometryError::Invalid("GLB BIN length overflows".to_owned()))?;
    if binary_end != glb.len() || &glb[binary_kind..binary_offset] != b"BIN\0" {
        return Err(GeometryError::Invalid(
            "GLB BIN chunk is invalid".to_owned(),
        ));
    }
    Ok((root, &glb[binary_offset..binary_end]))
}

/// A bounded, read-only triangle view used by the Runtime Visual Surface
/// projection.  The normal is derived from the decoded GLB positions rather
/// than trusted from metadata or the renderer.  This intentionally exposes no
/// arbitrary glTF scene state and is only callable after the strict artifact
/// profile has been admitted by the caller.
#[derive(Debug, Clone)]
pub struct SurfaceTriangle {
    pub positions: [[f32; 3]; 3],
    pub normal: [f32; 3],
    pub part_id: String,
}

#[derive(Debug, Clone)]
pub struct SurfaceMesh {
    pub triangles: Vec<SurfaceTriangle>,
    pub vertex_count: usize,
}

/// One decoded corner from the product-owned static GLB profile.  A corner
/// intentionally retains its shading attributes even when its position welds
/// to another corner, so UV seams and split normals are not erased by the
/// topological projection.
#[derive(Debug, Clone)]
pub struct TopologyCornerSource {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub texcoord_0: [f32; 2],
    pub tangent: [f32; 4],
}

#[derive(Debug, Clone)]
pub struct TopologyTriangleSource {
    pub part_id: String,
    pub corners: [TopologyCornerSource; 3],
    pub source_node_id: String,
    pub material_zone_id: String,
    pub solid: bool,
}

#[derive(Debug, Clone)]
pub struct TopologyPartMesh {
    pub triangles: Vec<TopologyTriangleSource>,
}

/// Ordered, read-only topology used by the independent geometric bake Worker
/// operation.  Unlike `TopologyPartMesh`, this view is not capped to one
/// Runtime projection Part; it preserves the semantic Part on every triangle
/// so the bake can build a deterministic low/cage correspondence and filter
/// high-ray queries by Part without consulting Runtime/Store state.
#[derive(Debug, Clone)]
pub struct TopologyMesh {
    pub triangles: Vec<TopologyTriangleSource>,
}

/// Ordered primitive-level geometry used by the independent High/Low/Cage
/// diagnostic.  This view is intentionally smaller than the surface/appearance
/// paths: it retains only decoded positions, indices and semantic lineage.  It
/// is not an authoring mesh, a persistent element identity projection, or a
/// surface-bake input.
#[derive(Debug, Clone)]
pub struct DiagnosticPrimitive {
    pub part_id: String,
    pub source_node_id: String,
    pub material_zone_id: String,
    pub solid: bool,
    pub positions: Vec<[f32; 3]>,
    pub indices: Vec<u32>,
}

#[derive(Debug, Clone)]
pub struct DiagnosticMesh {
    pub primitives: Vec<DiagnosticPrimitive>,
    pub triangle_count: usize,
}

/// Decode a bounded, ordered primitive list for the independent High/Low/Cage
/// diagnostic.  The caller must separately admit the GLB through
/// `inspect_glb`; this helper still rechecks the payload it consumes so a
/// malformed accessor cannot become a partial correspondence result.
pub fn extract_diagnostic_mesh(
    glb: &[u8],
    max_triangle_count: usize,
) -> Result<DiagnosticMesh, GeometryError> {
    if max_triangle_count == 0 {
        return Err(GeometryError::Invalid(
            "high/low/cage diagnostic triangle budget is empty".to_owned(),
        ));
    }
    let max_vertex_count = max_triangle_count.checked_mul(3).ok_or_else(|| {
        GeometryError::Invalid("high/low/cage diagnostic vertex budget overflows".to_owned())
    })?;
    let (root, binary) = parse_glb(glb)?;
    let meshes = required_array(&root, "meshes")?;
    let nodes = required_array(&root, "nodes")?;
    let accessors = required_array(&root, "accessors")?;
    let views = required_array(&root, "bufferViews")?;
    let mut primitives = Vec::new();
    let mut triangle_count = 0usize;
    let mut lineage_metrics = Metrics::default();

    for (mesh_index, mesh) in meshes.iter().enumerate() {
        let mesh_object = mesh.as_object().ok_or_else(|| {
            GeometryError::Invalid("diagnostic GLB mesh is not an object".to_owned())
        })?;
        let mesh_lineage = mesh_object.get("extras").and_then(Value::as_object);
        let node_lineages = matching_node_lineages(nodes, mesh_index);
        if node_lineages.len() != 1 {
            return Err(GeometryError::Invalid(
                "diagnostic GLB mesh must have exactly one lineage node".to_owned(),
            ));
        }
        let primitives_value = mesh_object
            .get("primitives")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                GeometryError::Invalid("diagnostic GLB primitive list is missing".to_owned())
            })?;
        if primitives_value.is_empty() {
            return Err(GeometryError::Invalid(
                "diagnostic GLB mesh has no primitives".to_owned(),
            ));
        }
        for primitive in primitives_value {
            let primitive_object = primitive.as_object().ok_or_else(|| {
                GeometryError::Invalid("diagnostic GLB primitive is not an object".to_owned())
            })?;
            let primitive_lineage = primitive_object.get("extras").and_then(Value::as_object);
            let lineage = merge_lineage(
                mesh_lineage,
                node_lineages.first(),
                primitive_lineage,
                &mut lineage_metrics,
            )
            .complete()
            .ok_or_else(|| {
                GeometryError::Invalid("diagnostic GLB primitive lineage is incomplete".to_owned())
            })?;
            let attributes = primitive_object
                .get("attributes")
                .and_then(Value::as_object)
                .ok_or_else(|| {
                    GeometryError::Invalid(
                        "diagnostic GLB primitive attributes are missing".to_owned(),
                    )
                })?;
            let position_accessor = required_index(attributes, "POSITION")?;
            let index_accessor = primitive_object
                .get("indices")
                .and_then(Value::as_u64)
                .and_then(|value| usize::try_from(value).ok())
                .ok_or_else(|| {
                    GeometryError::Invalid("diagnostic GLB index accessor is missing".to_owned())
                })?;
            let positions = read_vec3(&accessors, &views, binary, position_accessor)?;
            let indices = read_indices(&accessors, &views, binary, index_accessor)?;
            if positions.is_empty() || positions.len() > max_vertex_count {
                return Err(GeometryError::Invalid(
                    "diagnostic GLB primitive exceeds its bounded vertex budget".to_owned(),
                ));
            }
            if indices.is_empty() || indices.len() % 3 != 0 {
                return Err(GeometryError::Invalid(
                    "diagnostic GLB primitive has an invalid triangle payload".to_owned(),
                ));
            }
            let primitive_triangles = indices.len() / 3;
            triangle_count = triangle_count
                .checked_add(primitive_triangles)
                .ok_or_else(|| {
                    GeometryError::Invalid("diagnostic GLB triangle count overflows".to_owned())
                })?;
            if triangle_count > max_triangle_count {
                return Err(GeometryError::Invalid(
                    "diagnostic GLB exceeds its bounded triangle budget".to_owned(),
                ));
            }
            if positions.iter().any(|position| !finite3(*position))
                || indices
                    .iter()
                    .any(|index| *index as usize >= positions.len())
            {
                return Err(GeometryError::Invalid(
                    "diagnostic GLB contains a non-finite or out-of-range vertex".to_owned(),
                ));
            }
            for triangle in indices.chunks_exact(3) {
                let a = positions[triangle[0] as usize];
                let b = positions[triangle[1] as usize];
                let c = positions[triangle[2] as usize];
                let area = length3(cross3(sub3(b, a), sub3(c, a)));
                if !area.is_finite() || area <= DEGENERATE_AREA_EPSILON {
                    return Err(GeometryError::Invalid(
                        "diagnostic GLB contains a degenerate triangle".to_owned(),
                    ));
                }
            }
            primitives.push(DiagnosticPrimitive {
                part_id: lineage.part_id,
                source_node_id: lineage.source_node_id,
                material_zone_id: lineage.material_zone_id,
                solid: lineage.solid,
                positions,
                indices,
            });
        }
    }
    if primitives.is_empty() || triangle_count == 0 {
        return Err(GeometryError::Invalid(
            "diagnostic GLB contains no analyzable primitives".to_owned(),
        ));
    }
    if lineage_metrics.metadata_mismatch_count != 0 {
        return Err(GeometryError::Invalid(
            "diagnostic GLB lineage metadata conflicts".to_owned(),
        ));
    }
    Ok(DiagnosticMesh {
        primitives,
        triangle_count,
    })
}

/// Decode one bounded Part for the Runtime-owned `TopologySnapshot@1`
/// projection.  This is not an authoring-mesh importer: it exposes the
/// triangulated artifact after strict `ArtifactReadback@2` admission and
/// preserves loop/corner attributes separately from position welding.
pub fn extract_topology_part_mesh(
    glb: &[u8],
    requested_part_id: &str,
    max_face_count: usize,
) -> Result<TopologyPartMesh, GeometryError> {
    const MAX_TOPOLOGY_FACES: usize = 512;
    if requested_part_id.is_empty() || max_face_count == 0 || max_face_count > MAX_TOPOLOGY_FACES {
        return Err(GeometryError::Invalid(
            "topology snapshot request exceeds the bounded face budget".to_owned(),
        ));
    }

    let mesh =
        extract_topology_mesh_bounded(glb, max_face_count, Some(requested_part_id), "topology")?;
    Ok(TopologyPartMesh {
        triangles: mesh.triangles,
    })
}

/// Decode all static triangles needed by the Worker-only geometric bake.  The
/// same strict accessor/lineage/finite/degenerate checks as the Runtime
/// topology projection are retained, but the explicit budget is independent
/// of the Runtime's 512-face-per-Part projection cap.
pub fn extract_topology_mesh(
    glb: &[u8],
    max_face_count: usize,
) -> Result<TopologyMesh, GeometryError> {
    const MAX_BAKE_FACES: usize = 250_000;
    if max_face_count == 0 || max_face_count > MAX_BAKE_FACES {
        return Err(GeometryError::Invalid(
            "geometric bake topology exceeds the bounded face budget".to_owned(),
        ));
    }
    extract_topology_mesh_bounded(glb, max_face_count, None, "bake")
}

fn extract_topology_mesh_bounded(
    glb: &[u8],
    max_face_count: usize,
    requested_part_id: Option<&str>,
    label: &str,
) -> Result<TopologyMesh, GeometryError> {
    let (root, binary) = parse_glb(glb)?;
    let meshes = required_array(&root, "meshes")?;
    let nodes = required_array(&root, "nodes")?;
    let accessors = required_array(&root, "accessors")?;
    let views = required_array(&root, "bufferViews")?;
    let mut triangles = Vec::new();
    let mut lineage_metrics = Metrics::default();

    for (mesh_index, mesh) in meshes.iter().enumerate() {
        let mesh_object = mesh.as_object().ok_or_else(|| {
            GeometryError::Invalid("topology GLB mesh is not an object".to_owned())
        })?;
        let mesh_lineage = mesh_object.get("extras").and_then(Value::as_object);
        let node_lineages = matching_node_lineages(nodes, mesh_index);
        let node_lineage = node_lineages.first();
        let primitives = mesh_object
            .get("primitives")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                GeometryError::Invalid("topology GLB primitive list is missing".to_owned())
            })?;
        for primitive in primitives {
            let primitive_object = primitive.as_object().ok_or_else(|| {
                GeometryError::Invalid("topology GLB primitive is not an object".to_owned())
            })?;
            let primitive_lineage = primitive_object.get("extras").and_then(Value::as_object);
            let lineage = merge_lineage(
                mesh_lineage,
                node_lineage,
                primitive_lineage,
                &mut lineage_metrics,
            );
            let Some(lineage) = lineage.complete() else {
                return Err(GeometryError::Invalid(format!(
                    "{label} GLB lineage is incomplete"
                )));
            };
            if requested_part_id.is_some_and(|part_id| lineage.part_id != part_id) {
                continue;
            }
            let attributes = primitive_object
                .get("attributes")
                .and_then(Value::as_object)
                .ok_or_else(|| {
                    GeometryError::Invalid("topology GLB attributes are missing".to_owned())
                })?;
            let positions = read_vec3(
                &accessors,
                &views,
                binary,
                required_index(attributes, "POSITION")?,
            )?;
            let normals = read_vec3(
                &accessors,
                &views,
                binary,
                required_index(attributes, "NORMAL")?,
            )?;
            let texcoords = read_vec2(
                &accessors,
                &views,
                binary,
                required_index(attributes, "TEXCOORD_0")?,
            )?;
            let tangents = read_vec4(
                &accessors,
                &views,
                binary,
                required_index(attributes, "TANGENT")?,
            )?;
            if positions.is_empty()
                || normals.len() != positions.len()
                || texcoords.len() != positions.len()
                || tangents.len() != positions.len()
            {
                return Err(GeometryError::Invalid(
                    "topology GLB corner attribute counts differ".to_owned(),
                ));
            }
            let index_accessor = primitive_object
                .get("indices")
                .and_then(Value::as_u64)
                .and_then(|value| usize::try_from(value).ok())
                .ok_or_else(|| {
                    GeometryError::Invalid("topology GLB index accessor is missing".to_owned())
                })?;
            let indices = read_indices(&accessors, &views, binary, index_accessor)?;
            if indices.is_empty() || indices.len() % 3 != 0 {
                return Err(GeometryError::Invalid(
                    "topology GLB has an invalid triangle payload".to_owned(),
                ));
            }
            for triangle in indices.chunks_exact(3) {
                if triangles.len() >= max_face_count {
                    return Err(GeometryError::Invalid(if requested_part_id.is_some() {
                        "TOPOLOGY_SNAPSHOT_BUDGET_EXCEEDED: Part face count exceeds max_face_count"
                            .to_owned()
                    } else {
                        "GEOMETRIC_BAKE_TOPOLOGY_BUDGET_EXCEEDED: face count exceeds max_face_count"
                            .to_owned()
                    }));
                }
                let indices = [
                    triangle[0] as usize,
                    triangle[1] as usize,
                    triangle[2] as usize,
                ];
                if indices.iter().any(|index| *index >= positions.len()) {
                    return Err(GeometryError::Invalid(
                        "topology GLB triangle index is out of bounds".to_owned(),
                    ));
                }
                let corners = indices.map(|index| TopologyCornerSource {
                    position: positions[index],
                    normal: normals[index],
                    texcoord_0: texcoords[index],
                    tangent: tangents[index],
                });
                if corners.iter().any(|corner| {
                    !finite3(corner.position)
                        || !finite3(corner.normal)
                        || !finite2(corner.texcoord_0)
                        || !finite4(corner.tangent)
                }) {
                    return Err(GeometryError::Invalid(
                        "topology GLB contains non-finite corner data".to_owned(),
                    ));
                }
                let face = cross3(
                    sub3(corners[1].position, corners[0].position),
                    sub3(corners[2].position, corners[0].position),
                );
                if !length3(face).is_finite() || length3(face) <= DEGENERATE_AREA_EPSILON {
                    return Err(GeometryError::Invalid(
                        "topology GLB contains a degenerate triangle".to_owned(),
                    ));
                }
                triangles.push(TopologyTriangleSource {
                    part_id: lineage.part_id.clone(),
                    corners,
                    source_node_id: lineage.source_node_id.clone(),
                    material_zone_id: lineage.material_zone_id.clone(),
                    solid: lineage.solid,
                });
            }
        }
    }
    if triangles.is_empty() {
        return Err(GeometryError::Invalid(if requested_part_id.is_some() {
            "TOPOLOGY_PART_NOT_FOUND: requested Part has no triangles".to_owned()
        } else {
            "geometric bake topology contains no analyzable triangles".to_owned()
        }));
    }
    if lineage_metrics.metadata_mismatch_count != 0 {
        return Err(GeometryError::Invalid(
            "topology GLB lineage metadata conflicts".to_owned(),
        ));
    }
    Ok(TopologyMesh { triangles })
}

/// Decode the product-owned static mesh needed for bounded surface signals.
///
/// This is deliberately not a general glTF importer: the Runtime calls it only
/// after `ArtifactReadback@2` has passed.  It reads positions, indices and
/// semantic Part lineage from the same GLB BIN, caps the triangle count, and
/// rejects malformed or degenerate payloads instead of returning a partial
/// surface analysis.
pub fn extract_surface_mesh(glb: &[u8]) -> Result<SurfaceMesh, GeometryError> {
    const MAX_SURFACE_TRIANGLES: usize = 250_000;
    const WELD_SCALE: f32 = 1_000_000.0;

    let (root, binary) = parse_glb(glb)?;
    let meshes = required_array(&root, "meshes")?;
    let nodes = required_array(&root, "nodes")?;
    let accessors = required_array(&root, "accessors")?;
    let views = required_array(&root, "bufferViews")?;
    let mut triangles = Vec::new();
    let mut vertices = BTreeSet::<[i64; 3]>::new();
    let mut lineage_metrics = Metrics::default();

    for (mesh_index, mesh) in meshes.iter().enumerate() {
        let mesh_object = mesh.as_object().ok_or_else(|| {
            GeometryError::Invalid("surface GLB mesh is not an object".to_owned())
        })?;
        let mesh_lineage = mesh_object.get("extras").and_then(Value::as_object);
        let node_lineages = matching_node_lineages(nodes, mesh_index);
        let node_lineage = node_lineages.first();
        let primitives = mesh_object
            .get("primitives")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                GeometryError::Invalid("surface GLB primitive list is missing".to_owned())
            })?;
        for primitive in primitives {
            let primitive_object = primitive.as_object().ok_or_else(|| {
                GeometryError::Invalid("surface GLB primitive is not an object".to_owned())
            })?;
            let primitive_lineage = primitive_object.get("extras").and_then(Value::as_object);
            let lineage = merge_lineage(
                mesh_lineage,
                node_lineage,
                primitive_lineage,
                &mut lineage_metrics,
            );
            let part_id = lineage.part_id.ok_or_else(|| {
                GeometryError::Invalid("surface GLB Part lineage is missing".to_owned())
            })?;
            let attributes = primitive_object
                .get("attributes")
                .and_then(Value::as_object)
                .ok_or_else(|| {
                    GeometryError::Invalid(
                        "surface GLB primitive attributes are missing".to_owned(),
                    )
                })?;
            let position_accessor = required_index(attributes, "POSITION")?;
            let index_accessor = primitive_object
                .get("indices")
                .and_then(Value::as_u64)
                .and_then(|value| usize::try_from(value).ok())
                .ok_or_else(|| {
                    GeometryError::Invalid("surface GLB index accessor is missing".to_owned())
                })?;
            let positions = read_vec3(&accessors, &views, binary, position_accessor)?;
            let indices = read_indices(&accessors, &views, binary, index_accessor)?;
            if positions.is_empty() || indices.is_empty() || indices.len() % 3 != 0 {
                return Err(GeometryError::Invalid(
                    "surface GLB mesh has an invalid triangle payload".to_owned(),
                ));
            }
            for position in &positions {
                if !finite3(*position) {
                    return Err(GeometryError::Invalid(
                        "surface GLB contains a non-finite position".to_owned(),
                    ));
                }
                vertices.insert(position.map(|component| (component * WELD_SCALE).round() as i64));
            }
            for triangle in indices.chunks_exact(3) {
                if triangles.len() >= MAX_SURFACE_TRIANGLES {
                    return Err(GeometryError::Invalid(
                        "surface analysis exceeds the bounded triangle budget".to_owned(),
                    ));
                }
                let indices = [
                    triangle[0] as usize,
                    triangle[1] as usize,
                    triangle[2] as usize,
                ];
                if indices.iter().any(|index| *index >= positions.len()) {
                    return Err(GeometryError::Invalid(
                        "surface GLB triangle index is out of bounds".to_owned(),
                    ));
                }
                let triangle_positions = [
                    positions[indices[0]],
                    positions[indices[1]],
                    positions[indices[2]],
                ];
                let face = cross3(
                    sub3(triangle_positions[1], triangle_positions[0]),
                    sub3(triangle_positions[2], triangle_positions[0]),
                );
                let face_length = length3(face);
                if !face_length.is_finite() || face_length <= DEGENERATE_AREA_EPSILON {
                    return Err(GeometryError::Invalid(
                        "surface GLB contains a degenerate triangle".to_owned(),
                    ));
                }
                triangles.push(SurfaceTriangle {
                    positions: triangle_positions,
                    normal: normalize3(face),
                    part_id: part_id.clone(),
                });
            }
        }
    }
    if triangles.is_empty() || vertices.is_empty() {
        return Err(GeometryError::Invalid(
            "surface GLB contains no analyzable triangles".to_owned(),
        ));
    }
    Ok(SurfaceMesh {
        triangles,
        vertex_count: vertices.len(),
    })
}

fn read_vec2(
    accessors: &[Value],
    views: &[Value],
    binary: &[u8],
    index: usize,
) -> Result<Vec<[f32; 2]>, GeometryError> {
    let values = read_float_accessor(accessors, views, binary, index, "VEC2", 2)?;
    Ok(values
        .into_iter()
        .map(|value| [value[0], value[1]])
        .collect())
}

fn read_vec3(
    accessors: &[Value],
    views: &[Value],
    binary: &[u8],
    index: usize,
) -> Result<Vec<[f32; 3]>, GeometryError> {
    let values = read_float_accessor(accessors, views, binary, index, "VEC3", 3)?;
    Ok(values
        .into_iter()
        .map(|value| [value[0], value[1], value[2]])
        .collect())
}

fn read_vec4(
    accessors: &[Value],
    views: &[Value],
    binary: &[u8],
    index: usize,
) -> Result<Vec<[f32; 4]>, GeometryError> {
    let values = read_float_accessor(accessors, views, binary, index, "VEC4", 4)?;
    Ok(values
        .into_iter()
        .map(|value| [value[0], value[1], value[2], value[3]])
        .collect())
}

fn read_float_accessor(
    accessors: &[Value],
    views: &[Value],
    binary: &[u8],
    index: usize,
    expected_type: &str,
    dimensions: usize,
) -> Result<Vec<Vec<f32>>, GeometryError> {
    let (accessor, offset, count, stride) = accessor_window(
        accessors,
        views,
        binary,
        index,
        expected_type,
        5126,
        dimensions * 4,
    )?;
    let mut result = Vec::with_capacity(count);
    for item in 0..count {
        let start = offset + item * stride;
        let mut values = Vec::with_capacity(dimensions);
        for component in 0..dimensions {
            values.push(f32::from_le_bytes(
                binary[start + component * 4..start + (component + 1) * 4]
                    .try_into()
                    .map_err(|_| {
                        GeometryError::Invalid("GLB float bytes are invalid".to_owned())
                    })?,
            ));
        }
        result.push(values);
    }
    let _ = accessor;
    Ok(result)
}

fn read_indices(
    accessors: &[Value],
    views: &[Value],
    binary: &[u8],
    index: usize,
) -> Result<Vec<u32>, GeometryError> {
    let accessor = accessors
        .get(index)
        .and_then(Value::as_object)
        .ok_or_else(|| GeometryError::Invalid("GLB index accessor is invalid".to_owned()))?;
    let component_type = accessor
        .get("componentType")
        .and_then(Value::as_u64)
        .ok_or_else(|| GeometryError::Invalid("GLB index component type is missing".to_owned()))?;
    let component_size = match component_type {
        5121 => 1,
        5123 => 2,
        5125 => 4,
        _ => {
            return Err(GeometryError::Invalid(
                "GLB index accessor has unsupported component type".to_owned(),
            ))
        }
    };
    let (_, offset, count, stride) = accessor_window(
        accessors,
        views,
        binary,
        index,
        "SCALAR",
        component_type,
        component_size,
    )?;
    let mut result = Vec::with_capacity(count);
    for item in 0..count {
        let start = offset + item * stride;
        let value =
            match component_size {
                1 => binary[start] as u32,
                2 => u16::from_le_bytes(binary[start..start + 2].try_into().map_err(|_| {
                    GeometryError::Invalid("GLB index bytes are invalid".to_owned())
                })?) as u32,
                _ => u32::from_le_bytes(binary[start..start + 4].try_into().map_err(|_| {
                    GeometryError::Invalid("GLB index bytes are invalid".to_owned())
                })?),
            };
        result.push(value);
    }
    Ok(result)
}

fn accessor_window<'a>(
    accessors: &'a [Value],
    views: &[Value],
    binary: &[u8],
    index: usize,
    expected_type: &str,
    expected_component: u64,
    element_size: usize,
) -> Result<(&'a Map<String, Value>, usize, usize, usize), GeometryError> {
    let accessor = accessors
        .get(index)
        .and_then(Value::as_object)
        .ok_or_else(|| GeometryError::Invalid("GLB accessor index is invalid".to_owned()))?;
    if accessor.get("type").and_then(Value::as_str) != Some(expected_type)
        || accessor.get("componentType").and_then(Value::as_u64) != Some(expected_component)
        || accessor.get("sparse").is_some()
    {
        return Err(GeometryError::Invalid(
            "GLB accessor layout is invalid".to_owned(),
        ));
    }
    let count = accessor
        .get("count")
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| GeometryError::Invalid("GLB accessor count is invalid".to_owned()))?;
    let view_index = accessor
        .get("bufferView")
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| GeometryError::Invalid("GLB accessor buffer view is missing".to_owned()))?;
    let view = views
        .get(view_index)
        .and_then(Value::as_object)
        .ok_or_else(|| GeometryError::Invalid("GLB buffer view index is invalid".to_owned()))?;
    if view.get("buffer").and_then(Value::as_u64) != Some(0) {
        return Err(GeometryError::Invalid(
            "GLB uses an unsupported buffer".to_owned(),
        ));
    }
    let view_offset = optional_usize(view, "byteOffset")?.unwrap_or(0);
    let view_length = required_usize(view, "byteLength")?;
    let accessor_offset = optional_usize(accessor, "byteOffset")?.unwrap_or(0);
    let stride = optional_usize(view, "byteStride")?.unwrap_or(element_size);
    if stride < element_size {
        return Err(GeometryError::Invalid(
            "GLB byte stride is too small".to_owned(),
        ));
    }
    let start = view_offset
        .checked_add(accessor_offset)
        .ok_or_else(|| GeometryError::Invalid("GLB accessor offset overflows".to_owned()))?;
    let payload = if count == 0 {
        Some(0)
    } else {
        (count - 1)
            .checked_mul(stride)
            .and_then(|value| value.checked_add(element_size))
    };
    let payload = payload
        .ok_or_else(|| GeometryError::Invalid("GLB accessor payload overflows".to_owned()))?;
    let end = start
        .checked_add(payload)
        .ok_or_else(|| GeometryError::Invalid("GLB accessor range overflows".to_owned()))?;
    let view_end = view_offset
        .checked_add(view_length)
        .ok_or_else(|| GeometryError::Invalid("GLB buffer view range overflows".to_owned()))?;
    if end > view_end || end > binary.len() {
        return Err(GeometryError::Invalid(
            "GLB accessor exceeds BIN".to_owned(),
        ));
    }
    Ok((accessor, start, count, stride))
}

fn required_index(object: &Map<String, Value>, key: &str) -> Result<usize, GeometryError> {
    object
        .get(key)
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| GeometryError::Invalid(format!("GLB {key} accessor is missing")))
}

fn required_text<'a>(object: &'a Map<String, Value>, key: &str) -> Result<&'a str, GeometryError> {
    object
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| GeometryError::Invalid(format!("GLB {key} is missing")))
}

fn required_sha256<'a>(
    object: &'a Map<String, Value>,
    key: &str,
) -> Result<&'a str, GeometryError> {
    let value = required_text(object, key)?;
    if !is_sha256(value) {
        return Err(GeometryError::Invalid(format!("GLB {key} is invalid")));
    }
    Ok(value)
}

fn required_array<'a>(root: &'a Value, key: &str) -> Result<&'a Vec<Value>, GeometryError> {
    root.get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| GeometryError::Invalid(format!("GLB {key} is missing")))
}

fn optional_usize(object: &Map<String, Value>, key: &str) -> Result<Option<usize>, GeometryError> {
    match object.get(key) {
        None => Ok(None),
        Some(value) => value
            .as_u64()
            .and_then(|value| usize::try_from(value).ok())
            .map(Some)
            .ok_or_else(|| GeometryError::Invalid(format!("GLB {key} is invalid"))),
    }
}

fn required_usize(object: &Map<String, Value>, key: &str) -> Result<usize, GeometryError> {
    optional_usize(object, key)?
        .ok_or_else(|| GeometryError::Invalid(format!("GLB {key} is missing")))
}

fn external_uri_count(root: &Value) -> u64 {
    root.get("buffers")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .chain(
            root.get("images")
                .and_then(Value::as_array)
                .into_iter()
                .flatten(),
        )
        .filter(|value| {
            value
                .get("uri")
                .and_then(Value::as_str)
                .is_some_and(|uri| !uri.is_empty())
        })
        .count() as u64
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, GeometryError> {
    let end = offset
        .checked_add(4)
        .ok_or_else(|| GeometryError::Invalid("GLB integer offset overflows".to_owned()))?;
    let value: [u8; 4] = bytes
        .get(offset..end)
        .ok_or_else(|| GeometryError::Invalid("GLB integer is truncated".to_owned()))?
        .try_into()
        .map_err(|_| GeometryError::Invalid("GLB integer is invalid".to_owned()))?;
    Ok(u32::from_le_bytes(value))
}

fn finite3(value: [f32; 3]) -> bool {
    value.into_iter().all(f32::is_finite)
}

fn finite2(value: [f32; 2]) -> bool {
    value.into_iter().all(f32::is_finite)
}

fn finite4(value: [f32; 4]) -> bool {
    value.into_iter().all(f32::is_finite)
}

fn sub3(left: [f32; 3], right: [f32; 3]) -> [f32; 3] {
    [left[0] - right[0], left[1] - right[1], left[2] - right[2]]
}

fn add3(left: [f32; 3], right: [f32; 3]) -> [f32; 3] {
    [left[0] + right[0], left[1] + right[1], left[2] + right[2]]
}

fn cross3(left: [f32; 3], right: [f32; 3]) -> [f32; 3] {
    [
        left[1] * right[2] - left[2] * right[1],
        left[2] * right[0] - left[0] * right[2],
        left[0] * right[1] - left[1] * right[0],
    ]
}

fn dot3(left: [f32; 3], right: [f32; 3]) -> f32 {
    left[0] * right[0] + left[1] * right[1] + left[2] * right[2]
}

fn length3(value: [f32; 3]) -> f32 {
    dot3(value, value).sqrt()
}

fn normalize3(value: [f32; 3]) -> [f32; 3] {
    let length = length3(value);
    if !length.is_finite() || length <= f32::EPSILON {
        [0.0, 0.0, 0.0]
    } else {
        [value[0] / length, value[1] / length, value[2] / length]
    }
}

/// Reconstruct the local UV frame from decoded triangle positions and UVs.
/// This mirrors the geometric derivation used by the bounded writer, but is
/// intentionally calculated from the GLB payload rather than writer metadata.
fn tangent_frame_from_geometry(
    positions: [[f32; 3]; 3],
    uvs: [[f32; 2]; 3],
    uv_area: f32,
) -> Option<([f32; 3], [f32; 3])> {
    if !uv_area.is_finite() || uv_area.abs() <= UV_AREA_EPSILON {
        return None;
    }
    let edge_a = sub3(positions[1], positions[0]);
    let edge_b = sub3(positions[2], positions[0]);
    let uv_a = [uvs[1][0] - uvs[0][0], uvs[1][1] - uvs[0][1]];
    let uv_b = [uvs[2][0] - uvs[0][0], uvs[2][1] - uvs[0][1]];
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
    (finite3(tangent_basis) && finite3(bitangent_basis)).then_some((tangent_basis, bitangent_basis))
}

fn tangent_handedness_matches_geometry(
    normal: [f32; 3],
    tangent: [f32; 4],
    tangent_basis: [f32; 3],
    bitangent_basis: [f32; 3],
) -> bool {
    let normal = normalize3(normal);
    let tangent3 = [tangent[0], tangent[1], tangent[2]];
    if length3(normal) <= f32::EPSILON || length3(tangent3) <= f32::EPSILON {
        return false;
    }
    let expected_tangent = sub3(tangent_basis, scale3(normal, dot3(normal, tangent_basis)));
    if !finite3(expected_tangent) || length3(expected_tangent) <= f32::EPSILON {
        return false;
    }
    let tangent_direction = normalize3(tangent3);
    if dot3(tangent_direction, normalize3(expected_tangent)) < 1.0 - ORTHOGONALITY_EPSILON {
        return false;
    }
    let bitangent_direction = normalize3(bitangent_basis);
    if length3(bitangent_direction) <= f32::EPSILON {
        return false;
    }
    let orientation = dot3(cross3(normal, tangent_direction), bitangent_direction);
    if !orientation.is_finite() {
        return false;
    }
    if orientation.abs() <= ORTHOGONALITY_EPSILON {
        // A nearly collinear Boolean sliver has no numerically stable
        // bitangent orientation. Its tangent direction was already checked
        // above; there is no meaningful handedness bit to reject here.
        return true;
    }
    let expected_handedness = if orientation < 0.0 { -1.0 } else { 1.0 };
    (tangent[3] - expected_handedness).abs() <= ORTHOGONALITY_EPSILON
}

fn scale3(value: [f32; 3], scalar: f32) -> [f32; 3] {
    [value[0] * scalar, value[1] * scalar, value[2] * scalar]
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn sha256(bytes: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(bytes);
    digest
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn v2_program() -> Value {
        let mut draft = json!({
            "schema_version":"GeometryProgram@2",
            "project_id":"project-test",
            "representation_plan_sha256":"b".repeat(64),
            "operator_catalog_sha256":crate::operator_catalog_sha256(),
            "units":{"length":"meter","angle":"radian","coordinate_system":"right-handed-y-up"},
            "budgets":{
                "max_nodes":8,
                "max_triangles":10000,
                "max_glb_bytes":1048576,
                "max_worker_memory_bytes":536870912,
                "max_runtime_ms":10000
            },
            "nodes":[{
                "node_id":"shell",
                "operator_id":"forgecad.geometry.primitive@2",
                "inputs":[],
                "parameters":{
                    "shape":"box",
                    "size_m":[1.2,1.6,0.55],
                    "position_m":[0.0,1.7,0.0],
                    "rotation_rad":[0.0,0.0,0.0]
                }
            }],
            "part_outputs":[{
                "part_id":"shell",
                "input_node_ids":["shell"],
                "material_zone_id":"zone-white-shell",
                "solid":true
            }]
        });
        let hash = crate::geometry_program_v2_draft_hash(&draft).expect("valid V2 draft");
        draft["canonical_sha256"] = Value::String(hash);
        draft
    }

    fn v2_glb() -> Vec<u8> {
        crate::compile_geometry_program(&v2_program())
            .expect("compile V2 fixture")
            .glb
    }

    fn root_and_binary(glb: &[u8]) -> (Value, Vec<u8>) {
        let (root, binary) = parse_glb(glb).expect("valid fixture GLB");
        (root, binary.to_vec())
    }

    fn rebuild_glb(root: &Value, binary: &[u8]) -> Vec<u8> {
        let mut json = serde_json::to_vec(root).expect("serialize mutated root");
        while json.len() % 4 != 0 {
            json.push(b' ');
        }
        let total = 12 + 8 + json.len() + 8 + binary.len();
        let mut glb = Vec::with_capacity(total);
        glb.extend_from_slice(b"glTF");
        glb.extend_from_slice(&2u32.to_le_bytes());
        glb.extend_from_slice(&(total as u32).to_le_bytes());
        glb.extend_from_slice(&(json.len() as u32).to_le_bytes());
        glb.extend_from_slice(b"JSON");
        glb.extend_from_slice(&json);
        glb.extend_from_slice(&(binary.len() as u32).to_le_bytes());
        glb.extend_from_slice(b"BIN\0");
        glb.extend_from_slice(binary);
        glb
    }

    fn binary_offset(glb: &[u8]) -> usize {
        let json_length = u32::from_le_bytes(glb[12..16].try_into().expect("JSON length")) as usize;
        20 + json_length + 8
    }

    fn accessor_byte_offset(root: &Value, accessor_index: usize) -> usize {
        let accessor = &root["accessors"][accessor_index];
        let view_index = accessor["bufferView"].as_u64().expect("buffer view") as usize;
        root["bufferViews"][view_index]["byteOffset"]
            .as_u64()
            .unwrap_or(0) as usize
            + accessor["byteOffset"].as_u64().unwrap_or(0) as usize
    }

    #[test]
    fn tangent_w_sign_only_bin_tamper_fails_geometric_handedness_readback() {
        let glb = v2_glb();
        let (root, _) = root_and_binary(&glb);
        let tangent_accessor = root["meshes"][0]["primitives"][0]["attributes"]["TANGENT"]
            .as_u64()
            .expect("tangent accessor") as usize;
        let handedness_offset =
            binary_offset(&glb) + accessor_byte_offset(&root, tangent_accessor) + 12;
        let original = f32::from_le_bytes(
            glb[handedness_offset..handedness_offset + 4]
                .try_into()
                .expect("handedness bytes"),
        );
        assert!((original.abs() - 1.0).abs() <= ORTHOGONALITY_EPSILON);

        let mut tampered = glb.clone();
        tampered[handedness_offset..handedness_offset + 4]
            .copy_from_slice(&(-original).to_le_bytes());
        let report = inspect_glb(&tampered).expect("W-only BIN tamper remains inspectable");
        assert!(report.tangent_handedness_error_count > 0);
        assert!(report
            .failure_codes
            .iter()
            .any(|code| code == "TANGENT_HANDEDNESS"));
        assert!(!report.hard_gate_passed);
    }

    fn assert_v2_scene_graph_rejected(root: Value, binary: &[u8], mutation: &str) {
        let report = inspect_glb(&rebuild_glb(&root, binary))
            .unwrap_or_else(|error| panic!("{mutation} must remain inspectable: {error}"));
        assert!(
            report.metadata_mismatch_count > 0,
            "{mutation} did not record a canonical scene-graph mismatch"
        );
        assert!(
            report
                .failure_codes
                .iter()
                .any(|code| code == "METADATA_MISMATCH"),
            "{mutation} did not expose METADATA_MISMATCH: {:?}",
            report.failure_codes
        );
        assert!(!report.hard_gate_passed, "{mutation} unexpectedly passed");
    }

    fn assert_v2_closed_profile_rejected(root: Value, binary: &[u8], mutation: &str) {
        let report = inspect_glb(&rebuild_glb(&root, binary))
            .unwrap_or_else(|error| panic!("{mutation} must remain inspectable: {error}"));
        assert!(
            report.metadata_mismatch_count > 0,
            "{mutation} did not record a closed V2 profile mismatch"
        );
        assert!(
            report
                .failure_codes
                .iter()
                .any(|code| code == "METADATA_MISMATCH"),
            "{mutation} did not expose METADATA_MISMATCH: {:?}",
            report.failure_codes
        );
        assert!(!report.hard_gate_passed, "{mutation} unexpectedly passed");
    }

    #[test]
    fn v2_scene_graph_rejects_duplicate_unlineaged_child_and_transformed_instances() {
        let glb = v2_glb();
        let (root, binary) = root_and_binary(&glb);
        assert!(inspect_glb(&glb).expect("base V2 GLB").hard_gate_passed);

        let mut two_scenes = root.clone();
        two_scenes["scenes"]
            .as_array_mut()
            .expect("scenes")
            .push(json!({"nodes":[]}));
        assert_v2_scene_graph_rejected(two_scenes, &binary, "second scene");

        let mut duplicate_instance = root.clone();
        let duplicate_node = duplicate_instance["nodes"][0].clone();
        let duplicate_index = duplicate_instance["nodes"].as_array().expect("nodes").len();
        duplicate_instance["nodes"]
            .as_array_mut()
            .expect("nodes")
            .push(duplicate_node);
        duplicate_instance["scenes"][0]["nodes"]
            .as_array_mut()
            .expect("scene nodes")
            .push(json!(duplicate_index));
        assert_v2_scene_graph_rejected(duplicate_instance, &binary, "duplicate mesh instance");

        let mut unlineaged_instance = root.clone();
        unlineaged_instance["nodes"][0]
            .as_object_mut()
            .expect("node")
            .remove("extras");
        assert_v2_scene_graph_rejected(unlineaged_instance, &binary, "unlineaged instance");

        let mut child_instance = root.clone();
        child_instance["nodes"][0]["children"] = json!([]);
        assert_v2_scene_graph_rejected(child_instance, &binary, "child declaration");

        let mut translated_instance = root.clone();
        translated_instance["nodes"][0]["translation"] = json!([0.01, 0.0, 0.0]);
        assert_v2_scene_graph_rejected(translated_instance, &binary, "non-identity translation");

        let mut transformed_matrix_instance = root;
        transformed_matrix_instance["nodes"][0]["matrix"] = json!([
            1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.01, 0.0, 0.0, 1.0
        ]);
        assert_v2_scene_graph_rejected(transformed_matrix_instance, &binary, "non-identity matrix");
    }

    #[test]
    fn v2_closed_profile_rejects_morph_animation_skin_and_extension_json() {
        let glb = v2_glb();
        let (root, binary) = root_and_binary(&glb);
        assert!(inspect_glb(&glb).expect("base V2 GLB").hard_gate_passed);

        let mut mesh_weights = root.clone();
        mesh_weights["meshes"][0]["weights"] = json!([1.0]);
        assert_v2_closed_profile_rejected(mesh_weights, &binary, "mesh morph weights");

        let mut primitive_targets = root.clone();
        let position_accessor = primitive_targets["meshes"][0]["primitives"][0]["attributes"]
            ["POSITION"]
            .as_u64()
            .expect("position accessor");
        primitive_targets["meshes"][0]["primitives"][0]["targets"] =
            json!([{"POSITION":position_accessor}]);
        assert_v2_closed_profile_rejected(primitive_targets, &binary, "morph target");

        let mut node_weights = root.clone();
        node_weights["nodes"][0]["weights"] = json!([1.0]);
        assert_v2_closed_profile_rejected(node_weights, &binary, "node morph weights");

        let mut animations = root.clone();
        animations["animations"] = json!([{"channels":[],"samplers":[]}]);
        assert_v2_closed_profile_rejected(animations, &binary, "animation");

        let mut skins = root.clone();
        skins["skins"] = json!([{"joints":[0]}]);
        assert_v2_closed_profile_rejected(skins, &binary, "skin");

        let mut extensions = root;
        extensions["extensionsUsed"] = json!(["EXT_mesh_gpu_instancing"]);
        extensions["nodes"][0]["extensions"] = json!({"EXT_mesh_gpu_instancing":{"attributes":{}}});
        assert_v2_closed_profile_rejected(extensions, &binary, "non-allowlisted extension");
    }

    #[test]
    fn v2_closed_bin_layout_rejects_uninspected_morph_payload() {
        let glb = v2_glb();
        let (root, binary) = root_and_binary(&glb);

        // Even bytes which have no JSON accessor are not admissible padding:
        // the sole buffer's declared byteLength and the static view ranges
        // must close exactly over the BIN chunk.
        let mut trailing_root = root.clone();
        let mut trailing_binary = binary.clone();
        trailing_binary.extend_from_slice(&[0, 0, 0, 0]);
        trailing_root["buffers"][0]["byteLength"] = json!(trailing_binary.len());
        assert_v2_closed_profile_rejected(
            trailing_root,
            &trailing_binary,
            "uninspected trailing BIN bytes",
        );

        let mut root = root;
        let mut binary = binary;
        let primitive = &root["meshes"][0]["primitives"][0];
        let position_accessor = primitive["attributes"]["POSITION"]
            .as_u64()
            .expect("position accessor") as usize;
        let position_count = root["accessors"][position_accessor]["count"]
            .as_u64()
            .expect("position count") as usize;

        // A glTF morph target can use a normal-looking VEC3 accessor in the
        // same BIN chunk.  The static readback must reject it instead of
        // silently inspecting only the base POSITION accessor.
        let hidden_offset = binary.len();
        for _ in 0..position_count {
            binary.extend_from_slice(&0.125f32.to_le_bytes());
            binary.extend_from_slice(&0.0f32.to_le_bytes());
            binary.extend_from_slice(&0.0f32.to_le_bytes());
        }
        let hidden_view = root["bufferViews"].as_array().expect("buffer views").len();
        root["bufferViews"]
            .as_array_mut()
            .expect("buffer views")
            .push(json!({
                "buffer":0,
                "byteOffset":hidden_offset,
                "byteLength":position_count * 12,
                "target":34962
            }));
        let hidden_accessor = root["accessors"].as_array().expect("accessors").len();
        root["accessors"]
            .as_array_mut()
            .expect("accessors")
            .push(json!({
                "bufferView":hidden_view,
                "componentType":5126,
                "count":position_count,
                "type":"VEC3"
            }));
        root["buffers"][0]["byteLength"] = json!(binary.len());
        root["meshes"][0]["weights"] = json!([1.0]);
        root["meshes"][0]["primitives"][0]["targets"] = json!([{"POSITION":hidden_accessor}]);

        assert_v2_closed_profile_rejected(root, &binary, "uninspected morph BIN payload");
    }
}
