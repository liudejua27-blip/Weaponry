use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::{semantic_sha256, CoreError, CoreResult};

const GLB_JSON_CHUNK: u32 = 0x4e4f_534a;
const GLB_BINARY_CHUNK: u32 = 0x004e_4942;
const MAX_GLB_BYTES: usize = 128 * 1024 * 1024;
const RUNTIME_MANIFEST_VERSION: &str = "ShapeProgramRuntimeManifest@1";
const REQUIRED_PBR_ROLES: [&str; 5] = [
    "base_color",
    "metallic_roughness",
    "normal",
    "occlusion",
    "emissive",
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ForgeCadGlbReadback {
    pub artifact_profile_id: String,
    pub artifact_profile_sha256: String,
    pub artifact_profile: Value,
    pub runtime_manifest_version: String,
    pub glb_sha256: String,
    pub glb_byte_size: u64,
    pub triangle_count: u64,
    pub bounds_mm: Vec<f64>,
    pub mesh_count: u64,
    pub primitive_count: u64,
    pub material_count: u64,
    pub uv0_primitive_count: u64,
    pub normal_primitive_count: u64,
    pub tangent_primitive_count: u64,
    pub closed_manifold: bool,
    pub surface_provenance_present: bool,
    pub visual_texture_set_count: u64,
    pub visual_texture_map_count: u64,
}

pub fn verify_forgecad_glb(
    bytes: &[u8],
    expected_profile_id: Option<&str>,
) -> CoreResult<ForgeCadGlbReadback> {
    let chunks = parse_glb(bytes)?;
    let document: Value = serde_json::from_slice(chunks.json).map_err(|_| {
        invalid(
            "FORGECAD_GLB_JSON_INVALID",
            "Binary glTF JSON chunk cannot be decoded.",
        )
    })?;
    let profile = document
        .get("extras")
        .and_then(|extras| extras.get("forgecad_geometry_artifact_profile"))
        .and_then(Value::as_object)
        .cloned()
        .ok_or_else(|| {
            invalid(
                "GEOMETRY_PROFILE_MISSING",
                "GLB is missing the code-owned geometry artifact profile.",
            )
        })?;
    let artifact_profile_id = profile
        .get("artifact_profile_id")
        .and_then(Value::as_str)
        .filter(|value| matches!(*value, "interactive_preview" | "production_concept"))
        .ok_or_else(|| {
            invalid(
                "GEOMETRY_PROFILE_INVALID",
                "GLB artifact profile identity is invalid.",
            )
        })?
        .to_string();
    if expected_profile_id.is_some_and(|expected| expected != artifact_profile_id) {
        return Err(CoreError::conflict(
            "GEOMETRY_PROFILE_MISMATCH",
            "GLB artifact profile does not match the requested product artifact role.",
        ));
    }
    let artifact_profile_sha256 = required_sha(
        profile.get("profile_sha256"),
        "GEOMETRY_PROFILE_INVALID",
        "GLB artifact profile SHA-256 is invalid.",
    )?;
    let mut unsigned_profile = profile.clone();
    unsigned_profile.remove("profile_sha256");
    if Value::Object(unsigned_profile.clone()) != geometry_profile_contract(&artifact_profile_id)
        || semantic_sha256(&unsigned_profile)? != artifact_profile_sha256
    {
        return Err(invalid(
            "GEOMETRY_PROFILE_INVALID",
            "GLB artifact profile does not match the code-owned manifest and semantic hash.",
        ));
    }

    validate_feature_history(&document)?;
    let accessors = required_array(&document, "accessors", "GLB has no accessors.")?;
    let views = required_array(&document, "bufferViews", "GLB has no buffer views.")?;
    let meshes = required_array(&document, "meshes", "GLB has no meshes.")?;
    let materials = document
        .get("materials")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("FORGECAD_PBR_INVALID", "GLB has no material table."))?;
    let textures = document
        .get("textures")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("FORGECAD_PBR_INVALID", "GLB has no texture table."))?;
    let images = document
        .get("images")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("FORGECAD_PBR_INVALID", "GLB has no embedded image table."))?;

    let mut triangle_count = 0u64;
    let mut primitive_count = 0u64;
    let mut uv0_primitive_count = 0u64;
    let mut normal_primitive_count = 0u64;
    let mut tangent_primitive_count = 0u64;
    let mut minimum = [f64::INFINITY; 3];
    let mut maximum = [f64::NEG_INFINITY; 3];
    let mut closed_manifold = true;
    let mut surface_provenance_present = true;
    let mut surface_provenance_structure_missing = false;
    let mut surface_provenance_face_count_mismatch = false;
    let mut used_materials = BTreeSet::new();

    for mesh in meshes {
        let primitives = mesh
            .get("primitives")
            .and_then(Value::as_array)
            .filter(|items| !items.is_empty())
            .ok_or_else(|| {
                invalid(
                    "FORGECAD_GLB_GEOMETRY_INVALID",
                    "GLB mesh has no triangle primitives.",
                )
            })?;
        for primitive in primitives {
            if primitive.get("mode").and_then(Value::as_u64).unwrap_or(4) != 4 {
                return Err(invalid(
                    "FORGECAD_GLB_GEOMETRY_INVALID",
                    "GLB primitive is not a triangle list.",
                ));
            }
            primitive_count += 1;
            let index_accessor = primitive
                .get("indices")
                .and_then(Value::as_u64)
                .and_then(|index| accessors.get(index as usize))
                .ok_or_else(|| {
                    invalid(
                        "FORGECAD_GLB_GEOMETRY_INVALID",
                        "GLB primitive index accessor is invalid.",
                    )
                })?;
            let indices = read_index_accessor(index_accessor, views, chunks.binary)?;
            if indices.is_empty() || indices.len() % 3 != 0 {
                return Err(invalid(
                    "FORGECAD_GLB_GEOMETRY_INVALID",
                    "GLB index accessor is not a non-empty triangle list.",
                ));
            }
            triangle_count += (indices.len() / 3) as u64;

            let attributes = primitive
                .get("attributes")
                .and_then(Value::as_object)
                .ok_or_else(|| {
                    invalid(
                        "FORGECAD_GLB_GEOMETRY_INVALID",
                        "GLB primitive attributes are missing.",
                    )
                })?;
            let position_accessor = attributes
                .get("POSITION")
                .and_then(Value::as_u64)
                .and_then(|index| accessors.get(index as usize))
                .ok_or_else(|| {
                    invalid(
                        "FORGECAD_GLB_GEOMETRY_INVALID",
                        "GLB primitive POSITION accessor is invalid.",
                    )
                })?;
            let positions = read_position_accessor(position_accessor, views, chunks.binary)?;
            closed_manifold &= triangle_edges_are_closed(&indices, &positions);
            update_accessor_bounds(position_accessor, &mut minimum, &mut maximum)?;
            uv0_primitive_count += u64::from(attributes.get("TEXCOORD_0").is_some());
            normal_primitive_count += u64::from(attributes.get("NORMAL").is_some());
            tangent_primitive_count += u64::from(attributes.get("TANGENT").is_some());
            let material_index = primitive
                .get("material")
                .and_then(Value::as_u64)
                .filter(|index| (*index as usize) < materials.len())
                .ok_or_else(|| {
                    invalid(
                        "FORGECAD_PBR_INVALID",
                        "GLB primitive material reference is invalid.",
                    )
                })? as usize;
            used_materials.insert(material_index);

            let source_face_ids = attributes
                .get("_FORGECAD_SOURCE_FACE_ID")
                .and_then(Value::as_u64)
                .and_then(|index| accessors.get(index as usize))
                .map(|accessor| read_scalar_float_accessor(accessor, views, chunks.binary))
                .transpose()?;
            let extras = primitive.get("extras").and_then(Value::as_object);
            let legacy_source_face_count = extras
                .and_then(|extras| extras.get("forgecad_source_face_ids"))
                .and_then(Value::as_array)
                .map(Vec::len);
            let source_face_provenance_valid = match source_face_ids.as_ref() {
                Some(values) => {
                    values.len() == positions.len()
                        && indices.chunks_exact(3).all(|triangle| {
                            let Some(first) = values.get(triangle[0] as usize) else {
                                return false;
                            };
                            triangle
                                .iter()
                                .all(|index| values.get(*index as usize) == Some(first))
                        })
                }
                // Existing Core-generated fixtures encode the same bounded
                // provenance contract in extras; keep that canonical fixture
                // form readable while preferring the current vertex accessor.
                None => legacy_source_face_count == Some(indices.len() / 3),
            };
            let structure_present = extras.is_some_and(|extras| {
                extras
                    .get("forgecad_feature_node_id")
                    .and_then(Value::as_str)
                    .is_some()
                    && extras
                        .get("forgecad_material_zone_id")
                        .and_then(Value::as_str)
                        .is_some()
                    && extras
                        .get("forgecad_surface_ranges")
                        .and_then(Value::as_array)
                        .is_some()
            });
            surface_provenance_structure_missing |= !structure_present
                || (source_face_ids.is_none() && legacy_source_face_count.is_none());
            surface_provenance_face_count_mismatch |= !source_face_provenance_valid;
            surface_provenance_present &= structure_present && source_face_provenance_valid;
        }
    }
    if triangle_count == 0
        || minimum
            .iter()
            .chain(maximum.iter())
            .any(|value| !value.is_finite())
    {
        return Err(invalid(
            "FORGECAD_GLB_GEOMETRY_INVALID",
            "GLB readback has no finite triangle bounds.",
        ));
    }
    if uv0_primitive_count != primitive_count
        || normal_primitive_count != primitive_count
        || tangent_primitive_count != primitive_count
    {
        return Err(invalid(
            "FORGECAD_PBR_ATTRIBUTES_MISSING",
            "Every ForgeCAD primitive must carry UV0, normal and tangent attributes.",
        ));
    }
    if !closed_manifold {
        return Err(invalid(
            "FORGECAD_SURFACE_CLOSED_MANIFOLD_FAILED",
            "ForgeCAD GLB failed closed-manifold readback.",
        ));
    }
    if !surface_provenance_present {
        return Err(invalid(
            if surface_provenance_structure_missing {
                "FORGECAD_SURFACE_PROVENANCE_FIELDS_MISSING"
            } else if surface_provenance_face_count_mismatch {
                "FORGECAD_SURFACE_PROVENANCE_FACE_COUNT_MISMATCH"
            } else {
                "FORGECAD_SURFACE_PROVENANCE_MISSING"
            },
            "ForgeCAD GLB is missing valid surface provenance readback.",
        ));
    }
    let (texture_set_count, texture_map_count) = validate_used_pbr_materials(
        &artifact_profile_id,
        &used_materials,
        materials,
        textures,
        images,
        views,
        chunks.binary,
    )?;
    let bounds_mm = (0..3)
        .map(|axis| ((maximum[axis] - minimum[axis]) * 1_000.0 * 10_000.0).round() / 10_000.0)
        .collect::<Vec<_>>();
    Ok(ForgeCadGlbReadback {
        artifact_profile_id,
        artifact_profile_sha256,
        artifact_profile: Value::Object(profile),
        runtime_manifest_version: RUNTIME_MANIFEST_VERSION.to_string(),
        glb_sha256: hex_sha256(bytes),
        glb_byte_size: bytes.len() as u64,
        triangle_count,
        bounds_mm,
        mesh_count: meshes.len() as u64,
        primitive_count,
        material_count: materials.len() as u64,
        uv0_primitive_count,
        normal_primitive_count,
        tangent_primitive_count,
        closed_manifold,
        surface_provenance_present,
        visual_texture_set_count: texture_set_count,
        visual_texture_map_count: texture_map_count,
    })
}

