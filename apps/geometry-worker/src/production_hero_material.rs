//! Worker-only, deterministic 2K Hero material assembly for the production
//! weapon Low asset. The operation accepts only hash-bound in-memory bytes,
//! embeds every texture in one GLB, and never writes Runtime or CAS state.

use super::{integrity, GeometryError};
use base64::Engine;
use forgecad_worker_protocol::{
    PRODUCTION_WEAPON_GEOMETRIC_BAKE_NORMAL_CONVENTION,
    PRODUCTION_WEAPON_GEOMETRIC_BAKE_RESOLUTION, PRODUCTION_WEAPON_HERO_MATERIAL_OPERATION,
    PRODUCTION_WEAPON_HERO_MATERIAL_POLICY, PRODUCTION_WEAPON_HERO_MATERIAL_REQUEST_SCHEMA_VERSION,
    PRODUCTION_WEAPON_HERO_MATERIAL_RESULT_SCHEMA_VERSION,
};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

const MAX_GLB_BYTES: usize = 64 * 1024 * 1024;
const MAX_PNG_BYTES: usize = 16 * 1024 * 1024;
const PADDING_TEXELS: usize = 4;
const ALGORITHM_ID: &str = "forgecad-production-weapon-hero-material@1|2048|hash-bound-geometric-bake|fixed-4px-dilation|embedded-glb|metal-rough-pbr|OpenGL+Y|no-rng-no-time-no-network";

struct GlbDocument {
    root: Value,
    binary: Vec<u8>,
}

struct TextureOutput {
    name: &'static str,
    bytes: Vec<u8>,
    semantic: &'static str,
    color_space: &'static str,
    normal_convention: Option<&'static str>,
    source_sha256: Option<String>,
}

