//! Bounded, deterministic Hero UV layout diagnostics.
//!
//! This source-only Worker slice is deliberately separate from the legacy
//! `surface_bake` and `continuous_uv_atlas` paths.  It consumes one admitted
//! Low GLB, preserves its physical UV0 as the game-material channel, derives a
//! conservative per-triangle UV1 lightmap channel, and returns the diagnostics
//! needed by the `HeroUvLayout@1` contract.  It does not lower a new GLB, write
//! Runtime/CAS/SQLite state, advance a stage, or claim an art/visual pass.

use crate::integrity::{self, TopologyMesh};
use crate::GeometryError;
use base64::Engine;
use forgecad_worker_protocol::{
    PRODUCTION_WEAPON_HERO_UV_LAYOUT_OPERATION, PRODUCTION_WEAPON_HERO_UV_LAYOUT_POLICY,
    PRODUCTION_WEAPON_HERO_UV_LAYOUT_REQUEST_SCHEMA_VERSION,
    PRODUCTION_WEAPON_HERO_UV_LAYOUT_RESULT_SCHEMA_VERSION,
};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

const MAX_GLB_BYTES: usize = 32 * 1024 * 1024;
const MAX_TRIANGLES: usize = 8_192;
const MAX_VISIBILITY_WEIGHTS: usize = 4_096;
const MAX_OVERLAP_COMPARISONS: usize = 2_000_000;
const UV_EPSILON: f32 = 1.0e-6;
const AREA_EPSILON: f32 = 1.0e-10;
const POSITION_WELD_SCALE: f32 = 1_000_000.0;
const MIKK_DOT_THRESHOLD: f32 = 0.999;

#[derive(Debug, Clone, Copy)]
struct VisibilityWeight {
    first_person: f32,
    world: f32,
    hidden: f32,
}

#[derive(Debug, Clone)]
struct TriangleRecord {
    index: usize,
    part_id: String,
    material_zone_id: String,
    corners: [[f32; 3]; 3],
    normals: [[f32; 3]; 3],
    uv0: [[f32; 2]; 3],
    uv1: [[f32; 2]; 3],
    world_area: f32,
    uv0_signed_area: f32,
    uv0_area: f32,
    first_person_weight: f32,
    uv0_island: usize,
    uv1_chart: usize,
}

#[derive(Debug, Clone)]
struct EdgeRecord {
    triangle_index: usize,
    first_uv: [f32; 2],
    second_uv: [f32; 2],
    face_normal: [f32; 3],
}

#[derive(Debug, Clone, Copy)]
struct EdgeMetrics {
    boundary_count: u64,
    non_manifold_count: u64,
    hard_edge_count: u64,
    uv_seam_count: u64,
    hard_edge_without_seam_count: u64,
    material_boundary_count: u64,
}

#[derive(Debug, Clone)]
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

