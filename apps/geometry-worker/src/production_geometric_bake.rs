//! Worker-only, deterministic High/Low/Cage geometric bake.
//!
//! This module is intentionally separate from `surface_bake`: that older
//! operation derives a CandidateSurfaceBake from one surface projection and
//! is not a High-to-Low contract.  This operation consumes three independently
//! hash-bound GLBs, reads only strict decoded topology/UV/tangent attributes,
//! and emits fixed-size PNG bytes without Runtime/Store/MCP side effects.

use super::{integrity, GeometryError};
use base64::Engine;
use forgecad_worker_protocol::{
    PRODUCTION_WEAPON_GEOMETRIC_BAKE_AO_SAMPLE_COUNT,
    PRODUCTION_WEAPON_GEOMETRIC_BAKE_ATLAS_POLICY, PRODUCTION_WEAPON_GEOMETRIC_BAKE_BUDGET_PROFILE,
    PRODUCTION_WEAPON_GEOMETRIC_BAKE_NORMAL_CONVENTION, PRODUCTION_WEAPON_GEOMETRIC_BAKE_OPERATION,
    PRODUCTION_WEAPON_GEOMETRIC_BAKE_POLICY,
    PRODUCTION_WEAPON_GEOMETRIC_BAKE_REQUEST_SCHEMA_VERSION,
    PRODUCTION_WEAPON_GEOMETRIC_BAKE_RESOLUTION,
    PRODUCTION_WEAPON_GEOMETRIC_BAKE_RESULT_SCHEMA_VERSION,
};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

const MAX_INPUT_GLB_BYTES: usize = 64 * 1024 * 1024;
const MAX_TOTAL_INPUT_GLB_BYTES: usize = 96 * 1024 * 1024;
const MAX_TOTAL_INPUT_GLB_BASE64_BYTES: usize = MAX_TOTAL_INPUT_GLB_BYTES * 4 / 3 + 12;
const MAX_BAKE_TRIANGLES: usize = 250_000;
const BVH_LEAF_TRIANGLES: usize = 8;
const RAY_EPSILON: f32 = 1.0e-5;
const CAGE_OFFSET_EPSILON: f32 = 1.0e-4;
const POSITION_WELD_SCALE: f32 = 1_000_000.0;
const UV_EPSILON: f32 = 1.0e-5;
const BARYCENTRIC_EPSILON: f32 = 1.0e-6;
const AO_DISTANCE_FACTOR: f32 = 0.18;
const DILATION_TEXELS: usize = 8;
const CAGE_INTERSECTION_EPSILON: f32 = 2.0e-4;
const NORMAL_SKEW_DOT_MIN: f32 = 0.25;
const POSITION_MAP_ENCODING: &str = "world-position-unorm8-global-high-bounds@1";
const ID_MAP_ENCODING: &str = "sorted-u24-palette-rgb8@1";
const DILATION_POLICY: &str = "deterministic-nearest-owner-chebyshev-8@1";
const ALGORITHM_ID: &str =
    "forgecad-geometric-high-low-cage-bake@2|2048|low-uv-raster|bvh-ray-normal-ao-curvature-thickness-position-id|mikktspace-source-tangent|OpenGL+Y|fixed-ao-8|fixed-dilation-8|no-rng-no-time-no-network";

const AO_DIRECTIONS: [[f32; 3]; 8] = [
    [0.0, 0.0, 1.0],
    [0.57735026, 0.57735026, 0.57735026],
    [-0.57735026, 0.57735026, 0.57735026],
    [0.57735026, -0.57735026, 0.57735026],
    [-0.57735026, -0.57735026, 0.57735026],
    [0.8944272, 0.0, 0.4472136],
    [-0.8944272, 0.0, 0.4472136],
    [0.0, 0.8944272, 0.4472136],
];

type GroupKey = (String, String, String, bool);

struct LowCagePair<'a> {
    low: &'a integrity::TopologyTriangleSource,
    cage: &'a integrity::TopologyTriangleSource,
}

#[derive(Clone)]
struct HighTriangle {
    positions: [[f32; 3]; 3],
    corners: [integrity::TopologyCornerSource; 3],
    part_id: String,
    source_node_id: String,
    material_zone_id: String,
    face_normal: [f32; 3],
    curvature: f32,
}

struct HighPart {
    bvh: Bvh,
}

struct HighGeometry {
    parts: BTreeMap<String, HighPart>,
    bounds_min: [f32; 3],
    bounds_max: [f32; 3],
}

struct Bvh {
    triangles: Vec<HighTriangle>,
    nodes: Vec<BvhNode>,
    root: usize,
}

struct BvhNode {
    min: [f32; 3],
    max: [f32; 3],
    left: Option<usize>,
    right: Option<usize>,
    triangle_indices: Vec<usize>,
}

#[derive(Clone, Copy)]
struct RayHit {
    triangle_index: usize,
    t: f32,
    barycentric: [f32; 3],
    ray_direction: [f32; 3],
}

#[derive(Default)]
struct BakeDiagnostic {
    uv_overlap_count: u64,
    ray_sample_count: u64,
    front_ray_test_count: u64,
    back_ray_test_count: u64,
    front_ray_hit_count: u64,
    back_ray_hit_count: u64,
    ray_hit_count: u64,
    ray_miss_count: u64,
    cross_part_hit_count: u64,
    nearest_surface_fallback_count: u64,
    backface_hit_count: u64,
    skew_count: u64,
    penetration_count: u64,
    cage_intersection_count: u64,
    overlap_count: u64,
    out_of_range_count: u64,
    thickness_miss_count: u64,
    ao_ray_count: u64,
    ao_occluded_count: u64,
    curvature_samples: u64,
    max_observed_distance_m: f32,
    distance_histogram: [u64; 8],
}

struct BakeImages {
    normal: Vec<u8>,
    ao: Vec<u8>,
    curvature: Vec<u8>,
    thickness: Vec<u8>,
    position: Vec<u8>,
    object_id: Vec<u8>,
    material_id: Vec<u8>,
    part_id: Vec<u8>,
    primary_covered_pixels: u64,
    dilated_pixels: u64,
    covered_pixels: u64,
    normal_pixels: u64,
    ao_pixels: u64,
    curvature_pixels: u64,
}

struct ShadedPixel {
    normal: [u8; 3],
    ao: u8,
    curvature: u8,
    thickness: u8,
    position: [u8; 3],
    object_id: u32,
    material_id: u32,
    part_id: u32,
}

#[derive(Clone, Debug)]
struct IdPalette {
    object: BTreeMap<String, u32>,
    material: BTreeMap<String, u32>,
    part: BTreeMap<String, u32>,
}

