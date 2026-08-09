//! Bounded, product-owned geometry compiler for the MCP007 vertical slice.
//!
//! This is intentionally small: it accepts only a canonical GeometryProgram,
//! a few primitive operators, and emits a deterministic glTF 2.0 GLB.  It is
//! not a general scripting engine and never reads files, starts processes, or
//! calls a model/network service.

use base64::Engine;
use image::{ImageFormat, Rgba, RgbaImage};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::io::Cursor;
use std::collections::{HashMap, HashSet};

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
    operator_id: String,
    material_zone_id: String,
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    tangents: Vec<[f32; 4]>,
    indices: Vec<u32>,
    material: Value,
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

/// Compile a canonical GeometryProgram and, when supplied, a hash-bound
/// declarative AppearanceProgram. The worker never executes shader/script
/// payloads or reads external assets.
pub fn compile_geometry_program_with_appearance(
    program: &Value,
    appearance: Option<&Value>,
) -> Result<GeometryArtifact, GeometryError> {
    let object = program.as_object().ok_or(GeometryError::NotObject)?;
    if object.get("schema_version").and_then(Value::as_str) != Some("GeometryProgram@1") {
        return Err(GeometryError::Invalid("schema_version must be GeometryProgram@1".to_owned()));
    }
    let canonical_sha256 = object
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| GeometryError::Invalid("canonical_sha256 is required".to_owned()))?;
    let mut without_hash = object.clone();
    without_hash.remove("canonical_sha256");
    let program_sha256 = canonical_hash(&Value::Object(without_hash));
    if canonical_sha256 != program_sha256 {
        return Err(GeometryError::Invalid("canonical_sha256 does not match the program".to_owned()));
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
        return Err(GeometryError::Invalid("node count exceeds the declared budget".to_owned()));
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
            return Err(GeometryError::Invalid("node_id and part_id must be unique".to_owned()));
        }
        let operator_id = required_text(node, "operator_id")?;
        if operator_id != "forgecad.geometry.primitive@1" && operator_id != "forgecad.geometry.transform@1" {
            return Err(GeometryError::Invalid(format!("operator is not in the MCP007 allowlist: {operator_id}")));
        }
        let parameters = node
            .get("parameters")
            .and_then(Value::as_object)
            .ok_or_else(|| GeometryError::Invalid("node parameters must be an object".to_owned()))?;
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
            return Err(GeometryError::Invalid("segments must be between 8 and 64".to_owned()));
        }
        let (mut positions, mut normals, indices) = match shape {
            "box" => box_mesh(size),
            "cylinder" => cylinder_mesh(size, segments as usize),
            "sphere" => sphere_mesh(size, segments as usize),
            _ => return Err(GeometryError::Invalid(format!("unsupported primitive shape: {shape}"))),
        };
        for vertex in &mut positions {
            let rotated = rotate_y(*vertex, rotation_y);
            *vertex = [rotated[0] + position[0], rotated[1] + position[1], rotated[2] + position[2]];
        }
        for normal in &mut normals {
            *normal = normalize(rotate_y(*normal, rotation_y));
        }
        let (uvs, tangents) = uv_tangent_data(&positions, &normals);
        let material = appearance_zones
            .get(&material_zone_id)
            .cloned()
            .unwrap_or_else(|| material_for_zone(&material_zone_id));
        parts.push(PartMesh {
            part_id: part_id.to_owned(),
            operator_id: operator_id.to_owned(),
            material_zone_id,
            positions,
            normals,
            uvs,
            tangents,
            indices,
            material,
        });
    }
    let triangle_count = parts.iter().map(|part| part.indices.len() as u64 / 3).sum::<u64>();
    if triangle_count == 0 || triangle_count > max_triangles {
        return Err(GeometryError::Invalid("triangle count is outside the declared budget".to_owned()));
    }
    let glb = write_glb(&parts, &program_sha256, triangle_count)?;
    let mut material_zone_ids = parts
        .iter()
        .map(|part| part.material_zone_id.clone())
        .collect::<Vec<_>>();
    material_zone_ids.sort();
    material_zone_ids.dedup();
    Ok(GeometryArtifact {
        glb,
        part_ids: parts.into_iter().map(|part| part.part_id).collect(),
        triangle_count,
        program_sha256,
        uv_status: "passed".to_owned(),
        tangent_status: "passed".to_owned(),
        material_zone_ids,
    })
}