pub fn run(payload: &Map<String, Value>) -> Result<Value, GeometryError> {
    const FIELDS: &[&str] = &[
        "schema_version",
        "material_policy",
        "material_policy_sha256",
        "low_glb_base64",
        "low_artifact_sha256",
        "normal_png_base64",
        "normal_png_sha256",
        "ao_png_base64",
        "ao_png_sha256",
        "curvature_png_base64",
        "curvature_png_sha256",
        "geometric_bake_canonical_sha256",
        "resolution",
        "normal_convention",
        "canonical_sha256",
    ];
    super::require_closed_payload(payload, FIELDS)?;
    require_const(
        payload,
        "schema_version",
        PRODUCTION_WEAPON_HERO_MATERIAL_REQUEST_SCHEMA_VERSION,
    )?;
    require_const(
        payload,
        "material_policy",
        PRODUCTION_WEAPON_HERO_MATERIAL_POLICY,
    )?;
    require_const(
        payload,
        "normal_convention",
        PRODUCTION_WEAPON_GEOMETRIC_BAKE_NORMAL_CONVENTION,
    )?;
    if payload.get("resolution").and_then(Value::as_u64)
        != Some(PRODUCTION_WEAPON_GEOMETRIC_BAKE_RESOLUTION)
    {
        return Err(GeometryError::Invalid(
            "Hero material resolution must be fixed 2048".to_owned(),
        ));
    }
    let policy_hash = required_hash(payload, "material_policy_sha256")?;
    if policy_hash != hash_bytes(PRODUCTION_WEAPON_HERO_MATERIAL_POLICY.as_bytes()) {
        return Err(GeometryError::Invalid(
            "Hero material policy hash does not match".to_owned(),
        ));
    }
    let canonical = required_hash(payload, "canonical_sha256")?;
    let mut canonical_preimage = payload.clone();
    canonical_preimage.remove("canonical_sha256");
    if super::canonical_hash(&Value::Object(canonical_preimage)) != canonical {
        return Err(GeometryError::Invalid(
            "Hero material request canonical_sha256 does not match".to_owned(),
        ));
    }

    let low_hash = required_hash(payload, "low_artifact_sha256")?.to_owned();
    let geometric_bake_hash = required_hash(payload, "geometric_bake_canonical_sha256")?.to_owned();
    let low_bytes = decode_bound_bytes(
        payload,
        "low_glb_base64",
        &low_hash,
        MAX_GLB_BYTES,
        "Low GLB",
    )?;
    let low_inspection = integrity::inspect_glb(&low_bytes)?;
    if low_inspection.external_uri_count != 0
        || low_inspection.metadata_mismatch_count != 0
        || low_inspection.non_finite_count != 0
        || low_inspection.invalid_index_count != 0
        || low_inspection.uv_non_finite_count != 0
        || low_inspection.zero_area_uv_triangle_count != 0
        || low_inspection.tangent_non_finite_count != 0
        || low_inspection.tangent_orthogonality_error_count != 0
        || low_inspection.tangent_handedness_error_count != 0
    {
        return Err(GeometryError::Invalid(
            "Hero material Low GLB failed strict topology/UV/tangent admission".to_owned(),
        ));
    }

    let normal_source_hash = required_hash(payload, "normal_png_sha256")?.to_owned();
    let ao_source_hash = required_hash(payload, "ao_png_sha256")?.to_owned();
    let curvature_source_hash = required_hash(payload, "curvature_png_sha256")?.to_owned();
    let normal_png = decode_bound_bytes(
        payload,
        "normal_png_base64",
        &normal_source_hash,
        MAX_PNG_BYTES,
        "Normal PNG",
    )?;
    let ao_png = decode_bound_bytes(
        payload,
        "ao_png_base64",
        &ao_source_hash,
        MAX_PNG_BYTES,
        "AO PNG",
    )?;
    let curvature_png = decode_bound_bytes(
        payload,
        "curvature_png_base64",
        &curvature_source_hash,
        MAX_PNG_BYTES,
        "Curvature PNG",
    )?;
    let resolution = PRODUCTION_WEAPON_GEOMETRIC_BAKE_RESOLUTION as usize;
    let mut normal = decode_rgb_2k(&normal_png, "Normal")?;
    let mut ao = decode_luma_2k(&ao_png, "AO")?;
    let mut curvature = decode_luma_2k(&curvature_png, "Curvature")?;
    let mut covered = (0..resolution * resolution)
        .map(|index| {
            let offset = index * 3;
            normal[offset] != 128 || normal[offset + 1] != 128 || normal[offset + 2] != 128
        })
        .collect::<Vec<_>>();
    let source_covered_pixels = covered.iter().filter(|value| **value).count();
    dilate_bake_maps(
        &mut normal,
        &mut ao,
        &mut curvature,
        &mut covered,
        resolution,
        PADDING_TEXELS,
    );
    let padded_covered_pixels = covered.iter().filter(|value| **value).count();

    let mut base_color = vec![255u8; resolution * resolution * 3];
    let mut metallic_roughness = vec![255u8; resolution * resolution * 3];
    let mut emissive = vec![0u8; resolution * resolution * 3];
    for index in 0..resolution * resolution {
        if !covered[index] {
            continue;
        }
        let curve = curvature[index] as u16;
        let base = (204 + curve / 7).min(238) as u8;
        let offset = index * 3;
        base_color[offset] = base;
        base_color[offset + 1] = base.saturating_sub(3);
        base_color[offset + 2] = base.saturating_sub(8);
        metallic_roughness[offset] = 255;
        metallic_roughness[offset + 1] = (214u16.saturating_sub(curve / 5)).max(150) as u8;
        metallic_roughness[offset + 2] = 255;
        emissive[offset] = 255;
        emissive[offset + 1] = 116u8.saturating_add((curve / 8) as u8);
        emissive[offset + 2] = 10;
    }

    let textures = vec![
        TextureOutput {
            name: "hero-base-color-2048",
            bytes: super::encode_rgb8_png(&base_color, 2048, 2048)?,
            semantic: "baseColor",
            color_space: "sRGB",
            normal_convention: None,
            source_sha256: None,
        },
        TextureOutput {
            name: "hero-normal-open-gl-2048",
            bytes: super::encode_rgb8_png(&normal, 2048, 2048)?,
            semantic: "normal",
            color_space: "linear",
            normal_convention: Some(PRODUCTION_WEAPON_GEOMETRIC_BAKE_NORMAL_CONVENTION),
            source_sha256: Some(normal_source_hash.clone()),
        },
        TextureOutput {
            name: "hero-metallic-roughness-2048",
            bytes: super::encode_rgb8_png(&metallic_roughness, 2048, 2048)?,
            semantic: "metallicRoughness",
            color_space: "linear",
            normal_convention: None,
            source_sha256: None,
        },
        TextureOutput {
            name: "hero-ao-2048",
            bytes: super::encode_luma8_png(&ao, 2048, 2048)?,
            semantic: "occlusion",
            color_space: "linear",
            normal_convention: None,
            source_sha256: Some(ao_source_hash.clone()),
        },
        TextureOutput {
            name: "hero-emissive-2048",
            bytes: super::encode_rgb8_png(&emissive, 2048, 2048)?,
            semantic: "emissive",
            color_space: "sRGB",
            normal_convention: None,
            source_sha256: None,
        },
        TextureOutput {
            name: "hero-curvature-2048",
            bytes: super::encode_luma8_png(&curvature, 2048, 2048)?,
            semantic: "curvature",
            color_space: "linear",
            normal_convention: None,
            source_sha256: Some(curvature_source_hash.clone()),
        },
    ];
    let mut document = parse_glb(&low_bytes)?;
    bind_materials_and_textures(
        &mut document,
        &textures,
        &low_hash,
        &geometric_bake_hash,
        source_covered_pixels,
        padded_covered_pixels,
    )?;
    let glb = write_glb(document)?;
    let artifact_sha256 = hash_bytes(&glb);
    let inspection = integrity::inspect_glb(&glb)?;
    if inspection.external_uri_count != 0
        || inspection.metadata_mismatch_count != 0
        || inspection.non_finite_count != 0
        || inspection.invalid_index_count != 0
        || inspection.uv_non_finite_count != 0
        || inspection.zero_area_uv_triangle_count != 0
        || inspection.tangent_non_finite_count != 0
        || inspection.tangent_orthogonality_error_count != 0
        || inspection.tangent_handedness_error_count != 0
    {
        return Err(GeometryError::Invalid(
            "Hero material GLB failed strict post-build readback".to_owned(),
        ));
    }
    let output_receipts = textures
        .iter()
        .map(|texture| {
            json!({
                "texture_id":texture.name,
                "sha256":hash_bytes(&texture.bytes),
                "size_bytes":texture.bytes.len(),
                "width":2048,
                "height":2048,
                "mime":"image/png",
                "semantic":texture.semantic,
                "color_space":texture.color_space,
                "normal_convention":texture.normal_convention,
                "source_geometric_bake_png_sha256":texture.source_sha256,
            })
        })
        .collect::<Vec<_>>();
    let mut result = json!({
        "schema_version":PRODUCTION_WEAPON_HERO_MATERIAL_RESULT_SCHEMA_VERSION,
        "operation":PRODUCTION_WEAPON_HERO_MATERIAL_OPERATION,
        "material_policy":PRODUCTION_WEAPON_HERO_MATERIAL_POLICY,
        "material_policy_sha256":policy_hash,
        "low_artifact_sha256":low_hash,
        "geometric_bake_canonical_sha256":geometric_bake_hash,
        "source_geometric_bake_png_sha256":{
            "normal":normal_source_hash,
            "ao":ao_source_hash,
            "curvature":curvature_source_hash,
        },
        "resolution":2048,
        "normal_convention":PRODUCTION_WEAPON_GEOMETRIC_BAKE_NORMAL_CONVENTION,
        "padding_texels":PADDING_TEXELS,
        "source_covered_pixels":source_covered_pixels,
        "padded_covered_pixels":padded_covered_pixels,
        "material_zone_count":4,
        "texture_count":textures.len(),
        "embedded_only":true,
        "external_uri_count":inspection.external_uri_count,
        "triangle_count":inspection.triangle_count,
        "part_count":inspection.part_ids.len(),
        "outputs":output_receipts,
        "hero_material_glb_base64":base64::engine::general_purpose::STANDARD.encode(&glb),
        "hero_material_glb_sha256":artifact_sha256,
        "hero_material_glb_size_bytes":glb.len(),
        "worker_algorithm_id":ALGORITHM_ID,
        "worker_algorithm_sha256":hash_bytes(ALGORITHM_ID.as_bytes()),
        "quality_status":"SOURCE_STRUCTURAL_ONLY",
        "visual_quality_status":"NOT_PROVEN",
        "runtime_write_performed":false,
        "candidate_confirmed":false,
        "version_created":false,
        "export_performed":false,
        "canonical_sha256":"",
    });
    result["canonical_sha256"] = Value::String(super::canonical_hash(&result));
    Ok(result)
}