/// Run the closed source-only Hero UV diagnostic/producer.
pub fn run(payload: &Map<String, Value>) -> Result<Value, GeometryError> {
    const FIELDS: &[&str] = &[
        "schema_version",
        "low_artifact_sha256",
        "low_glb_base64",
        "resolution",
        "padding_texels",
        "min_mip_level",
        "hard_edge_angle_deg",
        "stretch_threshold",
        "visibility_weights",
        "canonical_sha256",
    ];
    crate::require_closed_payload(payload, FIELDS)?;
    require_const(
        payload,
        "schema_version",
        PRODUCTION_WEAPON_HERO_UV_LAYOUT_REQUEST_SCHEMA_VERSION,
    )?;

    let canonical = required_hash(payload, "canonical_sha256")?;
    let mut request_without_hash = payload.clone();
    request_without_hash.remove("canonical_sha256");
    if crate::canonical_hash(&Value::Object(request_without_hash)) != canonical {
        return Err(invalid("HERO_UV_REQUEST_CANONICAL_MISMATCH"));
    }

    let resolution = payload
        .get("resolution")
        .and_then(Value::as_u64)
        .ok_or_else(|| invalid("HERO_UV_RESOLUTION_INVALID"))?;
    if !matches!(resolution, 2048 | 4096) {
        return Err(invalid("HERO_UV_RESOLUTION_INVALID"));
    }
    let padding_texels = payload
        .get("padding_texels")
        .and_then(Value::as_u64)
        .filter(|value| (1..=128).contains(value))
        .ok_or_else(|| invalid("HERO_UV_PADDING_INVALID"))?;
    let min_mip_level = payload
        .get("min_mip_level")
        .and_then(Value::as_u64)
        .filter(|value| *value <= 12)
        .ok_or_else(|| invalid("HERO_UV_MIP_LEVEL_INVALID"))?;
    let required_padding_texels = 1u64
        .checked_shl(min_mip_level as u32)
        .ok_or_else(|| invalid("HERO_UV_MIP_LEVEL_INVALID"))?;
    let hard_edge_angle_deg = required_finite_f32(payload, "hard_edge_angle_deg")?;
    if hard_edge_angle_deg <= 0.1 || hard_edge_angle_deg >= 89.9 {
        return Err(invalid("HERO_UV_HARD_EDGE_ANGLE_INVALID"));
    }
    let stretch_threshold = required_finite_f32(payload, "stretch_threshold")?;
    if !(1.0..=100.0).contains(&stretch_threshold) {
        return Err(invalid("HERO_UV_STRETCH_THRESHOLD_INVALID"));
    }

    let source_hash = required_hash(payload, "low_artifact_sha256")?;
    let encoded = payload
        .get("low_glb_base64")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("HERO_UV_LOW_GLB_MISSING"))?;
    if encoded.len() > MAX_GLB_BYTES.saturating_mul(2) {
        return Err(invalid("HERO_UV_LOW_GLB_TOO_LARGE"));
    }
    let glb = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|_| invalid("HERO_UV_LOW_GLB_INVALID"))?;
    if glb.is_empty() || glb.len() > MAX_GLB_BYTES || sha256_hex(&glb) != source_hash {
        return Err(invalid("HERO_UV_LOW_GLB_HASH_MISMATCH"));
    }
    let inspection = integrity::inspect_glb(&glb)?;
    if !inspection.hard_gate_passed {
        return Err(invalid(format!(
            "HERO_UV_LOW_GLB_READBACK_FAILED: failures={:?}",
            inspection.failure_codes
        )));
    }
    let topology = integrity::extract_topology_mesh(&glb, MAX_TRIANGLES)?;
    if topology.triangles.len() > MAX_TRIANGLES {
        return Err(invalid("HERO_UV_TRIANGLE_BUDGET_EXCEEDED"));
    }
    let weights = parse_visibility_weights(
        payload
            .get("visibility_weights")
            .and_then(Value::as_array)
            .ok_or_else(|| invalid("HERO_UV_VISIBILITY_WEIGHTS_MISSING"))?,
    )?;
    validate_visibility_coverage(&topology, &weights)?;

    let mut triangles = build_triangle_records(&topology, &weights)?;
    let edge_metrics = classify_edges(&triangles, hard_edge_angle_deg)?;
    assign_uv0_islands(&mut triangles);
    assign_uv1(&mut triangles, resolution as f32, padding_texels as f32)?;

    let overlap_count = count_uv0_overlaps(&triangles)?;
    let uv1_overlap_count = count_uv1_overlaps(&triangles)?;
    let metrics = aggregate_metrics(
        &triangles,
        resolution as f32,
        stretch_threshold,
        overlap_count,
        uv1_overlap_count,
        edge_metrics,
        padding_texels,
        required_padding_texels,
    );
    let mikk = replay_mikk(&triangles)?;

    let mut result = json!({
        "schema_version": PRODUCTION_WEAPON_HERO_UV_LAYOUT_RESULT_SCHEMA_VERSION,
        "operation": PRODUCTION_WEAPON_HERO_UV_LAYOUT_OPERATION,
        "policy": PRODUCTION_WEAPON_HERO_UV_LAYOUT_POLICY,
        "policy_sha256": sha256_hex(PRODUCTION_WEAPON_HERO_UV_LAYOUT_POLICY.as_bytes()),
        "low_artifact_sha256": source_hash,
        "resolution": resolution,
        "uv0_semantic": "game-material-hero-channel@1",
        "uv1_semantic": "lightmap-bake-channel@1",
        "visibility_weight_policy": "first-person-world-hidden-per-part@1",
        "mip_padding_policy": "base-padding-at-least-2^min-mip-level@1",
        "seam_policy": "uv-seam-or-material-boundary-and-hard-edge-congruence@1",
        "hard_edge_policy": "face-normal-angle-threshold@1",
        "uv0_corners": uv0_corner_values(&triangles),
        "uv1_corners": uv1_corner_values(&triangles),
        "visibility_weights": visibility_weight_values(&weights),
        "islands": island_values(&triangles, resolution as f32),
        "metrics": metrics,
        "mikk_replay": mikk,
        "source_only": true,
        "quality_status": "structural_only",
        "structural_status": "PASS_SOURCE_STRUCTURAL",
        "visual_status": "NOT_PROVEN",
        "human_status": "NOT_RUN",
        "engine_status": "NOT_RUN",
        "distribution_status": "NOT_RUN",
        "runtime_write_performed": false,
        "production_stage_advanced": false,
        "candidate_confirmed": false,
        "version_created": false,
        "export_performed": false,
        "promotion_eligible": false,
        "canonical_sha256": ""
    });
    result["canonical_sha256"] = Value::String(wire_canonical_hash(&result)?);
    Ok(result)
}

fn build_triangle_records(
    topology: &TopologyMesh,
    weights: &BTreeMap<String, VisibilityWeight>,
) -> Result<Vec<TriangleRecord>, GeometryError> {
    topology
        .triangles
        .iter()
        .enumerate()
        .map(|(index, triangle)| {
            let weight = weights
                .get(&triangle.part_id)
                .ok_or_else(|| invalid("HERO_UV_VISIBILITY_WEIGHT_PART_MISSING"))?;
            let corners = [
                triangle.corners[0].position,
                triangle.corners[1].position,
                triangle.corners[2].position,
            ];
            let normals = [
                normalize3(triangle.corners[0].normal),
                normalize3(triangle.corners[1].normal),
                normalize3(triangle.corners[2].normal),
            ];
            let uv0 = [
                triangle.corners[0].texcoord_0,
                triangle.corners[1].texcoord_0,
                triangle.corners[2].texcoord_0,
            ];
            let world_cross = cross3(sub3(corners[1], corners[0]), sub3(corners[2], corners[0]));
            let world_area = 0.5 * length3(world_cross);
            let uv_cross = cross2(sub2(uv0[1], uv0[0]), sub2(uv0[2], uv0[0]));
            if !world_area.is_finite() || world_area <= AREA_EPSILON {
                return Err(invalid("HERO_UV_WORLD_TRIANGLE_DEGENERATE"));
            }
            if uv0.iter().flatten().any(|value| !value.is_finite()) {
                return Err(invalid("HERO_UV_NON_FINITE_UV0"));
            }
            Ok(TriangleRecord {
                index,
                part_id: triangle.part_id.clone(),
                material_zone_id: triangle.material_zone_id.clone(),
                corners,
                normals,
                uv0,
                uv1: [[0.0; 2]; 3],
                world_area,
                uv0_signed_area: uv_cross * 0.5,
                uv0_area: (uv_cross * 0.5).abs(),
                first_person_weight: weight.first_person,
                uv0_island: 0,
                uv1_chart: index,
            })
        })
        .collect()
}

