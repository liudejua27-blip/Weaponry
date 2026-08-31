//! Strict, local readback for the Native High embedded GLB.
//!
//! This module deliberately has no Runtime, Store, Worker, filesystem, or
//! network dependency.  The Worker supplies a compact readback, but Runtime
//! still parses the bytes locally before accepting that assertion.

use super::{canonical_json_hash, sha256_hex};
use serde::de::{self, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer};
use serde_json::{json, Map, Value};
use std::collections::BTreeSet;
use std::fmt;

const GLB_MAGIC: &[u8; 4] = b"glTF";
const JSON_CHUNK: &[u8; 4] = b"JSON";
const BIN_CHUNK: &[u8; 4] = b"BIN\0";
const GLB_SCHEMA: &str = "HighMeshArtifactGlb@1";
const READBACK_SCHEMA: &str = "NativeHighGlbReadback@1";
const MAX_GLB_BYTES: usize = 256 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NativeHighGlbReadbackError(pub(crate) String);

impl fmt::Display for NativeHighGlbReadbackError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for NativeHighGlbReadbackError {}

impl From<serde_json::Error> for NativeHighGlbReadbackError {
    fn from(error: serde_json::Error) -> Self {
        Self(format!("NATIVE_HIGH_GLB_JSON_INVALID:{error}"))
    }
}

fn invalid(message: impl Into<String>) -> NativeHighGlbReadbackError {
    NativeHighGlbReadbackError(format!(
        "NATIVE_HIGH_GLB_READBACK_INVALID:{}",
        message.into()
    ))
}

fn u32_at(bytes: &[u8], offset: usize) -> Result<u32, NativeHighGlbReadbackError> {
    let end = offset
        .checked_add(4)
        .ok_or_else(|| invalid("header offset overflow"))?;
    let value = bytes
        .get(offset..end)
        .ok_or_else(|| invalid("header is truncated"))?;
    Ok(u32::from_le_bytes(value.try_into().expect("four bytes")))
}

fn exact_fields(
    object: &Map<String, Value>,
    fields: &[&str],
    context: &str,
) -> Result<(), NativeHighGlbReadbackError> {
    let expected = fields.iter().copied().collect::<BTreeSet<_>>();
    let actual = object.keys().map(String::as_str).collect::<BTreeSet<_>>();
    if actual != expected {
        return Err(invalid(format!("{context} fields differ")));
    }
    Ok(())
}

fn string_field<'a>(
    object: &'a Map<String, Value>,
    key: &str,
) -> Result<&'a str, NativeHighGlbReadbackError> {
    object
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| invalid(format!("{key} is missing")))
}

fn id_field<'a>(
    object: &'a Map<String, Value>,
    key: &str,
) -> Result<&'a str, NativeHighGlbReadbackError> {
    let value = string_field(object, key)?;
    if value.len() > 128
        || !value.bytes().enumerate().all(|(index, byte)| {
            byte.is_ascii_alphanumeric() || (index > 0 && b"_.:-".contains(&byte))
        })
    {
        return Err(invalid(format!("{key} is not a bounded identifier")));
    }
    Ok(value)
}

fn sha_field<'a>(
    object: &'a Map<String, Value>,
    key: &str,
) -> Result<&'a str, NativeHighGlbReadbackError> {
    let value = string_field(object, key)?;
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(invalid(format!("{key} is not a lowercase SHA-256")));
    }
    Ok(value)
}

fn usize_field(
    object: &Map<String, Value>,
    key: &str,
) -> Result<usize, NativeHighGlbReadbackError> {
    usize::try_from(
        object
            .get(key)
            .and_then(Value::as_u64)
            .ok_or_else(|| invalid(format!("{key} is not an integer")))?,
    )
    .map_err(|_| invalid(format!("{key} is too large")))
}

fn bool_field(
    object: &Map<String, Value>,
    key: &str,
    expected: bool,
) -> Result<(), NativeHighGlbReadbackError> {
    if object.get(key) != Some(&Value::Bool(expected)) {
        return Err(invalid(format!("{key} differs")));
    }
    Ok(())
}

fn reject_external(value: &Value) -> Result<(), NativeHighGlbReadbackError> {
    match value {
        Value::Array(values) => values.iter().try_for_each(reject_external),
        Value::Object(object) => {
            for (key, child) in object {
                match key.as_str() {
                    "uri" | "path" | "script" => {
                        return Err(invalid(format!("external field {key} is forbidden")))
                    }
                    "scripts" | "external_uri" if child != &Value::Bool(false) => {
                        return Err(invalid(format!("external field {key} is enabled")))
                    }
                    _ => {}
                }
                reject_external(child)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn array<'a>(
    object: &'a Map<String, Value>,
    key: &str,
) -> Result<&'a [Value], NativeHighGlbReadbackError> {
    object
        .get(key)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| invalid(format!("{key} is not an array")))
}

fn unique_strings(
    values: &[Value],
    context: &str,
) -> Result<Vec<String>, NativeHighGlbReadbackError> {
    let mut seen = BTreeSet::new();
    let mut output = Vec::with_capacity(values.len());
    for value in values {
        let text = value
            .as_str()
            .ok_or_else(|| invalid(format!("{context} has a non-string id")))?;
        if text.is_empty() || !seen.insert(text.to_owned()) {
            return Err(invalid(format!(
                "{context} contains a duplicate or empty id"
            )));
        }
        output.push(text.to_owned());
    }
    Ok(output)
}

fn number(object: &Map<String, Value>, key: &str) -> Result<u64, NativeHighGlbReadbackError> {
    object
        .get(key)
        .and_then(Value::as_u64)
        .ok_or_else(|| invalid(format!("{key} is not an unsigned integer")))
}

fn object<'a>(
    value: &'a Value,
    context: &str,
) -> Result<&'a Map<String, Value>, NativeHighGlbReadbackError> {
    value
        .as_object()
        .ok_or_else(|| invalid(format!("{context} is not an object")))
}

fn checked_range(
    offset: usize,
    length: usize,
    total: usize,
    context: &str,
) -> Result<(), NativeHighGlbReadbackError> {
    let end = offset
        .checked_add(length)
        .ok_or_else(|| invalid(format!("{context} overflows")))?;
    if end > total {
        return Err(invalid(format!("{context} is out of bounds")));
    }
    Ok(())
}