fn require_const(
    payload: &Map<String, Value>,
    key: &str,
    expected: &str,
) -> Result<(), GeometryError> {
    if payload.get(key).and_then(Value::as_str) != Some(expected) {
        return Err(GeometryError::Invalid(format!(
            "Hero material {key} is invalid"
        )));
    }
    Ok(())
}

fn required_hash<'a>(payload: &'a Map<String, Value>, key: &str) -> Result<&'a str, GeometryError> {
    let value = payload
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| GeometryError::Invalid(format!("Hero material {key} is required")))?;
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(GeometryError::Invalid(format!(
            "Hero material {key} is not SHA-256"
        )));
    }
    Ok(value)
}

fn decode_bound_bytes(
    payload: &Map<String, Value>,
    key: &str,
    expected_hash: &str,
    maximum: usize,
    label: &str,
) -> Result<Vec<u8>, GeometryError> {
    let encoded = payload.get(key).and_then(Value::as_str).ok_or_else(|| {
        GeometryError::Invalid(format!("Hero material {label} bytes are required"))
    })?;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|_| GeometryError::Invalid(format!("Hero material {label} base64 is invalid")))?;
    if bytes.is_empty() || bytes.len() > maximum || hash_bytes(&bytes) != expected_hash {
        return Err(GeometryError::Invalid(format!(
            "Hero material {label} byte budget or hash does not match"
        )));
    }
    Ok(bytes)
}