struct GlbChunks<'a> {
    json: &'a [u8],
    binary: &'a [u8],
}

fn parse_glb(bytes: &[u8]) -> CoreResult<GlbChunks<'_>> {
    if bytes.len() < 20 || bytes.len() > MAX_GLB_BYTES || bytes.get(..4) != Some(b"glTF") {
        return Err(invalid(
            "FORGECAD_GLB_INVALID",
            "Artifact is not a bounded binary glTF payload.",
        ));
    }
    let version = read_u32_le(bytes, 4)?;
    let declared_length = read_u32_le(bytes, 8)? as usize;
    if version != 2 || declared_length != bytes.len() {
        return Err(invalid(
            "FORGECAD_GLB_INVALID",
            "Binary glTF header version or byte length is invalid.",
        ));
    }
    let mut cursor = 12usize;
    let mut json_chunk = None;
    let mut binary_chunk = None;
    while cursor < bytes.len() {
        let length = read_u32_le(bytes, cursor)? as usize;
        let kind = read_u32_le(bytes, cursor + 4)?;
        let start = cursor.checked_add(8).ok_or_else(|| {
            invalid(
                "FORGECAD_GLB_INVALID",
                "Binary glTF chunk offset overflowed.",
            )
        })?;
        let end = start
            .checked_add(length)
            .filter(|end| *end <= bytes.len())
            .ok_or_else(|| {
                invalid(
                    "FORGECAD_GLB_INVALID",
                    "Binary glTF chunk extends beyond its payload.",
                )
            })?;
        match kind {
            GLB_JSON_CHUNK if json_chunk.is_none() => json_chunk = Some(&bytes[start..end]),
            GLB_BINARY_CHUNK if binary_chunk.is_none() => binary_chunk = Some(&bytes[start..end]),
            _ => {}
        }
        cursor = end;
    }
    if cursor != bytes.len() {
        return Err(invalid(
            "FORGECAD_GLB_INVALID",
            "Binary glTF chunks do not consume the declared payload.",
        ));
    }
    Ok(GlbChunks {
        json: json_chunk.ok_or_else(|| {
            invalid(
                "FORGECAD_GLB_INVALID",
                "Binary glTF is missing its JSON chunk.",
            )
        })?,
        binary: binary_chunk.ok_or_else(|| {
            invalid(
                "FORGECAD_GLB_INVALID",
                "Binary glTF is missing its binary chunk.",
            )
        })?,
    })
}