fn accessor_bytes<'a>(
    accessors: &[Value],
    views: &[Value],
    binary: &'a [u8],
    accessor_index: usize,
    component_type: u64,
    value_type: &str,
    element_size: usize,
) -> Result<(&'a [u8], usize), NativeHighGlbReadbackError> {
    let accessor = object(
        accessors
            .get(accessor_index)
            .ok_or_else(|| invalid("accessor index is out of bounds"))?,
        "accessor",
    )?;
    let accessor_keys = if value_type == "VEC3" {
        &["bufferView", "componentType", "count", "type", "min", "max"][..]
    } else {
        &["bufferView", "componentType", "count", "type"][..]
    };
    exact_fields(accessor, accessor_keys, "accessor")?;
    if number(accessor, "componentType")? != component_type
        || accessor.get("type").and_then(Value::as_str) != Some(value_type)
    {
        return Err(invalid("accessor component/type differs"));
    }
    let count = usize_field(accessor, "count")?;
    if count == 0 {
        return Err(invalid("accessor count is zero"));
    }
    if value_type == "VEC3" {
        for key in ["min", "max"] {
            let values = accessor
                .get(key)
                .and_then(Value::as_array)
                .ok_or_else(|| invalid("position bounds are missing"))?;
            if values.len() != 3
                || values
                    .iter()
                    .any(|value| value.as_f64().is_none_or(|value| !value.is_finite()))
            {
                return Err(invalid("position bounds are invalid"));
            }
        }
    }
    let view_index = usize_field(accessor, "bufferView")?;
    let view = object(
        views
            .get(view_index)
            .ok_or_else(|| invalid("bufferView index is out of bounds"))?,
        "bufferView",
    )?;
    exact_fields(
        view,
        &["buffer", "byteOffset", "byteLength", "target"],
        "bufferView",
    )?;
    if number(view, "buffer")? != 0
        || number(view, "target")? != if value_type == "SCALAR" { 34963 } else { 34962 }
    {
        return Err(invalid("bufferView binding differs"));
    }
    let offset = usize_field(view, "byteOffset")?;
    let length = usize_field(view, "byteLength")?;
    if offset % 4 != 0
        || length
            != count
                .checked_mul(element_size)
                .ok_or_else(|| invalid("accessor byte length overflows"))?
    {
        return Err(invalid("bufferView alignment/length differs"));
    }
    checked_range(offset, length, binary.len(), "bufferView")?;
    Ok((&binary[offset..offset + length], count))
}

fn parse_strict_json(bytes: &[u8]) -> Result<Value, NativeHighGlbReadbackError> {
    let strict = serde_json::from_slice::<StrictJson>(bytes)?.0;
    Ok(strict)
}

/// Match a JSON accessor bound to the f32 value carried by the GLB payload.
///
/// The Worker writes accessor bounds from `[f32; 3]`, but JSON decoding stores
/// those numbers as `f64`.  A decimal such as `2.02` can therefore be a valid
/// serialization of the payload's `2.0199999809265137f32` even though the
/// decoded `f64`s are not numerically identical.  Round the JSON value through
/// the GLB component type and compare its bits; this admits only the exact f32
/// representation, never an epsilon or a non-finite value.
fn f32_accessor_bound_matches(value: &Value, expected: f32) -> bool {
    let Some(decoded) = value.as_f64() else {
        return false;
    };
    if !decoded.is_finite() {
        return false;
    }
    let round_tripped = decoded as f32;
    round_tripped.is_finite() && round_tripped.to_bits() == expected.to_bits()
}

/// Parse one embedded GLB and return a compact, canonical Runtime readback.
pub(crate) fn inspect_native_high_glb(glb: &[u8]) -> Result<Value, NativeHighGlbReadbackError> {
    inspect_native_high_glb_with_policy(glb, NativeHighGlbPolicy::Legacy)
}

/// Parse the direct V2 High Artifact GLB variant.
///
/// Direct V2 materialization currently has one evaluated base primitive and no
/// detail layer.  Keep that exception at this call site instead of weakening
/// `inspect_native_high_glb`, which is the legacy NativeHigh readback gate and
/// must continue requiring both primitive layers.  All other GLB, lineage and
/// payload checks remain shared with the legacy parser, including recomputing
/// the actual primitive, part and triangle totals from the embedded bytes.
pub(crate) fn inspect_authoring_mesh_v2_high_glb(
    glb: &[u8],
) -> Result<Value, NativeHighGlbReadbackError> {
    inspect_native_high_glb_with_policy(glb, NativeHighGlbPolicy::AuthoringMeshV2)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NativeHighGlbPolicy {
    Legacy,
    AuthoringMeshV2,
}

const V2_STITCHED_SUBDIVISION_POLICY: &str = "forgecad-owned-cpu-catmull-clark-stitched-polygon@2";

fn valid_v2_source_element_ref(value: &str) -> bool {
    if value.is_empty() || value.len() > 128 {
        return false;
    }
    if value == V2_STITCHED_SUBDIVISION_POLICY {
        return true;
    }
    let stable_id = |candidate: &str| {
        !candidate.is_empty()
            && candidate.len() <= 128
            && candidate
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"_.-".contains(&byte))
    };
    if stable_id(value) {
        return true;
    }
    for prefix in [
        "source-vertex:",
        "source-edge:",
        "source-face:",
        "subdivision-step:",
        "boolean-step:",
        "source-revision:",
        "material-zone:",
        "source-node:",
        "source-part:",
    ] {
        if let Some(payload) = value.strip_prefix(prefix) {
            return stable_id(payload);
        }
    }
    if let Some(payload) = value.strip_prefix("source-revision-sha256:") {
        return payload.len() == 64
            && payload
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase());
    }
    if let Some(payload) = value.strip_prefix("source-part-output-sha256:") {
        return payload.len() == 64
            && payload
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase());
    }
    if let Some(payload) = value.strip_prefix("source-part-index:") {
        return !payload.is_empty()
            && payload.bytes().all(|byte| byte.is_ascii_digit())
            && payload.parse::<u32>().is_ok();
    }
    if let Some(payload) = value.strip_prefix("subdivision-level:") {
        return matches!(payload, "1" | "2");
    }
    false
}