pub fn run(payload: &Map<String, Value>) -> Result<Value, GeometryError> {
    const FIELDS: &[&str] = &[
        "schema_version",
        "bake_policy",
        "bake_policy_sha256",
        "budget_profile",
        "atlas_policy",
        "high_glb_base64",
        "low_glb_base64",
        "cage_glb_base64",
        "high_artifact_sha256",
        "low_artifact_sha256",
        "cage_artifact_sha256",
        "resolution",
        "normal_convention",
        "max_ray_distance_m",
        "ao_sample_count",
        "surface_bake_reuse_allowed",
        "canonical_sha256",
    ];
    super::require_closed_payload(payload, FIELDS)?;

    require_string_const(
        payload,
        "schema_version",
        PRODUCTION_WEAPON_GEOMETRIC_BAKE_REQUEST_SCHEMA_VERSION,
    )?;
    require_string_const(
        payload,
        "bake_policy",
        PRODUCTION_WEAPON_GEOMETRIC_BAKE_POLICY,
    )?;
    require_string_const(
        payload,
        "budget_profile",
        PRODUCTION_WEAPON_GEOMETRIC_BAKE_BUDGET_PROFILE,
    )?;
    require_string_const(
        payload,
        "atlas_policy",
        PRODUCTION_WEAPON_GEOMETRIC_BAKE_ATLAS_POLICY,
    )?;
    let policy_hash = required_hash(payload, "bake_policy_sha256")?;
    if policy_hash != hash_bytes(PRODUCTION_WEAPON_GEOMETRIC_BAKE_POLICY.as_bytes()) {
        return Err(GeometryError::Invalid(
            "geometric bake policy hash does not match".to_owned(),
        ));
    }
    if payload
        .get("surface_bake_reuse_allowed")
        .and_then(Value::as_bool)
        != Some(false)
    {
        return Err(GeometryError::Invalid(
            "geometric bake cannot reuse CandidateSurfaceBake".to_owned(),
        ));
    }
    let canonical = required_hash(payload, "canonical_sha256")?;
    let mut request_without_hash = payload.clone();
    request_without_hash.remove("canonical_sha256");
    if super::canonical_hash(&Value::Object(request_without_hash)) != canonical {
        return Err(GeometryError::Invalid(
            "geometric bake request canonical_sha256 does not match".to_owned(),
        ));
    }
    let resolution = required_u64(payload, "resolution")?;
    if resolution != PRODUCTION_WEAPON_GEOMETRIC_BAKE_RESOLUTION {
        return Err(GeometryError::Invalid(
            "geometric bake resolution must be fixed 2048".to_owned(),
        ));
    }
    require_string_const(
        payload,
        "normal_convention",
        PRODUCTION_WEAPON_GEOMETRIC_BAKE_NORMAL_CONVENTION,
    )?;
    let max_ray_distance = required_finite_f32(payload, "max_ray_distance_m")?;
    if !(max_ray_distance > 0.0 && max_ray_distance <= 100.0) {
        return Err(GeometryError::Invalid(
            "geometric bake max_ray_distance_m is outside its bound".to_owned(),
        ));
    }
    let ao_sample_count = required_u64(payload, "ao_sample_count")?;
    if ao_sample_count != PRODUCTION_WEAPON_GEOMETRIC_BAKE_AO_SAMPLE_COUNT {
        return Err(GeometryError::Invalid(
            "geometric bake AO sample count is fixed at 8".to_owned(),
        ));
    }

    let high_hash = required_hash(payload, "high_artifact_sha256")?;
    let low_hash = required_hash(payload, "low_artifact_sha256")?;
    let cage_hash = required_hash(payload, "cage_artifact_sha256")?;
    if high_hash == low_hash || high_hash == cage_hash || low_hash == cage_hash {
        return Err(GeometryError::Invalid(
            "High, Low and Cage artifact hashes must be distinct".to_owned(),
        ));
    }

    let encoded_total = ["high_glb_base64", "low_glb_base64", "cage_glb_base64"]
        .into_iter()
        .map(|field| {
            payload
                .get(field)
                .and_then(Value::as_str)
                .map(str::len)
                .ok_or_else(|| GeometryError::Invalid(format!("{field} is required")))
        })
        .try_fold(0usize, |total, length| {
            total.checked_add(length?).ok_or_else(|| {
                GeometryError::Invalid("geometric bake encoded input budget overflowed".to_owned())
            })
        })?;
    if encoded_total > MAX_TOTAL_INPUT_GLB_BASE64_BYTES {
        return Err(GeometryError::Invalid(
            "geometric bake encoded inputs exceed the bounded total".to_owned(),
        ));
    }
    let high = decode_glb(payload, "high_glb_base64", high_hash)?;
    let low = decode_glb(payload, "low_glb_base64", low_hash)?;
    let cage = decode_glb(payload, "cage_glb_base64", cage_hash)?;
    let total_bytes = high
        .len()
        .checked_add(low.len())
        .and_then(|value| value.checked_add(cage.len()))
        .ok_or_else(|| {
            GeometryError::Invalid("geometric bake input byte budget overflowed".to_owned())
        })?;
    if total_bytes > MAX_TOTAL_INPUT_GLB_BYTES {
        return Err(GeometryError::Invalid(
            "geometric bake input bytes exceed the bounded total".to_owned(),
        ));
    }

    let high_admission = admit_high_source(&high)?;
    let low_integrity = admit_glb(&low, "Low")?;
    let cage_integrity = admit_glb(&cage, "Cage")?;
    if low_integrity.triangle_count != cage_integrity.triangle_count {
        return Err(GeometryError::Invalid(
            "Low and Cage triangle counts differ before correspondence".to_owned(),
        ));
    }
    if high_admission.topology.triangles.is_empty() || low_integrity.triangle_count == 0 {
        return Err(GeometryError::Invalid(
            "geometric bake requires non-empty High and Low meshes".to_owned(),
        ));
    }
    let HighInputAdmission {
        topology: high_mesh,
        diagnostic: high_diagnostic,
        policy: high_source_policy,
    } = high_admission;
    let low_mesh = integrity::extract_topology_mesh(&low, MAX_BAKE_TRIANGLES)?;
    let cage_mesh = integrity::extract_topology_mesh(&cage, MAX_BAKE_TRIANGLES)?;
    let low_diagnostic = integrity::extract_diagnostic_mesh(&low, MAX_BAKE_TRIANGLES)?;
    let cage_diagnostic = integrity::extract_diagnostic_mesh(&cage, MAX_BAKE_TRIANGLES)?;
    if high_mesh.triangles.len() > MAX_BAKE_TRIANGLES
        || low_mesh.triangles.len() > MAX_BAKE_TRIANGLES
        || cage_mesh.triangles.len() > MAX_BAKE_TRIANGLES
    {
        return Err(GeometryError::Invalid(
            "geometric bake topology exceeds its triangle budget".to_owned(),
        ));
    }

    validate_structural_correspondence(&high_diagnostic, &low_diagnostic, &cage_diagnostic)?;
    let pairs = pair_low_and_cage(&low_mesh, &cage_mesh)?;
    let low_parts = low_mesh
        .triangles
        .iter()
        .map(|triangle| triangle.part_id.as_str())
        .collect::<BTreeSet<_>>();
    let high_parts = high_mesh
        .triangles
        .iter()
        .map(|triangle| triangle.part_id.as_str())
        .collect::<BTreeSet<_>>();
    if low_parts
        .iter()
        .any(|part_id| !high_parts.contains(part_id))
    {
        return Err(GeometryError::Invalid(
            "High mesh does not contain every Low semantic Part".to_owned(),
        ));
    }

    let high_geometry = build_high_geometry(&high_mesh)?;
    let id_palette = build_id_palette(&high_geometry);
    let algorithm_sha256 = hash_bytes(ALGORITHM_ID.as_bytes());
    let bake = rasterize_and_shade_with_diagnostic(
        &pairs,
        &high_geometry,
        &id_palette,
        max_ray_distance,
        resolution as usize,
    )?;
    let normal_png =
        super::encode_rgb8_png(&bake.images.normal, resolution as u32, resolution as u32)?;
    let ao_png = super::encode_luma8_png(&bake.images.ao, resolution as u32, resolution as u32)?;
    let curvature_png =
        super::encode_luma8_png(&bake.images.curvature, resolution as u32, resolution as u32)?;
    let thickness_png =
        super::encode_luma8_png(&bake.images.thickness, resolution as u32, resolution as u32)?;
    let position_png =
        super::encode_rgb8_png(&bake.images.position, resolution as u32, resolution as u32)?;
    let object_id_png =
        super::encode_rgb8_png(&bake.images.object_id, resolution as u32, resolution as u32)?;
    let material_id_png = super::encode_rgb8_png(
        &bake.images.material_id,
        resolution as u32,
        resolution as u32,
    )?;
    let part_id_png =
        super::encode_rgb8_png(&bake.images.part_id, resolution as u32, resolution as u32)?;
    let normal_sha256 = hash_bytes(&normal_png);
    let ao_sha256 = hash_bytes(&ao_png);
    let curvature_sha256 = hash_bytes(&curvature_png);
    let thickness_sha256 = hash_bytes(&thickness_png);
    let position_sha256 = hash_bytes(&position_png);
    let object_id_sha256 = hash_bytes(&object_id_png);
    let material_id_sha256 = hash_bytes(&material_id_png);
    let part_id_sha256 = hash_bytes(&part_id_png);
    let output_bytes = normal_png
        .len()
        .checked_add(ao_png.len())
        .and_then(|value| value.checked_add(curvature_png.len()))
        .and_then(|value| value.checked_add(thickness_png.len()))
        .and_then(|value| value.checked_add(position_png.len()))
        .and_then(|value| value.checked_add(object_id_png.len()))
        .and_then(|value| value.checked_add(material_id_png.len()))
        .and_then(|value| value.checked_add(part_id_png.len()))
        .ok_or_else(|| {
            GeometryError::Invalid("geometric bake output byte budget overflowed".to_owned())
        })?;
    if output_bytes > forgecad_worker_protocol::MAX_WORKER_RESPONSE_BYTES {
        return Err(GeometryError::Invalid(
            "geometric bake PNG output exceeds the bounded total".to_owned(),
        ));
    }
    let distance_histogram = Value::Array(
        bake.diagnostic
            .distance_histogram
            .iter()
            .map(|value| Value::from(*value))
            .collect(),
    );
    let distance_histogram_sha256 = super::canonical_hash(&distance_histogram);
    let diagnostic_heatmap = json!({
        "uv_overlap_count":bake.diagnostic.uv_overlap_count,
        "ray_miss_count":bake.diagnostic.ray_miss_count,
        "nearest_surface_fallback_count":bake.diagnostic.nearest_surface_fallback_count,
        "cross_part_hit_count":bake.diagnostic.cross_part_hit_count,
        "backface_hit_count":bake.diagnostic.backface_hit_count,
        "skew_count":bake.diagnostic.skew_count,
        "penetration_count":bake.diagnostic.penetration_count,
        "cage_intersection_count":bake.diagnostic.cage_intersection_count,
        "overlap_count":bake.diagnostic.overlap_count,
        "out_of_range_count":bake.diagnostic.out_of_range_count,
        "thickness_miss_count":bake.diagnostic.thickness_miss_count,
        "dilated_pixels":bake.images.dilated_pixels,
        "distance_histogram_sha256":distance_histogram_sha256
    });
    let diagnostic_heatmap_sha256 = super::canonical_hash(&diagnostic_heatmap);
    let status = if bake.diagnostic.ray_miss_count == 0
        && bake.diagnostic.nearest_surface_fallback_count == 0
        && bake.diagnostic.cross_part_hit_count == 0
        && bake.diagnostic.backface_hit_count == 0
        && bake.diagnostic.skew_count == 0
        && bake.diagnostic.penetration_count == 0
        && bake.diagnostic.cage_intersection_count == 0
        && bake.diagnostic.overlap_count == 0
        && bake.diagnostic.out_of_range_count == 0
        && bake.diagnostic.thickness_miss_count == 0
        && bake.diagnostic.uv_overlap_count == 0
        && bake.images.primary_covered_pixels > 0
    {
        "PASS_SOURCE_STRUCTURAL"
    } else {
        // Maps are still returned for bounded diagnosis, but a miss/fallback,
        // foreign-Part hit, or any cage/UV diagnostic is never a bake pass.
        "FAILED"
    };

    let mut result = json!({
        "schema_version":PRODUCTION_WEAPON_GEOMETRIC_BAKE_RESULT_SCHEMA_VERSION,
        "operation":PRODUCTION_WEAPON_GEOMETRIC_BAKE_OPERATION,
        "bake_policy":PRODUCTION_WEAPON_GEOMETRIC_BAKE_POLICY,
        "bake_policy_sha256":hash_bytes(PRODUCTION_WEAPON_GEOMETRIC_BAKE_POLICY.as_bytes()),
        "budget_profile":PRODUCTION_WEAPON_GEOMETRIC_BAKE_BUDGET_PROFILE,
        "resolution":resolution,
        "normal_convention":PRODUCTION_WEAPON_GEOMETRIC_BAKE_NORMAL_CONVENTION,
        "atlas_policy":PRODUCTION_WEAPON_GEOMETRIC_BAKE_ATLAS_POLICY,
        "high_artifact_sha256":high_hash,
        "low_artifact_sha256":low_hash,
        "cage_artifact_sha256":cage_hash,
        "output_semantics":[
            "tangent-normal","ao","curvature","thickness","position","object-id","material-id","part-id"
        ],
        "ray_origin_policy":"cage-barycentric-surface-plus-minus-epsilon@1",
        "ray_direction_policy":"cage-face-normal-front-and-back@1",
        "ray_distance_policy":"bounded-positive-nearest-hit@1",
        "front_back_policy":"two-sided-front-back@1",
        "per_part_isolation_policy":"same-semantic-part-only@1",
        "anti_cross_hit_policy":"reject-nearer-foreign-part@2",
        "max_ray_distance_m":max_ray_distance,
        "padding_texels":DILATION_TEXELS,
        "dilation_texels":DILATION_TEXELS,
        "dilation_policy":DILATION_POLICY,
        "position_map_encoding":POSITION_MAP_ENCODING,
        "id_map_encoding":ID_MAP_ENCODING,
        "normal_png_base64":base64::engine::general_purpose::STANDARD.encode(&normal_png),
        "normal_png_sha256":normal_sha256,
        "tangent_normal_png_base64":base64::engine::general_purpose::STANDARD.encode(&normal_png),
        "tangent_normal_png_sha256":normal_sha256,
        "ao_png_base64":base64::engine::general_purpose::STANDARD.encode(&ao_png),
        "ao_png_sha256":ao_sha256,
        "curvature_png_base64":base64::engine::general_purpose::STANDARD.encode(&curvature_png),
        "curvature_png_sha256":curvature_sha256,
        "thickness_png_base64":base64::engine::general_purpose::STANDARD.encode(&thickness_png),
        "thickness_png_sha256":thickness_sha256,
        "position_png_base64":base64::engine::general_purpose::STANDARD.encode(&position_png),
        "position_png_sha256":position_sha256,
        "object_id_png_base64":base64::engine::general_purpose::STANDARD.encode(&object_id_png),
        "object_id_png_sha256":object_id_sha256,
        "material_id_png_base64":base64::engine::general_purpose::STANDARD.encode(&material_id_png),
        "material_id_png_sha256":material_id_sha256,
        "part_id_png_base64":base64::engine::general_purpose::STANDARD.encode(&part_id_png),
        "part_id_png_sha256":part_id_sha256,
        "maps":{
            "tangent-normal":png_map_value("tangent-normal", &normal_png, normal_sha256, "rgb8-unorm"),
            "ao":png_map_value("ao", &ao_png, ao_sha256, "luma8-unorm"),
            "curvature":png_map_value("curvature", &curvature_png, curvature_sha256, "luma8-unorm"),
            "thickness":png_map_value("thickness", &thickness_png, thickness_sha256, "luma8-unorm"),
            "position":png_map_value("position", &position_png, position_sha256, POSITION_MAP_ENCODING),
            "object-id":png_map_value("object-id", &object_id_png, object_id_sha256, ID_MAP_ENCODING),
            "material-id":png_map_value("material-id", &material_id_png, material_id_sha256, ID_MAP_ENCODING),
            "part-id":png_map_value("part-id", &part_id_png, part_id_sha256, ID_MAP_ENCODING)
        },
        "id_palette":id_palette_value(&id_palette),
        "position_bounds":{
            "min":high_geometry.bounds_min,
            "max":high_geometry.bounds_max
        },
        "coverage":{
            "atlas_pixels":bake.images.normal.len() as u64 / 3,
            "primary_covered_pixels":bake.images.primary_covered_pixels,
            "dilated_pixels":bake.images.dilated_pixels,
            "covered_pixels":bake.images.covered_pixels,
            "coverage_ratio":bake.images.covered_pixels as f64 / (resolution * resolution) as f64,
            "normal_pixels":bake.images.normal_pixels,
            "ao_pixels":bake.images.ao_pixels,
            "curvature_pixels":bake.images.curvature_pixels,
            "thickness_pixels":bake.images.covered_pixels,
            "position_pixels":bake.images.covered_pixels,
            "object_id_pixels":bake.images.covered_pixels,
            "material_id_pixels":bake.images.covered_pixels,
            "part_id_pixels":bake.images.covered_pixels
        },
        "diagnostic":{
            "status":status,
            "uv_overlap_count":bake.diagnostic.uv_overlap_count,
            "ray_sample_count":bake.diagnostic.ray_sample_count,
            "front_ray_test_count":bake.diagnostic.front_ray_test_count,
            "back_ray_test_count":bake.diagnostic.back_ray_test_count,
            "front_ray_hit_count":bake.diagnostic.front_ray_hit_count,
            "back_ray_hit_count":bake.diagnostic.back_ray_hit_count,
            "ray_hit_count":bake.diagnostic.ray_hit_count,
            "ray_miss_count":bake.diagnostic.ray_miss_count,
            "cross_part_hit_count":bake.diagnostic.cross_part_hit_count,
            "nearest_surface_fallback_count":bake.diagnostic.nearest_surface_fallback_count,
            "backface_hit_count":bake.diagnostic.backface_hit_count,
            "skew_count":bake.diagnostic.skew_count,
            "penetration_count":bake.diagnostic.penetration_count,
            "cage_intersection_count":bake.diagnostic.cage_intersection_count,
            "overlap_count":bake.diagnostic.overlap_count,
            "out_of_range_count":bake.diagnostic.out_of_range_count,
            "thickness_miss_count":bake.diagnostic.thickness_miss_count,
            "ao_sample_count":PRODUCTION_WEAPON_GEOMETRIC_BAKE_AO_SAMPLE_COUNT,
            "ao_ray_count":bake.diagnostic.ao_ray_count,
            "ao_occluded_count":bake.diagnostic.ao_occluded_count,
            "curvature_samples":bake.diagnostic.curvature_samples,
            "max_observed_distance_m":bake.diagnostic.max_observed_distance_m,
            "distance_histogram":distance_histogram,
            "distance_histogram_sha256":distance_histogram_sha256,
            "diagnostic_heatmap":diagnostic_heatmap,
            "diagnostic_heatmap_sha256":diagnostic_heatmap_sha256,
            "high_triangle_count":high_mesh.triangles.len(),
            "low_triangle_count":low_mesh.triangles.len(),
            "cage_triangle_count":cage_mesh.triangles.len(),
            "tangent_source":"Low decoded GLB TANGENT admitted by strict integrity/MikkTSpace compiler path; direct V2 High tangent optional",
            "tangent_normal_semantics":"low tangent frame with high normal transfer",
            "padding_texels":DILATION_TEXELS,
            "dilation_texels":DILATION_TEXELS,
            "dilation_policy":DILATION_POLICY,
            "surface_bake_reuse_allowed":false,
            "quality_gate":"NOT_RUN"
        },
        "formal_quality_gate":"NOT_RUN",
        "worker_algorithm_id":ALGORITHM_ID,
        "worker_algorithm_sha256":algorithm_sha256,
        "surface_bake_reuse_allowed":false,
        "runtime_write_performed":false,
        "production_stage_advanced":false,
        "canonical_sha256":""
    });
    result["high_source_policy"] = Value::String(high_source_policy.to_owned());
    result["high_tangent_policy"] =
        Value::String("not-required-for-high-projection-low-tangent-owned@1".to_owned());
    result["canonical_sha256"] = Value::String(wire_canonical_hash(&result)?);
    if serde_json::to_vec(&result)
        .map_err(|_| {
            GeometryError::Invalid("geometric bake result cannot be serialized".to_owned())
        })?
        .len()
        > forgecad_worker_protocol::MAX_WORKER_RESPONSE_BYTES
    {
        return Err(GeometryError::Invalid(
            "geometric bake result exceeds the bounded Worker response budget".to_owned(),
        ));
    }
    Ok(result)
}