fn validate_feature_history(document: &Value) -> CoreResult<()> {
    let history = document
        .get("extras")
        .and_then(|extras| extras.get("forgecad_feature_history"))
        .and_then(Value::as_array)
        .filter(|history| !history.is_empty())
        .ok_or_else(|| {
            invalid(
                "FORGECAD_FEATURE_HISTORY_MISSING",
                "GLB is missing immutable ShapeProgram feature history.",
            )
        })?;
    for feature in history {
        if feature
            .get("runtime_manifest_version")
            .and_then(Value::as_str)
            != Some(RUNTIME_MANIFEST_VERSION)
            || feature.get("node_id").and_then(Value::as_str).is_none()
            || feature
                .get("result_sha256")
                .and_then(Value::as_str)
                .is_none_or(|value| !is_sha256(value))
        {
            return Err(invalid(
                "FORGECAD_FEATURE_HISTORY_INVALID",
                "GLB feature history is stale or incomplete.",
            ));
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_used_pbr_materials(
    profile_id: &str,
    used_materials: &BTreeSet<usize>,
    materials: &[Value],
    textures: &[Value],
    images: &[Value],
    views: &[Value],
    binary: &[u8],
) -> CoreResult<(u64, u64)> {
    let expected_dimension = if profile_id == "production_concept" {
        1024
    } else {
        128
    };
    let expected_builtin_suffix = if profile_id == "production_concept" {
        "_builtin_v4"
    } else {
        "_builtin_v3"
    };
    let expected_profile_version = if profile_id == "production_concept" {
        "v4"
    } else {
        "v3"
    };
    let mut map_count = 0u64;
    for material_index in used_materials {
        let material = materials.get(*material_index).ok_or_else(|| {
            invalid(
                "FORGECAD_PBR_INVALID",
                "Used material index is outside the material table.",
            )
        })?;
        if material
            .pointer("/extras/forgecad_unused_material_placeholder")
            .and_then(Value::as_bool)
            == Some(true)
        {
            return Err(invalid(
                "FORGECAD_PBR_INVALID",
                "Used geometry cannot reference an unused material placeholder.",
            ));
        }
        let texture_set_id = material
            .pointer("/extras/forgecad_visual_texture_set_id")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                invalid(
                    "FORGECAD_TEXTURE_CONTRACT_STALE",
                    "Used material does not carry the current profile texture-set identity.",
                )
            })?;
        let builtin_identity = texture_set_id.ends_with(expected_builtin_suffix);
        let adornment_hash = material
            .pointer("/extras/forgecad_surface_adornment_sha256")
            .and_then(Value::as_str);
        let adornment_identity = adornment_hash.is_some_and(|hash| {
            is_sha256(hash)
                && texture_set_id
                    == format!("vtexset_a005_{}_{}", &hash[..32], expected_profile_version)
                && material
                    .pointer("/extras/forgecad_texture_material_id")
                    .and_then(Value::as_str)
                    == Some(format!("mat_a005_{}", &hash[..32]).as_str())
                && material
                    .pointer("/extras/forgecad_base_material_id")
                    .and_then(Value::as_str)
                    .is_some_and(|value| value.starts_with("mat_"))
                && material
                    .pointer("/extras/forgecad_surface_adornment")
                    .and_then(Value::as_object)
                    .is_some()
                && material
                    .pointer("/extras/forgecad_visual_only")
                    .and_then(Value::as_bool)
                    == Some(true)
        });
        if !builtin_identity && !adornment_identity {
            return Err(invalid(
                "FORGECAD_TEXTURE_CONTRACT_STALE",
                "Used material does not match the current built-in or A005 texture-set identity.",
            ));
        }
        let slots = BTreeMap::from([
            (
                "base_color",
                material.pointer("/pbrMetallicRoughness/baseColorTexture/index"),
            ),
            (
                "metallic_roughness",
                material.pointer("/pbrMetallicRoughness/metallicRoughnessTexture/index"),
            ),
            ("normal", material.pointer("/normalTexture/index")),
            ("occlusion", material.pointer("/occlusionTexture/index")),
            ("emissive", material.pointer("/emissiveTexture/index")),
        ]);
        let mut seen_texture_indices = BTreeSet::new();
        for role in REQUIRED_PBR_ROLES {
            let texture_index = slots
                .get(role)
                .and_then(|value| *value)
                .and_then(Value::as_u64)
                .filter(|index| (*index as usize) < textures.len())
                .ok_or_else(|| {
                    invalid(
                        "FORGECAD_PBR_INVALID",
                        "Used material is missing one required PBR texture slot.",
                    )
                })? as usize;
            if !seen_texture_indices.insert(texture_index) {
                return Err(invalid(
                    "FORGECAD_PBR_INVALID",
                    "Required PBR channels must keep distinct texture identities.",
                ));
            }
            let image_index = textures[texture_index]
                .get("source")
                .and_then(Value::as_u64)
                .filter(|index| (*index as usize) < images.len())
                .ok_or_else(|| {
                    invalid(
                        "FORGECAD_PBR_INVALID",
                        "PBR texture source image is invalid.",
                    )
                })? as usize;
            validate_texture_image(
                &images[image_index],
                role,
                texture_set_id,
                expected_dimension,
                views,
                binary,
            )?;
            map_count += 1;
        }
    }
    Ok((used_materials.len() as u64, map_count))
}

fn validate_texture_image(
    image: &Value,
    expected_role: &str,
    _texture_set_id: &str,
    expected_dimension: u32,
    views: &[Value],
    binary: &[u8],
) -> CoreResult<()> {
    if image.get("mimeType").and_then(Value::as_str) != Some("image/png") {
        return Err(invalid(
            "FORGECAD_TEXTURE_INVALID",
            "ForgeCAD PBR textures must be embedded PNG images.",
        ));
    }
    let manifest = image
        .pointer("/extras/forgecad_visual_texture")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            invalid(
                "FORGECAD_TEXTURE_INVALID",
                "Embedded PBR image is missing its visual texture manifest.",
            )
        })?;
    if manifest.get("texture_role").and_then(Value::as_str) != Some(expected_role)
        || manifest.get("mime_type").and_then(Value::as_str) != Some("image/png")
        || manifest.get("width").and_then(Value::as_u64) != Some(expected_dimension as u64)
        || manifest.get("height").and_then(Value::as_u64) != Some(expected_dimension as u64)
        || manifest.get("source").and_then(Value::as_str) != Some("forgecad_builtin")
        || manifest.get("license").and_then(Value::as_str) != Some("not_applicable")
        || manifest.get("fallback").and_then(Value::as_str) != Some("none")
        || manifest.get("visual_only").and_then(Value::as_bool) != Some(true)
    {
        return Err(invalid(
            "FORGECAD_TEXTURE_CONTRACT_STALE",
            "Embedded PBR texture manifest does not match the current visual contract.",
        ));
    }
    let view_index = image
        .get("bufferView")
        .and_then(Value::as_u64)
        .filter(|index| (*index as usize) < views.len())
        .ok_or_else(|| {
            invalid(
                "FORGECAD_TEXTURE_INVALID",
                "Embedded PBR image buffer view is invalid.",
            )
        })? as usize;
    let payload = buffer_view_bytes(&views[view_index], binary)?;
    if manifest.get("byte_size").and_then(Value::as_u64) != Some(payload.len() as u64)
        || required_sha(
            manifest.get("sha256"),
            "FORGECAD_TEXTURE_INVALID",
            "Embedded PBR texture SHA-256 is invalid.",
        )? != hex_sha256(payload)
    {
        return Err(invalid(
            "FORGECAD_TEXTURE_HASH_MISMATCH",
            "Embedded PBR texture bytes do not match their manifest.",
        ));
    }
    let (width, height) = png_dimensions(payload)?;
    if width != expected_dimension || height != expected_dimension {
        return Err(invalid(
            "FORGECAD_TEXTURE_DIMENSION_MISMATCH",
            "Embedded PBR PNG dimensions do not match the artifact profile.",
        ));
    }
    Ok(())
}