fn inspect_native_high_glb_with_policy(
    glb: &[u8],
    policy: NativeHighGlbPolicy,
) -> Result<Value, NativeHighGlbReadbackError> {
    if glb.len() < 28 || glb.len() > MAX_GLB_BYTES || glb.get(..4) != Some(GLB_MAGIC) {
        return Err(invalid("GLB2 header is invalid"));
    }
    if u32_at(glb, 4)? != 2 || usize::try_from(u32_at(glb, 8)?).ok() != Some(glb.len()) {
        return Err(invalid("GLB2 version/length is invalid"));
    }
    let json_len =
        usize::try_from(u32_at(glb, 12)?).map_err(|_| invalid("JSON chunk is too large"))?;
    if glb.get(16..20) != Some(JSON_CHUNK) || json_len == 0 || json_len % 4 != 0 {
        return Err(invalid("JSON chunk is invalid"));
    }
    let json_start = 20usize;
    let json_end = json_start
        .checked_add(json_len)
        .ok_or_else(|| invalid("JSON chunk overflows"))?;
    if json_end.checked_add(8).is_none() || json_end + 8 > glb.len() {
        return Err(invalid("BIN chunk header is truncated"));
    }
    let bin_len =
        usize::try_from(u32_at(glb, json_end)?).map_err(|_| invalid("BIN chunk is too large"))?;
    if glb.get(json_end + 4..json_end + 8) != Some(BIN_CHUNK) || bin_len % 4 != 0 {
        return Err(invalid("BIN chunk is invalid"));
    }
    let bin_start = json_end + 8;
    let bin_end = bin_start
        .checked_add(bin_len)
        .ok_or_else(|| invalid("BIN chunk overflows"))?;
    if bin_end != glb.len() {
        return Err(invalid("GLB has trailing or truncated chunks"));
    }
    let root = parse_strict_json(&glb[json_start..json_end])?;
    reject_external(&root)?;
    let root = object(&root, "GLB root")?;
    let mut root_fields = vec![
        "asset",
        "scene",
        "scenes",
        "nodes",
        "meshes",
        "buffers",
        "bufferViews",
        "accessors",
        "extras",
    ];
    if matches!(policy, NativeHighGlbPolicy::AuthoringMeshV2) {
        root_fields.push("materials");
    }
    exact_fields(root, &root_fields, "GLB root")?;

    let asset = object(
        root.get("asset")
            .ok_or_else(|| invalid("asset is missing"))?,
        "asset",
    )?;
    exact_fields(asset, &["version", "generator", "extras"], "asset")?;
    if asset.get("version").and_then(Value::as_str) != Some("2.0")
        || asset.get("generator").and_then(Value::as_str)
            != Some("ForgeCAD Native High GLB Lowering@1")
    {
        return Err(invalid("asset identity differs"));
    }
    let units = object(
        asset
            .get("extras")
            .ok_or_else(|| invalid("asset units are missing"))?,
        "asset extras",
    )?;
    exact_fields(units, &["unit", "meter", "length"], "asset extras")?;
    if units.get("unit").and_then(Value::as_str) != Some("meter")
        || units.get("length").and_then(Value::as_str) != Some("meter")
        || units.get("meter").and_then(Value::as_f64) != Some(1.0)
    {
        return Err(invalid("GLB units are not meters"));
    }

    let extras = object(
        root.get("extras")
            .ok_or_else(|| invalid("root extras are missing"))?,
        "root extras",
    )?;
    exact_fields(extras, &["forgecad"], "root extras")?;
    let forgecad = object(
        extras
            .get("forgecad")
            .ok_or_else(|| invalid("ForgeCAD extras are missing"))?,
        "ForgeCAD extras",
    )?;
    exact_fields(
        forgecad,
        &[
            "schema_version",
            "source_schema_version",
            "source_artifact_id",
            "source_artifact_sha256",
            "part_ids",
            "material_zone_ids",
            "base_primitive_count",
            "detail_primitive_count",
            "base_triangle_count",
            "detail_triangle_count",
            "triangle_count",
            "units",
            "embedded_only",
            "external_uri",
            "scripts",
            "primitive_lineage",
        ],
        "ForgeCAD extras",
    )?;
    if forgecad.get("schema_version").and_then(Value::as_str) != Some(GLB_SCHEMA)
        || forgecad
            .get("source_schema_version")
            .and_then(Value::as_str)
            != Some("HighMeshArtifact@1")
    {
        return Err(invalid("source GLB schema differs"));
    }
    let source_artifact_id = id_field(forgecad, "source_artifact_id")?.to_owned();
    let source_artifact_sha256 = sha_field(forgecad, "source_artifact_sha256")?.to_owned();
    bool_field(forgecad, "embedded_only", true)?;
    bool_field(forgecad, "external_uri", false)?;
    bool_field(forgecad, "scripts", false)?;
    let forgecad_units = object(
        forgecad
            .get("units")
            .ok_or_else(|| invalid("ForgeCAD units are missing"))?,
        "ForgeCAD units",
    )?;
    exact_fields(forgecad_units, &["length", "meter"], "ForgeCAD units")?;
    if forgecad_units.get("length").and_then(Value::as_str) != Some("meter")
        || forgecad_units.get("meter").and_then(Value::as_f64) != Some(1.0)
    {
        return Err(invalid("ForgeCAD unit metadata differs"));
    }
    let part_ids = unique_strings(array(forgecad, "part_ids")?, "part_ids")?;
    let material_zone_ids =
        unique_strings(array(forgecad, "material_zone_ids")?, "material_zone_ids")?;
    if matches!(policy, NativeHighGlbPolicy::AuthoringMeshV2) {
        let materials = array(root, "materials")?;
        if materials.len() != material_zone_ids.len() {
            return Err(invalid("V2 material inventory count differs"));
        }
        for (index, material) in materials.iter().enumerate() {
            let material = object(material, "material")?;
            exact_fields(material, &["name", "pbrMetallicRoughness"], "material")?;
            if material.get("name").and_then(Value::as_str)
                != material_zone_ids.get(index).map(String::as_str)
            {
                return Err(invalid(
                    "V2 material name/order differs from zone inventory",
                ));
            }
            let pbr = object(
                material
                    .get("pbrMetallicRoughness")
                    .ok_or_else(|| invalid("V2 neutral PBR material is missing"))?,
                "pbrMetallicRoughness",
            )?;
            exact_fields(
                pbr,
                &["baseColorFactor", "metallicFactor", "roughnessFactor"],
                "pbrMetallicRoughness",
            )?;
            if pbr.get("baseColorFactor") != Some(&serde_json::json!([0.5, 0.5, 0.5, 1.0]))
                || pbr.get("metallicFactor").and_then(Value::as_f64) != Some(0.0)
                || pbr.get("roughnessFactor").and_then(Value::as_f64) != Some(0.7)
            {
                return Err(invalid("V2 neutral transport material differs"));
            }
        }
    }
    let base_count = usize_field(forgecad, "base_primitive_count")?;
    let detail_count = usize_field(forgecad, "detail_primitive_count")?;
    if part_ids.is_empty()
        || material_zone_ids.is_empty()
        || base_count == 0
        || (matches!(policy, NativeHighGlbPolicy::Legacy) && detail_count == 0)
        || (matches!(policy, NativeHighGlbPolicy::AuthoringMeshV2) && detail_count != 0)
        || part_ids.len() != base_count
    {
        return Err(invalid("base/detail primitive counts must be positive"));
    }
    let total_primitives = base_count
        .checked_add(detail_count)
        .ok_or_else(|| invalid("primitive count overflows"))?;

    let buffers = array(root, "buffers")?;
    if buffers.len() != 1 {
        return Err(invalid("GLB must contain exactly one buffer"));
    }
    let buffer = object(&buffers[0], "buffer")?;
    exact_fields(buffer, &["byteLength"], "buffer")?;
    if usize_field(buffer, "byteLength")? != bin_len {
        return Err(invalid("embedded buffer length differs"));
    }
    let nodes = array(root, "nodes")?;
    let meshes = array(root, "meshes")?;
    let views = array(root, "bufferViews")?;
    let accessors = array(root, "accessors")?;
    let lineage = array(forgecad, "primitive_lineage")?;
    if nodes.len() != total_primitives
        || meshes.len() != total_primitives
        || lineage.len() != total_primitives
    {
        return Err(invalid("primitive/node/lineage counts differ"));
    }
    if root.get("scene").and_then(Value::as_u64) != Some(0) {
        return Err(invalid("default scene differs"));
    }
    let scenes = array(root, "scenes")?;
    if scenes.len() != 1 {
        return Err(invalid("scene count differs"));
    }
    let scene = object(&scenes[0], "scene")?;
    exact_fields(scene, &["nodes"], "scene")?;
    let scene_nodes = array(scene, "nodes")?;
    if scene_nodes.len() != total_primitives
        || scene_nodes
            .iter()
            .enumerate()
            .any(|(index, value)| value.as_u64() != Some(index as u64))
    {
        return Err(invalid("scene node binding differs"));
    }

    // V2 High is a source transport for Low retopology, not a baked surface.
    // Its contract requires POSITION/NORMAL/TEXCOORD_0, while TANGENT is
    // intentionally absent until the Low compiler's MikkTSpace pass. The
    // legacy Native High path remains POSITION-only and uses the historical
    // two-accessor layout below.
    let require_surface_attributes = matches!(policy, NativeHighGlbPolicy::AuthoringMeshV2);
    let accessors_per_primitive = if require_surface_attributes { 4 } else { 2 };
    let expected_accessor_count = total_primitives
        .checked_mul(accessors_per_primitive)
        .ok_or_else(|| invalid("accessor/bufferView count overflows"))?;
    if views.len() != expected_accessor_count || accessors.len() != expected_accessor_count {
        return Err(invalid("accessor/bufferView count differs"));
    }
    let mut ranges = Vec::<(usize, usize)>::new();
    let mut accessor_refs = BTreeSet::new();
    let mut names = BTreeSet::new();
    let mut source_node_ids = Vec::with_capacity(base_count);
    let mut source_node_id_set = BTreeSet::new();
    let mut used_material_zone_ids = BTreeSet::new();
    let mut primitive_bindings = Vec::with_capacity(base_count);
    let mut base_triangles = 0u64;
    let mut detail_triangles = 0u64;
    for index in 0..total_primitives {
        let line = object(&lineage[index], "primitive lineage")?;
        let mut lineage_fields = vec![
            "source_schema_version",
            "source_artifact_id",
            "source_artifact_sha256",
            "primitive_id",
            "kind",
            "part_id",
            "source_node_id",
            "material_zone_id",
            "source_element_lineage",
            "position_count",
            "triangle_count",
        ];
        if matches!(policy, NativeHighGlbPolicy::AuthoringMeshV2) {
            lineage_fields.push("source_node_ids");
        }
        exact_fields(line, &lineage_fields, "primitive lineage")?;
        if line.get("source_schema_version").and_then(Value::as_str) != Some("HighMeshArtifact@1")
            || line.get("source_artifact_id").and_then(Value::as_str)
                != Some(source_artifact_id.as_str())
            || line.get("source_artifact_sha256").and_then(Value::as_str)
                != Some(source_artifact_sha256.as_str())
        {
            return Err(invalid("primitive source lineage differs"));
        }
        let primitive_id = id_field(line, "primitive_id")?.to_owned();
        let part_id = id_field(line, "part_id")?.to_owned();
        let kind = string_field(line, "kind")?;
        if index < base_count {
            let expected_base_kind = match policy {
                NativeHighGlbPolicy::Legacy => "authoring_base",
                NativeHighGlbPolicy::AuthoringMeshV2 => "authoring_mesh_v2_high_evaluated",
            };
            if kind != expected_base_kind
                || part_ids.get(index).map(String::as_str) != Some(part_id.as_str())
            {
                return Err(invalid("base primitive kind/Part order differs"));
            }
        } else if !matches!(
            kind,
            "support_loop_patch" | "crease_metadata" | "floating_detail_box"
        ) {
            return Err(invalid("detail primitive kind is invalid"));
        }
        let source_node_id = id_field(line, "source_node_id")?.to_owned();
        let primitive_source_node_ids = if matches!(policy, NativeHighGlbPolicy::AuthoringMeshV2) {
            let values = unique_strings(array(line, "source_node_ids")?, "source_node_ids")?;
            if values.first() != Some(&source_node_id) {
                return Err(invalid("V2 source node owner differs from source node set"));
            }
            values
        } else {
            vec![source_node_id.clone()]
        };
        let material_zone_id = id_field(line, "material_zone_id")?.to_owned();
        if matches!(policy, NativeHighGlbPolicy::AuthoringMeshV2)
            && !material_zone_ids.contains(&material_zone_id)
        {
            return Err(invalid(
                "V2 material zone binding is not declared in the root inventory",
            ));
        }
        for primitive_source_node_id in &primitive_source_node_ids {
            if source_node_id_set.insert(primitive_source_node_id.clone()) {
                source_node_ids.push(primitive_source_node_id.clone());
            }
        }
        used_material_zone_ids.insert(material_zone_id.clone());
        let source_lineage = array(line, "source_element_lineage")?;
        let lineage_ids = unique_strings(source_lineage, "source_element_lineage")?;
        let invalid_lineage = match policy {
            NativeHighGlbPolicy::Legacy => lineage_ids.iter().any(|value| {
                !value
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"_.:-".contains(&byte))
            }),
            NativeHighGlbPolicy::AuthoringMeshV2 => lineage_ids
                .iter()
                .any(|value| !valid_v2_source_element_ref(value)),
        };
        if invalid_lineage {
            return Err(invalid("source element lineage contains an invalid id"));
        }
        if !names.insert(primitive_id.clone()) || !part_ids.contains(&part_id) {
            return Err(invalid("primitive id or Part lineage is duplicate/unknown"));
        }
        let line_vertices = usize_field(line, "position_count")?;
        let line_triangles = u64::try_from(usize_field(line, "triangle_count")?)
            .map_err(|_| invalid("lineage triangle count is too large"))?;
        if matches!(policy, NativeHighGlbPolicy::AuthoringMeshV2) && index < base_count {
            primitive_bindings.push(json!({
                "part_id": part_id,
                "source_node_id": source_node_id,
                "source_node_ids": primitive_source_node_ids,
                "material_zone_id": material_zone_id,
                "triangle_count": line_triangles
            }));
        }
        let mesh = object(&meshes[index], "mesh")?;
        exact_fields(mesh, &["name", "primitives", "extras"], "mesh")?;
        let name = id_field(mesh, "name")?;
        let expected_name = if index < base_count {
            part_id.as_str()
        } else {
            primitive_id.as_str()
        };
        if name != expected_name || !names.insert(format!("mesh:{name}")) {
            return Err(invalid("mesh name is duplicate or differs"));
        }
        if object(
            mesh.get("extras")
                .ok_or_else(|| invalid("mesh extras missing"))?,
            "mesh extras",
        )? != line
        {
            return Err(invalid("mesh extras lineage differs"));
        }
        let node = object(&nodes[index], "node")?;
        exact_fields(node, &["name", "mesh", "extras"], "node")?;
        if node.get("name").and_then(Value::as_str) != Some(name)
            || node.get("mesh").and_then(Value::as_u64) != Some(index as u64)
            || object(
                node.get("extras")
                    .ok_or_else(|| invalid("node extras missing"))?,
                "node extras",
            )? != line
        {
            return Err(invalid("node binding/lineage differs"));
        }
        let primitives = array(mesh, "primitives")?;
        if primitives.len() != 1 {
            return Err(invalid("each mesh must contain one primitive"));
        }
        let primitive = object(&primitives[0], "primitive")?;
        let primitive_fields = if require_surface_attributes {
            &["attributes", "indices", "mode", "material", "extras"][..]
        } else {
            &["attributes", "indices", "mode", "extras"][..]
        };
        exact_fields(primitive, primitive_fields, "primitive")?;
        if primitive.get("mode").and_then(Value::as_u64) != Some(4)
            || object(
                primitive
                    .get("extras")
                    .ok_or_else(|| invalid("primitive extras missing"))?,
                "primitive extras",
            )? != line
        {
            return Err(invalid("primitive mode/lineage differs"));
        }
        if require_surface_attributes {
            let expected_material = material_zone_ids
                .iter()
                .position(|zone| zone == &material_zone_id)
                .ok_or_else(|| invalid("V2 primitive material zone is undeclared"))?;
            if usize_field(primitive, "material")? != expected_material {
                return Err(invalid(
                    "V2 primitive material index differs from zone binding",
                ));
            }
        }
        let attributes = object(
            primitive
                .get("attributes")
                .ok_or_else(|| invalid("primitive attributes missing"))?,
            "attributes",
        )?;
        let attribute_fields = if require_surface_attributes {
            &["POSITION", "NORMAL", "TEXCOORD_0"][..]
        } else {
            &["POSITION"][..]
        };
        exact_fields(attributes, attribute_fields, "attributes")?;
        let position_accessor = usize_field(attributes, "POSITION")?;
        let normal_accessor = if require_surface_attributes {
            Some(usize_field(attributes, "NORMAL")?)
        } else {
            None
        };
        let uv_accessor = if require_surface_attributes {
            Some(usize_field(attributes, "TEXCOORD_0")?)
        } else {
            None
        };
        let index_accessor = usize_field(primitive, "indices")?;
        let primitive_accessors = [
            Some(position_accessor),
            normal_accessor,
            uv_accessor,
            Some(index_accessor),
        ];
        if primitive_accessors
            .iter()
            .flatten()
            .collect::<BTreeSet<_>>()
            .len()
            != primitive_accessors.iter().flatten().count()
        {
            return Err(invalid("primitive accessor is duplicated"));
        }
        if primitive_accessors
            .iter()
            .flatten()
            .any(|accessor| !accessor_refs.insert(*accessor))
        {
            return Err(invalid("accessor is referenced more than once"));
        }
        let (positions, position_count) = accessor_bytes(
            &accessors,
            &views,
            &glb[bin_start..bin_end],
            position_accessor,
            5126,
            "VEC3",
            12,
        )?;
        let (indices, index_count) = accessor_bytes(
            &accessors,
            &views,
            &glb[bin_start..bin_end],
            index_accessor,
            5125,
            "SCALAR",
            4,
        )?;
        if let (Some(normal_accessor), Some(uv_accessor)) = (normal_accessor, uv_accessor) {
            let (normals, normal_count) = accessor_bytes(
                &accessors,
                &views,
                &glb[bin_start..bin_end],
                normal_accessor,
                5126,
                "VEC3",
                12,
            )?;
            let (uvs, uv_count) = accessor_bytes(
                &accessors,
                &views,
                &glb[bin_start..bin_end],
                uv_accessor,
                5126,
                "VEC2",
                8,
            )?;
            if normal_count != position_count || uv_count != position_count {
                return Err(invalid("surface attribute count differs from positions"));
            }
            for chunk in normals.chunks_exact(12) {
                let values = [
                    f32::from_le_bytes(chunk[0..4].try_into().expect("normal x")),
                    f32::from_le_bytes(chunk[4..8].try_into().expect("normal y")),
                    f32::from_le_bytes(chunk[8..12].try_into().expect("normal z")),
                ];
                let length = values.iter().map(|value| value * value).sum::<f32>().sqrt();
                if values.iter().any(|value| !value.is_finite())
                    || !length.is_finite()
                    || !(0.999..=1.001).contains(&length)
                {
                    return Err(invalid("NORMAL payload is not a finite unit vector"));
                }
            }
            for chunk in uvs.chunks_exact(8) {
                let values = [
                    f32::from_le_bytes(chunk[0..4].try_into().expect("UV u")),
                    f32::from_le_bytes(chunk[4..8].try_into().expect("UV v")),
                ];
                if values
                    .iter()
                    .any(|value| !value.is_finite() || !(0.0..=1.0).contains(value))
                {
                    return Err(invalid("TEXCOORD_0 payload is outside [0,1]"));
                }
            }
        }
        if position_count != line_vertices
            || index_count % 3 != 0
            || u64::try_from(index_count / 3).ok() != Some(line_triangles)
        {
            return Err(invalid("primitive counts differ"));
        }
        let mut position_values = Vec::with_capacity(position_count);
        for chunk in positions.chunks_exact(12) {
            let xyz = [
                f32::from_le_bytes(chunk[0..4].try_into().expect("position x")),
                f32::from_le_bytes(chunk[4..8].try_into().expect("position y")),
                f32::from_le_bytes(chunk[8..12].try_into().expect("position z")),
            ];
            if xyz.iter().any(|value| !value.is_finite()) {
                return Err(invalid("position payload is non-finite"));
            }
            position_values.push(xyz);
        }
        let position_accessor_value = object(&accessors[position_accessor], "position accessor")?;
        for (bound_key, component) in [("min", 0usize), ("max", 1usize)] {
            let bound = position_accessor_value
                .get(bound_key)
                .and_then(Value::as_array)
                .ok_or_else(|| invalid("position accessor bounds are missing"))?;
            let mut expected = [if component == 0 {
                f32::INFINITY
            } else {
                f32::NEG_INFINITY
            }; 3];
            for position in &position_values {
                for axis in 0..3 {
                    expected[axis] = if component == 0 {
                        expected[axis].min(position[axis])
                    } else {
                        expected[axis].max(position[axis])
                    };
                }
            }
            if bound.len() != 3
                || bound
                    .iter()
                    .enumerate()
                    .any(|(axis, value)| !f32_accessor_bound_matches(value, expected[axis]))
            {
                return Err(invalid("position accessor bounds differ from payload"));
            }
        }
        for chunk in indices.chunks_exact(4) {
            let value = u32::from_le_bytes(chunk.try_into().expect("index"));
            if usize::try_from(value)
                .ok()
                .is_none_or(|value| value >= position_count)
            {
                return Err(invalid("triangle index is out of bounds"));
            }
        }
        let mut ranges_for_primitive = vec![
            view_range(&accessors, &views, position_accessor)?,
            view_range(&accessors, &views, index_accessor)?,
        ];
        if let Some(accessor) = normal_accessor {
            ranges_for_primitive.push(view_range(&accessors, &views, accessor)?);
        }
        if let Some(accessor) = uv_accessor {
            ranges_for_primitive.push(view_range(&accessors, &views, accessor)?);
        }
        for range in ranges_for_primitive {
            if ranges
                .iter()
                .any(|other| other.0 < range.1 && range.0 < other.1)
            {
                return Err(invalid("bufferView ranges overlap or repeat"));
            }
            ranges.push(range);
        }
        if index < base_count {
            base_triangles = base_triangles
                .checked_add(line_triangles)
                .ok_or_else(|| invalid("triangle count overflows"))?;
        } else {
            detail_triangles = detail_triangles
                .checked_add(line_triangles)
                .ok_or_else(|| invalid("triangle count overflows"))?;
        }
    }
    let triangle_count = base_triangles
        .checked_add(detail_triangles)
        .ok_or_else(|| invalid("triangle count overflows"))?;
    if number(forgecad, "base_triangle_count")? != base_triangles
        || number(forgecad, "detail_triangle_count")? != detail_triangles
        || number(forgecad, "triangle_count")? != triangle_count
    {
        return Err(invalid("triangle totals differ"));
    }
    if matches!(policy, NativeHighGlbPolicy::AuthoringMeshV2)
        && material_zone_ids.iter().cloned().collect::<BTreeSet<_>>() != used_material_zone_ids
    {
        return Err(invalid(
            "V2 material zone inventory differs from primitive bindings",
        ));
    }

    let mut result = serde_json::json!({
        "schema_version": READBACK_SCHEMA,
        "glb_sha256": sha256_hex(glb),
        "source_artifact_id": source_artifact_id,
        "source_artifact_sha256": source_artifact_sha256,
        "part_ids": part_ids,
        "base_primitive_count": base_count,
        "detail_primitive_count": detail_count,
        "base_triangle_count": base_triangles,
        "detail_triangle_count": detail_triangles,
        "triangle_count": triangle_count,
        "byte_length": glb.len(),
        "canonical_sha256": ""
    });
    if matches!(policy, NativeHighGlbPolicy::AuthoringMeshV2) {
        // Direct V2 readback exposes unique inventories, matching the strict
        // artifact contract.  The primitive lineage above remains the
        // per-primitive binding source of truth.
        result["source_node_ids"] = json!(source_node_ids);
        result["material_zone_ids"] = json!(material_zone_ids);
        result["primitive_bindings"] = json!(primitive_bindings);
    }
    result["canonical_sha256"] = Value::String(canonical_json_hash(&result));
    Ok(result)
}