fn classify_edges(
    triangles: &[TriangleRecord],
    hard_edge_angle_deg: f32,
) -> Result<EdgeMetrics, GeometryError> {
    let mut edges = BTreeMap::<([i64; 3], [i64; 3]), Vec<EdgeRecord>>::new();
    for triangle in triangles {
        let face_normal = normalize3(cross3(
            sub3(triangle.corners[1], triangle.corners[0]),
            sub3(triangle.corners[2], triangle.corners[0]),
        ));
        for (left, right) in [(0usize, 1usize), (1, 2), (2, 0)] {
            let left_key = position_key(triangle.corners[left]);
            let right_key = position_key(triangle.corners[right]);
            let (first_position, second_position, first_uv, second_uv) = if left_key <= right_key {
                (left_key, right_key, triangle.uv0[left], triangle.uv0[right])
            } else {
                (right_key, left_key, triangle.uv0[right], triangle.uv0[left])
            };
            edges
                .entry((first_position, second_position))
                .or_default()
                .push(EdgeRecord {
                    triangle_index: triangle.index,
                    first_uv,
                    second_uv,
                    face_normal,
                });
        }
    }

    let cos_threshold = (hard_edge_angle_deg.to_radians()).cos();
    let mut metrics = EdgeMetrics {
        boundary_count: 0,
        non_manifold_count: 0,
        hard_edge_count: 0,
        uv_seam_count: 0,
        hard_edge_without_seam_count: 0,
        material_boundary_count: 0,
    };
    for ((_, _), records) in edges {
        match records.as_slice() {
            [] => {}
            [record] => {
                metrics.boundary_count += 1;
                metrics.uv_seam_count += 1;
                let _ = record.triangle_index;
            }
            [left, right] => {
                let hard = dot3(left.face_normal, right.face_normal) < cos_threshold;
                let seam = !same_uv(left.first_uv, right.first_uv)
                    || !same_uv(left.second_uv, right.second_uv)
                    || triangle_material_zone(triangles, left.triangle_index)
                        != triangle_material_zone(triangles, right.triangle_index);
                if hard {
                    metrics.hard_edge_count += 1;
                }
                if seam {
                    metrics.uv_seam_count += 1;
                }
                if hard && !seam {
                    metrics.hard_edge_without_seam_count += 1;
                }
                if triangle_material_zone(triangles, left.triangle_index)
                    != triangle_material_zone(triangles, right.triangle_index)
                {
                    metrics.material_boundary_count += 1;
                }
            }
            _ => metrics.non_manifold_count += 1,
        }
    }
    Ok(metrics)
}

fn assign_uv0_islands(triangles: &mut [TriangleRecord]) {
    let mut parents = (0..triangles.len()).collect::<Vec<_>>();
    let mut edges = BTreeMap::<([i64; 3], [i64; 3]), Vec<EdgeRecord>>::new();
    for triangle in triangles.iter() {
        for (left, right) in [(0usize, 1usize), (1, 2), (2, 0)] {
            let left_key = position_key(triangle.corners[left]);
            let right_key = position_key(triangle.corners[right]);
            let (first_position, second_position, first_uv, second_uv) = if left_key <= right_key {
                (left_key, right_key, triangle.uv0[left], triangle.uv0[right])
            } else {
                (right_key, left_key, triangle.uv0[right], triangle.uv0[left])
            };
            edges
                .entry((first_position, second_position))
                .or_default()
                .push(EdgeRecord {
                    triangle_index: triangle.index,
                    first_uv,
                    second_uv,
                    face_normal: [0.0; 3],
                });
        }
    }
    for records in edges.values() {
        if let [left, right] = records.as_slice() {
            if same_uv(left.first_uv, right.first_uv) && same_uv(left.second_uv, right.second_uv) {
                union(&mut parents, left.triangle_index, right.triangle_index);
            }
        }
    }
    let mut roots = BTreeMap::<usize, usize>::new();
    for triangle in triangles.iter_mut() {
        let root = find(&mut parents, triangle.index);
        let island = if let Some(existing) = roots.get(&root) {
            *existing
        } else {
            let next = roots.len();
            roots.insert(root, next);
            next
        };
        triangle.uv0_island = island;
    }
}