pub fn worker_result(request: &Value) -> Result<Value, GeometryError> {
    let operation = request.get("operation").and_then(Value::as_str).unwrap_or_default();
    if operation != "compile_geometry" && operation != "render_fixed" {
        return Err(GeometryError::Invalid("only compile_geometry or render_fixed is supported".to_owned()));
    }
    let payload = request
        .get("payload")
        .and_then(Value::as_object)
        .ok_or_else(|| GeometryError::Invalid("payload is required".to_owned()))?;
    let program = payload
        .get("geometry_program")
        .ok_or_else(|| GeometryError::Invalid("geometry_program is required".to_owned()))?;
    let artifact = compile_geometry_program_with_appearance(program, payload.get("appearance_program"))?;
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

fn required_text<'a>(object: &'a Map<String, Value>, key: &str) -> Result<&'a str, GeometryError> {
    let value = object.get(key).and_then(Value::as_str).unwrap_or_default();
    if value.is_empty() || value.len() > 128 || value.contains(['/', '\\']) {
        return Err(GeometryError::Invalid(format!("{key} is invalid")));
    }
    Ok(value)
}

fn bounded_u64(object: &Map<String, Value>, key: &str, min: u64, max: u64) -> Result<u64, GeometryError> {
    let value = object
        .get(key)
        .and_then(Value::as_u64)
        .ok_or_else(|| GeometryError::Invalid(format!("{key} must be an integer")))?;
    if !(min..=max).contains(&value) {
        return Err(GeometryError::Invalid(format!("{key} is outside its budget")));
    }
    Ok(value)
}

fn finite_number(object: &Map<String, Value>, key: &str, default: f32) -> Result<f32, GeometryError> {
    let value = object.get(key).and_then(Value::as_f64).unwrap_or(default as f64) as f32;
    if !value.is_finite() || value.abs() > 100.0 {
        return Err(GeometryError::Invalid(format!("{key} is not finite or is too large")));
    }
    Ok(value)
}

fn vec3(object: &Map<String, Value>, key: &str, default: [f32; 3], limit: f32) -> Result<[f32; 3], GeometryError> {
    let values = object.get(key).and_then(Value::as_array);
    let values = match values {
        Some(values) if values.len() == 3 => values,
        Some(_) => return Err(GeometryError::Invalid(format!("{key} must have three values"))),
        None => return Ok(default),
    };
    let mut result = [0.0; 3];
    for (index, value) in values.iter().enumerate() {
        let number = value
            .as_f64()
            .ok_or_else(|| GeometryError::Invalid(format!("{key} contains a non-number")))? as f32;
        if !number.is_finite() || number.abs() > limit || (key == "size" && number <= 0.0) {
            return Err(GeometryError::Invalid(format!("{key} contains an out-of-range value")));
        }
        result[index] = number;
    }
    Ok(result)
}

fn rotate_y(point: [f32; 3], angle: f32) -> [f32; 3] {
    let (sin, cos) = angle.sin_cos();
    [point[0] * cos - point[2] * sin, point[1], point[0] * sin + point[2] * cos]
}

fn normalize(value: [f32; 3]) -> [f32; 3] {
    let length = (value[0] * value[0] + value[1] * value[1] + value[2] * value[2]).sqrt();
    if length <= f32::EPSILON {
        [0.0, 1.0, 0.0]
    } else {
        [value[0] / length, value[1] / length, value[2] / length]
    }
}

