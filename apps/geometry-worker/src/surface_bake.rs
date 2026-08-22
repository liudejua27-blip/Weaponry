use super::{encode_rgb8_png, normalize, pack_texture_bytes, GeometryError, PartMesh};
use image::RgbImage;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::VecDeque;
use std::collections::{BTreeMap, BTreeSet};

pub(super) const NORMAL_ID: &str = "forgecad_candidate_surface_normal";
pub(super) const AO_ID: &str = "forgecad_candidate_surface_ao";
pub(super) const BASE_COLOR_ID: &str = "forgecad_candidate_layered_base_color";
pub(super) const METALLIC_ROUGHNESS_ID: &str = "forgecad_candidate_layered_metallic_roughness";
pub(super) const CLEARCOAT_ID: &str = "forgecad_candidate_zone_clearcoat";
pub(super) const CLEARCOAT_ROUGHNESS_ID: &str = "forgecad_candidate_zone_clearcoat_roughness";

const SIZE: usize = 2048;
const PADDING: usize = 8;
const MAX_DISTANCE_RATIO: f32 = 0.18;

#[derive(Debug, Clone)]
pub(super) struct SurfaceBakeOutput {
    pub texture_id: String,
    pub bytes: Vec<u8>,
    pub semantic: &'static str,
    pub color_space: &'static str,
    pub normal_convention: Option<&'static str>,
}

#[derive(Debug, Clone)]
pub(super) struct SurfaceBake {
    pub outputs: Vec<SurfaceBakeOutput>,
    pub metadata: Value,
    pub clearcoat_zone_ids: BTreeSet<String>,
}

#[derive(Clone)]
struct Triangle {
    position: [[f32; 3]; 3],
    normal: [[f32; 3]; 3],
    tangent: [[f32; 4]; 3],
    uv: [[f32; 2]; 3],
    chart: u32,
    part_id: String,
    zone_id: String,
    base_key: Option<String>,
    normal_key: Option<String>,
    mr_key: Option<String>,
    vertex_ao: [f32; 3],
}

#[derive(Default)]
struct TextureSources {
    rgb: BTreeMap<String, RgbImage>,
}