fn view_range(
    accessors: &[Value],
    views: &[Value],
    accessor_index: usize,
) -> Result<(usize, usize), NativeHighGlbReadbackError> {
    let accessor = object(
        accessors
            .get(accessor_index)
            .ok_or_else(|| invalid("accessor index is out of bounds"))?,
        "accessor",
    )?;
    let view_index = usize_field(accessor, "bufferView")?;
    let view = object(
        views
            .get(view_index)
            .ok_or_else(|| invalid("bufferView index is out of bounds"))?,
        "view",
    )?;
    let offset = usize_field(view, "byteOffset")?;
    let length = usize_field(view, "byteLength")?;
    Ok((
        offset,
        offset
            .checked_add(length)
            .ok_or_else(|| invalid("view range overflows"))?,
    ))
}

/// Compare the local readback with the worker's strict readback envelope.
/// The worker readback is intentionally accepted only as a claim about the
/// same compact fields; the local parser remains the source of truth.
pub(crate) fn validate_against_worker_readback(
    readback: &Value,
    worker_readback: &Value,
) -> Result<(), NativeHighGlbReadbackError> {
    let local = object(readback, "local readback")?;
    let worker = object(worker_readback, "worker readback")?;
    exact_fields(
        worker,
        &[
            "glb_sha256",
            "source_artifact_id",
            "source_artifact_sha256",
            "part_ids",
            "base_primitive_count",
            "detail_primitive_count",
            "base_triangle_count",
            "detail_triangle_count",
            "triangle_count",
            "byte_length",
        ],
        "worker readback",
    )?;
    for key in [
        "glb_sha256",
        "source_artifact_id",
        "source_artifact_sha256",
        "part_ids",
        "base_primitive_count",
        "detail_primitive_count",
        "base_triangle_count",
        "detail_triangle_count",
        "triangle_count",
        "byte_length",
    ] {
        if local.get(key) != worker.get(key) {
            return Err(invalid(format!("worker readback field {key} differs")));
        }
    }
    Ok(())
}

