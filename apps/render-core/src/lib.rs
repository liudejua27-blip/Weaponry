//! Product-owned deterministic GLB renderer.
//!
//! This crate accepts only bounded, self-contained GLB bytes and typed camera
//! values. It never compiles GeometryProgram authoring input, opens a path,
//! starts a process, listens on a socket, or calls a model/network service.

use image::{imageops, ImageFormat, Rgba, RgbaImage};
use serde_json::{Map, Value};
use std::collections::HashSet;
use std::io::Cursor;

#[derive(Debug, Clone)]
pub struct RenderPass {
    pub pass: String,
    pub png: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

/// One source-lineage row in the transient fixed-renderer hit map.
///
/// The row is deliberately indexed by the renderer's deterministic triangle
/// walk (mesh, primitive, then triangle within the primitive).  It is not a
/// candidate/Stage record and is never persisted by this crate.  The GLB
/// primitive `extras` are the only accepted source of semantic lineage; an
/// attribution render fails closed when that metadata is absent or ambiguous.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RasterHitSourceMapEntry {
    pub triangle_index: u32,
    pub mesh_index: u32,
    pub primitive_index: u32,
    pub triangle_index_in_primitive: u32,
    pub semantic_part_id: String,
    pub source_node_id: String,
    pub lineage_source_node_ids: Vec<String>,
    pub material_zone_id: String,
}

/// The exact output-pixel hit produced by the fixed 512px renderer's depth
/// raster.  `triangle_index` and `source_map_index` are both retained: the
/// former is the face identity, while the latter makes the source table
/// binding explicit for consumers that aggregate pixels by Part/source node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RasterPixelHit {
    pub triangle_index: u32,
    pub source_map_index: u32,
    pub barycentric_milli: [u16; 3],
    pub depth_micros: u32,
}

/// A bounded, read-only pixel → triangle → source-node/semantic-Part
/// projection from the current fixed renderer.  It is transient by design:
/// callers must bind it to their candidate/reference/camera hashes before
/// using it in a diagnostic receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RasterHitSourceMap {
    pub width: u32,
    pub height: u32,
    pub raster_width: u32,
    pub raster_height: u32,
    pub pixels: Vec<Option<RasterPixelHit>>,
    pub sources: Vec<RasterHitSourceMapEntry>,
}

impl RasterHitSourceMap {
    /// Encode one little-endian u32 triangle id per output pixel. `u32::MAX`
    /// denotes the fixed renderer background.  This compact representation is
    /// suitable for the isolated Render Worker transport and carries no raw
    /// GLB/image bytes.
    pub fn encode_triangle_ids_le(&self) -> Vec<u8> {
        let mut encoded = Vec::with_capacity(self.pixels.len() * 4);
        for hit in &self.pixels {
            encoded.extend_from_slice(
                &hit.as_ref()
                    .map(|value| value.triangle_index)
                    .unwrap_or(u32::MAX)
                    .to_le_bytes(),
            );
        }
        encoded
    }
}

/// A transient, bounded override for one already-authored GLB material.
/// `material_zone_id` is matched against the glTF material name while
/// `material_id` is matched against `extras.forgecad.material_id`; callers
/// cannot select an arbitrary material index.
#[derive(Debug, Clone, PartialEq)]
pub struct EmissiveMaterialOverride {
    pub material_zone_id: String,
    pub material_id: String,
    pub color_linear_rgb: [f32; 3],
    pub emissive_strength: f32,
}

/// Bounded, product-owned HDR bloom controls. The fixed renderer keeps the
/// HDR buffer transient and encodes the two review passes as a normalized
/// linear PNG; no shader, kernel or caller-provided executable is accepted.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HdrBloomProfile {
    pub threshold: f32,
    pub radius_px: u32,
    pub intensity: f32,
    pub hdr_clamp: f32,
}

/// A bounded, typed particle input for the fictional-energy visual pass.
/// Particles are deliberately data-only: Runtime derives their deterministic
/// values from durable hashes, while the isolated Render Worker only projects
/// and rasterizes them. No shader, script, path, URL or caller RNG is part of
/// this contract.
#[derive(Debug, Clone, PartialEq)]
pub struct TypedParticle {
    pub emitter_id: String,
    pub id: u32,
    pub position: [f32; 3],
    pub radius_px: f32,
    pub color_linear_rgb: [f32; 3],
    pub alpha: f32,
    pub lifetime_ticks: u64,
    pub depth: f32,
}

/// A bounded, product-owned typed trail for the fictional-energy visual pass.
/// Trails are data-only polyline ribbons: Runtime derives their points from
/// durable hashes and exact LOD0 anchor/Part transforms, while the isolated
/// Render Worker only projects and rasterizes them.  There is no shader,
/// script, socket, path, URL or caller RNG in this contract.
#[derive(Debug, Clone, PartialEq)]
pub struct TypedTrail {
    pub emitter_id: String,
    pub id: u32,
    pub points: Vec<[f32; 3]>,
    pub radius_px: f32,
    pub color_linear_rgb: [f32; 3],
    pub alpha: f32,
    pub lifetime_ticks: u64,
}

/// The closed, product-owned HDR profile for typed trail Bloom.  This is
/// intentionally a distinct profile from material Bloom: trail color is
/// bounded to linear 0..1, so a fixed source gain is required to produce a
/// reviewable HDR source.  The operation never accepts a shader, kernel or
/// caller-selected gain.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TypedTrailBloomProfile {
    pub threshold: f32,
    pub radius_px: u32,
    pub intensity: f32,
    pub hdr_clamp: f32,
    pub source_gain: f32,
}

impl TypedTrailBloomProfile {
    pub const FIXED: Self = Self {
        threshold: 1.0,
        radius_px: 8,
        intensity: 4.0,
        hdr_clamp: 16.0,
        source_gain: 8.0,
    };
    pub const BLUR_PASSES: u32 = 2;

    pub fn validate_fixed(self) -> Result<Self, RenderError> {
        if self != Self::FIXED {
            return Err(RenderError::Invalid(
                "typed trail Bloom profile must use the fixed Runtime profile".to_owned(),
            ));
        }
        Ok(self)
    }
}

impl HdrBloomProfile {
    pub const MAX_RADIUS_PX: u32 = 8;
    pub const MAX_INTENSITY: f32 = 4.0;
    pub const MAX_HDR_CLAMP: f32 = 16.0;
    pub const BLUR_PASSES: u32 = 2;