pub(super) fn build(parts: &[PartMesh], stack: &Value) -> Result<SurfaceBake, GeometryError> {
    let stack = stack.as_object().ok_or_else(|| {
        GeometryError::Invalid("surface bake requires a MaterialLayerStack@1 object".to_owned())
    })?;
    let stack_hash = stack
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| GeometryError::Invalid("surface bake stack hash is missing".to_owned()))?;
    let layers = stack
        .get("layers")
        .and_then(Value::as_array)
        .ok_or_else(|| GeometryError::Invalid("surface bake layers are missing".to_owned()))?;
    let decal_targets = layer_targets(layers, "decal")?;
    let wear_targets = layer_targets(layers, "wear")?;
    let clearcoat_targets = layer_targets(layers, "clearcoat")?;
    let clearcoat_layer = layers
        .iter()
        .find(|layer| layer.get("kind").and_then(Value::as_str) == Some("clearcoat"))
        .ok_or_else(|| {
            GeometryError::Invalid("surface bake clearcoat layer is missing".to_owned())
        })?;
    let clearcoat_factor = clearcoat_layer
        .get("factor")
        .and_then(Value::as_f64)
        .unwrap_or(1.0)
        .clamp(0.0, 1.0) as f32;
    let clearcoat_roughness = clearcoat_layer
        .get("roughness")
        .and_then(Value::as_f64)
        .unwrap_or(0.12)
        .clamp(0.0, 1.0) as f32;

    let mut triangles = flatten_triangles(parts)?;
    let (bounds_min, bounds_max) = triangle_bounds(&triangles);
    let diagonal = length(sub(bounds_max, bounds_min)).max(1.0e-4);
    let all_positions = triangles
        .iter()
        .map(|triangle| triangle.position)
        .collect::<Vec<_>>();
    for triangle_index in 0..triangles.len() {
        for vertex in 0..3 {
            triangles[triangle_index].vertex_ao[vertex] = vertex_ao(
                triangles[triangle_index].position[vertex],
                triangles[triangle_index].normal[vertex],
                triangle_index,
                &all_positions,
                diagonal * MAX_DISTANCE_RATIO,
            );
        }
    }

    let mut sources = TextureSources::default();
    for key in triangles.iter().flat_map(|triangle| {
        [
            triangle.base_key.as_deref(),
            triangle.normal_key.as_deref(),
            triangle.mr_key.as_deref(),
        ]
        .into_iter()
        .flatten()
    }) {
        if sources.rgb.contains_key(key) {
            continue;
        }
        let bytes = pack_texture_bytes("forgecad-fictional-energy-weapon-2k", key)?;
        let decoded = image::load_from_memory(&bytes).map_err(|error| {
            GeometryError::Invalid(format!("surface bake source decode failed: {key}: {error}"))
        })?;
        sources.rgb.insert(key.to_owned(), decoded.to_rgb8());
    }

    let pixel_count = SIZE * SIZE;
    let mut base = vec![255u8; pixel_count * 3];
    let mut normal = vec![128u8; pixel_count * 3];
    normal.chunks_exact_mut(3).for_each(|pixel| pixel[2] = 255);
    let mut mr = vec![255u8; pixel_count * 3];
    let mut ao = vec![255u8; pixel_count];
    let mut clearcoat = vec![0u8; pixel_count];
    let mut clearcoat_rough = vec![u8_from_unit(clearcoat_roughness); pixel_count];
    let mut owner = vec![u32::MAX; pixel_count];

    for triangle in &triangles {
        raster_triangle(triangle, |x, y, bary| {
            let pixel = y * SIZE + x;
            if owner[pixel] != u32::MAX && owner[pixel] != triangle.chart {
                return;
            }
            owner[pixel] = triangle.chart;
            let uv = interpolate2(triangle.uv, bary);
            let base_sample =
                sample_rgb(&sources, triangle.base_key.as_deref(), uv, [255, 255, 255]);
            let normal_sample = sample_rgb(
                &sources,
                triangle.normal_key.as_deref(),
                uv,
                [128, 128, 255],
            );
            let mut mr_sample =
                sample_rgb(&sources, triangle.mr_key.as_deref(), uv, [255, 255, 255]);
            let surface_ao = triangle.vertex_ao[0] * bary[0]
                + triangle.vertex_ao[1] * bary[1]
                + triangle.vertex_ao[2] * bary[2];
            let face = normalize(cross(
                sub(triangle.position[1], triangle.position[0]),
                sub(triangle.position[2], triangle.position[0]),
            ));
            let interpolated_normal = normalize(interpolate3(triangle.normal, bary));
            let tangent4 = interpolate4(triangle.tangent, bary);
            let tangent = normalize([tangent4[0], tangent4[1], tangent4[2]]);
            let bitangent = normalize(scale(
                cross(interpolated_normal, tangent),
                tangent4[3].signum(),
            ));
            let geometric_tangent = normalize([
                dot(face, tangent),
                dot(face, bitangent),
                dot(face, interpolated_normal),
            ]);
            let source_tangent = normalize([
                unit_from_u8(normal_sample[0]),
                unit_from_u8(normal_sample[1]),
                unit_from_u8(normal_sample[2]),
            ]);
            let composed_normal = normalize([
                source_tangent[0] + geometric_tangent[0],
                source_tangent[1] + geometric_tangent[1],
                (source_tangent[2] * geometric_tangent[2]).max(0.02),
            ]);

            let decal =
                decal_targets.matches(&triangle.part_id, &triangle.zone_id) && fictional_decal(uv);
            let wear = if wear_targets.matches(&triangle.part_id, &triangle.zone_id) {
                ((1.0 - surface_ao) * 0.65 + (1.0 - dot(face, interpolated_normal).abs()) * 0.35)
                    .clamp(0.0, 0.35)
            } else {
                0.0
            };
            let mut layered_base = base_sample;
            if decal {
                layered_base = mix_rgb(layered_base, [245, 96, 20], 0.65);
            }
            layered_base = mix_rgb(layered_base, [185, 190, 196], wear);
            mr_sample[1] = u8_from_unit((f32::from(mr_sample[1]) / 255.0 + wear * 0.32).min(1.0));
            let offset = pixel * 3;
            base[offset..offset + 3].copy_from_slice(&layered_base);
            normal[offset..offset + 3].copy_from_slice(&[
                u8_from_signed(composed_normal[0]),
                u8_from_signed(composed_normal[1]),
                u8_from_signed(composed_normal[2]),
            ]);
            mr[offset..offset + 3].copy_from_slice(&mr_sample);
            ao[pixel] = u8_from_unit(surface_ao);
            if clearcoat_targets.matches(&triangle.part_id, &triangle.zone_id) {
                clearcoat[pixel] = u8_from_unit(clearcoat_factor);
                clearcoat_rough[pixel] = u8_from_unit(clearcoat_roughness);
            }
        });
    }

    dilate_padding(
        &mut owner,
        &mut base,
        &mut normal,
        &mut mr,
        &mut ao,
        &mut clearcoat,
        &mut clearcoat_rough,
    );

    let outputs = vec![
        output_rgb(BASE_COLOR_ID, &base, "layered-baseColor", "sRGB", None)?,
        output_rgb(
            NORMAL_ID,
            &normal,
            "candidate-surface-normal",
            "linear",
            Some("OpenGL+Y"),
        )?,
        output_rgb(
            METALLIC_ROUGHNESS_ID,
            &mr,
            "layered-metallicRoughness",
            "linear",
            None,
        )?,
        output_luma(AO_ID, &ao, "candidate-self-occlusion", "linear")?,
        output_luma(CLEARCOAT_ID, &clearcoat, "zone-clearcoat-factor", "linear")?,
        output_luma(
            CLEARCOAT_ROUGHNESS_ID,
            &clearcoat_rough,
            "zone-clearcoat-roughness",
            "linear",
        )?,
    ];
    let output_meta = outputs
        .iter()
        .map(|output| {
            json!({
                "texture_id":output.texture_id,
                "sha256":hex_sha256(&output.bytes),
                "size_bytes":output.bytes.len(),
                "width":SIZE,
                "height":SIZE,
                "mime":"image/png",
                "semantic":output.semantic,
                "color_space":output.color_space,
                "normal_convention":output.normal_convention,
            })
        })
        .collect::<Vec<_>>();
    let total_output_bytes = outputs
        .iter()
        .map(|output| output.bytes.len())
        .sum::<usize>();
    if outputs.len() > 8 || total_output_bytes > 67_108_864 {
        return Err(GeometryError::Invalid(
            "surface bake output budget exceeded".to_owned(),
        ));
    }
    let algorithm_hash = hex_sha256(b"candidate-uv-tbn-normal@1|fixed-8-ray-self-occlusion@1|fictional-safety-decal@1|geometry-normal-ao-wear@1|zone-clearcoat@1|2048|8px|no-rng-no-time-no-network");
    let mut metadata = json!({
        "schema_version":"CandidateSurfaceBake@1",
        "material_layer_stack_sha256":stack_hash,
        "algorithm":"candidate-uv-tbn-normal-plus-fixed-self-occlusion-layer-lowering@1",
        "worker_algorithm_sha256":algorithm_hash,
        "normal_bake_policy":"evaluated-candidate-surface-tangent-field-not-high-low-cage@1",
        "ao_bake_policy":"fixed-8-ray-candidate-self-occlusion-not-screen-space@1",
        "layer_lowering":["decal-to-baseColor","wear-to-baseColor-metallicRoughness","clearcoat-to-KHR_materials_clearcoat"],
        "resolution":SIZE,
        "padding_texels":PADDING,
        "embedded_only":true,
        "external_uri":false,
        "network_at_runtime":false,
        "outputs":output_meta,
        "total_output_bytes":total_output_bytes,
        "canonical_sha256":""
    });
    let mut preimage = metadata.as_object().expect("metadata object").clone();
    preimage.remove("canonical_sha256");
    metadata["canonical_sha256"] = Value::String(super::canonical_hash(&Value::Object(preimage)));
    Ok(SurfaceBake {
        outputs,
        metadata,
        clearcoat_zone_ids: clearcoat_targets.zones,
    })
}