fn required_array<'a>(document: &'a Value, key: &str, message: &str) -> CoreResult<&'a [Value]> {
    document
        .get(key)
        .and_then(Value::as_array)
        .filter(|items| !items.is_empty())
        .map(Vec::as_slice)
        .ok_or_else(|| invalid("FORGECAD_GLB_INVALID", message))
}

fn buffer_view_bytes<'a>(view: &Value, binary: &'a [u8]) -> CoreResult<&'a [u8]> {
    let offset = view.get("byteOffset").and_then(Value::as_u64).unwrap_or(0) as usize;
    let length = view
        .get("byteLength")
        .and_then(Value::as_u64)
        .filter(|length| *length > 0)
        .ok_or_else(|| {
            invalid(
                "FORGECAD_GLB_INVALID",
                "GLB buffer view has an invalid length.",
            )
        })? as usize;
    let end = offset
        .checked_add(length)
        .filter(|end| *end <= binary.len())
        .ok_or_else(|| {
            invalid(
                "FORGECAD_GLB_INVALID",
                "GLB buffer view extends beyond the binary chunk.",
            )
        })?;
    Ok(&binary[offset..end])
}

fn read_index_accessor(accessor: &Value, views: &[Value], binary: &[u8]) -> CoreResult<Vec<u32>> {
    let count = accessor
        .get("count")
        .and_then(Value::as_u64)
        .filter(|count| *count > 0 && *count <= 3_000_000)
        .ok_or_else(|| {
            invalid(
                "FORGECAD_GLB_GEOMETRY_INVALID",
                "GLB index accessor count is invalid.",
            )
        })? as usize;
    let component_type = accessor
        .get("componentType")
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            invalid(
                "FORGECAD_GLB_GEOMETRY_INVALID",
                "GLB index component type is missing.",
            )
        })?;
    let component_size = match component_type {
        5123 => 2usize,
        5125 => 4usize,
        _ => {
            return Err(invalid(
                "FORGECAD_GLB_GEOMETRY_INVALID",
                "GLB indices must use unsigned 16-bit or 32-bit components.",
            ))
        }
    };
    let view = accessor
        .get("bufferView")
        .and_then(Value::as_u64)
        .and_then(|index| views.get(index as usize))
        .ok_or_else(|| {
            invalid(
                "FORGECAD_GLB_GEOMETRY_INVALID",
                "GLB index buffer view is invalid.",
            )
        })?;
    let view_offset = view.get("byteOffset").and_then(Value::as_u64).unwrap_or(0) as usize;
    let accessor_offset = accessor
        .get("byteOffset")
        .and_then(Value::as_u64)
        .unwrap_or(0) as usize;
    let stride = view
        .get("byteStride")
        .and_then(Value::as_u64)
        .unwrap_or(component_size as u64) as usize;
    if stride < component_size {
        return Err(invalid(
            "FORGECAD_GLB_GEOMETRY_INVALID",
            "GLB index accessor stride is invalid.",
        ));
    }
    let start = view_offset.checked_add(accessor_offset).ok_or_else(|| {
        invalid(
            "FORGECAD_GLB_GEOMETRY_INVALID",
            "GLB index offset overflowed.",
        )
    })?;
    let mut result = Vec::with_capacity(count);
    for ordinal in 0..count {
        let offset = start
            .checked_add(ordinal.checked_mul(stride).ok_or_else(|| {
                invalid(
                    "FORGECAD_GLB_GEOMETRY_INVALID",
                    "GLB index stride overflowed.",
                )
            })?)
            .ok_or_else(|| {
                invalid(
                    "FORGECAD_GLB_GEOMETRY_INVALID",
                    "GLB index offset overflowed.",
                )
            })?;
        let value = match component_type {
            5123 => {
                let raw: [u8; 2] = binary
                    .get(offset..offset + 2)
                    .and_then(|slice| slice.try_into().ok())
                    .ok_or_else(|| {
                        invalid(
                            "FORGECAD_GLB_GEOMETRY_INVALID",
                            "GLB index data is truncated.",
                        )
                    })?;
                u16::from_le_bytes(raw) as u32
            }
            5125 => read_u32_le(binary, offset)?,
            _ => unreachable!(),
        };
        result.push(value);
    }
    Ok(result)
}