fn require_string_const(
    payload: &Map<String, Value>,
    field: &str,
    expected: &str,
) -> Result<(), GeometryError> {
    let value = payload
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| GeometryError::Invalid(format!("{field} is required")))?;
    if value != expected {
        return Err(GeometryError::Invalid(format!("{field} is invalid")));
    }
    Ok(())
}

fn required_u64(payload: &Map<String, Value>, field: &str) -> Result<u64, GeometryError> {
    payload
        .get(field)
        .and_then(Value::as_u64)
        .ok_or_else(|| GeometryError::Invalid(format!("{field} is invalid")))
}

fn required_finite_f32(payload: &Map<String, Value>, field: &str) -> Result<f32, GeometryError> {
    let value = payload
        .get(field)
        .and_then(Value::as_f64)
        .ok_or_else(|| GeometryError::Invalid(format!("{field} is invalid")))?;
    if !value.is_finite() {
        return Err(GeometryError::Invalid(format!("{field} is non-finite")));
    }
    let value = value as f32;
    if !value.is_finite() {
        return Err(GeometryError::Invalid(format!(
            "{field} is outside f32 bounds"
        )));
    }
    Ok(value)
}

fn required_hash<'a>(
    payload: &'a Map<String, Value>,
    field: &str,
) -> Result<&'a str, GeometryError> {
    let value = payload
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| GeometryError::Invalid(format!("{field} is required")))?;
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(GeometryError::Invalid(format!("{field} is not a SHA-256")));
    }
    Ok(value)
}

fn decode_glb<'a>(
    payload: &'a Map<String, Value>,
    field: &str,
    expected_hash: &str,
) -> Result<Vec<u8>, GeometryError> {
    let encoded = payload
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| GeometryError::Invalid(format!("{field} is required")))?;
    if encoded.is_empty() || encoded.len() > (MAX_INPUT_GLB_BYTES * 4 / 3 + 4) {
        return Err(GeometryError::Invalid(format!("{field} exceeds its bound")));
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|error| GeometryError::Invalid(format!("{field} is invalid base64: {error}")))?;
    if bytes.is_empty() || bytes.len() > MAX_INPUT_GLB_BYTES {
        return Err(GeometryError::Invalid(format!(
            "{field} decoded bytes exceed its bound"
        )));
    }
    let actual_hash = hash_bytes(&bytes);
    if actual_hash != expected_hash {
        return Err(GeometryError::Invalid(format!(
            "{field} SHA-256 does not match"
        )));
    }
    Ok(bytes)
}

fn admit_glb(bytes: &[u8], label: &str) -> Result<integrity::GlbIntegrity, GeometryError> {
    let inspection = integrity::inspect_glb(bytes)?;
    if !inspection.hard_gate_passed {
        return Err(GeometryError::Invalid(format!(
            "{label} GLB failed strict integrity: {}",
            inspection.failure_codes.join(",")
        )));
    }
    if inspection.artifact_schema_version != "ArtifactReadback@2" {
        return Err(GeometryError::Invalid(format!(
            "{label} GLB must be ArtifactReadback@2"
        )));
    }
    if inspection.uv_non_finite_count != 0
        || inspection.zero_area_uv_triangle_count != 0
        || inspection.tangent_non_finite_count != 0
        || inspection.tangent_orthogonality_error_count != 0
        || inspection.tangent_handedness_error_count != 0
    {
        return Err(GeometryError::Invalid(format!(
            "{label} GLB UV/tangent integrity is not admissible"
        )));
    }
    Ok(inspection)
}

struct HighInputAdmission {
    topology: integrity::TopologyMesh,
    diagnostic: integrity::DiagnosticMesh,
    policy: &'static str,
}

/// Admit either the established `ArtifactReadback@2` High or the direct V2
/// High artifact.  The direct V2 source intentionally requires only
/// POSITION/NORMAL/TEXCOORD_0 (TANGENT is optional); Low/Cage remain on the
/// strict `admit_glb` path above.  High tangent bytes are not used by the ray
/// projection: tangent-space normal encoding is owned by the admitted Low
/// tangent field in `shade_pixel`.
fn admit_high_source(bytes: &[u8]) -> Result<HighInputAdmission, GeometryError> {
    match integrity::inspect_glb(bytes) {
        Ok(inspection) => {
            if !inspection.hard_gate_passed {
                return Err(GeometryError::Invalid(format!(
                    "High GLB failed strict integrity: {}",
                    inspection.failure_codes.join(",")
                )));
            }
            if inspection.artifact_schema_version != "ArtifactReadback@2" {
                return Err(GeometryError::Invalid(
                    "High GLB has an unsupported admitted artifact schema".to_owned(),
                ));
            }
            let topology = integrity::extract_topology_mesh(bytes, MAX_BAKE_TRIANGLES)?;
            let diagnostic = integrity::extract_diagnostic_mesh(bytes, MAX_BAKE_TRIANGLES)?;
            Ok(HighInputAdmission {
                topology,
                diagnostic,
                policy: "artifact-readback-v2-position-normal-uv0-tangent@1",
            })
        }
        Err(strict_error) => {
            let (topology, diagnostic) = extract_direct_v2_high_source(bytes).map_err(|v2_error| {
                GeometryError::Invalid(format!(
                    "High GLB is neither strict ArtifactReadback@2 nor direct V2 source: strict={strict_error}; v2={v2_error}"
                ))
            })?;
            Ok(HighInputAdmission {
                topology,
                diagnostic,
                policy: "direct-v2-high-position-normal-uv0-optional-tangent@1",
            })
        }
    }
}

/// Decode only the bounded direct V2 High surface contract.  This parser is
/// deliberately local to the geometric bake Worker: it does not weaken the
/// shared GLB integrity gate used by Low/Cage and it never writes or augments
/// the High bytes.  A neutral tangent value is stored only because the shared
/// `TopologyCornerSource` shape is also used by Low/Cage; High projection code
/// never reads that field.
fn extract_direct_v2_high_source(
    bytes: &[u8],
) -> Result<(integrity::TopologyMesh, integrity::DiagnosticMesh), GeometryError> {
    const MAX_DIRECT_HIGH_BYTES: usize = 64 * 1024 * 1024;
    if bytes.is_empty() || bytes.len() > MAX_DIRECT_HIGH_BYTES {
        return Err(GeometryError::Invalid(
            "direct V2 High source exceeds its byte budget".to_owned(),
        ));
    }
    let (root, binary) = parse_direct_high_glb(bytes)?;
    let asset = root
        .get("asset")
        .and_then(Value::as_object)
        .ok_or_else(|| GeometryError::Invalid("direct V2 High asset is missing".to_owned()))?;
    if asset.get("version").and_then(Value::as_str) != Some("2.0") {
        return Err(GeometryError::Invalid(
            "direct V2 High asset version is invalid".to_owned(),
        ));
    }
    let forgecad = root
        .get("extras")
        .and_then(|value| value.get("forgecad"))
        .and_then(Value::as_object)
        .ok_or_else(|| {
            GeometryError::Invalid("direct V2 High ForgeCAD lineage is missing".to_owned())
        })?;
    if forgecad.get("schema_version").and_then(Value::as_str) != Some("HighMeshArtifactGlb@1")
        || forgecad
            .get("source_schema_version")
            .and_then(Value::as_str)
            != Some("HighMeshArtifact@1")
    {
        return Err(GeometryError::Invalid(
            "direct V2 High lineage schema is invalid".to_owned(),
        ));
    }
    let root_source_hash = forgecad
        .get("source_artifact_sha256")
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .ok_or_else(|| {
            GeometryError::Invalid("direct V2 High source artifact hash is invalid".to_owned())
        })?;
    if forgecad.get("embedded_only") != Some(&Value::Bool(true))
        || forgecad.get("external_uri") != Some(&Value::Bool(false))
        || forgecad.get("scripts") != Some(&Value::Bool(false))
    {
        return Err(GeometryError::Invalid(
            "direct V2 High source must be embedded-only and script-free".to_owned(),
        ));
    }
    let buffers = root
        .get("buffers")
        .and_then(Value::as_array)
        .ok_or_else(|| GeometryError::Invalid("direct V2 High buffers are missing".to_owned()))?;
    if buffers.len() != 1
        || buffers[0].get("uri").is_some()
        || buffers[0].get("byteLength").and_then(Value::as_u64) != Some(binary.len() as u64)
    {
        return Err(GeometryError::Invalid(
            "direct V2 High buffer is not embedded".to_owned(),
        ));
    }
    let meshes = root
        .get("meshes")
        .and_then(Value::as_array)
        .filter(|values| !values.is_empty())
        .ok_or_else(|| GeometryError::Invalid("direct V2 High meshes are missing".to_owned()))?;
    let nodes = root
        .get("nodes")
        .and_then(Value::as_array)
        .ok_or_else(|| GeometryError::Invalid("direct V2 High nodes are missing".to_owned()))?;
    let accessors = root
        .get("accessors")
        .and_then(Value::as_array)
        .ok_or_else(|| GeometryError::Invalid("direct V2 High accessors are missing".to_owned()))?;
    let views = root
        .get("bufferViews")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            GeometryError::Invalid("direct V2 High bufferViews are missing".to_owned())
        })?;
    let materials = root
        .get("materials")
        .and_then(Value::as_array)
        .ok_or_else(|| GeometryError::Invalid("direct V2 High materials are missing".to_owned()))?;

    let mut topology_triangles = Vec::new();
    let mut diagnostic_primitives = Vec::new();
    let mut diagnostic_triangle_count = 0usize;
    for (mesh_index, mesh) in meshes.iter().enumerate() {
        let mesh_object = mesh.as_object().ok_or_else(|| {
            GeometryError::Invalid("direct V2 High mesh is not an object".to_owned())
        })?;
        let mesh_lineage = mesh_object.get("extras").and_then(Value::as_object);
        let matching_nodes = nodes
            .iter()
            .filter_map(Value::as_object)
            .filter(|node| node.get("mesh").and_then(Value::as_u64) == Some(mesh_index as u64))
            .collect::<Vec<_>>();
        if matching_nodes.len() != 1 {
            return Err(GeometryError::Invalid(
                "direct V2 High mesh/node lineage is ambiguous".to_owned(),
            ));
        }
        let node_lineage = matching_nodes[0].get("extras").and_then(Value::as_object);
        let primitives = mesh_object
            .get("primitives")
            .and_then(Value::as_array)
            .filter(|values| !values.is_empty())
            .ok_or_else(|| {
                GeometryError::Invalid("direct V2 High primitive list is missing".to_owned())
            })?;
        for primitive in primitives {
            let primitive_object = primitive.as_object().ok_or_else(|| {
                GeometryError::Invalid("direct V2 High primitive is not an object".to_owned())
            })?;
            if primitive_object
                .get("mode")
                .and_then(Value::as_u64)
                .unwrap_or(4)
                != 4
            {
                return Err(GeometryError::Invalid(
                    "direct V2 High primitive must use triangle mode".to_owned(),
                ));
            }
            let primitive_lineage = primitive_object.get("extras").and_then(Value::as_object);
            let holders = [mesh_lineage, node_lineage, primitive_lineage];
            let part_id = direct_high_lineage_text(&holders, "part_id")?;
            let source_node_id = direct_high_lineage_text(&holders, "source_node_id")?;
            let material_zone_id = direct_high_lineage_text(&holders, "material_zone_id")?;
            let primitive_source_hash =
                direct_high_lineage_text(&holders, "source_artifact_sha256")?;
            if primitive_source_hash != root_source_hash {
                return Err(GeometryError::Invalid(
                    "direct V2 High primitive source hash differs from root".to_owned(),
                ));
            }
            let material_index = primitive_object
                .get("material")
                .and_then(Value::as_u64)
                .and_then(|value| usize::try_from(value).ok())
                .ok_or_else(|| {
                    GeometryError::Invalid("direct V2 High material index is missing".to_owned())
                })?;
            if materials
                .get(material_index)
                .and_then(Value::as_object)
                .and_then(|material| material.get("name"))
                .and_then(Value::as_str)
                != Some(material_zone_id.as_str())
            {
                return Err(GeometryError::Invalid(
                    "direct V2 High material lineage does not match".to_owned(),
                ));
            }
            let attributes = primitive_object
                .get("attributes")
                .and_then(Value::as_object)
                .ok_or_else(|| {
                    GeometryError::Invalid("direct V2 High attributes are missing".to_owned())
                })?;
            if attributes.keys().any(|key| {
                !matches!(
                    key.as_str(),
                    "POSITION" | "NORMAL" | "TEXCOORD_0" | "TANGENT"
                )
            }) || !attributes.contains_key("POSITION")
                || !attributes.contains_key("NORMAL")
                || !attributes.contains_key("TEXCOORD_0")
            {
                return Err(GeometryError::Invalid(
                    "direct V2 High requires POSITION/NORMAL/TEXCOORD_0 with optional TANGENT"
                        .to_owned(),
                ));
            }
            let positions = direct_high_vec3(
                accessors,
                views,
                binary,
                direct_high_index(attributes, "POSITION")?,
            )?;
            let normals = direct_high_vec3(
                accessors,
                views,
                binary,
                direct_high_index(attributes, "NORMAL")?,
            )?;
            let texcoords = direct_high_vec2(
                accessors,
                views,
                binary,
                direct_high_index(attributes, "TEXCOORD_0")?,
            )?;
            if positions.is_empty()
                || positions.len() != normals.len()
                || positions.len() != texcoords.len()
                || positions.len() > MAX_BAKE_TRIANGLES.saturating_mul(3)
            {
                return Err(GeometryError::Invalid(
                    "direct V2 High surface attribute counts differ".to_owned(),
                ));
            }
            let optional_tangents = attributes
                .get("TANGENT")
                .map(|value| {
                    let tangent_index = value
                        .as_u64()
                        .and_then(|value| usize::try_from(value).ok())
                        .ok_or_else(|| {
                            GeometryError::Invalid(
                                "direct V2 High optional tangent accessor is invalid".to_owned(),
                            )
                        })?;
                    direct_high_vec4(accessors, views, binary, tangent_index)
                })
                .transpose()?;
            if optional_tangents
                .as_ref()
                .is_some_and(|values| values.len() != positions.len())
            {
                return Err(GeometryError::Invalid(
                    "direct V2 High optional tangent count differs".to_owned(),
                ));
            }
            let indices = direct_high_indices(
                accessors,
                views,
                binary,
                primitive_object
                    .get("indices")
                    .and_then(Value::as_u64)
                    .and_then(|value| usize::try_from(value).ok())
                    .ok_or_else(|| {
                        GeometryError::Invalid("direct V2 High indices are missing".to_owned())
                    })?,
            )?;
            if indices.is_empty() || indices.len() % 3 != 0 {
                return Err(GeometryError::Invalid(
                    "direct V2 High indices are not triangles".to_owned(),
                ));
            }
            diagnostic_triangle_count = diagnostic_triangle_count
                .checked_add(indices.len() / 3)
                .ok_or_else(|| {
                    GeometryError::Invalid("direct V2 High triangle count overflowed".to_owned())
                })?;
            if diagnostic_triangle_count > MAX_BAKE_TRIANGLES
                || indices
                    .iter()
                    .any(|index| *index as usize >= positions.len())
            {
                return Err(GeometryError::Invalid(
                    "direct V2 High triangle budget or index range is invalid".to_owned(),
                ));
            }
            if positions
                .iter()
                .any(|position| !direct_high_finite3(*position))
                || normals.iter().any(|normal| {
                    !direct_high_finite3(*normal) || direct_high_length3(*normal) <= f32::EPSILON
                })
                || texcoords.iter().any(|uv| !direct_high_finite2(*uv))
            {
                return Err(GeometryError::Invalid(
                    "direct V2 High source contains non-finite surface data".to_owned(),
                ));
            }
            if let Some(tangents) = &optional_tangents {
                if tangents.iter().any(|tangent| {
                    !direct_high_finite4(*tangent) || (tangent[3].abs() - 1.0).abs() > 1.0e-5
                }) {
                    return Err(GeometryError::Invalid(
                        "direct V2 High optional tangent data is invalid".to_owned(),
                    ));
                }
            }
            for triangle in indices.chunks_exact(3) {
                let corner_indices = [
                    triangle[0] as usize,
                    triangle[1] as usize,
                    triangle[2] as usize,
                ];
                let corners = corner_indices.map(|index| integrity::TopologyCornerSource {
                    position: positions[index],
                    normal: normals[index],
                    texcoord_0: texcoords[index],
                    tangent: optional_tangents
                        .as_ref()
                        .map(|values| values[index])
                        .unwrap_or([0.0, 0.0, 0.0, 1.0]),
                });
                let face = direct_high_cross(
                    direct_high_sub(corners[1].position, corners[0].position),
                    direct_high_sub(corners[2].position, corners[0].position),
                );
                if direct_high_length3(face) <= 1.0e-12 {
                    return Err(GeometryError::Invalid(
                        "direct V2 High source contains a degenerate triangle".to_owned(),
                    ));
                }
                topology_triangles.push(integrity::TopologyTriangleSource {
                    part_id: part_id.clone(),
                    corners,
                    source_node_id: source_node_id.clone(),
                    material_zone_id: material_zone_id.clone(),
                    solid: false,
                });
            }
            diagnostic_primitives.push(integrity::DiagnosticPrimitive {
                part_id,
                source_node_id,
                material_zone_id,
                solid: false,
                positions,
                indices,
            });
        }
    }
    if topology_triangles.is_empty() || diagnostic_primitives.is_empty() {
        return Err(GeometryError::Invalid(
            "direct V2 High source contains no triangles".to_owned(),
        ));
    }
    if forgecad.get("triangle_count").and_then(Value::as_u64)
        != Some(diagnostic_triangle_count as u64)
    {
        return Err(GeometryError::Invalid(
            "direct V2 High declared triangle count differs".to_owned(),
        ));
    }
    Ok((
        integrity::TopologyMesh {
            triangles: topology_triangles,
        },
        integrity::DiagnosticMesh {
            primitives: diagnostic_primitives,
            triangle_count: diagnostic_triangle_count,
        },
    ))
}