    pub fn validate(self) -> Result<Self, RenderError> {
        if !self.threshold.is_finite()
            || !(0.0..=Self::MAX_HDR_CLAMP).contains(&self.threshold)
            || self.radius_px == 0
            || self.radius_px > Self::MAX_RADIUS_PX
            || !self.intensity.is_finite()
            || !(0.0..=Self::MAX_INTENSITY).contains(&self.intensity)
            || !self.hdr_clamp.is_finite()
            || !(1.0..=Self::MAX_HDR_CLAMP).contains(&self.hdr_clamp)
        {
            return Err(RenderError::Invalid(
                "HDR bloom profile is outside the bounded domain".to_owned(),
            ));
        }
        Ok(self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppliedEmissiveMaterialOverride {
    pub material_zone_id: String,
    pub material_id: String,
    pub glb_material_index: usize,
}

#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    #[error("render input is invalid: {0}")]
    Invalid(String),
}

const ID_PALETTE_CAPACITY: usize = 256;

fn validate_id_palette_domain(root: &Value) -> Result<(), RenderError> {
    let meshes = root
        .get("meshes")
        .and_then(Value::as_array)
        .ok_or_else(|| RenderError::Invalid("GLB meshes are missing".to_owned()))?;
    if meshes.is_empty() || meshes.len() > ID_PALETTE_CAPACITY {
        return Err(RenderError::Invalid(
            "GLB mesh count exceeds the fixed Part-ID palette".to_owned(),
        ));
    }
    if root
        .get("materials")
        .and_then(Value::as_array)
        .is_some_and(|materials| materials.len() > ID_PALETTE_CAPACITY)
    {
        return Err(RenderError::Invalid(
            "GLB material count exceeds the fixed Material-ID palette".to_owned(),
        ));
    }
    for mesh in meshes {
        let primitives = mesh
            .get("primitives")
            .and_then(Value::as_array)
            .ok_or_else(|| RenderError::Invalid("GLB primitive list is missing".to_owned()))?;
        if primitives.iter().any(|primitive| {
            primitive
                .get("material")
                .and_then(Value::as_u64)
                .is_some_and(|index| index >= ID_PALETTE_CAPACITY as u64)
        }) {
            return Err(RenderError::Invalid(
                "GLB material index exceeds the fixed Material-ID palette".to_owned(),
            ));
        }
    }
    Ok(())
}

fn resolve_emissive_material_overrides(
    root: &Value,
    overrides: &[EmissiveMaterialOverride],
) -> Result<Vec<AppliedEmissiveMaterialOverride>, RenderError> {
    if overrides.len() > 8 {
        return Err(RenderError::Invalid(
            "emissive override count exceeds the fixed limit".to_owned(),
        ));
    }
    let materials = root
        .get("materials")
        .and_then(Value::as_array)
        .ok_or_else(|| RenderError::Invalid("GLB materials are missing".to_owned()))?;
    let mut seen_zones = HashSet::new();
    let mut seen_indices = HashSet::new();
    let mut applied = Vec::with_capacity(overrides.len());
    for override_value in overrides {
        if override_value.material_zone_id.is_empty()
            || override_value.material_zone_id.len() > 128
            || override_value.material_id.is_empty()
            || override_value.material_id.len() > 128
            || override_value
                .color_linear_rgb
                .iter()
                .any(|value| !value.is_finite() || !(0.0..=1.0).contains(value))
            || !override_value.emissive_strength.is_finite()
            || !(0.0..=16.0).contains(&override_value.emissive_strength)
        {
            return Err(RenderError::Invalid(
                "emissive override is outside the bounded domain".to_owned(),
            ));
        }
        if !seen_zones.insert(override_value.material_zone_id.as_str()) {
            return Err(RenderError::Invalid(
                "emissive override material zone is duplicated".to_owned(),
            ));
        }
        let matches = materials
            .iter()
            .enumerate()
            .filter(|(_, material)| {
                material.get("name").and_then(Value::as_str)
                    == Some(override_value.material_zone_id.as_str())
                    && material
                        .get("extras")
                        .and_then(|value| value.get("forgecad"))
                        .and_then(|value| value.get("material_id"))
                        .and_then(Value::as_str)
                        == Some(override_value.material_id.as_str())
            })
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        if matches.len() != 1 || !seen_indices.insert(matches[0]) {
            return Err(RenderError::Invalid(
                "emissive override does not resolve to exactly one GLB material".to_owned(),
            ));
        }
        applied.push(AppliedEmissiveMaterialOverride {
            material_zone_id: override_value.material_zone_id.clone(),
            material_id: override_value.material_id.clone(),
            glb_material_index: matches[0],
        });
    }
    Ok(applied)
}

fn normalize(value: [f32; 3]) -> [f32; 3] {
    let length = (value[0] * value[0] + value[1] * value[1] + value[2] * value[2]).sqrt();
    if length <= f32::EPSILON {
        [0.0, 1.0, 0.0]
    } else {
        [value[0] / length, value[1] / length, value[2] / length]
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

pub fn render_fixed_glb(glb: &[u8]) -> Result<Vec<RenderPass>, RenderError> {
    let (root, binary) = parse_glb(glb)?;
    validate_id_palette_domain(&root)?;
    let meshes = root
        .get("meshes")
        .and_then(Value::as_array)
        .ok_or_else(|| RenderError::Invalid("GLB meshes are missing".to_owned()))?;
    let accessors = root
        .get("accessors")
        .and_then(Value::as_array)
        .ok_or_else(|| RenderError::Invalid("GLB accessors are missing".to_owned()))?;
    let views = root
        .get("bufferViews")
        .and_then(Value::as_array)
        .ok_or_else(|| RenderError::Invalid("GLB bufferViews are missing".to_owned()))?;
    let mut vertices = Vec::<([f32; 3], [f32; 3], usize)>::new();
    let mut triangles = Vec::<([usize; 3], usize)>::new();
    for (mesh_index, mesh) in meshes.iter().enumerate() {
        let primitives = mesh
            .get("primitives")
            .and_then(Value::as_array)
            .ok_or_else(|| RenderError::Invalid("GLB primitive list is missing".to_owned()))?;
        for primitive in primitives {
            let attributes = primitive
                .get("attributes")
                .and_then(Value::as_object)
                .ok_or_else(|| {
                    RenderError::Invalid("GLB primitive attributes are missing".to_owned())
                })?;
            let position_accessor = attributes
                .get("POSITION")
                .and_then(Value::as_u64)
                .ok_or_else(|| {
                    RenderError::Invalid("GLB POSITION accessor is missing".to_owned())
                })? as usize;
            let normal_accessor = attributes
                .get("NORMAL")
                .and_then(Value::as_u64)
                .map(|value| value as usize);
            let index_accessor = primitive
                .get("indices")
                .and_then(Value::as_u64)
                .ok_or_else(|| RenderError::Invalid("GLB index accessor is missing".to_owned()))?
                as usize;
            let positions = read_vec3_accessor(accessors, views, &binary, position_accessor)?;
            let normals = normal_accessor
                .map(|index| read_vec3_accessor(accessors, views, &binary, index))
                .transpose()?
                .unwrap_or_else(|| vec![[0.0, 1.0, 0.0]; positions.len()]);
            if normals.len() != positions.len() {
                return Err(RenderError::Invalid(
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
        return Err(RenderError::Invalid(
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
                RenderError::Invalid(format!("fixed render encode failed: {error}"))
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
    tangent: [f32; 4],
    uv: [f32; 2],
}

#[derive(Clone, Copy)]
struct RasterHit {
    depth: f32,
    world: [f32; 3],
    normal: [f32; 3],
    tangent: [f32; 4],
    uv: [f32; 2],
    mesh_index: usize,
    material_index: usize,
    triangle_index: u32,
    source_map_index: u32,
    barycentric: [f32; 3],
    edge: bool,
    uv_stretch: f32,
}

#[derive(Clone, Copy)]
enum CameraProjection {
    Perspective { focal: f32 },
    Orthographic { scale: f32 },
}

type Mat4 = [[f32; 4]; 4];

/// The bounded static Hero beauty output. This is deliberately a transient
/// render size and does not alter the formal RenderSet@2 512px AOV contract.
pub const HERO_BEAUTY_RESOLUTION: u32 = 2048;
/// The fixed camera policy used by the render-worker-only Hero operation.
pub const HERO_BEAUTY_CAMERA_POLICY: &str = "fixed-static-hero-perspective@1";
/// The existing product-owned three-point studio light policy used by the
/// material shader. No caller-supplied light, shader, or HDRI is accepted.
pub const HERO_BEAUTY_LIGHTING_POLICY: &str = "fixed-three-point-studio@1";
// Hero uses one sample per fixed 2048px output pixel. Keeping this separate
// from the existing 2x formal-AOV path avoids allocating a 4096x4096 hit
// buffer while retaining a literal 2048x2048 Hero raster.
const HERO_BEAUTY_RASTER_RESOLUTION: u32 = HERO_BEAUTY_RESOLUTION;

/// Render a self-contained GLB using the C-stage fixed camera contract. This
/// is deliberately a small deterministic software renderer: node transforms,
/// V1 perspective and V2 orthographic projections, a depth buffer, fixed
/// GGX-like direct lighting and deterministic 2x supersampling are all
/// product-owned and offline. It does not accept shaders, scripts, URLs or
/// material paths from the request.
pub fn render_perspective_glb(glb: &[u8], camera: &Value) -> Result<Vec<RenderPass>, RenderError> {
    render_perspective_glb_at_resolution(glb, camera, 512)
}

/// Render the fixed 512×512 camera and return its transient pixel hit/source
/// projection. This uses the same 1024×1024 supersampled raster grid as the
/// formal nine-AOV path, then samples that depth-resolved grid at output
/// pixels. It never writes a RenderSet, candidate, CAS object, or stage.
pub fn render_perspective_glb_raster_hit_source_map(
    glb: &[u8],
    camera: &Value,
) -> Result<RasterHitSourceMap, RenderError> {
    let (_, source_map) = render_perspective_glb_at_resolution_with_passes_bounded(
        glb,
        camera,
        512,
        &["silhouette", "part-id"],
        &[],
        None,
        false,
        true,
    )?;
    source_map.ok_or_else(|| {
        RenderError::Invalid("fixed raster hit/source map was not produced".to_owned())
    })
}

/// Render the same fixed camera at a bounded internal resolution. The public
/// evidence path remains 512×512; the lower-resolution variant is only for
/// Runtime's transient silhouette search and is never persisted as an AOV.
pub fn render_perspective_glb_at_resolution(
    glb: &[u8],
    camera: &Value,
    resolution: u32,
) -> Result<Vec<RenderPass>, RenderError> {
    render_perspective_glb_at_resolution_with_passes(
        glb,
        camera,
        resolution,
        &[
            "beauty",
            "silhouette",
            "depth",
            "normal",
            "ao",
            "part-id",
            "material-id",
            "wireframe",
            "uv-stretch",
        ],
        &[],
        None,
    )
}

/// Render one deterministic static Hero beauty PNG from the exact supplied
/// GLB. The camera, three-point lights, resolution, and pass inventory are
/// closed in this function; the caller cannot provide paths, shaders, or
/// arbitrary render stages. The returned pass is transient and does not
/// advance a candidate, create a version, or write Runtime/CAS state.
pub fn render_static_hero_beauty_glb(glb: &[u8]) -> Result<RenderPass, RenderError> {
    let camera = serde_json::json!({
        "schema_version":"CameraCalibration@1",
        "projection":"perspective",
        "transform":{
            "position_m":[0.0,0.25,10.0],
            "target_m":[0.0,0.25,0.0],
            "up":[0.0,1.0,0.0]
        },
        "fov_y_degrees":40.0,
        "near_m":0.05,
        "far_m":100.0,
        // CameraCalibration remains the 512px calibration contract; this
        // operation's transient raster/output size is fixed independently.
        "resolution":{"width":512,"height":512},
        "coordinate_system":"right-handed-y-up-meter"
    });
    let passes = render_perspective_glb_at_resolution_with_passes_bounded(
        glb,
        &camera,
        HERO_BEAUTY_RESOLUTION,
        &["beauty"],
        &[],
        None,
        true,
        false,
    )?;
    let mut passes = passes.0;
    if passes.len() != 1 {
        return Err(RenderError::Invalid(
            "static Hero beauty pass inventory is not fixed".to_owned(),
        ));
    }
    Ok(passes.remove(0))
}

/// Render a formal 512x512 nine-AOV frame while replacing emissive values for
/// a small set of exactly identified GLB materials. This is transient render
/// input only: it does not mutate the GLB or any candidate/CAS state.
pub fn render_perspective_glb_with_emissive_overrides(
    glb: &[u8],
    camera: &Value,
    overrides: &[EmissiveMaterialOverride],
) -> Result<(Vec<RenderPass>, Vec<AppliedEmissiveMaterialOverride>), RenderError> {
    let (root, _) = parse_glb(glb)?;
    let applied = resolve_emissive_material_overrides(&root, overrides)?;
    let passes = render_perspective_glb_at_resolution_with_passes(
        glb,
        camera,
        512,
        &[
            "beauty",
            "silhouette",
            "depth",
            "normal",
            "ao",
            "part-id",
            "material-id",
            "wireframe",
            "uv-stretch",
        ],
        overrides,
        None,
    )?;
    Ok((passes, applied))
}

/// Render the fixed nine AOVs plus independent HDR emissive-source and
/// two-pass separable bloom contribution passes. The base AOV bytes are
/// produced by the same raster path and remain free of post-process bloom.
pub fn render_perspective_glb_with_hdr_bloom(
    glb: &[u8],
    camera: &Value,
    overrides: &[EmissiveMaterialOverride],
    profile: HdrBloomProfile,
) -> Result<(Vec<RenderPass>, Vec<AppliedEmissiveMaterialOverride>), RenderError> {
    let profile = profile.validate()?;
    let (root, _) = parse_glb(glb)?;
    let applied = resolve_emissive_material_overrides(&root, overrides)?;
    let bloom_passes = ["emissive-source", "bloom-contribution"];
    let passes = render_perspective_glb_at_resolution_with_passes(
        glb,
        camera,
        512,
        &bloom_passes,
        overrides,
        Some(&profile),
    )?;
    if passes.len() != 2 {
        return Err(RenderError::Invalid(
            "HDR bloom pass inventory is not fixed".to_owned(),
        ));
    }
    Ok((passes, applied))
}

/// Render only the two passes needed by the transient camera/Rig solver. This
/// avoids encoding seven irrelevant AOVs for every search camera; the public
/// fixed-render evidence path above remains the complete nine-pass renderer.
pub fn render_perspective_glb_fit_at_resolution(
    glb: &[u8],
    camera: &Value,
    resolution: u32,
) -> Result<Vec<RenderPass>, RenderError> {
    render_perspective_glb_at_resolution_with_passes(
        glb,
        camera,
        resolution,
        &["silhouette", "part-id"],
        &[],
        None,
    )
}

/// Render the independent typed-particle AOV set at the fixed 512x512 output
/// resolution. This pass never reads or changes a GLB and therefore cannot
/// contaminate the candidate-bound base nine AOVs or the HDR bloom passes.
pub fn render_typed_particles(
    camera: &Value,
    particles: &[TypedParticle],
) -> Result<Vec<RenderPass>, RenderError> {
    render_typed_particles_internal(camera, particles, None)
}

/// Render typed particles against the opaque depth of the exact candidate
/// GLB. The GLB is read only for the depth buffer; no base or bloom pass is
/// returned or modified by this operation.
pub fn render_typed_particles_with_glb(
    glb: &[u8],
    camera: &Value,
    particles: &[TypedParticle],
) -> Result<Vec<RenderPass>, RenderError> {
    let depth_passes =
        render_perspective_glb_at_resolution_with_passes(glb, camera, 512, &["depth"], &[], None)?;
    let depth_pass = depth_passes
        .first()
        .ok_or_else(|| RenderError::Invalid("opaque depth pass is unavailable".to_owned()))?;
    let depth_image = image::load_from_memory(&depth_pass.png)
        .map_err(|error| RenderError::Invalid(format!("opaque depth pass decode failed: {error}")))?
        .to_rgba8();
    render_typed_particles_internal(camera, particles, Some(&depth_image))
}

fn render_typed_particles_internal(
    camera: &Value,
    particles: &[TypedParticle],
    geometry_depth: Option<&RgbaImage>,
) -> Result<Vec<RenderPass>, RenderError> {
    const WIDTH: u32 = 512;
    const HEIGHT: u32 = 512;
    if particles.is_empty() || particles.len() > 128 {
        return Err(RenderError::Invalid(
            "typed particle count is outside the bounded domain".to_owned(),
        ));
    }
    let (camera_position, forward, right, up, projection, near, far) = parse_camera(camera)?;
    let aspect = WIDTH as f32 / HEIGHT as f32;
    let mut projected = Vec::with_capacity(particles.len());
    for particle in particles {
        if particle.emitter_id != "muzzle-burst" && particle.emitter_id != "energy-core-sparks" {
            return Err(RenderError::Invalid(
                "typed particle emitter is not in the closed set".to_owned(),
            ));
        }
        if particle.id == 0
            || particle.id > 65_535
            || particle
                .position
                .iter()
                .any(|value| !value.is_finite() || value.abs() > 10.0)
            || !particle.radius_px.is_finite()
            || !(1.0..=8.0).contains(&particle.radius_px)
            || particle
                .color_linear_rgb
                .iter()
                .any(|value| !value.is_finite() || !(0.0..=1.0).contains(value))
            || !particle.alpha.is_finite()
            || !(0.0..=1.0).contains(&particle.alpha)
            || particle.lifetime_ticks == 0
            || particle.lifetime_ticks > 1_000_000
            || !particle.depth.is_finite()
            || !(0.0..=1.0).contains(&particle.depth)
        {
            return Err(RenderError::Invalid(
                "typed particle attribute is outside the bounded domain".to_owned(),
            ));
        }
        let relative = subtract3(particle.position, camera_position);
        let z = dot3(relative, forward);
        if !z.is_finite() || z <= near || z >= far {
            continue;
        }
        let projected_depth = ((z - near) / (far - near)).clamp(0.0, 1.0);
        if (particle.depth - projected_depth).abs() > 1.0e-5 {
            return Err(RenderError::Invalid(
                "typed particle depth differs from its camera-space position".to_owned(),
            ));
        }
        let x = dot3(relative, right);
        let y = dot3(relative, up);
        let (ndc_x, ndc_y) = match projection {
            CameraProjection::Perspective { focal } => ((x * focal / aspect) / z, (y * focal) / z),
            CameraProjection::Orthographic { scale } => {
                ((x / (scale * aspect * 0.5)), (y / (scale * 0.5)))
            }
        };
        let screen_x = (ndc_x * 0.5 + 0.5) * WIDTH as f32;
        let screen_y = (1.0 - (ndc_y * 0.5 + 0.5)) * HEIGHT as f32;
        if screen_x.is_finite()
            && screen_y.is_finite()
            && screen_x >= -8.0
            && screen_x <= WIDTH as f32 + 8.0
            && screen_y >= -8.0
            && screen_y <= HEIGHT as f32 + 8.0
        {
            projected.push((particle, screen_x, screen_y, projected_depth));
        }
    }
    if projected.is_empty() {
        return Err(RenderError::Invalid(
            "camera produced no visible typed particles".to_owned(),
        ));
    }

    let mut color = RgbaImage::from_pixel(WIDTH, HEIGHT, Rgba([0, 0, 0, 0]));
    let mut id = RgbaImage::from_pixel(WIDTH, HEIGHT, Rgba([0, 0, 0, 0]));
    let mut depth = RgbaImage::from_pixel(WIDTH, HEIGHT, Rgba([0, 0, 0, 0]));
    let mut nearest_depth = vec![1.0_f32; (WIDTH * HEIGHT) as usize];
    let mut nearest_id = vec![u32::MAX; (WIDTH * HEIGHT) as usize];
    // Stable input order is part of the typed Runtime receipt. Color uses
    // deterministic source-over compositing; ID/depth use nearest depth.
    for (particle, screen_x, screen_y, projected_depth) in projected {
        let radius = particle.radius_px.ceil() as i32;
        let min_x = (screen_x.floor() as i32 - radius).max(0) as u32;
        let max_x = (screen_x.ceil() as i32 + radius)
            .min(WIDTH as i32 - 1)
            .max(0) as u32;
        let min_y = (screen_y.floor() as i32 - radius).max(0) as u32;
        let max_y = (screen_y.ceil() as i32 + radius)
            .min(HEIGHT as i32 - 1)
            .max(0) as u32;
        let radius_sq = particle.radius_px * particle.radius_px;
        let source = [
            linear_to_srgb(particle.color_linear_rgb[0]),
            linear_to_srgb(particle.color_linear_rgb[1]),
            linear_to_srgb(particle.color_linear_rgb[2]),
        ];
        for y in min_y..=max_y {
            for x in min_x..=max_x {
                let dx = x as f32 + 0.5 - screen_x;
                let dy = y as f32 + 0.5 - screen_y;
                if dx * dx + dy * dy > radius_sq {
                    continue;
                }
                let index = (y * WIDTH + x) as usize;
                if let Some(geometry_depth) = geometry_depth {
                    let opaque = geometry_depth.get_pixel(x, y);
                    // The fixed depth AOV uses the opaque background palette
                    // [8,12,18]. Any foreground value is reversed normalized
                    // camera depth, matching the base renderer's contract.
                    let background = opaque[0] == 8 && opaque[1] == 12 && opaque[2] == 18;
                    if !background {
                        let opaque_depth = 1.0 - opaque[0] as f32 / 255.0;
                        if opaque_depth <= projected_depth + 1.0e-4 {
                            continue;
                        }
                    }
                }
                let destination = *color.get_pixel(x, y);
                let src_alpha = particle.alpha.clamp(0.0, 1.0);
                let dst_alpha = destination[3] as f32 / 255.0;
                let out_alpha = src_alpha + dst_alpha * (1.0 - src_alpha);
                if out_alpha > 0.0 {
                    let mut output = [0_u8; 4];
                    for channel in 0..3 {
                        let value = (source[channel] as f32 * src_alpha
                            + destination[channel] as f32 * dst_alpha * (1.0 - src_alpha))
                            / out_alpha;
                        output[channel] = value.round().clamp(0.0, 255.0) as u8;
                    }
                    output[3] = (out_alpha * 255.0).round().clamp(0.0, 255.0) as u8;
                    color.put_pixel(x, y, Rgba(output));
                }
                if projected_depth < nearest_depth[index]
                    || ((projected_depth - nearest_depth[index]).abs() <= f32::EPSILON
                        && particle.id < nearest_id[index])
                {
                    nearest_depth[index] = projected_depth;
                    nearest_id[index] = particle.id;
                    id.put_pixel(x, y, Rgba(particle_id_color(particle.id)));
                    let value = ((1.0 - projected_depth) * 255.0).round() as u8;
                    depth.put_pixel(x, y, Rgba([value, value, value, 255]));
                }
            }
        }
    }
    fn encode(image: RgbaImage, pass: &str) -> Result<RenderPass, RenderError> {
        let mut bytes = Vec::new();
        image::DynamicImage::ImageRgba8(image)
            .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)
            .map_err(|error| {
                RenderError::Invalid(format!("typed particle encode failed: {error}"))
            })?;
        Ok(RenderPass {
            pass: pass.to_owned(),
            png: bytes,
            width: WIDTH,
            height: HEIGHT,
        })
    }
    Ok(vec![
        encode(color, "particle-color")?,
        encode(id, "particle-id")?,
        encode(depth, "particle-depth")?,
    ])
}

/// Render one bounded typed-trail AOV set against the opaque depth of the
/// exact candidate GLB.  The GLB is read only for the depth buffer; no base or
/// bloom pass is returned or modified by this operation.
pub fn render_typed_trails_with_glb(
    glb: &[u8],
    camera: &Value,
    trails: &[TypedTrail],
) -> Result<Vec<RenderPass>, RenderError> {
    let depth_passes =
        render_perspective_glb_at_resolution_with_passes(glb, camera, 512, &["depth"], &[], None)?;
    let depth_pass = depth_passes
        .first()
        .ok_or_else(|| RenderError::Invalid("opaque depth pass is unavailable".to_owned()))?;
    let depth_image = image::load_from_memory(&depth_pass.png)
        .map_err(|error| RenderError::Invalid(format!("opaque depth pass decode failed: {error}")))?
        .to_rgba8();
    render_typed_trails_internal(camera, trails, Some(&depth_image))
}

/// Render the typed trail AOVs and their independent Bloom passes against the
/// opaque depth of the exact candidate GLB.  The first three passes are
/// produced by the same raster result as `render_typed_trails_with_glb`; the
/// Bloom passes are appended and never replace or mutate the original trail or
/// material Bloom outputs.
pub fn render_typed_trails_bloom_with_glb(
    glb: &[u8],
    camera: &Value,
    trails: &[TypedTrail],
    profile: TypedTrailBloomProfile,
) -> Result<Vec<RenderPass>, RenderError> {
    let profile = profile.validate_fixed()?;
    let depth_passes =
        render_perspective_glb_at_resolution_with_passes(glb, camera, 512, &["depth"], &[], None)?;
    let depth_pass = depth_passes
        .first()
        .ok_or_else(|| RenderError::Invalid("opaque depth pass is unavailable".to_owned()))?;
    let depth_image = image::load_from_memory(&depth_pass.png)
        .map_err(|error| RenderError::Invalid(format!("opaque depth pass decode failed: {error}")))?
        .to_rgba8();
    render_typed_trails_bloom_internal(camera, trails, Some(&depth_image), profile)
}

/// Render typed trails without geometry occlusion.  This is used only by
/// focused Render Core tests; the product Worker always calls the GLB-bound
/// function above.
pub fn render_typed_trails(
    camera: &Value,
    trails: &[TypedTrail],
) -> Result<Vec<RenderPass>, RenderError> {
    render_typed_trails_internal(camera, trails, None)
}

#[derive(Debug)]
struct TypedTrailRaster {
    color: RgbaImage,
    id: RgbaImage,
    depth: RgbaImage,
    /// Linear, straight-alpha color retained before display encoding.  The
    /// three persisted trail AOVs are still generated from `color`, so this
    /// buffer cannot change their byte representation.
    linear_color: Vec<[f32; 3]>,
    linear_alpha: Vec<f32>,
    nearest_depth: Vec<f32>,
    nearest_id: Vec<u32>,
}

fn render_typed_trails_internal(
    camera: &Value,
    trails: &[TypedTrail],
    geometry_depth: Option<&RgbaImage>,
) -> Result<Vec<RenderPass>, RenderError> {
    let raster = rasterize_typed_trails_internal(camera, trails, geometry_depth)?;
    Ok(vec![
        encode_typed_trail_image(raster.color, "trail-color")?,
        encode_typed_trail_image(raster.id, "trail-id")?,
        encode_typed_trail_image(raster.depth, "trail-depth")?,
    ])
}

fn render_typed_trails_bloom_internal(
    camera: &Value,
    trails: &[TypedTrail],
    geometry_depth: Option<&RgbaImage>,
    profile: TypedTrailBloomProfile,
) -> Result<Vec<RenderPass>, RenderError> {
    let profile = profile.validate_fixed()?;
    let raster = rasterize_typed_trails_internal(camera, trails, geometry_depth)?;
    let mut passes = vec![
        encode_typed_trail_image(raster.color, "trail-color")?,
        encode_typed_trail_image(raster.id, "trail-id")?,
        encode_typed_trail_image(raster.depth, "trail-depth")?,
    ];
    let pixel_count = (TYPED_TRAIL_WIDTH * TYPED_TRAIL_HEIGHT) as usize;
    let mut source = vec![[0.0_f32; 4]; pixel_count];
    for index in 0..pixel_count {
        if raster.nearest_id[index] == u32::MAX || raster.linear_alpha[index] <= 0.0 {
            continue;
        }
        let alpha = raster.linear_alpha[index].clamp(0.0, 1.0);
        for channel in 0..3 {
            source[index][channel] =
                (raster.linear_color[index][channel] * alpha * profile.source_gain)
                    .clamp(0.0, profile.hdr_clamp);
        }
        source[index][3] = 1.0;
    }
    let thresholded = source
        .iter()
        .map(|pixel| {
            let mut value = [0.0_f32; 4];
            for channel in 0..3 {
                value[channel] = ((pixel[channel] - profile.threshold).max(0.0)
                    * profile.intensity)
                    .min(profile.hdr_clamp);
            }
            value[3] = 1.0;
            value
        })
        .collect::<Vec<_>>();
    let mut contribution = separable_blur_two_passes(
        &thresholded,
        TYPED_TRAIL_WIDTH,
        TYPED_TRAIL_HEIGHT,
        profile.radius_px,
        profile.hdr_clamp,
    );
    // The source raster already applies the opaque depth test.  Keep the
    // screen-space blur from leaking through a nearer opaque surface by
    // propagating the nearest visible trail depth through the same bounded
    // separable support and masking the final contribution at the target.
    let source_depth = raster
        .nearest_depth
        .iter()
        .zip(&raster.nearest_id)
        .map(|(depth, id)| {
            if *id == u32::MAX {
                f32::INFINITY
            } else {
                *depth
            }
        })
        .collect::<Vec<_>>();
    let propagated_depth = separable_min_depth(
        &source_depth,
        TYPED_TRAIL_WIDTH,
        TYPED_TRAIL_HEIGHT,
        profile.radius_px,
    );
    for y in 0..TYPED_TRAIL_HEIGHT {
        for x in 0..TYPED_TRAIL_WIDTH {
            let index = (y * TYPED_TRAIL_WIDTH + x) as usize;
            let blocked = geometry_depth.is_some_and(|opaque_image| {
                let opaque = opaque_image.get_pixel(x, y);
                let background = opaque[0] == 8 && opaque[1] == 12 && opaque[2] == 18;
                if background {
                    false
                } else {
                    let opaque_depth = 1.0 - opaque[0] as f32 / 255.0;
                    !propagated_depth[index].is_finite()
                        || opaque_depth <= propagated_depth[index] + 1.0e-4
                }
            });
            if blocked {
                contribution[index] = [0.0; 4];
            }
        }
    }
    let mut source_image =
        RgbaImage::from_pixel(TYPED_TRAIL_WIDTH, TYPED_TRAIL_HEIGHT, Rgba([0, 0, 0, 255]));
    let mut contribution_image = source_image.clone();
    for y in 0..TYPED_TRAIL_HEIGHT {
        for x in 0..TYPED_TRAIL_WIDTH {
            let index = (y * TYPED_TRAIL_WIDTH + x) as usize;
            source_image.put_pixel(
                x,
                y,
                Rgba(hdr_pixel_to_rgba8(source[index], profile.hdr_clamp)),
            );
            contribution_image.put_pixel(
                x,
                y,
                Rgba(hdr_pixel_to_rgba8(contribution[index], profile.hdr_clamp)),
            );
        }
    }
    passes.push(encode_typed_trail_image(
        source_image,
        "trail-emissive-source",
    )?);
    passes.push(encode_typed_trail_image(
        contribution_image,
        "trail-bloom-contribution",
    )?);
    Ok(passes)
}

const TYPED_TRAIL_WIDTH: u32 = 512;
const TYPED_TRAIL_HEIGHT: u32 = 512;
const TYPED_TRAIL_MAX_TRAILS: usize = 16;
const TYPED_TRAIL_MAX_POINTS_PER_TRAIL: usize = 32;
const TYPED_TRAIL_MAX_SEGMENTS: usize = 128;

fn encode_typed_trail_image(image: RgbaImage, pass: &str) -> Result<RenderPass, RenderError> {
    let mut bytes = Vec::new();
    image::DynamicImage::ImageRgba8(image)
        .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)
        .map_err(|error| RenderError::Invalid(format!("typed trail encode failed: {error}")))?;
    Ok(RenderPass {
        pass: pass.to_owned(),
        png: bytes,
        width: TYPED_TRAIL_WIDTH,
        height: TYPED_TRAIL_HEIGHT,
    })
}

fn rasterize_typed_trails_internal(
    camera: &Value,
    trails: &[TypedTrail],
    geometry_depth: Option<&RgbaImage>,
) -> Result<TypedTrailRaster, RenderError> {
    if trails.is_empty() || trails.len() > TYPED_TRAIL_MAX_TRAILS {
        return Err(RenderError::Invalid(
            "typed trail count is outside the bounded domain".to_owned(),
        ));
    }
    let (camera_position, forward, right, up, projection, near, far) = parse_camera(camera)?;
    let aspect = TYPED_TRAIL_WIDTH as f32 / TYPED_TRAIL_HEIGHT as f32;
    let mut projected = Vec::<(usize, Vec<([f32; 2], f32)>)>::with_capacity(trails.len());
    let mut segment_count = 0usize;
    for (trail_index, trail) in trails.iter().enumerate() {
        if trail.emitter_id != "muzzle-trail" && trail.emitter_id != "energy-core-trail" {
            return Err(RenderError::Invalid(
                "typed trail emitter is not in the closed set".to_owned(),
            ));
        }
        if !(1..=65_535).contains(&trail.id)
            || trail.points.len() < 2
            || trail.points.len() > TYPED_TRAIL_MAX_POINTS_PER_TRAIL
            || !trail.radius_px.is_finite()
            || !(1.0..=8.0).contains(&trail.radius_px)
            || trail
                .color_linear_rgb
                .iter()
                .any(|value| !value.is_finite() || !(0.0..=1.0).contains(value))
            || !trail.alpha.is_finite()
            || !(0.0..=1.0).contains(&trail.alpha)
            || trail.lifetime_ticks == 0
            || trail.lifetime_ticks > 1_000_000
            || trail.points.iter().any(|point| {
                point
                    .iter()
                    .any(|value| !value.is_finite() || value.abs() > 10.0)
            })
        {
            return Err(RenderError::Invalid(
                "typed trail attribute is outside the bounded domain".to_owned(),
            ));
        }
        let mut points = Vec::with_capacity(trail.points.len());
        for point in &trail.points {
            let relative = subtract3(*point, camera_position);
            let z = dot3(relative, forward);
            if !z.is_finite() || z <= near || z >= far {
                return Err(RenderError::Invalid(
                    "typed trail point falls outside the fixed camera clip range".to_owned(),
                ));
            }
            let depth = ((z - near) / (far - near)).clamp(0.0, 1.0);
            let x = dot3(relative, right);
            let y = dot3(relative, up);
            let (ndc_x, ndc_y) = match projection {
                CameraProjection::Perspective { focal } => {
                    ((x * focal / aspect) / z, (y * focal) / z)
                }
                CameraProjection::Orthographic { scale } => {
                    ((x / (scale * aspect * 0.5)), (y / (scale * 0.5)))
                }
            };
            let screen_x = (ndc_x * 0.5 + 0.5) * TYPED_TRAIL_WIDTH as f32;
            let screen_y = (1.0 - (ndc_y * 0.5 + 0.5)) * TYPED_TRAIL_HEIGHT as f32;
            if !screen_x.is_finite()
                || !screen_y.is_finite()
                || screen_x < -8.0
                || screen_x > TYPED_TRAIL_WIDTH as f32 + 8.0
                || screen_y < -8.0
                || screen_y > TYPED_TRAIL_HEIGHT as f32 + 8.0
            {
                return Err(RenderError::Invalid(
                    "typed trail point is outside the bounded screen domain".to_owned(),
                ));
            }
            points.push(([screen_x, screen_y], depth));
        }
        segment_count = segment_count.saturating_add(points.len().saturating_sub(1));
        if segment_count > TYPED_TRAIL_MAX_SEGMENTS {
            return Err(RenderError::Invalid(
                "typed trail segment count exceeds the fixed limit".to_owned(),
            ));
        }
        projected.push((trail_index, points));
    }

    let mut color =
        RgbaImage::from_pixel(TYPED_TRAIL_WIDTH, TYPED_TRAIL_HEIGHT, Rgba([0, 0, 0, 0]));
    let mut id = RgbaImage::from_pixel(TYPED_TRAIL_WIDTH, TYPED_TRAIL_HEIGHT, Rgba([0, 0, 0, 0]));
    let mut depth =
        RgbaImage::from_pixel(TYPED_TRAIL_WIDTH, TYPED_TRAIL_HEIGHT, Rgba([0, 0, 0, 0]));
    let mut linear_color = vec![[0.0_f32; 3]; (TYPED_TRAIL_WIDTH * TYPED_TRAIL_HEIGHT) as usize];
    let mut linear_alpha = vec![0.0_f32; (TYPED_TRAIL_WIDTH * TYPED_TRAIL_HEIGHT) as usize];
    let mut nearest_depth = vec![1.0_f32; (TYPED_TRAIL_WIDTH * TYPED_TRAIL_HEIGHT) as usize];
    let mut nearest_id = vec![u32::MAX; (TYPED_TRAIL_WIDTH * TYPED_TRAIL_HEIGHT) as usize];
    let mut rendered_segments = 0usize;
    for (trail_index, points) in projected {
        let trail = &trails[trail_index];
        let source = [
            linear_to_srgb(trail.color_linear_rgb[0]),
            linear_to_srgb(trail.color_linear_rgb[1]),
            linear_to_srgb(trail.color_linear_rgb[2]),
        ];
        for segment in points.windows(2) {
            let ([x0, y0], depth0) = segment[0];
            let ([x1, y1], depth1) = segment[1];
            let dx = x1 - x0;
            let dy = y1 - y0;
            let length_sq = dx * dx + dy * dy;
            if !length_sq.is_finite() || length_sq <= f32::EPSILON {
                continue;
            }
            rendered_segments += 1;
            let radius = trail.radius_px;
            let min_x = (x0.min(x1).floor() as i32 - radius.ceil() as i32)
                .max(0)
                .min(TYPED_TRAIL_WIDTH as i32 - 1) as u32;
            let max_x = (x0.max(x1).ceil() as i32 + radius.ceil() as i32)
                .min(TYPED_TRAIL_WIDTH as i32 - 1)
                .max(0) as u32;
            let min_y = (y0.min(y1).floor() as i32 - radius.ceil() as i32)
                .max(0)
                .min(TYPED_TRAIL_HEIGHT as i32 - 1) as u32;
            let max_y = (y0.max(y1).ceil() as i32 + radius.ceil() as i32)
                .min(TYPED_TRAIL_HEIGHT as i32 - 1)
                .max(0) as u32;
            let radius_sq = radius * radius;
            for y in min_y..=max_y {
                for x in min_x..=max_x {
                    let px = x as f32 + 0.5;
                    let py = y as f32 + 0.5;
                    let t = (((px - x0) * dx + (py - y0) * dy) / length_sq).clamp(0.0, 1.0);
                    let nearest_x = x0 + t * dx;
                    let nearest_y = y0 + t * dy;
                    let distance_x = px - nearest_x;
                    let distance_y = py - nearest_y;
                    if distance_x * distance_x + distance_y * distance_y > radius_sq {
                        continue;
                    }
                    let projected_depth = depth0 + (depth1 - depth0) * t;
                    let index = (y * TYPED_TRAIL_WIDTH + x) as usize;
                    if let Some(geometry_depth) = geometry_depth {
                        let opaque = geometry_depth.get_pixel(x, y);
                        let background = opaque[0] == 8 && opaque[1] == 12 && opaque[2] == 18;
                        if !background {
                            let opaque_depth = 1.0 - opaque[0] as f32 / 255.0;
                            if opaque_depth <= projected_depth + 1.0e-4 {
                                continue;
                            }
                        }
                    }
                    let destination = *color.get_pixel(x, y);
                    let src_alpha = trail.alpha.clamp(0.0, 1.0);
                    let dst_alpha = destination[3] as f32 / 255.0;
                    let out_alpha = src_alpha + dst_alpha * (1.0 - src_alpha);
                    if out_alpha > 0.0 {
                        let mut output = [0_u8; 4];
                        for channel in 0..3 {
                            let value = (source[channel] as f32 * src_alpha
                                + destination[channel] as f32 * dst_alpha * (1.0 - src_alpha))
                                / out_alpha;
                            output[channel] = value.round().clamp(0.0, 255.0) as u8;
                        }
                        output[3] = (out_alpha * 255.0).round().clamp(0.0, 255.0) as u8;
                        color.put_pixel(x, y, Rgba(output));

                        // Keep a separate linear-space trail raster for the
                        // Bloom operation.  The persisted color pass above
                        // remains the original sRGB-u8 path byte-for-byte.
                        let linear_destination = linear_color[index];
                        let linear_destination_alpha = linear_alpha[index];
                        let mut linear_output = [0.0_f32; 3];
                        for channel in 0..3 {
                            linear_output[channel] = (trail.color_linear_rgb[channel] * src_alpha
                                + linear_destination[channel]
                                    * linear_destination_alpha
                                    * (1.0 - src_alpha))
                                / out_alpha;
                        }
                        linear_color[index] = linear_output;
                        linear_alpha[index] = output[3] as f32 / 255.0;
                    }
                    if projected_depth < nearest_depth[index]
                        || ((projected_depth - nearest_depth[index]).abs() <= f32::EPSILON
                            && trail.id < nearest_id[index])
                    {
                        nearest_depth[index] = projected_depth;
                        nearest_id[index] = trail.id;
                        id.put_pixel(x, y, Rgba(particle_id_color(trail.id)));
                        let value = ((1.0 - projected_depth) * 255.0).round() as u8;
                        depth.put_pixel(x, y, Rgba([value, value, value, 255]));
                    }
                }
            }
        }
    }
    if rendered_segments == 0 {
        return Err(RenderError::Invalid(
            "typed trail set contains no visible segments".to_owned(),
        ));
    }
    Ok(TypedTrailRaster {
        color,
        id,
        depth,
        linear_color,
        linear_alpha,
        nearest_depth,
        nearest_id,
    })
}

fn separable_min_depth(input: &[f32], width: u32, height: u32, radius: u32) -> Vec<f32> {
    let radius = radius as i32;
    let width_i32 = width as i32;
    let height_i32 = height as i32;
    let mut horizontal = vec![f32::INFINITY; input.len()];
    for y in 0..height_i32 {
        for x in 0..width_i32 {
            let mut minimum = f32::INFINITY;
            for offset in -radius..=radius {
                let sample_x = (x + offset).clamp(0, width_i32 - 1) as u32;
                minimum = minimum.min(input[(y as u32 * width + sample_x) as usize]);
            }
            horizontal[(y as u32 * width + x as u32) as usize] = minimum;
        }
    }
    let mut vertical = vec![f32::INFINITY; input.len()];
    for x in 0..width_i32 {
        for y in 0..height_i32 {
            let mut minimum = f32::INFINITY;
            for offset in -radius..=radius {
                let sample_y = (y + offset).clamp(0, height_i32 - 1) as u32;
                minimum = minimum.min(horizontal[(sample_y * width + x as u32) as usize]);
            }
            vertical[(y as u32 * width + x as u32) as usize] = minimum;
        }
    }
    vertical
}

fn required_lineage_text(
    extras: &Map<String, Value>,
    field: &str,
    label: &str,
) -> Result<String, RenderError> {
    extras
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty() && value.len() <= 256)
        .map(str::to_owned)
        .ok_or_else(|| RenderError::Invalid(format!("{label} lineage {field} is missing")))
}

fn raster_source_lineage(
    mesh: &Map<String, Value>,
    primitive: &Map<String, Value>,
    mesh_index: usize,
    primitive_index: usize,
) -> Result<(String, String, Vec<String>, String), RenderError> {
    let primitive_extras = primitive
        .get("extras")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            RenderError::Invalid(format!(
                "raster attribution primitive {mesh_index}:{primitive_index} lineage is missing"
            ))
        })?;
    let semantic_part_id =
        required_lineage_text(primitive_extras, "part_id", "raster attribution primitive")?;
    let source_node_id = required_lineage_text(
        primitive_extras,
        "source_node_id",
        "raster attribution primitive",
    )?;
    let material_zone_id = required_lineage_text(
        primitive_extras,
        "material_zone_id",
        "raster attribution primitive",
    )?;
    let lineage_source_node_ids = primitive_extras
        .get("lineage_source_node_ids")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            RenderError::Invalid(
                "raster attribution primitive lineage_source_node_ids is missing".to_owned(),
            )
        })?
        .iter()
        .map(|value| {
            value
                .as_str()
                .filter(|value| !value.is_empty() && value.len() <= 256)
                .map(str::to_owned)
                .ok_or_else(|| {
                    RenderError::Invalid(
                        "raster attribution source lineage node id is invalid".to_owned(),
                    )
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    if lineage_source_node_ids.is_empty()
        || !lineage_source_node_ids
            .iter()
            .any(|value| value == &source_node_id)
        || lineage_source_node_ids.iter().collect::<HashSet<_>>().len()
            != lineage_source_node_ids.len()
    {
        return Err(RenderError::Invalid(
            "raster attribution source lineage is ambiguous".to_owned(),
        ));
    }
    if let Some(mesh_extras) = mesh.get("extras").and_then(Value::as_object) {
        for (field, expected) in [
            ("part_id", semantic_part_id.as_str()),
            ("material_zone_id", material_zone_id.as_str()),
        ] {
            if mesh_extras
                .get(field)
                .and_then(Value::as_str)
                .is_some_and(|observed| observed != expected)
            {
                return Err(RenderError::Invalid(format!(
                    "raster attribution mesh {mesh_index} lineage {field} differs"
                )));
            }
        }
    }
    Ok((
        semantic_part_id,
        source_node_id,
        lineage_source_node_ids,
        material_zone_id,
    ))
}

fn quantize_barycentric(values: [f32; 3]) -> [u16; 3] {
    values.map(|value| (value.clamp(0.0, 1.0) * 1000.0).round() as u16)
}

fn render_perspective_glb_at_resolution_with_passes(
    glb: &[u8],
    camera: &Value,
    resolution: u32,
    requested_passes: &[&str],
    emissive_overrides: &[EmissiveMaterialOverride],
    hdr_bloom_profile: Option<&HdrBloomProfile>,
) -> Result<Vec<RenderPass>, RenderError> {
    render_perspective_glb_at_resolution_with_passes_bounded(
        glb,
        camera,
        resolution,
        requested_passes,
        emissive_overrides,
        hdr_bloom_profile,
        false,
        false,
    )
    .map(|(passes, _)| passes)
}

fn render_perspective_glb_at_resolution_with_passes_bounded(
    glb: &[u8],
    camera: &Value,
    resolution: u32,
    requested_passes: &[&str],
    emissive_overrides: &[EmissiveMaterialOverride],
    hdr_bloom_profile: Option<&HdrBloomProfile>,
    allow_hero_resolution: bool,
    capture_raster_attribution: bool,
) -> Result<(Vec<RenderPass>, Option<RasterHitSourceMap>), RenderError> {
    let fixed_resolution = (64..=512).contains(&resolution);
    let hero_resolution = allow_hero_resolution && resolution == HERO_BEAUTY_RESOLUTION;
    if !fixed_resolution && !hero_resolution {
        return Err(RenderError::Invalid(
            "fit render resolution is outside the bounded range".to_owned(),
        ));
    }
    if requested_passes.is_empty()
        || requested_passes.iter().any(|pass| {
            !matches!(
                *pass,
                "beauty"
                    | "silhouette"
                    | "depth"
                    | "normal"
                    | "ao"
                    | "part-id"
                    | "material-id"
                    | "wireframe"
                    | "uv-stretch"
                    | "emissive-source"
                    | "bloom-contribution"
            )
        })
    {
        return Err(RenderError::Invalid(
            "requested render passes are outside the fixed allowlist".to_owned(),
        ));
    }
    if requested_passes
        .iter()
        .any(|pass| matches!(*pass, "emissive-source" | "bloom-contribution"))
        && hdr_bloom_profile.is_none()
    {
        return Err(RenderError::Invalid(
            "HDR bloom passes require a bounded profile".to_owned(),
        ));
    }
    let hdr_bloom_profile = hdr_bloom_profile
        .map(|profile| profile.validate())
        .transpose()?;
    let (root, binary) = parse_glb(glb)?;
    validate_id_palette_domain(&root)?;
    let resolved_overrides = resolve_emissive_material_overrides(&root, emissive_overrides)?;
    let mut override_by_material = vec![None; ID_PALETTE_CAPACITY];
    for (override_value, resolved) in emissive_overrides.iter().zip(&resolved_overrides) {
        override_by_material[resolved.glb_material_index] = Some(override_value);
    }
    let meshes = root
        .get("meshes")
        .and_then(Value::as_array)
        .ok_or_else(|| RenderError::Invalid("GLB meshes are missing".to_owned()))?;
    let accessors = root
        .get("accessors")
        .and_then(Value::as_array)
        .ok_or_else(|| RenderError::Invalid("GLB accessors are missing".to_owned()))?;
    let views = root
        .get("bufferViews")
        .and_then(Value::as_array)
        .ok_or_else(|| RenderError::Invalid("GLB bufferViews are missing".to_owned()))?;
    let nodes = root
        .get("nodes")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let instances = scene_mesh_instances(&root, &nodes)?;
    if instances.is_empty() {
        return Err(RenderError::Invalid(
            "GLB has no scene mesh instances".to_owned(),
        ));
    }
    let (camera_position, forward, right, up, projection, near, far) = parse_camera(camera)?;
    let textures = if requested_passes.contains(&"beauty")
        || requested_passes.contains(&"emissive-source")
        || requested_passes.contains(&"bloom-contribution")
    {
        embedded_render_textures(&root, &views, &binary)?
    } else {
        Vec::new()
    };
    let width = resolution;
    let height = resolution;
    // The fixed nine-AOV renderer keeps its deterministic 2x supersampling
    // path.  The transient fit renderer only asks for binary silhouette and
    // Part-ID passes.  Keep the cheaper half-resolution raster only for the
    // 128px exploratory contract; a 512px fit must use the exact same 1024px
    // sample grid as the formal 512px comparison renderer, otherwise Primary
    // Form can optimize a different contour than the acceptance gate.
    let transient_binary_fit = requested_passes.len() == 2
        && requested_passes.contains(&"silhouette")
        && requested_passes.contains(&"part-id");
    let raster_resolution = if allow_hero_resolution {
        HERO_BEAUTY_RASTER_RESOLUTION
    } else if transient_binary_fit {
        if resolution == 512 {
            resolution * 2
        } else {
            // A 64px binary raster is sufficient for ranking a bounded
            // 128px/256px camera neighborhood; the result is deterministically
            // upsampled to the transient contract and is never persisted.
            (resolution / 2).max(64)
        }
    } else {
        resolution * 2
    };
    let sample_width = raster_resolution;
    let sample_height = raster_resolution;
    let mut hits = vec![None::<RasterHit>; (sample_width * sample_height) as usize];
    let aspect = width as f32 / height as f32;
    let mut rendered_triangles = 0usize;
    let mut raster_sources = Vec::<RasterHitSourceMapEntry>::new();
    let mut primitive_source_bases = vec![Vec::<u32>::new(); meshes.len()];
    if capture_raster_attribution {
        for (mesh_index, mesh) in meshes.iter().enumerate() {
            let mesh = mesh.as_object().ok_or_else(|| {
                RenderError::Invalid("raster attribution mesh is invalid".to_owned())
            })?;
            let primitives = mesh
                .get("primitives")
                .and_then(Value::as_array)
                .ok_or_else(|| {
                    RenderError::Invalid("raster attribution primitive list is missing".to_owned())
                })?;
            for (primitive_index, primitive) in primitives.iter().enumerate() {
                let primitive = primitive.as_object().ok_or_else(|| {
                    RenderError::Invalid("raster attribution primitive is invalid".to_owned())
                })?;
                let (semantic_part_id, source_node_id, lineage_source_node_ids, material_zone_id) =
                    raster_source_lineage(mesh, primitive, mesh_index, primitive_index)?;
                let index_accessor = primitive
                    .get("indices")
                    .and_then(Value::as_u64)
                    .ok_or_else(|| {
                        RenderError::Invalid(
                            "raster attribution index accessor is missing".to_owned(),
                        )
                    })? as usize;
                let indices = read_indices_accessor(accessors, views, &binary, index_accessor)?;
                let triangle_count = indices.len() / 3;
                let first_triangle = u32::try_from(raster_sources.len()).map_err(|_| {
                    RenderError::Invalid("raster attribution triangle map exceeds u32".to_owned())
                })?;
                primitive_source_bases[mesh_index].push(first_triangle);
                for triangle_index_in_primitive in 0..triangle_count {
                    let triangle_index = u32::try_from(raster_sources.len()).map_err(|_| {
                        RenderError::Invalid(
                            "raster attribution triangle map exceeds u32".to_owned(),
                        )
                    })?;
                    raster_sources.push(RasterHitSourceMapEntry {
                        triangle_index,
                        mesh_index: mesh_index as u32,
                        primitive_index: primitive_index as u32,
                        triangle_index_in_primitive: triangle_index_in_primitive as u32,
                        semantic_part_id: semantic_part_id.clone(),
                        source_node_id: source_node_id.clone(),
                        lineage_source_node_ids: lineage_source_node_ids.clone(),
                        material_zone_id: material_zone_id.clone(),
                    });
                }
            }
        }
        if raster_sources.is_empty() {
            return Err(RenderError::Invalid(
                "raster attribution has no triangles".to_owned(),
            ));
        }
    }

    for (mesh_index, transform) in instances {
        let mesh = meshes
            .get(mesh_index)
            .and_then(Value::as_object)
            .ok_or_else(|| RenderError::Invalid("GLB scene mesh is invalid".to_owned()))?;
        let primitives = mesh
            .get("primitives")
            .and_then(Value::as_array)
            .ok_or_else(|| RenderError::Invalid("GLB primitive list is missing".to_owned()))?;
        for (primitive_index, primitive) in primitives.iter().enumerate() {
            let attributes = primitive
                .get("attributes")
                .and_then(Value::as_object)
                .ok_or_else(|| {
                    RenderError::Invalid("GLB primitive attributes are missing".to_owned())
                })?;
            let position_accessor = attributes
                .get("POSITION")
                .and_then(Value::as_u64)
                .ok_or_else(|| {
                    RenderError::Invalid("GLB POSITION accessor is missing".to_owned())
                })? as usize;
            let normal_accessor = attributes
                .get("NORMAL")
                .and_then(Value::as_u64)
                .ok_or_else(|| RenderError::Invalid("GLB NORMAL accessor is missing".to_owned()))?
                as usize;
            let uv_accessor = attributes
                .get("TEXCOORD_0")
                .and_then(Value::as_u64)
                .ok_or_else(|| {
                    RenderError::Invalid("GLB TEXCOORD_0 accessor is missing".to_owned())
                })? as usize;
            let index_accessor = primitive
                .get("indices")
                .and_then(Value::as_u64)
                .ok_or_else(|| RenderError::Invalid("GLB index accessor is missing".to_owned()))?
                as usize;
            let positions = read_vec3_accessor(accessors, views, &binary, position_accessor)?;
            let normals = read_vec3_accessor(accessors, views, &binary, normal_accessor)?;
            let uvs = read_vec2_accessor(accessors, views, &binary, uv_accessor)?;
            let tangents = attributes
                .get("TANGENT")
                .and_then(Value::as_u64)
                .map(|index| read_vec4_accessor(accessors, views, &binary, index as usize))
                .transpose()?
                .unwrap_or_else(|| vec![[1.0, 0.0, 0.0, 1.0]; positions.len()]);
            let indices = read_indices_accessor(accessors, views, &binary, index_accessor)?;
            if positions.len() != normals.len()
                || positions.len() != uvs.len()
                || positions.len() != tangents.len()
            {
                return Err(RenderError::Invalid(
                    "GLB render attributes have mismatched counts".to_owned(),
                ));
            }
            let material_index = primitive
                .get("material")
                .and_then(Value::as_u64)
                .unwrap_or(0) as usize;
            for (triangle_index_in_primitive, index_triplet) in indices.chunks_exact(3).enumerate()
            {
                let mut projected = [PerspectiveVertex {
                    screen_x: 0.0,
                    screen_y: 0.0,
                    depth: 0.0,
                    world: [0.0; 3],
                    normal: [0.0, 1.0, 0.0],
                    tangent: [1.0, 0.0, 0.0, 1.0],
                    uv: [0.0; 2],
                }; 3];
                let mut rejected = false;
                for (slot, source_index) in index_triplet.iter().enumerate() {
                    let source_index = *source_index as usize;
                    let world = transform_point(
                        transform,
                        positions.get(source_index).copied().ok_or_else(|| {
                            RenderError::Invalid("GLB render index is invalid".to_owned())
                        })?,
                    );
                    let normal = normalize(transform_direction(transform, normals[source_index]));
                    let source_tangent = tangents[source_index];
                    let tangent_direction = normalize(transform_direction(
                        transform,
                        [source_tangent[0], source_tangent[1], source_tangent[2]],
                    ));
                    let relative = subtract3(world, camera_position);
                    let z = dot3(relative, forward);
                    if !z.is_finite() || z <= near || z >= far {
                        rejected = true;
                        break;
                    }
                    let x = dot3(relative, right);
                    let y = dot3(relative, up);
                    let (ndc_x, ndc_y) = match projection {
                        CameraProjection::Perspective { focal } => {
                            ((x * focal / aspect) / z, (y * focal) / z)
                        }
                        CameraProjection::Orthographic { scale } => {
                            ((x / (scale * aspect * 0.5)), (y / (scale * 0.5)))
                        }
                    };
                    projected[slot] = PerspectiveVertex {
                        screen_x: (ndc_x * 0.5 + 0.5) * sample_width as f32,
                        screen_y: (1.0 - (ndc_y * 0.5 + 0.5)) * sample_height as f32,
                        depth: (z - near) / (far - near),
                        world,
                        normal,
                        tangent: [
                            tangent_direction[0],
                            tangent_direction[1],
                            tangent_direction[2],
                            if source_tangent[3].is_sign_negative() {
                                -1.0
                            } else {
                                1.0
                            },
                        ],
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
                let triangle_index = if capture_raster_attribution {
                    primitive_source_bases
                        .get(mesh_index)
                        .and_then(|bases| bases.get(primitive_index))
                        .copied()
                        .and_then(|base| base.checked_add(triangle_index_in_primitive as u32))
                        .ok_or_else(|| {
                            RenderError::Invalid(
                                "raster attribution triangle lineage is unavailable".to_owned(),
                            )
                        })?
                } else {
                    0
                };
                rasterize_perspective_triangle(
                    &mut hits,
                    sample_width,
                    sample_height,
                    projected,
                    area,
                    mesh_index,
                    material_index,
                    triangle_index,
                    triangle_index,
                );
            }
        }
    }
    if rendered_triangles == 0 {
        return Err(RenderError::Invalid(
            "camera produced no visible triangles".to_owned(),
        ));
    }
    let (emissive_source, bloom_contribution) = if let Some(profile) = hdr_bloom_profile {
        let source = build_hdr_emissive_source(
            &root,
            &textures,
            &hits,
            sample_width,
            sample_height,
            width,
            height,
            &override_by_material,
            &profile,
        );
        let thresholded = source
            .iter()
            .map(|pixel| {
                let mut value = [0.0_f32; 4];
                for channel in 0..3 {
                    value[channel] = ((pixel[channel] - profile.threshold).max(0.0)
                        * profile.intensity)
                        .min(profile.hdr_clamp);
                }
                value[3] = 1.0;
                value
            })
            .collect::<Vec<_>>();
        let blurred = separable_blur_two_passes(
            &thresholded,
            width,
            height,
            profile.radius_px,
            profile.hdr_clamp,
        );
        (source, blurred)
    } else {
        (Vec::new(), Vec::new())
    };
    let mut passes = Vec::with_capacity(requested_passes.len());
    for pass in requested_passes.iter().copied() {
        let background = if matches!(pass, "emissive-source" | "bloom-contribution") {
            [0, 0, 0, 255]
        } else {
            [8, 12, 18, 255]
        };
        let mut image = RgbaImage::from_pixel(sample_width, sample_height, Rgba(background));
        for y in 0..sample_height {
            for x in 0..sample_width {
                let index = (y * sample_width + x) as usize;
                let Some(hit) = hits[index] else { continue };
                let output_x = (x * width / sample_width).min(width - 1);
                let output_y = (y * height / sample_height).min(height - 1);
                let output_index = (output_y * width + output_x) as usize;
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
                    "emissive-source" => hdr_pixel_to_rgba8(
                        emissive_source
                            .get(output_index)
                            .copied()
                            .unwrap_or([0.0; 4]),
                        hdr_bloom_profile
                            .expect("HDR bloom profile validated")
                            .hdr_clamp,
                    ),
                    "bloom-contribution" => hdr_pixel_to_rgba8(
                        bloom_contribution
                            .get(output_index)
                            .copied()
                            .unwrap_or([0.0; 4]),
                        hdr_bloom_profile
                            .expect("HDR bloom profile validated")
                            .hdr_clamp,
                    ),
                    _ => shade_material(
                        &root,
                        &textures,
                        hit.material_index,
                        hit.normal,
                        hit.tangent,
                        hit.world,
                        camera_position,
                        hit.uv,
                        override_by_material[hit.material_index],
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
                RenderError::Invalid(format!("fixed render encode failed: {error}"))
            })?;
        passes.push(RenderPass {
            pass: pass.to_owned(),
            png: bytes,
            width,
            height,
        });
    }
    let source_map = if capture_raster_attribution {
        let mut pixels = Vec::with_capacity((width * height) as usize);
        for y in 0..height {
            // Keep the hit projection byte-for-byte aligned with the formal
            // `imageops::resize(..., FilterType::Nearest)` Part-ID output.
            // image's nearest sampler selects the source pixel at the output
            // pixel centre, so a 512 -> 1024 map starts at sample 1 rather
            // than sample 0.
            let sample_y = (((y as f32 + 0.5) * sample_height as f32 / height as f32).floor()
                as u32)
                .min(sample_height - 1);
            for x in 0..width {
                let sample_x = (((x as f32 + 0.5) * sample_width as f32 / width as f32).floor()
                    as u32)
                    .min(sample_width - 1);
                let hit =
                    hits[(sample_y * sample_width + sample_x) as usize].map(|hit| RasterPixelHit {
                        triangle_index: hit.triangle_index,
                        source_map_index: hit.source_map_index,
                        barycentric_milli: quantize_barycentric(hit.barycentric),
                        depth_micros: (hit.depth.clamp(0.0, 1.0) * 1_000_000.0).round() as u32,
                    });
                pixels.push(hit);
            }
        }
        Some(RasterHitSourceMap {
            width,
            height,
            raster_width: sample_width,
            raster_height: sample_height,
            pixels,
            sources: raster_sources,
        })
    } else {
        None
    };
    Ok((passes, source_map))
}

fn parse_camera(
    camera: &Value,
) -> Result<
    (
        [f32; 3],
        [f32; 3],
        [f32; 3],
        [f32; 3],
        CameraProjection,
        f32,
        f32,
    ),
    RenderError,
> {
    let object = camera
        .as_object()
        .ok_or_else(|| RenderError::Invalid("camera must be an object".to_owned()))?;
    let schema_version = object.get("schema_version").and_then(Value::as_str);
    let projection = object.get("projection").and_then(Value::as_str);
    if !matches!(
        schema_version,
        Some("CameraCalibration@1" | "CameraCalibration@2")
    ) || !matches!(projection, Some("perspective" | "orthographic"))
        || (schema_version == Some("CameraCalibration@1") && projection != Some("perspective"))
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
        return Err(RenderError::Invalid(
            "CameraCalibration is not the fixed bounded camera contract".to_owned(),
        ));
    }
    let transform = object
        .get("transform")
        .and_then(Value::as_object)
        .ok_or_else(|| RenderError::Invalid("camera transform is missing".to_owned()))?;
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
        return Err(RenderError::Invalid(
            "camera basis is degenerate".to_owned(),
        ));
    }
    let near = object
        .get("near_m")
        .and_then(Value::as_f64)
        .ok_or_else(|| RenderError::Invalid("camera near is missing".to_owned()))?
        as f32;
    let far = object
        .get("far_m")
        .and_then(Value::as_f64)
        .ok_or_else(|| RenderError::Invalid("camera far is missing".to_owned()))?
        as f32;
    if !(near > 0.0 && far > near) {
        return Err(RenderError::Invalid(
            "camera clipping limits are invalid".to_owned(),
        ));
    }
    let projection = match projection {
        Some("perspective") => {
            let fov_y = object
                .get("fov_y_degrees")
                .and_then(Value::as_f64)
                .ok_or_else(|| RenderError::Invalid("camera fov is missing".to_owned()))?
                as f32;
            if !(fov_y > 1.0 && fov_y < 179.0) {
                return Err(RenderError::Invalid(
                    "camera perspective limits are invalid".to_owned(),
                ));
            }
            CameraProjection::Perspective {
                focal: 1.0 / (fov_y.to_radians() * 0.5).tan(),
            }
        }
        Some("orthographic") => {
            let scale = object
                .get("ortho_scale")
                .and_then(Value::as_f64)
                .ok_or_else(|| RenderError::Invalid("camera ortho scale is missing".to_owned()))?
                as f32;
            if !(scale.is_finite() && scale > 0.0 && scale <= 100.0) {
                return Err(RenderError::Invalid(
                    "camera orthographic limits are invalid".to_owned(),
                ));
            }
            CameraProjection::Orthographic { scale }
        }
        _ => {
            return Err(RenderError::Invalid(
                "camera projection is invalid".to_owned(),
            ))
        }
    };
    Ok((position, forward, right, up, projection, near, far))
}

fn emissive_hdr_material(
    root: &Value,
    textures: &[Option<RgbaImage>],
    material_index: usize,
    uv: [f32; 2],
    emissive_override: Option<&EmissiveMaterialOverride>,
) -> [f32; 3] {
    let (_, _, _, mut emissive) = material_parameters(root, material_index);
    let mut strength = material_extension_factor(
        root,
        material_index,
        "KHR_materials_emissive_strength",
        "emissiveStrength",
    )
    .unwrap_or(1.0);
    if let Some(override_value) = emissive_override {
        emissive = override_value.color_linear_rgb;
        strength = override_value.emissive_strength;
    }
    if let Some(texture_index) = material_texture_index(root, material_index, "emissiveTexture") {
        if let Some(Some(texture)) = textures.get(texture_index) {
            let sampled = sample_texture(texture, uv);
            for channel in 0..3 {
                emissive[channel] *= srgb_to_linear(sampled[channel]);
            }
        }
    }
    [
        (emissive[0] * strength).max(0.0),
        (emissive[1] * strength).max(0.0),
        (emissive[2] * strength).max(0.0),
    ]
}

fn build_hdr_emissive_source(
    root: &Value,
    textures: &[Option<RgbaImage>],
    hits: &[Option<RasterHit>],
    sample_width: u32,
    sample_height: u32,
    width: u32,
    height: u32,
    override_by_material: &[Option<&EmissiveMaterialOverride>],
    profile: &HdrBloomProfile,
) -> Vec<[f32; 4]> {
    let mut output = vec![[0.0; 4]; (width * height) as usize];
    for y in 0..height {
        let y0 = y * sample_height / height;
        let y1 = ((y + 1) * sample_height / height)
            .max(y0 + 1)
            .min(sample_height);
        for x in 0..width {
            let x0 = x * sample_width / width;
            let x1 = ((x + 1) * sample_width / width)
                .max(x0 + 1)
                .min(sample_width);
            let mut sum = [0.0_f32; 3];
            let mut count = 0.0_f32;
            for sample_y in y0..y1 {
                for sample_x in x0..x1 {
                    let Some(hit) = hits[(sample_y * sample_width + sample_x) as usize] else {
                        continue;
                    };
                    let emissive = emissive_hdr_material(
                        root,
                        textures,
                        hit.material_index,
                        hit.uv,
                        override_by_material
                            .get(hit.material_index)
                            .copied()
                            .flatten(),
                    );
                    for channel in 0..3 {
                        sum[channel] += emissive[channel];
                    }
                    count += 1.0;
                }
            }
            if count > 0.0 {
                for channel in 0..3 {
                    output[(y * width + x) as usize][channel] =
                        (sum[channel] / count).min(profile.hdr_clamp);
                }
                output[(y * width + x) as usize][3] = 1.0;
            }
        }
    }
    output
}

fn separable_blur_two_passes(
    input: &[[f32; 4]],
    width: u32,
    height: u32,
    radius: u32,
    hdr_clamp: f32,
) -> Vec<[f32; 4]> {
    let radius = radius as i32;
    let width_i32 = width as i32;
    let height_i32 = height as i32;
    let weight = 1.0_f32 / (radius * 2 + 1) as f32;
    let sample = |buffer: &[[f32; 4]], x: i32, y: i32| -> [f32; 4] {
        let x = x.clamp(0, width_i32 - 1) as u32;
        let y = y.clamp(0, height_i32 - 1) as u32;
        buffer[(y * width + x) as usize]
    };
    let mut horizontal = vec![[0.0; 4]; input.len()];
    for y in 0..height_i32 {
        let mut sum = [0.0_f32; 4];
        for offset in -radius..=radius {
            let pixel = sample(input, offset, y);
            for channel in 0..4 {
                sum[channel] += pixel[channel];
            }
        }
        for x in 0..width_i32 {
            let mut value = [0.0_f32; 4];
            for channel in 0..4 {
                value[channel] = (sum[channel] * weight).clamp(0.0, hdr_clamp);
            }
            horizontal[(y as u32 * width + x as u32) as usize] = value;
            let leaving = sample(input, x - radius, y);
            let entering = sample(input, x + radius + 1, y);
            for channel in 0..4 {
                sum[channel] += entering[channel] - leaving[channel];
            }
        }
    }
    let mut vertical = vec![[0.0; 4]; input.len()];
    for x in 0..width_i32 {
        let mut sum = [0.0_f32; 4];
        for offset in -radius..=radius {
            let pixel = sample(&horizontal, x, offset);
            for channel in 0..4 {
                sum[channel] += pixel[channel];
            }
        }
        for y in 0..height_i32 {
            let mut value = [0.0_f32; 4];
            for channel in 0..4 {
                value[channel] = (sum[channel] * weight).clamp(0.0, hdr_clamp);
            }
            vertical[(y as u32 * width + x as u32) as usize] = value;
            let leaving = sample(&horizontal, x, y - radius);
            let entering = sample(&horizontal, x, y + radius + 1);
            for channel in 0..4 {
                sum[channel] += entering[channel] - leaving[channel];
            }
        }
    }
    vertical
}

fn hdr_pixel_to_rgba8(value: [f32; 4], hdr_clamp: f32) -> [u8; 4] {
    [
        (value[0].clamp(0.0, hdr_clamp) / hdr_clamp * 255.0).round() as u8,
        (value[1].clamp(0.0, hdr_clamp) / hdr_clamp * 255.0).round() as u8,
        (value[2].clamp(0.0, hdr_clamp) / hdr_clamp * 255.0).round() as u8,
        255,
    ]
}

fn required_vec3(value: Option<&Value>, label: &str) -> Result<[f32; 3], RenderError> {
    let values = value
        .and_then(Value::as_array)
        .ok_or_else(|| RenderError::Invalid(format!("{label} is missing")))?;
    if values.len() != 3 {
        return Err(RenderError::Invalid(format!(
            "{label} must have three values"
        )));
    }
    let result = [
        values[0]
            .as_f64()
            .ok_or_else(|| RenderError::Invalid(format!("{label} is invalid")))? as f32,
        values[1]
            .as_f64()
            .ok_or_else(|| RenderError::Invalid(format!("{label} is invalid")))? as f32,
        values[2]
            .as_f64()
            .ok_or_else(|| RenderError::Invalid(format!("{label} is invalid")))? as f32,
    ];
    if result.iter().any(|value| !value.is_finite()) {
        return Err(RenderError::Invalid(format!("{label} is non-finite")));
    }
    Ok(result)
}

fn scene_mesh_instances(root: &Value, nodes: &[Value]) -> Result<Vec<(usize, Mat4)>, RenderError> {
    if nodes.is_empty() {
        return Err(RenderError::Invalid("GLB nodes are missing".to_owned()));
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
            .ok_or_else(|| RenderError::Invalid("scene node index is invalid".to_owned()))?
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
) -> Result<(), RenderError> {
    if !visited.insert(index) {
        return Err(RenderError::Invalid(
            "GLB node graph contains a cycle or duplicate instance".to_owned(),
        ));
    }
    let node = nodes
        .get(index)
        .and_then(Value::as_object)
        .ok_or_else(|| RenderError::Invalid("GLB node is invalid".to_owned()))?;
    let transform = mat4_mul(parent, node_transform(node)?);
    if let Some(mesh) = node.get("mesh").and_then(Value::as_u64) {
        instances.push((mesh as usize, transform));
    }
    if let Some(children) = node.get("children").and_then(Value::as_array) {
        for child in children {
            collect_node_instances(
                nodes,
                child
                    .as_u64()
                    .ok_or_else(|| RenderError::Invalid("GLB child index is invalid".to_owned()))?
                    as usize,
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
fn node_transform(node: &Map<String, Value>) -> Result<Mat4, RenderError> {
    if let Some(matrix) = node.get("matrix").and_then(Value::as_array) {
        if matrix.len() != 16 {
            return Err(RenderError::Invalid(
                "GLB node matrix is invalid".to_owned(),
            ));
        }
        let mut result = [[0.0; 4]; 4];
        for (index, value) in matrix.iter().enumerate() {
            result[index % 4][index / 4] = value
                .as_f64()
                .ok_or_else(|| RenderError::Invalid("GLB node matrix is non-numeric".to_owned()))?
                as f32;
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
                    return Err(RenderError::Invalid("node rotation is invalid".to_owned()));
                }
                Ok([
                    values[0].as_f64().ok_or_else(|| {
                        RenderError::Invalid("node rotation is invalid".to_owned())
                    })? as f32,
                    values[1].as_f64().ok_or_else(|| {
                        RenderError::Invalid("node rotation is invalid".to_owned())
                    })? as f32,
                    values[2].as_f64().ok_or_else(|| {
                        RenderError::Invalid("node rotation is invalid".to_owned())
                    })? as f32,
                    values[3].as_f64().ok_or_else(|| {
                        RenderError::Invalid("node rotation is invalid".to_owned())
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
    triangle_index: u32,
    source_map_index: u32,
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
                tangent: [1.0, 0.0, 0.0, 1.0],
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
                tangent: {
                    let tangent = normalize([
                        w0 * vertices[0].tangent[0]
                            + w1 * vertices[1].tangent[0]
                            + w2 * vertices[2].tangent[0],
                        w0 * vertices[0].tangent[1]
                            + w1 * vertices[1].tangent[1]
                            + w2 * vertices[2].tangent[1],
                        w0 * vertices[0].tangent[2]
                            + w1 * vertices[1].tangent[2]
                            + w2 * vertices[2].tangent[2],
                    ]);
                    [tangent[0], tangent[1], tangent[2], vertices[0].tangent[3]]
                },
                uv: [
                    w0 * vertices[0].uv[0] + w1 * vertices[1].uv[0] + w2 * vertices[2].uv[0],
                    w0 * vertices[0].uv[1] + w1 * vertices[1].uv[1] + w2 * vertices[2].uv[1],
                ],
                mesh_index,
                material_index,
                triangle_index,
                source_map_index,
                barycentric: [w0, w1, w2],
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

fn particle_id_color(id: u32) -> [u8; 4] {
    // Lossless 24-bit data encoding; zero remains reserved for transparent
    // background, and this pass is separate from Part/Material ID palettes.
    let encoded = id + 1;
    [
        (encoded & 0xff) as u8,
        ((encoded >> 8) & 0xff) as u8,
        ((encoded >> 16) & 0xff) as u8,
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
    textures: &[Option<RgbaImage>],
    material_index: usize,
    normal: [f32; 3],
    tangent: [f32; 4],
    world: [f32; 3],
    camera: [f32; 3],
    uv: [f32; 2],
    emissive_override: Option<&EmissiveMaterialOverride>,
) -> [u8; 4] {
    let (mut base, mut metallic, mut roughness, mut emissive) =
        material_parameters(root, material_index);
    let mut emissive_strength = material_extension_factor(
        root,
        material_index,
        "KHR_materials_emissive_strength",
        "emissiveStrength",
    )
    .unwrap_or(1.0);
    if let Some(override_value) = emissive_override {
        emissive = override_value.color_linear_rgb;
        emissive_strength = override_value.emissive_strength;
    }
    if let Some(texture_index) = material_base_color_texture_index(root, material_index) {
        if let Some(Some(texture)) = textures.get(texture_index) {
            let sampled = sample_texture(texture, uv);
            for channel in 0..3 {
                base[channel] *= srgb_to_linear(sampled[channel]);
            }
        }
    }
    if let Some(texture_index) =
        material_texture_index(root, material_index, "metallicRoughnessTexture")
    {
        if let Some(Some(texture)) = textures.get(texture_index) {
            let sampled = sample_texture_unit(texture, uv);
            // glTF stores roughness in G and metallic in B.  The bundled
            // roughness-only maps are grayscale, so this remains compatible
            // while allowing a future packed metal/rough texture.
            roughness = (roughness * sampled[1]).clamp(0.04, 1.0);
            metallic = (metallic * sampled[2]).clamp(0.0, 1.0);
        }
    }
    let ao = material_texture_index(root, material_index, "occlusionTexture")
        .and_then(|texture_index| textures.get(texture_index))
        .and_then(|texture| texture.as_ref())
        .map(|texture| sample_texture_unit(texture, uv)[0])
        .unwrap_or(1.0);
    if let Some(texture_index) = material_texture_index(root, material_index, "emissiveTexture") {
        if let Some(Some(texture)) = textures.get(texture_index) {
            let sampled = sample_texture(texture, uv);
            for channel in 0..3 {
                emissive[channel] *= srgb_to_linear(sampled[channel]);
            }
        }
    }
    let mut clearcoat = material_extension_factor(
        root,
        material_index,
        "KHR_materials_clearcoat",
        "clearcoatFactor",
    )
    .unwrap_or(0.0)
    .clamp(0.0, 1.0);
    if let Some(texture_index) = material_extension_texture_index(
        root,
        material_index,
        "KHR_materials_clearcoat",
        "clearcoatTexture",
    ) {
        if let Some(Some(texture)) = textures.get(texture_index) {
            clearcoat *= sample_texture_unit(texture, uv)[0];
        }
    }
    let mut clearcoat_roughness = material_extension_factor(
        root,
        material_index,
        "KHR_materials_clearcoat",
        "clearcoatRoughnessFactor",
    )
    .unwrap_or(0.0)
    .clamp(0.0, 1.0);
    if let Some(texture_index) = material_extension_texture_index(
        root,
        material_index,
        "KHR_materials_clearcoat",
        "clearcoatRoughnessTexture",
    ) {
        if let Some(Some(texture)) = textures.get(texture_index) {
            clearcoat_roughness *= sample_texture_unit(texture, uv)[1];
        }
    }
    let mut n = normalize(normal);
    if let Some(texture_index) = material_texture_index(root, material_index, "normalTexture") {
        if let Some(Some(texture)) = textures.get(texture_index) {
            let sampled = sample_texture_unit(texture, uv);
            let tangent_sign = tangent[3].signum();
            let tangent = normalize([tangent[0], tangent[1], tangent[2]]);
            let bitangent = normalize(scale3(cross3(n, tangent), tangent_sign));
            let mapped = normalize([
                sampled[0] * 2.0 - 1.0,
                sampled[1] * 2.0 - 1.0,
                sampled[2] * 2.0 - 1.0,
            ]);
            n = normalize(add3(
                add3(scale3(tangent, mapped[0]), scale3(bitangent, mapped[1])),
                scale3(n, mapped[2]),
            ));
        }
    }
    let view = normalize(subtract3(camera, world));
    let f0 = [
        0.04 * (1.0 - metallic) + base[0] * metallic,
        0.04 * (1.0 - metallic) + base[1] * metallic,
        0.04 * (1.0 - metallic) + base[2] * metallic,
    ];
    // Three fixed studio lights give the hard-surface renderer readable key,
    // fill and rim separation without accepting an HDRI, shader or URL.
    let lights = [
        (normalize([0.45, 0.80, 0.60]), [1.0, 0.86, 0.72], 1.10),
        (normalize([-0.65, 0.35, 0.70]), [0.32, 0.45, 0.70], 0.38),
        (normalize([-0.35, 0.60, -0.82]), [0.58, 0.68, 1.0], 0.62),
    ];
    let mut color = [0.0; 3];
    for (light, light_color, intensity) in lights {
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
        let fresnel = (1.0 - vdoth).powi(5);
        for i in 0..3 {
            let spec = (d * g1 * g2 * f0[i] * (1.0 - fresnel) + f0[i] * fresnel) * ndotl;
            color[i] +=
                (base[i] * (1.0 - metallic) * ndotl * 0.78 + spec) * intensity * light_color[i];
        }
        if clearcoat > 0.0 {
            let coat_roughness = clearcoat_roughness.clamp(0.04, 1.0);
            let coat_alpha = coat_roughness * coat_roughness;
            let coat_d = (coat_alpha * coat_alpha)
                / (std::f32::consts::PI
                    * ((ndoth * ndoth * (coat_alpha * coat_alpha - 1.0) + 1.0).powi(2)));
            let coat_spec = coat_d * g1 * g2 * (1.0 - fresnel) * ndotl * clearcoat;
            for i in 0..3 {
                color[i] += coat_spec * intensity * light_color[i];
            }
        }
    }
    for i in 0..3 {
        color[i] += base[i] * (0.055 + 0.045 * ao) + emissive[i] * emissive_strength;
    }
    [
        linear_to_srgb(color[0]),
        linear_to_srgb(color[1]),
        linear_to_srgb(color[2]),
        255,
    ]
}

fn embedded_render_textures(
    root: &Value,
    views: &[Value],
    binary: &[u8],
) -> Result<Vec<Option<RgbaImage>>, RenderError> {
    let Some(textures) = root.get("textures").and_then(Value::as_array) else {
        return Ok(Vec::new());
    };
    let images = root
        .get("images")
        .and_then(Value::as_array)
        .ok_or_else(|| RenderError::Invalid("GLB textures require images".to_owned()))?;
    textures
        .iter()
        .map(|texture| {
            let Some(source) = texture.get("source").and_then(Value::as_u64) else {
                return Ok(None);
            };
            let image = images.get(source as usize).ok_or_else(|| {
                RenderError::Invalid("GLB texture image index is invalid".to_owned())
            })?;
            let Some(view_index) = image.get("bufferView").and_then(Value::as_u64) else {
                return Ok(None);
            };
            let bytes = read_buffer_view_bytes(views, binary, view_index as usize)?;
            let decoded = image::load_from_memory(bytes)
                .map_err(|error| {
                    RenderError::Invalid(format!("GLB texture decode failed: {error}"))
                })?
                .to_rgba8();
            if decoded.width() == 0 || decoded.height() == 0 {
                return Err(RenderError::Invalid("GLB texture is empty".to_owned()));
            }
            Ok(Some(decoded))
        })
        .collect()
}

fn read_buffer_view_bytes<'a>(
    views: &[Value],
    binary: &'a [u8],
    index: usize,
) -> Result<&'a [u8], RenderError> {
    let view = views
        .get(index)
        .and_then(Value::as_object)
        .ok_or_else(|| RenderError::Invalid("GLB image bufferView is invalid".to_owned()))?;
    let offset = view.get("byteOffset").and_then(Value::as_u64).unwrap_or(0) as usize;
    let length = view
        .get("byteLength")
        .and_then(Value::as_u64)
        .ok_or_else(|| RenderError::Invalid("GLB image byteLength is missing".to_owned()))?
        as usize;
    let end = offset
        .checked_add(length)
        .ok_or_else(|| RenderError::Invalid("GLB image range overflow".to_owned()))?;
    binary
        .get(offset..end)
        .ok_or_else(|| RenderError::Invalid("GLB image exceeds BIN".to_owned()))
}

fn material_base_color_texture_index(root: &Value, material_index: usize) -> Option<usize> {
    root.get("materials")
        .and_then(Value::as_array)
        .and_then(|materials| materials.get(material_index))
        .and_then(|material| material.get("pbrMetallicRoughness"))
        .and_then(|pbr| pbr.get("baseColorTexture"))
        .and_then(|texture| texture.get("index"))
        .and_then(Value::as_u64)
        .map(|index| index as usize)
}

fn material_texture_index(root: &Value, material_index: usize, slot: &str) -> Option<usize> {
    root.get("materials")
        .and_then(Value::as_array)
        .and_then(|materials| materials.get(material_index))
        .and_then(|material| {
            material
                .get("pbrMetallicRoughness")
                .and_then(|pbr| pbr.get(slot))
                .or_else(|| material.get(slot))
        })
        .and_then(|texture| texture.get("index"))
        .and_then(Value::as_u64)
        .map(|index| index as usize)
}

fn material_extension_factor(
    root: &Value,
    material_index: usize,
    extension: &str,
    field: &str,
) -> Option<f32> {
    root.get("materials")
        .and_then(Value::as_array)
        .and_then(|materials| materials.get(material_index))
        .and_then(|material| material.get("extensions"))
        .and_then(|extensions| extensions.get(extension))
        .and_then(|value| value.get(field))
        .and_then(Value::as_f64)
        .map(|value| value as f32)
}

fn material_extension_texture_index(
    root: &Value,
    material_index: usize,
    extension: &str,
    field: &str,
) -> Option<usize> {
    root.get("materials")
        .and_then(Value::as_array)
        .and_then(|materials| materials.get(material_index))
        .and_then(|material| material.get("extensions"))
        .and_then(|extensions| extensions.get(extension))
        .and_then(|value| value.get(field))
        .and_then(|texture| texture.get("index"))
        .and_then(Value::as_u64)
        .map(|index| index as usize)
}

fn sample_texture(texture: &RgbaImage, uv: [f32; 2]) -> [u8; 3] {
    let u = uv[0].rem_euclid(1.0);
    let v = 1.0 - uv[1].rem_euclid(1.0);
    let x = (u * texture.width().saturating_sub(1) as f32).round() as u32;
    let y = (v * texture.height().saturating_sub(1) as f32).round() as u32;
    let pixel = texture.get_pixel(x, y);
    [pixel[0], pixel[1], pixel[2]]
}

fn sample_texture_unit(texture: &RgbaImage, uv: [f32; 2]) -> [f32; 4] {
    let pixel = sample_texture_pixel(texture, uv);
    [
        pixel[0] as f32 / 255.0,
        pixel[1] as f32 / 255.0,
        pixel[2] as f32 / 255.0,
        pixel[3] as f32 / 255.0,
    ]
}

fn sample_texture_pixel(texture: &RgbaImage, uv: [f32; 2]) -> [u8; 4] {
    let u = uv[0].rem_euclid(1.0);
    let v = 1.0 - uv[1].rem_euclid(1.0);
    let x = (u * texture.width().saturating_sub(1) as f32).round() as u32;
    let y = (v * texture.height().saturating_sub(1) as f32).round() as u32;
    let pixel = texture.get_pixel(x, y);
    [pixel[0], pixel[1], pixel[2], pixel[3]]
}

fn srgb_to_linear(value: u8) -> f32 {
    let value = value as f32 / 255.0;
    if value <= 0.04045 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
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

fn parse_glb(glb: &[u8]) -> Result<(Value, Vec<u8>), RenderError> {
    if glb.len() < 20
        || &glb[..4] != b"glTF"
        || u32::from_le_bytes(glb[4..8].try_into().unwrap()) != 2
    {
        return Err(RenderError::Invalid("GLB header is invalid".to_owned()));
    }
    let total = u32::from_le_bytes(glb[8..12].try_into().unwrap()) as usize;
    if total != glb.len() {
        return Err(RenderError::Invalid("GLB length is invalid".to_owned()));
    }
    let json_len = u32::from_le_bytes(glb[12..16].try_into().unwrap()) as usize;
    if &glb[16..20] != b"JSON" || 20 + json_len + 8 > glb.len() {
        return Err(RenderError::Invalid("GLB JSON chunk is invalid".to_owned()));
    }
    let root = serde_json::from_slice(&glb[20..20 + json_len])
        .map_err(|error| RenderError::Invalid(error.to_string()))?;
    let binary_offset = 20 + json_len;
    let binary_len =
        u32::from_le_bytes(glb[binary_offset..binary_offset + 4].try_into().unwrap()) as usize;
    if &glb[binary_offset + 4..binary_offset + 8] != b"BIN\0"
        || binary_offset + 8 + binary_len != glb.len()
    {
        return Err(RenderError::Invalid("GLB BIN chunk is invalid".to_owned()));
    }
    Ok((root, glb[binary_offset + 8..].to_vec()))
}

fn accessor_view<'a>(
    accessors: &'a [Value],
    views: &'a [Value],
    index: usize,
) -> Result<(&'a Value, &'a Value), RenderError> {
    let accessor = accessors
        .get(index)
        .ok_or_else(|| RenderError::Invalid("GLB accessor index is invalid".to_owned()))?;
    let view_index = accessor
        .get("bufferView")
        .and_then(Value::as_u64)
        .ok_or_else(|| RenderError::Invalid("GLB accessor bufferView is missing".to_owned()))?
        as usize;
    let view = views
        .get(view_index)
        .ok_or_else(|| RenderError::Invalid("GLB bufferView index is invalid".to_owned()))?;
    Ok((accessor, view))
}

fn read_vec3_accessor(
    accessors: &[Value],
    views: &[Value],
    binary: &[u8],
    index: usize,
) -> Result<Vec<[f32; 3]>, RenderError> {
    let (accessor, view) = accessor_view(accessors, views, index)?;
    if accessor.get("componentType").and_then(Value::as_u64) != Some(5126)
        || accessor.get("type").and_then(Value::as_str) != Some("VEC3")
    {
        return Err(RenderError::Invalid(
            "GLB VEC3 accessor is not float".to_owned(),
        ));
    }
    let count = accessor
        .get("count")
        .and_then(Value::as_u64)
        .ok_or_else(|| RenderError::Invalid("GLB accessor count is missing".to_owned()))?
        as usize;
    let offset = view.get("byteOffset").and_then(Value::as_u64).unwrap_or(0) as usize
        + accessor
            .get("byteOffset")
            .and_then(Value::as_u64)
            .unwrap_or(0) as usize;
    if offset + count.saturating_mul(12) > binary.len() {
        return Err(RenderError::Invalid(
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

fn read_vec4_accessor(
    accessors: &[Value],
    views: &[Value],
    binary: &[u8],
    index: usize,
) -> Result<Vec<[f32; 4]>, RenderError> {
    let (accessor, view) = accessor_view(accessors, views, index)?;
    if accessor.get("componentType").and_then(Value::as_u64) != Some(5126)
        || accessor.get("type").and_then(Value::as_str) != Some("VEC4")
    {
        return Err(RenderError::Invalid(
            "GLB VEC4 accessor is not float".to_owned(),
        ));
    }
    let count = accessor
        .get("count")
        .and_then(Value::as_u64)
        .ok_or_else(|| RenderError::Invalid("GLB accessor count is missing".to_owned()))?
        as usize;
    let offset = view.get("byteOffset").and_then(Value::as_u64).unwrap_or(0) as usize
        + accessor
            .get("byteOffset")
            .and_then(Value::as_u64)
            .unwrap_or(0) as usize;
    if offset + count.saturating_mul(16) > binary.len() {
        return Err(RenderError::Invalid(
            "GLB VEC4 accessor exceeds BIN".to_owned(),
        ));
    }
    Ok(binary[offset..offset + count * 16]
        .chunks_exact(16)
        .map(|chunk| {
            [
                f32::from_le_bytes(chunk[0..4].try_into().unwrap()),
                f32::from_le_bytes(chunk[4..8].try_into().unwrap()),
                f32::from_le_bytes(chunk[8..12].try_into().unwrap()),
                f32::from_le_bytes(chunk[12..16].try_into().unwrap()),
            ]
        })
        .collect())
}

fn read_indices_accessor(
    accessors: &[Value],
    views: &[Value],
    binary: &[u8],
    index: usize,
) -> Result<Vec<u32>, RenderError> {
    let (accessor, view) = accessor_view(accessors, views, index)?;
    if accessor.get("componentType").and_then(Value::as_u64) != Some(5125)
        || accessor.get("type").and_then(Value::as_str) != Some("SCALAR")
    {
        return Err(RenderError::Invalid(
            "GLB index accessor is not uint32".to_owned(),
        ));
    }
    let count = accessor
        .get("count")
        .and_then(Value::as_u64)
        .ok_or_else(|| RenderError::Invalid("GLB index count is missing".to_owned()))?
        as usize;
    let offset = view.get("byteOffset").and_then(Value::as_u64).unwrap_or(0) as usize
        + accessor
            .get("byteOffset")
            .and_then(Value::as_u64)
            .unwrap_or(0) as usize;
    if offset + count.saturating_mul(4) > binary.len() {
        return Err(RenderError::Invalid(
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
) -> Result<Vec<[f32; 2]>, RenderError> {
    let (accessor, view) = accessor_view(accessors, views, index)?;
    if accessor.get("componentType").and_then(Value::as_u64) != Some(5126)
        || accessor.get("type").and_then(Value::as_str) != Some("VEC2")
    {
        return Err(RenderError::Invalid(
            "GLB VEC2 accessor is not float".to_owned(),
        ));
    }
    let count = accessor
        .get("count")
        .and_then(Value::as_u64)
        .ok_or_else(|| RenderError::Invalid("GLB VEC2 accessor count is missing".to_owned()))?
        as usize;
    let offset = view.get("byteOffset").and_then(Value::as_u64).unwrap_or(0) as usize
        + accessor
            .get("byteOffset")
            .and_then(Value::as_u64)
            .unwrap_or(0) as usize;
    if offset + count.saturating_mul(8) > binary.len() {
        return Err(RenderError::Invalid(
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn push_f32_bytes(output: &mut Vec<u8>, value: f32) {
        output.extend_from_slice(&value.to_le_bytes());
    }

    fn push_u32_bytes(output: &mut Vec<u8>, value: u32) {
        output.extend_from_slice(&value.to_le_bytes());
    }

    fn hero_test_glb() -> Vec<u8> {
        let mut binary = Vec::new();
        for vertex in [[-1.0_f32, -1.0, 0.0], [1.0, -1.0, 0.0], [0.0, 1.0, 0.0]] {
            for value in vertex {
                push_f32_bytes(&mut binary, value);
            }
        }
        for _ in 0..3 {
            for value in [0.0_f32, 0.0, 1.0] {
                push_f32_bytes(&mut binary, value);
            }
        }
        for uv in [[0.0_f32, 0.0], [1.0, 0.0], [0.5, 1.0]] {
            for value in uv {
                push_f32_bytes(&mut binary, value);
            }
        }
        for value in [0_u32, 1, 2] {
            push_u32_bytes(&mut binary, value);
        }
        let root = json!({
            "asset":{"version":"2.0"},
            "scene":0,
            "scenes":[{"nodes":[0]}],
            "nodes":[{"mesh":0,"extras":{"part_id":"hero-part","material_zone_id":"hero-zone"}}],
            "materials":[{"name":"hero-test-material","pbrMetallicRoughness":{"baseColorFactor":[0.4,0.5,0.7,1.0]}}],
            "meshes":[{"extras":{"part_id":"hero-part","material_zone_id":"hero-zone"},"primitives":[{
                "attributes":{"POSITION":0,"NORMAL":1,"TEXCOORD_0":2},
                "indices":3,
                "material":0,
                "extras":{"part_id":"hero-part","source_node_id":"hero-node","lineage_source_node_ids":["hero-node"],"material_zone_id":"hero-zone"}
            }]}],
            "accessors":[
                {"bufferView":0,"componentType":5126,"count":3,"type":"VEC3"},
                {"bufferView":1,"componentType":5126,"count":3,"type":"VEC3"},
                {"bufferView":2,"componentType":5126,"count":3,"type":"VEC2"},
                {"bufferView":3,"componentType":5125,"count":3,"type":"SCALAR"}
            ],
            "bufferViews":[
                {"buffer":0,"byteOffset":0,"byteLength":36},
                {"buffer":0,"byteOffset":36,"byteLength":36},
                {"buffer":0,"byteOffset":72,"byteLength":24},
                {"buffer":0,"byteOffset":96,"byteLength":12}
            ],
            "buffers":[{"byteLength":108}]
        });
        let mut json_bytes = serde_json::to_vec(&root).expect("test GLB JSON serializes");
        while json_bytes.len() % 4 != 0 {
            json_bytes.push(b' ');
        }
        let total_length = 12 + 8 + json_bytes.len() + 8 + binary.len();
        let mut glb = Vec::with_capacity(total_length);
        glb.extend_from_slice(b"glTF");
        glb.extend_from_slice(&2_u32.to_le_bytes());
        glb.extend_from_slice(&(total_length as u32).to_le_bytes());
        glb.extend_from_slice(&(json_bytes.len() as u32).to_le_bytes());
        glb.extend_from_slice(b"JSON");
        glb.extend_from_slice(&json_bytes);
        glb.extend_from_slice(&(binary.len() as u32).to_le_bytes());
        glb.extend_from_slice(b"BIN\0");
        glb.extend_from_slice(&binary);
        glb
    }

    fn hero_test_camera() -> Value {
        json!({
            "schema_version":"CameraCalibration@1",
            "projection":"perspective",
            "transform":{
                "position_m":[0.0,0.25,10.0],
                "target_m":[0.0,0.25,0.0],
                "up":[0.0,1.0,0.0]
            },
            "fov_y_degrees":40.0,
            "near_m":0.05,
            "far_m":100.0,
            "resolution":{"width":512,"height":512},
            "coordinate_system":"right-handed-y-up-meter"
        })
    }

    #[test]
    fn static_hero_beauty_is_bounded_deterministic_and_keeps_formal_aovs() {
        let glb = hero_test_glb();
        let first = render_static_hero_beauty_glb(&glb).expect("static Hero beauty render");
        let second = render_static_hero_beauty_glb(&glb).expect("deterministic Hero replay");
        assert_eq!(first.pass, "beauty");
        assert_eq!((first.width, first.height), (2048, 2048));
        assert_eq!(first.png, second.png);
        let image = image::load_from_memory(&first.png)
            .expect("Hero PNG decodes")
            .to_rgba8();
        assert_eq!((image.width(), image.height()), (2048, 2048));

        let formal = render_perspective_glb(&glb, &hero_test_camera())
            .expect("formal RenderSet@2 camera remains available");
        assert_eq!(formal.len(), 9);
        assert!(formal
            .iter()
            .all(|pass| { pass.width == 512 && pass.height == 512 }));
    }

    #[test]
    fn fixed_renderer_exposes_deterministic_pixel_triangle_source_lineage() {
        let glb = hero_test_glb();
        let camera = hero_test_camera();
        let map = render_perspective_glb_raster_hit_source_map(&glb, &camera)
            .expect("fixed renderer source map");
        let replay = render_perspective_glb_raster_hit_source_map(&glb, &camera)
            .expect("fixed renderer source map replay");
        assert_eq!(map, replay);
        assert_eq!((map.width, map.height), (512, 512));
        assert_eq!((map.raster_width, map.raster_height), (1024, 1024));
        assert_eq!(map.pixels.len(), 512 * 512);
        assert_eq!(map.sources.len(), 1);
        let source = &map.sources[0];
        assert_eq!(source.triangle_index, 0);
        assert_eq!(source.semantic_part_id, "hero-part");
        assert_eq!(source.source_node_id, "hero-node");
        assert_eq!(source.lineage_source_node_ids, ["hero-node"]);
        assert_eq!(source.material_zone_id, "hero-zone");
        let visible = map.pixels.iter().flatten().collect::<Vec<_>>();
        assert!(!visible.is_empty());
        assert!(visible.iter().all(|hit| {
            hit.triangle_index == 0
                && hit.source_map_index == 0
                && hit.barycentric_milli.iter().all(|value| *value <= 1000)
                && hit.depth_micros <= 1_000_000
        }));
        let part_id = render_perspective_glb(&glb, &camera)
            .expect("formal AOV render")
            .into_iter()
            .find(|pass| pass.pass == "part-id")
            .expect("part-id pass");
        let part_id = image::load_from_memory(&part_id.png)
            .expect("part-id PNG decodes")
            .to_rgba8();
        let expected_part_color = part_color(0);
        for (index, hit) in map.pixels.iter().enumerate() {
            let x = (index % 512) as u32;
            let y = (index / 512) as u32;
            let observed = part_id.get_pixel(x, y).0;
            assert_eq!(hit.is_some(), observed == expected_part_color);
        }
        assert_eq!(map.encode_triangle_ids_le().len(), 512 * 512 * 4);
    }

    #[test]
    fn raster_source_map_fails_closed_when_primitive_lineage_is_missing() {
        let glb = hero_test_glb();
        let (mut root, binary) = parse_glb(&glb).expect("test GLB parses");
        root["meshes"][0]["primitives"][0]
            .as_object_mut()
            .expect("primitive object")
            .remove("extras");
        let mut json_bytes = serde_json::to_vec(&root).expect("test GLB JSON serializes");
        while json_bytes.len() % 4 != 0 {
            json_bytes.push(b' ');
        }
        let total_length = 12 + 8 + json_bytes.len() + 8 + binary.len();
        let mut modified = Vec::with_capacity(total_length);
        modified.extend_from_slice(b"glTF");
        modified.extend_from_slice(&2_u32.to_le_bytes());
        modified.extend_from_slice(&(total_length as u32).to_le_bytes());
        modified.extend_from_slice(&(json_bytes.len() as u32).to_le_bytes());
        modified.extend_from_slice(b"JSON");
        modified.extend_from_slice(&json_bytes);
        modified.extend_from_slice(&(binary.len() as u32).to_le_bytes());
        modified.extend_from_slice(b"BIN\0");
        modified.extend_from_slice(&binary);
        let error = render_perspective_glb_raster_hit_source_map(&modified, &hero_test_camera())
            .expect_err("lineage is required for attribution");
        assert!(error.to_string().contains("lineage is missing"));
    }

    #[test]
    fn fixed_id_palettes_fail_closed_outside_u8_domain() {
        let mesh = json!({"primitives":[{}]});
        let mut root = json!({
            "meshes": vec![mesh.clone(); ID_PALETTE_CAPACITY],
            "materials": vec![json!({}); ID_PALETTE_CAPACITY]
        });
        validate_id_palette_domain(&root).expect("256 entries fit the fixed palettes");

        root["meshes"] = Value::Array(vec![mesh.clone(); ID_PALETTE_CAPACITY + 1]);
        assert!(validate_id_palette_domain(&root).is_err());

        root["meshes"] = Value::Array(vec![mesh]);
        root["materials"] = Value::Array(vec![json!({}); ID_PALETTE_CAPACITY + 1]);
        assert!(validate_id_palette_domain(&root).is_err());

        root["materials"] = Value::Array(Vec::new());
        root["meshes"] = json!([{"primitives":[{"material":256}]}]);
        assert!(validate_id_palette_domain(&root).is_err());
    }

    #[test]
    fn pbr_renderer_resolves_embedded_texture_slots_and_extensions() {
        let root = json!({
            "materials":[{
                "pbrMetallicRoughness":{
                    "baseColorTexture":{"index":0},
                    "metallicRoughnessTexture":{"index":1}
                },
                "normalTexture":{"index":2},
                "occlusionTexture":{"index":3},
                "emissiveTexture":{"index":4},
                "extensions":{
                    "KHR_materials_clearcoat":{"clearcoatFactor":0.7,"clearcoatTexture":{"index":5},"clearcoatRoughnessFactor":0.4,"clearcoatRoughnessTexture":{"index":6}},
                    "KHR_materials_emissive_strength":{"emissiveStrength":3.0}
                }
            }]
        });
        assert_eq!(material_base_color_texture_index(&root, 0), Some(0));
        assert_eq!(
            material_texture_index(&root, 0, "metallicRoughnessTexture"),
            Some(1)
        );
        assert_eq!(material_texture_index(&root, 0, "normalTexture"), Some(2));
        assert_eq!(
            material_texture_index(&root, 0, "occlusionTexture"),
            Some(3)
        );
        assert_eq!(material_texture_index(&root, 0, "emissiveTexture"), Some(4));
        assert_eq!(
            material_extension_factor(&root, 0, "KHR_materials_clearcoat", "clearcoatFactor"),
            Some(0.7)
        );
        assert_eq!(
            material_extension_texture_index(
                &root,
                0,
                "KHR_materials_clearcoat",
                "clearcoatTexture"
            ),
            Some(5)
        );
        assert_eq!(
            material_extension_texture_index(
                &root,
                0,
                "KHR_materials_clearcoat",
                "clearcoatRoughnessTexture"
            ),
            Some(6)
        );
        assert_eq!(
            material_extension_factor(
                &root,
                0,
                "KHR_materials_emissive_strength",
                "emissiveStrength"
            ),
            Some(3.0)
        );
        let texture = RgbaImage::from_pixel(2, 2, Rgba([128, 64, 255, 255]));
        assert_eq!(
            sample_texture_unit(&texture, [0.25, 0.75]),
            [128.0 / 255.0, 64.0 / 255.0, 1.0, 1.0]
        );
    }

    #[test]
    fn texture_backed_clearcoat_physically_modulates_fixed_shading() {
        let root = json!({
            "materials":[{
                "pbrMetallicRoughness":{"baseColorFactor":[0.3,0.32,0.35,1.0],"metallicFactor":0.1,"roughnessFactor":0.35},
                "emissiveFactor":[0.0,0.0,0.0],
                "extensions":{"KHR_materials_clearcoat":{"clearcoatFactor":1.0,"clearcoatTexture":{"index":0},"clearcoatRoughnessFactor":1.0,"clearcoatRoughnessTexture":{"index":1}}}
            }]
        });
        let roughness = RgbaImage::from_pixel(1, 1, Rgba([31, 31, 31, 255]));
        let no_coat = vec![
            Some(RgbaImage::from_pixel(1, 1, Rgba([0, 0, 0, 255]))),
            Some(roughness.clone()),
        ];
        let full_coat = vec![
            Some(RgbaImage::from_pixel(1, 1, Rgba([255, 255, 255, 255]))),
            Some(roughness),
        ];
        let parameters = (
            0usize,
            [0.0, 1.0, 0.0],
            [1.0, 0.0, 0.0, 1.0],
            [0.0, 0.0, 0.0],
            [0.0, 3.0, 3.0],
            [0.5, 0.5],
        );
        let without = shade_material(
            &root,
            &no_coat,
            parameters.0,
            parameters.1,
            parameters.2,
            parameters.3,
            parameters.4,
            parameters.5,
            None,
        );
        let with = shade_material(
            &root,
            &full_coat,
            parameters.0,
            parameters.1,
            parameters.2,
            parameters.3,
            parameters.4,
            parameters.5,
            None,
        );
        assert_ne!(
            without, with,
            "clearcoat texture must affect beauty shading"
        );
    }

    #[test]
    fn emissive_overrides_require_exact_zone_and_material_identity() {
        let root = json!({
            "materials":[
                {"name":"zone-core-emissive","extras":{"forgecad":{"material_id":"energy-cyan-emissive"}}},
                {"name":"zone-body","extras":{"forgecad":{"material_id":"anodized-dark-alloy"}}}
            ]
        });
        let valid = EmissiveMaterialOverride {
            material_zone_id: "zone-core-emissive".to_owned(),
            material_id: "energy-cyan-emissive".to_owned(),
            color_linear_rgb: [0.0, 0.82, 1.0],
            emissive_strength: 8.0,
        };
        let applied = resolve_emissive_material_overrides(&root, &[valid.clone()])
            .expect("exact identity resolves");
        assert_eq!(applied[0].glb_material_index, 0);

        let mut wrong_material = valid.clone();
        wrong_material.material_id = "anodized-dark-alloy".to_owned();
        assert!(resolve_emissive_material_overrides(&root, &[wrong_material]).is_err());

        let mut excessive = valid;
        excessive.emissive_strength = 16.01;
        assert!(resolve_emissive_material_overrides(&root, &[excessive]).is_err());
    }

    #[test]
    fn camera_parser_accepts_bounded_orthographic_v2() {
        let camera = json!({
            "schema_version":"CameraCalibration@2",
            "camera_hash":"a".repeat(64),
            "projection":"orthographic",
            "transform":{
                "position_m":[0.0,0.0,20.0],
                "target_m":[0.0,0.0,0.0],
                "up":[0.0,1.0,0.0]
            },
            "fov_y_degrees":null,
            "ortho_scale":2.4,
            "near_m":0.05,
            "far_m":100.0,
            "resolution":{"width":512,"height":512},
            "coordinate_system":"right-handed-y-up-meter",
            "renderer_revision":"forgecad-renderer-2",
            "canonical_sha256":"b".repeat(64)
        });
        let (_, _, _, _, projection, near, far) =
            parse_camera(&camera).expect("orthographic camera");
        assert!(
            matches!(projection, CameraProjection::Orthographic { scale } if (scale - 2.4).abs() < f32::EPSILON)
        );
        assert_eq!(near, 0.05);
        assert_eq!(far, 100.0);
    }

    #[test]
    fn camera_parser_rejects_orthographic_v1() {
        let camera = json!({
            "schema_version":"CameraCalibration@1",
            "camera_hash":"a".repeat(64),
            "projection":"orthographic",
            "transform":{
                "position_m":[0.0,0.0,20.0],
                "target_m":[0.0,0.0,0.0],
                "up":[0.0,1.0,0.0]
            },
            "fov_y_degrees":null,
            "ortho_scale":2.4,
            "near_m":0.05,
            "far_m":100.0,
            "resolution":{"width":512,"height":512},
            "coordinate_system":"right-handed-y-up-meter",
            "renderer_revision":"forgecad-renderer-2",
            "canonical_sha256":"b".repeat(64)
        });
        assert!(parse_camera(&camera).is_err());
    }

    #[test]
    fn typed_particles_recompute_depth_and_use_stable_id_tie_break() {
        let camera = json!({
            "schema_version":"CameraCalibration@1",
            "camera_hash":"a".repeat(64),
            "projection":"perspective",
            "transform":{
                "position_m":[0.0,0.0,5.0],
                "target_m":[0.0,0.0,0.0],
                "up":[0.0,1.0,0.0]
            },
            "fov_y_degrees":45.0,
            "near_m":0.1,
            "far_m":10.0,
            "resolution":{"width":512,"height":512},
            "coordinate_system":"right-handed-y-up-meter",
            "renderer_revision":"forgecad-renderer-2",
            "canonical_sha256":"b".repeat(64)
        });
        let expected_depth = (5.0_f32 - 0.1) / (10.0 - 0.1);
        let particle = |id| TypedParticle {
            emitter_id: "muzzle-burst".to_owned(),
            id,
            position: [0.0, 0.0, 0.0],
            radius_px: 4.0,
            color_linear_rgb: [0.0, 0.8, 1.0],
            alpha: 0.8,
            lifetime_ticks: 120,
            depth: expected_depth,
        };
        let particles = vec![particle(2), particle(1)];
        let first = render_typed_particles(&camera, &particles).expect("typed particles render");
        let second = render_typed_particles(&camera, &particles).expect("deterministic replay");
        assert!(first
            .iter()
            .zip(&second)
            .all(|(left, right)| left.pass == right.pass && left.png == right.png));
        let id_pass = first
            .iter()
            .find(|pass| pass.pass == "particle-id")
            .expect("particle ID pass");
        let id_image = image::load_from_memory(&id_pass.png)
            .expect("particle ID PNG")
            .to_rgba8();
        assert_eq!(id_image.get_pixel(256, 256).0, [2, 0, 0, 255]);

        let mut invalid = particle(3);
        invalid.depth = 0.25;
        assert!(render_typed_particles(&camera, &[invalid]).is_err());

        assert!(render_typed_particles(&camera, &[particle(0)]).is_err());
    }

    #[test]
    fn typed_trails_are_deterministic_independent_and_reject_reserved_id() {
        let camera = json!({
            "schema_version":"CameraCalibration@1",
            "camera_hash":"a".repeat(64),
            "projection":"perspective",
            "transform":{
                "position_m":[0.0,0.0,5.0],
                "target_m":[0.0,0.0,0.0],
                "up":[0.0,1.0,0.0]
            },
            "fov_y_degrees":45.0,
            "near_m":0.1,
            "far_m":10.0,
            "resolution":{"width":512,"height":512},
            "coordinate_system":"right-handed-y-up-meter",
            "renderer_revision":"forgecad-renderer-2",
            "canonical_sha256":"b".repeat(64)
        });
        let trail = TypedTrail {
            emitter_id: "muzzle-trail".to_owned(),
            id: 30000,
            points: vec![[0.0, 0.0, 0.0], [0.1, 0.0, 0.0], [0.2, 0.01, 0.0]],
            radius_px: 3.0,
            color_linear_rgb: [0.0, 0.8, 1.0],
            alpha: 0.75,
            lifetime_ticks: 180,
        };
        let first = render_typed_trails(&camera, std::slice::from_ref(&trail))
            .expect("typed trails render");
        let second = render_typed_trails(&camera, std::slice::from_ref(&trail))
            .expect("deterministic trail replay");
        assert_eq!(
            first
                .iter()
                .map(|pass| pass.pass.as_str())
                .collect::<Vec<_>>(),
            vec!["trail-color", "trail-id", "trail-depth"]
        );
        assert!(first
            .iter()
            .zip(&second)
            .all(|(left, right)| left.png == right.png));
        assert!(first.iter().all(|pass| {
            image::load_from_memory(&pass.png)
                .expect("typed trail PNG")
                .to_rgba8()
                .pixels()
                .any(|pixel| pixel.0[3] != 0)
        }));
        let mut invalid = trail;
        invalid.id = 0;
        assert!(render_typed_trails(&camera, std::slice::from_ref(&invalid)).is_err());

        let mut upper_bound = invalid.clone();
        upper_bound.id = 65_535;
        assert!(render_typed_trails(&camera, &[upper_bound.clone()]).is_ok());
        upper_bound.id = 65_536;
        assert!(render_typed_trails(&camera, &[upper_bound]).is_err());

        let mut one_point = invalid.clone();
        one_point.id = 1;
        one_point.points = vec![[0.0, 0.0, 0.0]];
        assert!(render_typed_trails(&camera, &[one_point]).is_err());
        let mut too_many_points = invalid.clone();
        too_many_points.id = 1;
        too_many_points.points = (0..33)
            .map(|index| [index as f32 / 100.0, 0.0, 0.0])
            .collect();
        assert!(render_typed_trails(&camera, &[too_many_points]).is_err());

        let too_many_trails = (0..17)
            .map(|index| TypedTrail {
                emitter_id: if index % 2 == 0 {
                    "muzzle-trail".to_owned()
                } else {
                    "energy-core-trail".to_owned()
                },
                id: index + 1,
                points: vec![[0.0, 0.0, 0.0], [0.1, 0.0, 0.0]],
                radius_px: 1.0,
                color_linear_rgb: [0.0, 0.8, 1.0],
                alpha: 0.5,
                lifetime_ticks: 1,
            })
            .collect::<Vec<_>>();
        assert!(render_typed_trails(&camera, &too_many_trails).is_err());

        let too_many_segments = (0..5)
            .map(|index| TypedTrail {
                emitter_id: if index % 2 == 0 {
                    "muzzle-trail".to_owned()
                } else {
                    "energy-core-trail".to_owned()
                },
                id: index + 1,
                points: (0..32)
                    .map(|point| [point as f32 / 100.0, index as f32 / 100.0, 0.0])
                    .collect(),
                radius_px: 1.0,
                color_linear_rgb: [0.0, 0.8, 1.0],
                alpha: 0.5,
                lifetime_ticks: 1,
            })
            .collect::<Vec<_>>();
        assert!(render_typed_trails(&camera, &too_many_segments).is_err());
    }

    #[test]
    fn typed_trail_bloom_appends_fixed_passes_without_changing_trail_aovs() {
        let camera = json!({
            "schema_version":"CameraCalibration@1",
            "camera_hash":"a".repeat(64),
            "projection":"perspective",
            "transform":{
                "position_m":[0.0,0.0,5.0],
                "target_m":[0.0,0.0,0.0],
                "up":[0.0,1.0,0.0]
            },
            "fov_y_degrees":45.0,
            "near_m":0.1,
            "far_m":10.0,
            "resolution":{"width":512,"height":512},
            "coordinate_system":"right-handed-y-up-meter",
            "renderer_revision":"forgecad-renderer-2",
            "canonical_sha256":"b".repeat(64)
        });
        let trail = TypedTrail {
            emitter_id: "muzzle-trail".to_owned(),
            id: 30_000,
            points: vec![[0.0, 0.0, 0.0], [0.15, 0.0, 0.0]],
            radius_px: 3.0,
            color_linear_rgb: [0.0, 0.82, 1.0],
            alpha: 0.75,
            lifetime_ticks: 180,
        };
        let base =
            render_typed_trails(&camera, std::slice::from_ref(&trail)).expect("typed trail base");
        let bloom = render_typed_trails_bloom_internal(
            &camera,
            std::slice::from_ref(&trail),
            None,
            TypedTrailBloomProfile::FIXED,
        )
        .expect("typed trail Bloom");
        assert_eq!(bloom.len(), 5);
        assert_eq!(
            bloom
                .iter()
                .map(|pass| pass.pass.as_str())
                .collect::<Vec<_>>(),
            vec![
                "trail-color",
                "trail-id",
                "trail-depth",
                "trail-emissive-source",
                "trail-bloom-contribution"
            ]
        );
        assert_eq!(
            base.iter().map(|pass| &pass.png).collect::<Vec<_>>(),
            bloom[..3].iter().map(|pass| &pass.png).collect::<Vec<_>>()
        );
        for pass in bloom.iter().skip(3) {
            assert!(image::load_from_memory(&pass.png)
                .expect("trail Bloom PNG")
                .to_rgba8()
                .pixels()
                .any(|pixel| pixel.0[0] != 0 || pixel.0[1] != 0 || pixel.0[2] != 0));
        }
        let mut invalid = TypedTrailBloomProfile::FIXED;
        invalid.source_gain = 7.0;
        assert!(render_typed_trails_bloom_internal(
            &camera,
            std::slice::from_ref(&trail),
            None,
            invalid
        )
        .is_err());
    }
}