fn read_position_accessor(
    accessor: &Value,
    views: &[Value],
    binary: &[u8],
) -> CoreResult<Vec<[i64; 3]>> {
    let count = accessor
        .get("count")
        .and_then(Value::as_u64)
        .filter(|count| *count > 0 && *count <= 3_000_000)
        .ok_or_else(|| {
            invalid(
                "FORGECAD_GLB_GEOMETRY_INVALID",
                "GLB POSITION count is invalid.",
            )
        })? as usize;
    if accessor.get("componentType").and_then(Value::as_u64) != Some(5126)
        || accessor.get("type").and_then(Value::as_str) != Some("VEC3")
    {
        return Err(invalid(
            "FORGECAD_GLB_GEOMETRY_INVALID",
            "GLB POSITION accessor must be a float VEC3.",
        ));
    }
    let view = accessor
        .get("bufferView")
        .and_then(Value::as_u64)
        .and_then(|index| views.get(index as usize))
        .ok_or_else(|| {
            invalid(
                "FORGECAD_GLB_GEOMETRY_INVALID",
                "GLB POSITION buffer view is invalid.",
            )
        })?;
    let view_offset = view.get("byteOffset").and_then(Value::as_u64).unwrap_or(0) as usize;
    let accessor_offset = accessor
        .get("byteOffset")
        .and_then(Value::as_u64)
        .unwrap_or(0) as usize;
    let stride = view.get("byteStride").and_then(Value::as_u64).unwrap_or(12) as usize;
    if stride < 12 {
        return Err(invalid(
            "FORGECAD_GLB_GEOMETRY_INVALID",
            "GLB POSITION accessor stride is invalid.",
        ));
    }
    let start = view_offset.checked_add(accessor_offset).ok_or_else(|| {
        invalid(
            "FORGECAD_GLB_GEOMETRY_INVALID",
            "GLB POSITION offset overflowed.",
        )
    })?;
    let mut positions = Vec::with_capacity(count);
    for ordinal in 0..count {
        let offset = start
            .checked_add(ordinal.checked_mul(stride).ok_or_else(|| {
                invalid(
                    "FORGECAD_GLB_GEOMETRY_INVALID",
                    "GLB POSITION stride overflowed.",
                )
            })?)
            .ok_or_else(|| {
                invalid(
                    "FORGECAD_GLB_GEOMETRY_INVALID",
                    "GLB POSITION offset overflowed.",
                )
            })?;
        let mut position = [0_i64; 3];
        for (axis, value) in position.iter_mut().enumerate() {
            let raw = binary
                .get(offset + axis * 4..offset + axis * 4 + 4)
                .and_then(|slice| slice.try_into().ok())
                .ok_or_else(|| {
                    invalid(
                        "FORGECAD_GLB_GEOMETRY_INVALID",
                        "GLB POSITION data is truncated.",
                    )
                })?;
            let float = f32::from_le_bytes(raw);
            if !float.is_finite() {
                return Err(invalid(
                    "FORGECAD_GLB_GEOMETRY_INVALID",
                    "GLB POSITION contains a non-finite value.",
                ));
            }
            // The Worker defines topology after rounding POSITION values to
            // eight decimals. This welds split normal/UV vertices, normalizes
            // signed zero and absorbs harmless trig seam noise without
            // weakening the closed-edge count.
            *value = ((float as f64) * 100_000_000.0).round() as i64;
        }
        positions.push(position);
    }
    Ok(positions)
}