fn parse_direct_high_glb(bytes: &[u8]) -> Result<(Value, &[u8]), GeometryError> {
    if bytes.len() < 28 || &bytes[..4] != b"glTF" {
        return Err(GeometryError::Invalid(
            "direct V2 High GLB header is invalid".to_owned(),
        ));
    }
    let version =
        u32::from_le_bytes(bytes[4..8].try_into().map_err(|_| {
            GeometryError::Invalid("direct V2 High GLB version is invalid".to_owned())
        })?);
    let total =
        u32::from_le_bytes(bytes[8..12].try_into().map_err(|_| {
            GeometryError::Invalid("direct V2 High GLB length is invalid".to_owned())
        })?) as usize;
    if version != 2 || total != bytes.len() {
        return Err(GeometryError::Invalid(
            "direct V2 High GLB version or length is invalid".to_owned(),
        ));
    }
    let json_length = u32::from_le_bytes(bytes[12..16].try_into().map_err(|_| {
        GeometryError::Invalid("direct V2 High GLB JSON length is invalid".to_owned())
    })?) as usize;
    let json_start = 20usize;
    let json_end = json_start.checked_add(json_length).ok_or_else(|| {
        GeometryError::Invalid("direct V2 High GLB JSON length overflowed".to_owned())
    })?;
    if &bytes[16..20] != b"JSON" || json_end.checked_add(8).is_none_or(|end| end > bytes.len()) {
        return Err(GeometryError::Invalid(
            "direct V2 High GLB JSON chunk is invalid".to_owned(),
        ));
    }
    let binary_length =
        u32::from_le_bytes(bytes[json_end..json_end + 4].try_into().map_err(|_| {
            GeometryError::Invalid("direct V2 High GLB BIN length is invalid".to_owned())
        })?) as usize;
    if &bytes[json_end + 4..json_end + 8] != b"BIN\0"
        || json_end
            .checked_add(8)
            .and_then(|start| start.checked_add(binary_length))
            != Some(bytes.len())
    {
        return Err(GeometryError::Invalid(
            "direct V2 High GLB BIN chunk is invalid".to_owned(),
        ));
    }
    let root = serde_json::from_slice(&bytes[json_start..json_end])
        .map_err(|_| GeometryError::Invalid("direct V2 High GLB JSON is invalid".to_owned()))?;
    Ok((root, &bytes[json_end + 8..]))
}

fn direct_high_lineage_text(
    holders: &[Option<&Map<String, Value>>],
    key: &str,
) -> Result<String, GeometryError> {
    let mut selected = None;
    for holder in holders.iter().flatten() {
        if let Some(value) = holder
            .get(key)
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
        {
            if selected.is_some_and(|previous: &str| previous != value) {
                return Err(GeometryError::Invalid(format!(
                    "direct V2 High lineage field {key} differs between holders"
                )));
            }
            selected = Some(value);
        }
    }
    selected.map(str::to_owned).ok_or_else(|| {
        GeometryError::Invalid(format!("direct V2 High lineage field {key} is missing"))
    })
}

fn direct_high_index(attributes: &Map<String, Value>, key: &str) -> Result<usize, GeometryError> {
    attributes
        .get(key)
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| GeometryError::Invalid(format!("direct V2 High {key} accessor is invalid")))
}

fn direct_high_accessor_window(
    accessors: &[Value],
    views: &[Value],
    binary: &[u8],
    index: usize,
    expected_type: &str,
    expected_component: u64,
    element_size: usize,
) -> Result<(usize, usize, usize), GeometryError> {
    let accessor = accessors
        .get(index)
        .and_then(Value::as_object)
        .ok_or_else(|| GeometryError::Invalid("direct V2 High accessor is invalid".to_owned()))?;
    if accessor.get("type").and_then(Value::as_str) != Some(expected_type)
        || accessor.get("componentType").and_then(Value::as_u64) != Some(expected_component)
        || accessor.get("sparse").is_some()
    {
        return Err(GeometryError::Invalid(
            "direct V2 High accessor layout is invalid".to_owned(),
        ));
    }
    let count = accessor
        .get("count")
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| {
            GeometryError::Invalid("direct V2 High accessor count is invalid".to_owned())
        })?;
    let view_index = accessor
        .get("bufferView")
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| {
            GeometryError::Invalid("direct V2 High accessor view is missing".to_owned())
        })?;
    let view = views
        .get(view_index)
        .and_then(Value::as_object)
        .ok_or_else(|| GeometryError::Invalid("direct V2 High bufferView is invalid".to_owned()))?;
    if view.get("buffer").and_then(Value::as_u64) != Some(0) {
        return Err(GeometryError::Invalid(
            "direct V2 High accessor uses a non-embedded buffer".to_owned(),
        ));
    }
    let view_offset = view.get("byteOffset").and_then(Value::as_u64).unwrap_or(0);
    let view_length = view
        .get("byteLength")
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| {
            GeometryError::Invalid("direct V2 High bufferView length is invalid".to_owned())
        })?;
    let accessor_offset = accessor
        .get("byteOffset")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let accessor_offset = usize::try_from(accessor_offset).map_err(|_| {
        GeometryError::Invalid("direct V2 High accessor offset is invalid".to_owned())
    })?;
    let stride = view
        .get("byteStride")
        .and_then(Value::as_u64)
        .map(|value| usize::try_from(value))
        .transpose()
        .map_err(|_| GeometryError::Invalid("direct V2 High stride is invalid".to_owned()))?
        .unwrap_or(element_size);
    if stride < element_size {
        return Err(GeometryError::Invalid(
            "direct V2 High stride is too small".to_owned(),
        ));
    }
    let start = usize::try_from(view_offset)
        .ok()
        .and_then(|offset| offset.checked_add(accessor_offset))
        .ok_or_else(|| {
            GeometryError::Invalid("direct V2 High accessor offset overflowed".to_owned())
        })?;
    let payload = if count == 0 {
        0
    } else {
        (count - 1)
            .checked_mul(stride)
            .and_then(|value| value.checked_add(element_size))
            .ok_or_else(|| {
                GeometryError::Invalid("direct V2 High accessor range overflowed".to_owned())
            })?
    };
    let end = start.checked_add(payload).ok_or_else(|| {
        GeometryError::Invalid("direct V2 High accessor end overflowed".to_owned())
    })?;
    let view_end = usize::try_from(view_offset)
        .ok()
        .and_then(|offset| offset.checked_add(view_length))
        .ok_or_else(|| {
            GeometryError::Invalid("direct V2 High bufferView range overflowed".to_owned())
        })?;
    if end > view_end || end > binary.len() {
        return Err(GeometryError::Invalid(
            "direct V2 High accessor exceeds BIN".to_owned(),
        ));
    }
    Ok((start, count, stride))
}

fn direct_high_vec2(
    accessors: &[Value],
    views: &[Value],
    binary: &[u8],
    index: usize,
) -> Result<Vec<[f32; 2]>, GeometryError> {
    let (start, count, stride) =
        direct_high_accessor_window(accessors, views, binary, index, "VEC2", 5126, 8)?;
    (0..count)
        .map(|item| {
            let offset = start + item * stride;
            Ok([
                f32::from_le_bytes(binary[offset..offset + 4].try_into().map_err(|_| {
                    GeometryError::Invalid("direct V2 High float bytes are invalid".to_owned())
                })?),
                f32::from_le_bytes(binary[offset + 4..offset + 8].try_into().map_err(|_| {
                    GeometryError::Invalid("direct V2 High float bytes are invalid".to_owned())
                })?),
            ])
        })
        .collect()
}

