//! Runtime-owned typed adapter for the isolated Render Worker.
//!
//! Geometry Worker compilation ends at a persisted-model GLB.  This module
//! owns the Runtime-side Render Worker protocol projection after that point:
//! it accepts only bounded GLB bytes and typed cameras, validates the fixed
//! response shape, and returns transient or nine-AOV render passes.  It does
//! not write Runtime state and it never accepts a GeometryProgram.

use super::geometry_worker::{self, GeometryWorkerError};
use image::ImageDecoder;
use serde_json::{json, Value};
use std::io::Cursor;

const RENDER_WORKER_BINARY: &str = "forgecad-render-worker";

/// Launch the fixed Render Worker sibling through the generic Runtime
/// transport seam. This module owns the Render Worker identity; Geometry
/// Worker owns neither this binary nor this protocol.
fn execute_render_worker(operation: &str, payload: Value) -> Result<Value, GeometryWorkerError> {
    geometry_worker::execute_sibling_worker(RENDER_WORKER_BINARY, operation, payload)
}

fn execute_render_worker_with_metadata(
    operation: &str,
    payload: Value,
) -> Result<geometry_worker::SiblingWorkerResult, GeometryWorkerError> {
    geometry_worker::execute_sibling_worker_with_metadata(RENDER_WORKER_BINARY, operation, payload)
}

#[derive(Debug, Clone)]
pub(crate) struct RenderWorkerRender {
    pub passes: Vec<RenderPass>,
    pub build_cohort_sha256: Option<String>,
    pub render_profile: Value,
}

#[derive(Debug, Clone)]
pub(crate) struct AppliedEmissiveOverride {
    pub material_zone_id: String,
    pub material_id: String,
    pub glb_material_index: usize,
}

#[derive(Debug, Clone)]
pub(crate) struct EmissiveMaterialOverride {
    pub material_zone_id: String,
    pub material_id: String,
    pub color_linear_rgb: [f32; 3],
    pub emissive_strength: f32,
}