fn uv_tangent_data(
    positions: &[[f32; 3]],
    normals: &[[f32; 3]],
) -> (Vec<[f32; 2]>, Vec<[f32; 4]>) {
    let (min, max) = bounds(positions);
    let extent = [
        (max[0] - min[0]).max(0.0001),
        (max[1] - min[1]).max(0.0001),
        (max[2] - min[2]).max(0.0001),
    ];
    let mut uvs = Vec::with_capacity(positions.len());
    let mut tangents = Vec::with_capacity(positions.len());
    for (position, normal) in positions.iter().zip(normals.iter()) {
        let n = normalize(*normal);
        let abs = [n[0].abs(), n[1].abs(), n[2].abs()];
        let uv = if abs[1] >= abs[0] && abs[1] >= abs[2] {
            [(position[0] - min[0]) / extent[0], (position[2] - min[2]) / extent[2]]
        } else if abs[0] >= abs[2] {
            [(position[2] - min[2]) / extent[2], (position[1] - min[1]) / extent[1]]
        } else {
            [(position[0] - min[0]) / extent[0], (position[1] - min[1]) / extent[1]]
        };
        let reference = if n[1].abs() < 0.9 { [0.0, 1.0, 0.0] } else { [1.0, 0.0, 0.0] };
        let tangent = normalize([
            reference[1] * n[2] - reference[2] * n[1],
            reference[2] * n[0] - reference[0] * n[2],
            reference[0] * n[1] - reference[1] * n[0],
        ]);
        uvs.push([uv[0].clamp(0.0, 1.0), uv[1].clamp(0.0, 1.0)]);
        tangents.push([tangent[0], tangent[1], tangent[2], 1.0]);
    }
    (uvs, tangents)
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
        return Err(GeometryError::Invalid("appearance project_id is required".to_owned()));
    }
    let expected_geometry = object
        .get("geometry_program_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| GeometryError::Invalid("appearance geometry_program_sha256 is required".to_owned()))?;
    if expected_geometry != geometry_program_sha256 {
        return Err(GeometryError::Invalid(
            "appearance is not bound to the geometry program".to_owned(),
        ));
    }
    let canonical_sha256 = object
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| GeometryError::Invalid("appearance canonical_sha256 is required".to_owned()))?;
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
        .ok_or_else(|| GeometryError::Invalid("appearance material_zones is required".to_owned()))?;
    if zones.is_empty() || zones.len() > 32 {
        return Err(GeometryError::Invalid("appearance material_zones is outside its budget".to_owned()));
    }
    let mut result = HashMap::new();
    for zone in zones {
        let zone = zone
            .as_object()
            .ok_or_else(|| GeometryError::Invalid("material zone must be an object".to_owned()))?;
        let zone_id = required_text(zone, "zone_id")?.to_owned();
        if result.contains_key(&zone_id) {
            return Err(GeometryError::Invalid("material zone ids must be unique".to_owned()));
        }
        let part_ids = zone
            .get("part_ids")
            .and_then(Value::as_array)
            .ok_or_else(|| GeometryError::Invalid("material zone part_ids is required".to_owned()))?;
        if part_ids.is_empty() || part_ids.iter().any(|value| value.as_str().is_none()) {
            return Err(GeometryError::Invalid("material zone part_ids is invalid".to_owned()));
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

fn bounded_float(object: &Map<String, Value>, key: &str, min: f32, max: f32) -> Result<f32, GeometryError> {
    let value = object
        .get(key)
        .and_then(Value::as_f64)
        .ok_or_else(|| GeometryError::Invalid(format!("{key} must be a number")))? as f32;
    if !value.is_finite() || value < min || value > max {
        return Err(GeometryError::Invalid(format!("{key} is outside its range")));
    }
    Ok(value)
}

fn color3(object: &Map<String, Value>, key: &str) -> Result<[f32; 3], GeometryError> {
    let values = object
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| GeometryError::Invalid(format!("{key} must be a three-component color")))?;
    if values.len() != 3 {
        return Err(GeometryError::Invalid(format!("{key} must be a three-component color")));
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
        return Err(GeometryError::Invalid(format!("{key} must be a four-component color")));
    }
    Ok([
        bounded_number(values[0].as_f64(), key)?,
        bounded_number(values[1].as_f64(), key)?,
        bounded_number(values[2].as_f64(), key)?,
        bounded_number(values[3].as_f64(), key)?,
    ])
}

fn bounded_number(value: Option<f64>, key: &str) -> Result<f32, GeometryError> {
    let value = value.ok_or_else(|| GeometryError::Invalid(format!("{key} contains a non-number")))? as f32;
    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
        return Err(GeometryError::Invalid(format!("{key} contains an out-of-range value")));
    }
    Ok(value)
}