fn assign_uv1(
    triangles: &mut [TriangleRecord],
    resolution: f32,
    padding_texels: f32,
) -> Result<(), GeometryError> {
    let chart_count = triangles.len();
    if chart_count == 0 {
        return Err(invalid("HERO_UV_NO_TRIANGLES"));
    }
    let columns = (chart_count as f32).sqrt().ceil().max(1.0) as usize;
    let rows = chart_count.div_ceil(columns).max(1);
    let cell_u = 1.0 / columns as f32;
    let cell_v = 1.0 / rows as f32;
    let padding_u = padding_texels / resolution;
    let padding_v = padding_texels / resolution;
    if cell_u <= 2.0 * padding_u || cell_v <= 2.0 * padding_v {
        return Err(invalid("HERO_UV_UV1_PADDING_BUDGET_EXCEEDED"));
    }
    for triangle in triangles.iter_mut() {
        let projection = best_planar_projection(triangle.corners);
        let min = [
            projection
                .iter()
                .map(|point| point[0])
                .fold(f32::INFINITY, f32::min),
            projection
                .iter()
                .map(|point| point[1])
                .fold(f32::INFINITY, f32::min),
        ];
        let max = [
            projection
                .iter()
                .map(|point| point[0])
                .fold(f32::NEG_INFINITY, f32::max),
            projection
                .iter()
                .map(|point| point[1])
                .fold(f32::NEG_INFINITY, f32::max),
        ];
        let extent = [
            (max[0] - min[0]).max(UV_EPSILON),
            (max[1] - min[1]).max(UV_EPSILON),
        ];
        let column = triangle.uv1_chart % columns;
        let row = triangle.uv1_chart / columns;
        let inner_u = cell_u - 2.0 * padding_u;
        let inner_v = cell_v - 2.0 * padding_v;
        for vertex in 0..3 {
            let mut local = [
                (projection[vertex][0] - min[0]) / extent[0],
                (projection[vertex][1] - min[1]) / extent[1],
            ];
            local[0] = local[0].clamp(0.0, 1.0);
            local[1] = local[1].clamp(0.0, 1.0);
            triangle.uv1[vertex] = [
                column as f32 * cell_u + padding_u + local[0] * inner_u,
                row as f32 * cell_v + padding_v + local[1] * inner_v,
            ];
        }
        let signed = cross2(
            sub2(triangle.uv1[1], triangle.uv1[0]),
            sub2(triangle.uv1[2], triangle.uv1[0]),
        );
        if signed < 0.0 {
            for uv in &mut triangle.uv1 {
                uv[1] = row as f32 * cell_v + padding_v + inner_v
                    - (uv[1] - row as f32 * cell_v - padding_v);
            }
        }
        if triangle
            .uv1
            .iter()
            .flatten()
            .any(|value| !value.is_finite())
        {
            return Err(invalid("HERO_UV_UV1_NON_FINITE"));
        }
    }
    Ok(())
}

fn aggregate_metrics(
    triangles: &[TriangleRecord],
    resolution: f32,
    stretch_threshold: f32,
    uv0_overlap_count: u64,
    uv1_overlap_count: u64,
    edge_metrics: EdgeMetrics,
    padding_texels: u64,
    required_padding_texels: u64,
) -> Value {
    let mut out_of_bounds_count = 0u64;
    let mut uv0_zero_area_count = 0u64;
    let mut uv0_inverted_count = 0u64;
    let mut uv1_out_of_bounds_count = 0u64;
    let mut stretch_exceeded_count = 0u64;
    let mut weighted_world_area = 0.0f64;
    let mut weighted_uv_area = 0.0f64;
    let mut weighted_world_area_all = 0.0f64;
    let mut weighted_uv_area_all = 0.0f64;
    let mut max_stretch = 1.0f32;
    for triangle in triangles {
        if triangle.uv0.iter().any(|uv| {
            uv[0] < -UV_EPSILON
                || uv[0] > 1.0 + UV_EPSILON
                || uv[1] < -UV_EPSILON
                || uv[1] > 1.0 + UV_EPSILON
        }) {
            out_of_bounds_count += 1;
        }
        if triangle.uv1.iter().any(|uv| {
            uv[0] < -UV_EPSILON
                || uv[0] > 1.0 + UV_EPSILON
                || uv[1] < -UV_EPSILON
                || uv[1] > 1.0 + UV_EPSILON
        }) {
            uv1_out_of_bounds_count += 1;
        }
        if triangle.uv0_area <= AREA_EPSILON {
            uv0_zero_area_count += 1;
        }
        if triangle.uv0_signed_area < -AREA_EPSILON {
            uv0_inverted_count += 1;
        }
        let stretch = triangle_stretch(triangle);
        max_stretch = max_stretch.max(stretch);
        if stretch > stretch_threshold {
            stretch_exceeded_count += 1;
        }
        weighted_world_area_all += f64::from(triangle.world_area);
        weighted_uv_area_all += f64::from(triangle.uv0_area);
        weighted_world_area += f64::from(triangle.world_area * triangle.first_person_weight);
        weighted_uv_area += f64::from(triangle.uv0_area * triangle.first_person_weight);
    }
    let weighted_density = texel_density(weighted_uv_area, weighted_world_area, resolution);
    let all_density = texel_density(weighted_uv_area_all, weighted_world_area_all, resolution);
    json!({
        "triangle_count": triangles.len(),
        "uv0_island_count": triangles.iter().map(|triangle| triangle.uv0_island).collect::<BTreeSet<_>>().len(),
        "uv1_chart_count": triangles.len(),
        "uv0_overlap_count": uv0_overlap_count,
        "uv1_overlap_count": uv1_overlap_count,
        "uv0_out_of_bounds_triangle_count": out_of_bounds_count,
        "uv1_out_of_bounds_triangle_count": uv1_out_of_bounds_count,
        "uv0_zero_area_triangle_count": uv0_zero_area_count,
        "uv0_inverted_triangle_count": uv0_inverted_count,
        "stretch_exceeded_triangle_count": stretch_exceeded_count,
        "max_stretch_ratio": max_stretch,
        "first_person_weighted_texel_density": weighted_density,
        "all_surface_texel_density": all_density,
        "boundary_edge_count": edge_metrics.boundary_count,
        "non_manifold_edge_count": edge_metrics.non_manifold_count,
        "hard_edge_count": edge_metrics.hard_edge_count,
        "uv_seam_count": edge_metrics.uv_seam_count,
        "material_boundary_count": edge_metrics.material_boundary_count,
        "hard_edge_without_seam_count": edge_metrics.hard_edge_without_seam_count,
        "seam_hard_edge_congruence": edge_metrics.hard_edge_without_seam_count == 0,
        "padding_texels": padding_texels,
        "required_mip_padding_texels": required_padding_texels,
        "mip_padding_passed": padding_texels >= required_padding_texels,
        "first_person_weighting_applied": true,
        "uv0_structural_gate": uv0_overlap_count == 0
            && out_of_bounds_count == 0
            && uv0_zero_area_count == 0
            && edge_metrics.hard_edge_without_seam_count == 0,
        "uv1_structural_gate": uv1_overlap_count == 0 && uv1_out_of_bounds_count == 0
    })
}

