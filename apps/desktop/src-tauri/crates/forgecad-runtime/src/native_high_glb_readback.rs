//! Strict, local readback for the Native High embedded GLB.
//!
//! This module deliberately has no Runtime, Store, Worker, filesystem, or
//! network dependency.  The Worker supplies a compact readback, but Runtime
//! still parses the bytes locally before accepting that assertion.

use super::{canonical_json_hash, sha256_hex};
use serde::de::{self, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer};
use serde_json::{Map, Value};
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
        || number(view, "target")? != if value_type == "VEC3" { 34962 } else { 34963 }
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

/// Parse one embedded GLB and return a compact, canonical Runtime readback.
pub(crate) fn inspect_native_high_glb(glb: &[u8]) -> Result<Value, NativeHighGlbReadbackError> {
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
    exact_fields(
        root,
        &[
            "asset",
            "scene",
            "scenes",
            "nodes",
            "meshes",
            "buffers",
            "bufferViews",
            "accessors",
            "extras",
        ],
        "GLB root",
    )?;

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
    let base_count = usize_field(forgecad, "base_primitive_count")?;
    let detail_count = usize_field(forgecad, "detail_primitive_count")?;
    if part_ids.is_empty()
        || material_zone_ids.is_empty()
        || base_count == 0
        || detail_count == 0
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

    if views.len() != total_primitives * 2 || accessors.len() != total_primitives * 2 {
        return Err(invalid("accessor/bufferView count differs"));
    }
    let mut ranges = Vec::<(usize, usize)>::new();
    let mut accessor_refs = BTreeSet::new();
    let mut names = BTreeSet::new();
    let mut base_triangles = 0u64;
    let mut detail_triangles = 0u64;
    for index in 0..total_primitives {
        let line = object(&lineage[index], "primitive lineage")?;
        exact_fields(
            line,
            &[
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
            ],
            "primitive lineage",
        )?;
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
            if kind != "authoring_base"
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
        let _source_node_id = id_field(line, "source_node_id")?;
        let _material_zone_id = id_field(line, "material_zone_id")?;
        let source_lineage = array(line, "source_element_lineage")?;
        let lineage_ids = unique_strings(source_lineage, "source_element_lineage")?;
        if lineage_ids.iter().any(|value| {
            !value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"_.:-".contains(&byte))
        }) {
            return Err(invalid("source element lineage contains an invalid id"));
        }
        if !names.insert(primitive_id.clone()) || !part_ids.contains(&part_id) {
            return Err(invalid("primitive id or Part lineage is duplicate/unknown"));
        }
        let line_vertices = usize_field(line, "position_count")?;
        let line_triangles = u64::try_from(usize_field(line, "triangle_count")?)
            .map_err(|_| invalid("lineage triangle count is too large"))?;
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
        exact_fields(
            primitive,
            &["attributes", "indices", "mode", "extras"],
            "primitive",
        )?;
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
        let attributes = object(
            primitive
                .get("attributes")
                .ok_or_else(|| invalid("primitive attributes missing"))?,
            "attributes",
        )?;
        exact_fields(attributes, &["POSITION"], "attributes")?;
        let position_accessor = usize_field(attributes, "POSITION")?;
        let index_accessor = usize_field(primitive, "indices")?;
        if position_accessor == index_accessor {
            return Err(invalid("position/index accessor is duplicated"));
        }
        if !accessor_refs.insert(position_accessor) || !accessor_refs.insert(index_accessor) {
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
                    .any(|(axis, value)| value.as_f64() != Some(f64::from(expected[axis])))
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
        let range_a = view_range(&accessors, &views, position_accessor)?;
        let range_b = view_range(&accessors, &views, index_accessor)?;
        for range in [range_a, range_b] {
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
    if number(forgecad, "base_triangle_count")? != base_triangles
        || number(forgecad, "detail_triangle_count")? != detail_triangles
        || number(forgecad, "triangle_count")? != base_triangles + detail_triangles
    {
        return Err(invalid("triangle totals differ"));
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
        "triangle_count": base_triangles + detail_triangles,
        "byte_length": glb.len(),
        "canonical_sha256": ""
    });
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