#[derive(Debug, Clone)]
pub(crate) struct RenderWorkerVfxFrame {
    pub passes: Vec<RenderPass>,
    pub applied_emissive_overrides: Vec<AppliedEmissiveOverride>,
    pub build_cohort_sha256: Option<String>,
    pub render_profile: Value,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct HdrBloomProfile {
    pub threshold: f32,
    pub radius_px: u32,
    pub intensity: f32,
    pub hdr_clamp: f32,
}

#[derive(Debug, Clone)]
pub(crate) struct RenderWorkerVfxBloomFrame {
    pub bloom_passes: Vec<RenderPass>,
    pub applied_emissive_overrides: Vec<AppliedEmissiveOverride>,
    pub build_cohort_sha256: Option<String>,
    pub render_profile: Value,
    pub bloom_profile: HdrBloomProfile,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TypedParticle {
    pub emitter_id: String,
    pub id: u32,
    pub position: [f32; 3],
    pub radius_px: f32,
    pub color_linear_rgb: [f32; 3],
    pub alpha: f32,
    pub lifetime_ticks: u64,
    pub depth: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TypedTrail {
    pub emitter_id: String,
    pub id: u32,
    pub points: Vec<[f32; 3]>,
    pub radius_px: f32,
    pub color_linear_rgb: [f32; 3],
    pub alpha: f32,
    pub lifetime_ticks: u64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct TypedTrailBloomProfile {
    pub threshold: f32,
    pub radius_px: u32,
    pub intensity: f32,
    pub hdr_clamp: f32,
    pub source_gain: f32,
}

impl TypedTrailBloomProfile {
    pub(crate) const FIXED: Self = Self {
        threshold: 1.0,
        radius_px: 8,
        intensity: 4.0,
        hdr_clamp: 16.0,
        source_gain: 8.0,
    };

    fn validate_fixed(self) -> Result<Self, GeometryWorkerError> {
        if self != Self::FIXED {
            return Err(GeometryWorkerError::Protocol);
        }
        Ok(self)
    }
}

#[derive(Debug, Clone)]
pub(crate) struct RenderWorkerVfxParticlesFrame {
    pub particle_passes: Vec<RenderPass>,
    pub particle_count: usize,
    pub emitter_counts: [usize; 2],
    pub seed_sha256: String,
    pub build_cohort_sha256: Option<String>,
    pub render_profile: Value,
}

/// Runtime-side projection of the closed animated-socket particle operation.
/// The worker owns the TRS application and camera-depth calculation; this
/// adapter only accepts the typed request projection and verifies the complete
/// response before a caller can persist any pass bytes.
#[derive(Debug, Clone)]
pub(crate) struct RenderWorkerAnimatedSocketParticlesFrame {
    pub particle_passes: Vec<RenderPass>,
    pub particle_count: usize,
    pub emitter_counts: [usize; 2],
    pub seed_sha256: String,
    pub projection_key_sha256: String,
    pub frame_index: u64,
    pub sample_time_ticks: u64,
    pub projection_input_sha256: String,
    pub projection_socket_transform_inventory_sha256: String,
    pub projection_socket_transform_readback_sha256: String,
    pub emitter_binding_sha256: String,
    pub world_particle_inventory_sha256: String,
    pub world_particle_inventory: Value,
    pub build_cohort_sha256: Option<String>,
    pub render_profile: Value,
}

#[derive(Debug, Clone)]
pub(crate) struct RenderWorkerAnimatedSocketTrailsFrame {
    pub trail_passes: Vec<RenderPass>,
    pub trail_count: usize,
    pub segment_count: usize,
    pub emitter_counts: [usize; 2],
    pub seed_sha256: String,
    pub projection_key_sha256: String,
    pub current_frame_index: u64,
    pub current_sample_time_ticks: u64,
    pub projection_input_sha256: String,
    pub projection_sample_set_sha256: String,
    pub emitter_binding_sha256: String,
    pub trail_inventory_sha256: String,
    pub trail_inventory: Value,
    pub build_cohort_sha256: Option<String>,
    pub render_profile: Value,
}

#[derive(Debug, Clone)]
pub(crate) struct RenderWorkerAnimatedSocketTrailsBloomFrame {
    pub trail_bloom_passes: Vec<RenderPass>,
    pub trail_count: usize,
    pub segment_count: usize,
    pub emitter_counts: [usize; 2],
    pub seed_sha256: String,
    pub projection_key_sha256: String,
    pub current_frame_index: u64,
    pub current_sample_time_ticks: u64,
    pub projection_input_sha256: String,
    pub projection_sample_set_sha256: String,
    pub emitter_binding_sha256: String,
    pub trail_inventory_sha256: String,
    pub trail_inventory: Value,
    pub build_cohort_sha256: Option<String>,
    pub render_profile: Value,
    pub trail_bloom_profile: TypedTrailBloomProfile,
}

#[derive(Debug, Clone)]
pub(crate) struct RenderWorkerVfxTrailsFrame {
    pub trail_passes: Vec<RenderPass>,
    pub trail_count: usize,
    pub segment_count: usize,
    pub emitter_counts: [usize; 2],
    pub seed_sha256: String,
    pub build_cohort_sha256: Option<String>,
    pub render_profile: Value,
}

#[derive(Debug, Clone)]
pub(crate) struct RenderWorkerVfxTrailsBloomFrame {
    pub trail_bloom_passes: Vec<RenderPass>,
    pub trail_count: usize,
    pub segment_count: usize,
    pub emitter_counts: [usize; 2],
    pub seed_sha256: String,
    pub build_cohort_sha256: Option<String>,
    pub render_profile: Value,
    pub trail_bloom_profile: TypedTrailBloomProfile,
}

#[derive(Debug, Clone)]
pub(crate) struct RenderPass {
    pub pass: String,
    pub png: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

pub(crate) fn render_fixed_glb(glb: &[u8]) -> Result<Vec<RenderPass>, GeometryWorkerError> {
    if glb.is_empty() || glb.len() > 64 * 1024 * 1024 {
        return Err(GeometryWorkerError::Protocol);
    }
    let encoded = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, glb);
    let result = execute_render_worker("render_fixed", json!({"glb_base64":encoded}))?;
    let object = strict_object(&result)?;
    require_exact_keys(object, &["schema_version", "passes"])?;
    if object.get("schema_version").and_then(Value::as_str) != Some("RenderWorkerResult@1") {
        return Err(GeometryWorkerError::Protocol);
    }
    let values = object
        .get("passes")
        .and_then(Value::as_array)
        .filter(|values| !values.is_empty() && values.len() <= 16)
        .ok_or(GeometryWorkerError::Protocol)?;
    let mut passes = Vec::with_capacity(values.len());
    for value in values {
        let pass = strict_object(value)?;
        require_exact_keys(pass, &["pass", "mime", "width", "height", "png_base64"])?;
        let pass_name = pass
            .get("pass")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty() && value.len() <= 64)
            .ok_or(GeometryWorkerError::Protocol)?;
        if pass.get("mime").and_then(Value::as_str) != Some("image/png") {
            return Err(GeometryWorkerError::Protocol);
        }
        let width = pass
            .get("width")
            .and_then(Value::as_u64)
            .and_then(|value| u32::try_from(value).ok())
            .filter(|value| *value > 0 && *value <= 4096)
            .ok_or(GeometryWorkerError::Protocol)?;
        let height = pass
            .get("height")
            .and_then(Value::as_u64)
            .and_then(|value| u32::try_from(value).ok())
            .filter(|value| *value > 0 && *value <= 4096)
            .ok_or(GeometryWorkerError::Protocol)?;
        let png = decode_png(pass.get("png_base64"), width, height)?;
        passes.push(RenderPass {
            pass: pass_name.to_owned(),
            png,
            width,
            height,
        });
    }
    Ok(passes)
}

pub(crate) fn render_glb(
    glb: &[u8],
    camera: &Value,
) -> Result<Vec<RenderPass>, GeometryWorkerError> {
    render_glb_with_worker_identity(glb, camera).map(|render| render.passes)
}

/// Render one bounded GLB and retain the child Worker cohort for the
/// Runtime-owned RenderSet evidence. The ordinary `render_glb` API deliberately
/// remains a pass-only helper for transient fit/search callers.
pub(crate) fn render_glb_with_worker_identity(
    glb: &[u8],
    camera: &Value,
) -> Result<RenderWorkerRender, GeometryWorkerError> {
    if glb.is_empty() || glb.len() > 64 * 1024 * 1024 {
        return Err(GeometryWorkerError::Protocol);
    }
    let encoded = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, glb);
    let result = execute_render_worker_with_metadata(
        "render_glb",
        json!({"glb_base64":encoded,"camera":camera}),
    )?;
    let object = strict_object(&result.result)?;
    require_exact_keys(
        object,
        &[
            "schema_version",
            "width",
            "height",
            "renderer_revision",
            "render_profile",
            "passes",
        ],
    )?;
    if object.get("schema_version").and_then(Value::as_str) != Some("RenderWorkerResult@2")
        || object.get("width").and_then(Value::as_u64) != Some(512)
        || object.get("height").and_then(Value::as_u64) != Some(512)
        || object.get("renderer_revision").and_then(Value::as_str) != Some("forgecad-renderer-2")
        || object.get("render_profile") != Some(&forgecad_worker_protocol::render_profile())
    {
        return Err(GeometryWorkerError::Protocol);
    }
    let values = object
        .get("passes")
        .and_then(Value::as_array)
        .filter(|values| values.len() == 9)
        .ok_or(GeometryWorkerError::Protocol)?;
    let expected = [
        "beauty",
        "silhouette",
        "depth",
        "normal",
        "ao",
        "part-id",
        "material-id",
        "wireframe",
        "uv-stretch",
    ];
    let mut passes = Vec::with_capacity(9);
    for (value, expected_name) in values.iter().zip(expected) {
        let pass = strict_object(value)?;
        require_exact_keys(pass, &["pass", "mime", "width", "height", "png_base64"])?;
        if pass.get("pass").and_then(Value::as_str) != Some(expected_name)
            || pass.get("mime").and_then(Value::as_str) != Some("image/png")
            || pass.get("width").and_then(Value::as_u64) != Some(512)
            || pass.get("height").and_then(Value::as_u64) != Some(512)
        {
            return Err(GeometryWorkerError::Protocol);
        }
        let png = decode_png(pass.get("png_base64"), 512, 512)?;
        passes.push(RenderPass {
            pass: expected_name.to_owned(),
            png,
            width: 512,
            height: 512,
        });
    }
    Ok(RenderWorkerRender {
        passes,
        build_cohort_sha256: result.build_cohort_sha256,
        render_profile: forgecad_worker_protocol::render_profile(),
    })
}

pub(crate) fn render_glb_vfx_frame_with_worker_identity(
    glb: &[u8],
    camera: &Value,
    overrides: &[EmissiveMaterialOverride],
) -> Result<RenderWorkerVfxFrame, GeometryWorkerError> {
    if glb.is_empty() || glb.len() > 64 * 1024 * 1024 || overrides.is_empty() || overrides.len() > 8
    {
        return Err(GeometryWorkerError::Protocol);
    }
    let encoded = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, glb);
    let override_values = overrides
        .iter()
        .map(|value| {
            json!({
                "material_zone_id":value.material_zone_id,
                "material_id":value.material_id,
                "color_linear_rgb":value.color_linear_rgb,
                "emissive_strength":value.emissive_strength
            })
        })
        .collect::<Vec<_>>();
    let result = execute_render_worker_with_metadata(
        "render_glb_vfx_frame",
        json!({
            "glb_base64":encoded,
            "camera":camera,
            "emissive_overrides":override_values
        }),
    )?;
    let object = strict_object(&result.result)?;
    require_exact_keys(
        object,
        &[
            "schema_version",
            "width",
            "height",
            "renderer_revision",
            "render_profile",
            "applied_emissive_overrides",
            "passes",
        ],
    )?;
    if object.get("schema_version").and_then(Value::as_str) != Some("RenderWorkerVfxFrameResult@1")
        || object.get("width").and_then(Value::as_u64) != Some(512)
        || object.get("height").and_then(Value::as_u64) != Some(512)
        || object.get("renderer_revision").and_then(Value::as_str) != Some("forgecad-renderer-2")
        || object.get("render_profile") != Some(&forgecad_worker_protocol::render_profile())
    {
        return Err(GeometryWorkerError::Protocol);
    }
    let applied_values = object
        .get("applied_emissive_overrides")
        .and_then(Value::as_array)
        .filter(|values| values.len() == overrides.len())
        .ok_or(GeometryWorkerError::Protocol)?;
    let mut applied_emissive_overrides = Vec::with_capacity(applied_values.len());
    for (actual, expected) in applied_values.iter().zip(overrides) {
        let actual = strict_object(actual)?;
        require_exact_keys(
            actual,
            &["material_zone_id", "material_id", "glb_material_index"],
        )?;
        let material_zone_id = actual
            .get("material_zone_id")
            .and_then(Value::as_str)
            .filter(|value| *value == expected.material_zone_id.as_str())
            .ok_or(GeometryWorkerError::Protocol)?;
        let material_id = actual
            .get("material_id")
            .and_then(Value::as_str)
            .filter(|value| *value == expected.material_id.as_str())
            .ok_or(GeometryWorkerError::Protocol)?;
        let glb_material_index = actual
            .get("glb_material_index")
            .and_then(Value::as_u64)
            .and_then(|value| usize::try_from(value).ok())
            .filter(|value| *value < 256)
            .ok_or(GeometryWorkerError::Protocol)?;
        applied_emissive_overrides.push(AppliedEmissiveOverride {
            material_zone_id: material_zone_id.to_owned(),
            material_id: material_id.to_owned(),
            glb_material_index,
        });
    }
    let values = object
        .get("passes")
        .and_then(Value::as_array)
        .filter(|values| values.len() == 9)
        .ok_or(GeometryWorkerError::Protocol)?;
    let expected_passes = [
        "beauty",
        "silhouette",
        "depth",
        "normal",
        "ao",
        "part-id",
        "material-id",
        "wireframe",
        "uv-stretch",
    ];
    let mut passes = Vec::with_capacity(9);
    for (value, expected_name) in values.iter().zip(expected_passes) {
        let pass = strict_object(value)?;
        require_exact_keys(pass, &["pass", "mime", "width", "height", "png_base64"])?;
        if pass.get("pass").and_then(Value::as_str) != Some(expected_name)
            || pass.get("mime").and_then(Value::as_str) != Some("image/png")
            || pass.get("width").and_then(Value::as_u64) != Some(512)
            || pass.get("height").and_then(Value::as_u64) != Some(512)
        {
            return Err(GeometryWorkerError::Protocol);
        }
        passes.push(RenderPass {
            pass: expected_name.to_owned(),
            png: decode_png(pass.get("png_base64"), 512, 512)?,
            width: 512,
            height: 512,
        });
    }
    Ok(RenderWorkerVfxFrame {
        passes,
        applied_emissive_overrides,
        build_cohort_sha256: result.build_cohort_sha256,
        render_profile: forgecad_worker_protocol::render_profile(),
    })
}

pub(crate) fn render_glb_vfx_bloom_frame_with_worker_identity(
    glb: &[u8],
    camera: &Value,
    overrides: &[EmissiveMaterialOverride],
    bloom_profile: HdrBloomProfile,
) -> Result<RenderWorkerVfxBloomFrame, GeometryWorkerError> {
    if glb.is_empty()
        || glb.len() > 64 * 1024 * 1024
        || overrides.is_empty()
        || overrides.len() > 8
        || !bloom_profile.threshold.is_finite()
        || !(0.0..=16.0).contains(&bloom_profile.threshold)
        || !(1..=8).contains(&bloom_profile.radius_px)
        || !bloom_profile.intensity.is_finite()
        || !(0.0..=4.0).contains(&bloom_profile.intensity)
        || !bloom_profile.hdr_clamp.is_finite()
        || !(1.0..=16.0).contains(&bloom_profile.hdr_clamp)
    {
        return Err(GeometryWorkerError::Protocol);
    }
    let encoded = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, glb);
    let override_values = overrides
        .iter()
        .map(|value| {
            json!({
                "material_zone_id":value.material_zone_id,
                "material_id":value.material_id,
                "color_linear_rgb":value.color_linear_rgb,
                "emissive_strength":value.emissive_strength
            })
        })
        .collect::<Vec<_>>();
    let result = execute_render_worker_with_metadata(
        "render_glb_vfx_bloom_frame",
        json!({
            "glb_base64":encoded,
            "camera":camera,
            "emissive_overrides":override_values,
            "bloom_profile":{
                "threshold":bloom_profile.threshold,
                "radius_px":bloom_profile.radius_px,
                "intensity":bloom_profile.intensity,
                "hdr_clamp":bloom_profile.hdr_clamp
            }
        }),
    )?;
    let object = strict_object(&result.result)?;
    require_exact_keys(
        object,
        &[
            "schema_version",
            "width",
            "height",
            "renderer_revision",
            "render_profile",
            "bloom_profile",
            "applied_emissive_overrides",
            "bloom_passes",
        ],
    )?;
    if object.get("schema_version").and_then(Value::as_str)
        != Some("RenderWorkerVfxBloomFrameResult@1")
        || object.get("width").and_then(Value::as_u64) != Some(512)
        || object.get("height").and_then(Value::as_u64) != Some(512)
        || object.get("renderer_revision").and_then(Value::as_str) != Some("forgecad-renderer-2")
        || object.get("render_profile") != Some(&forgecad_worker_protocol::render_profile())
    {
        return Err(GeometryWorkerError::Protocol);
    }
    let returned_profile = strict_object(
        object
            .get("bloom_profile")
            .ok_or(GeometryWorkerError::Protocol)?,
    )?;
    require_exact_keys(
        returned_profile,
        &[
            "threshold",
            "radius_px",
            "intensity",
            "hdr_clamp",
            "blur_passes",
        ],
    )?;
    if returned_profile
        .get("threshold")
        .and_then(Value::as_f64)
        .map(|value| value as f32)
        != Some(bloom_profile.threshold)
        || returned_profile.get("radius_px").and_then(Value::as_u64)
            != Some(u64::from(bloom_profile.radius_px))
        || returned_profile
            .get("intensity")
            .and_then(Value::as_f64)
            .map(|value| value as f32)
            != Some(bloom_profile.intensity)
        || returned_profile
            .get("hdr_clamp")
            .and_then(Value::as_f64)
            .map(|value| value as f32)
            != Some(bloom_profile.hdr_clamp)
        || returned_profile.get("blur_passes").and_then(Value::as_u64) != Some(2)
    {
        return Err(GeometryWorkerError::Protocol);
    }
    let applied_values = object
        .get("applied_emissive_overrides")
        .and_then(Value::as_array)
        .filter(|values| values.len() == overrides.len())
        .ok_or(GeometryWorkerError::Protocol)?;
    let mut applied_emissive_overrides = Vec::with_capacity(applied_values.len());
    for (actual, expected) in applied_values.iter().zip(overrides) {
        let actual = strict_object(actual)?;
        require_exact_keys(
            actual,
            &["material_zone_id", "material_id", "glb_material_index"],
        )?;
        let material_zone_id = actual
            .get("material_zone_id")
            .and_then(Value::as_str)
            .filter(|value| *value == expected.material_zone_id.as_str())
            .ok_or(GeometryWorkerError::Protocol)?;
        let material_id = actual
            .get("material_id")
            .and_then(Value::as_str)
            .filter(|value| *value == expected.material_id.as_str())
            .ok_or(GeometryWorkerError::Protocol)?;
        let glb_material_index = actual
            .get("glb_material_index")
            .and_then(Value::as_u64)
            .and_then(|value| usize::try_from(value).ok())
            .filter(|value| *value < 256)
            .ok_or(GeometryWorkerError::Protocol)?;
        applied_emissive_overrides.push(AppliedEmissiveOverride {
            material_zone_id: material_zone_id.to_owned(),
            material_id: material_id.to_owned(),
            glb_material_index,
        });
    }
    let parse_passes = |key: &str, expected: &[&str]| {
        let values = object
            .get(key)
            .and_then(Value::as_array)
            .filter(|values| values.len() == expected.len())
            .ok_or(GeometryWorkerError::Protocol)?;
        let mut passes = Vec::with_capacity(expected.len());
        for (value, expected_name) in values.iter().zip(expected.iter().copied()) {
            let pass = strict_object(value)?;
            require_exact_keys(pass, &["pass", "mime", "width", "height", "png_base64"])?;
            if pass.get("pass").and_then(Value::as_str) != Some(expected_name)
                || pass.get("mime").and_then(Value::as_str) != Some("image/png")
                || pass.get("width").and_then(Value::as_u64) != Some(512)
                || pass.get("height").and_then(Value::as_u64) != Some(512)
            {
                return Err(GeometryWorkerError::Protocol);
            }
            passes.push(RenderPass {
                pass: expected_name.to_owned(),
                png: decode_png(pass.get("png_base64"), 512, 512)?,
                width: 512,
                height: 512,
            });
        }
        Ok::<Vec<RenderPass>, GeometryWorkerError>(passes)
    };
    let bloom_passes = parse_passes("bloom_passes", &["emissive-source", "bloom-contribution"])?;
    Ok(RenderWorkerVfxBloomFrame {
        bloom_passes,
        applied_emissive_overrides,
        build_cohort_sha256: result.build_cohort_sha256,
        render_profile: forgecad_worker_protocol::render_profile(),
        bloom_profile,
    })
}