fn decode_rgb_2k(bytes: &[u8], label: &str) -> Result<Vec<u8>, GeometryError> {
    let image = image::load_from_memory(bytes)
        .map_err(|error| GeometryError::Invalid(format!("{label} PNG decode failed: {error}")))?;
    if image.width() != 2048 || image.height() != 2048 {
        return Err(GeometryError::Invalid(format!(
            "{label} PNG is not 2048x2048"
        )));
    }
    Ok(image.to_rgb8().into_raw())
}

fn decode_luma_2k(bytes: &[u8], label: &str) -> Result<Vec<u8>, GeometryError> {
    let image = image::load_from_memory(bytes)
        .map_err(|error| GeometryError::Invalid(format!("{label} PNG decode failed: {error}")))?;
    if image.width() != 2048 || image.height() != 2048 {
        return Err(GeometryError::Invalid(format!(
            "{label} PNG is not 2048x2048"
        )));
    }
    Ok(image.to_luma8().into_raw())
}

fn dilate_bake_maps(
    normal: &mut Vec<u8>,
    ao: &mut Vec<u8>,
    curvature: &mut Vec<u8>,
    covered: &mut Vec<bool>,
    resolution: usize,
    iterations: usize,
) {
    const NEIGHBORS: [(isize, isize); 4] = [(0, -1), (-1, 0), (1, 0), (0, 1)];
    for _ in 0..iterations {
        let previous_covered = covered.clone();
        let previous_normal = normal.clone();
        let previous_ao = ao.clone();
        let previous_curvature = curvature.clone();
        for y in 0..resolution {
            for x in 0..resolution {
                let index = y * resolution + x;
                if previous_covered[index] {
                    continue;
                }
                let source = NEIGHBORS.iter().find_map(|(dx, dy)| {
                    let nx = x as isize + dx;
                    let ny = y as isize + dy;
                    if nx < 0 || ny < 0 || nx >= resolution as isize || ny >= resolution as isize {
                        return None;
                    }
                    let neighbor = ny as usize * resolution + nx as usize;
                    previous_covered[neighbor].then_some(neighbor)
                });
                if let Some(source) = source {
                    covered[index] = true;
                    normal[index * 3..index * 3 + 3]
                        .copy_from_slice(&previous_normal[source * 3..source * 3 + 3]);
                    ao[index] = previous_ao[source];
                    curvature[index] = previous_curvature[source];
                }
            }
        }
    }
}

