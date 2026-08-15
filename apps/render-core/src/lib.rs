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

#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    #[error("render input is invalid: {0}")]
    Invalid(String),
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
) -> Result<Vec<RenderPass>, RenderError> {
    render_perspective_glb_at_resolution(glb, camera, 512)
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
    )
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
    )
}

fn render_perspective_glb_at_resolution_with_passes(
    glb: &[u8],
    camera: &Value,
    resolution: u32,
    requested_passes: &[&str],
) -> Result<Vec<RenderPass>, RenderError> {
    if !(64..=512).contains(&resolution) {
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
            )
        })
    {
        return Err(RenderError::Invalid(
            "requested render passes are outside the fixed allowlist".to_owned(),
        ));
    }
    let (root, binary) = parse_glb(glb)?;
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
    let (camera_position, forward, right, up, fov_y, near, far) = parse_camera(camera)?;
    let textures = if requested_passes.contains(&"beauty") {
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
    let raster_resolution = if transient_binary_fit {
        if resolution == 512 {
            resolution * 2
        } else {
            // A 64px binary raster is sufficient for ranking a bounded
            // 128px camera neighborhood; the result is deterministically
            // upsampled to the transient contract and is never persisted.
            (resolution / 2).max(64)
        }
    } else {
        resolution * 2
    };
    let sample_width = raster_resolution;
    let sample_height = raster_resolution;
    let mut hits = vec![None::<RasterHit>; (sample_width * sample_height) as usize];
    let focal = 1.0 / (fov_y.to_radians() * 0.5).tan();
    let aspect = width as f32 / height as f32;
    let mut rendered_triangles = 0usize;

    for (mesh_index, transform) in instances {
        let mesh = meshes
            .get(mesh_index)
            .and_then(Value::as_object)
            .ok_or_else(|| RenderError::Invalid("GLB scene mesh is invalid".to_owned()))?;
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
                .ok_or_else(|| {
                    RenderError::Invalid("GLB NORMAL accessor is missing".to_owned())
                })? as usize;
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
            for index_triplet in indices.chunks_exact(3) {
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
                    let ndc_x = (x * focal / aspect) / z;
                    let ndc_y = (y * focal) / z;
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
        return Err(RenderError::Invalid(
            "camera produced no visible triangles".to_owned(),
        ));
    }
    let mut passes = Vec::with_capacity(requested_passes.len());
    for pass in requested_passes.iter().copied() {
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
                        &textures,
                        hit.material_index,
                        hit.normal,
                        hit.tangent,
                        hit.world,
                        camera_position,
                        hit.uv,
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
    Ok(passes)
}

fn parse_camera(
    camera: &Value,
) -> Result<([f32; 3], [f32; 3], [f32; 3], [f32; 3], f32, f32, f32), RenderError> {
    let object = camera
        .as_object()
        .ok_or_else(|| RenderError::Invalid("camera must be an object".to_owned()))?;
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
        return Err(RenderError::Invalid(
            "CameraCalibration@1 is not the fixed perspective contract".to_owned(),
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
    let fov_y = object
        .get("fov_y_degrees")
        .and_then(Value::as_f64)
        .ok_or_else(|| RenderError::Invalid("camera fov is missing".to_owned()))?
        as f32;
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
    if !(fov_y > 1.0 && fov_y < 179.0 && near > 0.0 && far > near) {
        return Err(RenderError::Invalid(
            "camera perspective limits are invalid".to_owned(),
        ));
    }
    Ok((position, forward, right, up, fov_y, near, far))
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

fn scene_mesh_instances(
    root: &Value,
    nodes: &[Value],
) -> Result<Vec<(usize, Mat4)>, RenderError> {
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
                child.as_u64().ok_or_else(|| {
                    RenderError::Invalid("GLB child index is invalid".to_owned())
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
fn node_transform(node: &Map<String, Value>) -> Result<Mat4, RenderError> {
    if let Some(matrix) = node.get("matrix").and_then(Value::as_array) {
        if matrix.len() != 16 {
            return Err(RenderError::Invalid(
                "GLB node matrix is invalid".to_owned(),
            ));
        }
        let mut result = [[0.0; 4]; 4];
        for (index, value) in matrix.iter().enumerate() {
            result[index % 4][index / 4] = value.as_f64().ok_or_else(|| {
                RenderError::Invalid("GLB node matrix is non-numeric".to_owned())
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
                    return Err(RenderError::Invalid(
                        "node rotation is invalid".to_owned(),
                    ));
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
    textures: &[Option<RgbaImage>],
    material_index: usize,
    normal: [f32; 3],
    tangent: [f32; 4],
    world: [f32; 3],
    camera: [f32; 3],
    uv: [f32; 2],
) -> [u8; 4] {
    let (mut base, mut metallic, mut roughness, mut emissive) =
        material_parameters(root, material_index);
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
    let emissive_strength = material_extension_factor(
        root,
        material_index,
        "KHR_materials_emissive_strength",
        "emissiveStrength",
    )
    .unwrap_or(1.0);
    let clearcoat = material_extension_factor(
        root,
        material_index,
        "KHR_materials_clearcoat",
        "clearcoatFactor",
    )
    .unwrap_or(0.0)
    .clamp(0.0, 1.0);
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
            let coat_roughness = 0.10;
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
        return Err(RenderError::Invalid(
            "GLB JSON chunk is invalid".to_owned(),
        ));
    }
    let root = serde_json::from_slice(&glb[20..20 + json_len])
        .map_err(|error| RenderError::Invalid(error.to_string()))?;
    let binary_offset = 20 + json_len;
    let binary_len =
        u32::from_le_bytes(glb[binary_offset..binary_offset + 4].try_into().unwrap()) as usize;
    if &glb[binary_offset + 4..binary_offset + 8] != b"BIN\0"
        || binary_offset + 8 + binary_len != glb.len()
    {
        return Err(RenderError::Invalid(
            "GLB BIN chunk is invalid".to_owned(),
        ));
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
                    "KHR_materials_clearcoat":{"clearcoatFactor":0.7},
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


}