fn read_scalar_float_accessor(
    accessor: &Value,
    views: &[Value],
    binary: &[u8],
) -> CoreResult<Vec<f32>> {
    let count = accessor
        .get("count")
        .and_then(Value::as_u64)
        .filter(|count| *count > 0 && *count <= 3_000_000)
        .ok_or_else(|| {
            invalid(
                "FORGECAD_GLB_GEOMETRY_INVALID",
                "GLB provenance accessor count is invalid.",
            )
        })? as usize;
    if accessor.get("componentType").and_then(Value::as_u64) != Some(5126)
        || accessor.get("type").and_then(Value::as_str) != Some("SCALAR")
    {
        return Err(invalid(
            "FORGECAD_SURFACE_PROVENANCE_INVALID",
            "GLB source-face provenance accessor must be a float scalar.",
        ));
    }
    let view = accessor
        .get("bufferView")
        .and_then(Value::as_u64)
        .and_then(|index| views.get(index as usize))
        .ok_or_else(|| {
            invalid(
                "FORGECAD_SURFACE_PROVENANCE_INVALID",
                "GLB source-face provenance buffer view is invalid.",
            )
        })?;
    let view_offset = view.get("byteOffset").and_then(Value::as_u64).unwrap_or(0) as usize;
    let accessor_offset = accessor
        .get("byteOffset")
        .and_then(Value::as_u64)
        .unwrap_or(0) as usize;
    let stride = view.get("byteStride").and_then(Value::as_u64).unwrap_or(4) as usize;
    if stride < 4 {
        return Err(invalid(
            "FORGECAD_SURFACE_PROVENANCE_INVALID",
            "GLB source-face provenance accessor stride is invalid.",
        ));
    }
    let start = view_offset.checked_add(accessor_offset).ok_or_else(|| {
        invalid(
            "FORGECAD_SURFACE_PROVENANCE_INVALID",
            "GLB source-face provenance offset overflowed.",
        )
    })?;
    let mut values = Vec::with_capacity(count);
    for ordinal in 0..count {
        let offset = start
            .checked_add(ordinal.checked_mul(stride).ok_or_else(|| {
                invalid(
                    "FORGECAD_SURFACE_PROVENANCE_INVALID",
                    "GLB source-face provenance stride overflowed.",
                )
            })?)
            .ok_or_else(|| {
                invalid(
                    "FORGECAD_SURFACE_PROVENANCE_INVALID",
                    "GLB source-face provenance offset overflowed.",
                )
            })?;
        let raw = binary
            .get(offset..offset + 4)
            .and_then(|slice| slice.try_into().ok())
            .ok_or_else(|| {
                invalid(
                    "FORGECAD_SURFACE_PROVENANCE_INVALID",
                    "GLB source-face provenance data is truncated.",
                )
            })?;
        let value = f32::from_le_bytes(raw);
        if !value.is_finite() || value < 0.0 || value.fract() != 0.0 {
            return Err(invalid(
                "FORGECAD_SURFACE_PROVENANCE_INVALID",
                "GLB source-face provenance values must be finite non-negative integers.",
            ));
        }
        values.push(value);
    }
    Ok(values)
}