fn direct_high_vec3(
    accessors: &[Value],
    views: &[Value],
    binary: &[u8],
    index: usize,
) -> Result<Vec<[f32; 3]>, GeometryError> {
    let (start, count, stride) =
        direct_high_accessor_window(accessors, views, binary, index, "VEC3", 5126, 12)?;
    (0..count)
        .map(|item| {
            let offset = start + item * stride;
            Ok([
                direct_high_float(binary, offset)?,
                direct_high_float(binary, offset + 4)?,
                direct_high_float(binary, offset + 8)?,
            ])
        })
        .collect()
}

fn direct_high_vec4(
    accessors: &[Value],
    views: &[Value],
    binary: &[u8],
    index: usize,
) -> Result<Vec<[f32; 4]>, GeometryError> {
    let (start, count, stride) =
        direct_high_accessor_window(accessors, views, binary, index, "VEC4", 5126, 16)?;
    (0..count)
        .map(|item| {
            let offset = start + item * stride;
            Ok([
                direct_high_float(binary, offset)?,
                direct_high_float(binary, offset + 4)?,
                direct_high_float(binary, offset + 8)?,
                direct_high_float(binary, offset + 12)?,
            ])
        })
        .collect()
}

fn direct_high_float(binary: &[u8], offset: usize) -> Result<f32, GeometryError> {
    let bytes = binary.get(offset..offset + 4).ok_or_else(|| {
        GeometryError::Invalid("direct V2 High float accessor exceeds BIN".to_owned())
    })?;
    Ok(f32::from_le_bytes(bytes.try_into().map_err(|_| {
        GeometryError::Invalid("direct V2 High float bytes are invalid".to_owned())
    })?))
}

fn direct_high_indices(
    accessors: &[Value],
    views: &[Value],
    binary: &[u8],
    index: usize,
) -> Result<Vec<u32>, GeometryError> {
    let accessor = accessors
        .get(index)
        .and_then(Value::as_object)
        .ok_or_else(|| {
            GeometryError::Invalid("direct V2 High index accessor is invalid".to_owned())
        })?;
    let component_type = accessor
        .get("componentType")
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            GeometryError::Invalid("direct V2 High index component is missing".to_owned())
        })?;
    let (component_size, expected_component) = match component_type {
        5121 => (1, 5121),
        5123 => (2, 5123),
        5125 => (4, 5125),
        _ => {
            return Err(GeometryError::Invalid(
                "direct V2 High index component is unsupported".to_owned(),
            ))
        }
    };
    let (start, count, stride) = direct_high_accessor_window(
        accessors,
        views,
        binary,
        index,
        "SCALAR",
        expected_component,
        component_size,
    )?;
    (0..count)
        .map(|item| {
            let offset = start + item * stride;
            Ok(match component_size {
                1 => binary[offset] as u32,
                2 => u16::from_le_bytes(binary[offset..offset + 2].try_into().map_err(|_| {
                    GeometryError::Invalid("direct V2 High index bytes are invalid".to_owned())
                })?) as u32,
                _ => u32::from_le_bytes(binary[offset..offset + 4].try_into().map_err(|_| {
                    GeometryError::Invalid("direct V2 High index bytes are invalid".to_owned())
                })?),
            })
        })
        .collect()
}

fn direct_high_finite2(value: [f32; 2]) -> bool {
    value.iter().all(|component| component.is_finite())
}

fn direct_high_finite3(value: [f32; 3]) -> bool {
    value.iter().all(|component| component.is_finite())
}

fn direct_high_finite4(value: [f32; 4]) -> bool {
    value.iter().all(|component| component.is_finite())
}

fn direct_high_length3(value: [f32; 3]) -> f32 {
    (value[0] * value[0] + value[1] * value[1] + value[2] * value[2]).sqrt()
}

fn direct_high_sub(left: [f32; 3], right: [f32; 3]) -> [f32; 3] {
    [left[0] - right[0], left[1] - right[1], left[2] - right[2]]
}

fn direct_high_cross(left: [f32; 3], right: [f32; 3]) -> [f32; 3] {
    [
        left[1] * right[2] - left[2] * right[1],
        left[2] * right[0] - left[0] * right[2],
        left[0] * right[1] - left[1] * right[0],
    ]
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}

fn pair_low_and_cage<'a>(
    low: &'a integrity::TopologyMesh,
    cage: &'a integrity::TopologyMesh,
) -> Result<Vec<LowCagePair<'a>>, GeometryError> {
    if low.triangles.len() != cage.triangles.len() {
        return Err(GeometryError::Invalid(
            "Low/Cage triangle counts differ for exact correspondence".to_owned(),
        ));
    }
    let mut pairs = Vec::with_capacity(low.triangles.len());
    for (ordinal, (low_triangle, cage_triangle)) in
        low.triangles.iter().zip(cage.triangles.iter()).enumerate()
    {
        if group_key(low_triangle) != group_key(cage_triangle) {
            return Err(GeometryError::Invalid(format!(
                "Low/Cage semantic correspondence differs at triangle {ordinal}"
            )));
        }
        let low_positions = low_triangle.corners.clone().map(|corner| corner.position);
        let cage_positions = cage_triangle.corners.clone().map(|corner| corner.position);
        let low_face = cross(
            sub(low_positions[1], low_positions[0]),
            sub(low_positions[2], low_positions[0]),
        );
        let cage_face = cross(
            sub(cage_positions[1], cage_positions[0]),
            sub(cage_positions[2], cage_positions[0]),
        );
        let low_normal = normalize(low_face).ok_or_else(|| {
            GeometryError::Invalid(format!(
                "Low/Cage correspondence has a degenerate Low triangle {ordinal}"
            ))
        })?;
        let cage_normal = normalize(cage_face).ok_or_else(|| {
            GeometryError::Invalid(format!(
                "Low/Cage correspondence has a degenerate Cage triangle {ordinal}"
            ))
        })?;
        if dot(low_normal, cage_normal) <= 0.0 {
            return Err(GeometryError::Invalid(format!(
                "Low/Cage winding differs at triangle {ordinal}"
            )));
        }
        let low_edges = [
            length(sub(low_positions[1], low_positions[0])),
            length(sub(low_positions[2], low_positions[1])),
            length(sub(low_positions[0], low_positions[2])),
        ];
        let cage_edges = [
            length(sub(cage_positions[1], cage_positions[0])),
            length(sub(cage_positions[2], cage_positions[1])),
            length(sub(cage_positions[0], cage_positions[2])),
        ];
        if low_edges
            .into_iter()
            .zip(cage_edges)
            .any(|(low_edge, cage_edge)| {
                !low_edge.is_finite()
                    || !cage_edge.is_finite()
                    || low_edge <= RAY_EPSILON
                    || cage_edge <= RAY_EPSILON
                    || cage_edge < low_edge * 0.05
                    || cage_edge > low_edge * 20.0
            })
        {
            return Err(GeometryError::Invalid(format!(
                "Low/Cage edge correspondence is not bounded at triangle {ordinal}"
            )));
        }
        pairs.push(LowCagePair {
            low: low_triangle,
            cage: cage_triangle,
        });
    }
    if pairs.is_empty() {
        return Err(GeometryError::Invalid(
            "Low/Cage correspondence is empty".to_owned(),
        ));
    }
    Ok(pairs)
}

fn group_key(triangle: &integrity::TopologyTriangleSource) -> GroupKey {
    (
        triangle.part_id.clone(),
        triangle.source_node_id.clone(),
        triangle.material_zone_id.clone(),
        triangle.solid,
    )
}

fn validate_structural_correspondence(
    high: &integrity::DiagnosticMesh,
    low: &integrity::DiagnosticMesh,
    cage: &integrity::DiagnosticMesh,
) -> Result<(), GeometryError> {
    if low.primitives.len() != cage.primitives.len() || low.triangle_count != cage.triangle_count {
        return Err(GeometryError::Invalid(
            "Low/Cage primitive or triangle counts differ for exact correspondence".to_owned(),
        ));
    }
    for (ordinal, (low_primitive, cage_primitive)) in low
        .primitives
        .iter()
        .zip(cage.primitives.iter())
        .enumerate()
    {
        if low_primitive.part_id != cage_primitive.part_id
            || low_primitive.source_node_id != cage_primitive.source_node_id
            || low_primitive.material_zone_id != cage_primitive.material_zone_id
            || low_primitive.solid != cage_primitive.solid
            || low_primitive.positions.len() != cage_primitive.positions.len()
            || low_primitive.indices != cage_primitive.indices
        {
            return Err(GeometryError::Invalid(format!(
                "Low/Cage primitive topology or lineage differs at primitive {ordinal}"
            )));
        }
        if low_primitive
            .positions
            .iter()
            .zip(cage_primitive.positions.iter())
            .map(|(low, cage)| length(sub(*cage, *low)))
            .any(|distance| !distance.is_finite())
        {
            return Err(GeometryError::Invalid(format!(
                "Low/Cage displacement is non-finite at primitive {ordinal}"
            )));
        }
    }
    if semantic_materials(high) != semantic_materials(low) {
        return Err(GeometryError::Invalid(
            "High semantic Part/material-zone set does not match Low".to_owned(),
        ));
    }
    Ok(())
}

fn semantic_materials(mesh: &integrity::DiagnosticMesh) -> BTreeMap<String, BTreeSet<String>> {
    let mut result = BTreeMap::new();
    for primitive in &mesh.primitives {
        result
            .entry(primitive.part_id.clone())
            .or_insert_with(BTreeSet::new)
            .insert(primitive.material_zone_id.clone());
    }
    result
}

fn build_high_geometry(mesh: &integrity::TopologyMesh) -> Result<HighGeometry, GeometryError> {
    let mut grouped = BTreeMap::<String, Vec<HighTriangle>>::new();
    let mut bounds_min = [f32::INFINITY; 3];
    let mut bounds_max = [f32::NEG_INFINITY; 3];
    for triangle in &mesh.triangles {
        let corners = triangle.corners.clone();
        let positions = corners.clone().map(|corner| corner.position);
        let face = cross(
            sub(positions[1], positions[0]),
            sub(positions[2], positions[0]),
        );
        let face_normal = normalize(face).ok_or_else(|| {
            GeometryError::Invalid("High triangle has a zero-length face normal".to_owned())
        })?;
        for position in &positions {
            for axis in 0..3 {
                bounds_min[axis] = bounds_min[axis].min(position[axis]);
                bounds_max[axis] = bounds_max[axis].max(position[axis]);
            }
        }
        grouped
            .entry(triangle.part_id.clone())
            .or_default()
            .push(HighTriangle {
                positions,
                corners,
                part_id: triangle.part_id.clone(),
                source_node_id: triangle.source_node_id.clone(),
                material_zone_id: triangle.material_zone_id.clone(),
                face_normal,
                curvature: 0.0,
            });
    }
    let mut parts = BTreeMap::new();
    for (part_id, mut triangles) in grouped {
        compute_curvature(&mut triangles);
        let bvh = Bvh::new(triangles)?;
        parts.insert(part_id, HighPart { bvh });
    }
    if bounds_min
        .iter()
        .chain(bounds_max.iter())
        .any(|value| !value.is_finite())
    {
        return Err(GeometryError::Invalid(
            "High position bounds are not finite".to_owned(),
        ));
    }
    Ok(HighGeometry {
        parts,
        bounds_min,
        bounds_max,
    })
}

fn build_id_palette(high_geometry: &HighGeometry) -> IdPalette {
    let mut objects = BTreeSet::new();
    let mut materials = BTreeSet::new();
    let mut parts = BTreeSet::new();
    for (part_id, high_part) in &high_geometry.parts {
        parts.insert(part_id.clone());
        for triangle in &high_part.bvh.triangles {
            objects.insert(triangle.source_node_id.clone());
            materials.insert(triangle.material_zone_id.clone());
        }
    }
    IdPalette {
        object: enumerate_palette(objects),
        material: enumerate_palette(materials),
        part: enumerate_palette(parts),
    }
}

fn enumerate_palette(values: BTreeSet<String>) -> BTreeMap<String, u32> {
    values
        .into_iter()
        .enumerate()
        .map(|(index, value)| (value, (index + 1) as u32))
        .collect()
}

fn id_palette_value(palette: &IdPalette) -> Value {
    fn entries(values: &BTreeMap<String, u32>) -> Value {
        Value::Array(
            values
                .iter()
                .map(|(value, id)| json!({"id":id,"value":value}))
                .collect(),
        )
    }
    json!({
        "encoding":ID_MAP_ENCODING,
        "zero":"unassigned",
        "object-id":entries(&palette.object),
        "material-id":entries(&palette.material),
        "part-id":entries(&palette.part)
    })
}

fn png_map_value(semantic: &str, bytes: &[u8], sha256: String, encoding: &str) -> Value {
    json!({
        "semantic":semantic,
        "encoding":encoding,
        "mime":"image/png",
        "resolution":PRODUCTION_WEAPON_GEOMETRIC_BAKE_RESOLUTION,
        "png_base64":base64::engine::general_purpose::STANDARD.encode(bytes),
        "png_sha256":sha256
    })
}