fn replay_mikk(triangles: &[TriangleRecord]) -> Result<Value, GeometryError> {
    let mut mesh = MikkTriangleMesh {
        positions: Vec::with_capacity(triangles.len() * 3),
        normals: Vec::with_capacity(triangles.len() * 3),
        uvs: Vec::with_capacity(triangles.len() * 3),
        tangents: vec![[0.0; 4]; triangles.len() * 3],
    };
    for triangle in triangles {
        mesh.positions.extend(triangle.corners);
        mesh.normals.extend(triangle.normals);
        mesh.uvs.extend(triangle.uv0);
    }
    if !mikktspace::generate_tangents(&mut mesh) {
        return Err(invalid("HERO_UV_MIKK_REPLAY_FAILED"));
    }
    let non_finite_count = mesh
        .tangents
        .iter()
        .filter(|tangent| tangent.iter().any(|value| !value.is_finite()))
        .count();
    if non_finite_count != 0 {
        return Err(invalid("HERO_UV_MIKK_REPLAY_NON_FINITE"));
    }
    let zero = [0.0, 0.0, 0.0];
    let mut mismatch_count = 0u64;
    for (index, generated) in mesh.tangents.iter().enumerate() {
        let source = triangles[index / 3].normals[index % 3];
        let source_frame = tangent_from_uv_frame(
            [
                triangles[index / 3].corners[0],
                triangles[index / 3].corners[1],
                triangles[index / 3].corners[2],
            ],
            source,
            triangles[index / 3].uv0,
        )
        .unwrap_or([0.0, 0.0, 0.0, 1.0]);
        let dot = dot3(
            [generated[0], generated[1], generated[2]],
            [source_frame[0], source_frame[1], source_frame[2]],
        );
        if dot < MIKK_DOT_THRESHOLD || generated[3].signum() != source_frame[3].signum() {
            mismatch_count += 1;
        }
    }
    let _ = zero;
    Ok(json!({
        "algorithm": "MikkTSpace@0.3.0",
        "status": "PASS_SOURCE_STRUCTURAL",
        "triangle_corner_count": mesh.tangents.len(),
        "non_finite_count": non_finite_count,
        "input_frame_mismatch_count": mismatch_count,
        "tangent_semantics": "UV0-derived-tangent-input-replay@1",
        "normal_convention": "OpenGL+Y"
    }))
}

fn tangent_from_uv_frame(
    positions: [[f32; 3]; 3],
    normal: [f32; 3],
    uvs: [[f32; 2]; 3],
) -> Option<[f32; 4]> {
    let edge_a = sub3(positions[1], positions[0]);
    let edge_b = sub3(positions[2], positions[0]);
    let uv_a = sub2(uvs[1], uvs[0]);
    let uv_b = sub2(uvs[2], uvs[0]);
    let area = cross2(uv_a, uv_b);
    if !area.is_finite() || area.abs() <= UV_EPSILON {
        return None;
    }
    let reciprocal = 1.0 / area;
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
    let normal = normalize3(normal);
    let tangent = normalize3(sub3(
        tangent_basis,
        scale3(normal, dot3(normal, tangent_basis)),
    ));
    let bitangent = normalize3(sub3(
        bitangent_basis,
        scale3(normal, dot3(normal, bitangent_basis)),
    ));
    if !tangent.iter().all(|value| value.is_finite()) || length3(tangent) <= UV_EPSILON {
        return None;
    }
    let sign = if dot3(cross3(normal, tangent), bitangent) < 0.0 {
        -1.0
    } else {
        1.0
    };
    Some([tangent[0], tangent[1], tangent[2], sign])
}

fn count_uv0_overlaps(triangles: &[TriangleRecord]) -> Result<u64, GeometryError> {
    count_overlaps(triangles, |triangle| triangle.uv0)
}

fn count_uv1_overlaps(triangles: &[TriangleRecord]) -> Result<u64, GeometryError> {
    count_overlaps(triangles, |triangle| triangle.uv1)
}

fn count_overlaps<F>(triangles: &[TriangleRecord], mut channel: F) -> Result<u64, GeometryError>
where
    F: FnMut(&TriangleRecord) -> [[f32; 2]; 3],
{
    const GRID: usize = 64;
    let mut buckets = BTreeMap::<(usize, usize), Vec<usize>>::new();
    for (index, triangle) in triangles.iter().enumerate() {
        let uv = channel(triangle);
        let min_u = uv
            .iter()
            .map(|point| point[0])
            .fold(f32::INFINITY, f32::min);
        let max_u = uv
            .iter()
            .map(|point| point[0])
            .fold(f32::NEG_INFINITY, f32::max);
        let min_v = uv
            .iter()
            .map(|point| point[1])
            .fold(f32::INFINITY, f32::min);
        let max_v = uv
            .iter()
            .map(|point| point[1])
            .fold(f32::NEG_INFINITY, f32::max);
        let x0 = ((min_u.clamp(0.0, 1.0) * GRID as f32).floor() as usize).min(GRID - 1);
        let x1 = ((max_u.clamp(0.0, 1.0) * GRID as f32).floor() as usize).min(GRID - 1);
        let y0 = ((min_v.clamp(0.0, 1.0) * GRID as f32).floor() as usize).min(GRID - 1);
        let y1 = ((max_v.clamp(0.0, 1.0) * GRID as f32).floor() as usize).min(GRID - 1);
        for x in x0..=x1 {
            for y in y0..=y1 {
                buckets.entry((x, y)).or_default().push(index);
            }
        }
    }
    let mut pairs = BTreeSet::<(usize, usize)>::new();
    for indices in buckets.values() {
        for left in 0..indices.len() {
            for right in (left + 1)..indices.len() {
                let pair = if indices[left] < indices[right] {
                    (indices[left], indices[right])
                } else {
                    (indices[right], indices[left])
                };
                if pairs.len() >= MAX_OVERLAP_COMPARISONS && !pairs.contains(&pair) {
                    return Err(invalid("HERO_UV_OVERLAP_COMPARISON_BUDGET_EXCEEDED"));
                }
                pairs.insert(pair);
            }
        }
    }
    let mut overlap_count = 0u64;
    for (left, right) in pairs {
        let first = channel(&triangles[left]);
        let second = channel(&triangles[right]);
        if integrity::triangle_intersection_area(first, second) > 1.0e-10 {
            overlap_count += 1;
        }
    }
    Ok(overlap_count)
}