fn box_mesh(size: [f32; 3]) -> (Vec<[f32; 3]>, Vec<[f32; 3]>, Vec<u32>) {
    let [x, y, z] = [size[0] / 2.0, size[1] / 2.0, size[2] / 2.0];
    let faces = [
        ([1.0, 0.0, 0.0], [[x, -y, -z], [x, -y, z], [x, y, z], [x, y, -z]]),
        ([-1.0, 0.0, 0.0], [[-x, -y, z], [-x, -y, -z], [-x, y, -z], [-x, y, z]]),
        ([0.0, 1.0, 0.0], [[-x, y, -z], [x, y, -z], [x, y, z], [-x, y, z]]),
        ([0.0, -1.0, 0.0], [[-x, -y, z], [x, -y, z], [x, -y, -z], [-x, -y, -z]]),
        ([0.0, 0.0, 1.0], [[x, -y, z], [-x, -y, z], [-x, y, z], [x, y, z]]),
        ([0.0, 0.0, -1.0], [[-x, -y, -z], [x, -y, -z], [x, y, -z], [-x, y, -z]]),
    ];
    let mut positions = Vec::with_capacity(24);
    let mut normals = Vec::with_capacity(24);
    let mut indices = Vec::with_capacity(36);
    for (face_index, (normal, vertices)) in faces.into_iter().enumerate() {
        let base = (face_index * 4) as u32;
        positions.extend(vertices);
        normals.extend([normal; 4]);
        indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    (positions, normals, indices)
}

fn cylinder_mesh(size: [f32; 3], segments: usize) -> (Vec<[f32; 3]>, Vec<[f32; 3]>, Vec<u32>) {
    let radius = (size[0].max(size[2])) / 2.0;
    let half_height = size[1] / 2.0;
    let mut positions = Vec::with_capacity(segments * 4 + 2);
    let mut normals = Vec::with_capacity(segments * 4 + 2);
    let mut indices = Vec::new();
    for ring in [-half_height, half_height] {
        for i in 0..segments {
            let angle = std::f32::consts::TAU * i as f32 / segments as f32;
            positions.push([radius * angle.cos(), ring, radius * angle.sin()]);
            normals.push([angle.cos(), 0.0, angle.sin()]);
        }
    }
    for i in 0..segments {
        let next = (i + 1) % segments;
        let bottom = i as u32;
        let top = (segments + i) as u32;
        indices.extend([bottom, next as u32, top, next as u32, (segments + next) as u32, top]);
    }
    let bottom_center = positions.len() as u32;
    positions.push([0.0, -half_height, 0.0]);
    normals.push([0.0, -1.0, 0.0]);
    let top_center = positions.len() as u32;
    positions.push([0.0, half_height, 0.0]);
    normals.push([0.0, 1.0, 0.0]);
    for i in 0..segments {
        let next = (i + 1) % segments;
        indices.extend([bottom_center, next as u32, i as u32]);
        indices.extend([top_center, (segments + i) as u32, (segments + next) as u32]);
    }
    (positions, normals, indices)
}

fn sphere_mesh(size: [f32; 3], segments: usize) -> (Vec<[f32; 3]>, Vec<[f32; 3]>, Vec<u32>) {
    let rings = (segments / 2).max(4);
    let radii = [size[0] / 2.0, size[1] / 2.0, size[2] / 2.0];
    let mut positions = Vec::with_capacity((rings + 1) * (segments + 1));
    let mut normals = Vec::with_capacity((rings + 1) * (segments + 1));
    let mut indices = Vec::new();
    for ring in 0..=rings {
        let v = ring as f32 / rings as f32;
        let phi = std::f32::consts::PI * v;
        for segment in 0..=segments {
            let u = segment as f32 / segments as f32;
            let theta = std::f32::consts::TAU * u;
            let normal = [phi.sin() * theta.cos(), phi.cos(), phi.sin() * theta.sin()];
            positions.push([normal[0] * radii[0], normal[1] * radii[1], normal[2] * radii[2]]);
            normals.push(normal);
        }
    }
    let stride = segments + 1;
    for ring in 0..rings {
        for segment in 0..segments {
            let a = (ring * stride + segment) as u32;
            let b = a + 1;
            let c = ((ring + 1) * stride + segment) as u32;
            let d = c + 1;
            indices.extend([a, c, b, b, c, d]);
        }
    }
    (positions, normals, indices)
}

fn write_glb(parts: &[PartMesh], program_sha256: &str, triangle_count: u64) -> Result<Vec<u8>, GeometryError> {
    let mut binary = Vec::new();
    let mut buffer_views = Vec::new();
    let mut accessors = Vec::new();
    let mut meshes = Vec::new();
    let mut nodes = Vec::new();
    let mut materials = Vec::new();
    for (mesh_index, part) in parts.iter().enumerate() {
        let positions_offset = append_f32_vec(&mut binary, &part.positions);
        let positions_length = part.positions.len() * 12;
        let normals_offset = append_f32_vec(&mut binary, &part.normals);
        let normals_length = part.normals.len() * 12;
        let uvs_offset = append_f32_vec2(&mut binary, &part.uvs);
        let uvs_length = part.uvs.len() * 8;
        let tangents_offset = append_f32_vec4(&mut binary, &part.tangents);
        let tangents_length = part.tangents.len() * 16;
        let indices_offset = append_u32_vec(&mut binary, &part.indices);
        let indices_length = part.indices.len() * 4;
        let pos_view = buffer_views.len();
        buffer_views.push(json!({"buffer":0,"byteOffset":positions_offset,"byteLength":positions_length,"target":34962}));
        let norm_view = buffer_views.len();
        buffer_views.push(json!({"buffer":0,"byteOffset":normals_offset,"byteLength":normals_length,"target":34962}));
        let uv_view = buffer_views.len();
        buffer_views.push(json!({"buffer":0,"byteOffset":uvs_offset,"byteLength":uvs_length,"target":34962}));
        let tangent_view = buffer_views.len();
        buffer_views.push(json!({"buffer":0,"byteOffset":tangents_offset,"byteLength":tangents_length,"target":34962}));
        let index_view = buffer_views.len();
        buffer_views.push(json!({"buffer":0,"byteOffset":indices_offset,"byteLength":indices_length,"target":34963}));
        let (min, max) = bounds(&part.positions);
        let pos_accessor = accessors.len();
        accessors.push(json!({"bufferView":pos_view,"componentType":5126,"count":part.positions.len(),"type":"VEC3","min":min,"max":max}));
        let norm_accessor = accessors.len();
        accessors.push(json!({"bufferView":norm_view,"componentType":5126,"count":part.normals.len(),"type":"VEC3"}));
        let uv_accessor = accessors.len();
        accessors.push(json!({"bufferView":uv_view,"componentType":5126,"count":part.uvs.len(),"type":"VEC2","min":[0.0,0.0],"max":[1.0,1.0]}));
        let tangent_accessor = accessors.len();
        accessors.push(json!({"bufferView":tangent_view,"componentType":5126,"count":part.tangents.len(),"type":"VEC4"}));
        let index_accessor = accessors.len();
        accessors.push(json!({"bufferView":index_view,"componentType":5125,"count":part.indices.len(),"type":"SCALAR"}));
        let material_index = materials.len();
        materials.push(part.material.clone());
        meshes.push(json!({"name":part.part_id,"primitives":[{"attributes":{"POSITION":pos_accessor,"NORMAL":norm_accessor,"TEXCOORD_0":uv_accessor,"TANGENT":tangent_accessor},"indices":index_accessor,"material":material_index}],"extras":{"part_id":part.part_id,"operator_id":part.operator_id,"material_zone_id":part.material_zone_id}}));
        nodes.push(json!({"name":part.part_id,"mesh":mesh_index,"extras":{"part_id":part.part_id,"operator_id":part.operator_id,"material_zone_id":part.material_zone_id}}));
    }
    while binary.len() % 4 != 0 { binary.push(0); }
    let root = json!({
        "asset":{"version":"2.0","generator":"ForgeCAD MCP008 bounded appearance compiler"},
        "scene":0,
        "scenes":[{"nodes":(0..nodes.len()).collect::<Vec<_>>() }],
        "nodes":nodes,
        "meshes":meshes,
        "materials":materials,
        "buffers":[{"byteLength":binary.len()}],
        "bufferViews":buffer_views,
        "accessors":accessors,
        "extras":{"forgecad":{"schema_version":"ArtifactReadback@1","program_sha256":program_sha256,"triangle_count":triangle_count,"part_ids":parts.iter().map(|part|part.part_id.clone()).collect::<Vec<_>>(),"uv_status":"passed","tangent_status":"passed","material_zone_ids":parts.iter().map(|part|part.material_zone_id.clone()).collect::<Vec<_>>()}}
    });
    let mut json_bytes = serde_json::to_vec(&root).map_err(|error| GeometryError::Invalid(error.to_string()))?;
    while json_bytes.len() % 4 != 0 { json_bytes.push(b' '); }
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
                .ok_or_else(|| GeometryError::Invalid("GLB primitive attributes are missing".to_owned()))?;
            let position_accessor = attributes
                .get("POSITION")
                .and_then(Value::as_u64)
                .ok_or_else(|| GeometryError::Invalid("GLB POSITION accessor is missing".to_owned()))? as usize;
            let normal_accessor = attributes.get("NORMAL").and_then(Value::as_u64).map(|value| value as usize);
            let index_accessor = primitive
                .get("indices")
                .and_then(Value::as_u64)
                .ok_or_else(|| GeometryError::Invalid("GLB index accessor is missing".to_owned()))? as usize;
            let positions = read_vec3_accessor(accessors, views, &binary, position_accessor)?;
            let normals = normal_accessor
                .map(|index| read_vec3_accessor(accessors, views, &binary, index))
                .transpose()?
                .unwrap_or_else(|| vec![[0.0, 1.0, 0.0]; positions.len()]);
            if normals.len() != positions.len() {
                return Err(GeometryError::Invalid("GLB normal count does not match positions".to_owned()));
            }
            let base = vertices.len();
            vertices.extend(positions.into_iter().zip(normals.into_iter()).map(|(position, normal)| (position, normal, mesh_index)));
            for indices in read_indices_accessor(accessors, views, &binary, index_accessor)?.chunks_exact(3) {
                triangles.push(([base + indices[0] as usize, base + indices[1] as usize, base + indices[2] as usize], mesh_index));
            }
        }
    }
    if vertices.is_empty() || triangles.is_empty() {
        return Err(GeometryError::Invalid("GLB has no renderable triangles".to_owned()));
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
            let projected = triangle.map(|index| project(vertices[index].0, min, scale, width, height));
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
            .map_err(|error| GeometryError::Invalid(format!("fixed render encode failed: {error}")))?;
        passes.push(RenderPass { pass: pass.to_owned(), png: bytes, width, height });
    }
    Ok(passes)
}