/// Convenience form used by Runtime call sites that have not yet retained
/// the local readback value.
pub(crate) fn inspect_and_validate_against_worker_readback(
    glb: &[u8],
    worker_readback: &Value,
) -> Result<Value, NativeHighGlbReadbackError> {
    let readback = inspect_native_high_glb(glb)?;
    validate_against_worker_readback(&readback, worker_readback)?;
    Ok(readback)
}

// serde_json's default Value deserializer silently keeps the last duplicate
// object key.  GLB JSON is a signed/readback boundary, so use a tiny visitor
// that rejects duplicates before materializing Value.
struct StrictJson(Value);

impl<'de> Deserialize<'de> for StrictJson {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct StrictVisitor;
        impl<'de> Visitor<'de> for StrictVisitor {
            type Value = StrictJson;
            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("strict JSON value")
            }
            fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E> {
                Ok(StrictJson(Value::Bool(value)))
            }
            fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(StrictJson(Value::from(value)))
            }
            fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(StrictJson(Value::from(value)))
            }
            fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(StrictJson(
                    serde_json::Number::from_f64(value)
                        .ok_or_else(|| E::custom("non-finite number"))?
                        .into(),
                ))
            }
            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(StrictJson(Value::String(value.to_owned())))
            }
            fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(StrictJson(Value::String(value)))
            }
            fn visit_none<E>(self) -> Result<Self::Value, E> {
                Ok(StrictJson(Value::Null))
            }
            fn visit_unit<E>(self) -> Result<Self::Value, E> {
                Ok(StrictJson(Value::Null))
            }
            fn visit_seq<A>(self, mut access: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let mut values = Vec::new();
                while let Some(value) = access.next_element::<StrictJson>()? {
                    values.push(value.0);
                }
                Ok(StrictJson(Value::Array(values)))
            }
            fn visit_map<A>(self, mut access: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut values = Map::new();
                while let Some(key) = access.next_key::<String>()? {
                    if values.contains_key(&key) {
                        return Err(de::Error::custom(format!("duplicate JSON key {key}")));
                    }
                    let value = access.next_value::<StrictJson>()?;
                    values.insert(key, value.0);
                }
                Ok(StrictJson(Value::Object(values)))
            }
        }
        deserializer.deserialize_any(StrictVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn v2_glb_fixture(edit_forgecad: impl FnOnce(&mut Value)) -> Vec<u8> {
        let source_artifact_sha256 = "a".repeat(64);
        let source_artifact_id = format!("high-mesh-{}", &source_artifact_sha256[..24]);
        let lineage = json!({
            "source_schema_version": "HighMeshArtifact@1",
            "source_artifact_id": source_artifact_id,
            "source_artifact_sha256": source_artifact_sha256,
            "primitive_id": "primitive-blade-body",
            "kind": "authoring_mesh_v2_high_evaluated",
            "part_id": "blade-body",
            "source_node_id": "node-blade-body",
            "material_zone_id": "blade-steel",
            "source_element_lineage": ["element-0", V2_STITCHED_SUBDIVISION_POLICY],
            "position_count": 3,
            "triangle_count": 1
        });
        let binary = [
            2.02f32.to_le_bytes(),
            (-0.31f32).to_le_bytes(),
            (-0.03f32).to_le_bytes(),
            0.42f32.to_le_bytes(),
            0.0f32.to_le_bytes(),
            0.0f32.to_le_bytes(),
            0.0f32.to_le_bytes(),
            1.01f32.to_le_bytes(),
            0.0f32.to_le_bytes(),
            0u32.to_le_bytes(),
            1u32.to_le_bytes(),
            2u32.to_le_bytes(),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
        let mut root = json!({
            "asset": {
                "version": "2.0",
                "generator": "ForgeCAD Native High GLB Lowering@1",
                "extras": {"unit": "meter", "meter": 1.0, "length": "meter"}
            },
            "scene": 0,
            "scenes": [{"nodes": [0]}],
            "nodes": [{
                "name": "blade-body",
                "mesh": 0,
                "extras": lineage
            }],
            "meshes": [{
                "name": "blade-body",
                "primitives": [{
                    "attributes": {"POSITION": 0},
                    "indices": 1,
                    "mode": 4,
                    "extras": lineage
                }],
                "extras": lineage
            }],
            "buffers": [{"byteLength": binary.len()}],
            "bufferViews": [
                {"buffer": 0, "byteOffset": 0, "byteLength": 36, "target": 34962},
                {"buffer": 0, "byteOffset": 36, "byteLength": 12, "target": 34963}
            ],
            "accessors": [
                {
                    "bufferView": 0,
                    "componentType": 5126,
                    "count": 3,
                    "type": "VEC3",
                    "min": [0.0, -0.31, -0.03],
                    "max": [2.02, 1.01, 0.0]
                },
                {"bufferView": 1, "componentType": 5125, "count": 3, "type": "SCALAR"}
            ],
            "extras": {"forgecad": {
                "schema_version": "HighMeshArtifactGlb@1",
                "source_schema_version": "HighMeshArtifact@1",
                "source_artifact_id": source_artifact_id,
                "source_artifact_sha256": source_artifact_sha256,
                "part_ids": ["blade-body"],
                "material_zone_ids": ["blade-steel"],
                "base_primitive_count": 1,
                "detail_primitive_count": 0,
                "base_triangle_count": 1,
                "detail_triangle_count": 0,
                "triangle_count": 1,
                "units": {"length": "meter", "meter": 1.0},
                "embedded_only": true,
                "external_uri": false,
                "scripts": false,
                "primitive_lineage": [lineage]
            }}
        });
        edit_forgecad(
            root.get_mut("extras")
                .and_then(|extras| extras.get_mut("forgecad"))
                .expect("ForgeCAD fixture extras"),
        );
        let mut json_bytes = serde_json::to_vec(&root).expect("V2 GLB JSON");
        while json_bytes.len() % 4 != 0 {
            json_bytes.push(b' ');
        }
        let total_length = 12 + 8 + json_bytes.len() + 8 + binary.len();
        let mut glb = Vec::with_capacity(total_length);
        glb.extend_from_slice(GLB_MAGIC);
        glb.extend_from_slice(&2u32.to_le_bytes());
        glb.extend_from_slice(&(total_length as u32).to_le_bytes());
        glb.extend_from_slice(&(json_bytes.len() as u32).to_le_bytes());
        glb.extend_from_slice(JSON_CHUNK);
        glb.extend_from_slice(&json_bytes);
        glb.extend_from_slice(&(binary.len() as u32).to_le_bytes());
        glb.extend_from_slice(BIN_CHUNK);
        glb.extend_from_slice(&binary);
        glb
    }

    #[test]
    fn direct_v2_readback_uses_actual_base_and_zero_detail_counts() {
        let glb = v2_glb_fixture(|_| {});
        let readback = inspect_authoring_mesh_v2_high_glb(&glb).expect("valid V2 High GLB");
        assert_eq!(readback["base_primitive_count"], 1);
        assert_eq!(readback["detail_primitive_count"], 0);
        assert_eq!(readback["base_triangle_count"], 1);
        assert_eq!(readback["detail_triangle_count"], 0);
        assert_eq!(readback["triangle_count"], 1);
        assert_eq!(readback["part_ids"], json!(["blade-body"]));
    }

    #[test]
    fn accessor_bounds_round_trip_through_f32_bits_and_reject_non_finite_or_drift() {
        // The fixture uses decimal JSON such as 2.02 while its binary payload
        // carries 2.0199999809265137f32, matching the Worker's f32 serializer.
        inspect_authoring_mesh_v2_high_glb(&v2_glb_fixture(|_| {}))
            .expect("shortest decimal f32 bounds must be accepted");

        assert!(f32_accessor_bound_matches(&json!(2.02), 2.02f32));
        assert!(f32_accessor_bound_matches(&json!(-0.31), -0.31f32));
        assert!(!f32_accessor_bound_matches(&json!(2.020001), 2.02f32));
        assert!(!f32_accessor_bound_matches(&json!("NaN"), 0.0f32));
        assert!(!f32_accessor_bound_matches(&json!("Infinity"), 0.0f32));
        assert!(!f32_accessor_bound_matches(&json!(f64::MAX), 0.0f32));
    }

    #[test]
    fn legacy_readback_keeps_positive_detail_gate_and_v2_rejects_triangle_drift() {
        let glb = v2_glb_fixture(|forgecad| {
            forgecad["base_triangle_count"] = Value::from(2u64);
            forgecad["triangle_count"] = Value::from(2u64);
        });
        let v2_error = inspect_authoring_mesh_v2_high_glb(&glb)
            .expect_err("actual triangle payload must bind to V2 metadata");
        assert!(v2_error.to_string().contains("triangle totals differ"));

        let valid_glb = v2_glb_fixture(|_| {});
        let legacy_error = inspect_native_high_glb(&valid_glb)
            .expect_err("legacy NativeHigh must retain its detail-layer gate");
        assert!(legacy_error
            .to_string()
            .contains("base/detail primitive counts must be positive"));
    }

    #[test]
    fn direct_v2_readback_rejects_detail_layer_wrong_kind_and_lineage_id_drift() {
        let detail_error = inspect_authoring_mesh_v2_high_glb(&v2_glb_fixture(|forgecad| {
            forgecad["detail_primitive_count"] = Value::from(1u64);
        }))
        .expect_err("direct V2 must not admit a detail layer");
        assert!(detail_error
            .to_string()
            .contains("base/detail primitive counts must be positive"));

        let kind_error = inspect_authoring_mesh_v2_high_glb(&v2_glb_fixture(|forgecad| {
            forgecad["primitive_lineage"][0]["kind"] = Value::String("authoring_base".into());
        }))
        .expect_err("direct V2 must reject legacy primitive kinds");
        assert!(kind_error
            .to_string()
            .contains("base primitive kind/Part order differs"));

        let part_error = inspect_authoring_mesh_v2_high_glb(&v2_glb_fixture(|forgecad| {
            forgecad["part_ids"][0] = Value::String("other-part".into());
        }))
        .expect_err("direct V2 must bind the primitive to the declared Part");
        assert!(part_error
            .to_string()
            .contains("base primitive kind/Part order differs"));
    }

    #[test]
    fn v2_source_element_lineage_accepts_only_closed_stable_ref_forms() {
        let sha = "a".repeat(64);
        for valid in [
            "amv2-mesh-b05db075f4be0baccb9c83f102294461b8210dcb1c4e36e1",
            "source-vertex:v-01",
            "source-edge:e-01",
            "source-face:f-01",
            "subdivision-step:step-01",
            "boolean-step:step-01",
            "source-revision:amrev-01",
            &format!("source-revision-sha256:{sha}"),
            "subdivision-level:1",
            V2_STITCHED_SUBDIVISION_POLICY,
        ] {
            assert!(valid_v2_source_element_ref(valid), "accepted form: {valid}");
        }
        for invalid in [
            "source-edge:e/01",
            "source-edge:e 01",
            "source-edge:e@01",
            "source-revision-sha256:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
            "subdivision-level:0",
            "subdivision-level:3",
            "forgecad-owned-cpu-catmull-clark-stitched-polygon@3",
            "arbitrary-policy@2",
            "source-edge:",
            "source-edge:e-01\n",
        ] {
            assert!(!valid_v2_source_element_ref(invalid), "rejected form: {invalid:?}");
        }
    }
}