fn uv0_corner_values(triangles: &[TriangleRecord]) -> Value {
    Value::Array(
        triangles
            .iter()
            .map(|triangle| {
                json!({
                    "triangle_index": triangle.index,
                    "part_id": triangle.part_id,
                    "material_zone_id": triangle.material_zone_id,
                    "island_id": triangle.uv0_island,
                    "first_person_weight": triangle.first_person_weight,
                    "uv": triangle.uv0
                })
            })
            .collect(),
    )
}

fn uv1_corner_values(triangles: &[TriangleRecord]) -> Value {
    Value::Array(
        triangles
            .iter()
            .map(|triangle| {
                json!({
                    "triangle_index": triangle.index,
                    "part_id": triangle.part_id,
                    "chart_id": triangle.uv1_chart,
                    "uv": triangle.uv1
                })
            })
            .collect(),
    )
}

fn visibility_weight_values(weights: &BTreeMap<String, VisibilityWeight>) -> Value {
    Value::Array(
        weights
            .iter()
            .map(|(part_id, weight)| {
                json!({
                    "part_id": part_id,
                    "first_person": weight.first_person,
                    "world": weight.world,
                    "hidden": weight.hidden
                })
            })
            .collect(),
    )
}

fn island_values(triangles: &[TriangleRecord], resolution: f32) -> Value {
    let mut grouped = BTreeMap::<usize, Vec<&TriangleRecord>>::new();
    for triangle in triangles {
        grouped
            .entry(triangle.uv0_island)
            .or_default()
            .push(triangle);
    }
    Value::Array(
        grouped
            .into_iter()
            .map(|(island_id, members)| {
                let mut min = [f32::INFINITY; 2];
                let mut max = [f32::NEG_INFINITY; 2];
                let mut world_area = 0.0f64;
                let mut uv_area = 0.0f64;
                let mut weighted_world = 0.0f64;
                let mut weighted_uv = 0.0f64;
                for triangle in &members {
                    for uv in triangle.uv0 {
                        min[0] = min[0].min(uv[0]);
                        min[1] = min[1].min(uv[1]);
                        max[0] = max[0].max(uv[0]);
                        max[1] = max[1].max(uv[1]);
                    }
                    world_area += f64::from(triangle.world_area);
                    uv_area += f64::from(triangle.uv0_area);
                    weighted_world += f64::from(triangle.world_area * triangle.first_person_weight);
                    weighted_uv += f64::from(triangle.uv0_area * triangle.first_person_weight);
                }
                json!({
                    "island_id": island_id,
                    "triangle_count": members.len(),
                    "part_ids": members.iter().map(|triangle| triangle.part_id.clone()).collect::<BTreeSet<_>>(),
                    "uv0_bbox": [min, max],
                    "texel_density": texel_density(uv_area, world_area, resolution),
                    "first_person_weighted_texel_density": texel_density(weighted_uv, weighted_world, resolution),
                    "seam_policy": "edge-shared-or-explicit-seam@1"
                })
            })
            .collect(),
    )
}

fn parse_visibility_weights(
    values: &[Value],
) -> Result<BTreeMap<String, VisibilityWeight>, GeometryError> {
    if values.is_empty() || values.len() > MAX_VISIBILITY_WEIGHTS {
        return Err(invalid("HERO_UV_VISIBILITY_WEIGHTS_INVALID"));
    }
    let mut result = BTreeMap::new();
    for value in values {
        let object = value
            .as_object()
            .ok_or_else(|| invalid("HERO_UV_VISIBILITY_WEIGHT_INVALID"))?;
        if object.keys().any(|key| {
            !matches!(
                key.as_str(),
                "part_id" | "first_person" | "world" | "hidden"
            )
        }) || object.len() != 4
        {
            return Err(invalid("HERO_UV_VISIBILITY_WEIGHT_FIELDS_INVALID"));
        }
        let part_id = object
            .get("part_id")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty() && value.len() <= 128)
            .ok_or_else(|| invalid("HERO_UV_VISIBILITY_WEIGHT_PART_INVALID"))?;
        let first_person = finite_weight(object, "first_person")?;
        let world = finite_weight(object, "world")?;
        let hidden = finite_weight(object, "hidden")?;
        if result
            .insert(
                part_id.to_owned(),
                VisibilityWeight {
                    first_person,
                    world,
                    hidden,
                },
            )
            .is_some()
        {
            return Err(invalid("HERO_UV_VISIBILITY_WEIGHT_DUPLICATE_PART"));
        }
    }
    Ok(result)
}