pub(crate) fn render_typed_particles_with_worker_identity(
    glb: &[u8],
    camera: &Value,
    particles: &[TypedParticle],
    seed_sha256: &str,
) -> Result<RenderWorkerVfxParticlesFrame, GeometryWorkerError> {
    if glb.is_empty()
        || glb.len() > 64 * 1024 * 1024
        || particles.is_empty()
        || particles.len() > 128
        || seed_sha256.len() != 64
    {
        return Err(GeometryWorkerError::Protocol);
    }
    let encoded = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, glb);
    let particle_values = particles
        .iter()
        .map(|particle| {
            json!({
                "emitter_id":particle.emitter_id,
                "id":particle.id,
                "position":particle.position,
                "radius_px":particle.radius_px,
                "color_linear_rgb":particle.color_linear_rgb,
                "alpha":particle.alpha,
                "lifetime_ticks":particle.lifetime_ticks,
                "depth":particle.depth
            })
        })
        .collect::<Vec<_>>();
    let result = execute_render_worker_with_metadata(
        "render_typed_particles",
        json!({
            "glb_base64":encoded,
            "camera":camera,
            "particles":particle_values,
            "seed_sha256":seed_sha256
        }),
    )?;
    let object = strict_object(&result.result)?;
    require_exact_keys(
        object,
        &[
            "schema_version",
            "width",
            "height",
            "renderer_revision",
            "render_profile",
            "seed_sha256",
            "particle_count",
            "emitter_counts",
            "particle_passes",
        ],
    )?;
    if object.get("schema_version").and_then(Value::as_str)
        != Some("RenderWorkerVfxParticlesFrameResult@1")
        || object.get("width").and_then(Value::as_u64) != Some(512)
        || object.get("height").and_then(Value::as_u64) != Some(512)
        || object.get("renderer_revision").and_then(Value::as_str) != Some("forgecad-renderer-2")
        || object.get("render_profile") != Some(&forgecad_worker_protocol::render_profile())
        || object.get("seed_sha256").and_then(Value::as_str) != Some(seed_sha256)
        || object.get("particle_count").and_then(Value::as_u64) != Some(particles.len() as u64)
    {
        return Err(GeometryWorkerError::Protocol);
    }
    let counts = strict_object(
        object
            .get("emitter_counts")
            .ok_or(GeometryWorkerError::Protocol)?,
    )?;
    require_exact_keys(counts, &["muzzle-burst", "energy-core-sparks"])?;
    let emitter_counts = [
        counts
            .get("muzzle-burst")
            .and_then(Value::as_u64)
            .map(|value| value as usize)
            .ok_or(GeometryWorkerError::Protocol)?,
        counts
            .get("energy-core-sparks")
            .and_then(Value::as_u64)
            .map(|value| value as usize)
            .ok_or(GeometryWorkerError::Protocol)?,
    ];
    let expected_passes = ["particle-color", "particle-id", "particle-depth"];
    let values = object
        .get("particle_passes")
        .and_then(Value::as_array)
        .filter(|values| values.len() == expected_passes.len())
        .ok_or(GeometryWorkerError::Protocol)?;
    let mut particle_passes = Vec::with_capacity(values.len());
    for (value, expected_name) in values.iter().zip(expected_passes) {
        let pass = strict_object(value)?;
        require_exact_keys(pass, &["pass", "mime", "width", "height", "png_base64"])?;
        if pass.get("pass").and_then(Value::as_str) != Some(expected_name)
            || pass.get("mime").and_then(Value::as_str) != Some("image/png")
            || pass.get("width").and_then(Value::as_u64) != Some(512)
            || pass.get("height").and_then(Value::as_u64) != Some(512)
        {
            return Err(GeometryWorkerError::Protocol);
        }
        particle_passes.push(RenderPass {
            pass: expected_name.to_owned(),
            png: decode_png(pass.get("png_base64"), 512, 512)?,
            width: 512,
            height: 512,
        });
    }
    Ok(RenderWorkerVfxParticlesFrame {
        particle_passes,
        particle_count: particles.len(),
        emitter_counts,
        seed_sha256: seed_sha256.to_owned(),
        build_cohort_sha256: result.build_cohort_sha256,
        render_profile: forgecad_worker_protocol::render_profile(),
    })
}

fn animated_socket_emitter_binding_values(
    emitter_bindings: &Value,
) -> Result<&Vec<Value>, GeometryWorkerError> {
    let object = strict_object(emitter_bindings)?;
    require_exact_keys(object, &["schema_version", "emitters"])?;
    if object.get("schema_version").and_then(Value::as_str)
        != Some("RenderWorkerAnimatedSocketEmitterBindings@1")
    {
        return Err(GeometryWorkerError::Protocol);
    }
    object
        .get("emitters")
        .and_then(Value::as_array)
        .filter(|values| values.len() == 2)
        .ok_or(GeometryWorkerError::Protocol)
}

pub(crate) fn render_typed_animated_socket_particles_with_worker_identity(
    glb: &[u8],
    camera: &Value,
    projection_key_sha256: &str,
    frame_index: u64,
    sample_time_ticks: u64,
    projection_input_sha256: &str,
    projection_socket_transform_inventory_sha256: &str,
    projection_socket_transform_readback_sha256: &str,
    emitter_bindings: &Value,
    particles: &Value,
    seed_sha256: &str,
) -> Result<RenderWorkerAnimatedSocketParticlesFrame, GeometryWorkerError> {
    let emitter_binding_values = animated_socket_emitter_binding_values(emitter_bindings)?;
    if glb.is_empty()
        || glb.len() > 64 * 1024 * 1024
        || !forgecad_contracts::is_sha256(projection_key_sha256)
        || frame_index > 15
        || sample_time_ticks > 1_000_000
        || !forgecad_contracts::is_sha256(projection_input_sha256)
        || !forgecad_contracts::is_sha256(projection_socket_transform_inventory_sha256)
        || !forgecad_contracts::is_sha256(projection_socket_transform_readback_sha256)
        || !forgecad_contracts::is_sha256(seed_sha256)
        || !particles.is_array()
    {
        return Err(GeometryWorkerError::Protocol);
    }
    let encoded = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, glb);
    let result = execute_render_worker_with_metadata(
        forgecad_worker_protocol::RENDER_TYPED_ANIMATED_SOCKET_PARTICLES_OPERATION,
        json!({
            "glb_base64":encoded,
            "camera":camera,
            "projection_key_sha256":projection_key_sha256,
            "frame_index":frame_index,
            "sample_time_ticks":sample_time_ticks,
            "projection_input_sha256":projection_input_sha256,
            "projection_socket_transform_inventory_sha256":projection_socket_transform_inventory_sha256,
            "projection_socket_transform_readback_sha256":projection_socket_transform_readback_sha256,
            "emitter_bindings":emitter_binding_values,
            "particles":particles,
            "seed_sha256":seed_sha256
        }),
    )?;
    let object = strict_object(&result.result)?;
    require_exact_keys(
        object,
        &[
            "schema_version",
            "width",
            "height",
            "renderer_revision",
            "render_profile",
            "projection_key_sha256",
            "frame_index",
            "sample_time_ticks",
            "projection_input_sha256",
            "projection_socket_transform_inventory_sha256",
            "projection_socket_transform_readback_sha256",
            "seed_sha256",
            "emitter_binding_sha256",
            "world_particle_inventory_sha256",
            "world_particle_inventory",
            "particle_count",
            "emitter_counts",
            "particle_passes",
        ],
    )?;
    let profile = forgecad_worker_protocol::render_profile();
    if object.get("schema_version").and_then(Value::as_str)
        != Some("RenderWorkerAnimatedSocketParticlesFrameResult@1")
        || object.get("width").and_then(Value::as_u64) != Some(512)
        || object.get("height").and_then(Value::as_u64) != Some(512)
        || object.get("renderer_revision").and_then(Value::as_str) != Some("forgecad-renderer-2")
        || object.get("render_profile") != Some(&profile)
        || object.get("projection_key_sha256").and_then(Value::as_str)
            != Some(projection_key_sha256)
        || object.get("frame_index").and_then(Value::as_u64) != Some(frame_index)
        || object.get("sample_time_ticks").and_then(Value::as_u64) != Some(sample_time_ticks)
        || object
            .get("projection_input_sha256")
            .and_then(Value::as_str)
            != Some(projection_input_sha256)
        || object
            .get("projection_socket_transform_inventory_sha256")
            .and_then(Value::as_str)
            != Some(projection_socket_transform_inventory_sha256)
        || object
            .get("projection_socket_transform_readback_sha256")
            .and_then(Value::as_str)
            != Some(projection_socket_transform_readback_sha256)
        || object.get("seed_sha256").and_then(Value::as_str) != Some(seed_sha256)
        || object
            .get("emitter_binding_sha256")
            .and_then(Value::as_str)
            .is_none_or(|value| !forgecad_contracts::is_sha256(value))
        || object
            .get("world_particle_inventory_sha256")
            .and_then(Value::as_str)
            .is_none_or(|value| !forgecad_contracts::is_sha256(value))
        || object.get("particle_count").and_then(Value::as_u64) != Some(56)
    {
        return Err(GeometryWorkerError::Protocol);
    }
    let emitter_binding_sha256 = object
        .get("emitter_binding_sha256")
        .and_then(Value::as_str)
        .ok_or(GeometryWorkerError::Protocol)?
        .to_owned();
    let world_particle_inventory_sha256 = object
        .get("world_particle_inventory_sha256")
        .and_then(Value::as_str)
        .ok_or(GeometryWorkerError::Protocol)?
        .to_owned();
    let world_particle_inventory = object
        .get("world_particle_inventory")
        .cloned()
        .ok_or(GeometryWorkerError::Protocol)?;
    let world_inventory_object = strict_object(&world_particle_inventory)?;
    require_exact_keys(
        world_inventory_object,
        &[
            "schema_version",
            "projection_key_sha256",
            "frame_index",
            "sample_time_ticks",
            "seed_sha256",
            "particle_count",
            "particles",
            "canonical_sha256",
        ],
    )?;
    if world_inventory_object
        .get("schema_version")
        .and_then(Value::as_str)
        != Some("RenderWorkerAnimatedSocketParticleWorldInventory@1")
        || world_inventory_object
            .get("projection_key_sha256")
            .and_then(Value::as_str)
            != Some(projection_key_sha256)
        || world_inventory_object
            .get("frame_index")
            .and_then(Value::as_u64)
            != Some(frame_index)
        || world_inventory_object
            .get("sample_time_ticks")
            .and_then(Value::as_u64)
            != Some(sample_time_ticks)
        || world_inventory_object
            .get("seed_sha256")
            .and_then(Value::as_str)
            != Some(seed_sha256)
        || world_inventory_object
            .get("particle_count")
            .and_then(Value::as_u64)
            != Some(56)
        || world_inventory_object
            .get("canonical_sha256")
            .and_then(Value::as_str)
            != Some(world_particle_inventory_sha256.as_str())
        || world_inventory_object
            .get("particles")
            .and_then(Value::as_array)
            .is_none_or(|values| values.len() != 56)
    {
        return Err(GeometryWorkerError::Protocol);
    }
    let mut world_inventory_preimage = world_inventory_object.clone();
    world_inventory_preimage.remove("canonical_sha256");
    if forgecad_worker_protocol::canonical_json_sha256(&Value::Object(world_inventory_preimage))
        != world_particle_inventory_sha256
    {
        return Err(GeometryWorkerError::Protocol);
    }
    let counts = strict_object(
        object
            .get("emitter_counts")
            .ok_or(GeometryWorkerError::Protocol)?,
    )?;
    require_exact_keys(counts, &["muzzle-burst", "energy-core-sparks"])?;
    let emitter_counts = [
        counts
            .get("muzzle-burst")
            .and_then(Value::as_u64)
            .map(|value| value as usize)
            .ok_or(GeometryWorkerError::Protocol)?,
        counts
            .get("energy-core-sparks")
            .and_then(Value::as_u64)
            .map(|value| value as usize)
            .ok_or(GeometryWorkerError::Protocol)?,
    ];
    if emitter_counts != [24, 32] {
        return Err(GeometryWorkerError::Protocol);
    }
    let expected_passes = ["particle-color", "particle-id", "particle-depth"];
    let values = object
        .get("particle_passes")
        .and_then(Value::as_array)
        .filter(|values| values.len() == expected_passes.len())
        .ok_or(GeometryWorkerError::Protocol)?;
    let mut particle_passes = Vec::with_capacity(values.len());
    for (value, expected_name) in values.iter().zip(expected_passes) {
        let pass = strict_object(value)?;
        require_exact_keys(pass, &["pass", "mime", "width", "height", "png_base64"])?;
        if pass.get("pass").and_then(Value::as_str) != Some(expected_name)
            || pass.get("mime").and_then(Value::as_str) != Some("image/png")
            || pass.get("width").and_then(Value::as_u64) != Some(512)
            || pass.get("height").and_then(Value::as_u64) != Some(512)
        {
            return Err(GeometryWorkerError::Protocol);
        }
        particle_passes.push(RenderPass {
            pass: expected_name.to_owned(),
            png: decode_png(pass.get("png_base64"), 512, 512)?,
            width: 512,
            height: 512,
        });
    }
    Ok(RenderWorkerAnimatedSocketParticlesFrame {
        particle_passes,
        particle_count: 56,
        emitter_counts,
        seed_sha256: seed_sha256.to_owned(),
        projection_key_sha256: projection_key_sha256.to_owned(),
        frame_index,
        sample_time_ticks,
        projection_input_sha256: projection_input_sha256.to_owned(),
        projection_socket_transform_inventory_sha256: projection_socket_transform_inventory_sha256
            .to_owned(),
        projection_socket_transform_readback_sha256: projection_socket_transform_readback_sha256
            .to_owned(),
        emitter_binding_sha256,
        world_particle_inventory_sha256,
        world_particle_inventory,
        build_cohort_sha256: result.build_cohort_sha256,
        render_profile: profile,
    })
}