fn parse_glb(glb: &[u8]) -> Result<(Value, Vec<u8>), GeometryError> {
    if glb.len() < 20 || &glb[..4] != b"glTF" || u32::from_le_bytes(glb[4..8].try_into().unwrap()) != 2 {
        return Err(GeometryError::Invalid("GLB header is invalid".to_owned()));
    }
    let total = u32::from_le_bytes(glb[8..12].try_into().unwrap()) as usize;
    if total != glb.len() { return Err(GeometryError::Invalid("GLB length is invalid".to_owned())); }
    let json_len = u32::from_le_bytes(glb[12..16].try_into().unwrap()) as usize;
    if &glb[16..20] != b"JSON" || 20 + json_len + 8 > glb.len() { return Err(GeometryError::Invalid("GLB JSON chunk is invalid".to_owned())); }
    let root = serde_json::from_slice(&glb[20..20 + json_len]).map_err(|error| GeometryError::Invalid(error.to_string()))?;
    let binary_offset = 20 + json_len;
    let binary_len = u32::from_le_bytes(glb[binary_offset..binary_offset + 4].try_into().unwrap()) as usize;
    if &glb[binary_offset + 4..binary_offset + 8] != b"BIN\0" || binary_offset + 8 + binary_len != glb.len() {
        return Err(GeometryError::Invalid("GLB BIN chunk is invalid".to_owned()));
    }
    Ok((root, glb[binary_offset + 8..].to_vec()))
}