fn parse_glb(bytes: &[u8]) -> Result<GlbDocument, GeometryError> {
    if bytes.len() < 28
        || &bytes[..4] != b"glTF"
        || u32::from_le_bytes(bytes[4..8].try_into().unwrap()) != 2
    {
        return Err(GeometryError::Invalid(
            "Hero material Low GLB header is invalid".to_owned(),
        ));
    }
    let declared = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
    if declared != bytes.len() {
        return Err(GeometryError::Invalid(
            "Hero material Low GLB length drifted".to_owned(),
        ));
    }
    let json_length = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    if u32::from_le_bytes(bytes[16..20].try_into().unwrap()) != 0x4e4f534a {
        return Err(GeometryError::Invalid(
            "Hero material Low GLB JSON chunk is missing".to_owned(),
        ));
    }
    let json_end = 20usize
        .checked_add(json_length)
        .filter(|end| *end + 8 <= bytes.len())
        .ok_or_else(|| {
            GeometryError::Invalid("Hero material Low GLB JSON range overflowed".to_owned())
        })?;
    let root = serde_json::from_slice::<Value>(&bytes[20..json_end]).map_err(|error| {
        GeometryError::Invalid(format!("Hero material Low GLB JSON is invalid: {error}"))
    })?;
    let bin_length = u32::from_le_bytes(bytes[json_end..json_end + 4].try_into().unwrap()) as usize;
    if u32::from_le_bytes(bytes[json_end + 4..json_end + 8].try_into().unwrap()) != 0x004e4942 {
        return Err(GeometryError::Invalid(
            "Hero material Low GLB BIN chunk is missing".to_owned(),
        ));
    }
    let bin_start = json_end + 8;
    let bin_end = bin_start
        .checked_add(bin_length)
        .filter(|end| *end == bytes.len())
        .ok_or_else(|| {
            GeometryError::Invalid("Hero material Low GLB BIN range drifted".to_owned())
        })?;
    Ok(GlbDocument {
        root,
        binary: bytes[bin_start..bin_end].to_vec(),
    })
}