fn validate_animated_socket_trail_adapter_inputs(
    glb: &[u8],
    projection_key_sha256: &str,
    projection_input_sha256: &str,
    current_frame_index: u64,
    current_sample_time_ticks: u64,
    projection_samples: &Value,
    trails: &Value,
    seed_sha256: &str,
) -> Result<usize, GeometryWorkerError> {
    if glb.is_empty()
        || glb.len() > 64 * 1024 * 1024
        || !forgecad_contracts::is_sha256(projection_key_sha256)
        || !forgecad_contracts::is_sha256(projection_input_sha256)
        || !forgecad_contracts::is_sha256(seed_sha256)
        || current_frame_index > 15
        || current_sample_time_ticks > 1_000_000
    {
        return Err(GeometryWorkerError::Protocol);
    }
    let sample_count = projection_samples
        .as_array()
        .filter(|values| (2..=9).contains(&values.len()))
        .map(Vec::len)
        .ok_or(GeometryWorkerError::Protocol)?;
    if trails.as_array().is_none_or(|values| values.len() != 2) {
        return Err(GeometryWorkerError::Protocol);
    }
    Ok(sample_count)
}

fn animated_socket_trail_payload(
    glb: &[u8],
    camera: &Value,
    projection_key_sha256: &str,
    projection_input_sha256: &str,
    current_frame_index: u64,
    current_sample_time_ticks: u64,
    projection_samples: &Value,
    trails: &Value,
    seed_sha256: &str,
    bloom_profile: Option<TypedTrailBloomProfile>,
) -> Value {
    let mut payload = json!({
        "glb_base64":base64::Engine::encode(&base64::engine::general_purpose::STANDARD, glb),
        "camera":camera,
        "projection_key_sha256":projection_key_sha256,
        "current_frame_index":current_frame_index,
        "current_sample_time_ticks":current_sample_time_ticks,
        "projection_input_sha256":projection_input_sha256,
        "projection_samples":projection_samples,
        "trails":trails,
        "seed_sha256":seed_sha256
    });
    if let Some(profile) = bloom_profile {
        payload["trail_bloom_profile"] = json!({
            "threshold":profile.threshold,
            "radius_px":profile.radius_px,
            "intensity":profile.intensity,
            "hdr_clamp":profile.hdr_clamp,
            "source_gain":profile.source_gain
        });
    }
    payload
}

fn parse_animated_socket_trail_common(
    _result: &geometry_worker::SiblingWorkerResult,
    object: &serde_json::Map<String, Value>,
    projection_key_sha256: &str,
    projection_input_sha256: &str,
    current_frame_index: u64,
    current_sample_time_ticks: u64,
    seed_sha256: &str,
    sample_count: usize,
    bloom: bool,
) -> Result<(String, String, String, String, Value, usize, [usize; 2]), GeometryWorkerError> {
    let expected_schema = if bloom {
        "RenderWorkerAnimatedSocketTrailsBloomFrameResult@1"
    } else {
        "RenderWorkerAnimatedSocketTrailsFrameResult@1"
    };
    let mut expected = vec![
        "schema_version",
        "width",
        "height",
        "renderer_revision",
        "render_profile",
        "projection_key_sha256",
        "current_frame_index",
        "current_sample_time_ticks",
        "projection_input_sha256",
        "projection_sample_set_sha256",
        "emitter_binding_sha256",
        "trail_inventory_sha256",
        "trail_inventory",
        "seed_sha256",
        "trail_count",
        "segment_count",
        "emitter_counts",
    ];
    expected.push(if bloom {
        "trail_bloom_profile"
    } else {
        "trail_passes"
    });
    if bloom {
        expected.push("trail_bloom_passes");
    }
    require_exact_keys(object, &expected)?;
    let profile = forgecad_worker_protocol::render_profile();
    if object.get("schema_version").and_then(Value::as_str) != Some(expected_schema)
        || object.get("width").and_then(Value::as_u64) != Some(512)
        || object.get("height").and_then(Value::as_u64) != Some(512)
        || object.get("renderer_revision").and_then(Value::as_str) != Some("forgecad-renderer-2")
        || object.get("render_profile") != Some(&profile)
        || object.get("projection_key_sha256").and_then(Value::as_str)
            != Some(projection_key_sha256)
        || object.get("current_frame_index").and_then(Value::as_u64) != Some(current_frame_index)
        || object
            .get("current_sample_time_ticks")
            .and_then(Value::as_u64)
            != Some(current_sample_time_ticks)
        || object
            .get("projection_input_sha256")
            .and_then(Value::as_str)
            != Some(projection_input_sha256)
        || object.get("seed_sha256").and_then(Value::as_str) != Some(seed_sha256)
        || object.get("trail_count").and_then(Value::as_u64) != Some(2)
        || object.get("segment_count").and_then(Value::as_u64)
            != Some((2 * sample_count.saturating_sub(1)) as u64)
    {
        return Err(GeometryWorkerError::Protocol);
    }
    let projection_sample_set_sha256 = object
        .get("projection_sample_set_sha256")
        .and_then(Value::as_str)
        .filter(|value| forgecad_contracts::is_sha256(value))
        .ok_or(GeometryWorkerError::Protocol)?
        .to_owned();
    let emitter_binding_sha256 = object
        .get("emitter_binding_sha256")
        .and_then(Value::as_str)
        .filter(|value| forgecad_contracts::is_sha256(value))
        .ok_or(GeometryWorkerError::Protocol)?
        .to_owned();
    let trail_inventory_sha256 = object
        .get("trail_inventory_sha256")
        .and_then(Value::as_str)
        .filter(|value| forgecad_contracts::is_sha256(value))
        .ok_or(GeometryWorkerError::Protocol)?
        .to_owned();
    let trail_inventory = object
        .get("trail_inventory")
        .cloned()
        .ok_or(GeometryWorkerError::Protocol)?;
    let inventory_object = strict_object(&trail_inventory)?;
    require_exact_keys(
        inventory_object,
        &[
            "schema_version",
            "projection_key_sha256",
            "current_frame_index",
            "current_sample_time_ticks",
            "sample_count",
            "seed_sha256",
            "trails",
            "canonical_sha256",
        ],
    )?;
    if inventory_object
        .get("schema_version")
        .and_then(Value::as_str)
        != Some("RenderWorkerAnimatedSocketTrailInventory@1")
        || inventory_object
            .get("projection_key_sha256")
            .and_then(Value::as_str)
            != Some(projection_key_sha256)
        || inventory_object
            .get("current_frame_index")
            .and_then(Value::as_u64)
            != Some(current_frame_index)
        || inventory_object
            .get("current_sample_time_ticks")
            .and_then(Value::as_u64)
            != Some(current_sample_time_ticks)
        || inventory_object.get("sample_count").and_then(Value::as_u64) != Some(sample_count as u64)
        || inventory_object.get("seed_sha256").and_then(Value::as_str) != Some(seed_sha256)
        || inventory_object
            .get("trails")
            .and_then(Value::as_array)
            .is_none_or(|values| values.len() != 2)
        || inventory_object
            .get("canonical_sha256")
            .and_then(Value::as_str)
            != Some(trail_inventory_sha256.as_str())
    {
        return Err(GeometryWorkerError::Protocol);
    }
    let mut inventory_preimage = inventory_object.clone();
    inventory_preimage.remove("canonical_sha256");
    inventory_preimage.remove("seed_sha256");
    if forgecad_worker_protocol::canonical_json_sha256(&Value::Object(inventory_preimage))
        != trail_inventory_sha256
    {
        return Err(GeometryWorkerError::Protocol);
    }
    let counts = strict_object(
        object
            .get("emitter_counts")
            .ok_or(GeometryWorkerError::Protocol)?,
    )?;
    require_exact_keys(counts, &["muzzle-trail", "energy-core-trail"])?;
    let emitter_counts = [
        counts
            .get("muzzle-trail")
            .and_then(Value::as_u64)
            .map(|value| value as usize)
            .ok_or(GeometryWorkerError::Protocol)?,
        counts
            .get("energy-core-trail")
            .and_then(Value::as_u64)
            .map(|value| value as usize)
            .ok_or(GeometryWorkerError::Protocol)?,
    ];
    if emitter_counts != [1, 1] {
        return Err(GeometryWorkerError::Protocol);
    }
    Ok((
        projection_sample_set_sha256,
        emitter_binding_sha256,
        trail_inventory_sha256,
        seed_sha256.to_owned(),
        trail_inventory,
        (2 * sample_count.saturating_sub(1)),
        emitter_counts,
    ))
}

fn decode_animated_socket_trail_passes(
    object: &serde_json::Map<String, Value>,
    key: &str,
    expected_names: &[&str],
) -> Result<Vec<RenderPass>, GeometryWorkerError> {
    let values = object
        .get(key)
        .and_then(Value::as_array)
        .filter(|values| values.len() == expected_names.len())
        .ok_or(GeometryWorkerError::Protocol)?;
    values
        .iter()
        .zip(expected_names)
        .map(|(value, expected_name)| {
            let pass = strict_object(value)?;
            require_exact_keys(pass, &["pass", "mime", "width", "height", "png_base64"])?;
            if pass.get("pass").and_then(Value::as_str) != Some(*expected_name)
                || pass.get("mime").and_then(Value::as_str) != Some("image/png")
                || pass.get("width").and_then(Value::as_u64) != Some(512)
                || pass.get("height").and_then(Value::as_u64) != Some(512)
            {
                return Err(GeometryWorkerError::Protocol);
            }
            Ok(RenderPass {
                pass: (*expected_name).to_owned(),
                png: decode_png(pass.get("png_base64"), 512, 512)?,
                width: 512,
                height: 512,
            })
        })
        .collect()
}