#[derive(Default)]
struct Targets {
    parts: BTreeSet<String>,
    zones: BTreeSet<String>,
}

impl Targets {
    fn matches(&self, part: &str, zone: &str) -> bool {
        self.parts.contains(part) && self.zones.contains(zone)
    }
}

fn layer_targets(layers: &[Value], kind: &str) -> Result<Targets, GeometryError> {
    let layer = layers
        .iter()
        .find(|layer| layer.get("kind").and_then(Value::as_str) == Some(kind))
        .ok_or_else(|| GeometryError::Invalid(format!("surface bake {kind} layer is missing")))?;
    let targets = layer
        .get("targets")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            GeometryError::Invalid(format!("surface bake {kind} targets are missing"))
        })?;
    Ok(Targets {
        parts: targets
            .get("part_ids")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect(),
        zones: targets
            .get("material_zone_ids")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect(),
    })
}

fn flatten_triangles(parts: &[PartMesh]) -> Result<Vec<Triangle>, GeometryError> {
    let mut triangles = Vec::new();
    for part in parts {
        let keys = part.material["extras"]["forgecad"]["texture_keys"]
            .as_object()
            .cloned()
            .unwrap_or_default();
        for source in &part.sources {
            for (triangle_index, indices) in source.indices.chunks_exact(3).enumerate() {
                let mut position = [[0.0; 3]; 3];
                let mut normal = [[0.0; 3]; 3];
                let mut tangent = [[0.0; 4]; 3];
                let mut uv = [[0.0; 2]; 3];
                for vertex in 0..3 {
                    let index = indices[vertex] as usize;
                    position[vertex] = *source.positions.get(index).ok_or_else(|| {
                        GeometryError::Invalid("surface bake position index overflowed".to_owned())
                    })?;
                    normal[vertex] = *source.normals.get(index).ok_or_else(|| {
                        GeometryError::Invalid("surface bake normal index overflowed".to_owned())
                    })?;
                    tangent[vertex] = *source.tangents.get(index).ok_or_else(|| {
                        GeometryError::Invalid("surface bake tangent index overflowed".to_owned())
                    })?;
                    uv[vertex] = *source.uvs.get(index).ok_or_else(|| {
                        GeometryError::Invalid("surface bake UV index overflowed".to_owned())
                    })?;
                }
                triangles.push(Triangle {
                    position,
                    normal,
                    tangent,
                    uv,
                    chart: *source
                        .uv_chart_ids
                        .get(triangle_index)
                        .unwrap_or(&triangle_index) as u32,
                    part_id: part.part_id.clone(),
                    zone_id: part.material_zone_id.clone(),
                    base_key: keys
                        .get("base_color")
                        .and_then(Value::as_str)
                        .map(str::to_owned),
                    normal_key: keys
                        .get("normal")
                        .and_then(Value::as_str)
                        .map(str::to_owned),
                    mr_key: keys
                        .get("metallic_roughness")
                        .and_then(Value::as_str)
                        .map(str::to_owned),
                    vertex_ao: [1.0; 3],
                });
            }
        }
    }
    if triangles.is_empty() {
        return Err(GeometryError::Invalid(
            "surface bake has no triangles".to_owned(),
        ));
    }
    Ok(triangles)
}