fn validate_visibility_coverage(
    topology: &TopologyMesh,
    weights: &BTreeMap<String, VisibilityWeight>,
) -> Result<(), GeometryError> {
    let parts = topology
        .triangles
        .iter()
        .map(|triangle| triangle.part_id.as_str())
        .collect::<BTreeSet<_>>();
    if parts.len() != weights.len() || parts.iter().any(|part_id| !weights.contains_key(*part_id)) {
        return Err(invalid("HERO_UV_VISIBILITY_WEIGHT_COVERAGE_MISMATCH"));
    }
    Ok(())
}

fn triangle_material_zone<'a>(triangles: &'a [TriangleRecord], index: usize) -> &'a str {
    triangles[index].material_zone_id.as_str()
}

fn best_planar_projection(corners: [[f32; 3]; 3]) -> [[f32; 2]; 3] {
    let projections = [
        corners.map(|point| [point[0], point[1]]),
        corners.map(|point| [point[0], point[2]]),
        corners.map(|point| [point[1], point[2]]),
    ];
    projections
        .into_iter()
        .max_by(|left, right| {
            cross2(sub2(left[1], left[0]), sub2(left[2], left[0]))
                .abs()
                .total_cmp(&cross2(sub2(right[1], right[0]), sub2(right[2], right[0])).abs())
        })
        .unwrap_or(projections[0])
}

fn triangle_stretch(triangle: &TriangleRecord) -> f32 {
    if triangle.uv0_area <= AREA_EPSILON || triangle.world_area <= AREA_EPSILON {
        return f32::INFINITY;
    }
    let area_scale = (triangle.uv0_area / triangle.world_area).sqrt();
    if !area_scale.is_finite() || area_scale <= UV_EPSILON {
        return f32::INFINITY;
    }
    let mut max_ratio = 1.0f32;
    for (left, right) in [(0usize, 1usize), (1, 2), (2, 0)] {
        let world_edge = length3(sub3(triangle.corners[right], triangle.corners[left]));
        let uv_edge = length2(sub2(triangle.uv0[right], triangle.uv0[left]));
        if world_edge <= UV_EPSILON || !world_edge.is_finite() || !uv_edge.is_finite() {
            return f32::INFINITY;
        }
        let ratio = uv_edge / world_edge / area_scale;
        if ratio.is_finite() && ratio > 0.0 {
            max_ratio = max_ratio.max(ratio).max(1.0 / ratio);
        } else {
            return f32::INFINITY;
        }
    }
    max_ratio
}

fn texel_density(uv_area: f64, world_area: f64, resolution: f32) -> f64 {
    if uv_area <= 0.0 || world_area <= 0.0 {
        return 0.0;
    }
    (uv_area / world_area).sqrt() * f64::from(resolution)
}

fn union(parents: &mut [usize], left: usize, right: usize) {
    let left = find(parents, left);
    let right = find(parents, right);
    if left != right {
        parents[left] = right;
    }
}

fn find(parents: &mut [usize], value: usize) -> usize {
    if parents[value] != value {
        let root = find(parents, parents[value]);
        parents[value] = root;
    }
    parents[value]
}

fn position_key(position: [f32; 3]) -> [i64; 3] {
    position.map(|value| (value * POSITION_WELD_SCALE).round() as i64)
}

fn same_uv(left: [f32; 2], right: [f32; 2]) -> bool {
    (left[0] - right[0]).abs() <= UV_EPSILON && (left[1] - right[1]).abs() <= UV_EPSILON
}

fn finite_weight(object: &Map<String, Value>, key: &str) -> Result<f32, GeometryError> {
    let value = object
        .get(key)
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite())
        .map(|value| value as f32)
        .filter(|value| value.is_finite() && (0.0..=1.0).contains(value))
        .ok_or_else(|| invalid("HERO_UV_VISIBILITY_WEIGHT_VALUE_INVALID"))?;
    Ok(value)
}

fn required_finite_f32(payload: &Map<String, Value>, field: &str) -> Result<f32, GeometryError> {
    payload
        .get(field)
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite())
        .map(|value| value as f32)
        .filter(|value| value.is_finite())
        .ok_or_else(|| invalid("HERO_UV_NUMBER_INVALID"))
}

fn required_hash(payload: &Map<String, Value>, field: &str) -> Result<String, GeometryError> {
    let value = payload
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| {
            value.len() == 64
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        })
        .ok_or_else(|| invalid("HERO_UV_HASH_INVALID"))?;
    Ok(value.to_owned())
}

fn require_const(
    payload: &Map<String, Value>,
    field: &str,
    expected: &str,
) -> Result<(), GeometryError> {
    if payload.get(field).and_then(Value::as_str) != Some(expected) {
        return Err(invalid("HERO_UV_SCHEMA_MARKER_INVALID"));
    }
    Ok(())
}

fn wire_canonical_hash(value: &Value) -> Result<String, GeometryError> {
    let bytes = serde_json::to_vec(value)
        .map_err(|_| invalid("HERO_UV_RESULT_CANONICAL_SERIALIZE_FAILED"))?;
    let mut wire: Value = serde_json::from_slice(&bytes)
        .map_err(|_| invalid("HERO_UV_RESULT_CANONICAL_PARSE_FAILED"))?;
    wire["canonical_sha256"] = Value::String(String::new());
    Ok(crate::canonical_hash(&wire))
}