pub(crate) fn render_typed_animated_socket_trails_with_worker_identity(
    glb: &[u8],
    camera: &Value,
    projection_key_sha256: &str,
    projection_input_sha256: &str,
    current_frame_index: u64,
    current_sample_time_ticks: u64,
    projection_samples: &Value,
    trails: &Value,
    seed_sha256: &str,
) -> Result<RenderWorkerAnimatedSocketTrailsFrame, GeometryWorkerError> {
    let sample_count = validate_animated_socket_trail_adapter_inputs(
        glb,
        projection_key_sha256,
        projection_input_sha256,
        current_frame_index,
        current_sample_time_ticks,
        projection_samples,
        trails,
        seed_sha256,
    )?;
    let result = execute_render_worker_with_metadata(
        forgecad_worker_protocol::RENDER_TYPED_ANIMATED_SOCKET_TRAILS_OPERATION,
        animated_socket_trail_payload(
            glb,
            camera,
            projection_key_sha256,
            projection_input_sha256,
            current_frame_index,
            current_sample_time_ticks,
            projection_samples,
            trails,
            seed_sha256,
            None,
        ),
    )?;
    let object = strict_object(&result.result)?;
    let (
        projection_sample_set_sha256,
        emitter_binding_sha256,
        trail_inventory_sha256,
        seed_sha256,
        trail_inventory,
        segment_count,
        emitter_counts,
    ) = parse_animated_socket_trail_common(
        &result,
        object,
        projection_key_sha256,
        projection_input_sha256,
        current_frame_index,
        current_sample_time_ticks,
        seed_sha256,
        sample_count,
        false,
    )?;
    let trail_passes = decode_animated_socket_trail_passes(
        object,
        "trail_passes",
        &["trail-color", "trail-id", "trail-depth"],
    )?;
    Ok(RenderWorkerAnimatedSocketTrailsFrame {
        trail_passes,
        trail_count: 2,
        segment_count,
        emitter_counts,
        seed_sha256,
        projection_key_sha256: projection_key_sha256.to_owned(),
        current_frame_index,
        current_sample_time_ticks,
        projection_input_sha256: projection_input_sha256.to_owned(),
        projection_sample_set_sha256,
        emitter_binding_sha256,
        trail_inventory_sha256,
        trail_inventory,
        build_cohort_sha256: result.build_cohort_sha256,
        render_profile: forgecad_worker_protocol::render_profile(),
    })
}

pub(crate) fn render_typed_animated_socket_trails_bloom_with_worker_identity(
    glb: &[u8],
    camera: &Value,
    projection_key_sha256: &str,
    projection_input_sha256: &str,
    current_frame_index: u64,
    current_sample_time_ticks: u64,
    projection_samples: &Value,
    trails: &Value,
    seed_sha256: &str,
    trail_bloom_profile: TypedTrailBloomProfile,
) -> Result<RenderWorkerAnimatedSocketTrailsBloomFrame, GeometryWorkerError> {
    let trail_bloom_profile = trail_bloom_profile.validate_fixed()?;
    let sample_count = validate_animated_socket_trail_adapter_inputs(
        glb,
        projection_key_sha256,
        projection_input_sha256,
        current_frame_index,
        current_sample_time_ticks,
        projection_samples,
        trails,
        seed_sha256,
    )?;
    let result = execute_render_worker_with_metadata(
        forgecad_worker_protocol::RENDER_TYPED_ANIMATED_SOCKET_TRAILS_BLOOM_OPERATION,
        animated_socket_trail_payload(
            glb,
            camera,
            projection_key_sha256,
            projection_input_sha256,
            current_frame_index,
            current_sample_time_ticks,
            projection_samples,
            trails,
            seed_sha256,
            Some(trail_bloom_profile),
        ),
    )?;
    let object = strict_object(&result.result)?;
    let (
        projection_sample_set_sha256,
        emitter_binding_sha256,
        trail_inventory_sha256,
        seed_sha256,
        trail_inventory,
        segment_count,
        emitter_counts,
    ) = parse_animated_socket_trail_common(
        &result,
        object,
        projection_key_sha256,
        projection_input_sha256,
        current_frame_index,
        current_sample_time_ticks,
        seed_sha256,
        sample_count,
        true,
    )?;
    let returned_profile = strict_object(
        object
            .get("trail_bloom_profile")
            .ok_or(GeometryWorkerError::Protocol)?,
    )?;
    require_exact_keys(
        returned_profile,
        &[
            "threshold",
            "radius_px",
            "intensity",
            "hdr_clamp",
            "source_gain",
            "blur_passes",
        ],
    )?;
    if returned_profile
        .get("threshold")
        .and_then(Value::as_f64)
        .map(|v| v as f32)
        != Some(trail_bloom_profile.threshold)
        || returned_profile.get("radius_px").and_then(Value::as_u64)
            != Some(u64::from(trail_bloom_profile.radius_px))
        || returned_profile
            .get("intensity")
            .and_then(Value::as_f64)
            .map(|v| v as f32)
            != Some(trail_bloom_profile.intensity)
        || returned_profile
            .get("hdr_clamp")
            .and_then(Value::as_f64)
            .map(|v| v as f32)
            != Some(trail_bloom_profile.hdr_clamp)
        || returned_profile
            .get("source_gain")
            .and_then(Value::as_f64)
            .map(|v| v as f32)
            != Some(trail_bloom_profile.source_gain)
        || returned_profile.get("blur_passes").and_then(Value::as_u64) != Some(2)
    {
        return Err(GeometryWorkerError::Protocol);
    }
    let trail_bloom_passes = decode_animated_socket_trail_passes(
        object,
        "trail_bloom_passes",
        &[
            "trail-color",
            "trail-id",
            "trail-depth",
            "trail-emissive-source",
            "trail-bloom-contribution",
        ],
    )?;
    Ok(RenderWorkerAnimatedSocketTrailsBloomFrame {
        trail_bloom_passes,
        trail_count: 2,
        segment_count,
        emitter_counts,
        seed_sha256,
        projection_key_sha256: projection_key_sha256.to_owned(),
        current_frame_index,
        current_sample_time_ticks,
        projection_input_sha256: projection_input_sha256.to_owned(),
        projection_sample_set_sha256,
        emitter_binding_sha256,
        trail_inventory_sha256,
        trail_inventory,
        build_cohort_sha256: result.build_cohort_sha256,
        render_profile: forgecad_worker_protocol::render_profile(),
        trail_bloom_profile,
    })
}

pub(crate) fn render_typed_trails_with_worker_identity(
    glb: &[u8],
    camera: &Value,
    trails: &[TypedTrail],
    seed_sha256: &str,
) -> Result<RenderWorkerVfxTrailsFrame, GeometryWorkerError> {
    if glb.is_empty()
        || glb.len() > 64 * 1024 * 1024
        || trails.is_empty()
        || trails.len() > 16
        || seed_sha256.len() != 64
    {
        return Err(GeometryWorkerError::Protocol);
    }
    let encoded = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, glb);
    let trail_values = trails
        .iter()
        .map(|trail| {
            json!({
                "emitter_id":trail.emitter_id,
                "id":trail.id,
                "points":trail.points,
                "radius_px":trail.radius_px,
                "color_linear_rgb":trail.color_linear_rgb,
                "alpha":trail.alpha,
                "lifetime_ticks":trail.lifetime_ticks
            })
        })
        .collect::<Vec<_>>();
    let result = execute_render_worker_with_metadata(
        "render_typed_trails",
        json!({
            "glb_base64":encoded,
            "camera":camera,
            "trails":trail_values,
            "seed_sha256":seed_sha256
        }),
    )?;
    let object = strict_object(&result.result)?;
    require_exact_keys(
        object,
        &[
            "schema_version",
            "width",
            "height",
            "renderer_revision",
            "render_profile",
            "seed_sha256",
            "trail_count",
            "segment_count",
            "emitter_counts",
            "trail_passes",
        ],
    )?;
    if object.get("schema_version").and_then(Value::as_str)
        != Some("RenderWorkerVfxTrailsFrameResult@1")
        || object.get("width").and_then(Value::as_u64) != Some(512)
        || object.get("height").and_then(Value::as_u64) != Some(512)
        || object.get("renderer_revision").and_then(Value::as_str) != Some("forgecad-renderer-2")
        || object.get("render_profile") != Some(&forgecad_worker_protocol::render_profile())
        || object.get("seed_sha256").and_then(Value::as_str) != Some(seed_sha256)
        || object.get("trail_count").and_then(Value::as_u64) != Some(trails.len() as u64)
        || object.get("segment_count").and_then(Value::as_u64)
            != Some(
                trails
                    .iter()
                    .map(|trail| trail.points.len().saturating_sub(1))
                    .sum::<usize>() as u64,
            )
    {
        return Err(GeometryWorkerError::Protocol);
    }
    let counts = strict_object(
        object
            .get("emitter_counts")
            .ok_or(GeometryWorkerError::Protocol)?,
    )?;
    require_exact_keys(counts, &["muzzle-trail", "energy-core-trail"])?;
    let emitter_counts = [
        counts
            .get("muzzle-trail")
            .and_then(Value::as_u64)
            .map(|value| value as usize)
            .ok_or(GeometryWorkerError::Protocol)?,
        counts
            .get("energy-core-trail")
            .and_then(Value::as_u64)
            .map(|value| value as usize)
            .ok_or(GeometryWorkerError::Protocol)?,
    ];
    let expected_passes = ["trail-color", "trail-id", "trail-depth"];
    let values = object
        .get("trail_passes")
        .and_then(Value::as_array)
        .filter(|values| values.len() == expected_passes.len())
        .ok_or(GeometryWorkerError::Protocol)?;
    let mut trail_passes = Vec::with_capacity(values.len());
    for (value, expected_name) in values.iter().zip(expected_passes) {
        let pass = strict_object(value)?;
        require_exact_keys(pass, &["pass", "mime", "width", "height", "png_base64"])?;
        if pass.get("pass").and_then(Value::as_str) != Some(expected_name)
            || pass.get("mime").and_then(Value::as_str) != Some("image/png")
            || pass.get("width").and_then(Value::as_u64) != Some(512)
            || pass.get("height").and_then(Value::as_u64) != Some(512)
        {
            return Err(GeometryWorkerError::Protocol);
        }
        trail_passes.push(RenderPass {
            pass: expected_name.to_owned(),
            png: decode_png(pass.get("png_base64"), 512, 512)?,
            width: 512,
            height: 512,
        });
    }
    Ok(RenderWorkerVfxTrailsFrame {
        trail_passes,
        trail_count: trails.len(),
        segment_count: trails
            .iter()
            .map(|trail| trail.points.len().saturating_sub(1))
            .sum(),
        emitter_counts,
        seed_sha256: seed_sha256.to_owned(),
        build_cohort_sha256: result.build_cohort_sha256,
        render_profile: forgecad_worker_protocol::render_profile(),
    })
}