fn accessor_view<'a>(accessors: &'a [Value], views: &'a [Value], index: usize) -> Result<(&'a Value, &'a Value), GeometryError> {
    let accessor = accessors.get(index).ok_or_else(|| GeometryError::Invalid("GLB accessor index is invalid".to_owned()))?;
    let view_index = accessor.get("bufferView").and_then(Value::as_u64).ok_or_else(|| GeometryError::Invalid("GLB accessor bufferView is missing".to_owned()))? as usize;
    let view = views.get(view_index).ok_or_else(|| GeometryError::Invalid("GLB bufferView index is invalid".to_owned()))?;
    Ok((accessor, view))
}

fn read_vec3_accessor(accessors: &[Value], views: &[Value], binary: &[u8], index: usize) -> Result<Vec<[f32; 3]>, GeometryError> {
    let (accessor, view) = accessor_view(accessors, views, index)?;
    if accessor.get("componentType").and_then(Value::as_u64) != Some(5126) || accessor.get("type").and_then(Value::as_str) != Some("VEC3") { return Err(GeometryError::Invalid("GLB VEC3 accessor is not float".to_owned())); }
    let count = accessor.get("count").and_then(Value::as_u64).ok_or_else(|| GeometryError::Invalid("GLB accessor count is missing".to_owned()))? as usize;
    let offset = view.get("byteOffset").and_then(Value::as_u64).unwrap_or(0) as usize + accessor.get("byteOffset").and_then(Value::as_u64).unwrap_or(0) as usize;
    if offset + count.saturating_mul(12) > binary.len() { return Err(GeometryError::Invalid("GLB VEC3 accessor exceeds BIN".to_owned())); }
    let mut values = Vec::with_capacity(count);
    for chunk in binary[offset..offset + count * 12].chunks_exact(12) {
        values.push([f32::from_le_bytes(chunk[0..4].try_into().unwrap()), f32::from_le_bytes(chunk[4..8].try_into().unwrap()), f32::from_le_bytes(chunk[8..12].try_into().unwrap())]);
    }
    Ok(values)
}