fn bind_materials_and_textures(
    document: &mut GlbDocument,
    outputs: &[TextureOutput],
    low_hash: &str,
    bake_hash: &str,
    source_covered_pixels: usize,
    padded_covered_pixels: usize,
) -> Result<(), GeometryError> {
    let root = document.root.as_object_mut().ok_or_else(|| {
        GeometryError::Invalid("Hero material GLB root is not an object".to_owned())
    })?;
    let original_materials = root
        .get("materials")
        .and_then(Value::as_array)
        .cloned()
        .ok_or_else(|| {
            GeometryError::Invalid("Hero material Low material inventory is missing".to_owned())
        })?;
    let zone_order = [
        "zone-white-shell",
        "zone-black-mechanical",
        "zone-gold-accent",
        "zone-amber-emissive",
    ];
    let zone_to_index = zone_order
        .iter()
        .enumerate()
        .map(|(index, zone)| ((*zone).to_owned(), index))
        .collect::<BTreeMap<_, _>>();
    for mesh in root
        .get_mut("meshes")
        .and_then(Value::as_array_mut)
        .into_iter()
        .flatten()
    {
        for primitive in mesh
            .get_mut("primitives")
            .and_then(Value::as_array_mut)
            .into_iter()
            .flatten()
        {
            let original_index = primitive
                .get("material")
                .and_then(Value::as_u64)
                .and_then(|value| usize::try_from(value).ok())
                .ok_or_else(|| {
                    GeometryError::Invalid("Hero material primitive binding is invalid".to_owned())
                })?;
            let zone = original_materials
                .get(original_index)
                .and_then(|value| value.get("name"))
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    GeometryError::Invalid("Hero material source zone is missing".to_owned())
                })?;
            primitive["material"] = Value::from(*zone_to_index.get(zone).ok_or_else(|| {
                GeometryError::Invalid(format!(
                    "Hero material source zone is not allowlisted: {zone}"
                ))
            })? as u64);
        }
    }
    root.insert(
        "materials".to_owned(),
        Value::Array(zone_order.iter().map(|zone| hero_material(zone)).collect()),
    );
    let views = root
        .get_mut("bufferViews")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| {
            GeometryError::Invalid("Hero material bufferView inventory is missing".to_owned())
        })?;
    let mut images = Vec::new();
    let mut textures = Vec::new();
    for output in outputs {
        while document.binary.len() % 4 != 0 {
            document.binary.push(0);
        }
        let offset = document.binary.len();
        document.binary.extend_from_slice(&output.bytes);
        while document.binary.len() % 4 != 0 {
            document.binary.push(0);
        }
        let stored_length = document.binary.len() - offset;
        let view_index = views.len();
        views.push(json!({"buffer":0,"byteOffset":offset,"byteLength":stored_length}));
        let image_index = images.len();
        images.push(json!({"bufferView":view_index,"mimeType":"image/png","name":output.name}));
        textures.push(json!({"source":image_index}));
    }
    root.insert("images".to_owned(), Value::Array(images));
    root.insert("textures".to_owned(), Value::Array(textures));
    let materials = root
        .get_mut("materials")
        .and_then(Value::as_array_mut)
        .unwrap();
    for material in materials {
        material["pbrMetallicRoughness"]["baseColorTexture"] = json!({"index":0});
        material["normalTexture"] = json!({"index":1});
        material["pbrMetallicRoughness"]["metallicRoughnessTexture"] = json!({"index":2});
        material["occlusionTexture"] = json!({"index":3});
        material["emissiveTexture"] = json!({"index":4});
        material["extras"]["forgecad"]["curvature_texture_index"] = Value::from(5u64);
    }
    let output_receipts = outputs
        .iter()
        .map(|output| {
            json!({
                "texture_id":output.name,
                "sha256":hash_bytes(&output.bytes),
                "semantic":output.semantic,
                "color_space":output.color_space,
                "normal_convention":output.normal_convention,
                "source_geometric_bake_png_sha256":output.source_sha256,
            })
        })
        .collect::<Vec<_>>();
    let mut build = json!({
        "schema_version":"ProductionWeaponHeroMaterialBuild@1",
        "algorithm":ALGORITHM_ID,
        "worker_algorithm_sha256":hash_bytes(ALGORITHM_ID.as_bytes()),
        "low_artifact_sha256":low_hash,
        "geometric_bake_canonical_sha256":bake_hash,
        "resolution":2048,
        "normal_convention":PRODUCTION_WEAPON_GEOMETRIC_BAKE_NORMAL_CONVENTION,
        "padding_texels":PADDING_TEXELS,
        "source_covered_pixels":source_covered_pixels,
        "padded_covered_pixels":padded_covered_pixels,
        "embedded_only":true,
        "external_uri":false,
        "outputs":output_receipts,
        "canonical_sha256":"",
    });
    build["canonical_sha256"] = Value::String(super::canonical_hash(&build));
    root["extras"]["forgecad"]["hero_material_build"] = build;
    root["extras"]["forgecad"]["texture_count"] = Value::from(outputs.len() as u64);
    root["extras"]["forgecad"]["uv_atlas"]["resolution"] = Value::from(2048u64);
    root["extras"]["forgecad"]["uv_atlas"]["padding_texels"] = Value::from(PADDING_TEXELS as u64);
    root.get_mut("buffers")
        .and_then(Value::as_array_mut)
        .and_then(|buffers| buffers.first_mut())
        .ok_or_else(|| {
            GeometryError::Invalid("Hero material GLB buffer declaration is missing".to_owned())
        })?["byteLength"] = Value::from(document.binary.len() as u64);
    Ok(())
}