pub(crate) fn render_typed_trails_bloom_with_worker_identity(
    glb: &[u8],
    camera: &Value,
    trails: &[TypedTrail],
    seed_sha256: &str,
    trail_bloom_profile: TypedTrailBloomProfile,
) -> Result<RenderWorkerVfxTrailsBloomFrame, GeometryWorkerError> {
    let trail_bloom_profile = trail_bloom_profile.validate_fixed()?;
    if glb.is_empty()
        || glb.len() > 64 * 1024 * 1024
        || trails.is_empty()
        || trails.len() > 16
        || seed_sha256.len() != 64
    {
        return Err(GeometryWorkerError::Protocol);
    }
    let encoded = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, glb);
    let trail_values = trails
        .iter()
        .map(|trail| {
            json!({
                "emitter_id":trail.emitter_id,
                "id":trail.id,
                "points":trail.points,
                "radius_px":trail.radius_px,
                "color_linear_rgb":trail.color_linear_rgb,
                "alpha":trail.alpha,
                "lifetime_ticks":trail.lifetime_ticks
            })
        })
        .collect::<Vec<_>>();
    let result = execute_render_worker_with_metadata(
        "render_typed_trails_bloom",
        json!({
            "glb_base64":encoded,
            "camera":camera,
            "trails":trail_values,
            "trail_bloom_profile":{
                "threshold":trail_bloom_profile.threshold,
                "radius_px":trail_bloom_profile.radius_px,
                "intensity":trail_bloom_profile.intensity,
                "hdr_clamp":trail_bloom_profile.hdr_clamp,
                "source_gain":trail_bloom_profile.source_gain
            },
            "seed_sha256":seed_sha256
        }),
    )?;
    let object = strict_object(&result.result)?;
    require_exact_keys(
        object,
        &[
            "schema_version",
            "width",
            "height",
            "renderer_revision",
            "render_profile",
            "trail_bloom_profile",
            "seed_sha256",
            "trail_count",
            "segment_count",
            "emitter_counts",
            "trail_bloom_passes",
        ],
    )?;
    if object.get("schema_version").and_then(Value::as_str)
        != Some("RenderWorkerVfxTrailsBloomFrameResult@1")
        || object.get("width").and_then(Value::as_u64) != Some(512)
        || object.get("height").and_then(Value::as_u64) != Some(512)
        || object.get("renderer_revision").and_then(Value::as_str) != Some("forgecad-renderer-2")
        || object.get("render_profile") != Some(&forgecad_worker_protocol::render_profile())
        || object.get("seed_sha256").and_then(Value::as_str) != Some(seed_sha256)
        || object.get("trail_count").and_then(Value::as_u64) != Some(trails.len() as u64)
        || object.get("segment_count").and_then(Value::as_u64)
            != Some(
                trails
                    .iter()
                    .map(|trail| trail.points.len().saturating_sub(1))
                    .sum::<usize>() as u64,
            )
    {
        return Err(GeometryWorkerError::Protocol);
    }
    let returned_profile = strict_object(
        object
            .get("trail_bloom_profile")
            .ok_or(GeometryWorkerError::Protocol)?,
    )?;
    require_exact_keys(
        returned_profile,
        &[
            "threshold",
            "radius_px",
            "intensity",
            "hdr_clamp",
            "source_gain",
            "blur_passes",
        ],
    )?;
    if returned_profile
        .get("threshold")
        .and_then(Value::as_f64)
        .map(|value| value as f32)
        != Some(trail_bloom_profile.threshold)
        || returned_profile.get("radius_px").and_then(Value::as_u64)
            != Some(u64::from(trail_bloom_profile.radius_px))
        || returned_profile
            .get("intensity")
            .and_then(Value::as_f64)
            .map(|value| value as f32)
            != Some(trail_bloom_profile.intensity)
        || returned_profile
            .get("hdr_clamp")
            .and_then(Value::as_f64)
            .map(|value| value as f32)
            != Some(trail_bloom_profile.hdr_clamp)
        || returned_profile
            .get("source_gain")
            .and_then(Value::as_f64)
            .map(|value| value as f32)
            != Some(trail_bloom_profile.source_gain)
        || returned_profile.get("blur_passes").and_then(Value::as_u64) != Some(2)
    {
        return Err(GeometryWorkerError::Protocol);
    }
    let counts = strict_object(
        object
            .get("emitter_counts")
            .ok_or(GeometryWorkerError::Protocol)?,
    )?;
    require_exact_keys(counts, &["muzzle-trail", "energy-core-trail"])?;
    let expected_emitter_counts = [
        trails
            .iter()
            .filter(|trail| trail.emitter_id == "muzzle-trail")
            .count(),
        trails
            .iter()
            .filter(|trail| trail.emitter_id == "energy-core-trail")
            .count(),
    ];
    let emitter_counts = [
        counts
            .get("muzzle-trail")
            .and_then(Value::as_u64)
            .map(|value| value as usize)
            .ok_or(GeometryWorkerError::Protocol)?,
        counts
            .get("energy-core-trail")
            .and_then(Value::as_u64)
            .map(|value| value as usize)
            .ok_or(GeometryWorkerError::Protocol)?,
    ];
    if emitter_counts != expected_emitter_counts {
        return Err(GeometryWorkerError::Protocol);
    }
    let expected_passes = [
        "trail-color",
        "trail-id",
        "trail-depth",
        "trail-emissive-source",
        "trail-bloom-contribution",
    ];
    let values = object
        .get("trail_bloom_passes")
        .and_then(Value::as_array)
        .filter(|values| values.len() == expected_passes.len())
        .ok_or(GeometryWorkerError::Protocol)?;
    let mut passes = Vec::with_capacity(values.len());
    for (value, expected_name) in values.iter().zip(expected_passes) {
        let pass = strict_object(value)?;
        require_exact_keys(pass, &["pass", "mime", "width", "height", "png_base64"])?;
        if pass.get("pass").and_then(Value::as_str) != Some(expected_name)
            || pass.get("mime").and_then(Value::as_str) != Some("image/png")
            || pass.get("width").and_then(Value::as_u64) != Some(512)
            || pass.get("height").and_then(Value::as_u64) != Some(512)
        {
            return Err(GeometryWorkerError::Protocol);
        }
        passes.push(RenderPass {
            pass: expected_name.to_owned(),
            png: decode_png(pass.get("png_base64"), 512, 512)?,
            width: 512,
            height: 512,
        });
    }
    Ok(RenderWorkerVfxTrailsBloomFrame {
        trail_bloom_passes: passes,
        trail_count: trails.len(),
        segment_count: trails
            .iter()
            .map(|trail| trail.points.len().saturating_sub(1))
            .sum(),
        emitter_counts,
        seed_sha256: seed_sha256.to_owned(),
        build_cohort_sha256: result.build_cohort_sha256,
        render_profile: forgecad_worker_protocol::render_profile(),
        trail_bloom_profile,
    })
}

pub(crate) fn render_glb_fit_batch_at_resolution(
    glb: &[u8],
    cameras: &[Value],
    resolution: u32,
) -> Result<Vec<Vec<RenderPass>>, GeometryWorkerError> {
    if glb.is_empty() || glb.len() > 64 * 1024 * 1024 || cameras.is_empty() || cameras.len() > 64 {
        return Err(GeometryWorkerError::Protocol);
    }
    if !matches!(resolution, 128 | 256 | 512) {
        return Err(GeometryWorkerError::Protocol);
    }
    #[cfg(feature = "test-render-worker-fallback")]
    let fallback = || {
        cameras
            .iter()
            .map(|camera| {
                forgecad_render_core::render_perspective_glb_fit_at_resolution(
                    glb, camera, resolution,
                )
                .map(|passes| {
                    passes
                        .into_iter()
                        .map(|pass| RenderPass {
                            pass: pass.pass,
                            png: pass.png,
                            width: pass.width,
                            height: pass.height,
                        })
                        .collect()
                })
                .map_err(|_| GeometryWorkerError::Rejected)
            })
            .collect::<Result<Vec<_>, _>>()
    };
    let encoded = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, glb);
    let result = match execute_render_worker(
        "render_glb_fit_batch",
        json!({"glb_base64":encoded,"cameras":cameras,"resolution":resolution}),
    ) {
        Ok(result) => result,
        #[cfg(feature = "test-render-worker-fallback")]
        Err(GeometryWorkerError::Unavailable) => return fallback(),
        Err(error) => return Err(error),
    };
    let object = strict_object(&result)?;
    require_exact_keys(
        object,
        &[
            "schema_version",
            "width",
            "height",
            "renderer_revision",
            "renders",
        ],
    )?;
    if object.get("schema_version").and_then(Value::as_str) != Some("RenderWorkerFitBatchResult@1")
        || object.get("width").and_then(Value::as_u64) != Some(resolution as u64)
        || object.get("height").and_then(Value::as_u64) != Some(resolution as u64)
        || object.get("renderer_revision").and_then(Value::as_str) != Some("forgecad-renderer-2")
    {
        return Err(GeometryWorkerError::Protocol);
    }
    let renders = object
        .get("renders")
        .and_then(Value::as_array)
        .filter(|values| values.len() == cameras.len() && values.len() <= 64)
        .ok_or(GeometryWorkerError::Protocol)?;
    let expected = ["silhouette", "part-id"];
    let mut output = Vec::with_capacity(renders.len());
    for (index, render) in renders.iter().enumerate() {
        let render = strict_object(render)?;
        require_exact_keys(render, &["index", "passes"])?;
        if render.get("index").and_then(Value::as_u64) != Some(index as u64) {
            return Err(GeometryWorkerError::Protocol);
        }
        let values = render
            .get("passes")
            .and_then(Value::as_array)
            .filter(|values| values.len() == expected.len())
            .ok_or(GeometryWorkerError::Protocol)?;
        let mut passes = Vec::with_capacity(expected.len());
        for (value, expected_name) in values.iter().zip(expected) {
            let pass = strict_object(value)?;
            require_exact_keys(pass, &["pass", "mime", "width", "height", "png_base64"])?;
            if pass.get("pass").and_then(Value::as_str) != Some(expected_name)
                || pass.get("mime").and_then(Value::as_str) != Some("image/png")
                || pass.get("width").and_then(Value::as_u64) != Some(resolution as u64)
                || pass.get("height").and_then(Value::as_u64) != Some(resolution as u64)
            {
                return Err(GeometryWorkerError::Protocol);
            }
            passes.push(RenderPass {
                pass: expected_name.to_owned(),
                png: decode_png(pass.get("png_base64"), resolution, resolution)?,
                width: resolution,
                height: resolution,
            });
        }
        output.push(passes);
    }
    Ok(output)
}

fn strict_object(value: &Value) -> Result<&serde_json::Map<String, Value>, GeometryWorkerError> {
    value.as_object().ok_or(GeometryWorkerError::Protocol)
}

fn require_exact_keys(
    value: &serde_json::Map<String, Value>,
    allowed: &[&str],
) -> Result<(), GeometryWorkerError> {
    if value.len() != allowed.len() || allowed.iter().any(|key| !value.contains_key(*key)) {
        return Err(GeometryWorkerError::Protocol);
    }
    Ok(())
}

fn decode_png(
    value: Option<&Value>,
    expected_width: u32,
    expected_height: u32,
) -> Result<Vec<u8>, GeometryWorkerError> {
    let encoded = value
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or(GeometryWorkerError::Protocol)?;
    let png = base64::Engine::decode(
        &base64::engine::general_purpose::STANDARD,
        encoded.as_bytes(),
    )
    .map_err(|_| GeometryWorkerError::Protocol)?;
    validate_png_rgba8_bytes(&png, expected_width, expected_height)?;
    Ok(png)
}

pub(crate) fn validate_png_rgba8_bytes(
    png: &[u8],
    expected_width: u32,
    expected_height: u32,
) -> Result<(), GeometryWorkerError> {
    decode_png_rgba8_pixels(png, expected_width, expected_height).map(|_| ())
}