fn read_indices_accessor(accessors: &[Value], views: &[Value], binary: &[u8], index: usize) -> Result<Vec<u32>, GeometryError> {
    let (accessor, view) = accessor_view(accessors, views, index)?;
    if accessor.get("componentType").and_then(Value::as_u64) != Some(5125) || accessor.get("type").and_then(Value::as_str) != Some("SCALAR") { return Err(GeometryError::Invalid("GLB index accessor is not uint32".to_owned())); }
    let count = accessor.get("count").and_then(Value::as_u64).ok_or_else(|| GeometryError::Invalid("GLB index count is missing".to_owned()))? as usize;
    let offset = view.get("byteOffset").and_then(Value::as_u64).unwrap_or(0) as usize + accessor.get("byteOffset").and_then(Value::as_u64).unwrap_or(0) as usize;
    if offset + count.saturating_mul(4) > binary.len() { return Err(GeometryError::Invalid("GLB index accessor exceeds BIN".to_owned())); }
    Ok(binary[offset..offset + count * 4].chunks_exact(4).map(|chunk| u32::from_le_bytes(chunk.try_into().unwrap())).collect())
}

fn project(position: [f32; 3], min: [f32; 3], scale: f32, width: u32, height: u32) -> [f32; 2] {
    let margin = 18.0;
    let x = margin + (position[0] - min[0]) / scale * (width as f32 - 2.0 * margin);
    let y = height as f32 - margin - (position[1] - min[1]) / scale * (height as f32 - 2.0 * margin);
    [x, y]
}

fn rasterize_triangle(image: &mut RgbaImage, points: [[f32; 2]; 3], color: [u8; 4]) {
    let min_x = points.iter().map(|point| point[0]).fold(f32::INFINITY, f32::min).floor().max(0.0) as u32;
    let max_x = points.iter().map(|point| point[0]).fold(f32::NEG_INFINITY, f32::max).ceil().min(image.width() as f32 - 1.0) as u32;
    let min_y = points.iter().map(|point| point[1]).fold(f32::INFINITY, f32::min).floor().max(0.0) as u32;
    let max_y = points.iter().map(|point| point[1]).fold(f32::NEG_INFINITY, f32::max).ceil().min(image.height() as f32 - 1.0) as u32;
    let area = edge(points[0], points[1], points[2]);
    if area.abs() < f32::EPSILON { return; }
    for y in min_y..=max_y { for x in min_x..=max_x {
        let point = [x as f32 + 0.5, y as f32 + 0.5];
        let w0 = edge(points[1], points[2], point);
        let w1 = edge(points[2], points[0], point);
        let w2 = edge(points[0], points[1], point);
        if (w0 >= 0.0 && w1 >= 0.0 && w2 >= 0.0) || (w0 <= 0.0 && w1 <= 0.0 && w2 <= 0.0) { image.put_pixel(x, y, Rgba(color)); }
    }}
}