fn hero_material(zone: &str) -> Value {
    let (base, metallic, roughness, emissive) = match zone {
        "zone-white-shell" => (
            json!([0.78, 0.82, 0.88, 1.0]),
            0.50,
            0.38,
            json!([0.0, 0.0, 0.0]),
        ),
        "zone-black-mechanical" => (
            json!([0.035, 0.045, 0.06, 1.0]),
            0.72,
            0.52,
            json!([0.0, 0.0, 0.0]),
        ),
        "zone-gold-accent" => (
            json!([0.86, 0.48, 0.08, 1.0]),
            0.92,
            0.28,
            json!([0.0, 0.0, 0.0]),
        ),
        "zone-amber-emissive" => (
            json!([0.20, 0.055, 0.008, 1.0]),
            0.12,
            0.34,
            json!([1.0, 0.18, 0.02]),
        ),
        _ => unreachable!("closed Hero material zone"),
    };
    json!({
        "name":zone,
        "pbrMetallicRoughness":{"baseColorFactor":base,"metallicFactor":metallic,"roughnessFactor":roughness},
        "emissiveFactor":emissive,
        "extras":{"forgecad":{"material_zone_id":zone}}
    })
}

fn write_glb(mut document: GlbDocument) -> Result<Vec<u8>, GeometryError> {
    while document.binary.len() % 4 != 0 {
        document.binary.push(0);
    }
    document.root["buffers"][0]["byteLength"] = Value::from(document.binary.len() as u64);
    let mut json_bytes = serde_json::to_vec(&document.root).map_err(|error| {
        GeometryError::Invalid(format!(
            "Hero material GLB JSON serialization failed: {error}"
        ))
    })?;
    while json_bytes.len() % 4 != 0 {
        json_bytes.push(b' ');
    }
    let total = 12usize
        .checked_add(8 + json_bytes.len())
        .and_then(|value| value.checked_add(8 + document.binary.len()))
        .and_then(|value| u32::try_from(value).ok())
        .ok_or_else(|| {
            GeometryError::Invalid("Hero material GLB output length overflowed".to_owned())
        })?;
    let mut output = Vec::with_capacity(total as usize);
    output.extend_from_slice(b"glTF");
    output.extend_from_slice(&2u32.to_le_bytes());
    output.extend_from_slice(&total.to_le_bytes());
    output.extend_from_slice(&(json_bytes.len() as u32).to_le_bytes());
    output.extend_from_slice(&0x4e4f534au32.to_le_bytes());
    output.extend_from_slice(&json_bytes);
    output.extend_from_slice(&(document.binary.len() as u32).to_le_bytes());
    output.extend_from_slice(&0x004e4942u32.to_le_bytes());
    output.extend_from_slice(&document.binary);
    Ok(output)
}

fn hash_bytes(bytes: &[u8]) -> String {
    let mut hash = Sha256::new();
    hash.update(bytes);
    format!("{:x}", hash.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_dilation_copies_all_geometric_channels_in_lockstep() {
        let mut normal = vec![128u8; 3 * 3 * 3];
        let mut ao = vec![255u8; 3 * 3];
        let mut curvature = vec![0u8; 3 * 3];
        let mut covered = vec![false; 3 * 3];
        covered[4] = true;
        normal[12..15].copy_from_slice(&[10, 20, 30]);
        ao[4] = 90;
        curvature[4] = 70;
        dilate_bake_maps(&mut normal, &mut ao, &mut curvature, &mut covered, 3, 1);
        assert_eq!(covered.iter().filter(|value| **value).count(), 5);
        for index in [1usize, 3, 4, 5, 7] {
            assert_eq!(&normal[index * 3..index * 3 + 3], &[10, 20, 30]);
            assert_eq!(ao[index], 90);
            assert_eq!(curvature[index], 70);
        }
    }

    #[test]
    fn closed_hero_material_zones_have_distinct_layers() {
        let white = hero_material("zone-white-shell");
        let black = hero_material("zone-black-mechanical");
        let gold = hero_material("zone-gold-accent");
        let amber = hero_material("zone-amber-emissive");
        assert_ne!(white["pbrMetallicRoughness"], black["pbrMetallicRoughness"]);
        assert_ne!(black["pbrMetallicRoughness"], gold["pbrMetallicRoughness"]);
        assert_eq!(amber["emissiveFactor"], json!([1.0, 0.18, 0.02]));
        assert_eq!(white["emissiveFactor"], json!([0.0, 0.0, 0.0]));
    }
}