/// Canonicalize after the same JSON wire round-trip used by the isolated
/// Worker envelope.  The map payload is large, so keeping this explicit also
/// makes a replay mismatch fail closed rather than relying on serde's in-memory
/// number representation.
fn wire_canonical_hash(value: &Value) -> Result<String, GeometryError> {
    let bytes = serde_json::to_vec(value).map_err(|_| {
        GeometryError::Invalid("geometric bake result cannot be serialized".to_owned())
    })?;
    let mut wire: Value = serde_json::from_slice(&bytes).map_err(|_| {
        GeometryError::Invalid(
            "geometric bake result cannot be parsed after wire round-trip".to_owned(),
        )
    })?;
    wire["canonical_sha256"] = Value::String(String::new());
    Ok(super::canonical_hash(&wire))
}

fn compute_curvature(triangles: &mut [HighTriangle]) {
    let mut edges = BTreeMap::<([i64; 3], [i64; 3]), Vec<usize>>::new();
    for (index, triangle) in triangles.iter().enumerate() {
        let positions = triangle.positions;
        for (first, second) in [
            (positions[0], positions[1]),
            (positions[1], positions[2]),
            (positions[2], positions[0]),
        ] {
            let first = weld_position(first);
            let second = weld_position(second);
            let key = if first <= second {
                (first, second)
            } else {
                (second, first)
            };
            edges.entry(key).or_default().push(index);
        }
    }
    let mut neighbors = vec![Vec::<usize>::new(); triangles.len()];
    for indices in edges.values() {
        for (offset, first) in indices.iter().enumerate() {
            for second in indices.iter().skip(offset + 1) {
                if first != second {
                    neighbors[*first].push(*second);
                    neighbors[*second].push(*first);
                }
            }
        }
    }
    let face_normals = triangles
        .iter()
        .map(|triangle| triangle.face_normal)
        .collect::<Vec<_>>();
    for (index, triangle) in triangles.iter_mut().enumerate() {
        let mut signal = 0.0;
        let mut count = 0.0;
        for neighbor in &neighbors[index] {
            let dot = dot(triangle.face_normal, face_normals[*neighbor]).clamp(-1.0, 1.0);
            signal += (1.0 - dot) * 0.5;
            count += 1.0;
        }
        triangle.curvature = if count > 0.0 {
            (signal / count).clamp(0.0, 1.0)
        } else {
            0.0
        };
    }
}

impl Bvh {
    fn new(triangles: Vec<HighTriangle>) -> Result<Self, GeometryError> {
        if triangles.is_empty() {
            return Err(GeometryError::Invalid(
                "High semantic Part has no triangles".to_owned(),
            ));
        }
        let mut bvh = Self {
            triangles,
            nodes: Vec::new(),
            root: 0,
        };
        let mut indices = (0..bvh.triangles.len()).collect::<Vec<_>>();
        bvh.root = bvh.build_node(&mut indices)?;
        Ok(bvh)
    }

    fn build_node(&mut self, indices: &mut [usize]) -> Result<usize, GeometryError> {
        let (min, max) = indices_bounds(indices, &self.triangles);
        let node_index = self.nodes.len();
        self.nodes.push(BvhNode {
            min,
            max,
            left: None,
            right: None,
            triangle_indices: Vec::new(),
        });
        if indices.len() <= BVH_LEAF_TRIANGLES {
            self.nodes[node_index].triangle_indices = indices.to_vec();
            return Ok(node_index);
        }
        let extent = sub(max, min);
        let axis = if extent[0] >= extent[1] && extent[0] >= extent[2] {
            0
        } else if extent[1] >= extent[2] {
            1
        } else {
            2
        };
        indices.sort_by(|first, second| {
            let a = centroid(self.triangles[*first].positions)[axis];
            let b = centroid(self.triangles[*second].positions)[axis];
            a.total_cmp(&b).then_with(|| first.cmp(second))
        });
        let middle = indices.len() / 2;
        let (left_indices, right_indices) = indices.split_at_mut(middle);
        let left = self.build_node(left_indices)?;
        let right = self.build_node(right_indices)?;
        self.nodes[node_index].left = Some(left);
        self.nodes[node_index].right = Some(right);
        Ok(node_index)
    }

    fn raycast(&self, origin: [f32; 3], direction: [f32; 3], max_distance: f32) -> Option<RayHit> {
        self.raycast_range(origin, direction, RAY_EPSILON, max_distance)
    }

    fn raycast_range(
        &self,
        origin: [f32; 3],
        direction: [f32; 3],
        min_distance: f32,
        max_distance: f32,
    ) -> Option<RayHit> {
        if !min_distance.is_finite()
            || !max_distance.is_finite()
            || min_distance < 0.0
            || max_distance <= min_distance
        {
            return None;
        }
        let mut stack = vec![self.root];
        let mut best = None;
        let mut best_t = max_distance;
        while let Some(node_index) = stack.pop() {
            let node = &self.nodes[node_index];
            if !ray_aabb(node.min, node.max, origin, direction, best_t) {
                continue;
            }
            if node.left.is_none() && node.right.is_none() {
                for triangle_index in &node.triangle_indices {
                    if let Some(hit) = ray_triangle(
                        &self.triangles[*triangle_index],
                        origin,
                        direction,
                        min_distance,
                        best_t,
                    ) {
                        best_t = hit.t;
                        best = Some(RayHit {
                            triangle_index: *triangle_index,
                            ..hit
                        });
                    }
                }
            } else {
                // Push right first so the lower-indexed left branch is
                // visited first.  The final nearest-t comparison remains the
                // authority even when bounds overlap.
                if let Some(right) = node.right {
                    stack.push(right);
                }
                if let Some(left) = node.left {
                    stack.push(left);
                }
            }
        }
        best
    }

    fn closest_surface(&self, point: [f32; 3], max_distance: f32) -> Option<RayHit> {
        if !max_distance.is_finite() || max_distance < 0.0 {
            return None;
        }
        let mut best_distance_squared = max_distance * max_distance;
        let mut best: Option<RayHit> = None;
        let mut stack = vec![self.root];
        while let Some(node_index) = stack.pop() {
            let node = &self.nodes[node_index];
            if point_aabb_distance_squared(point, node.min, node.max) > best_distance_squared {
                continue;
            }
            if node.left.is_none() && node.right.is_none() {
                for triangle_index in &node.triangle_indices {
                    let triangle = &self.triangles[*triangle_index];
                    let (closest, barycentric) =
                        closest_point_on_triangle(point, triangle.positions);
                    let delta = sub(closest, point);
                    let distance_squared = dot(delta, delta);
                    let replaces = distance_squared < best_distance_squared
                        || (distance_squared == best_distance_squared
                            && best
                                .map(|current| *triangle_index > current.triangle_index)
                                .unwrap_or(true));
                    if replaces {
                        best_distance_squared = distance_squared;
                        best = Some(RayHit {
                            triangle_index: *triangle_index,
                            t: distance_squared.sqrt(),
                            barycentric,
                            ray_direction: [0.0, 0.0, 0.0],
                        });
                    }
                }
            } else {
                // Push right first so the lower-indexed left branch is visited
                // first.  Tie selection above keeps the result stable even
                // when multiple leaves share the same point distance.
                if let Some(right) = node.right {
                    stack.push(right);
                }
                if let Some(left) = node.left {
                    stack.push(left);
                }
            }
        }
        best
    }
}

struct BakeRun {
    images: BakeImages,
    diagnostic: BakeDiagnostic,
}