fn triangle_edges_are_closed(indices: &[u32], positions: &[[i64; 3]]) -> bool {
    let mut welded_vertices = BTreeMap::<[i64; 3], u32>::new();
    let mut welded_indices = Vec::with_capacity(indices.len());
    for index in indices {
        let Some(position) = positions.get(*index as usize) else {
            return false;
        };
        let next = welded_vertices.len() as u32;
        let welded = *welded_vertices.entry(*position).or_insert(next);
        welded_indices.push(welded);
    }
    let mut edges = BTreeMap::<(u32, u32), u32>::new();
    for triangle in welded_indices.chunks_exact(3) {
        if triangle[0] == triangle[1] || triangle[1] == triangle[2] || triangle[2] == triangle[0] {
            return false;
        }
        for (left, right) in [
            (triangle[0], triangle[1]),
            (triangle[1], triangle[2]),
            (triangle[2], triangle[0]),
        ] {
            let edge = if left < right {
                (left, right)
            } else {
                (right, left)
            };
            *edges.entry(edge).or_default() += 1;
        }
    }
    !edges.is_empty() && edges.values().all(|count| *count == 2)
}

fn update_accessor_bounds(
    accessor: &Value,
    minimum: &mut [f64; 3],
    maximum: &mut [f64; 3],
) -> CoreResult<()> {
    let lower = accessor
        .get("min")
        .and_then(Value::as_array)
        .filter(|values| values.len() == 3)
        .ok_or_else(|| {
            invalid(
                "FORGECAD_GLB_GEOMETRY_INVALID",
                "GLB POSITION accessor is missing finite minimum bounds.",
            )
        })?;
    let upper = accessor
        .get("max")
        .and_then(Value::as_array)
        .filter(|values| values.len() == 3)
        .ok_or_else(|| {
            invalid(
                "FORGECAD_GLB_GEOMETRY_INVALID",
                "GLB POSITION accessor is missing finite maximum bounds.",
            )
        })?;
    for axis in 0..3 {
        let lower = lower[axis]
            .as_f64()
            .filter(|value| value.is_finite())
            .ok_or_else(|| {
                invalid(
                    "FORGECAD_GLB_GEOMETRY_INVALID",
                    "GLB POSITION minimum is not finite.",
                )
            })?;
        let upper = upper[axis]
            .as_f64()
            .filter(|value| value.is_finite() && *value >= lower)
            .ok_or_else(|| {
                invalid(
                    "FORGECAD_GLB_GEOMETRY_INVALID",
                    "GLB POSITION maximum is invalid.",
                )
            })?;
        minimum[axis] = minimum[axis].min(lower);
        maximum[axis] = maximum[axis].max(upper);
    }
    Ok(())
}