fn invalid(message: impl Into<String>) -> GeometryError {
    GeometryError::Invalid(message.into())
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn sub2(left: [f32; 2], right: [f32; 2]) -> [f32; 2] {
    [left[0] - right[0], left[1] - right[1]]
}

fn cross2(left: [f32; 2], right: [f32; 2]) -> f32 {
    left[0] * right[1] - left[1] * right[0]
}

fn length2(value: [f32; 2]) -> f32 {
    (value[0] * value[0] + value[1] * value[1]).sqrt()
}

fn sub3(left: [f32; 3], right: [f32; 3]) -> [f32; 3] {
    [left[0] - right[0], left[1] - right[1], left[2] - right[2]]
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

fn scale3(value: [f32; 3], scalar: f32) -> [f32; 3] {
    [value[0] * scalar, value[1] * scalar, value[2] * scalar]
}

fn length3(value: [f32; 3]) -> f32 {
    dot3(value, value).sqrt()
}

fn normalize3(value: [f32; 3]) -> [f32; 3] {
    let length = length3(value);
    if !length.is_finite() || length <= UV_EPSILON {
        [0.0, 0.0, 1.0]
    } else {
        scale3(value, 1.0 / length)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::integrity::{TopologyCornerSource, TopologyMesh, TopologyTriangleSource};

    fn triangle(part_id: &str, uv: [[f32; 2]; 3]) -> TopologyTriangleSource {
        TopologyTriangleSource {
            part_id: part_id.to_owned(),
            source_node_id: format!("{part_id}-node"),
            material_zone_id: "zone-test".to_owned(),
            solid: true,
            corners: [
                TopologyCornerSource {
                    position: [0.0, 0.0, 0.0],
                    normal: [0.0, 0.0, 1.0],
                    texcoord_0: uv[0],
                    tangent: [1.0, 0.0, 0.0, 1.0],
                },
                TopologyCornerSource {
                    position: [1.0, 0.0, 0.0],
                    normal: [0.0, 0.0, 1.0],
                    texcoord_0: uv[1],
                    tangent: [1.0, 0.0, 0.0, 1.0],
                },
                TopologyCornerSource {
                    position: [0.0, 1.0, 0.0],
                    normal: [0.0, 0.0, 1.0],
                    texcoord_0: uv[2],
                    tangent: [1.0, 0.0, 0.0, 1.0],
                },
            ],
        }
    }

    #[test]
    fn uv1_is_deterministic_and_has_distinct_triangle_charts() {
        let topology = TopologyMesh {
            triangles: vec![triangle("receiver", [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]])],
        };
        let weights = BTreeMap::from([(
            "receiver".to_owned(),
            VisibilityWeight {
                first_person: 1.0,
                world: 0.5,
                hidden: 0.1,
            },
        )]);
        let mut first = build_triangle_records(&topology, &weights).expect("records");
        assign_uv0_islands(&mut first);
        assign_uv1(&mut first, 2048.0, 8.0).expect("UV1");
        let mut second = build_triangle_records(&topology, &weights).expect("records");
        assign_uv0_islands(&mut second);
        assign_uv1(&mut second, 2048.0, 8.0).expect("UV1");
        assert_eq!(first[0].uv1, second[0].uv1);
        assert!(first[0].uv1.iter().all(|uv| uv[0] > 0.0 && uv[0] < 1.0));
        assert!(first[0].uv1.iter().all(|uv| uv[1] > 0.0 && uv[1] < 1.0));
    }

    #[test]
    fn hard_edge_without_uv_seam_is_reported() {
        let mut left = triangle("receiver", [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]]);
        let mut right = triangle("receiver", [[1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]);
        right.corners[0].position = [1.0, 0.0, 0.0];
        right.corners[1].position = [1.0, 0.0, 1.0];
        right.corners[2].position = [0.0, 1.0, 0.0];
        for corner in &mut right.corners {
            corner.normal = [0.0, 1.0, 0.0];
        }
        // Make the shared edge UV-continuous; only the face-normal break is
        // relevant to this focused diagnostic.
        left.corners[1].texcoord_0 = [1.0, 0.0];
        right.corners[0].texcoord_0 = [1.0, 0.0];
        left.corners[2].texcoord_0 = [0.0, 1.0];
        right.corners[2].texcoord_0 = [0.0, 1.0];
        let topology = TopologyMesh {
            triangles: vec![left, right],
        };
        let weights = BTreeMap::from([(
            "receiver".to_owned(),
            VisibilityWeight {
                first_person: 1.0,
                world: 1.0,
                hidden: 1.0,
            },
        )]);
        let mut records = build_triangle_records(&topology, &weights).expect("records");
        let metrics = classify_edges(&records, 45.0).expect("edge diagnostics");
        assert!(metrics.hard_edge_count >= 1);
        assert!(metrics.hard_edge_without_seam_count >= 1);
        assign_uv0_islands(&mut records);
        assert_eq!(records[0].uv0_island, records[1].uv0_island);
    }

    #[test]
    fn overlap_budget_is_deterministic_and_detects_positive_area() {
        let topology = TopologyMesh {
            triangles: vec![
                triangle("a", [[0.0, 0.0], [0.8, 0.0], [0.0, 0.8]]),
                triangle("b", [[0.1, 0.1], [0.9, 0.1], [0.1, 0.9]]),
            ],
        };
        let weights = BTreeMap::from([
            (
                "a".to_owned(),
                VisibilityWeight {
                    first_person: 1.0,
                    world: 1.0,
                    hidden: 1.0,
                },
            ),
            (
                "b".to_owned(),
                VisibilityWeight {
                    first_person: 1.0,
                    world: 1.0,
                    hidden: 1.0,
                },
            ),
        ]);
        let records = build_triangle_records(&topology, &weights).expect("records");
        assert_eq!(count_uv0_overlaps(&records).expect("overlap count"), 1);
        assert_eq!(count_uv0_overlaps(&records).expect("replay"), 1);
    }
}