fn rasterize_and_shade_with_diagnostic(
    pairs: &[LowCagePair<'_>],
    high_geometry: &HighGeometry,
    id_palette: &IdPalette,
    max_ray_distance: f32,
    resolution: usize,
) -> Result<BakeRun, GeometryError> {
    if resolution != PRODUCTION_WEAPON_GEOMETRIC_BAKE_RESOLUTION as usize {
        return Err(GeometryError::Invalid(
            "geometric bake raster resolution is not fixed 2048".to_owned(),
        ));
    }
    let pixel_count = resolution.checked_mul(resolution).ok_or_else(|| {
        GeometryError::Invalid("geometric bake pixel count overflowed".to_owned())
    })?;
    let mut owners = vec![usize::MAX; pixel_count];
    let mut images = BakeImages {
        normal: vec![128; pixel_count * 3],
        ao: vec![255; pixel_count],
        curvature: vec![0; pixel_count],
        thickness: vec![0; pixel_count],
        position: vec![0; pixel_count * 3],
        object_id: vec![0; pixel_count * 3],
        material_id: vec![0; pixel_count * 3],
        part_id: vec![0; pixel_count * 3],
        primary_covered_pixels: 0,
        dilated_pixels: 0,
        covered_pixels: 0,
        normal_pixels: 0,
        ao_pixels: 0,
        curvature_pixels: 0,
    };
    let mut diagnostic = BakeDiagnostic::default();
    for pair in pairs {
        accumulate_topology_diagnostics(pair, &mut diagnostic);
    }
    for (triangle_index, pair) in pairs.iter().enumerate() {
        let uv = pair.low.corners.clone().map(|corner| corner.texcoord_0);
        for coordinate in uv {
            if !coordinate[0].is_finite()
                || !coordinate[1].is_finite()
                || coordinate[0] < -UV_EPSILON
                || coordinate[0] > 1.0 + UV_EPSILON
                || coordinate[1] < -UV_EPSILON
                || coordinate[1] > 1.0 + UV_EPSILON
            {
                return Err(GeometryError::Invalid(
                    "Low TEXCOORD_0 is outside the fixed [0,1] atlas".to_owned(),
                ));
            }
        }
        let min_u = uv
            .iter()
            .map(|value| value[0])
            .fold(1.0, f32::min)
            .clamp(0.0, 1.0);
        let max_u = uv
            .iter()
            .map(|value| value[0])
            .fold(0.0, f32::max)
            .clamp(0.0, 1.0);
        let min_v = uv
            .iter()
            .map(|value| value[1])
            .fold(1.0, f32::min)
            .clamp(0.0, 1.0);
        let max_v = uv
            .iter()
            .map(|value| value[1])
            .fold(0.0, f32::max)
            .clamp(0.0, 1.0);
        let x_start = ((min_u * resolution as f32).floor() as isize)
            .clamp(0, resolution as isize - 1) as usize;
        let x_end =
            ((max_u * resolution as f32).ceil() as isize).clamp(0, resolution as isize) as usize;
        let y_start = (((1.0 - max_v) * resolution as f32).floor() as isize)
            .clamp(0, resolution as isize - 1) as usize;
        let y_end = (((1.0 - min_v) * resolution as f32).ceil() as isize)
            .clamp(0, resolution as isize) as usize;
        if x_start >= x_end || y_start >= y_end {
            continue;
        }
        for y in y_start..y_end {
            for x in x_start..x_end {
                let sample_uv = [
                    (x as f32 + 0.5) / resolution as f32,
                    1.0 - (y as f32 + 0.5) / resolution as f32,
                ];
                let Some(barycentric) = barycentric_uv(uv, sample_uv) else {
                    continue;
                };
                let pixel_index = y * resolution + x;
                if owners[pixel_index] != usize::MAX {
                    diagnostic.uv_overlap_count = diagnostic.uv_overlap_count.saturating_add(1);
                    continue;
                }
                owners[pixel_index] = triangle_index;
                let shaded = shade_pixel(
                    pair,
                    barycentric,
                    high_geometry,
                    id_palette,
                    max_ray_distance,
                    &mut diagnostic,
                );
                write_shaded_pixel(&mut images, pixel_index, shaded);
                images.primary_covered_pixels = images.primary_covered_pixels.saturating_add(1);
            }
        }
        if images.primary_covered_pixels == pixel_count as u64 {
            break;
        }
    }
    images.covered_pixels = images.primary_covered_pixels;
    dilate_uncovered_pixels(&mut owners, &mut images, resolution, DILATION_TEXELS);
    images.normal_pixels = images.covered_pixels;
    images.ao_pixels = images.covered_pixels;
    images.curvature_pixels = images.covered_pixels;
    Ok(BakeRun { images, diagnostic })
}

fn write_shaded_pixel(images: &mut BakeImages, pixel_index: usize, shaded: ShadedPixel) {
    let offset = pixel_index * 3;
    images.normal[offset..offset + 3].copy_from_slice(&shaded.normal);
    images.ao[pixel_index] = shaded.ao;
    images.curvature[pixel_index] = shaded.curvature;
    images.thickness[pixel_index] = shaded.thickness;
    images.position[offset..offset + 3].copy_from_slice(&shaded.position);
    encode_u24_into(&mut images.object_id[offset..offset + 3], shaded.object_id);
    encode_u24_into(
        &mut images.material_id[offset..offset + 3],
        shaded.material_id,
    );
    encode_u24_into(&mut images.part_id[offset..offset + 3], shaded.part_id);
}

fn accumulate_topology_diagnostics(pair: &LowCagePair<'_>, diagnostic: &mut BakeDiagnostic) {
    let low_positions = pair.low.corners.clone().map(|corner| corner.position);
    let cage_positions = pair.cage.corners.clone().map(|corner| corner.position);
    let low_face = cross(
        sub(low_positions[1], low_positions[0]),
        sub(low_positions[2], low_positions[0]),
    );
    let cage_face = cross(
        sub(cage_positions[1], cage_positions[0]),
        sub(cage_positions[2], cage_positions[0]),
    );
    if let (Some(low_normal), Some(cage_normal)) = (normalize(low_face), normalize(cage_face)) {
        if dot(low_normal, cage_normal) < NORMAL_SKEW_DOT_MIN {
            diagnostic.skew_count = diagnostic.skew_count.saturating_add(1);
        }
    }
    let (low_min, low_max) = bounds3(low_positions);
    let (cage_min, cage_max) = bounds3(cage_positions);
    for point in low_positions {
        if (0..3).any(|axis| {
            point[axis] < cage_min[axis] - CAGE_INTERSECTION_EPSILON
                || point[axis] > cage_max[axis] + CAGE_INTERSECTION_EPSILON
        }) {
            diagnostic.out_of_range_count = diagnostic.out_of_range_count.saturating_add(1);
            diagnostic.penetration_count = diagnostic.penetration_count.saturating_add(1);
            diagnostic.cage_intersection_count =
                diagnostic.cage_intersection_count.saturating_add(1);
        }
    }
    for point in cage_positions {
        if (0..3).all(|axis| {
            point[axis] > low_min[axis] + CAGE_INTERSECTION_EPSILON
                && point[axis] < low_max[axis] - CAGE_INTERSECTION_EPSILON
        }) {
            diagnostic.overlap_count = diagnostic.overlap_count.saturating_add(1);
            diagnostic.cage_intersection_count =
                diagnostic.cage_intersection_count.saturating_add(1);
        }
    }
}

fn bounds3(positions: [[f32; 3]; 3]) -> ([f32; 3], [f32; 3]) {
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

fn dilate_uncovered_pixels(
    owners: &mut [usize],
    images: &mut BakeImages,
    resolution: usize,
    radius: usize,
) {
    if radius == 0 {
        return;
    }
    for _distance in 0..radius {
        let mut fills = Vec::<(usize, usize)>::new();
        for y in 0..resolution {
            for x in 0..resolution {
                let destination = y * resolution + x;
                if owners[destination] != usize::MAX {
                    continue;
                }
                let mut best: Option<(usize, usize)> = None;
                for dy in -1isize..=1 {
                    for dx in -1isize..=1 {
                        if dx == 0 && dy == 0 {
                            continue;
                        }
                        let nx = x as isize + dx;
                        let ny = y as isize + dy;
                        if nx < 0
                            || ny < 0
                            || nx >= resolution as isize
                            || ny >= resolution as isize
                        {
                            continue;
                        }
                        let source = ny as usize * resolution + nx as usize;
                        let owner = owners[source];
                        if owner == usize::MAX {
                            continue;
                        }
                        let candidate = (owner, source);
                        if best.is_none_or(|current| candidate < current) {
                            best = Some(candidate);
                        }
                    }
                }
                if let Some((_owner, source)) = best {
                    fills.push((destination, source));
                }
            }
        }
        if fills.is_empty() {
            break;
        }
        for (destination, source) in fills {
            let destination_offset = destination * 3;
            let source_offset = source * 3;
            let normal = images.normal[source_offset..source_offset + 3].to_owned();
            let position = images.position[source_offset..source_offset + 3].to_owned();
            let object_id = images.object_id[source_offset..source_offset + 3].to_owned();
            let material_id = images.material_id[source_offset..source_offset + 3].to_owned();
            let part_id = images.part_id[source_offset..source_offset + 3].to_owned();
            let ao = images.ao[source];
            let curvature = images.curvature[source];
            let thickness = images.thickness[source];
            owners[destination] = owners[source];
            images.normal[destination_offset..destination_offset + 3].copy_from_slice(&normal);
            images.position[destination_offset..destination_offset + 3].copy_from_slice(&position);
            images.object_id[destination_offset..destination_offset + 3]
                .copy_from_slice(&object_id);
            images.material_id[destination_offset..destination_offset + 3]
                .copy_from_slice(&material_id);
            images.part_id[destination_offset..destination_offset + 3].copy_from_slice(&part_id);
            images.ao[destination] = ao;
            images.curvature[destination] = curvature;
            images.thickness[destination] = thickness;
            images.dilated_pixels = images.dilated_pixels.saturating_add(1);
            images.covered_pixels = images.covered_pixels.saturating_add(1);
        }
    }
}

fn encode_u24_into(target: &mut [u8], value: u32) {
    let value = value.min(0x00ff_ffff);
    target[0] = (value >> 16) as u8;
    target[1] = (value >> 8) as u8;
    target[2] = value as u8;
}

fn shade_pixel(
    pair: &LowCagePair<'_>,
    barycentric: [f32; 3],
    high_geometry: &HighGeometry,
    id_palette: &IdPalette,
    max_ray_distance: f32,
    diagnostic: &mut BakeDiagnostic,
) -> ShadedPixel {
    let low_normal = interpolate_vec3(
        pair.low.corners.clone().map(|corner| corner.normal),
        barycentric,
    )
    .and_then(normalize)
    .unwrap_or([0.0, 0.0, 1.0]);
    let low_tangent_raw = interpolate_vec4(
        pair.low.corners.clone().map(|corner| corner.tangent),
        barycentric,
    );
    let tangent = normalize(sub(
        [low_tangent_raw[0], low_tangent_raw[1], low_tangent_raw[2]],
        scale(
            low_normal,
            dot(
                low_normal,
                [low_tangent_raw[0], low_tangent_raw[1], low_tangent_raw[2]],
            ),
        ),
    ))
    .unwrap_or([1.0, 0.0, 0.0]);
    let tangent_sign = if low_tangent_raw[3] < 0.0 { -1.0 } else { 1.0 };
    let bitangent =
        normalize(scale(cross(low_normal, tangent), tangent_sign)).unwrap_or([0.0, 1.0, 0.0]);
    let low_positions = pair.low.corners.clone().map(|corner| corner.position);
    let low_position = barycentric_point(low_positions, barycentric);
    let low_face_normal = normalize(cross(
        sub(low_positions[1], low_positions[0]),
        sub(low_positions[2], low_positions[0]),
    ))
    .unwrap_or(low_normal);
    let cage_positions = pair.cage.corners.clone().map(|corner| corner.position);
    let cage_position = barycentric_point(cage_positions, barycentric);
    let cage_face = cross(
        sub(cage_positions[1], cage_positions[0]),
        sub(cage_positions[2], cage_positions[0]),
    );
    let cage_normal = normalize(cage_face).unwrap_or(low_normal);
    let signed_offset = dot(sub(cage_position, low_position), low_face_normal);
    if !signed_offset.is_finite() || signed_offset < -CAGE_INTERSECTION_EPSILON {
        diagnostic.penetration_count = diagnostic.penetration_count.saturating_add(1);
    } else if signed_offset.abs() <= CAGE_INTERSECTION_EPSILON {
        diagnostic.cage_intersection_count = diagnostic.cage_intersection_count.saturating_add(1);
    }
    let low_centroid = centroid(low_positions);
    let cage_centroid = centroid(cage_positions);
    if length(sub(cage_centroid, low_centroid)) <= CAGE_INTERSECTION_EPSILON {
        diagnostic.overlap_count = diagnostic.overlap_count.saturating_add(1);
    }
    if dot(low_normal, cage_normal) < NORMAL_SKEW_DOT_MIN {
        diagnostic.skew_count = diagnostic.skew_count.saturating_add(1);
    }

    diagnostic.ray_sample_count = diagnostic.ray_sample_count.saturating_add(1);
    let candidates = [
        (
            add(cage_position, scale(cage_normal, CAGE_OFFSET_EPSILON)),
            scale(cage_normal, -1.0),
            true,
        ),
        (
            add(cage_position, scale(cage_normal, -CAGE_OFFSET_EPSILON)),
            cage_normal,
            false,
        ),
    ];
    let mut candidate_hits = Vec::<CandidateHit>::with_capacity(2);
    for (origin, direction, front) in candidates {
        if front {
            diagnostic.front_ray_test_count = diagnostic.front_ray_test_count.saturating_add(1);
        } else {
            diagnostic.back_ray_test_count = diagnostic.back_ray_test_count.saturating_add(1);
        }
        if let Some(hit) = probe_same_part(
            high_geometry,
            pair.low.part_id.as_str(),
            origin,
            direction,
            max_ray_distance,
            diagnostic,
        ) {
            if front {
                diagnostic.front_ray_hit_count = diagnostic.front_ray_hit_count.saturating_add(1);
            } else {
                diagnostic.back_ray_hit_count = diagnostic.back_ray_hit_count.saturating_add(1);
            }
            candidate_hits.push(CandidateHit { hit, front });
        }
    }
    let selected = candidate_hits.into_iter().min_by(|first, second| {
        first
            .hit
            .t
            .total_cmp(&second.hit.t)
            .then_with(|| second.front.cmp(&first.front))
    });
    let high_part = high_geometry.parts.get(pair.low.part_id.as_str());
    let Some(high_part) = high_part else {
        diagnostic.ray_miss_count = diagnostic.ray_miss_count.saturating_add(1);
        return empty_shaded_pixel();
    };
    let (hit, used_fallback) = if let Some(selected) = selected {
        (selected.hit, false)
    } else {
        diagnostic.ray_miss_count = diagnostic.ray_miss_count.saturating_add(1);
        let fallback = high_part
            .bvh
            .closest_surface(cage_position, max_ray_distance);
        if fallback.is_some() {
            diagnostic.nearest_surface_fallback_count =
                diagnostic.nearest_surface_fallback_count.saturating_add(1);
        }
        let Some(fallback) = fallback else {
            return empty_shaded_pixel();
        };
        (fallback, true)
    };
    diagnostic.ray_hit_count = diagnostic.ray_hit_count.saturating_add(1);
    if hit.t >= max_ray_distance - RAY_EPSILON {
        diagnostic.out_of_range_count = diagnostic.out_of_range_count.saturating_add(1);
    }
    if hit.t.is_finite() {
        diagnostic.max_observed_distance_m = diagnostic.max_observed_distance_m.max(hit.t);
        let bin = ((hit.t / max_ray_distance) * diagnostic.distance_histogram.len() as f32)
            .floor()
            .clamp(0.0, diagnostic.distance_histogram.len() as f32 - 1.0)
            as usize;
        diagnostic.distance_histogram[bin] = diagnostic.distance_histogram[bin].saturating_add(1);
    }
    let high_triangle = &high_part.bvh.triangles[hit.triangle_index];
    if !used_fallback && dot(high_triangle.face_normal, hit.ray_direction) >= 0.0 {
        diagnostic.backface_hit_count = diagnostic.backface_hit_count.saturating_add(1);
    }
    let high_normal = interpolate_vec3(
        high_triangle.corners.clone().map(|corner| corner.normal),
        hit.barycentric,
    )
    .and_then(normalize)
    .unwrap_or(high_triangle.face_normal);
    let tangent_space = [
        dot(high_normal, tangent),
        dot(high_normal, bitangent),
        dot(high_normal, low_normal),
    ];
    let normal = [
        encode_unit(tangent_space[0]),
        encode_unit(tangent_space[1]),
        encode_unit(tangent_space[2]),
    ];
    if dot(high_normal, cage_normal) < NORMAL_SKEW_DOT_MIN
        || dot(high_normal, low_normal) < NORMAL_SKEW_DOT_MIN
    {
        diagnostic.skew_count = diagnostic.skew_count.saturating_add(1);
    }

    let (ao, occluded) = sample_ao(
        high_part,
        add(
            barycentric_point(high_triangle.positions, hit.barycentric),
            scale(high_normal, CAGE_OFFSET_EPSILON),
        ),
        high_normal,
        max_ray_distance * AO_DISTANCE_FACTOR,
    );
    diagnostic.ao_ray_count = diagnostic
        .ao_ray_count
        .saturating_add(PRODUCTION_WEAPON_GEOMETRIC_BAKE_AO_SAMPLE_COUNT);
    diagnostic.ao_occluded_count = diagnostic.ao_occluded_count.saturating_add(occluded);
    diagnostic.curvature_samples = diagnostic.curvature_samples.saturating_add(1);
    let hit_position = barycentric_point(high_triangle.positions, hit.barycentric);
    let (thickness, thickness_incomplete) =
        sample_thickness(high_part, hit_position, high_normal, max_ray_distance);
    if thickness_incomplete {
        diagnostic.thickness_miss_count = diagnostic.thickness_miss_count.saturating_add(1);
    }
    let thickness_byte = thickness
        .map(|value| encode_unorm(value / max_ray_distance))
        .unwrap_or(0);
    ShadedPixel {
        normal,
        ao,
        curvature: (high_triangle.curvature * 255.0).round().clamp(0.0, 255.0) as u8,
        thickness: thickness_byte,
        position: encode_position(
            hit_position,
            high_geometry.bounds_min,
            high_geometry.bounds_max,
        ),
        object_id: id_palette
            .object
            .get(&high_triangle.source_node_id)
            .copied()
            .unwrap_or(0),
        material_id: id_palette
            .material
            .get(&high_triangle.material_zone_id)
            .copied()
            .unwrap_or(0),
        part_id: id_palette
            .part
            .get(&high_triangle.part_id)
            .copied()
            .unwrap_or(0),
    }
}

#[derive(Clone, Copy)]
struct CandidateHit {
    hit: RayHit,
    front: bool,
}

fn empty_shaded_pixel() -> ShadedPixel {
    ShadedPixel {
        normal: [128, 128, 255],
        ao: 255,
        curvature: 0,
        thickness: 0,
        position: [0, 0, 0],
        object_id: 0,
        material_id: 0,
        part_id: 0,
    }
}

fn probe_same_part(
    high_geometry: &HighGeometry,
    part_id: &str,
    origin: [f32; 3],
    direction: [f32; 3],
    max_distance: f32,
    diagnostic: &mut BakeDiagnostic,
) -> Option<RayHit> {
    let same = high_geometry
        .parts
        .get(part_id)
        .and_then(|part| part.bvh.raycast(origin, direction, max_distance));
    let foreign = high_geometry
        .parts
        .iter()
        .filter(|(candidate_part, _)| candidate_part.as_str() != part_id)
        .filter_map(|(_, part)| part.bvh.raycast(origin, direction, max_distance))
        .min_by(|first, second| first.t.total_cmp(&second.t));
    if foreign.is_some() {
        diagnostic.cross_part_hit_count = diagnostic.cross_part_hit_count.saturating_add(1);
    }
    match (same, foreign) {
        (Some(same), Some(foreign)) if foreign.t <= same.t + RAY_EPSILON => None,
        (Some(same), _) => Some(same),
        (None, Some(_)) | (None, None) => None,
    }
}

fn sample_thickness(
    high_part: &HighPart,
    point: [f32; 3],
    normal: [f32; 3],
    max_distance: f32,
) -> (Option<f32>, bool) {
    let outside = add(point, scale(normal, CAGE_OFFSET_EPSILON * 2.0));
    let inside = add(point, scale(normal, -CAGE_OFFSET_EPSILON * 2.0));
    let inward = high_part.bvh.raycast_range(
        outside,
        scale(normal, -1.0),
        2.0 * CAGE_OFFSET_EPSILON,
        max_distance,
    );
    let outward =
        high_part
            .bvh
            .raycast_range(inside, normal, 2.0 * CAGE_OFFSET_EPSILON, max_distance);
    match (inward, outward) {
        (Some(first), Some(second)) => (Some((first.t + second.t) * 0.5), false),
        (Some(first), None) => (Some(first.t), true),
        (None, Some(second)) => (Some(second.t), true),
        (None, None) => (None, true),
    }
}

fn encode_position(position: [f32; 3], min: [f32; 3], max: [f32; 3]) -> [u8; 3] {
    let mut encoded = [0; 3];
    for axis in 0..3 {
        let extent = (max[axis] - min[axis]).max(f32::EPSILON);
        let normalized = ((position[axis] - min[axis]) / extent).clamp(0.0, 1.0);
        encoded[axis] = (normalized * 255.0).round() as u8;
    }
    encoded
}

fn sample_ao(
    high_part: &HighPart,
    origin: [f32; 3],
    normal: [f32; 3],
    max_distance: f32,
) -> (u8, u64) {
    let reference = if normal[1].abs() < 0.9 {
        [0.0, 1.0, 0.0]
    } else {
        [1.0, 0.0, 0.0]
    };
    let tangent = normalize(cross(reference, normal)).unwrap_or([1.0, 0.0, 0.0]);
    let bitangent = normalize(cross(normal, tangent)).unwrap_or([0.0, 1.0, 0.0]);
    let max_distance = max_distance.max(0.01);
    let mut occluded = 0_u64;
    for sample in AO_DIRECTIONS {
        let direction = normalize(add(
            add(scale(tangent, sample[0]), scale(bitangent, sample[1])),
            scale(normal, sample[2]),
        ))
        .unwrap_or(normal);
        if high_part
            .bvh
            .raycast(origin, direction, max_distance)
            .is_some()
        {
            occluded = occluded.saturating_add(1);
        }
    }
    let visible = PRODUCTION_WEAPON_GEOMETRIC_BAKE_AO_SAMPLE_COUNT - occluded;
    (
        ((visible as f32 / PRODUCTION_WEAPON_GEOMETRIC_BAKE_AO_SAMPLE_COUNT as f32) * 255.0).round()
            as u8,
        occluded,
    )
}

fn barycentric_uv(uv: [[f32; 2]; 3], point: [f32; 2]) -> Option<[f32; 3]> {
    let denominator = (uv[1][1] - uv[2][1]) * (uv[0][0] - uv[2][0])
        + (uv[2][0] - uv[1][0]) * (uv[0][1] - uv[2][1]);
    if denominator.abs() <= 1.0e-12 {
        return None;
    }
    let first = ((uv[1][1] - uv[2][1]) * (point[0] - uv[2][0])
        + (uv[2][0] - uv[1][0]) * (point[1] - uv[2][1]))
        / denominator;
    let second = ((uv[2][1] - uv[0][1]) * (point[0] - uv[2][0])
        + (uv[0][0] - uv[2][0]) * (point[1] - uv[2][1]))
        / denominator;
    let third = 1.0 - first - second;
    if first >= -BARYCENTRIC_EPSILON
        && second >= -BARYCENTRIC_EPSILON
        && third >= -BARYCENTRIC_EPSILON
    {
        Some([first, second, third])
    } else {
        None
    }
}

fn interpolate_vec3(values: [[f32; 3]; 3], barycentric: [f32; 3]) -> Option<[f32; 3]> {
    normalize([
        values[0][0] * barycentric[0]
            + values[1][0] * barycentric[1]
            + values[2][0] * barycentric[2],
        values[0][1] * barycentric[0]
            + values[1][1] * barycentric[1]
            + values[2][1] * barycentric[2],
        values[0][2] * barycentric[0]
            + values[1][2] * barycentric[1]
            + values[2][2] * barycentric[2],
    ])
}

fn interpolate_vec4(values: [[f32; 4]; 3], barycentric: [f32; 3]) -> [f32; 4] {
    [
        values[0][0] * barycentric[0]
            + values[1][0] * barycentric[1]
            + values[2][0] * barycentric[2],
        values[0][1] * barycentric[0]
            + values[1][1] * barycentric[1]
            + values[2][1] * barycentric[2],
        values[0][2] * barycentric[0]
            + values[1][2] * barycentric[1]
            + values[2][2] * barycentric[2],
        values[0][3] * barycentric[0]
            + values[1][3] * barycentric[1]
            + values[2][3] * barycentric[2],
    ]
}

fn barycentric_point(values: [[f32; 3]; 3], barycentric: [f32; 3]) -> [f32; 3] {
    [
        values[0][0] * barycentric[0]
            + values[1][0] * barycentric[1]
            + values[2][0] * barycentric[2],
        values[0][1] * barycentric[0]
            + values[1][1] * barycentric[1]
            + values[2][1] * barycentric[2],
        values[0][2] * barycentric[0]
            + values[1][2] * barycentric[1]
            + values[2][2] * barycentric[2],
    ]
}

fn closest_point_on_triangle(point: [f32; 3], triangle: [[f32; 3]; 3]) -> ([f32; 3], [f32; 3]) {
    let [a, b, c] = triangle;
    let ab = sub(b, a);
    let ac = sub(c, a);
    let ap = sub(point, a);
    let d1 = dot(ab, ap);
    let d2 = dot(ac, ap);
    if d1 <= 0.0 && d2 <= 0.0 {
        return (a, [1.0, 0.0, 0.0]);
    }
    let bp = sub(point, b);
    let d3 = dot(ab, bp);
    let d4 = dot(ac, bp);
    if d3 >= 0.0 && d4 <= d3 {
        return (b, [0.0, 1.0, 0.0]);
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        let v = d1 / (d1 - d3);
        return (add(a, scale(ab, v)), [1.0 - v, v, 0.0]);
    }
    let cp = sub(point, c);
    let d5 = dot(ab, cp);
    let d6 = dot(ac, cp);
    if d6 >= 0.0 && d5 <= d6 {
        return (c, [0.0, 0.0, 1.0]);
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        let w = d2 / (d2 - d6);
        return (add(a, scale(ac, w)), [1.0 - w, 0.0, w]);
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        let edge = sub(c, b);
        let w = (d4 - d3) / ((d4 - d3) + (d5 - d6));
        return (add(b, scale(edge, w)), [0.0, 1.0 - w, w]);
    }
    let denominator = 1.0 / (va + vb + vc);
    let v = vb * denominator;
    let w = vc * denominator;
    (add(a, add(scale(ab, v), scale(ac, w))), [1.0 - v - w, v, w])
}

fn ray_triangle(
    triangle: &HighTriangle,
    origin: [f32; 3],
    direction: [f32; 3],
    min_distance: f32,
    max_distance: f32,
) -> Option<RayHit> {
    let edge_one = sub(triangle.positions[1], triangle.positions[0]);
    let edge_two = sub(triangle.positions[2], triangle.positions[0]);
    let pvec = cross(direction, edge_two);
    let determinant = dot(edge_one, pvec);
    if determinant.abs() <= 1.0e-8 {
        return None;
    }
    let inverse = 1.0 / determinant;
    let tvec = sub(origin, triangle.positions[0]);
    let first = dot(tvec, pvec) * inverse;
    if first < -BARYCENTRIC_EPSILON || first > 1.0 + BARYCENTRIC_EPSILON {
        return None;
    }
    let qvec = cross(tvec, edge_one);
    let second = dot(direction, qvec) * inverse;
    if second < -BARYCENTRIC_EPSILON || first + second > 1.0 + BARYCENTRIC_EPSILON {
        return None;
    }
    let distance = dot(edge_two, qvec) * inverse;
    if distance <= min_distance || distance > max_distance {
        return None;
    }
    Some(RayHit {
        triangle_index: 0,
        t: distance,
        barycentric: [1.0 - first - second, first, second],
        ray_direction: direction,
    })
}

fn indices_bounds(indices: &[usize], triangles: &[HighTriangle]) -> ([f32; 3], [f32; 3]) {
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    for index in indices {
        for position in triangles[*index].positions {
            for axis in 0..3 {
                min[axis] = min[axis].min(position[axis]);
                max[axis] = max[axis].max(position[axis]);
            }
        }
    }
    (min, max)
}

fn ray_aabb(
    min: [f32; 3],
    max: [f32; 3],
    origin: [f32; 3],
    direction: [f32; 3],
    max_distance: f32,
) -> bool {
    let mut near: f32 = 0.0;
    let mut far: f32 = max_distance;
    for axis in 0..3 {
        if direction[axis].abs() <= 1.0e-12 {
            if origin[axis] < min[axis] || origin[axis] > max[axis] {
                return false;
            }
            continue;
        }
        let inverse = 1.0 / direction[axis];
        let mut first = (min[axis] - origin[axis]) * inverse;
        let mut second = (max[axis] - origin[axis]) * inverse;
        if first > second {
            std::mem::swap(&mut first, &mut second);
        }
        near = near.max(first);
        far = far.min(second);
        if near > far {
            return false;
        }
    }
    far >= RAY_EPSILON
}

fn point_aabb_distance_squared(point: [f32; 3], min: [f32; 3], max: [f32; 3]) -> f32 {
    let mut distance_squared = 0.0;
    for axis in 0..3 {
        let distance = if point[axis] < min[axis] {
            min[axis] - point[axis]
        } else if point[axis] > max[axis] {
            point[axis] - max[axis]
        } else {
            0.0
        };
        distance_squared += distance * distance;
    }
    distance_squared
}

fn weld_position(value: [f32; 3]) -> [i64; 3] {
    [
        (value[0] * POSITION_WELD_SCALE).round() as i64,
        (value[1] * POSITION_WELD_SCALE).round() as i64,
        (value[2] * POSITION_WELD_SCALE).round() as i64,
    ]
}

fn centroid(values: [[f32; 3]; 3]) -> [f32; 3] {
    [
        (values[0][0] + values[1][0] + values[2][0]) / 3.0,
        (values[0][1] + values[1][1] + values[2][1]) / 3.0,
        (values[0][2] + values[1][2] + values[2][2]) / 3.0,
    ]
}

fn normalize(value: [f32; 3]) -> Option<[f32; 3]> {
    let length = dot(value, value).sqrt();
    if !length.is_finite() || length <= 1.0e-8 {
        None
    } else {
        Some(scale(value, 1.0 / length))
    }
}

fn length(value: [f32; 3]) -> f32 {
    dot(value, value).sqrt()
}

fn dot(first: [f32; 3], second: [f32; 3]) -> f32 {
    first[0] * second[0] + first[1] * second[1] + first[2] * second[2]
}

fn cross(first: [f32; 3], second: [f32; 3]) -> [f32; 3] {
    [
        first[1] * second[2] - first[2] * second[1],
        first[2] * second[0] - first[0] * second[2],
        first[0] * second[1] - first[1] * second[0],
    ]
}

fn sub(first: [f32; 3], second: [f32; 3]) -> [f32; 3] {
    [
        first[0] - second[0],
        first[1] - second[1],
        first[2] - second[2],
    ]
}

fn add(first: [f32; 3], second: [f32; 3]) -> [f32; 3] {
    [
        first[0] + second[0],
        first[1] + second[1],
        first[2] + second[2],
    ]
}

fn scale(value: [f32; 3], factor: f32) -> [f32; 3] {
    [value[0] * factor, value[1] * factor, value[2] * factor]
}

fn encode_unit(value: f32) -> u8 {
    ((value.clamp(-1.0, 1.0) * 0.5 + 0.5) * 255.0).round() as u8
}

fn encode_unorm(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn hash_bytes(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