fn raster_triangle(triangle: &Triangle, mut write: impl FnMut(usize, usize, [f32; 3])) {
    let to_pixel = |uv: [f32; 2]| [uv[0] * (SIZE - 1) as f32, (1.0 - uv[1]) * (SIZE - 1) as f32];
    let p = [
        to_pixel(triangle.uv[0]),
        to_pixel(triangle.uv[1]),
        to_pixel(triangle.uv[2]),
    ];
    let min_x = p
        .iter()
        .map(|p| p[0].floor() as isize)
        .min()
        .unwrap_or(0)
        .clamp(0, (SIZE - 1) as isize) as usize;
    let max_x = p
        .iter()
        .map(|p| p[0].ceil() as isize)
        .max()
        .unwrap_or(0)
        .clamp(0, (SIZE - 1) as isize) as usize;
    let min_y = p
        .iter()
        .map(|p| p[1].floor() as isize)
        .min()
        .unwrap_or(0)
        .clamp(0, (SIZE - 1) as isize) as usize;
    let max_y = p
        .iter()
        .map(|p| p[1].ceil() as isize)
        .max()
        .unwrap_or(0)
        .clamp(0, (SIZE - 1) as isize) as usize;
    let denominator =
        (p[1][1] - p[2][1]) * (p[0][0] - p[2][0]) + (p[2][0] - p[1][0]) * (p[0][1] - p[2][1]);
    if denominator.abs() < 1.0e-8 {
        return;
    }
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let sample = [x as f32 + 0.5, y as f32 + 0.5];
            let a = ((p[1][1] - p[2][1]) * (sample[0] - p[2][0])
                + (p[2][0] - p[1][0]) * (sample[1] - p[2][1]))
                / denominator;
            let b = ((p[2][1] - p[0][1]) * (sample[0] - p[2][0])
                + (p[0][0] - p[2][0]) * (sample[1] - p[2][1]))
                / denominator;
            let c = 1.0 - a - b;
            if a >= -1.0e-5 && b >= -1.0e-5 && c >= -1.0e-5 {
                write(x, y, [a, b, c]);
            }
        }
    }
}