pub(crate) fn decode_png_rgba8_pixels(
    png: &[u8],
    expected_width: u32,
    expected_height: u32,
) -> Result<Vec<u8>, GeometryWorkerError> {
    if png.is_empty() || png.len() > 16 * 1024 * 1024 || !png.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Err(GeometryWorkerError::Protocol);
    }
    let decoder = image::codecs::png::PngDecoder::new(Cursor::new(&png))
        .map_err(|_| GeometryWorkerError::Protocol)?;
    if decoder.dimensions() != (expected_width, expected_height)
        || decoder.color_type() != image::ColorType::Rgba8
        || decoder.total_bytes() != u64::from(expected_width) * u64::from(expected_height) * 4
        || decoder.total_bytes() > 64 * 1024 * 1024
    {
        return Err(GeometryWorkerError::Protocol);
    }
    let mut decoded = vec![0_u8; decoder.total_bytes() as usize];
    decoder
        .read_image(&mut decoded)
        .map_err(|_| GeometryWorkerError::Protocol)?;
    Ok(decoded)
}

#[cfg(test)]
mod png_validation_tests {
    use super::*;
    use image::ImageEncoder;

    fn encoded_png(color: image::ExtendedColorType) -> Value {
        let pixels = match color {
            image::ExtendedColorType::Rgba8 => vec![10, 20, 30, 255],
            image::ExtendedColorType::Rgb8 => vec![10, 20, 30],
            _ => unreachable!(),
        };
        let mut png = Vec::new();
        image::codecs::png::PngEncoder::new(&mut png)
            .write_image(&pixels, 1, 1, color)
            .expect("encode PNG fixture");
        Value::String(base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            png,
        ))
    }

    #[test]
    fn png_decode_requires_valid_exact_rgba8_dimensions() {
        let rgba = encoded_png(image::ExtendedColorType::Rgba8);
        assert!(decode_png(Some(&rgba), 1, 1).is_ok());
        assert!(decode_png(Some(&rgba), 2, 1).is_err());

        let rgb = encoded_png(image::ExtendedColorType::Rgb8);
        assert!(decode_png(Some(&rgb), 1, 1).is_err());

        let truncated = rgba
            .as_str()
            .and_then(|value| {
                base64::Engine::decode(&base64::engine::general_purpose::STANDARD, value).ok()
            })
            .map(|mut bytes| {
                bytes.truncate(bytes.len().saturating_sub(8));
                Value::String(base64::Engine::encode(
                    &base64::engine::general_purpose::STANDARD,
                    bytes,
                ))
            })
            .expect("truncate PNG fixture");
        assert!(decode_png(Some(&truncated), 1, 1).is_err());
    }

    #[test]
    fn animated_socket_particle_adapter_unwraps_only_the_typed_binding_object() {
        let binding = json!({
            "schema_version":"RenderWorkerAnimatedSocketEmitterBindings@1",
            "emitters":[{},{}]
        });
        assert_eq!(
            animated_socket_emitter_binding_values(&binding)
                .expect("typed binding object")
                .len(),
            2
        );
        assert!(animated_socket_emitter_binding_values(&json!([{}, {}])).is_err());
        assert!(animated_socket_emitter_binding_values(&json!({
            "schema_version":"RenderWorkerAnimatedSocketEmitterBindings@1",
            "emitters":[{},{}],
            "unexpected":true
        }))
        .is_err());
        assert!(animated_socket_emitter_binding_values(&json!({
            "schema_version":"FictionalEnergyVfxAnimatedSocketEmitterBindings@1",
            "emitters":[{},{}]
        }))
        .is_err());
    }

    #[test]
    fn animated_socket_trail_adapter_bounds_history_and_fixed_trail_count() {
        let samples = json!([{}, {}]);
        let trails = json!([{}, {}]);
        assert_eq!(
            validate_animated_socket_trail_adapter_inputs(
                &[1_u8],
                &"a".repeat(64),
                &"b".repeat(64),
                3,
                240,
                &samples,
                &trails,
                &"c".repeat(64),
            )
            .expect("bounded adapter input"),
            2
        );
        let mut too_short = samples.clone();
        too_short.as_array_mut().unwrap().pop();
        assert!(validate_animated_socket_trail_adapter_inputs(
            &[1_u8],
            &"a".repeat(64),
            &"b".repeat(64),
            3,
            240,
            &too_short,
            &trails,
            &"c".repeat(64),
        )
        .is_err());
        let wrong_trail_count = json!([{}, {}, {}]);
        assert!(validate_animated_socket_trail_adapter_inputs(
            &[1_u8],
            &"a".repeat(64),
            &"b".repeat(64),
            3,
            240,
            &samples,
            &wrong_trail_count,
            &"c".repeat(64),
        )
        .is_err());
        assert!(validate_animated_socket_trail_adapter_inputs(
            &[],
            &"a".repeat(64),
            &"b".repeat(64),
            3,
            240,
            &samples,
            &trails,
            &"c".repeat(64),
        )
        .is_err());
    }
}

// This gate deliberately requires a source-built same-cohort Render Worker
// sibling. The GLB fixture is compiled locally only to isolate the render
// boundary; the nine AOVs and their determinism must come from the real
// Render Worker process, not the in-process render-core fallback.
#[cfg(all(test, target_os = "macos"))]
mod isolated_tests {
    use super::*;
    use serde_json::json;

    fn v1_fixture_program() -> Value {
        let mut program = json!({
            "schema_version":"GeometryProgram@1",
            "project_id":"mcp010f-render-worker-isolation-fixture",
            "representation_plan_sha256":"a".repeat(64),
            "nodes":[
                {"node_id":"torso","operator_id":"forgecad.geometry.primitive@1","part_id":"torso","parameters":{"shape":"box","size":[1.2,1.6,0.55],"position":[0.0,1.7,0.0],"material_zone_id":"zone-white-shell"}},
                {"node_id":"core","operator_id":"forgecad.geometry.primitive@1","part_id":"core","parameters":{"shape":"cylinder","size":[0.55,1.2,0.55],"position":[0.0,1.5,0.0],"material_zone_id":"zone-black-mechanical","segments":16}},
                {"node_id":"head","operator_id":"forgecad.geometry.primitive@1","part_id":"head","parameters":{"shape":"sphere","size":[0.85,0.9,0.85],"position":[0.0,2.75,0.0],"material_zone_id":"zone-white-shell","segments":16}}
            ],
            "budgets":{"max_nodes":16,"max_triangles":20000,"max_runtime_ms":1000}
        });
        program["canonical_sha256"] = Value::String(crate::canonical_json_hash(&program));
        program
    }

    fn fixed_camera() -> Value {
        json!({
            "schema_version":"CameraCalibration@1",
            "projection":"perspective",
            "transform":{"position_m":[4.0,3.0,6.0],"target_m":[0.0,1.5,0.0],"up":[0.0,1.0,0.0]},
            "fov_y_degrees":42.0,
            "near_m":0.05,
            "far_m":20.0,
            "resolution":{"width":512,"height":512},
            "coordinate_system":"right-handed-y-up-meter"
        })
    }

    fn vfx_fixture_artifact() -> geometry_worker::GeometryArtifact {
        let mut geometry = json!({
            "schema_version":"GeometryProgram@2",
            "project_id":"mcp010f-vfx-frame-isolation-fixture",
            "representation_plan_sha256":"a".repeat(64),
            "operator_catalog_sha256":forgecad_worker_protocol::operator_catalog_sha256(),
            "units":{"length":"meter","angle":"radian","coordinate_system":"right-handed-y-up"},
            "budgets":{"max_nodes":2,"max_triangles":2000,"max_glb_bytes":67108864,"max_worker_memory_bytes":536870912,"max_runtime_ms":10000},
            "nodes":[
                {"node_id":"core-node","operator_id":"forgecad.geometry.primitive@2","inputs":[],"parameters":{"shape":"box","size_m":[1.2,0.5,0.5],"position_m":[0.0,1.5,0.0],"rotation_rad":[0.0,0.0,0.0]}},
                {"node_id":"body-node","operator_id":"forgecad.geometry.primitive@2","inputs":[],"parameters":{"shape":"box","size_m":[1.8,0.35,0.65],"position_m":[0.0,0.8,0.0],"rotation_rad":[0.0,0.0,0.0]}}
            ],
            "part_outputs":[
                {"part_id":"energy-core","input_node_ids":["core-node"],"material_zone_id":"zone-core-emissive","solid":true},
                {"part_id":"body","input_node_ids":["body-node"],"material_zone_id":"zone-body","solid":true}
            ]
        });
        geometry["canonical_sha256"] = Value::String(crate::canonical_json_hash(&geometry));
        let mut appearance = json!({
            "schema_version":"AppearanceProgram@2",
            "project_id":"mcp010f-vfx-frame-isolation-fixture",
            "geometry_program_sha256":geometry["canonical_sha256"],
            "material_pack_id":"forgecad-fictional-energy-weapon-2k",
            "material_pack_manifest_sha256":forgecad_worker_protocol::material_pack_manifest_sha256_by_id("forgecad-fictional-energy-weapon-2k").expect("2K pack"),
            "material_zones":[
                {"zone_id":"zone-core-emissive","part_ids":["energy-core"],"material_id":"energy-cyan-emissive","texture_set_id":null},
                {"zone_id":"zone-body","part_ids":["body"],"material_id":"energy-dark-painted-metal","texture_set_id":"weapon-metal-surface"}
            ]
        });
        appearance["canonical_sha256"] = Value::String(crate::canonical_json_hash(&appearance));
        geometry_worker::compile_geometry_test_fallback(&geometry, Some(&appearance))
            .expect("VFX fixture appearance GLB")
    }

    #[test]
    #[ignore = "requires source-built same-cohort forgecad-render-worker sibling"]
    fn isolated_render_worker_returns_deterministic_nine_aov_set() {
        let artifact = geometry_worker::compile_geometry_test_fallback(&v1_fixture_program(), None)
            .expect("fixture GLB");
        let camera = fixed_camera();
        let first = render_glb(&artifact.glb, &camera).expect("isolated render");
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
        assert!(first.iter().all(|pass| {
            pass.width == 512 && pass.height == 512 && pass.png.starts_with(b"\x89PNG\r\n\x1a\n")
        }));
        let second = render_glb(&artifact.glb, &camera).expect("repeat isolated render");
        assert_eq!(
            first.iter().map(|pass| &pass.png).collect::<Vec<_>>(),
            second.iter().map(|pass| &pass.png).collect::<Vec<_>>()
        );
    }

    #[test]
    #[ignore = "requires source-built same-cohort forgecad-render-worker sibling"]
    fn isolated_render_worker_rejects_geometry_program_payload() {
        let error = execute_render_worker(
            "render_glb",
            json!({"geometry_program":v1_fixture_program()}),
        )
        .expect_err("Render Worker must reject compiler payloads");
        assert!(matches!(
            error,
            GeometryWorkerError::RejectedWithDetails { .. }
        ));
    }