fn geometry_profile_contract(profile_id: &str) -> Value {
    let production = profile_id == "production_concept";
    json!({
        "schema_version": "GeometryArtifactProfile@1",
        "artifact_profile_id": profile_id,
        "radial_segments": if production { 64 } else { 24 },
        "capsule_hemisphere_segments": if production { 14 } else { 5 },
        "smooth_loft_normals": production,
        "texture_width": if production { 1024 } else { 128 },
        "texture_height": if production { 1024 } else { 128 },
        "texture_mime_type": "image/png",
        "texture_compression": "png_deflate",
        "delivery": if production { "on_demand" } else { "interactive" },
        "triangle_budget_multiplier": if production { 6 } else { 1 },
        "max_triangle_count": if production { 250_000 } else { 100_000 },
    })
}

fn png_dimensions(bytes: &[u8]) -> CoreResult<(u32, u32)> {
    const SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
    if bytes.len() < 24 || bytes.get(..8) != Some(SIGNATURE) || bytes.get(12..16) != Some(b"IHDR") {
        return Err(invalid(
            "FORGECAD_TEXTURE_INVALID",
            "Embedded texture is not a PNG with an IHDR header.",
        ));
    }
    let width = u32::from_be_bytes(bytes[16..20].try_into().expect("validated PNG width"));
    let height = u32::from_be_bytes(bytes[20..24].try_into().expect("validated PNG height"));
    Ok((width, height))
}

fn read_u32_le(bytes: &[u8], offset: usize) -> CoreResult<u32> {
    let raw: [u8; 4] = bytes
        .get(offset..offset + 4)
        .and_then(|slice| slice.try_into().ok())
        .ok_or_else(|| invalid("FORGECAD_GLB_INVALID", "Binary glTF integer is truncated."))?;
    Ok(u32::from_le_bytes(raw))
}

fn required_sha(
    value: Option<&Value>,
    code: &'static str,
    message: &'static str,
) -> CoreResult<String> {
    value
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .map(str::to_string)
        .ok_or_else(|| invalid(code, message))
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn hex_sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn invalid(code: &'static str, message: impl Into<String>) -> CoreError {
    CoreError::invalid_data(code, message)
}

#[cfg(test)]
mod tests {
    use super::triangle_edges_are_closed;

    #[test]
    fn closed_manifold_readback_welds_split_normal_vertices() {
        let tetrahedron = [
            [0, 0, 0],
            [100_000_000, 0, 0],
            [0, 100_000_000, 0],
            [0, 0, 100_000_000],
        ];
        let positions = vec![
            tetrahedron[0],
            tetrahedron[1],
            tetrahedron[2],
            tetrahedron[0],
            tetrahedron[3],
            tetrahedron[1],
            tetrahedron[1],
            tetrahedron[3],
            tetrahedron[2],
            tetrahedron[2],
            tetrahedron[3],
            tetrahedron[0],
        ];
        let indices = (0_u32..12).collect::<Vec<_>>();
        assert!(triangle_edges_are_closed(&indices, &positions));
    }

    #[test]
    fn open_surface_stays_rejected_after_vertex_welding() {
        let positions = vec![[0, 0, 0], [100_000_000, 0, 0], [0, 100_000_000, 0]];
        assert!(!triangle_edges_are_closed(&[0, 1, 2], &positions));
    }
}