fn vertex_ao(
    origin: [f32; 3],
    normal: [f32; 3],
    own_triangle: usize,
    triangles: &[[[f32; 3]; 3]],
    max_distance: f32,
) -> f32 {
    let n = normalize(normal);
    let helper = if n[2].abs() < 0.9 {
        [0.0, 0.0, 1.0]
    } else {
        [0.0, 1.0, 0.0]
    };
    let t = normalize(cross(helper, n));
    let b = normalize(cross(n, t));
    let samples = [
        [0.0, 0.0, 1.0],
        [0.55, 0.0, 0.835],
        [-0.55, 0.0, 0.835],
        [0.0, 0.55, 0.835],
        [0.0, -0.55, 0.835],
        [0.42, 0.42, 0.805],
        [-0.42, 0.42, 0.805],
        [0.42, -0.42, 0.805],
    ];
    let origin = add(origin, scale(n, max_distance * 0.0005));
    let mut occluded = 0usize;
    for sample in samples {
        let direction = normalize(add(
            add(scale(t, sample[0]), scale(b, sample[1])),
            scale(n, sample[2]),
        ));
        if triangles.iter().enumerate().any(|(index, triangle)| {
            index != own_triangle
                && ray_triangle(origin, direction, *triangle)
                    .is_some_and(|distance| distance < max_distance)
        }) {
            occluded += 1;
        }
    }
    (1.0 - occluded as f32 / 8.0 * 0.72).clamp(0.28, 1.0)
}

fn ray_triangle(origin: [f32; 3], direction: [f32; 3], triangle: [[f32; 3]; 3]) -> Option<f32> {
    let edge1 = sub(triangle[1], triangle[0]);
    let edge2 = sub(triangle[2], triangle[0]);
    let h = cross(direction, edge2);
    let determinant = dot(edge1, h);
    if determinant.abs() < 1.0e-7 {
        return None;
    }
    let inverse = 1.0 / determinant;
    let s = sub(origin, triangle[0]);
    let u = inverse * dot(s, h);
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let q = cross(s, edge1);
    let v = inverse * dot(direction, q);
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    let distance = inverse * dot(edge2, q);
    (distance > 1.0e-6).then_some(distance)
}

fn dilate_padding(
    owner: &mut Vec<u32>,
    base: &mut [u8],
    normal: &mut [u8],
    mr: &mut [u8],
    ao: &mut [u8],
    clearcoat: &mut [u8],
    clearcoat_rough: &mut [u8],
) {
    let mut distance = vec![u8::MAX; owner.len()];
    let mut queue = VecDeque::new();
    for y in 0..SIZE {
        for x in 0..SIZE {
            let pixel = y * SIZE + x;
            if owner[pixel] == u32::MAX {
                continue;
            }
            if neighbor_indices(x, y)
                .into_iter()
                .flatten()
                .any(|neighbor| owner[neighbor] == u32::MAX)
            {
                distance[pixel] = 0;
                queue.push_back(pixel);
            }
        }
    }
    while let Some(source) = queue.pop_front() {
        let next_distance = distance[source].saturating_add(1);
        if next_distance > PADDING as u8 {
            continue;
        }
        let x = source % SIZE;
        let y = source / SIZE;
        for pixel in neighbor_indices(x, y).into_iter().flatten() {
            if owner[pixel] != u32::MAX {
                continue;
            }
            owner[pixel] = owner[source];
            distance[pixel] = next_distance;
            for image in [&mut *base, &mut *normal, &mut *mr] {
                let target = pixel * 3;
                let from = source * 3;
                image.copy_within(from..from + 3, target);
            }
            ao[pixel] = ao[source];
            clearcoat[pixel] = clearcoat[source];
            clearcoat_rough[pixel] = clearcoat_rough[source];
            queue.push_back(pixel);
        }
    }
}

fn neighbor_indices(x: usize, y: usize) -> [Option<usize>; 4] {
    [
        x.checked_sub(1).map(|nx| y * SIZE + nx),
        (x + 1 < SIZE).then_some(y * SIZE + x + 1),
        y.checked_sub(1).map(|ny| ny * SIZE + x),
        (y + 1 < SIZE).then_some((y + 1) * SIZE + x),
    ]
}