    #[test]
    #[ignore = "requires source-built same-cohort forgecad-render-worker sibling"]
    fn isolated_vfx_frame_override_is_exact_targeted_and_deterministic() {
        let artifact = vfx_fixture_artifact();
        let camera = fixed_camera();
        let off = EmissiveMaterialOverride {
            material_zone_id: "zone-core-emissive".to_owned(),
            material_id: "energy-cyan-emissive".to_owned(),
            color_linear_rgb: [0.0, 0.82, 1.0],
            emissive_strength: 0.0,
        };
        let on = EmissiveMaterialOverride {
            emissive_strength: 8.0,
            ..off.clone()
        };
        let off_render = render_glb_vfx_frame_with_worker_identity(&artifact.glb, &camera, &[off])
            .expect("off frame");
        let on_render =
            render_glb_vfx_frame_with_worker_identity(&artifact.glb, &camera, &[on.clone()])
                .expect("on frame");
        let replay = render_glb_vfx_frame_with_worker_identity(&artifact.glb, &camera, &[on])
            .expect("on frame replay");
        assert_eq!(on_render.applied_emissive_overrides.len(), 1);
        assert_eq!(
            on_render.applied_emissive_overrides[0].material_zone_id,
            "zone-core-emissive"
        );
        assert_ne!(off_render.passes[0].png, on_render.passes[0].png);
        assert_eq!(
            on_render
                .passes
                .iter()
                .skip(1)
                .map(|pass| &pass.png)
                .collect::<Vec<_>>(),
            off_render
                .passes
                .iter()
                .skip(1)
                .map(|pass| &pass.png)
                .collect::<Vec<_>>()
        );
        assert_eq!(
            on_render
                .passes
                .iter()
                .map(|pass| &pass.png)
                .collect::<Vec<_>>(),
            replay
                .passes
                .iter()
                .map(|pass| &pass.png)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    #[ignore = "requires source-built same-cohort forgecad-render-worker sibling"]
    fn isolated_hdr_bloom_is_independent_bounded_and_deterministic() {
        let artifact = vfx_fixture_artifact();
        let camera = fixed_camera();
        let emissive = EmissiveMaterialOverride {
            material_zone_id: "zone-core-emissive".to_owned(),
            material_id: "energy-cyan-emissive".to_owned(),
            color_linear_rgb: [0.0, 0.82, 1.0],
            emissive_strength: 8.0,
        };
        let profile = HdrBloomProfile {
            threshold: 1.0,
            radius_px: 6,
            intensity: 1.5,
            hdr_clamp: 16.0,
        };
        let base = render_glb_vfx_frame_with_worker_identity(
            &artifact.glb,
            &camera,
            std::slice::from_ref(&emissive),
        )
        .expect("base emissive frame");
        let bloom = render_glb_vfx_bloom_frame_with_worker_identity(
            &artifact.glb,
            &camera,
            std::slice::from_ref(&emissive),
            profile,
        )
        .expect("HDR bloom frame");
        let replay = render_glb_vfx_bloom_frame_with_worker_identity(
            &artifact.glb,
            &camera,
            &[emissive],
            profile,
        )
        .expect("HDR bloom replay");

        let base_after = render_glb_vfx_frame_with_worker_identity(
            &artifact.glb,
            &camera,
            &[EmissiveMaterialOverride {
                material_zone_id: "zone-core-emissive".to_owned(),
                material_id: "energy-cyan-emissive".to_owned(),
                color_linear_rgb: [0.0, 0.82, 1.0],
                emissive_strength: 8.0,
            }],
        )
        .expect("base emissive frame after bloom");
        assert_eq!(bloom.build_cohort_sha256, base.build_cohort_sha256);
        assert_eq!(bloom.bloom_passes.len(), 2);
        assert_eq!(
            base.passes.iter().map(|pass| &pass.png).collect::<Vec<_>>(),
            base_after
                .passes
                .iter()
                .map(|pass| &pass.png)
                .collect::<Vec<_>>()
        );
        assert_eq!(
            bloom
                .bloom_passes
                .iter()
                .map(|pass| &pass.png)
                .collect::<Vec<_>>(),
            replay
                .bloom_passes
                .iter()
                .map(|pass| &pass.png)
                .collect::<Vec<_>>()
        );
        let contribution = decode_png_rgba8_pixels(&bloom.bloom_passes[1].png, 512, 512)
            .expect("decode bloom contribution");
        assert!(contribution
            .chunks_exact(4)
            .any(|pixel| pixel[0] != 0 || pixel[1] != 0 || pixel[2] != 0));

        let invalid = HdrBloomProfile {
            radius_px: 9,
            ..profile
        };
        assert!(render_glb_vfx_bloom_frame_with_worker_identity(
            &artifact.glb,
            &camera,
            &replay
                .applied_emissive_overrides
                .iter()
                .map(|actual| EmissiveMaterialOverride {
                    material_zone_id: actual.material_zone_id.clone(),
                    material_id: actual.material_id.clone(),
                    color_linear_rgb: [0.0, 0.82, 1.0],
                    emissive_strength: 8.0,
                })
                .collect::<Vec<_>>(),
            invalid,
        )
        .is_err());
    }

    #[test]
    #[ignore = "requires source-built same-cohort forgecad-render-worker sibling"]
    fn isolated_typed_particle_worker_is_same_cohort_byte_exact() {
        let artifact = vfx_fixture_artifact();
        let camera = fixed_camera();
        let camera_position = [4.0_f32, 3.0, 6.0];
        let target = [0.0_f32, 1.5, 0.0];
        let delta = [
            target[0] - camera_position[0],
            target[1] - camera_position[1],
            target[2] - camera_position[2],
        ];
        let z = (delta[0] * delta[0] + delta[1] * delta[1] + delta[2] * delta[2]).sqrt();
        let depth = (z - 0.05) / (20.0 - 0.05);
        let particles = vec![
            TypedParticle {
                emitter_id: "muzzle-burst".to_owned(),
                id: 10_000,
                position: [0.0, 1.5, 0.8],
                radius_px: 4.0,
                color_linear_rgb: [0.0, 0.82, 1.0],
                alpha: 0.8,
                lifetime_ticks: 120,
                depth: {
                    let relative = [
                        -camera_position[0],
                        1.5 - camera_position[1],
                        0.8 - camera_position[2],
                    ];
                    let forward = [delta[0] / z, delta[1] / z, delta[2] / z];
                    let projected = relative[0] * forward[0]
                        + relative[1] * forward[1]
                        + relative[2] * forward[2];
                    (projected - 0.05) / (20.0 - 0.05)
                },
            },
            TypedParticle {
                emitter_id: "energy-core-sparks".to_owned(),
                id: 20_000,
                position: target,
                radius_px: 3.0,
                color_linear_rgb: [1.0, 0.4, 0.05],
                alpha: 0.7,
                lifetime_ticks: 160,
                depth,
            },
        ];
        let seed = "7".repeat(64);
        let first =
            render_typed_particles_with_worker_identity(&artifact.glb, &camera, &particles, &seed)
                .expect("typed particle frame");
        let second =
            render_typed_particles_with_worker_identity(&artifact.glb, &camera, &particles, &seed)
                .expect("typed particle replay");
        assert!(first.build_cohort_sha256.is_some());
        assert_eq!(first.build_cohort_sha256, second.build_cohort_sha256);
        assert_eq!(first.particle_count, 2);
        assert_eq!(first.emitter_counts, [1, 1]);
        assert_eq!(
            first
                .particle_passes
                .iter()
                .map(|pass| (pass.pass.as_str(), &pass.png))
                .collect::<Vec<_>>(),
            second
                .particle_passes
                .iter()
                .map(|pass| (pass.pass.as_str(), &pass.png))
                .collect::<Vec<_>>()
        );
    }

    #[test]
    #[ignore = "requires source-built same-cohort forgecad-render-worker sibling"]
    fn isolated_typed_trail_worker_is_same_cohort_byte_exact() {
        let artifact = vfx_fixture_artifact();
        let camera = fixed_camera();
        let trails = vec![
            TypedTrail {
                emitter_id: "muzzle-trail".to_owned(),
                id: 30_000,
                points: vec![[0.0, 1.5, 0.8], [0.03, 1.5, 0.8]],
                radius_px: 3.0,
                color_linear_rgb: [0.0, 0.82, 1.0],
                alpha: 0.75,
                lifetime_ticks: 120,
            },
            TypedTrail {
                emitter_id: "energy-core-trail".to_owned(),
                id: 31_000,
                points: vec![[0.0, 1.5, 0.0], [0.02, 1.5, 0.0]],
                radius_px: 2.5,
                color_linear_rgb: [1.0, 0.4, 0.05],
                alpha: 0.7,
                lifetime_ticks: 160,
            },
        ];
        let seed = "8".repeat(64);
        let first =
            render_typed_trails_with_worker_identity(&artifact.glb, &camera, &trails, &seed)
                .expect("typed trail frame");
        let second =
            render_typed_trails_with_worker_identity(&artifact.glb, &camera, &trails, &seed)
                .expect("typed trail replay");
        assert!(first.build_cohort_sha256.is_some());
        assert_eq!(first.build_cohort_sha256, second.build_cohort_sha256);
        assert_eq!(first.trail_count, 2);
        assert_eq!(first.segment_count, 2);
        assert_eq!(first.emitter_counts, [1, 1]);
        assert_eq!(first.seed_sha256, seed);
        assert_eq!(
            first
                .trail_passes
                .iter()
                .map(|pass| (pass.pass.as_str(), &pass.png))
                .collect::<Vec<_>>(),
            second
                .trail_passes
                .iter()
                .map(|pass| (pass.pass.as_str(), &pass.png))
                .collect::<Vec<_>>()
        );
    }

    #[test]
    #[ignore = "requires source-built same-cohort forgecad-render-worker sibling"]
    fn isolated_typed_trail_bloom_worker_preserves_legacy_aovs_and_is_byte_exact() {
        let artifact = vfx_fixture_artifact();
        let camera = fixed_camera();
        let trails = vec![
            TypedTrail {
                emitter_id: "muzzle-trail".to_owned(),
                id: 30_000,
                points: vec![[0.0, 1.5, 0.8], [0.03, 1.5, 0.8]],
                radius_px: 3.0,
                color_linear_rgb: [0.0, 0.82, 1.0],
                alpha: 0.75,
                lifetime_ticks: 120,
            },
            TypedTrail {
                emitter_id: "energy-core-trail".to_owned(),
                id: 31_000,
                points: vec![[0.0, 1.5, 0.0], [0.02, 1.5, 0.0]],
                radius_px: 2.5,
                color_linear_rgb: [1.0, 0.4, 0.05],
                alpha: 0.7,
                lifetime_ticks: 160,
            },
        ];
        let seed = "9".repeat(64);
        let legacy =
            render_typed_trails_with_worker_identity(&artifact.glb, &camera, &trails, &seed)
                .expect("legacy typed trail frame");
        let first = render_typed_trails_bloom_with_worker_identity(
            &artifact.glb,
            &camera,
            &trails,
            &seed,
            TypedTrailBloomProfile::FIXED,
        )
        .expect("typed trail Bloom frame");
        let second = render_typed_trails_bloom_with_worker_identity(
            &artifact.glb,
            &camera,
            &trails,
            &seed,
            TypedTrailBloomProfile::FIXED,
        )
        .expect("typed trail Bloom replay");
        assert_eq!(first.trail_bloom_passes.len(), 5);
        assert_eq!(first.trail_count, 2);
        assert_eq!(first.segment_count, 2);
        assert_eq!(first.emitter_counts, [1, 1]);
        assert_eq!(first.seed_sha256, seed);
        assert_eq!(first.trail_bloom_profile, TypedTrailBloomProfile::FIXED);
        assert!(first.build_cohort_sha256.is_some());
        assert_eq!(first.build_cohort_sha256, second.build_cohort_sha256);
        assert_eq!(
            first
                .trail_bloom_passes
                .iter()
                .map(|pass| (pass.pass.as_str(), &pass.png))
                .collect::<Vec<_>>(),
            second
                .trail_bloom_passes
                .iter()
                .map(|pass| (pass.pass.as_str(), &pass.png))
                .collect::<Vec<_>>()
        );
        assert_eq!(
            legacy
                .trail_passes
                .iter()
                .map(|pass| &pass.png)
                .collect::<Vec<_>>(),
            first
                .trail_bloom_passes
                .iter()
                .take(3)
                .map(|pass| &pass.png)
                .collect::<Vec<_>>()
        );
    }
}