fn edge(a: [f32; 2], b: [f32; 2], c: [f32; 2]) -> f32 { (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]) }
fn normal_color(normal: [f32; 3]) -> [u8; 4] { [((normal[0] * 0.5 + 0.5) * 255.0) as u8, ((normal[1] * 0.5 + 0.5) * 255.0) as u8, ((normal[2] * 0.5 + 0.5) * 255.0) as u8, 255] }
fn part_color(index: usize) -> [u8; 4] { [((index.wrapping_mul(97) + 53) % 220 + 20) as u8, ((index.wrapping_mul(53) + 79) % 170 + 40) as u8, ((index.wrapping_mul(31) + 131) % 120 + 80) as u8, 255] }
fn material_color(root: &Value, mesh_index: usize) -> [u8; 4] {
    let material_index = root.get("meshes").and_then(Value::as_array).and_then(|meshes| meshes.get(mesh_index)).and_then(|mesh| mesh.get("primitives")).and_then(Value::as_array).and_then(|primitives| primitives.first()).and_then(|primitive| primitive.get("material")).and_then(Value::as_u64).unwrap_or(0) as usize;
    let factor = root.get("materials").and_then(Value::as_array).and_then(|materials| materials.get(material_index)).and_then(|material| material.get("pbrMetallicRoughness")).and_then(|pbr| pbr.get("baseColorFactor")).and_then(Value::as_array).cloned().unwrap_or_else(|| vec![Value::from(0.6), Value::from(0.65), Value::from(0.7), Value::from(1.0)]);
    [
        (factor.first().and_then(Value::as_f64).unwrap_or(0.6).clamp(0.0, 1.0) * 255.0) as u8,
        (factor.get(1).and_then(Value::as_f64).unwrap_or(0.65).clamp(0.0, 1.0) * 255.0) as u8,
        (factor.get(2).and_then(Value::as_f64).unwrap_or(0.7).clamp(0.0, 1.0) * 255.0) as u8,
        255,
    ]
}

fn append_f32_vec(binary: &mut Vec<u8>, values: &[[f32; 3]]) -> usize {
    while binary.len() % 4 != 0 { binary.push(0); }
    let offset = binary.len();
    for value in values { for component in value { binary.extend_from_slice(&component.to_le_bytes()); } }
    offset
}

fn append_u32_vec(binary: &mut Vec<u8>, values: &[u32]) -> usize {
    while binary.len() % 4 != 0 { binary.push(0); }
    let offset = binary.len();
    for value in values { binary.extend_from_slice(&value.to_le_bytes()); }
    offset
}

fn append_f32_vec2(binary: &mut Vec<u8>, values: &[[f32; 2]]) -> usize {
    while binary.len() % 4 != 0 { binary.push(0); }
    let offset = binary.len();
    for value in values { for component in value { binary.extend_from_slice(&component.to_le_bytes()); } }
    offset
}

fn append_f32_vec4(binary: &mut Vec<u8>, values: &[[f32; 4]]) -> usize {
    while binary.len() % 4 != 0 { binary.push(0); }
    let offset = binary.len();
    for value in values { for component in value { binary.extend_from_slice(&component.to_le_bytes()); } }
    offset
}

fn bounds(values: &[[f32; 3]]) -> ([f32; 3], [f32; 3]) {
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    for value in values { for index in 0..3 { min[index] = min[index].min(value[index]); max[index] = max[index].max(value[index]); } }
    (min, max)
}

fn material_for_zone(zone: &str) -> Value {
    let (base, metallic, roughness, emissive) = if zone.contains("mechanical") || zone.contains("black") {
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
    digest.finalize().iter().map(|byte| format!("{byte:02x}")).collect()
}

fn write_canonical(value: &Value, output: &mut Vec<u8>) {
    match value {
        Value::Null => output.extend_from_slice(b"null"),
        Value::Bool(value) => output.extend_from_slice(if *value { b"true" } else { b"false" }),
        Value::Number(value) => output.extend_from_slice(value.to_string().as_bytes()),
        Value::String(value) => serde_json::to_writer(&mut *output, value).expect("string serializes"),
        Value::Array(values) => { output.push(b'['); for (index, value) in values.iter().enumerate() { if index != 0 { output.push(b','); } write_canonical(value, output); } output.push(b']'); }
        Value::Object(values) => { let mut keys = values.keys().collect::<Vec<_>>(); keys.sort_unstable(); output.push(b'{'); for (index, key) in keys.iter().enumerate() { if index != 0 { output.push(b','); } serde_json::to_writer(&mut *output, key).expect("key serializes"); output.push(b':'); write_canonical(&values[*key], output); } output.push(b'}'); }
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
        program.as_object_mut().unwrap().insert("canonical_sha256".to_owned(), Value::String(hash));
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

    fn without_hash(value: &Value) -> Value { let mut object = value.as_object().unwrap().clone(); object.remove("canonical_sha256"); Value::Object(object) }
}