fn output_rgb(
    id: &str,
    pixels: &[u8],
    semantic: &'static str,
    color_space: &'static str,
    normal_convention: Option<&'static str>,
) -> Result<SurfaceBakeOutput, GeometryError> {
    Ok(SurfaceBakeOutput {
        texture_id: id.to_owned(),
        bytes: encode_rgb8_png(pixels, SIZE as u32, SIZE as u32)?,
        semantic,
        color_space,
        normal_convention,
    })
}

fn output_luma(
    id: &str,
    pixels: &[u8],
    semantic: &'static str,
    color_space: &'static str,
) -> Result<SurfaceBakeOutput, GeometryError> {
    Ok(SurfaceBakeOutput {
        texture_id: id.to_owned(),
        bytes: super::encode_luma8_png(pixels, SIZE as u32, SIZE as u32)?,
        semantic,
        color_space,
        normal_convention: None,
    })
}

fn sample_rgb(
    sources: &TextureSources,
    key: Option<&str>,
    uv: [f32; 2],
    fallback: [u8; 3],
) -> [u8; 3] {
    let Some(image) = key.and_then(|key| sources.rgb.get(key)) else {
        return fallback;
    };
    let x = (uv[0].rem_euclid(1.0) * (image.width() - 1) as f32).round() as u32;
    let y = ((1.0 - uv[1].rem_euclid(1.0)) * (image.height() - 1) as f32).round() as u32;
    image.get_pixel(x, y).0
}

fn fictional_decal(uv: [f32; 2]) -> bool {
    let u = (uv[0] * 64.0).fract();
    let v = (uv[1] * 64.0).fract();
    (0.08..0.92).contains(&u) && ((0.10..0.18).contains(&v) || (0.78..0.86).contains(&v))
}

fn mix_rgb(a: [u8; 3], b: [u8; 3], factor: f32) -> [u8; 3] {
    let f = factor.clamp(0.0, 1.0);
    [0, 1, 2].map(|index| {
        ((f32::from(a[index]) * (1.0 - f) + f32::from(b[index]) * f).round()).clamp(0.0, 255.0)
            as u8
    })
}

fn triangle_bounds(triangles: &[Triangle]) -> ([f32; 3], [f32; 3]) {
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    for point in triangles.iter().flat_map(|triangle| triangle.position) {
        for axis in 0..3 {
            min[axis] = min[axis].min(point[axis]);
            max[axis] = max[axis].max(point[axis]);
        }
    }
    (min, max)
}

fn interpolate2(values: [[f32; 2]; 3], bary: [f32; 3]) -> [f32; 2] {
    [
        values[0][0] * bary[0] + values[1][0] * bary[1] + values[2][0] * bary[2],
        values[0][1] * bary[0] + values[1][1] * bary[1] + values[2][1] * bary[2],
    ]
}
fn interpolate3(values: [[f32; 3]; 3], bary: [f32; 3]) -> [f32; 3] {
    [
        values[0][0] * bary[0] + values[1][0] * bary[1] + values[2][0] * bary[2],
        values[0][1] * bary[0] + values[1][1] * bary[1] + values[2][1] * bary[2],
        values[0][2] * bary[0] + values[1][2] * bary[1] + values[2][2] * bary[2],
    ]
}
fn interpolate4(values: [[f32; 4]; 3], bary: [f32; 3]) -> [f32; 4] {
    [
        values[0][0] * bary[0] + values[1][0] * bary[1] + values[2][0] * bary[2],
        values[0][1] * bary[0] + values[1][1] * bary[1] + values[2][1] * bary[2],
        values[0][2] * bary[0] + values[1][2] * bary[1] + values[2][2] * bary[2],
        values[0][3] * bary[0] + values[1][3] * bary[1] + values[2][3] * bary[2],
    ]
}
fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn scale(a: [f32; 3], factor: f32) -> [f32; 3] {
    [a[0] * factor, a[1] * factor, a[2] * factor]
}
fn length(a: [f32; 3]) -> f32 {
    dot(a, a).sqrt()
}
fn u8_from_unit(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}
fn u8_from_signed(value: f32) -> u8 {
    u8_from_unit(value * 0.5 + 0.5)
}
fn unit_from_u8(value: u8) -> f32 {
    f32::from(value) / 255.0 * 2.0 - 1.0
}
fn hex_sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
